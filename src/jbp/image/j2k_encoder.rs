//! JPEG 2000 block encoder for NITF imagery.
//!
//! This module provides the `Jpeg2000BlockEncoder` which implements the
//! `BlockEncoder` trait for JPEG 2000 compressed imagery (IC=C8, CD).
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::j2k::encoder::Jpeg2000BlockEncoder;
//! use osml_imagery_io::jbp::j2k::comrat::J2KEncodingHints;
//! use osml_imagery_io::jbp::image::encoder::BlockEncoder;
//!
//! let hints = J2KEncodingHints::lossless();
//! let mut encoder = Jpeg2000BlockEncoder::new(
//!     512, 512, 3, 8, false, 256, 256, &hints
//! )?;
//! encoder.encode_block(0, 0, &block_data, [256, 256, 3])?;
//! let codestream = Box::new(encoder).finalize()?;
//! ```

use std::collections::HashSet;
use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::image::encoder::BlockEncoder;
use crate::j2k::{J2KCodec, J2KEncodeParams, J2KEncodeState};
use crate::j2k::comrat::J2KEncodingHints;

#[cfg(feature = "openjpeg")]
use crate::j2k::get_j2k_codec;

// =============================================================================
// Jpeg2000BlockEncoder
// =============================================================================

/// Block encoder for JPEG 2000 compressed NITF imagery (IC=C8, CD).
///
/// This encoder implements the `BlockEncoder` trait, allowing JPEG 2000
/// compressed images to be written tile-by-tile without loading the entire
/// image into memory.
///
/// # Tile-Based Encoding
///
/// The encoder uses the underlying J2K codec's tile-based encoding capability.
/// Each call to `encode_block()` encodes one tile of the image. The tiles
/// must be encoded in row-major order (left-to-right, top-to-bottom).
///
/// # Thread Safety
///
/// The encoder is `Send + Sync` to allow use from multiple threads, though
/// the underlying codec may serialize tile encoding internally.
pub struct Jpeg2000BlockEncoder {
    /// The J2K codec to use for encoding
    #[allow(dead_code)]
    codec: Arc<dyn J2KCodec>,
    /// Encoding state from the codec
    encode_state: Box<dyn J2KEncodeState>,
    /// Block grid dimensions (rows, cols)
    block_grid: (u32, u32),
    /// Block dimensions in pixels (height, width)
    block_dims: (u32, u32),
    /// Compression type (C8 or CD)
    ic: String,
    /// Track which blocks have been encoded
    encoded_blocks: HashSet<(u32, u32)>,
    /// Bytes per pixel (derived from bits per pixel)
    bytes_per_pixel: usize,
}

impl std::fmt::Debug for Jpeg2000BlockEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jpeg2000BlockEncoder")
            .field("block_grid", &self.block_grid)
            .field("block_dims", &self.block_dims)
            .field("ic", &self.ic)
            .field("encoded_blocks", &self.encoded_blocks.len())
            .field("bytes_per_pixel", &self.bytes_per_pixel)
            .finish()
    }
}


impl Jpeg2000BlockEncoder {
    /// Create a new JPEG 2000 block encoder.
    ///
    /// # Arguments
    /// * `nrows` - Image height in pixels
    /// * `ncols` - Image width in pixels
    /// * `nbands` - Number of bands (components)
    /// * `nbpp` - Bits per pixel (1-38 for J2K)
    /// * `is_signed` - Whether pixel values are signed
    /// * `nppbh` - Block width in pixels (tile width)
    /// * `nppbv` - Block height in pixels (tile height)
    /// * `hints` - Encoding hints specifying compression parameters
    ///
    /// # Errors
    /// Returns `CodecError::Unsupported` if the codec doesn't support HTJ2K
    /// encoding when `hints.htj2k` is true.
    #[cfg(feature = "openjpeg")]
    pub fn new(
        nrows: u32,
        ncols: u32,
        nbands: u32,
        nbpp: u8,
        is_signed: bool,
        nppbh: u32,
        nppbv: u32,
        hints: &J2KEncodingHints,
    ) -> Result<Self, CodecError> {
        Self::with_codec(
            get_j2k_codec(),
            nrows,
            ncols,
            nbands,
            nbpp,
            is_signed,
            nppbh,
            nppbv,
            hints,
        )
    }

    /// Create a new JPEG 2000 block encoder with a specific codec.
    ///
    /// This constructor allows injecting a custom codec for testing or
    /// using alternative codec implementations.
    ///
    /// # Arguments
    /// * `codec` - The J2K codec to use for encoding
    /// * `nrows` - Image height in pixels
    /// * `ncols` - Image width in pixels
    /// * `nbands` - Number of bands (components)
    /// * `nbpp` - Bits per pixel (1-38 for J2K)
    /// * `is_signed` - Whether pixel values are signed
    /// * `nppbh` - Block width in pixels (tile width)
    /// * `nppbv` - Block height in pixels (tile height)
    /// * `hints` - Encoding hints specifying compression parameters
    ///
    /// # Errors
    /// Returns `CodecError::Unsupported` if the codec doesn't support HTJ2K
    /// encoding when `hints.htj2k` is true.
    pub fn with_codec(
        codec: Arc<dyn J2KCodec>,
        nrows: u32,
        ncols: u32,
        nbands: u32,
        nbpp: u8,
        is_signed: bool,
        nppbh: u32,
        nppbv: u32,
        hints: &J2KEncodingHints,
    ) -> Result<Self, CodecError> {
        // Check codec capabilities for HTJ2K
        let capabilities = codec.capabilities();
        if hints.htj2k && !capabilities.htj2k_encode {
            return Err(CodecError::Unsupported(format!(
                "Codec '{}' does not support HTJ2K encoding",
                capabilities.name
            )));
        }

        // Validate bit depth
        if nbpp > capabilities.max_bit_depth {
            return Err(CodecError::Unsupported(format!(
                "Bit depth {} exceeds codec '{}' maximum of {}",
                nbpp, capabilities.name, capabilities.max_bit_depth
            )));
        }

        // Build encoding parameters
        let params = J2KEncodeParams {
            width: ncols,
            height: nrows,
            num_components: nbands,
            bits_per_component: nbpp,
            is_signed,
            compression_ratio: hints.compression_ratio,
            lossless: hints.lossless,
            num_decomposition_levels: hints.decomposition_levels,
            num_quality_layers: hints.quality_layers,
            htj2k: hints.htj2k,
            tile_width: nppbh,
            tile_height: nppbv,
        };

        // Start encoding
        let encode_state = codec.start_encode(&params)?;

        // Calculate block grid based on tile size
        let block_cols = ncols.div_ceil(nppbh);
        let block_rows = nrows.div_ceil(nppbv);

        // Calculate bytes per pixel (round up bits to bytes)
        let bytes_per_pixel = (nbpp as usize).div_ceil(8);

        Ok(Self {
            codec,
            encode_state,
            block_grid: (block_rows, block_cols),
            block_dims: (nppbv, nppbh),
            ic: if hints.htj2k { "CD" } else { "C8" }.to_string(),
            encoded_blocks: HashSet::new(),
            bytes_per_pixel,
        })
    }
}


impl BlockEncoder for Jpeg2000BlockEncoder {
    fn encode_block(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        shape: [u32; 3],
    ) -> Result<(), CodecError> {
        // Validate block coordinates
        if block_row >= self.block_grid.0 || block_col >= self.block_grid.1 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                0,
            ));
        }

        // Validate data size matches shape (accounting for bytes per pixel)
        // Shape is [bands, rows, cols], data is raw bytes
        let num_elements = (shape[0] as usize) * (shape[1] as usize) * (shape[2] as usize);
        let expected_size = num_elements * self.bytes_per_pixel;
        if data.len() != expected_size {
            return Err(CodecError::Encode(format!(
                "Data size {} doesn't match shape {:?} with {} bytes/pixel (expected {})",
                data.len(),
                shape,
                self.bytes_per_pixel,
                expected_size
            )));
        }

        // Calculate tile index (row-major order)
        let tile_index = block_row * self.block_grid.1 + block_col;

        // Internal representation is native-endian, which is what OpenJPEG
        // expects. Pass data through directly.
        self.encode_state.encode_tile(tile_index, data)?;

        // Track encoded block
        self.encoded_blocks.insert((block_row, block_col));

        Ok(())
    }

    fn skip_block(&mut self, block_row: u32, block_col: u32) -> Result<(), CodecError> {
        // Validate block coordinates
        if block_row >= self.block_grid.0 || block_col >= self.block_grid.1 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                0,
            ));
        }

        // Mark block as handled (skipped for masked images)
        self.encoded_blocks.insert((block_row, block_col));

        Ok(())
    }

    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError> {
        // Verify all blocks were encoded
        let expected_blocks = self.block_grid.0 * self.block_grid.1;
        if self.encoded_blocks.len() != expected_blocks as usize {
            return Err(CodecError::Encode(format!(
                "Incomplete encoding: {} of {} blocks encoded",
                self.encoded_blocks.len(),
                expected_blocks
            )));
        }

        self.encode_state.finalize()
    }

    fn compression_type(&self) -> &str {
        &self.ic
    }

    fn block_grid_size(&self) -> (u32, u32) {
        self.block_grid
    }

    fn block_dimensions(&self) -> (u32, u32) {
        self.block_dims
    }
}

// Safety: Jpeg2000BlockEncoder can be sent between threads
// The encode_state is Send, and the codec is Send + Sync
unsafe impl Send for Jpeg2000BlockEncoder {}
unsafe impl Sync for Jpeg2000BlockEncoder {}


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j2k::{J2KCodecCapabilities, J2KDecodeParams, J2KDecodeResult};

    /// Mock J2K codec for testing
    struct MockJ2KCodec {
        htj2k_encode: bool,
        max_bit_depth: u8,
    }

    impl MockJ2KCodec {
        fn new() -> Self {
            Self {
                htj2k_encode: false,
                max_bit_depth: 38,
            }
        }

        fn with_htj2k_support(mut self) -> Self {
            self.htj2k_encode = true;
            self
        }

        fn with_max_bit_depth(mut self, depth: u8) -> Self {
            self.max_bit_depth = depth;
            self
        }
    }

    /// Mock encode state for testing
    struct MockEncodeState {
        tiles_written: u32,
        total_tiles: u32,
        tile_data: Vec<Vec<u8>>,
    }

    impl MockEncodeState {
        fn new(total_tiles: u32) -> Self {
            Self {
                tiles_written: 0,
                total_tiles,
                tile_data: Vec::new(),
            }
        }
    }

    impl J2KEncodeState for MockEncodeState {
        fn encode_tile(&mut self, tile_index: u32, data: &[u8]) -> Result<(), CodecError> {
            if tile_index >= self.total_tiles {
                return Err(CodecError::Encode(format!(
                    "Tile index {} out of range",
                    tile_index
                )));
            }
            self.tile_data.push(data.to_vec());
            self.tiles_written += 1;
            Ok(())
        }

        fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError> {
            if self.tiles_written != self.total_tiles {
                return Err(CodecError::Encode(format!(
                    "Incomplete: {} of {} tiles",
                    self.tiles_written, self.total_tiles
                )));
            }
            // Return mock codestream (J2K magic bytes + dummy data)
            let mut result = vec![0xFF, 0x4F, 0xFF, 0x51]; // SOC + SIZ markers
            for tile in &self.tile_data {
                result.extend_from_slice(tile);
            }
            Ok(result)
        }
    }

    impl J2KCodec for MockJ2KCodec {
        fn capabilities(&self) -> J2KCodecCapabilities {
            J2KCodecCapabilities {
                max_bit_depth: self.max_bit_depth,
                htj2k_decode: self.htj2k_encode,
                htj2k_encode: self.htj2k_encode,
                name: "MockCodec",
            }
        }

        fn decode(
            &self,
            _codestream: &[u8],
            _params: &J2KDecodeParams,
        ) -> Result<J2KDecodeResult, CodecError> {
            Err(CodecError::Unsupported("Mock codec doesn't decode".into()))
        }

        fn start_encode(
            &self,
            params: &J2KEncodeParams,
        ) -> Result<Box<dyn J2KEncodeState>, CodecError> {
            let tiles_x = (params.width + params.tile_width - 1) / params.tile_width;
            let tiles_y = (params.height + params.tile_height - 1) / params.tile_height;
            Ok(Box::new(MockEncodeState::new(tiles_x * tiles_y)))
        }

        fn get_resolution_levels(&self, _codestream: &[u8]) -> Result<u32, CodecError> {
            Ok(6)
        }

        fn get_dimensions(&self, _codestream: &[u8]) -> Result<(u32, u32, u32), CodecError> {
            Ok((256, 256, 3))
        }

        fn get_tile_info(&self, _codestream: &[u8]) -> Result<(u32, u32, u32, u32), CodecError> {
            Ok((256, 256, 1, 1)) // Single tile by default
        }

        fn decode_tile(
            &self,
            _codestream: &[u8],
            _tile_index: u32,
            _params: &J2KDecodeParams,
        ) -> Result<J2KDecodeResult, CodecError> {
            Err(CodecError::Unsupported("Mock codec doesn't decode tiles".into()))
        }
    }

    #[test]
    fn test_new_encoder_c8() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::default();
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        )
        .unwrap();

        assert_eq!(encoder.compression_type(), "C8");
        assert_eq!(encoder.block_grid_size(), (2, 2));
        assert_eq!(encoder.block_dimensions(), (128, 128));
    }

    #[test]
    fn test_new_encoder_cd() {
        let codec = Arc::new(MockJ2KCodec::new().with_htj2k_support());
        let hints = J2KEncodingHints::htj2k(false);
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        )
        .unwrap();

        assert_eq!(encoder.compression_type(), "CD");
    }

    #[test]
    fn test_htj2k_not_supported() {
        let codec = Arc::new(MockJ2KCodec::new()); // No HTJ2K support
        let hints = J2KEncodingHints::htj2k(false);
        let result = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("HTJ2K"));
    }

    #[test]
    fn test_bit_depth_exceeds_max() {
        let codec = Arc::new(MockJ2KCodec::new().with_max_bit_depth(16));
        let hints = J2KEncodingHints::default();
        let result = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 32, false, 128, 128, &hints,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Bit depth"));
    }

    #[test]
    fn test_encode_block_validates_coordinates() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::default();
        let mut encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        )
        .unwrap();

        // Valid coordinates
        let data = vec![0u8; 128 * 128 * 3];
        assert!(encoder.encode_block(0, 0, &data, [128, 128, 3]).is_ok());

        // Invalid row
        let result = encoder.encode_block(5, 0, &data, [128, 128, 3]);
        assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(5, 0, 0))));

        // Invalid column
        let result = encoder.encode_block(0, 5, &data, [128, 128, 3]);
        assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(0, 5, 0))));
    }

    #[test]
    fn test_encode_block_validates_data_size() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::default();
        let mut encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        )
        .unwrap();

        // Wrong data size
        let data = vec![0u8; 100]; // Too small
        let result = encoder.encode_block(0, 0, &data, [128, 128, 3]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Data size"));
    }

    #[test]
    fn test_finalize_fails_if_blocks_missing() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::default();
        let mut encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        )
        .unwrap();

        // Only encode one block
        let data = vec![0u8; 128 * 128 * 3];
        encoder.encode_block(0, 0, &data, [128, 128, 3]).unwrap();

        // Finalize should fail
        let result = Box::new(encoder).finalize();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Incomplete"));
    }

    #[test]
    fn test_finalize_succeeds_when_all_blocks_encoded() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::default();
        let mut encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 128, 128, &hints,
        )
        .unwrap();

        // Encode all 4 blocks (2x2 grid)
        let data = vec![0u8; 128 * 128 * 3];
        encoder.encode_block(0, 0, &data, [128, 128, 3]).unwrap();
        encoder.encode_block(0, 1, &data, [128, 128, 3]).unwrap();
        encoder.encode_block(1, 0, &data, [128, 128, 3]).unwrap();
        encoder.encode_block(1, 1, &data, [128, 128, 3]).unwrap();

        // Finalize should succeed
        let result = Box::new(encoder).finalize();
        assert!(result.is_ok());
        let codestream = result.unwrap();
        // Check J2K magic bytes
        assert_eq!(&codestream[0..2], &[0xFF, 0x4F]);
    }

    #[test]
    fn test_block_grid_calculation() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::default();

        // Exact fit: 256x256 with 128x128 tiles = 2x2 grid
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec.clone(), 256, 256, 1, 8, false, 128, 128, &hints,
        )
        .unwrap();
        assert_eq!(encoder.block_grid_size(), (2, 2));

        // Non-exact fit: 300x200 with 128x128 tiles = 2x3 grid (ceil)
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec.clone(), 200, 300, 1, 8, false, 128, 128, &hints,
        )
        .unwrap();
        assert_eq!(encoder.block_grid_size(), (2, 3));

        // Single tile: 64x64 with 128x128 tiles = 1x1 grid
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 64, 64, 1, 8, false, 128, 128, &hints,
        )
        .unwrap();
        assert_eq!(encoder.block_grid_size(), (1, 1));
    }

    #[test]
    fn test_lossless_hints() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::lossless();
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 256, 256, &hints,
        )
        .unwrap();

        assert_eq!(encoder.compression_type(), "C8");
    }

    #[test]
    fn test_lossy_hints() {
        let codec = Arc::new(MockJ2KCodec::new());
        let hints = J2KEncodingHints::lossy(20.0);
        let encoder = Jpeg2000BlockEncoder::with_codec(
            codec, 256, 256, 3, 8, false, 256, 256, &hints,
        )
        .unwrap();

        assert_eq!(encoder.compression_type(), "C8");
    }
}
