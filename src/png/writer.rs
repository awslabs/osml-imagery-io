//! PNGDatasetWriter — implements DatasetWriter for PNG files.
//!
//! Encodes pixel data from an `ImageAssetProvider` into a valid PNG file.
//! The writer queues a single image asset via `add_asset`, stores optional
//! metadata via `set_metadata`, and performs all encoding work in `close()`.
//!
//! During `close()`:
//! 1. Reads all pixel data from the queued provider via `get_block(0, 0, 0, None)`
//! 2. Converts BSQ (band-sequential) to interleaved row-major format
//! 3. Determines PNG color type from num_bands and pixel_type
//! 4. Encodes via the `png` crate
//! 5. Writes tEXt chunks from metadata
//! 6. Flushes to disk

use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetWriter, MetadataProvider};
use crate::types::{AssetType, PixelType};

/// Metadata keys that are not written as tEXt chunks (they control encoding
/// or represent structural properties).
const SKIP_TEXT_KEYS: &[&str] = &[
    "width",
    "height",
    "bit_depth",
    "color_type",
    "PLTE",
    "gAMA",
    "pHYs",
    "tIME",
];

/// An image asset queued for writing.
struct QueuedPngAsset {
    provider: AssetProvider,
    #[allow(dead_code)]
    key: String,
}

/// Writer for PNG datasets implementing the `DatasetWriter` trait.
///
/// Queues a single image asset and optional metadata, then encodes
/// everything into a valid PNG file on `close()`.
pub struct PNGDatasetWriter {
    /// Output file path.
    path: PathBuf,
    /// Whether an image asset has been queued.
    image_queued: bool,
    /// Dataset-level metadata (encoding hints, tEXt source).
    metadata: Option<Arc<dyn MetadataProvider>>,
    /// Whether `close()` has been called.
    closed: bool,
    /// Queued assets (at most one).
    assets: Vec<QueuedPngAsset>,
}

impl PNGDatasetWriter {
    /// Create a new writer targeting the given output path.
    ///
    /// No file is opened until `close()` is called.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CodecError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            image_queued: false,
            metadata: None,
            closed: false,
            assets: Vec::new(),
        })
    }

    /// Determine the PNG color type from band count and optional metadata hints.
    ///
    /// Returns `(png::ColorType, png::BitDepth)`.
    fn determine_color_config(
        num_bands: u32,
        pixel_type: PixelType,
        metadata: Option<&dyn MetadataProvider>,
    ) -> Result<(png::ColorType, png::BitDepth), CodecError> {
        let meta_dict = metadata.map(|m| m.as_dict(None));

        // Check for indexed palette mode
        if num_bands == 1 && pixel_type == PixelType::UInt8 {
            if let Some(ref dict) = meta_dict {
                if dict.contains_key("PLTE") {
                    return Ok((png::ColorType::Indexed, png::BitDepth::Eight));
                }
            }
        }

        // Check for sub-byte bit depth hint
        if num_bands == 1 && pixel_type == PixelType::UInt8 {
            if let Some(ref dict) = meta_dict {
                if let Some(bd_val) = dict.get("bit_depth").and_then(|v| v.as_u64()) {
                    let bd = match bd_val {
                        1 => png::BitDepth::One,
                        2 => png::BitDepth::Two,
                        4 => png::BitDepth::Four,
                        _ => png::BitDepth::Eight,
                    };
                    if matches!(
                        bd,
                        png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four
                    ) {
                        return Ok((png::ColorType::Grayscale, bd));
                    }
                }
            }
        }

        // Standard configurations
        let bit_depth = match pixel_type {
            PixelType::UInt8 => png::BitDepth::Eight,
            PixelType::UInt16 => png::BitDepth::Sixteen,
            _ => {
                return Err(CodecError::Unsupported(format!(
                    "PNG does not support pixel type {:?}",
                    pixel_type
                )));
            }
        };

        let color_type = match num_bands {
            1 => png::ColorType::Grayscale,
            2 => png::ColorType::GrayscaleAlpha,
            3 => png::ColorType::Rgb,
            4 => png::ColorType::Rgba,
            n => {
                return Err(CodecError::Unsupported(format!(
                    "PNG does not support {}-band images",
                    n
                )));
            }
        };

        Ok((color_type, bit_depth))
    }

    /// Convert BSQ pixel data to interleaved row-major format.
    ///
    /// BSQ: all band-0 pixels, then band-1, etc.
    /// Interleaved: `[R0G0B0 R1G1B1 ...]` per row.
    ///
    /// For UInt16, converts native-endian to big-endian (png crate expects BE).
    fn bsq_to_interleaved(
        bsq: &[u8],
        width: u32,
        height: u32,
        num_bands: u32,
        pixel_type: PixelType,
    ) -> Vec<u8> {
        let num_pixels = (width as usize) * (height as usize);
        let bps = pixel_type.bytes_per_pixel(); // bytes per sample
        let total_bytes = num_pixels * num_bands as usize * bps;

        if num_bands == 1 {
            // Single band: no interleaving needed, just handle endianness
            if bps == 2 {
                return Self::convert_ne_to_be_u16(bsq, num_pixels);
            }
            return bsq.to_vec();
        }

        let mut interleaved = vec![0u8; total_bytes];
        let band_size_bytes = num_pixels * bps;

        if bps == 1 {
            // UInt8: simple byte shuffle
            for row in 0..height as usize {
                for col in 0..width as usize {
                    let linear_idx = row * width as usize + col;
                    let dst_offset = (row * width as usize + col) * num_bands as usize;
                    for band in 0..num_bands as usize {
                        interleaved[dst_offset + band] = bsq[band * num_pixels + linear_idx];
                    }
                }
            }
        } else {
            // UInt16: native-endian → big-endian interleaved
            for row in 0..height as usize {
                for col in 0..width as usize {
                    let linear_idx = row * width as usize + col;
                    let dst_offset = (row * width as usize + col) * num_bands as usize * 2;
                    for band in 0..num_bands as usize {
                        let src = band * band_size_bytes + linear_idx * 2;
                        let ne_val = u16::from_ne_bytes([bsq[src], bsq[src + 1]]);
                        let be_bytes = ne_val.to_be_bytes();
                        let dst = dst_offset + band * 2;
                        interleaved[dst] = be_bytes[0];
                        interleaved[dst + 1] = be_bytes[1];
                    }
                }
            }
        }

        interleaved
    }

    /// Convert native-endian u16 samples to big-endian for the png crate.
    fn convert_ne_to_be_u16(data: &[u8], num_samples: usize) -> Vec<u8> {
        let mut out = vec![0u8; num_samples * 2];
        for i in 0..num_samples {
            let ne_val = u16::from_ne_bytes([data[i * 2], data[i * 2 + 1]]);
            let be = ne_val.to_be_bytes();
            out[i * 2] = be[0];
            out[i * 2 + 1] = be[1];
        }
        out
    }

    /// Pack UInt8 pixel values into sub-byte samples for 1/2/4-bit grayscale.
    ///
    /// Each row is packed independently and padded to byte boundaries.
    fn pack_sub_byte(data: &[u8], width: u32, height: u32, bit_depth: u8) -> Vec<u8> {
        let samples_per_byte = 8 / bit_depth as usize;
        let row_bytes = (width as usize).div_ceil(samples_per_byte);
        let mut packed = vec![0u8; row_bytes * height as usize];

        for row in 0..height as usize {
            let row_start = row * width as usize;
            for col in 0..width as usize {
                let sample = data[row_start + col];
                let byte_idx = row * row_bytes + col / samples_per_byte;
                let bit_offset =
                    (samples_per_byte - 1 - (col % samples_per_byte)) * bit_depth as usize;
                packed[byte_idx] |= (sample & ((1 << bit_depth) - 1)) << bit_offset;
            }
        }

        packed
    }
}

// =============================================================================
// DatasetWriter Implementation
// =============================================================================

impl DatasetWriter for PNGDatasetWriter {
    fn add_asset(
        &mut self,
        key: &str,
        provider: AssetProvider,
        _title: &str,
        _description: &str,
        _roles: &[String],
    ) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Unsupported(
                "Writer is already closed".to_string(),
            ));
        }

        // PNG supports only image assets
        if provider.asset_type() != AssetType::Image {
            return Err(CodecError::Unsupported(
                "PNG format supports only image assets".to_string(),
            ));
        }

        // PNG supports only a single image per file
        if self.image_queued {
            return Err(CodecError::Unsupported(
                "PNG format supports only a single image per file".to_string(),
            ));
        }

        self.assets.push(QueuedPngAsset {
            provider,
            key: key.to_string(),
        });
        self.image_queued = true;

        Ok(())
    }

    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
        self.metadata = Some(metadata);
        Ok(())
    }

    fn close(&mut self) -> Result<(), CodecError> {
        // Idempotent: second close is a no-op
        if self.closed {
            return Ok(());
        }
        self.closed = true;

        // If no asset was queued, nothing to write
        let asset = match self.assets.first() {
            Some(a) => a,
            None => return Ok(()),
        };

        let image = asset
            .provider
            .as_image()
            .ok_or_else(|| CodecError::Unsupported("Asset is not an Image variant".to_string()))?;
        let image = image.as_ref();

        let width = image.num_columns();
        let height = image.num_rows();
        let num_bands = image.num_bands();
        let pixel_type = image.pixel_value_type();

        // Read all pixel data (BSQ format)
        let (bsq_data, _shape) = image.get_block(0, 0, 0, None)?;

        // Determine PNG color type and bit depth
        let meta_ref = self.metadata.as_deref();
        let (color_type, bit_depth) =
            Self::determine_color_config(num_bands, pixel_type, meta_ref)?;

        // Prepare pixel data for the png crate
        let encoded_data = match color_type {
            png::ColorType::Indexed => {
                // Indexed: single band UInt8, no interleaving needed
                bsq_data.clone()
            }
            png::ColorType::Grayscale
                if matches!(
                    bit_depth,
                    png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four
                ) =>
            {
                // Sub-byte packing
                let bd_val = match bit_depth {
                    png::BitDepth::One => 1,
                    png::BitDepth::Two => 2,
                    png::BitDepth::Four => 4,
                    _ => unreachable!(),
                };
                Self::pack_sub_byte(&bsq_data, width, height, bd_val)
            }
            _ => {
                // Standard: convert BSQ to interleaved row-major
                Self::bsq_to_interleaved(&bsq_data, width, height, num_bands, pixel_type)
            }
        };

        // Create the output file and encoder
        let file = File::create(&self.path).map_err(CodecError::Io)?;
        let buf_writer = BufWriter::new(file);

        let mut encoder = png::Encoder::new(buf_writer, width, height);
        encoder.set_color(color_type);
        encoder.set_depth(bit_depth);

        // Set palette for indexed images
        if color_type == png::ColorType::Indexed {
            if let Some(ref meta) = self.metadata {
                let dict = meta.as_dict(None);
                if let Some(plte_val) = dict.get("PLTE") {
                    if let Some(arr) = plte_val.as_array() {
                        let mut palette = Vec::with_capacity(arr.len() * 3);
                        for entry in arr {
                            if let Some(rgb) = entry.as_array() {
                                for c in rgb.iter().take(3) {
                                    palette.push(c.as_u64().unwrap_or(0) as u8);
                                }
                            }
                        }
                        encoder.set_palette(palette);
                    }
                }
            }
        }

        // Write tEXt chunks from metadata
        let skip_keys: HashSet<&str> = SKIP_TEXT_KEYS.iter().copied().collect();
        if let Some(ref meta) = self.metadata {
            let dict = meta.as_dict(None);
            for (key, value) in &dict {
                if skip_keys.contains(key.as_str()) {
                    continue;
                }
                // Only write string values as tEXt chunks
                if let Some(text) = value.as_str() {
                    encoder
                        .add_text_chunk(key.clone(), text.to_string())
                        .map_err(|e| CodecError::Encode(format!("PNG tEXt chunk error: {}", e)))?;
                }
            }
        }

        let mut writer = encoder
            .write_header()
            .map_err(|e| CodecError::Encode(format!("PNG encode error: {}", e)))?;

        writer
            .write_image_data(&encoded_data)
            .map_err(|e| CodecError::Encode(format!("PNG encode error: {}", e)))?;

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
    use crate::png::reader::PNGDatasetReader;
    use crate::traits::image::ImageAssetProvider;
    use crate::traits::reader::DatasetReader;
    use std::sync::Arc;

    /// Helper: create a BufferedImageAssetProvider with the given config and BSQ data.
    fn make_image_provider(
        width: u32,
        height: u32,
        num_bands: u32,
        pixel_type: PixelType,
        bsq_data: &[u8],
    ) -> Arc<BufferedImageAssetProvider> {
        let config = MemoryImageConfig::new(width, height)
            .with_bands(num_bands)
            .with_block_size(width, height)
            .with_pixel_type(pixel_type);
        let provider = BufferedImageAssetProvider::new("image:0", config);
        provider.set_block(0, 0, bsq_data).unwrap();
        Arc::new(provider)
    }

    // =========================================================================
    // test_writer_new — Creates instance without error
    // =========================================================================

    #[test]
    fn test_writer_new() {
        let writer = PNGDatasetWriter::new("/tmp/test_writer_new.png");
        assert!(writer.is_ok());
        let w = writer.unwrap();
        assert!(!w.closed);
        assert!(!w.image_queued);
        assert!(w.assets.is_empty());
    }

    // =========================================================================
    // test_add_image_asset — Accepts image asset
    // =========================================================================

    #[test]
    fn test_add_image_asset() {
        let mut writer = PNGDatasetWriter::new("/tmp/test_add_image.png").unwrap();
        let provider = make_image_provider(2, 2, 1, PixelType::UInt8, &[10, 20, 30, 40]);

        let result = writer.add_asset(
            "image:0",
            AssetProvider::Image(provider),
            "Test",
            "Test image",
            &[],
        );
        assert!(result.is_ok());
        assert!(writer.image_queued);
        assert_eq!(writer.assets.len(), 1);
    }

    // =========================================================================
    // test_add_non_image_rejected — Non-image asset returns Unsupported
    // =========================================================================

    #[test]
    fn test_add_non_image_rejected() {
        use crate::buffered::BufferedTextAssetProvider;

        let mut writer = PNGDatasetWriter::new("/tmp/test_non_image.png").unwrap();
        let text_provider = Arc::new(BufferedTextAssetProvider::new(
            "text_0",
            "Hello".to_string(),
            "utf-8",
        ));

        let result = writer.add_asset(
            "text_0",
            AssetProvider::Text(text_provider),
            "Text",
            "A text asset",
            &[],
        );
        match result {
            Err(CodecError::Unsupported(msg)) => {
                assert!(msg.contains("only image assets"));
            }
            other => panic!("Expected Unsupported, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_add_duplicate_rejected — Second add_asset returns Unsupported
    // =========================================================================

    #[test]
    fn test_add_duplicate_rejected() {
        let mut writer = PNGDatasetWriter::new("/tmp/test_dup.png").unwrap();
        let provider1 = make_image_provider(2, 2, 1, PixelType::UInt8, &[1, 2, 3, 4]);
        let provider2 = make_image_provider(2, 2, 1, PixelType::UInt8, &[5, 6, 7, 8]);

        writer
            .add_asset(
                "img0",
                AssetProvider::Image(provider1),
                "First",
                "First image",
                &[],
            )
            .unwrap();

        let result = writer.add_asset(
            "img1",
            AssetProvider::Image(provider2),
            "Second",
            "Second image",
            &[],
        );
        match result {
            Err(CodecError::Unsupported(msg)) => {
                assert!(msg.contains("single image per file"));
            }
            other => panic!("Expected Unsupported, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_close_idempotent — Double close is safe
    // =========================================================================

    #[test]
    fn test_close_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("idempotent.png");

        let mut writer = PNGDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider(
            2,
            2,
            3,
            PixelType::UInt8,
            &[
                1, 2, 3, 4, // R
                5, 6, 7, 8, // G
                9, 10, 11, 12, // B
            ],
        );
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();

        // First close should succeed
        assert!(writer.close().is_ok());
        assert!(writer.closed);

        // Second close should be a no-op (idempotent)
        assert!(writer.close().is_ok());
    }

    // =========================================================================
    // Roundtrip: write then read back grayscale 8-bit
    // =========================================================================

    #[test]
    fn test_roundtrip_grayscale_8bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gray8.png");

        let pixels: Vec<u8> = vec![10, 20, 30, 40];
        let provider = make_image_provider(2, 2, 1, PixelType::UInt8, &pixels);

        let mut writer = PNGDatasetWriter::new(&path).unwrap();
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();
        writer.close().unwrap();

        // Read back
        let data = std::fs::read(&path).unwrap();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 2]);
        assert_eq!(read_pixels, pixels);
    }

    // =========================================================================
    // Roundtrip: write then read back RGB 8-bit
    // =========================================================================

    #[test]
    fn test_roundtrip_rgb_8bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rgb8.png");

        // BSQ: R=[1,2,3,4], G=[5,6,7,8], B=[9,10,11,12]
        let pixels: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let provider = make_image_provider(2, 2, 3, PixelType::UInt8, &pixels);

        let mut writer = PNGDatasetWriter::new(&path).unwrap();
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();
        writer.close().unwrap();

        let data = std::fs::read(&path).unwrap();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 2, 2]);
        assert_eq!(read_pixels, pixels);
    }
}
