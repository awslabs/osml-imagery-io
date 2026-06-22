//! J2KDatasetWriter — implements DatasetWriter for standalone JPEG 2000 files.
//!
//! Encodes pixel data from an `ImageAssetProvider` into a valid JPEG 2000
//! codestream file. The writer queues a single image asset via `add_asset`,
//! stores optional metadata via `set_metadata`, and performs all encoding
//! work in `close()`.
//!
//! During `close()`:
//! 1. Reads all pixel data from the queued provider via `get_block(0, 0, 0, None)`
//! 2. Constructs `J2KEncodeParams` from image properties and encoding hints
//! 3. Encodes via the `J2KCodec` trait (start_encode → encode_tile → finalize)
//! 4. Writes the resulting codestream to disk
//!
//! # Encoding Hints
//!
//! Encoding parameters are read from the dataset-level metadata provider:
//! - `J2K_LOSSLESS` (bool) — lossless mode (default true for standalone files)
//! - `J2K_COMPRESSION_RATIO` (f64) — target compression ratio (ignored when lossless)
//! - `J2K_DECOMPOSITION_LEVELS` (u8) — decomposition levels (default 5)
//! - `J2K_QUALITY_LAYERS` (u8) — quality layers (default 1)
//! - `J2K_HTJK` (bool) — use HTJ2K (Part 15) encoding (default false)

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::assembly::TileAssembler;
use crate::error::CodecError;
use crate::j2k::codec::{J2KCodec, J2KEncodeParams};
use crate::traits::{AssetProvider, DatasetWriter, MetadataProvider};
use crate::types::{AssetType, PixelType};

#[cfg(feature = "openjpeg")]
use crate::j2k::openjpeg::get_j2k_codec;

/// Parsed encoding hints extracted from metadata.
struct J2KEncodingHints {
    lossless: bool,
    compression_ratio: Option<f64>,
    decomposition_levels: u8,
    quality_layers: u8,
    htj2k: bool,
    tile_width: Option<u32>,
    tile_height: Option<u32>,
}

/// An image asset queued for writing.
struct QueuedJ2KAsset {
    provider: AssetProvider,
    #[allow(dead_code)]
    key: String,
}

/// Writer for standalone JPEG 2000 datasets implementing the `DatasetWriter` trait.
///
/// Queues a single image asset and optional metadata, then encodes
/// everything into a JPEG 2000 codestream file on `close()`.
pub struct J2KDatasetWriter {
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
    assets: Vec<QueuedJ2KAsset>,
    /// J2K codec for encoding.
    codec: Arc<dyn J2KCodec>,
}

impl J2KDatasetWriter {
    /// Create a new writer targeting the given output path.
    ///
    /// The file is opened immediately and wrapped in a `BufWriter<File>`,
    /// then delegated to `new_with_output`.
    #[cfg(feature = "openjpeg")]
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
    #[cfg(feature = "openjpeg")]
    pub fn new_with_output(output: Box<dyn Write + Send>) -> Result<Self, CodecError> {
        Self::new_with_output_and_codec(output, get_j2k_codec())
    }

    /// Create a new writer with a specific codec (for testing).
    ///
    /// The file is opened immediately and wrapped in a `BufWriter<File>`,
    /// then delegated to `new_with_output_and_codec`.
    pub(crate) fn new_with_codec(
        path: impl AsRef<Path>,
        codec: Arc<dyn J2KCodec>,
    ) -> Result<Self, CodecError> {
        let file = File::create(path.as_ref()).map_err(CodecError::Io)?;
        let buf_writer = BufWriter::new(file);
        Self::new_with_output_and_codec(Box::new(buf_writer), codec)
    }

    /// Create a new writer with a specific output writer and codec (for testing).
    pub(crate) fn new_with_output_and_codec(
        output: Box<dyn Write + Send>,
        codec: Arc<dyn J2KCodec>,
    ) -> Result<Self, CodecError> {
        Ok(Self {
            output: Mutex::new(Some(output)),
            image_queued: false,
            metadata: None,
            closed: false,
            assets: Vec::new(),
            codec,
        })
    }

    /// Extract encoding hints from metadata, falling back to defaults.
    ///
    /// Standalone J2K files default to lossless encoding (unlike NITF which
    /// defaults to lossy 10:1). This matches user expectations for standalone
    /// file workflows where data fidelity is the priority.
    fn encoding_hints_from_metadata(metadata: Option<&dyn MetadataProvider>) -> J2KEncodingHints {
        let dict = metadata.map(|m| m.entries(None));

        let lossless = dict
            .as_ref()
            .and_then(|d| d.get("J2K_LOSSLESS"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // default lossless for standalone files

        let compression_ratio = if lossless {
            None
        } else {
            dict.as_ref()
                .and_then(|d| d.get("J2K_COMPRESSION_RATIO"))
                .and_then(|v| v.as_f64())
                .or(Some(10.0)) // default 10:1 when lossy
        };

        let decomposition_levels = dict
            .as_ref()
            .and_then(|d| d.get("J2K_DECOMPOSITION_LEVELS"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(5);

        let quality_layers = dict
            .as_ref()
            .and_then(|d| d.get("J2K_QUALITY_LAYERS"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(1);

        let htj2k = dict
            .as_ref()
            .and_then(|d| d.get("J2K_HTJK"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let tile_width = dict
            .as_ref()
            .and_then(|d| d.get("J2K_TILE_WIDTH"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let tile_height = dict
            .as_ref()
            .and_then(|d| d.get("J2K_TILE_HEIGHT"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        J2KEncodingHints {
            lossless,
            compression_ratio,
            decomposition_levels,
            quality_layers,
            htj2k,
            tile_width,
            tile_height,
        }
    }

    /// Derive `(bits_per_component, is_signed)` from a `PixelType`.
    fn pixel_type_to_siz(pixel_type: PixelType) -> (u8, bool) {
        match pixel_type {
            PixelType::UInt8 => (8, false),
            PixelType::Int8 => (8, true),
            PixelType::UInt16 => (16, false),
            PixelType::Int16 => (16, true),
            PixelType::UInt32 => (32, false),
            PixelType::Int32 => (32, true),
            PixelType::Float32 => (32, false), // treated as 32-bit unsigned by J2K
            PixelType::Float64 => (64, false),
        }
    }
}

// =============================================================================
// DatasetWriter Implementation
// =============================================================================

impl DatasetWriter for J2KDatasetWriter {
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

        // J2K supports only image assets
        if provider.asset_type() != AssetType::Image {
            return Err(CodecError::Unsupported(
                "J2K format supports only image assets".to_string(),
            ));
        }

        // J2K supports only a single image per file
        if self.image_queued {
            return Err(CodecError::Unsupported(
                "J2K format supports only a single image per file".to_string(),
            ));
        }

        self.assets.push(QueuedJ2KAsset {
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

        // Extract encoding hints from metadata
        let meta_ref = self.metadata.as_deref();
        let hints = Self::encoding_hints_from_metadata(meta_ref);

        let (bits_per_component, is_signed) = Self::pixel_type_to_siz(pixel_type);

        // Determine output tile dimensions: use metadata hints or default to full image
        let tile_width = hints.tile_width.unwrap_or(width);
        let tile_height = hints.tile_height.unwrap_or(height);

        let mut params = J2KEncodeParams {
            width,
            height,
            num_components: num_bands,
            bits_per_component,
            is_signed,
            compression_ratio: hints.compression_ratio,
            lossless: hints.lossless,
            num_decomposition_levels: hints.decomposition_levels,
            num_quality_layers: hints.quality_layers,
            htj2k: hints.htj2k,
            tile_width,
            tile_height,
        };
        params.clamp_decomposition_levels();

        // Encode using TileAssembler to handle grid mismatches
        let assembler = TileAssembler::new(image, tile_width, tile_height);
        let (grid_rows, grid_cols) = assembler.output_grid_size();

        let mut state = self
            .codec
            .start_encode(&params)
            .map_err(|e| CodecError::Encode(format!("J2K encode error: {}", e)))?;

        for row in 0..grid_rows {
            for col in 0..grid_cols {
                let tile_index = row * grid_cols + col;
                let (tile_data, _shape) = assembler.get_output_tile(row, col)?;
                state
                    .encode_tile(tile_index, &tile_data)
                    .map_err(|e| CodecError::Encode(format!("J2K encode error: {}", e)))?;
            }
        }

        let codestream = state
            .finalize()
            .map_err(|e| CodecError::Encode(format!("J2K encode error: {}", e)))?;

        // Write codestream to the stored output writer
        let mut output = self
            .output
            .lock()
            .map_err(|_| CodecError::Unsupported("J2K writer output mutex poisoned".to_string()))?
            .take()
            .ok_or_else(|| {
                CodecError::Unsupported("J2K writer output is not available".to_string())
            })?;
        output.write_all(&codestream).map_err(CodecError::Io)?;
        output.flush().map_err(CodecError::Io)?;

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(all(test, feature = "openjpeg"))]
mod tests {
    use super::*;
    use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
    use crate::j2k::reader::J2KDatasetReader;
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
        let writer = J2KDatasetWriter::new("/tmp/test_j2k_writer_new.j2k");
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
        let mut writer = J2KDatasetWriter::new("/tmp/test_j2k_add_image.j2k").unwrap();
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

        let mut writer = J2KDatasetWriter::new("/tmp/test_j2k_non_image.j2k").unwrap();
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
        let mut writer = J2KDatasetWriter::new("/tmp/test_j2k_dup.j2k").unwrap();
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
        let path = dir.path().join("closed.j2k");

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
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
    // test_close_idempotent — Double close is safe
    // =========================================================================

    #[test]
    fn test_close_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("idempotent.j2k");

        // Use 64x64 to avoid decomposition level issues with tiny images
        let npix = 64 * 64;
        let mut pixels = Vec::with_capacity(npix * 3);
        for band in 0u8..3 {
            for i in 0..npix {
                pixels.push(band.wrapping_mul(80).wrapping_add((i % 256) as u8));
            }
        }
        let provider = make_image_provider(64, 64, 3, PixelType::UInt8, &pixels);
        let mut writer = J2KDatasetWriter::new(&path).unwrap();
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
    // Roundtrip: write then read back grayscale 8-bit (lossless)
    // =========================================================================

    #[test]
    fn test_roundtrip_grayscale_8bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gray8.j2k");

        let npix = 64 * 64;
        let pixels: Vec<u8> = (0..npix).map(|i| (i % 256) as u8).collect();
        let provider = make_image_provider(64, 64, 1, PixelType::UInt8, &pixels);

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
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
        let reader = J2KDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(read_pixels, pixels);
    }

    // =========================================================================
    // Roundtrip: write then read back RGB 8-bit (lossless)
    // =========================================================================

    #[test]
    fn test_roundtrip_rgb_8bit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rgb8.j2k");

        // 64x64 RGB in BSQ
        let npix = 64 * 64usize;
        let mut pixels = Vec::with_capacity(npix * 3);
        for band in 0u8..3 {
            for i in 0..npix {
                pixels.push(band.wrapping_mul(80).wrapping_add((i % 256) as u8));
            }
        }
        let provider = make_image_provider(64, 64, 3, PixelType::UInt8, &pixels);

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
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
        let reader = J2KDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(read_pixels, pixels);
    }

    // =========================================================================
    // Roundtrip: 16-bit unsigned lossless
    // =========================================================================

    #[test]
    fn test_roundtrip_16bit_unsigned() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("u16.j2k");

        // 64x64 single-band UInt16 image
        let npix = 64 * 64;
        let values: Vec<u16> = (0..npix).map(|i| (i % 65536) as u16).collect();
        let pixels: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
        let provider = make_image_provider(64, 64, 1, PixelType::UInt16, &pixels);

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
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
        let reader = J2KDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        assert_eq!(image.pixel_value_type(), PixelType::UInt16);
        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(read_pixels, pixels);
    }

    // =========================================================================
    // Multi-tile: J2K_TILE_WIDTH/J2K_TILE_HEIGHT metadata → tiled output
    // =========================================================================

    #[test]
    fn test_roundtrip_multi_tile() {
        use crate::buffered::BufferedMetadataProvider;
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiled.j2k");

        // 128x128 RGB image, source has a single block (128x128)
        let npix = 128 * 128usize;
        let mut pixels = Vec::with_capacity(npix * 3);
        for band in 0u8..3 {
            for i in 0..npix {
                pixels.push(band.wrapping_mul(70).wrapping_add((i % 256) as u8));
            }
        }
        let provider = make_image_provider(128, 128, 3, PixelType::UInt8, &pixels);

        // Set tile metadata: 64x64 tiles → 2x2 tile grid
        let metadata = BufferedMetadataProvider::new();
        metadata.set("J2K_TILE_WIDTH", json!(64));
        metadata.set("J2K_TILE_HEIGHT", json!(64));

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();
        writer.set_metadata(Arc::new(metadata)).unwrap();
        writer.close().unwrap();

        // Read back and verify the J2K file has the expected tile grid
        let data = std::fs::read(&path).unwrap();
        let reader = J2KDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        // Verify tile grid reported by reader (2x2 for 128x128 with 64x64 tiles)
        assert_eq!(image.num_columns(), 128);
        assert_eq!(image.num_rows(), 128);
        assert_eq!(image.num_pixels_per_block_horizontal(), 64);
        assert_eq!(image.num_pixels_per_block_vertical(), 64);

        // Read all tiles and verify pixel correctness
        let mut reconstructed = vec![0u8; npix * 3];
        let bw = 64usize;
        let bh = 64usize;
        let img_w = 128usize;

        for tile_row in 0..2u32 {
            for tile_col in 0..2u32 {
                let (tile_data, shape) = image.get_block(tile_row, tile_col, 0, None).unwrap();
                assert_eq!(shape, [3, 64, 64]);

                // Copy into reconstructed full image (BSQ)
                for band in 0..3usize {
                    for row in 0..bh {
                        for col in 0..bw {
                            let img_x = tile_col as usize * bw + col;
                            let img_y = tile_row as usize * bh + row;
                            let src_off = band * bw * bh + row * bw + col;
                            let dst_off = band * img_w * img_w + img_y * img_w + img_x;
                            reconstructed[dst_off] = tile_data[src_off];
                        }
                    }
                }
            }
        }

        assert_eq!(reconstructed, pixels);
    }

    // =========================================================================
    // Multi-tile: source with different block grid from output tiles
    // =========================================================================

    #[test]
    fn test_roundtrip_multi_tile_mismatched_source_grid() {
        use crate::buffered::BufferedMetadataProvider;
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiled_mismatch.j2k");

        // 128x128 grayscale, source split into 32x32 blocks (4x4 grid)
        let npix = 128 * 128usize;
        let pixels: Vec<u8> = (0..npix).map(|i| (i % 256) as u8).collect();

        let config = MemoryImageConfig::new(128, 128)
            .with_bands(1)
            .with_block_size(32, 32)
            .with_pixel_type(PixelType::UInt8);
        let provider = BufferedImageAssetProvider::new("image:0", config);
        // Fill all 4x4=16 blocks
        for br in 0..4u32 {
            for bc in 0..4u32 {
                let mut block = Vec::with_capacity(32 * 32);
                for row in 0..32usize {
                    for col in 0..32usize {
                        let img_x = bc as usize * 32 + col;
                        let img_y = br as usize * 32 + row;
                        block.push(((img_y * 128 + img_x) % 256) as u8);
                    }
                }
                provider.set_block(br, bc, &block).unwrap();
            }
        }
        let provider = Arc::new(provider);

        // Output tiles: 64x64 (different from source's 32x32 blocks)
        let metadata = BufferedMetadataProvider::new();
        metadata.set("J2K_TILE_WIDTH", json!(64));
        metadata.set("J2K_TILE_HEIGHT", json!(64));

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();
        writer.set_metadata(Arc::new(metadata)).unwrap();
        writer.close().unwrap();

        // Read back
        let data = std::fs::read(&path).unwrap();
        let reader = J2KDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        assert_eq!(image.num_pixels_per_block_horizontal(), 64);
        assert_eq!(image.num_pixels_per_block_vertical(), 64);

        // Reconstruct and verify
        let mut reconstructed = vec![0u8; npix];
        for tile_row in 0..2u32 {
            for tile_col in 0..2u32 {
                let (tile_data, shape) = image.get_block(tile_row, tile_col, 0, None).unwrap();
                assert_eq!(shape, [1, 64, 64]);

                for row in 0..64usize {
                    for col in 0..64usize {
                        let img_x = tile_col as usize * 64 + col;
                        let img_y = tile_row as usize * 64 + row;
                        let src_off = row * 64 + col;
                        let dst_off = img_y * 128 + img_x;
                        reconstructed[dst_off] = tile_data[src_off];
                    }
                }
            }
        }

        assert_eq!(reconstructed, pixels);
    }

    // =========================================================================
    // Multi-tile: decomposition levels clamped per-tile
    // =========================================================================

    #[test]
    fn test_multi_tile_decomposition_levels_clamped() {
        use crate::buffered::BufferedMetadataProvider;
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small_tiles.j2k");

        // 64x64 image with 16x16 tiles. floor(log2(16))=4, so levels clamped from 5 to 4.
        let npix = 64 * 64usize;
        let pixels: Vec<u8> = (0..npix).map(|i| (i % 256) as u8).collect();
        let provider = make_image_provider(64, 64, 1, PixelType::UInt8, &pixels);

        let metadata = BufferedMetadataProvider::new();
        metadata.set("J2K_TILE_WIDTH", json!(16));
        metadata.set("J2K_TILE_HEIGHT", json!(16));
        metadata.set("J2K_DECOMPOSITION_LEVELS", json!(5));

        let mut writer = J2KDatasetWriter::new(&path).unwrap();
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Test",
                "Test",
                &[],
            )
            .unwrap();
        writer.set_metadata(Arc::new(metadata)).unwrap();
        writer.close().unwrap();

        // Should succeed (levels clamped internally) and produce valid output
        let data = std::fs::read(&path).unwrap();
        let reader = J2KDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("Expected Image variant");

        assert_eq!(image.num_columns(), 64);
        assert_eq!(image.num_rows(), 64);
        assert_eq!(image.num_pixels_per_block_horizontal(), 16);
        assert_eq!(image.num_pixels_per_block_vertical(), 16);

        // Verify pixel data is lossless
        let mut reconstructed = vec![0u8; npix];
        for tile_row in 0..4u32 {
            for tile_col in 0..4u32 {
                let (tile_data, shape) = image.get_block(tile_row, tile_col, 0, None).unwrap();
                assert_eq!(shape, [1, 16, 16]);

                for row in 0..16usize {
                    for col in 0..16usize {
                        let img_x = tile_col as usize * 16 + col;
                        let img_y = tile_row as usize * 16 + row;
                        reconstructed[img_y * 64 + img_x] = tile_data[row * 16 + col];
                    }
                }
            }
        }
        assert_eq!(reconstructed, pixels);
    }
}
