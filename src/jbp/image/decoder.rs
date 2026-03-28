//! Block decoder trait and implementations for NITF image data.
//!
//! This module provides the strategy pattern for decoding image blocks from
//! various compression formats. The [`BlockDecoder`] trait defines the interface,
//! and implementations handle specific compression types.
//!
//! # Supported Compression Types
//!
//! | IC Code | Description | Implementation |
//! |---------|-------------|----------------|
//! | NC | No compression | [`nc_decoder::UncompressedBlockDecoder`](super::nc_decoder::UncompressedBlockDecoder) |
//! | NM | No compression with mask | [`nc_decoder::UncompressedBlockDecoder`](super::nc_decoder::UncompressedBlockDecoder) |
//! | C3 | JPEG DCT | [`JpegNitfBlockDecoder`](super::jpeg_decoder::JpegNitfBlockDecoder) |
//! | M3 | JPEG DCT with mask | [`JpegNitfBlockDecoder`](super::jpeg_decoder::JpegNitfBlockDecoder) |
//! | I1 | Downsampled JPEG | [`JpegNitfBlockDecoder`](super::jpeg_decoder::JpegNitfBlockDecoder) |
//! | C8 | JPEG 2000 Part 1 | [`Jpeg2000BlockDecoder`] |
//! | CD | JPEG 2000 Part 15 (HTJ2K) | [`Jpeg2000BlockDecoder`] |
//! | M8 | JPEG 2000 with mask | [`Jpeg2000BlockDecoder`] |
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::decoder::{create_block_decoder, BlockDecoder};
//! use osml_io::jbp::image::facade::ImageSubheaderFacade;
//!
//! let decoder = create_block_decoder(&facade, image_data)?;
//! let (block_data, shape) = decoder.decode_block(0, 0, 0, None)?;
//! ```

use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::image::facade::ImageSubheaderFacade;
use crate::jbp::image::nc_decoder::UncompressedBlockDecoder;

#[cfg(feature = "openjpeg")]
use crate::j2k::get_j2k_codec;
#[cfg(feature = "openjpeg")]
use crate::jbp::image::j2k_decoder::Jpeg2000BlockDecoder;

#[cfg(feature = "libjpeg-turbo")]
use crate::jbp::image::jpeg_decoder::JpegNitfBlockDecoder;

/// Convert a byte buffer from big-endian to native-endian.
///
/// NITF mandates big-endian for uncompressed multi-byte pixel data
/// (JBP Section 4.6.2, requirement JBP-2021.2-013). This function converts
/// the raw on-disk bytes to native-endian so the internal `Vec<u8>` contract
/// is native throughout, consistent with 3rd-party codec output.
///
/// For single-byte data (`bytes_per_pixel == 1`) this is a no-op.
#[inline]
pub fn swap_be_to_ne(data: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
    if cfg!(target_endian = "big") || bytes_per_pixel <= 1 {
        return data.to_vec();
    }
    match bytes_per_pixel {
        2 => data
            .chunks_exact(2)
            .flat_map(|c| u16::from_be_bytes([c[0], c[1]]).to_ne_bytes())
            .collect(),
        4 => data
            .chunks_exact(4)
            .flat_map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]).to_ne_bytes())
            .collect(),
        8 => data
            .chunks_exact(8)
            .flat_map(|c| {
                u64::from_be_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
                    .to_ne_bytes()
            })
            .collect(),
        _ => data.to_vec(),
    }
}

/// Trait for decoding image blocks from various compression formats.
///
/// This trait defines the interface for block-based image decoding. Different
/// compression formats implement this trait, allowing the image asset provider
/// to delegate to the appropriate decoder based on the IC field.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent
/// block access from multiple threads.
pub trait BlockDecoder: Send + Sync {
    /// Decode a single block of image data.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    /// * `resolution_level` - Resolution level to decode (0 = full resolution, N = 1/2^N)
    /// * `bands` - Optional slice of band indices to retrieve. If `None`, all bands are returned.
    ///
    /// # Returns
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` describing the block dimensions at the requested resolution (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if the block coordinates or resolution level
    /// are out of bounds.
    fn decode_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError>;

    /// Check if a block exists at the given coordinates.
    ///
    /// For uncompressed images, this checks if the coordinates are within
    /// the block grid. For masked images, this also checks the block mask.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    ///
    /// # Returns
    /// `true` if the block exists and contains data, `false` otherwise.
    fn has_block(&self, block_row: u32, block_col: u32) -> bool;

    /// Get the compression type identifier.
    ///
    /// # Returns
    /// The IC field value (e.g., "NC", "NM", "C8").
    fn compression_type(&self) -> &str;

    /// Get the number of resolution levels.
    ///
    /// For uncompressed images, this is always 1.
    /// For JPEG 2000, this depends on the number of decomposition levels.
    ///
    /// # Returns
    /// The number of resolution levels (minimum 1).
    fn num_resolution_levels(&self) -> u32;

    /// Decode a block at a specific byte offset.
    ///
    /// This method is used for masked images where block offsets come from
    /// the Image Data Mask table rather than being calculated from block
    /// coordinates. The offset is relative to the start of the image data
    /// (after the mask table).
    ///
    /// # Arguments
    /// * `offset` - Byte offset from the start of image data to the block
    /// * `block_row` - Row index of the block (for dimension calculation)
    /// * `block_col` - Column index of the block (for dimension calculation)
    /// * `resolution_level` - Resolution level to decode (0 = full resolution)
    /// * `bands` - Optional slice of band indices to retrieve
    ///
    /// # Returns
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` describing the block dimensions (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if the offset is invalid or decoding fails.
    ///
    /// # Requirements
    /// - 2.4: Masked block decoding using offsets from mask table
    fn decode_block_at_offset(
        &self,
        offset: u64,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError>;

    /// Return per-tile byte ranges relative to the start of the image data buffer.
    ///
    /// Returns `None` if the decoder does not support byte range reporting.
    /// The returned offsets are relative to the start of the image data
    /// (i.e., codestream-relative for J2K, buffer-relative for NC/JPEG).
    /// The caller (JBPImageAssetProvider) translates these to file-relative
    /// offsets by adding `location.data_offset`.
    fn tile_byte_ranges(&self) -> Option<std::collections::HashMap<(u32, u32), (u64, u64)>> {
        None
    }

    /// Return codec configuration needed for independent tile decoding.
    ///
    /// Returns `None` if no additional configuration is needed.
    fn codec_configuration(&self) -> Option<std::collections::HashMap<String, Vec<u8>>> {
        None
    }
}

/// Factory function to create the appropriate block decoder based on IC field.
///
/// # Arguments
/// * `subheader` - The image subheader facade for accessing metadata
/// * `image_data` - The raw image data bytes
///
/// # Returns
/// A boxed `BlockDecoder` implementation appropriate for the compression type.
///
/// # Errors
/// Returns `CodecError::Unsupported` if the compression type is not supported.
///
/// # Supported Compression Types
/// - `NC`, `NM`: Uncompressed imagery
/// - `C3`, `M3`: JPEG DCT (requires `libjpeg-turbo` feature)
/// - `I1`: Downsampled JPEG (requires `libjpeg-turbo` feature)
/// - `C8`, `M8`: JPEG 2000 Part 1 (requires `openjpeg` feature)
/// - `CD`, `MD`: JPEG 2000 Part 15 HTJ2K (requires `openjpeg` feature)
pub fn create_block_decoder(
    subheader: &ImageSubheaderFacade,
    image_data: Arc<[u8]>,
) -> Result<Box<dyn BlockDecoder>, CodecError> {
    use crate::jbp::image::{is_masked_ic, unmask_ic};
    
    let ic = subheader.ic()?;
    let ic_trimmed = ic.trim();
    
    // For masked IC codes, use the underlying compression type for decoder selection
    let effective_ic = if is_masked_ic(ic_trimmed) {
        unmask_ic(ic_trimmed)
    } else {
        ic_trimmed
    };

    match effective_ic {
        "NC" => {
            let decoder = UncompressedBlockDecoder::new(subheader, image_data)?;
            Ok(Box::new(decoder))
        }
        #[cfg(feature = "libjpeg-turbo")]
        "C3" | "I1" => {
            let decoder = JpegNitfBlockDecoder::new(subheader, image_data)?;
            Ok(Box::new(decoder))
        }
        #[cfg(not(feature = "libjpeg-turbo"))]
        "C3" | "I1" => Err(CodecError::Unsupported(format!(
            "JPEG DCT compression (IC='{}') requires the 'libjpeg-turbo' feature to be enabled.",
            ic_trimmed
        ))),
        #[cfg(feature = "openjpeg")]
        "C8" | "CD" => {
            let codec = get_j2k_codec();
            let decoder = Jpeg2000BlockDecoder::new(subheader, image_data, codec)?;
            Ok(Box::new(decoder))
        }
        #[cfg(not(feature = "openjpeg"))]
        "C8" | "CD" => Err(CodecError::Unsupported(format!(
            "JPEG 2000 compression (IC='{}') requires the 'openjpeg' feature to be enabled.",
            ic_trimmed
        ))),
        _ => Err(CodecError::Unsupported(format!(
            "Unsupported compression type: '{}'. Supported: NC, NM, C3, M3, I1, C8, M8, CD, MD.",
            ic_trimmed
        ))),
    }
}
