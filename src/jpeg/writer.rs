//! JPEGDatasetWriter — implements DatasetWriter for standalone JPEG files.
//!
//! Encodes pixel data from an `ImageAssetProvider` into a valid JPEG file.
//! The writer queues a single image asset via `add_asset`, stores optional
//! metadata via `set_metadata`, and performs all encoding work in `close()`.
//!
//! During `close()`:
//! 1. Reads all pixel data from the queued provider via `get_block(0, 0, 0, None)`
//! 2. Converts BSQ pixel data to pixel-interleaved format for libjpeg-turbo
//! 3. Encodes via `ffi::compress_8bit()` with the configured quality
//! 4. Writes the resulting JPEG bytes to the configured output writer
//!
//! # Encoding Hints
//!
//! Encoding parameters are read from the dataset-level metadata provider:
//! - `JPEG_QUALITY` (u8, 1-100) — JPEG quality factor (default 75)
//!
//! # Constraints
//!
//! - Only UInt8 pixel type is supported
//! - Only 1 (grayscale) or 3 (RGB) bands are supported
//! - Only a single image asset per file

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::assembly::reassemble_full_image;
use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetWriter, MetadataProvider};
use crate::types::{AssetType, PixelType};

/// An image asset queued for writing.
struct QueuedJPEGAsset {
    provider: AssetProvider,
    #[allow(dead_code)]
    key: String,
}

/// Writer for standalone JPEG datasets implementing the `DatasetWriter` trait.
///
/// Queues a single image asset and optional metadata, then encodes
/// everything into a JPEG file on `close()`.
pub struct JPEGDatasetWriter {
    /// Output target, taken by `close()` when writing.
    ///
    /// Wrapped in `Mutex` so the struct is `Sync` (required by the
    /// `DatasetWriter` trait) even though `Box<dyn Write + Send>` alone is
    /// only `Send`. The inner `Option` allows `close()` to move the writer
    /// out via `take()` for a final `write_all` + `flush`. There is no
    /// runtime contention because the `DatasetWriter` methods only ever take
    /// `&mut self`.
    output: Mutex<Option<Box<dyn Write + Send>>>,
    /// Whether an image asset has been queued.
    image_queued: bool,
    /// Dataset-level metadata (encoding hints source).
    metadata: Option<Arc<dyn MetadataProvider>>,
    /// Whether `close()` has been called.
    closed: bool,
    /// Queued assets (at most one).
    assets: Vec<QueuedJPEGAsset>,
}

impl JPEGDatasetWriter {
    /// Create a new writer targeting the given output path.
    ///
    /// The file is opened immediately and wrapped in a `BufWriter<File>`,
    /// then delegated to `new_with_output`.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CodecError> {
        let file = File::create(path.as_ref()).map_err(CodecError::Io)?;
        let buf_writer = BufWriter::new(file);
        Self::new_with_output(Box::new(buf_writer))
    }

    /// Create a new writer targeting the given output writer.
    ///
    /// Accepts any `Box<dyn Write + Send>`, enabling output to files, Python
    /// streams (via `PyWriteStream`), in-memory buffers, or any other
    /// `Write` implementation.
    pub fn new_with_output(output: Box<dyn Write + Send>) -> Result<Self, CodecError> {
        Ok(Self {
            output: Mutex::new(Some(output)),
            image_queued: false,
            metadata: None,
            closed: false,
            assets: Vec::new(),
        })
    }

    /// Extract JPEG quality from metadata hints, defaulting to 75.
    fn quality_from_metadata(metadata: Option<&dyn MetadataProvider>) -> u8 {
        metadata
            .map(|m| m.entries(None))
            .and_then(|d| d.get("JPEG_QUALITY").cloned())
            .and_then(|v| v.as_u64())
            .map(|v| (v as u8).clamp(1, 100))
            .unwrap_or(75)
    }

    /// Convert BSQ pixel data to pixel-interleaved format.
    ///
    /// BSQ: [R0,R1,...,Rn, G0,G1,...,Gn, B0,B1,...,Bn]
    /// Interleaved: [R0,G0,B0, R1,G1,B1, ..., Rn,Gn,Bn]
    ///
    /// For single-band (grayscale), this is a no-op copy.
    fn bsq_to_interleaved(bsq: &[u8], num_pixels: usize, num_bands: usize) -> Vec<u8> {
        if num_bands == 1 {
            return bsq.to_vec();
        }

        let mut interleaved = vec![0u8; num_pixels * num_bands];
        for pixel in 0..num_pixels {
            for band in 0..num_bands {
                interleaved[pixel * num_bands + band] = bsq[band * num_pixels + pixel];
            }
        }
        interleaved
    }
}

// =============================================================================
// DatasetWriter Implementation
// =============================================================================

impl DatasetWriter for JPEGDatasetWriter {
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

        // JPEG supports only image assets
        if provider.asset_type() != AssetType::Image {
            return Err(CodecError::Unsupported(
                "JPEG format supports only image assets".to_string(),
            ));
        }

        // JPEG supports only a single image per file
        if self.image_queued {
            return Err(CodecError::Unsupported(
                "JPEG format supports only a single image per file".to_string(),
            ));
        }

        // Validate image constraints before accepting
        let image = provider
            .as_image()
            .ok_or_else(|| CodecError::Unsupported("Asset is not an Image variant".to_string()))?;

        // JPEG only supports UInt8
        if image.pixel_value_type() != PixelType::UInt8 {
            return Err(CodecError::Unsupported(
                "JPEG only supports UInt8 pixel type".to_string(),
            ));
        }

        // JPEG supports 1 (grayscale) or 3 (RGB) bands
        let num_bands = image.num_bands();
        if num_bands > 3 {
            return Err(CodecError::Unsupported(format!(
                "JPEG does not support {}-band images",
                num_bands
            )));
        }

        self.assets.push(QueuedJPEGAsset {
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
        let num_pixels = (width * height) as usize;

        // Read all pixel data (BSQ format) — reassemble from multi-block providers
        let (bsq_data, _shape) = reassemble_full_image(image)?;

        // Convert BSQ to pixel-interleaved for libjpeg-turbo
        let interleaved = Self::bsq_to_interleaved(&bsq_data, num_pixels, num_bands as usize);

        // Extract quality from metadata hints
        let quality = Self::quality_from_metadata(self.metadata.as_deref());

        // Encode via libjpeg-turbo
        let jpeg_data = crate::jpeg::ffi::compress_8bit(
            &interleaved,
            width as usize,
            height as usize,
            num_bands as usize,
            quality,
        )
        .map_err(|e| CodecError::Encode(format!("JPEG encode error: {}", e)))?;

        // Write JPEG data to the stored output writer
        let mut output = self
            .output
            .lock()
            .map_err(|_| CodecError::Unsupported("JPEG writer output mutex poisoned".to_string()))?
            .take()
            .ok_or_else(|| {
                CodecError::Unsupported("JPEG writer output is not available".to_string())
            })?;
        output.write_all(&jpeg_data).map_err(CodecError::Io)?;
        output.flush().map_err(CodecError::Io)?;

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(all(test, feature = "libjpeg-turbo"))]
mod tests {
    use super::*;
    use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
    use crate::jpeg::reader::JPEGDatasetReader;
    use crate::owned_buffer::OwnedBuffer;
    use crate::traits::reader::DatasetReader;

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
        let writer = JPEGDatasetWriter::new("/tmp/test_jpeg_writer_new.jpg");
        assert!(writer.is_ok());
        let w = writer.unwrap();
        assert!(!w.closed);
        assert!(!w.image_queued);
        assert!(w.assets.is_empty());
    }

    // =========================================================================
    // test_add_image_asset — Accepts valid image asset
    // =========================================================================

    #[test]
    fn test_add_image_asset() {
        let mut writer = JPEGDatasetWriter::new("/tmp/test_jpeg_add_image.jpg").unwrap();
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

        let mut writer = JPEGDatasetWriter::new("/tmp/test_jpeg_non_image.jpg").unwrap();
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
        let mut writer = JPEGDatasetWriter::new("/tmp/test_jpeg_dup.jpg").unwrap();
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
    // test_add_after_close_rejected — add_asset after close returns Unsupported
    // =========================================================================

    #[test]
    fn test_add_after_close_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("closed.jpg");

        let mut writer = JPEGDatasetWriter::new(&path).unwrap();
        writer.close().unwrap();

        let provider = make_image_provider(2, 2, 1, PixelType::UInt8, &[1, 2, 3, 4]);
        let result = writer.add_asset("img0", AssetProvider::Image(provider), "Test", "Test", &[]);
        match result {
            Err(CodecError::Unsupported(msg)) => {
                assert!(msg.contains("already closed"));
            }
            other => panic!("Expected Unsupported, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_reject_non_uint8 — Non-UInt8 pixel type returns Unsupported
    // =========================================================================

    #[test]
    fn test_reject_non_uint8() {
        let mut writer = JPEGDatasetWriter::new("/tmp/test_jpeg_u16.jpg").unwrap();
        let pixels: Vec<u8> = vec![0; 2 * 2 * 2]; // 2x2 UInt16
        let provider = make_image_provider(2, 2, 1, PixelType::UInt16, &pixels);

        let result = writer.add_asset("img0", AssetProvider::Image(provider), "Test", "Test", &[]);
        match result {
            Err(CodecError::Unsupported(msg)) => {
                assert!(msg.contains("UInt8"));
            }
            other => panic!("Expected Unsupported, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_reject_too_many_bands — >3 bands returns Unsupported
    // =========================================================================

    #[test]
    fn test_reject_too_many_bands() {
        let mut writer = JPEGDatasetWriter::new("/tmp/test_jpeg_4band.jpg").unwrap();
        let pixels: Vec<u8> = vec![128; 2 * 2 * 4]; // 2x2 4-band
        let provider = make_image_provider(2, 2, 4, PixelType::UInt8, &pixels);

        let result = writer.add_asset("img0", AssetProvider::Image(provider), "Test", "Test", &[]);
        match result {
            Err(CodecError::Unsupported(msg)) => {
                assert!(msg.contains("4-band"));
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
        let path = dir.path().join("idempotent.jpg");

        let npix = 64 * 64;
        let pixels: Vec<u8> = (0..npix).map(|i| (i % 256) as u8).collect();
        let provider = make_image_provider(64, 64, 1, PixelType::UInt8, &pixels);

        let mut writer = JPEGDatasetWriter::new(&path).unwrap();
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();

        assert!(writer.close().is_ok());
        assert!(writer.closed);
        assert!(writer.close().is_ok());
    }

    // =========================================================================
    // Roundtrip: write then read back grayscale 8-bit
    // =========================================================================

    #[test]
    fn test_roundtrip_grayscale_8bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gray8.jpg");

        let npix = 64 * 64usize;
        let pixels: Vec<u8> = (0..npix).map(|i| (i % 256) as u8).collect();
        let provider = make_image_provider(64, 64, 1, PixelType::UInt8, &pixels);

        let mut writer = JPEGDatasetWriter::new(&path).unwrap();
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
        let reader = JPEGDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.num_rows(), 64);
        assert_eq!(image.num_columns(), 64);
        assert_eq!(image.pixel_value_type(), PixelType::UInt8);

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 64, 64]);
        // JPEG is lossy — verify dimensions match, not exact pixel values
        assert_eq!(read_pixels.len(), npix);
    }

    // =========================================================================
    // Roundtrip: write then read back RGB 8-bit
    // =========================================================================

    #[test]
    fn test_roundtrip_rgb_8bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rgb8.jpg");

        let npix = 64 * 64usize;
        let mut pixels = Vec::with_capacity(npix * 3);
        for band in 0u8..3 {
            for i in 0..npix {
                pixels.push(band.wrapping_mul(80).wrapping_add((i % 256) as u8));
            }
        }
        let provider = make_image_provider(64, 64, 3, PixelType::UInt8, &pixels);

        let mut writer = JPEGDatasetWriter::new(&path).unwrap();
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
        let reader = JPEGDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        assert_eq!(image.num_bands(), 3);
        assert_eq!(image.num_rows(), 64);
        assert_eq!(image.num_columns(), 64);

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(read_pixels.len(), npix * 3);
    }

    // =========================================================================
    // Roundtrip: multi-block RGB image (regression for multiband panic)
    // =========================================================================

    #[test]
    fn test_roundtrip_rgb_multiblock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rgb_multiblock.jpg");

        // 64x64 image with 32x32 blocks → 2x2 block grid
        let width: u32 = 64;
        let height: u32 = 64;
        let num_bands: u32 = 3;
        let block_w: u32 = 32;
        let block_h: u32 = 32;

        let config = MemoryImageConfig::new(width, height)
            .with_bands(num_bands)
            .with_block_size(block_w, block_h)
            .with_pixel_type(PixelType::UInt8);
        let provider = BufferedImageAssetProvider::new("image:0", config);

        // Fill each block with BSQ data
        let blk_pixels = (block_h * block_w) as usize;
        for br in 0..2u32 {
            for bc in 0..2u32 {
                let base = (br * 2 + bc) * 50;
                let mut block_bsq = Vec::with_capacity(blk_pixels * num_bands as usize);
                for band in 0..num_bands {
                    for i in 0..blk_pixels {
                        block_bsq.push(((base + band * 30 + i as u32) % 256) as u8);
                    }
                }
                provider.set_block(br, bc, &block_bsq).unwrap();
            }
        }

        let provider = Arc::new(provider);
        let mut writer = JPEGDatasetWriter::new(&path).unwrap();
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

        // Read back and verify dimensions
        let data = std::fs::read(&path).unwrap();
        let reader = JPEGDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        assert_eq!(image.num_bands(), 3);
        assert_eq!(image.num_rows(), 64);
        assert_eq!(image.num_columns(), 64);

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(read_pixels.len(), (width * height * num_bands) as usize);
    }
}
