//! TIFF dataset writer implementation.
//!
//! This module provides [`TIFFDatasetWriter`] which implements the `DatasetWriter`
//! trait for creating tiled TIFF files. The writer assembles the TIFF in memory
//! via `TIFFClientOpen` with custom write callbacks, then flushes to disk on `close()`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::CodecError;
use crate::tiff::ffi::TiffHandle;
use crate::tiff::tags;
use crate::traits::{AssetProvider, DatasetWriter, ImageAssetProvider, MetadataProvider};
use crate::types::{AssetType, PixelType};

// =============================================================================
// Constants
// =============================================================================

/// Default tile width in pixels.
const DEFAULT_TILE_WIDTH: u32 = 256;

/// Default tile height in pixels.
const DEFAULT_TILE_HEIGHT: u32 = 256;

// =============================================================================
// Encoding Hints
// =============================================================================

/// Parsed encoding hints controlling TIFF output parameters.
struct TiffEncodingHints {
    tile_width: u32,
    tile_height: u32,
    compression: u16,
    predictor: u16,
    planar_config: u16,
}

impl Default for TiffEncodingHints {
    fn default() -> Self {
        Self {
            tile_width: DEFAULT_TILE_WIDTH,
            tile_height: DEFAULT_TILE_HEIGHT,
            compression: tags::COMPRESSION_DEFLATE,
            predictor: 2, // Horizontal predictor for Deflate
            planar_config: tags::PLANAR_CONFIG_CONTIG,
        }
    }
}

impl TiffEncodingHints {
    /// Parse encoding hints from a `MetadataProvider`.
    fn from_metadata(metadata: &dyn MetadataProvider) -> Result<Self, CodecError> {
        let dict = metadata.as_dict(None);
        let mut hints = TiffEncodingHints::default();

        // Parse TileWidth
        if let Some(val) = dict.get("TileWidth") {
            hints.tile_width = parse_u32_from_json(val, "TileWidth")?;
        }

        // Parse TileHeight
        if let Some(val) = dict.get("TileHeight") {
            hints.tile_height = parse_u32_from_json(val, "TileHeight")?;
        }

        // Parse Compression
        if let Some(val) = dict.get("Compression") {
            let s = json_to_string(val, "Compression")?;
            hints.compression = match s.as_str() {
                "None" => tags::COMPRESSION_NONE,
                "LZW" => tags::COMPRESSION_LZW,
                "Deflate" => tags::COMPRESSION_DEFLATE,
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Unknown Compression value: '{}'. Expected None, LZW, or Deflate",
                        other
                    )));
                }
            };
        }

        // Parse Predictor
        let explicit_predictor = dict.contains_key("Predictor");
        if let Some(val) = dict.get("Predictor") {
            let s = json_to_string(val, "Predictor")?;
            hints.predictor = match s.as_str() {
                "None" => 1,
                "Horizontal" => 2,
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Unknown Predictor value: '{}'. Expected None or Horizontal",
                        other
                    )));
                }
            };
        }

        // Apply default predictor based on compression if not explicitly set
        if !explicit_predictor {
            hints.predictor = match hints.compression {
                tags::COMPRESSION_LZW | tags::COMPRESSION_DEFLATE => 2, // Horizontal
                _ => 1, // None
            };
        }

        // Parse PlanarConfiguration
        if let Some(val) = dict.get("PlanarConfiguration") {
            let s = json_to_string(val, "PlanarConfiguration")?;
            hints.planar_config = match s.as_str() {
                "Chunky" => tags::PLANAR_CONFIG_CONTIG,
                "Planar" => tags::PLANAR_CONFIG_SEPARATE,
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Unknown PlanarConfiguration value: '{}'. Expected Chunky or Planar",
                        other
                    )));
                }
            };
        }

        Ok(hints)
    }
}

/// Parse a u32 from a serde_json::Value, supporting both number and string representations.
fn parse_u32_from_json(val: &serde_json::Value, field: &str) -> Result<u32, CodecError> {
    if let Some(n) = val.as_u64() {
        return Ok(n as u32);
    }
    if let Some(s) = val.as_str() {
        return s.parse::<u32>().map_err(|_| {
            CodecError::InvalidFormat(format!("Cannot parse '{}' as integer for {}", s, field))
        });
    }
    Err(CodecError::InvalidFormat(format!(
        "Expected integer or string for {}, got {:?}",
        field, val
    )))
}

/// Extract a string from a serde_json::Value.
fn json_to_string(val: &serde_json::Value, field: &str) -> Result<String, CodecError> {
    if let Some(s) = val.as_str() {
        return Ok(s.to_string());
    }
    Err(CodecError::InvalidFormat(format!(
        "Expected string for {}, got {:?}",
        field, val
    )))
}


// =============================================================================
// Pixel Layout Conversion
// =============================================================================

/// Convert band-sequential (CHW) data to interleaved (HWC) format.
///
/// Input layout:  `[band0_pixel0, band0_pixel1, ..., band1_pixel0, band1_pixel1, ...]`
/// Output layout: `[pixel0_band0, pixel0_band1, ..., pixel1_band0, pixel1_band1, ...]`
///
/// `bytes_per_sample` is the number of bytes per pixel component (1, 2, 4, or 8).
fn bsq_to_interleaved(
    data: &[u8],
    num_bands: u32,
    pixels_per_band: u32,
    bytes_per_sample: u32,
) -> Vec<u8> {
    let bands = num_bands as usize;
    let pixels = pixels_per_band as usize;
    let bps = bytes_per_sample as usize;
    let total = data.len();

    let mut out = vec![0u8; total];
    let band_stride = pixels * bps;

    for band in 0..bands {
        let src_offset = band * band_stride;
        for pixel in 0..pixels {
            let src = src_offset + pixel * bps;
            let dst = (pixel * bands + band) * bps;
            out[dst..dst + bps].copy_from_slice(&data[src..src + bps]);
        }
    }

    out
}

// =============================================================================
// Edge Tile Padding
// =============================================================================

/// Pad a block to full tile dimensions, filling missing pixels with `pad_value`.
///
/// `block_data` is in CHW (band-sequential) format with shape `[bands, actual_rows, actual_cols]`.
/// Returns a new buffer in CHW format with shape `[bands, tile_height, tile_width]`.
fn pad_tile(
    block_data: &[u8],
    actual_rows: u32,
    actual_cols: u32,
    tile_height: u32,
    tile_width: u32,
    num_bands: u32,
    bytes_per_sample: u32,
    pad_value: u8,
) -> Vec<u8> {
    let bps = bytes_per_sample as usize;
    let full_row_bytes = tile_width as usize * bps;
    let actual_row_bytes = actual_cols as usize * bps;
    let full_band_size = tile_height as usize * full_row_bytes;
    let actual_band_size = actual_rows as usize * actual_row_bytes;

    let mut out = vec![pad_value; num_bands as usize * full_band_size];

    for band in 0..num_bands as usize {
        let dst_band_offset = band * full_band_size;
        let src_band_offset = band * actual_band_size;
        for row in 0..actual_rows as usize {
            let dst = dst_band_offset + row * full_row_bytes;
            let src = src_band_offset + row * actual_row_bytes;
            out[dst..dst + actual_row_bytes]
                .copy_from_slice(&block_data[src..src + actual_row_bytes]);
        }
    }

    out
}

// =============================================================================
// Queued Asset
// =============================================================================

/// An image asset queued for writing.
struct QueuedImageAsset {
    key: String,
    provider: Arc<dyn AssetProvider>,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    roles: Vec<String>,
}

// =============================================================================
// TIFFDatasetWriter
// =============================================================================

/// Writer for tiled TIFF files implementing the `DatasetWriter` trait.
///
/// Assembles the TIFF in memory via libtiff's `TIFFClientOpen` with custom
/// write callbacks, then flushes the bytes to disk on `close()`.
pub struct TIFFDatasetWriter {
    /// Output file path (written on close).
    path: PathBuf,
    /// Queued image assets in insertion order.
    assets: Vec<QueuedImageAsset>,
    /// Set of asset keys for duplicate detection.
    asset_keys: HashSet<String>,
    /// Dataset-level metadata (encoding hints source).
    metadata: Option<Arc<dyn MetadataProvider>>,
    /// Whether `close()` has been called.
    closed: bool,
}

impl TIFFDatasetWriter {
    /// Create a new writer for the given output path.
    ///
    /// No file or libtiff handle is opened until `close()` is called.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CodecError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            assets: Vec::new(),
            asset_keys: HashSet::new(),
            metadata: None,
            closed: false,
        })
    }

    /// Downcast an `AssetProvider` to `ImageAssetProvider`.
    fn get_image_provider(provider: &Arc<dyn AssetProvider>) -> Option<&dyn ImageAssetProvider> {
        use crate::buffered::BufferedImageAssetProvider;
        use crate::tiff::image::TIFFImageAssetProvider;

        if let Some(p) = provider.as_any().downcast_ref::<BufferedImageAssetProvider>() {
            return Some(p as &dyn ImageAssetProvider);
        }
        if let Some(p) = provider.as_any().downcast_ref::<TIFFImageAssetProvider>() {
            return Some(p as &dyn ImageAssetProvider);
        }
        None
    }

    /// Map a `PixelType` to the TIFF SampleFormat tag value.
    fn sample_format(pixel_type: PixelType) -> u16 {
        match pixel_type {
            PixelType::UInt8 | PixelType::UInt16 | PixelType::UInt32 => tags::SAMPLE_FORMAT_UINT,
            PixelType::Int8 | PixelType::Int16 | PixelType::Int32 => tags::SAMPLE_FORMAT_INT,
            PixelType::Float32 | PixelType::Float64 => tags::SAMPLE_FORMAT_FLOAT,
        }
    }

    /// Choose PhotometricInterpretation based on band count.
    fn photometric_interpretation(num_bands: u32) -> u16 {
        if num_bands >= 3 {
            tags::PHOTOMETRIC_RGB
        } else {
            tags::PHOTOMETRIC_MINISBLACK
        }
    }

    /// Compute the pad byte from `pad_pixel_value()`.
    /// Truncates the f64 to a u8 for use as a fill byte.
    fn pad_byte(pad_value: f64) -> u8 {
        pad_value as u8
    }


    /// Write a single image asset as one IFD.
    fn write_image_ifd(
        handle: &TiffHandle,
        image: &dyn ImageAssetProvider,
        hints: &TiffEncodingHints,
    ) -> Result<(), CodecError> {
        let num_cols = image.num_columns();
        let num_rows = image.num_rows();
        let num_bands = image.num_bands();
        let bits_per_sample = image.actual_bits_per_pixel();
        let pixel_type = image.pixel_value_type();
        let bytes_per_sample = pixel_type.bytes_per_pixel() as u32;

        // Set TIFF tags
        handle.set_field_u32(tags::IMAGE_WIDTH, num_cols)?;
        handle.set_field_u32(tags::IMAGE_LENGTH, num_rows)?;
        handle.set_field_u16(tags::BITS_PER_SAMPLE, bits_per_sample as u16)?;
        handle.set_field_u16(tags::SAMPLES_PER_PIXEL, num_bands as u16)?;
        handle.set_field_u16(tags::SAMPLE_FORMAT, Self::sample_format(pixel_type))?;
        handle.set_field_u16(
            tags::PHOTOMETRIC_INTERPRETATION,
            Self::photometric_interpretation(num_bands),
        )?;
        handle.set_field_u32(tags::TILE_WIDTH, hints.tile_width)?;
        handle.set_field_u32(tags::TILE_LENGTH, hints.tile_height)?;
        handle.set_field_u16(tags::COMPRESSION, hints.compression)?;
        // Only set Predictor tag for compressions that support it (LZW, Deflate).
        // libtiff rejects the Predictor tag for uncompressed images.
        if hints.compression == tags::COMPRESSION_LZW
            || hints.compression == tags::COMPRESSION_DEFLATE
        {
            handle.set_field_u16(tags::PREDICTOR, hints.predictor)?;
        }
        handle.set_field_u16(tags::PLANAR_CONFIGURATION, hints.planar_config)?;

        let tiles_across = (num_cols + hints.tile_width - 1) / hints.tile_width;
        let tiles_down = (num_rows + hints.tile_height - 1) / hints.tile_height;

        let is_planar = hints.planar_config == tags::PLANAR_CONFIG_SEPARATE;

        // Iterate over the block grid
        for block_row in 0..tiles_down {
            for block_col in 0..tiles_across {
                let (block_data, shape) = image.get_block(block_row, block_col, 0, None)?;
                let [_bands, actual_rows, actual_cols] = shape;

                let needs_padding = actual_rows < hints.tile_height
                    || actual_cols < hints.tile_width;

                // Pad edge tiles if needed
                let padded = if needs_padding {
                    pad_tile(
                        &block_data,
                        actual_rows,
                        actual_cols,
                        hints.tile_height,
                        hints.tile_width,
                        num_bands,
                        bytes_per_sample,
                        Self::pad_byte(image.pad_pixel_value()),
                    )
                } else {
                    block_data
                };

                if is_planar {
                    // Write each band as a separate tile plane
                    let tiles_per_plane = tiles_across * tiles_down;
                    let plane_size =
                        (hints.tile_height as usize) * (hints.tile_width as usize) * (bytes_per_sample as usize);
                    for band in 0..num_bands {
                        let tile_index =
                            band * tiles_per_plane + block_row * tiles_across + block_col;
                        let src_offset = band as usize * plane_size;
                        let band_data = &padded[src_offset..src_offset + plane_size];
                        handle.write_encoded_tile(tile_index, band_data)?;
                    }
                } else {
                    // Convert CHW → HWC and write as a single tile
                    let pixels_in_tile =
                        hints.tile_height * hints.tile_width;
                    let interleaved =
                        bsq_to_interleaved(&padded, num_bands, pixels_in_tile, bytes_per_sample);
                    let tile_index = block_row * tiles_across + block_col;
                    handle.write_encoded_tile(tile_index, &interleaved)?;
                }
            }
        }

        // Finalize this IFD
        handle.write_directory()?;

        Ok(())
    }
}

// =============================================================================
// DatasetWriter Implementation
// =============================================================================

impl DatasetWriter for TIFFDatasetWriter {
    fn add_asset(
        &mut self,
        key: &str,
        provider: Arc<dyn AssetProvider>,
        title: &str,
        description: &str,
        roles: &[String],
    ) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Writer has been closed",
            )));
        }

        // Only image assets are supported
        if provider.asset_type() != AssetType::Image {
            return Err(CodecError::Unsupported(format!(
                "TIFF does not support {:?} asset types, only Image",
                provider.asset_type()
            )));
        }

        // Reject duplicate keys
        if self.asset_keys.contains(key) {
            return Err(CodecError::DuplicateKey(key.to_string()));
        }

        self.assets.push(QueuedImageAsset {
            key: key.to_string(),
            provider,
            title: title.to_string(),
            description: description.to_string(),
            roles: roles.to_vec(),
        });
        self.asset_keys.insert(key.to_string());

        Ok(())
    }

    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
        self.metadata = Some(metadata);
        Ok(())
    }

    fn close(&mut self) -> Result<(), CodecError> {
        if self.closed {
            return Ok(());
        }

        // Parse encoding hints
        let hints = match &self.metadata {
            Some(meta) => TiffEncodingHints::from_metadata(meta.as_ref())?,
            None => TiffEncodingHints::default(),
        };

        // Open libtiff in write mode
        let handle = TiffHandle::from_write()?;

        // Write each queued image as a separate IFD
        for asset in &self.assets {
            let image = Self::get_image_provider(&asset.provider).ok_or_else(|| {
                CodecError::Unsupported(format!(
                    "Asset '{}' does not implement ImageAssetProvider",
                    asset.key
                ))
            })?;

            Self::write_image_ifd(&handle, image, &hints)?;
        }

        // Extract assembled TIFF bytes and write to disk
        let bytes = handle.into_bytes()?;
        std::fs::write(&self.path, &bytes).map_err(CodecError::Io)?;

        self.closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffered::{BufferedImageAssetProvider, BufferedMetadataProvider, BufferedTextAssetProvider, MemoryImageConfig};

    /// Helper: create a minimal 1-band 256×256 UInt8 image provider with block data populated.
    fn make_image_provider(key: &str) -> Arc<BufferedImageAssetProvider> {
        let config = MemoryImageConfig::new(256, 256)
            .with_bands(1)
            .with_block_size(256, 256)
            .with_pixel_type(PixelType::UInt8);
        let provider = BufferedImageAssetProvider::new(key, config);
        let data = vec![42u8; 256 * 256];
        provider.set_block(0, 0, &data).unwrap();
        Arc::new(provider)
    }

    /// Helper: create a text asset provider (non-image).
    fn make_text_provider() -> Arc<BufferedTextAssetProvider> {
        Arc::new(BufferedTextAssetProvider::new("text_0", "hello".to_string(), "UTF8"))
    }

    // =========================================================================
    // Constructor
    // =========================================================================

    #[test]
    fn writer_new_creates_instance() {
        let writer = TIFFDatasetWriter::new("/tmp/test_new.tif");
        assert!(writer.is_ok());
        let w = writer.unwrap();
        assert!(!w.closed);
        assert!(w.assets.is_empty());
        assert!(w.metadata.is_none());
    }

    // =========================================================================
    // add_asset
    // =========================================================================

    #[test]
    fn writer_add_image_asset_succeeds() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();
        let provider = make_image_provider("image_segment_0");
        let result = writer.add_asset(
            "image_segment_0",
            provider,
            "Image 0",
            "Test image",
            &["data".to_string()],
        );
        assert!(result.is_ok());
        assert_eq!(writer.assets.len(), 1);
        assert!(writer.asset_keys.contains("image_segment_0"));
    }

    #[test]
    fn writer_add_non_image_asset_rejected() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();
        let text = make_text_provider();
        let result = writer.add_asset("text_0", text, "Text", "desc", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, CodecError::Unsupported(_)),
            "Expected Unsupported error, got: {:?}",
            err
        );
        // No asset should have been queued
        assert!(writer.assets.is_empty());
    }

    #[test]
    fn writer_add_duplicate_key_rejected() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();
        let p1 = make_image_provider("img_0");
        let p2 = make_image_provider("img_1");
        writer
            .add_asset("image_segment_0", p1, "Image 0", "desc", &[])
            .unwrap();
        let result = writer.add_asset("image_segment_0", p2, "Image 0 dup", "desc", &[]);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CodecError::DuplicateKey(_)),
            "Expected DuplicateKey error"
        );
        assert_eq!(writer.assets.len(), 1);
    }

    #[test]
    fn writer_add_asset_after_close_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        // Add one asset so close() has something to write
        let provider = make_image_provider("image_segment_0");
        writer
            .add_asset("image_segment_0", provider, "Image", "desc", &[])
            .unwrap();
        writer.close().unwrap();

        let p2 = make_image_provider("img_1");
        let result = writer.add_asset("image_segment_1", p2, "Image 1", "desc", &[]);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CodecError::Io(_)),
            "Expected Io error after close"
        );
    }

    // =========================================================================
    // close() idempotent
    // =========================================================================

    #[test]
    fn writer_close_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_idempotent.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image_segment_0");
        writer
            .add_asset("image_segment_0", provider, "Image", "desc", &[])
            .unwrap();

        assert!(writer.close().is_ok());
        // Second close should also succeed
        assert!(writer.close().is_ok());
    }

    // =========================================================================
    // set_metadata
    // =========================================================================

    #[test]
    fn writer_set_metadata_stores_latest() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();

        let meta1 = BufferedMetadataProvider::new();
        meta1.set("Compression", "LZW");
        writer.set_metadata(Arc::new(meta1)).unwrap();

        let meta2 = BufferedMetadataProvider::new();
        meta2.set("Compression", "None");
        writer.set_metadata(Arc::new(meta2)).unwrap();

        // The latest metadata should be used
        let dict = writer.metadata.as_ref().unwrap().as_dict(None);
        assert_eq!(
            dict.get("Compression"),
            Some(&serde_json::json!("None"))
        );
    }

    // =========================================================================
    // Encoding hint parsing
    // =========================================================================

    #[test]
    fn writer_default_encoding_hints() {
        let hints = TiffEncodingHints::default();
        assert_eq!(hints.tile_width, 256);
        assert_eq!(hints.tile_height, 256);
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
        assert_eq!(hints.predictor, 2); // Horizontal for Deflate
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_CONTIG);
    }

    #[test]
    fn writer_parse_compression_none() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "None");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.compression, tags::COMPRESSION_NONE);
    }

    #[test]
    fn writer_parse_compression_lzw() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "LZW");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.compression, tags::COMPRESSION_LZW);
    }

    #[test]
    fn writer_parse_compression_deflate() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "Deflate");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
    }

    #[test]
    fn writer_parse_predictor_horizontal() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Predictor", "Horizontal");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 2);
    }

    #[test]
    fn writer_parse_predictor_none() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Predictor", "None");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 1);
    }

    #[test]
    fn writer_predictor_default_with_lzw_compression() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "LZW");
        // No explicit Predictor → should default to Horizontal (2)
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 2);
    }

    #[test]
    fn writer_predictor_default_with_deflate_compression() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "Deflate");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 2);
    }

    #[test]
    fn writer_predictor_default_without_compression() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "None");
        // No explicit Predictor + no compression → should default to None (1)
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 1);
    }

    #[test]
    fn writer_parse_tile_dimensions() {
        let meta = BufferedMetadataProvider::new();
        meta.set("TileWidth", "512");
        meta.set("TileHeight", "128");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.tile_width, 512);
        assert_eq!(hints.tile_height, 128);
    }

    #[test]
    fn writer_parse_planar_configuration_chunky() {
        let meta = BufferedMetadataProvider::new();
        meta.set("PlanarConfiguration", "Chunky");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_CONTIG);
    }

    #[test]
    fn writer_parse_planar_configuration_planar() {
        let meta = BufferedMetadataProvider::new();
        meta.set("PlanarConfiguration", "Planar");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_SEPARATE);
    }

    #[test]
    fn writer_parse_invalid_compression_returns_error() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "JPEG");
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn writer_parse_invalid_predictor_returns_error() {
        let meta = BufferedMetadataProvider::new();
        meta.set("Predictor", "FloatingPoint");
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    // =========================================================================
    // bsq_to_interleaved
    // =========================================================================

    #[test]
    fn writer_bsq_to_interleaved_3band() {
        // 3 bands, 2 pixels per band, 1 byte per sample
        // Input CHW: [R0 R1 | G0 G1 | B0 B1]
        let input: Vec<u8> = vec![10, 20, 30, 40, 50, 60];
        let result = bsq_to_interleaved(&input, 3, 2, 1);
        // Output HWC: [R0 G0 B0 | R1 G1 B1]
        assert_eq!(result, vec![10, 30, 50, 20, 40, 60]);
    }

    #[test]
    fn writer_bsq_to_interleaved_single_band() {
        // 1 band, 4 pixels, 1 byte per sample → no change
        let input: Vec<u8> = vec![1, 2, 3, 4];
        let result = bsq_to_interleaved(&input, 1, 4, 1);
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn writer_bsq_to_interleaved_2byte_samples() {
        // 2 bands, 2 pixels, 2 bytes per sample
        // Input CHW: [R0_lo R0_hi R1_lo R1_hi | G0_lo G0_hi G1_lo G1_hi]
        let input: Vec<u8> = vec![0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00];
        let result = bsq_to_interleaved(&input, 2, 2, 2);
        // Output HWC: [R0_lo R0_hi G0_lo G0_hi | R1_lo R1_hi G1_lo G1_hi]
        assert_eq!(result, vec![0x01, 0x00, 0x03, 0x00, 0x02, 0x00, 0x04, 0x00]);
    }

    #[test]
    fn writer_bsq_to_interleaved_preserves_length() {
        let input: Vec<u8> = vec![0u8; 3 * 16 * 2]; // 3 bands, 16 pixels, 2 bps
        let result = bsq_to_interleaved(&input, 3, 16, 2);
        assert_eq!(result.len(), input.len());
    }

    // =========================================================================
    // sample_format mapping
    // =========================================================================

    #[test]
    fn writer_pixel_type_to_sample_format() {
        // Unsigned integers → SAMPLE_FORMAT_UINT (1)
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::UInt8), tags::SAMPLE_FORMAT_UINT);
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::UInt16), tags::SAMPLE_FORMAT_UINT);
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::UInt32), tags::SAMPLE_FORMAT_UINT);

        // Signed integers → SAMPLE_FORMAT_INT (2)
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::Int8), tags::SAMPLE_FORMAT_INT);
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::Int16), tags::SAMPLE_FORMAT_INT);
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::Int32), tags::SAMPLE_FORMAT_INT);

        // Floating point → SAMPLE_FORMAT_FLOAT (3)
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::Float32), tags::SAMPLE_FORMAT_FLOAT);
        assert_eq!(TIFFDatasetWriter::sample_format(PixelType::Float64), tags::SAMPLE_FORMAT_FLOAT);
    }

    // =========================================================================
    // photometric_interpretation
    // =========================================================================

    #[test]
    fn writer_photometric_interpretation_rgb() {
        // 3 or more bands → RGB (2)
        assert_eq!(TIFFDatasetWriter::photometric_interpretation(3), tags::PHOTOMETRIC_RGB);
        assert_eq!(TIFFDatasetWriter::photometric_interpretation(4), tags::PHOTOMETRIC_RGB);
    }

    #[test]
    fn writer_photometric_interpretation_minisblack() {
        // 1 or 2 bands → MinIsBlack (1)
        assert_eq!(TIFFDatasetWriter::photometric_interpretation(1), tags::PHOTOMETRIC_MINISBLACK);
        assert_eq!(TIFFDatasetWriter::photometric_interpretation(2), tags::PHOTOMETRIC_MINISBLACK);
    }

    // =========================================================================
    // pad_tile
    // =========================================================================

    #[test]
    fn writer_pad_tile_fills_edge() {
        // 1 band, actual 2×2 block, tile 4×4, 1 bps, pad with 0xFF
        let block_data = vec![1, 2, 3, 4]; // 2×2 pixels
        let padded = pad_tile(&block_data, 2, 2, 4, 4, 1, 1, 0xFF);
        // Expected: 4×4 tile with actual data in top-left, 0xFF elsewhere
        #[rustfmt::skip]
        let expected = vec![
            1, 2, 0xFF, 0xFF,
            3, 4, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert_eq!(padded, expected);
    }

    #[test]
    fn writer_pad_tile_no_padding_needed() {
        // When actual == tile dimensions, output should equal input
        let block_data = vec![10, 20, 30, 40]; // 2×2
        let padded = pad_tile(&block_data, 2, 2, 2, 2, 1, 1, 0);
        assert_eq!(padded, block_data);
    }

    // =========================================================================
    // Encoding hints from empty metadata (defaults)
    // =========================================================================

    #[test]
    fn writer_encoding_hints_from_empty_metadata() {
        let meta = BufferedMetadataProvider::new();
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.tile_width, 256);
        assert_eq!(hints.tile_height, 256);
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
        assert_eq!(hints.predictor, 2);
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_CONTIG);
    }

    // =========================================================================
    // proptest: bsq_to_interleaved properties
    // =========================================================================

    mod prop {
        use super::*;
        use proptest::prelude::*;

        /// Inverse of bsq_to_interleaved: HWC → CHW.
        fn interleaved_to_bsq(
            data: &[u8],
            num_bands: u32,
            pixels_per_band: u32,
            bytes_per_sample: u32,
        ) -> Vec<u8> {
            let bands = num_bands as usize;
            let pixels = pixels_per_band as usize;
            let bps = bytes_per_sample as usize;
            let mut out = vec![0u8; data.len()];
            let band_stride = pixels * bps;

            for band in 0..bands {
                let dst_offset = band * band_stride;
                for pixel in 0..pixels {
                    let src = (pixel * bands + band) * bps;
                    let dst = dst_offset + pixel * bps;
                    out[dst..dst + bps].copy_from_slice(&data[src..src + bps]);
                }
            }
            out
        }

        proptest! {
            #[test]
            fn prop_bsq_to_interleaved_roundtrip(
                num_bands in 1u32..=4,
                pixels_per_band in 1u32..=32,
                bps in prop_oneof![Just(1u32), Just(2u32), Just(4u32), Just(8u32)],
            ) {
                let len = (num_bands * pixels_per_band * bps) as usize;
                // Deterministic data based on index
                let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();

                let hwc = bsq_to_interleaved(&data, num_bands, pixels_per_band, bps);
                let back = interleaved_to_bsq(&hwc, num_bands, pixels_per_band, bps);

                prop_assert_eq!(&back, &data, "CHW → HWC → CHW roundtrip failed");
            }

            #[test]
            fn prop_bsq_to_interleaved_preserves_length(
                num_bands in 1u32..=4,
                pixels_per_band in 1u32..=64,
                bps in prop_oneof![Just(1u32), Just(2u32), Just(4u32), Just(8u32)],
            ) {
                let len = (num_bands * pixels_per_band * bps) as usize;
                let data = vec![0u8; len];

                let result = bsq_to_interleaved(&data, num_bands, pixels_per_band, bps);

                prop_assert_eq!(result.len(), data.len(), "Output length differs from input");
            }
        }
    }
}
