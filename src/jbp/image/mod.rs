//! Image segment support for JBP (NITF/NSIF) files.
//!
//! This module provides parsing, validation, and writing of image subheaders,
//! along with reading and writing uncompressed imagery with single and multi-band support.
//!
//! # Key Components
//!
//! - [`types`] - Core enums for pixel types, image representation, interleave modes
//! - [`facade`] - Facade pattern over StructureAccessor for typed field access
//! - [`builder`] - Builder pattern for constructing image subheaders
//! - [`decoder`] - Strategy pattern for block decoders (uncompressed, JPEG2000, etc.)
//! - [`encoder`] - Strategy pattern for block encoders (uncompressed, JPEG2000, etc.)
//! - [`pixel`] - Pixel value encoding/decoding for all PVTYPE values
//! - [`interleave`] - Conversion between interleave modes (B, P, R, S)
//! - [`validation`] - Image subheader validation rules
//! - [`mask`] - Image Data Mask support for masked images

pub mod builder;
pub mod decoder;
pub mod encoder;
pub mod facade;
pub mod interleave;
pub mod mask;
pub mod pixel;
pub mod types;
pub mod validation;

pub use builder::{BandInfoBuilder, ImageSubheaderBuilder};
pub use decoder::{create_block_decoder, BlockDecoder, UncompressedBlockDecoder};
pub use encoder::{create_block_encoder, BlockEncoder, TileAssembler};
pub use facade::{BandInfoFacade, ImageSubheaderFacade};
pub use interleave::{convert, from_band_sequential, to_band_sequential};
pub use mask::{ImageDataMask, EMPTY_BLOCK_OFFSET};
pub use types::{ImageRepresentation, InterleaveMode, LookUpTable, PixelJustification, PixelValueType};
pub use validation::{ImageValidationCode, ImageValidationResult, ImageValidator, ValidationSeverity};

/// Check if an IC (Image Compression) value indicates a masked image.
///
/// Masked IC values have a block mask table preceding the image data,
/// allowing sparse imagery where some blocks may be empty.
///
/// # Arguments
/// * `ic` - The IC field value from the image subheader
///
/// # Returns
/// `true` if the IC value indicates a masked image, `false` otherwise
///
/// # Examples
/// ```ignore
/// use aws_osml_io::jbp::image::is_masked_ic;
///
/// assert!(is_masked_ic("NM"));  // Uncompressed with mask
/// assert!(is_masked_ic("M8"));  // JPEG 2000 with mask
/// assert!(!is_masked_ic("NC")); // Uncompressed without mask
/// assert!(!is_masked_ic("C8")); // JPEG 2000 without mask
/// ```
pub fn is_masked_ic(ic: &str) -> bool {
    matches!(
        ic,
        "NM" | "M1" | "M3" | "M4" | "M5" | "M7" | "M8" | "M9" | "MA" | "MB" | "MC" | "MD" | "ME"
    )
}

/// Get the non-masked equivalent of a masked IC value.
///
/// Converts a masked IC value to its non-masked counterpart.
/// If the IC value is already non-masked or unknown, returns it unchanged.
///
/// # Arguments
/// * `ic` - The IC field value from the image subheader
///
/// # Returns
/// The non-masked equivalent IC value
///
/// # Examples
/// ```ignore
/// use aws_osml_io::jbp::image::unmask_ic;
///
/// assert_eq!(unmask_ic("NM"), "NC");  // Uncompressed
/// assert_eq!(unmask_ic("M8"), "C8");  // JPEG 2000
/// assert_eq!(unmask_ic("MD"), "CD");  // HTJ2K
/// assert_eq!(unmask_ic("NC"), "NC");  // Already non-masked
/// ```
pub fn unmask_ic(ic: &str) -> &str {
    match ic {
        "NM" => "NC",
        "M1" => "C1",
        "M3" => "C3",
        "M4" => "C4",
        "M5" => "C5",
        "M7" => "C7",
        "M8" => "C8",
        "M9" => "C9",
        "MA" => "CA",
        "MB" => "CB",
        "MC" => "CC",
        "MD" => "CD",
        "ME" => "CE",
        other => other,
    }
}

/// Get the masked equivalent of a non-masked IC value.
///
/// Converts a non-masked IC value to its masked counterpart.
/// If the IC value is already masked or unknown, returns it unchanged.
///
/// # Arguments
/// * `ic` - The IC field value from the image subheader
///
/// # Returns
/// The masked equivalent IC value
///
/// # Examples
/// ```ignore
/// use aws_osml_io::jbp::image::mask_ic;
///
/// assert_eq!(mask_ic("NC"), "NM");  // Uncompressed
/// assert_eq!(mask_ic("C8"), "M8");  // JPEG 2000
/// assert_eq!(mask_ic("CD"), "MD");  // HTJ2K
/// assert_eq!(mask_ic("NM"), "NM");  // Already masked
/// ```
pub fn mask_ic(ic: &str) -> &str {
    match ic {
        "NC" => "NM",
        "C1" => "M1",
        "C3" => "M3",
        "C4" => "M4",
        "C5" => "M5",
        "C7" => "M7",
        "C8" => "M8",
        "C9" => "M9",
        "CA" => "MA",
        "CB" => "MB",
        "CC" => "MC",
        "CD" => "MD",
        "CE" => "ME",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_masked_ic_masked_values() {
        // All masked IC values should return true
        let masked_values = [
            "NM", "M1", "M3", "M4", "M5", "M7", "M8", "M9", "MA", "MB", "MC", "MD", "ME",
        ];
        for ic in masked_values {
            assert!(is_masked_ic(ic), "Expected {} to be masked", ic);
        }
    }

    #[test]
    fn test_is_masked_ic_non_masked_values() {
        // All non-masked IC values should return false
        let non_masked_values = [
            "NC", "C1", "C3", "C4", "C5", "C7", "C8", "C9", "CA", "CB", "CC", "CD", "CE",
        ];
        for ic in non_masked_values {
            assert!(!is_masked_ic(ic), "Expected {} to be non-masked", ic);
        }
    }

    #[test]
    fn test_is_masked_ic_unknown_values() {
        // Unknown IC values should return false
        assert!(!is_masked_ic("XX"));
        assert!(!is_masked_ic(""));
        assert!(!is_masked_ic("INVALID"));
    }

    #[test]
    fn test_unmask_ic_masked_to_non_masked() {
        // Verify all masked -> non-masked conversions
        assert_eq!(unmask_ic("NM"), "NC");
        assert_eq!(unmask_ic("M1"), "C1");
        assert_eq!(unmask_ic("M3"), "C3");
        assert_eq!(unmask_ic("M4"), "C4");
        assert_eq!(unmask_ic("M5"), "C5");
        assert_eq!(unmask_ic("M7"), "C7");
        assert_eq!(unmask_ic("M8"), "C8");
        assert_eq!(unmask_ic("M9"), "C9");
        assert_eq!(unmask_ic("MA"), "CA");
        assert_eq!(unmask_ic("MB"), "CB");
        assert_eq!(unmask_ic("MC"), "CC");
        assert_eq!(unmask_ic("MD"), "CD");
        assert_eq!(unmask_ic("ME"), "CE");
    }

    #[test]
    fn test_unmask_ic_passthrough() {
        // Non-masked and unknown values should pass through unchanged
        assert_eq!(unmask_ic("NC"), "NC");
        assert_eq!(unmask_ic("C8"), "C8");
        assert_eq!(unmask_ic("XX"), "XX");
        assert_eq!(unmask_ic(""), "");
    }

    #[test]
    fn test_mask_ic_non_masked_to_masked() {
        // Verify all non-masked -> masked conversions
        assert_eq!(mask_ic("NC"), "NM");
        assert_eq!(mask_ic("C1"), "M1");
        assert_eq!(mask_ic("C3"), "M3");
        assert_eq!(mask_ic("C4"), "M4");
        assert_eq!(mask_ic("C5"), "M5");
        assert_eq!(mask_ic("C7"), "M7");
        assert_eq!(mask_ic("C8"), "M8");
        assert_eq!(mask_ic("C9"), "M9");
        assert_eq!(mask_ic("CA"), "MA");
        assert_eq!(mask_ic("CB"), "MB");
        assert_eq!(mask_ic("CC"), "MC");
        assert_eq!(mask_ic("CD"), "MD");
        assert_eq!(mask_ic("CE"), "ME");
    }

    #[test]
    fn test_mask_ic_passthrough() {
        // Already masked and unknown values should pass through unchanged
        assert_eq!(mask_ic("NM"), "NM");
        assert_eq!(mask_ic("M8"), "M8");
        assert_eq!(mask_ic("XX"), "XX");
        assert_eq!(mask_ic(""), "");
    }

    #[test]
    fn test_mask_unmask_roundtrip() {
        // Verify roundtrip: mask(unmask(x)) == x for masked values
        let masked_values = [
            "NM", "M1", "M3", "M4", "M5", "M7", "M8", "M9", "MA", "MB", "MC", "MD", "ME",
        ];
        for ic in masked_values {
            assert_eq!(mask_ic(unmask_ic(ic)), ic, "Roundtrip failed for {}", ic);
        }

        // Verify roundtrip: unmask(mask(x)) == x for non-masked values
        let non_masked_values = [
            "NC", "C1", "C3", "C4", "C5", "C7", "C8", "C9", "CA", "CB", "CC", "CD", "CE",
        ];
        for ic in non_masked_values {
            assert_eq!(unmask_ic(mask_ic(ic)), ic, "Roundtrip failed for {}", ic);
        }
    }
}
