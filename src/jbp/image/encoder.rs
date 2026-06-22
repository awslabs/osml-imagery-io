//! Block encoder trait and implementations for NITF image data.
//!
//! This module provides the strategy pattern for encoding image blocks to
//! various compression formats. The [`BlockEncoder`] trait defines the interface,
//! and implementations handle specific compression types.
//!
//! # Supported Compression Types
//!
//! | IC Code | Description | Implementation |
//! |---------|-------------|----------------|
//! | NC | No compression | [`nc_encoder::UncompressedBlockEncoder`](super::nc_encoder::UncompressedBlockEncoder) |
//! | C3 | JPEG DCT | [`JpegNitfBlockEncoder`](super::jpeg_encoder::JpegNitfBlockEncoder) |
//! | M3 | JPEG DCT with mask | [`JpegNitfBlockEncoder`](super::jpeg_encoder::JpegNitfBlockEncoder) |
//! | I1 | Downsampled JPEG | [`JpegNitfBlockEncoder`](super::jpeg_encoder::JpegNitfBlockEncoder) |
//! | C8 | JPEG 2000 Part 1 | [`Jpeg2000BlockEncoder`] |
//! | CD | JPEG 2000 Part 15 (HTJ2K) | [`Jpeg2000BlockEncoder`] |
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::encoder::{create_block_encoder, BlockEncoder};
//! use osml_io::jbp::image::types::InterleaveMode;
//!
//! let mut encoder = create_block_encoder(
//!     "NC", 64, 64, 3, 8, false, InterleaveMode::B, 32, 32, None, None
//! )?;
//! // Shape is [bands, rows, cols] (CHW format)
//! encoder.encode_block(0, 0, &block_data, [3, 32, 32])?;
//! let encoded = Box::new(encoder).finalize()?;
//! ```

use crate::error::CodecError;
use crate::jbp::image::nc_encoder::UncompressedBlockEncoder;
use crate::jbp::image::types::InterleaveMode;

#[cfg(feature = "openjpeg")]
use crate::j2k::J2KEncodingHints;
#[cfg(feature = "openjpeg")]
use crate::jbp::image::j2k_encoder::Jpeg2000BlockEncoder;

#[cfg(feature = "libjpeg-turbo")]
use crate::jbp::image::jpeg_encoder::JpegNitfBlockEncoder;

/// Convert a byte buffer from native-endian to big-endian.
///
/// NITF mandates big-endian for uncompressed multi-byte pixel data
/// (JBP Section 4.6.2, requirement JBP-2021.2-013). The internal `Vec<u8>`
/// contract uses native-endian, so this converts at the write boundary.
///
/// For single-byte data (`bytes_per_pixel == 1`) this is a no-op.
#[inline]
pub fn swap_ne_to_be(data: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
    if cfg!(target_endian = "big") || bytes_per_pixel <= 1 {
        return data.to_vec();
    }
    match bytes_per_pixel {
        2 => data
            .chunks_exact(2)
            .flat_map(|c| u16::from_ne_bytes([c[0], c[1]]).to_be_bytes())
            .collect(),
        4 => data
            .chunks_exact(4)
            .flat_map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]).to_be_bytes())
            .collect(),
        8 => data
            .chunks_exact(8)
            .flat_map(|c| {
                u64::from_ne_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]).to_be_bytes()
            })
            .collect(),
        _ => data.to_vec(),
    }
}

/// Trait for encoding image blocks to various compression formats.
///
/// This trait is symmetric to `BlockDecoder` and defines the interface for
/// block-based image encoding. Different compression formats implement this
/// trait, allowing the writer to delegate to the appropriate encoder based
/// on the IC field.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent
/// block encoding from multiple threads.
pub trait BlockEncoder: Send + Sync {
    /// Encode a single block of image data.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    /// * `data` - Pixel data in band-sequential format
    /// * `shape` - Shape of the data as [bands, rows, cols] (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if coordinates are out of bounds.
    /// Returns `CodecError::Encode` if data size doesn't match shape.
    fn encode_block(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        shape: [u32; 3],
    ) -> Result<(), CodecError>;

    /// Mark a block as intentionally skipped (for masked images).
    ///
    /// This method is used for masked images where some blocks are intentionally
    /// not encoded. Calling this method marks the block as "handled" so that
    /// `finalize()` won't fail due to missing blocks.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if coordinates are out of bounds.
    fn skip_block(&mut self, block_row: u32, block_col: u32) -> Result<(), CodecError>;

    /// Finalize encoding and return the complete encoded image data.
    ///
    /// This method must be called after all blocks have been encoded.
    /// The returned data is ready to be written to the NITF image segment.
    ///
    /// # Errors
    /// Returns `CodecError::Encode` if not all blocks have been encoded.
    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError>;

    /// Get the compression type identifier.
    ///
    /// # Returns
    /// The IC field value (e.g., "NC", "C8").
    fn compression_type(&self) -> &str;

    /// Get the block grid dimensions.
    ///
    /// # Returns
    /// A tuple of (num_block_rows, num_block_cols).
    fn block_grid_size(&self) -> (u32, u32);

    /// Get the output block dimensions in pixels.
    ///
    /// # Returns
    /// A tuple of (block_height, block_width).
    fn block_dimensions(&self) -> (u32, u32);
}

/// Factory function to create the appropriate block encoder based on IC field.
///
/// # Arguments
/// * `ic` - Image compression code (e.g., "NC", "C8", "CD", "C3", "M3", "I1")
/// * `nrows` - Image height in pixels
/// * `ncols` - Image width in pixels
/// * `nbands` - Number of bands
/// * `nbpp` - Bits per pixel
/// * `is_signed` - Whether pixel values are signed (for J2K encoding)
/// * `imode` - Target interleave mode (ignored for J2K, which always uses IMODE=B)
/// * `nppbh` - Block width in pixels
/// * `nppbv` - Block height in pixels
/// * `j2k_hints` - Optional JPEG 2000 encoding hints (required for C8/CD)
/// * `jpeg_comrat` - Optional JPEG COMRAT string for quality (for C3/M3/I1)
///
/// # Returns
/// A boxed `BlockEncoder` implementation appropriate for the compression type.
///
/// # Errors
/// Returns `CodecError::Unsupported` if the compression type is not supported.
///
/// # Supported Compression Types
/// - `NC`: Uncompressed imagery
/// - `C3`: JPEG DCT (requires `libjpeg-turbo` feature)
/// - `M3`: JPEG DCT with mask (requires `libjpeg-turbo` feature)
/// - `I1`: Downsampled JPEG (requires `libjpeg-turbo` feature)
/// - `C8`: JPEG 2000 Part 1 (requires `openjpeg` feature)
/// - `CD`: JPEG 2000 Part 15 HTJ2K (requires `openjpeg` feature)
#[allow(clippy::too_many_arguments)]
pub fn create_block_encoder(
    ic: &str,
    nrows: u32,
    ncols: u32,
    nbands: u32,
    nbpp: u8,
    is_signed: bool,
    imode: InterleaveMode,
    nppbh: u32,
    nppbv: u32,
    #[cfg(feature = "openjpeg")] j2k_hints: Option<&J2KEncodingHints>,
    #[cfg(not(feature = "openjpeg"))] _j2k_hints: Option<&()>,
    #[cfg(feature = "libjpeg-turbo")] jpeg_comrat: Option<&str>,
    #[cfg(not(feature = "libjpeg-turbo"))] _jpeg_comrat: Option<&str>,
) -> Result<Box<dyn BlockEncoder>, CodecError> {
    match ic.trim() {
        "NC" => Ok(Box::new(UncompressedBlockEncoder::new(
            nrows, ncols, nbands, nbpp, imode, nppbh, nppbv,
        ))),
        #[cfg(feature = "libjpeg-turbo")]
        "C3" | "M3" => {
            let encoder = JpegNitfBlockEncoder::new(
                ic.trim(),
                nrows,
                ncols,
                nbands,
                nbpp,
                imode,
                nppbh,
                nppbv,
                jpeg_comrat,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(feature = "libjpeg-turbo")]
        "I1" => {
            // I1 is a single-block downsampled JPEG with dimension constraint
            let encoder = JpegNitfBlockEncoder::new(
                "I1",
                nrows,
                ncols,
                nbands,
                nbpp,
                imode,
                nppbh,
                nppbv,
                jpeg_comrat,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(not(feature = "libjpeg-turbo"))]
        "C3" | "M3" | "I1" => Err(CodecError::Unsupported(format!(
            "JPEG DCT compression (IC='{}') requires the 'libjpeg-turbo' feature to be enabled.",
            ic.trim()
        ))),
        #[cfg(feature = "openjpeg")]
        "C8" => {
            let hints = j2k_hints.cloned().unwrap_or_default();
            let encoder = Jpeg2000BlockEncoder::new(
                nrows, ncols, nbands, nbpp, is_signed, nppbh, nppbv, &hints,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(feature = "openjpeg")]
        "CD" => {
            let hints = j2k_hints
                .cloned()
                .map(|mut h| {
                    h.htj2k = true;
                    h
                })
                .unwrap_or_else(|| J2KEncodingHints::htj2k(false));
            let encoder = Jpeg2000BlockEncoder::new(
                nrows, ncols, nbands, nbpp, is_signed, nppbh, nppbv, &hints,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(not(feature = "openjpeg"))]
        "C8" | "CD" => Err(CodecError::Unsupported(format!(
            "JPEG 2000 compression (IC='{}') requires the 'openjpeg' feature to be enabled.",
            ic.trim()
        ))),
        _ => Err(CodecError::Unsupported(format!(
            "Unsupported compression type for encoding: '{}'. Supported: NC, C3, M3, I1, C8, CD.",
            ic
        ))),
    }
}
pub use crate::assembly::TileAssembler;

#[cfg(test)]
mod tests {
    use super::*;

    mod block_encoder_trait {
        use super::*;

        /// Test that BlockEncoder trait is object-safe by creating a trait object
        #[test]
        fn trait_is_object_safe() {
            // This test verifies that BlockEncoder can be used as a trait object
            // If this compiles, the trait is object-safe
            fn _accepts_trait_object(_encoder: Box<dyn BlockEncoder>) {}
        }

        /// Test that BlockEncoder requires Send + Sync bounds
        #[test]
        fn trait_requires_send_sync() {
            // This test verifies that BlockEncoder implementations must be Send + Sync
            // If this compiles, the bounds are correctly specified
            fn _assert_send_sync<T: Send + Sync>() {}
            fn _check_bounds<T: BlockEncoder>() {
                _assert_send_sync::<T>();
            }
        }
    }

    mod create_block_encoder_tests {
        use super::*;

        #[test]
        #[cfg(feature = "openjpeg")]
        fn c8_returns_encoder() {
            let result = create_block_encoder(
                "C8",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "C8");
        }

        #[test]
        #[cfg(not(feature = "openjpeg"))]
        fn c8_without_feature_returns_error() {
            let result = create_block_encoder(
                "C8",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("openjpeg"));
                }
                _ => panic!("Expected Unsupported error"),
            }
        }

        #[test]
        fn nc_returns_encoder() {
            #[cfg(feature = "openjpeg")]
            let result = create_block_encoder(
                "NC",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            #[cfg(not(feature = "openjpeg"))]
            let result = create_block_encoder(
                "NC",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "NC");
            assert_eq!(encoder.block_grid_size(), (2, 2));
            assert_eq!(encoder.block_dimensions(), (32, 32));
        }

        #[test]
        #[cfg(feature = "openjpeg")]
        fn cd_returns_encoder() {
            let result = create_block_encoder(
                "CD",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            // CD (HTJ2K) may fail if codec doesn't support it
            // OpenJPEG doesn't support HTJ2K, so this should fail
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("HTJ2K"));
                }
                _ => panic!("Expected Unsupported error for HTJ2K"),
            }
        }

        #[test]
        fn unsupported_ic_returns_error() {
            #[cfg(feature = "openjpeg")]
            let result = create_block_encoder(
                "XX",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            #[cfg(not(feature = "openjpeg"))]
            let result = create_block_encoder(
                "XX",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("XX"));
                }
                _ => panic!("Expected Unsupported error"),
            }
        }

        #[test]
        fn nc_with_whitespace_is_trimmed() {
            #[cfg(feature = "openjpeg")]
            let result = create_block_encoder(
                "  NC  ",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            #[cfg(not(feature = "openjpeg"))]
            let result = create_block_encoder(
                "  NC  ",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_ok());
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn c3_returns_encoder() {
            let result = create_block_encoder(
                "C3",
                64,
                64,
                1,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                Some("75.0"),
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "C3");
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn m3_returns_encoder() {
            let result = create_block_encoder(
                "M3",
                64,
                64,
                1,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None, // Default quality
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "M3");
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn i1_returns_encoder() {
            let result = create_block_encoder(
                "I1",
                1024,
                1024,
                1,
                8,
                false,
                InterleaveMode::B,
                1024,
                1024,
                None,
                None,
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "I1");
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn i1_dimension_constraint_error() {
            // I1 requires dimensions ≤2048×2048
            let result = create_block_encoder(
                "I1",
                4096,
                4096,
                1,
                8,
                false,
                InterleaveMode::B,
                4096,
                4096,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidFormat(msg)) => {
                    assert!(msg.contains("2048"));
                }
                _ => panic!("Expected InvalidFormat error for I1 dimension constraint"),
            }
        }

        #[test]
        #[cfg(not(feature = "libjpeg-turbo"))]
        fn c3_without_feature_returns_error() {
            let result = create_block_encoder(
                "C3",
                64,
                64,
                1,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("libjpeg-turbo"));
                }
                _ => panic!("Expected Unsupported error"),
            }
        }
    }
}

// Property-based tests for block encoder
#[cfg(test)]
mod property_tests {
    use super::*;

    use crate::jbp::image::decoder::BlockDecoder;
    use crate::traits::ImageAssetProvider;
    use proptest::prelude::*;
    use std::sync::Arc;

    /// Generate a valid InterleaveMode
    fn interleave_mode_strategy() -> impl Strategy<Value = InterleaveMode> {
        prop_oneof![
            Just(InterleaveMode::B),
            Just(InterleaveMode::P),
            Just(InterleaveMode::R),
            Just(InterleaveMode::S),
        ]
    }

    /// Generate valid image dimensions for testing (small for speed)
    fn image_dimensions_strategy() -> impl Strategy<Value = (u32, u32, u32, u8)> {
        (
            1u32..=32,                                                           // nrows
            1u32..=32,                                                           // ncols
            1u32..=4,                                                            // nbands
            prop_oneof![Just(1u8), Just(2u8), Just(4u8), Just(8u8), Just(16u8)], // nbpp
        )
    }

    // Property 3: IMODE Conversion Preserves Pixels
    // For any valid image data and target IMODE, encoding with BlockEncoder
    // and then decoding with BlockDecoder SHALL produce byte-identical pixel data.
    // **Validates: Requirements 2.2, 2.4, 6.4, 7.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn imode_conversion_preserves_pixels(
            (nrows, ncols, nbands, nbpp) in image_dimensions_strategy(),
            imode in interleave_mode_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property 3: IMODE Conversion Preserves Pixels
            // Sub-byte IMODE P/R don't apply (NITF sub-byte is always BSQ/B-interleaved)
            if nbpp < 8 && matches!(imode, InterleaveMode::P | InterleaveMode::R) {
                return Ok(());
            }

            let bpp = (nbpp as usize).div_ceil(8);
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let data_size = total_pixels * bpp;

            // Generate image data in BSQ format with values within the valid range
            let max_val = if nbpp < 8 { (1u16 << nbpp) - 1 } else { 255 };
            let original_data: Vec<u8> = (0..data_size)
                .map(|i| (i as u16 % (max_val + 1)) as u8)
                .collect();

            // Use single block for simplicity (block size = image size)
            let mut encoder = UncompressedBlockEncoder::new(
                nrows, ncols, nbands, nbpp, imode, ncols, nrows
            );

            // Encode the single block - shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &original_data, [nbands, nrows, ncols]).unwrap();

            // Finalize to get encoded data
            let encoded_data = Box::new(encoder).finalize().unwrap();

            // Now decode using the decoder
            // We need to create a mock subheader facade or use the decoder directly
            // For this test, we'll verify the round-trip by creating a decoder
            // with matching parameters

            // Create decoder with same parameters (use real decoder)
            let decoder = crate::jbp::image::nc_decoder::UncompressedBlockDecoder::from_raw_params(
                crate::owned_buffer::OwnedBuffer::from_vec(encoded_data),
                nrows, ncols, 1, 1, ncols, nrows, nbands, nbpp, nbpp,
                crate::jbp::image::types::PixelValueType::UnsignedInt,
                crate::jbp::image::types::PixelJustification::Right,
                imode,
                "NC".to_string(),
            );

            // Decode the block
            let (decoded_data, shape) = decoder.decode_block(0, 0, 0, None).unwrap();

            // Verify shape matches - shape is [bands, rows, cols] (CHW format)
            prop_assert_eq!(shape, [nbands, nrows, ncols]);

            // Verify pixel data matches original
            prop_assert_eq!(
                decoded_data, original_data,
                "IMODE {:?} conversion should preserve pixels for {}x{}x{} image with {} bpp",
                imode, nrows, ncols, nbands, nbpp
            );
        }
    }

    /// Mock ImageAssetProvider for testing TileAssembler
    /// Returns data in BSQ format (same as JBP reader)
    struct MockBsqImageProvider {
        /// Full image data in BSQ format
        image_data: Vec<u8>,
        /// Image dimensions
        nrows: u32,
        ncols: u32,
        nbands: u32,
        /// Block dimensions
        block_width: u32,
        block_height: u32,
        /// Bytes per pixel
        bytes_per_pixel: usize,
    }

    impl MockBsqImageProvider {
        fn new(
            image_data: Vec<u8>,
            nrows: u32,
            ncols: u32,
            nbands: u32,
            block_width: u32,
            block_height: u32,
            bytes_per_pixel: usize,
        ) -> Self {
            Self {
                image_data,
                nrows,
                ncols,
                nbands,
                block_width,
                block_height,
                bytes_per_pixel,
            }
        }

        fn block_grid_size(&self) -> (u32, u32) {
            let rows = self.nrows.div_ceil(self.block_height);
            let cols = self.ncols.div_ceil(self.block_width);
            (rows, cols)
        }
    }

    impl crate::traits::AssetMetadata for MockBsqImageProvider {
        fn key(&self) -> &str {
            "mock"
        }
        fn title(&self) -> &str {
            "Mock Image"
        }
        fn description(&self) -> &str {
            "Mock image for testing"
        }
        fn media_type(&self) -> &str {
            "application/octet-stream"
        }
        fn roles(&self) -> &[String] {
            &[]
        }
        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(self.image_data.clone())
        }
        fn metadata(&self) -> Arc<dyn crate::traits::MetadataProvider> {
            Arc::new(EmptyMetadataProvider)
        }
    }

    struct EmptyMetadataProvider;
    impl crate::traits::MetadataProvider for EmptyMetadataProvider {
        fn entries(
            &self,
            _prefix: Option<&str>,
        ) -> std::collections::HashMap<String, serde_json::Value> {
            std::collections::HashMap::new()
        }
        fn raw(&self) -> &[u8] {
            &[]
        }
    }

    impl ImageAssetProvider for MockBsqImageProvider {
        fn has_block(
            &self,
            block_row: u32,
            block_col: u32,
            resolution_level: u32,
        ) -> Result<bool, CodecError> {
            if resolution_level != 0 {
                return Ok(false);
            }
            let (grid_rows, grid_cols) = self.block_grid_size();
            Ok(block_row < grid_rows && block_col < grid_cols)
        }

        fn get_block(
            &self,
            block_row: u32,
            block_col: u32,
            resolution_level: u32,
            _bands: Option<&[u32]>,
        ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
            if resolution_level != 0 {
                return Err(CodecError::InvalidBlockCoordinates(
                    block_row,
                    block_col,
                    resolution_level,
                ));
            }

            let (grid_rows, grid_cols) = self.block_grid_size();
            if block_row >= grid_rows || block_col >= grid_cols {
                return Err(CodecError::InvalidBlockCoordinates(
                    block_row,
                    block_col,
                    resolution_level,
                ));
            }

            // Calculate block bounds
            let start_x = block_col * self.block_width;
            let start_y = block_row * self.block_height;
            let end_x = (start_x + self.block_width).min(self.ncols);
            let end_y = (start_y + self.block_height).min(self.nrows);
            let actual_width = end_x - start_x;
            let actual_height = end_y - start_y;

            // Extract block data in BSQ format
            let pixels_per_band_full = (self.nrows as usize) * (self.ncols as usize);
            let block_pixels = (actual_width as usize) * (actual_height as usize);
            let mut block_data =
                Vec::with_capacity(block_pixels * (self.nbands as usize) * self.bytes_per_pixel);

            for band in 0..self.nbands {
                let band_offset = (band as usize) * pixels_per_band_full * self.bytes_per_pixel;
                for row in start_y..end_y {
                    let row_offset =
                        band_offset + (row as usize) * (self.ncols as usize) * self.bytes_per_pixel;
                    let start_offset = row_offset + (start_x as usize) * self.bytes_per_pixel;
                    let end_offset = start_offset + (actual_width as usize) * self.bytes_per_pixel;
                    block_data.extend_from_slice(&self.image_data[start_offset..end_offset]);
                }
            }

            Ok((block_data, [self.nbands, actual_height, actual_width]))
        }

        fn num_resolution_levels(&self) -> u32 {
            1
        }
        fn num_bands(&self) -> u32 {
            self.nbands
        }
        fn num_rows(&self) -> u32 {
            self.nrows
        }
        fn num_columns(&self) -> u32 {
            self.ncols
        }
        fn num_pixels_per_block_horizontal(&self) -> u32 {
            self.block_width
        }
        fn num_pixels_per_block_vertical(&self) -> u32 {
            self.block_height
        }
        fn num_bits_per_pixel(&self) -> u32 {
            (self.bytes_per_pixel * 8) as u32
        }
        fn actual_bits_per_pixel(&self) -> u32 {
            (self.bytes_per_pixel * 8) as u32
        }
        fn pixel_value_type(&self) -> crate::types::PixelType {
            crate::types::PixelType::UInt8
        }
        fn pad_pixel_value(&self) -> f64 {
            0.0
        }
    }

    /// Strategy for generating image dimensions that work with tile sizes
    fn image_with_tiles_strategy() -> impl Strategy<Value = (u32, u32, u32, u32, u32, u32, u32)> {
        (
            4u32..=32, // image width
            4u32..=32, // image height
            1u32..=3,  // nbands
            2u32..=8,  // source tile width
            2u32..=8,  // source tile height
            2u32..=8,  // output tile width
            2u32..=8,  // output tile height
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 2: Tile Size Conversion Preserves Pixels
        /// For any combination of input tile size and output tile size, and any image
        /// dimensions, the pixel values in the assembled output SHALL exactly match
        /// the pixel values from the source.
        /// **Validates: Requirements 5.1, 5.3, 5.4, 5.5, 7.2**
        #[test]
        fn tile_size_conversion_preserves_pixels(
            (image_width, image_height, nbands, src_tile_w, src_tile_h, out_tile_w, out_tile_h)
                in image_with_tiles_strategy()
        ) {
            // Feature: block-encoder-refactor, Property 2: Tile Size Conversion Preserves Pixels
            let bytes_per_pixel = 1usize;
            let total_pixels = (image_width as usize) * (image_height as usize) * (nbands as usize);

            // Generate deterministic image data in BSQ format
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Create mock provider with source tile size
            let provider = MockBsqImageProvider::new(
                original_data.clone(),
                image_height,
                image_width,
                nbands,
                src_tile_w,
                src_tile_h,
                bytes_per_pixel,
            );

            // Create TileAssembler with different output tile size
            let assembler = TileAssembler::new(&provider, out_tile_w, out_tile_h);
            let (grid_rows, grid_cols) = assembler.output_grid_size();

            // Reassemble the full image from output tiles
            let mut reassembled = vec![0u8; total_pixels];
            let pixels_per_band = (image_width as usize) * (image_height as usize);

            for out_row in 0..grid_rows {
                for out_col in 0..grid_cols {
                    let (tile_data, shape) = assembler.get_output_tile(out_row, out_col).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    // Calculate where this tile goes in the full image
                    let start_x = out_col * out_tile_w;
                    let start_y = out_row * out_tile_h;

                    // Copy tile data back to reassembled image (BSQ format)
                    let tile_pixels = (tile_width as usize) * (tile_height as usize);
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels * bytes_per_pixel;
                        let dst_band_offset = (band as usize) * pixels_per_band * bytes_per_pixel;

                        for row in 0..tile_height {
                            let src_row_offset = src_band_offset + (row as usize) * (tile_width as usize) * bytes_per_pixel;
                            let dst_row_offset = dst_band_offset
                                + ((start_y + row) as usize) * (image_width as usize) * bytes_per_pixel
                                + (start_x as usize) * bytes_per_pixel;
                            let row_bytes = (tile_width as usize) * bytes_per_pixel;

                            reassembled[dst_row_offset..dst_row_offset + row_bytes]
                                .copy_from_slice(&tile_data[src_row_offset..src_row_offset + row_bytes]);
                        }
                    }
                }
            }

            // Verify all pixels match
            prop_assert_eq!(
                reassembled, original_data,
                "Tile assembly should preserve all pixels for {}x{} image with {} bands, \
                 source tiles {}x{}, output tiles {}x{}",
                image_width, image_height, nbands, src_tile_w, src_tile_h, out_tile_w, out_tile_h
            );
        }
    }
}

// Property-based tests for round-trip consistency (Task 9).
// These tests validate that encoding with JBPDatasetWriter and decoding with
// JBPDatasetReader produces pixel-perfect results.
#[cfg(test)]
mod round_trip_property_tests {
    use super::*;
    use crate::buffered::{
        BufferedImageAssetProvider, BufferedMetadataProvider, MemoryImageConfig,
    };
    use crate::jbp::reader::JBPDatasetReader;
    use crate::jbp::types::NitfFormat;
    use crate::jbp::writer::JBPDatasetWriter;
    use crate::owned_buffer::OwnedBuffer;
    use crate::traits::{AssetProvider, DatasetReader, DatasetWriter};
    use crate::types::AssetType;
    use proptest::prelude::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Strategy for generating valid image dimensions for round-trip tests.
    /// Uses small dimensions for fast test execution.
    fn round_trip_dimensions_strategy() -> impl Strategy<Value = (u32, u32, u32)> {
        (
            4u32..=32, // nrows
            4u32..=32, // ncols
            1u32..=4,  // nbands
        )
    }

    /// Strategy for generating block sizes that are valid for given dimensions.
    fn block_size_strategy() -> impl Strategy<Value = (u32, u32)> {
        (
            2u32..=16, // block_width
            2u32..=16, // block_height
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 1: Round-Trip Consistency
        /// For any valid image data (any dimensions, any bit depth, any band count),
        /// encoding with JBPDatasetWriter and then decoding with JBPDatasetReader
        /// SHALL produce byte-identical pixel data with matching dimensions.
        /// **Validates: Requirements 7.1, 7.4**
        #[test]
        fn round_trip_consistency(
            (nrows, ncols, nbands) in round_trip_dimensions_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property 1: Round-Trip Consistency
            let dir = tempdir().unwrap();
            let path = dir.path().join("round_trip_test.ntf");

            // Create test image configuration
            let config = MemoryImageConfig::new(ncols, nrows)
                .with_bands(nbands)
                .with_block_size(ncols, nrows); // Single block for simplicity

            let provider = BufferedImageAssetProvider::new("test_image", config);

            // Generate deterministic test data in BSQ format
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Set the full image data
            provider.set_full_image(&original_data).unwrap();

            // Write the NITF file
            let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
            writer.add_asset(
                "test_image",
                AssetProvider::Image(Arc::new(provider)),
                "Test Image",
                "Round-trip test",
                &[]
            ).unwrap();
            writer.close().unwrap();

            // Read the file back
            let file_data = std::fs::read(&path).unwrap();
            let reader = JBPDatasetReader::from_buffer(OwnedBuffer::from_vec(file_data)).unwrap();

            // Verify we have one image asset
            let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
            prop_assert_eq!(asset_keys.len(), 1, "Should have exactly one image asset");

            // Get the image asset
            let asset = reader.get_asset(&asset_keys[0]).unwrap();

            // Get the ImageAssetProvider via the enum's typed accessor
            let image_provider = asset.as_image()
                .expect("Asset should be an image provider");

            // Verify dimensions
            prop_assert_eq!(image_provider.num_columns(), ncols, "Width mismatch");
            prop_assert_eq!(image_provider.num_rows(), nrows, "Height mismatch");
            prop_assert_eq!(image_provider.num_bands(), nbands, "Band count mismatch");

            // Read back all blocks and reassemble the image
            let block_width = image_provider.num_pixels_per_block_horizontal();
            let block_height = image_provider.num_pixels_per_block_vertical();
            let grid_cols = ncols.div_ceil(block_width);
            let grid_rows = nrows.div_ceil(block_height);

            // Reassemble the full image from blocks
            let pixels_per_band = (nrows as usize) * (ncols as usize);
            let mut reassembled = vec![0u8; total_pixels];

            for block_row in 0..grid_rows {
                for block_col in 0..grid_cols {
                    let (block_data, shape) = image_provider.get_block(block_row, block_col, 0, None).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    // Calculate where this block goes in the full image
                    let start_x = block_col * block_width;
                    let start_y = block_row * block_height;

                    // Block data is in BSQ format (band-sequential)
                    let tile_pixels_per_band = (tile_width as usize) * (tile_height as usize);

                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels_per_band;
                        let dst_band_offset = (band as usize) * pixels_per_band;
                        for row in 0..tile_height {
                            for col in 0..tile_width {
                                let src_offset = src_band_offset + (row as usize) * (tile_width as usize) + (col as usize);

                                let dst_row = start_y + row;
                                let dst_col = start_x + col;
                                let dst_offset = dst_band_offset + (dst_row as usize) * (ncols as usize) + (dst_col as usize);

                                if src_offset < block_data.len() && dst_offset < reassembled.len() {
                                    reassembled[dst_offset] = block_data[src_offset];
                                }
                            }
                        }
                    }
                }
            }

            // Verify pixel data matches original
            prop_assert_eq!(
                reassembled, original_data,
                "Round-trip should preserve all pixels for {}x{}x{} image",
                ncols, nrows, nbands
            );
        }

        /// Property 5: Block Grid Calculation
        /// For any image dimensions (NROWS, NCOLS) and block dimensions (NPPBH, NPPBV),
        /// the block grid size returned by block_grid_size() SHALL equal
        /// (ceil(NROWS/NPPBV), ceil(NCOLS/NPPBH)).
        /// **Validates: Requirements 1.5**
        #[test]
        fn block_grid_calculation(
            (nrows, ncols, nbands) in round_trip_dimensions_strategy(),
            (block_width, block_height) in block_size_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property 5: Block Grid Calculation
            let encoder = UncompressedBlockEncoder::new(
                nrows, ncols, nbands, 8, InterleaveMode::B, block_width, block_height
            );

            let (grid_rows, grid_cols) = encoder.block_grid_size();

            // Expected values using ceiling division
            let expected_rows = nrows.div_ceil(block_height);
            let expected_cols = ncols.div_ceil(block_width);

            prop_assert_eq!(
                grid_rows, expected_rows,
                "Grid rows should be ceil({}/{}) = {}, got {}",
                nrows, block_height, expected_rows, grid_rows
            );
            prop_assert_eq!(
                grid_cols, expected_cols,
                "Grid cols should be ceil({}/{}) = {}, got {}",
                ncols, block_width, expected_cols, grid_cols
            );
        }

        /// Property 4: Edge Block Handling
        /// For any image dimensions that are not evenly divisible by the block size,
        /// edge blocks (partial blocks at right and bottom boundaries) SHALL be
        /// encoded correctly with the actual pixel count, not padded dimensions.
        /// **Validates: Requirements 2.5**
        #[test]
        fn edge_block_handling(
            nrows in 5u32..=32,
            ncols in 5u32..=32,
            nbands in 1u32..=3,
            block_width in 3u32..=8,
            block_height in 3u32..=8,
        ) {
            // Feature: block-encoder-refactor, Property 4: Edge Block Handling
            // Skip cases where dimensions are evenly divisible (no edge blocks)
            prop_assume!(nrows % block_height != 0 || ncols % block_width != 0);

            let dir = tempdir().unwrap();
            let path = dir.path().join("edge_block_test.ntf");

            // Create test image configuration with non-divisible dimensions
            let config = MemoryImageConfig::new(ncols, nrows)
                .with_bands(nbands)
                .with_block_size(block_width, block_height);

            let provider = BufferedImageAssetProvider::new("test_image", config);

            // Generate deterministic test data in BSQ format
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Set the full image data
            provider.set_full_image(&original_data).unwrap();

            // Write the NITF file
            let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
            writer.add_asset(
                "test_image",
                AssetProvider::Image(Arc::new(provider)),
                "Test Image",
                "Edge block test",
                &[]
            ).unwrap();
            writer.close().unwrap();

            // Read the file back
            let file_data = std::fs::read(&path).unwrap();
            let reader = JBPDatasetReader::from_buffer(OwnedBuffer::from_vec(file_data)).unwrap();
            let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
            let asset = reader.get_asset(&asset_keys[0]).unwrap();

            let image_provider = asset.as_image()
                .expect("Asset should be an image provider");

            // Calculate expected grid size
            let expected_grid_cols = ncols.div_ceil(block_width);
            let expected_grid_rows = nrows.div_ceil(block_height);

            // Verify edge blocks have correct dimensions
            // Check right edge block (last column)
            // Shape is [bands, rows, cols] (CHW format)
            if ncols % block_width != 0 {
                let edge_col = expected_grid_cols - 1;
                let expected_edge_width = ncols - (edge_col * block_width);

                let (_block_data, shape) = image_provider.get_block(0, edge_col, 0, None).unwrap();
                prop_assert_eq!(
                    shape[2], expected_edge_width,
                    "Right edge block width should be {}, got {}",
                    expected_edge_width, shape[2]
                );
            }

            // Check bottom edge block (last row)
            if nrows % block_height != 0 {
                let edge_row = expected_grid_rows - 1;
                let expected_edge_height = nrows - (edge_row * block_height);

                let (_block_data, shape) = image_provider.get_block(edge_row, 0, 0, None).unwrap();
                prop_assert_eq!(
                    shape[1], expected_edge_height,
                    "Bottom edge block height should be {}, got {}",
                    expected_edge_height, shape[1]
                );
            }

            // Reassemble and verify all pixels
            let pixels_per_band = (nrows as usize) * (ncols as usize);
            let mut reassembled = vec![0u8; total_pixels];

            for block_row in 0..expected_grid_rows {
                for block_col in 0..expected_grid_cols {
                    let (block_data, shape) = image_provider.get_block(block_row, block_col, 0, None).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    let start_x = block_col * block_width;
                    let start_y = block_row * block_height;

                    // Block data is in BSQ format (band-sequential)
                    let tile_pixels_per_band = (tile_height as usize) * (tile_width as usize);
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels_per_band;
                        let dst_band_offset = (band as usize) * pixels_per_band;
                        for row in 0..tile_height {
                            for col in 0..tile_width {
                                let src_offset = src_band_offset + (row as usize) * (tile_width as usize) + (col as usize);

                                let dst_row = start_y + row;
                                let dst_col = start_x + col;
                                let dst_offset = dst_band_offset + (dst_row as usize) * (ncols as usize) + (dst_col as usize);

                                if src_offset < block_data.len() && dst_offset < reassembled.len() {
                                    reassembled[dst_offset] = block_data[src_offset];
                                }
                            }
                        }
                    }
                }
            }

            prop_assert_eq!(
                reassembled, original_data,
                "Edge block handling should preserve all pixels for {}x{}x{} image with {}x{} blocks",
                ncols, nrows, nbands, block_width, block_height
            );
        }

        /// Property: Tile Size Round-Trip
        /// For images with various source tile sizes, writing with different output
        /// tile sizes and reading back SHALL preserve pixel values exactly.
        /// **Validates: Requirements 5.5, 7.2**
        #[test]
        fn tile_size_round_trip(
            (nrows, ncols, nbands) in round_trip_dimensions_strategy(),
            (src_block_w, src_block_h) in block_size_strategy(),
            (out_block_w, out_block_h) in block_size_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property: Tile Size Round-Trip
            let dir = tempdir().unwrap();
            let path = dir.path().join("tile_size_round_trip_test.ntf");

            // Create test image with source tile size
            let config = MemoryImageConfig::new(ncols, nrows)
                .with_bands(nbands)
                .with_block_size(src_block_w, src_block_h);

            let provider = BufferedImageAssetProvider::new("test_image", config);

            // Generate deterministic test data in BSQ format
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Set the full image data
            provider.set_full_image(&original_data).unwrap();

            // Create metadata with output block size hints
            let metadata = BufferedMetadataProvider::new();
            metadata.set("nppbh", serde_json::json!(out_block_w.to_string()));
            metadata.set("nppbv", serde_json::json!(out_block_h.to_string()));

            let provider_with_meta = BufferedImageAssetProvider::new("test_image",
                MemoryImageConfig::new(ncols, nrows)
                    .with_bands(nbands)
                    .with_block_size(src_block_w, src_block_h)
            ).with_metadata(Arc::new(metadata));
            provider_with_meta.set_full_image(&original_data).unwrap();

            // Write the NITF file
            let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
            writer.add_asset(
                "test_image",
                AssetProvider::Image(Arc::new(provider_with_meta)),
                "Test Image",
                "Tile size round-trip test",
                &[]
            ).unwrap();
            writer.close().unwrap();

            // Read the file back
            let file_data = std::fs::read(&path).unwrap();
            let reader = JBPDatasetReader::from_buffer(OwnedBuffer::from_vec(file_data)).unwrap();
            let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
            let asset = reader.get_asset(&asset_keys[0]).unwrap();

            let image_provider = asset.as_image()
                .expect("Asset should be an image provider");

            // Verify dimensions match
            prop_assert_eq!(image_provider.num_columns(), ncols);
            prop_assert_eq!(image_provider.num_rows(), nrows);
            prop_assert_eq!(image_provider.num_bands(), nbands);

            // Reassemble the full image from blocks
            let read_block_width = image_provider.num_pixels_per_block_horizontal();
            let read_block_height = image_provider.num_pixels_per_block_vertical();
            let grid_cols = ncols.div_ceil(read_block_width);
            let grid_rows = nrows.div_ceil(read_block_height);

            let pixels_per_band = (nrows as usize) * (ncols as usize);
            let mut reassembled = vec![0u8; total_pixels];

            for block_row in 0..grid_rows {
                for block_col in 0..grid_cols {
                    let (block_data, shape) = image_provider.get_block(block_row, block_col, 0, None).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    let start_x = block_col * read_block_width;
                    let start_y = block_row * read_block_height;

                    // Block data is in BSQ format
                    let tile_pixels_per_band = (tile_height as usize) * (tile_width as usize);
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels_per_band;
                        let dst_band_offset = (band as usize) * pixels_per_band;
                        for row in 0..tile_height {
                            for col in 0..tile_width {
                                let src_offset = src_band_offset + (row as usize) * (tile_width as usize) + (col as usize);

                                let dst_row = start_y + row;
                                let dst_col = start_x + col;
                                let dst_offset = dst_band_offset + (dst_row as usize) * (ncols as usize) + (dst_col as usize);

                                if src_offset < block_data.len() && dst_offset < reassembled.len() {
                                    reassembled[dst_offset] = block_data[src_offset];
                                }
                            }
                        }
                    }
                }
            }

            prop_assert_eq!(
                reassembled, original_data,
                "Tile size round-trip should preserve all pixels for {}x{}x{} image, \
                 source tiles {}x{}, output tiles {}x{}",
                ncols, nrows, nbands, src_block_w, src_block_h, out_block_w, out_block_h
            );
        }
    }
}
