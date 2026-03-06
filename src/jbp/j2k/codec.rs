//! JPEG 2000 codec trait and types.
//!
//! This module defines the `J2KCodec` trait that abstracts JPEG 2000 encoding
//! and decoding operations, allowing different backend implementations
//! (OpenJPEG, nvJPEG2000, etc.) to be used interchangeably.

use crate::error::CodecError;

// =============================================================================
// Codec Capabilities
// =============================================================================

/// Capabilities reported by a J2K codec implementation.
///
/// Used for error messages and validation before attempting operations.
#[derive(Debug, Clone)]
pub struct J2KCodecCapabilities {
    /// Maximum supported bit depth
    pub max_bit_depth: u8,
    /// Whether HTJ2K (Part 15) decoding is supported
    pub htj2k_decode: bool,
    /// Whether HTJ2K (Part 15) encoding is supported
    pub htj2k_encode: bool,
    /// Human-readable codec name (for error messages)
    pub name: &'static str,
}

// =============================================================================
// Decode Parameters and Result
// =============================================================================

/// Parameters for JPEG 2000 decoding.
#[derive(Debug, Clone, Default)]
pub struct J2KDecodeParams {
    /// Target resolution level (0 = full resolution)
    pub resolution_level: u32,
    /// Optional region of interest (x, y, width, height)
    pub region: Option<(u32, u32, u32, u32)>,
}

/// Result of decoding a JPEG 2000 codestream.
#[derive(Debug, Clone)]
pub struct J2KDecodeResult {
    /// Decoded pixel data in band-sequential format
    pub data: Vec<u8>,
    /// Image width at decoded resolution
    pub width: u32,
    /// Image height at decoded resolution
    pub height: u32,
    /// Number of components (bands)
    pub num_components: u32,
    /// Bits per component
    pub bits_per_component: u8,
    /// Whether components are signed
    pub is_signed: bool,
    /// Number of available resolution levels
    pub num_resolution_levels: u32,
}

// =============================================================================
// Encode Parameters
// =============================================================================

/// Parameters for JPEG 2000 encoding.
#[derive(Debug, Clone)]
pub struct J2KEncodeParams {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Number of components (bands)
    pub num_components: u32,
    /// Bits per component
    pub bits_per_component: u8,
    /// Whether components are signed
    pub is_signed: bool,
    /// Target compression ratio (e.g., 10.0 for 10:1)
    pub compression_ratio: Option<f64>,
    /// Lossless encoding flag
    pub lossless: bool,
    /// Number of decomposition levels (resolution levels - 1)
    pub num_decomposition_levels: u8,
    /// Number of quality layers
    pub num_quality_layers: u8,
    /// Use HTJ2K (Part 15) encoding
    pub htj2k: bool,
    /// Tile width (for incremental encoding)
    pub tile_width: u32,
    /// Tile height (for incremental encoding)
    pub tile_height: u32,
}

impl Default for J2KEncodeParams {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            num_components: 1,
            bits_per_component: 8,
            is_signed: false,
            compression_ratio: Some(10.0),
            lossless: false,
            num_decomposition_levels: 5,
            num_quality_layers: 1,
            htj2k: false,
            tile_width: 1024,
            tile_height: 1024,
        }
    }
}

// =============================================================================
// Encode State Trait
// =============================================================================

/// State for incremental JPEG 2000 encoding.
///
/// This trait allows encoding images tile-by-tile without loading the entire
/// image into memory. Implementations maintain internal state for the encoding
/// process.
pub trait J2KEncodeState: Send {
    /// Encode a single tile.
    ///
    /// # Arguments
    /// * `tile_index` - Index of the tile (row-major order)
    /// * `data` - Pixel data for this tile in band-sequential format
    ///
    /// # Errors
    /// Returns `CodecError::Encode` if tile encoding fails.
    fn encode_tile(&mut self, tile_index: u32, data: &[u8]) -> Result<(), CodecError>;

    /// Finalize encoding and return the complete codestream.
    ///
    /// This method must be called after all tiles have been encoded.
    /// Consumes the encode state.
    ///
    /// # Errors
    /// Returns `CodecError::Encode` if not all tiles were encoded or
    /// finalization fails.
    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError>;
}

// =============================================================================
// J2K Codec Trait
// =============================================================================

/// Trait for JPEG 2000 codec implementations.
///
/// This trait abstracts JPEG 2000 encoding and decoding operations, allowing
/// different backend implementations (OpenJPEG, nvJPEG2000, etc.) to be used
/// without changing application code.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example
///
/// ```ignore
/// use osml_imagery_io::jbp::j2k::{J2KCodec, J2KDecodeParams, OpenJpegCodec};
///
/// let codec = OpenJpegCodec::new();
/// let params = J2KDecodeParams::default();
/// let result = codec.decode(&codestream, &params)?;
/// ```
pub trait J2KCodec: Send + Sync {
    /// Get codec capabilities.
    ///
    /// Returns information about what the codec supports, including maximum
    /// bit depth and HTJ2K support.
    fn capabilities(&self) -> J2KCodecCapabilities;

    /// Decode a JPEG 2000 codestream from a byte slice.
    ///
    /// # Arguments
    /// * `codestream` - Byte slice containing the J2K codestream (can be memory-mapped)
    /// * `params` - Decoding parameters
    ///
    /// # Returns
    /// Decoded image data and metadata.
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if decoding fails.
    fn decode(
        &self,
        codestream: &[u8],
        params: &J2KDecodeParams,
    ) -> Result<J2KDecodeResult, CodecError>;

    /// Start encoding a new JPEG 2000 codestream.
    ///
    /// # Arguments
    /// * `params` - Encoding parameters including dimensions and tile size
    ///
    /// # Returns
    /// Encoder state for subsequent tile writes.
    ///
    /// # Errors
    /// Returns `CodecError::Encode` if encoder setup fails.
    /// Returns `CodecError::Unsupported` if the codec doesn't support the
    /// requested encoding mode (e.g., HTJ2K).
    fn start_encode(&self, params: &J2KEncodeParams) -> Result<Box<dyn J2KEncodeState>, CodecError>;

    /// Get the number of resolution levels in a codestream without full decode.
    ///
    /// # Arguments
    /// * `codestream` - Byte slice containing the J2K codestream
    ///
    /// # Returns
    /// Number of available resolution levels.
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if the codestream header cannot be read.
    fn get_resolution_levels(&self, codestream: &[u8]) -> Result<u32, CodecError>;

    /// Get image dimensions from codestream header without full decode.
    ///
    /// # Arguments
    /// * `codestream` - Byte slice containing the J2K codestream
    ///
    /// # Returns
    /// Tuple of (width, height, num_components).
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if the codestream header cannot be read.
    fn get_dimensions(&self, codestream: &[u8]) -> Result<(u32, u32, u32), CodecError>;

    /// Get tile grid information from codestream header.
    ///
    /// # Arguments
    /// * `codestream` - Byte slice containing the J2K codestream
    ///
    /// # Returns
    /// Tuple of (tile_width, tile_height, num_tiles_x, num_tiles_y).
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if the codestream header cannot be read.
    fn get_tile_info(&self, codestream: &[u8]) -> Result<(u32, u32, u32, u32), CodecError>;

    /// Decode a single tile from a JPEG 2000 codestream.
    ///
    /// This is more efficient than decoding the entire image when only
    /// a specific tile is needed.
    ///
    /// # Arguments
    /// * `codestream` - Byte slice containing the J2K codestream
    /// * `tile_index` - Index of the tile to decode (row-major order)
    /// * `params` - Decoding parameters (resolution level)
    ///
    /// # Returns
    /// Decoded tile data and metadata.
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if decoding fails.
    /// Returns `CodecError::InvalidBlockCoordinates` if tile_index is out of range.
    fn decode_tile(
        &self,
        codestream: &[u8],
        tile_index: u32,
        params: &J2KDecodeParams,
    ) -> Result<J2KDecodeResult, CodecError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_params_default() {
        let params = J2KDecodeParams::default();
        assert_eq!(params.resolution_level, 0);
        assert!(params.region.is_none());
    }

    #[test]
    fn test_encode_params_default() {
        let params = J2KEncodeParams::default();
        assert_eq!(params.width, 0);
        assert_eq!(params.height, 0);
        assert_eq!(params.num_components, 1);
        assert_eq!(params.bits_per_component, 8);
        assert!(!params.is_signed);
        assert_eq!(params.compression_ratio, Some(10.0));
        assert!(!params.lossless);
        assert_eq!(params.num_decomposition_levels, 5);
        assert_eq!(params.num_quality_layers, 1);
        assert!(!params.htj2k);
        assert_eq!(params.tile_width, 1024);
        assert_eq!(params.tile_height, 1024);
    }
}
