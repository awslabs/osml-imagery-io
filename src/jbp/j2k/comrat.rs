//! COMRAT (Compression Rate) parsing and generation for JPEG 2000 images.
//!
//! The COMRAT field in NITF image subheaders specifies the compression rate
//! for JPEG 2000 compressed imagery. This module provides parsing and generation
//! of COMRAT values according to the BPJ2K01.20 profile.
//!
//! # COMRAT Formats
//!
//! The COMRAT field is 4 characters. For JPEG 2000 (IC=C8, CD), the format is:
//!
//! | Format | Example | Meaning |
//! |--------|---------|---------|
//! | Nn.n | N1.0 | Numerically lossless |
//! | Vn.n | V1.5 | Visually lossless, quality factor |
//! | nn.n | 00.5 | Target bits per pixel |
//!
//! Note: The field can also be "----" if the value is unknown.
//!
//! # Example
//!
//! ```ignore
//! use aws_osml_io::jbp::j2k::comrat::{J2KComrat, J2KEncodingHints, generate_comrat};
//!
//! // Parse a COMRAT value
//! let comrat = J2KComrat::parse("N1.0").unwrap();
//! assert_eq!(comrat, J2KComrat::NumericallyLossless);
//!
//! // Generate COMRAT from encoding hints
//! let hints = J2KEncodingHints::default();
//! let comrat_str = generate_comrat(&hints);
//! ```

use crate::error::CodecError;
use std::fmt;

// =============================================================================
// J2KComrat Enum
// =============================================================================

/// Parsed COMRAT value for JPEG 2000.
///
/// Represents the three types of compression rate specifications used in
/// NITF JPEG 2000 imagery.
#[derive(Debug, Clone, PartialEq)]
pub enum J2KComrat {
    /// Numerically lossless compression (e.g., "N1.0").
    ///
    /// The decoded image is bit-for-bit identical to the original.
    NumericallyLossless,

    /// Visually lossless compression with quality factor (e.g., "V1.5").
    ///
    /// The compression introduces minimal visual artifacts. The quality
    /// factor indicates the compression quality level.
    VisuallyLossless(f32),

    /// Target bits per pixel rate (e.g., "00.5" = 0.5 bpp).
    ///
    /// Lossy compression targeting a specific bits-per-pixel rate.
    /// Lower values mean higher compression (smaller files, more artifacts).
    TargetBpp(f32),

    /// Unknown compression rate (represented as "----").
    Unknown,
}

impl J2KComrat {
    /// Parse a COMRAT string for J2K images.
    ///
    /// # Arguments
    /// * `comrat` - The 4-character COMRAT field value
    ///
    /// # Returns
    /// The parsed COMRAT value.
    ///
    /// # Errors
    /// Returns `CodecError::InvalidFormat` if:
    /// - The COMRAT string is not 4 characters (after trimming)
    /// - The numeric portion cannot be parsed as a float
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use aws_osml_io::jbp::j2k::comrat::J2KComrat;
    ///
    /// // Numerically lossless
    /// let comrat = J2KComrat::parse("N1.0").unwrap();
    /// assert_eq!(comrat, J2KComrat::NumericallyLossless);
    ///
    /// // Visually lossless
    /// let comrat = J2KComrat::parse("V1.5").unwrap();
    /// assert!(matches!(comrat, J2KComrat::VisuallyLossless(_)));
    ///
    /// // Target bpp
    /// let comrat = J2KComrat::parse("00.5").unwrap();
    /// assert!(matches!(comrat, J2KComrat::TargetBpp(_)));
    ///
    /// // Unknown
    /// let comrat = J2KComrat::parse("----").unwrap();
    /// assert_eq!(comrat, J2KComrat::Unknown);
    /// ```
    pub fn parse(comrat: &str) -> Result<Self, CodecError> {
        let comrat = comrat.trim();

        if comrat.len() != 4 {
            return Err(CodecError::InvalidFormat(format!(
                "COMRAT must be 4 characters, got '{}' (length {})",
                comrat,
                comrat.len()
            )));
        }

        // Check for unknown value
        if comrat == "----" {
            return Ok(J2KComrat::Unknown);
        }

        if comrat.starts_with('N') {
            // Numerically lossless: "Nn.n" - we don't need to parse the value
            // as it's always effectively 1.0 (no loss)
            Ok(J2KComrat::NumericallyLossless)
        } else if comrat.starts_with('V') {
            // Visually lossless: "Vn.n"
            let value_str = &comrat[1..];
            let value: f32 = value_str.parse().map_err(|_| {
                CodecError::InvalidFormat(format!(
                    "Invalid COMRAT visually lossless value: '{}' (from '{}')",
                    value_str, comrat
                ))
            })?;
            Ok(J2KComrat::VisuallyLossless(value))
        } else {
            // Target bpp: "nn.n"
            let value: f32 = comrat.parse().map_err(|_| {
                CodecError::InvalidFormat(format!(
                    "Invalid COMRAT target bpp value: '{}'",
                    comrat
                ))
            })?;
            Ok(J2KComrat::TargetBpp(value))
        }
    }

    /// Generate a COMRAT string from this value.
    ///
    /// # Returns
    /// A 4-character COMRAT string suitable for the NITF image subheader.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use aws_osml_io::jbp::j2k::comrat::J2KComrat;
    ///
    /// assert_eq!(J2KComrat::NumericallyLossless.to_comrat_string(), "N1.0");
    /// assert_eq!(J2KComrat::VisuallyLossless(1.5).to_comrat_string(), "V1.5");
    /// assert_eq!(J2KComrat::TargetBpp(0.5).to_comrat_string(), "00.5");
    /// assert_eq!(J2KComrat::Unknown.to_comrat_string(), "----");
    /// ```
    pub fn to_comrat_string(&self) -> String {
        match self {
            J2KComrat::NumericallyLossless => "N1.0".to_string(),
            J2KComrat::VisuallyLossless(v) => {
                // Format as "Vn.n" - 3 characters after V
                // Clamp to valid range [0.0, 9.9]
                let clamped = v.clamp(0.0, 9.9);
                format!("V{:.1}", clamped)
            }
            J2KComrat::TargetBpp(bpp) => {
                // Format as "nn.n" - 4 characters total
                // Clamp to valid range [0.0, 99.9]
                let clamped = bpp.clamp(0.0, 99.9);
                format!("{:04.1}", clamped)
            }
            J2KComrat::Unknown => "----".to_string(),
        }
    }

    /// Check if this COMRAT represents lossless compression.
    ///
    /// # Returns
    /// `true` if numerically lossless, `false` otherwise.
    pub fn is_lossless(&self) -> bool {
        matches!(self, J2KComrat::NumericallyLossless)
    }

    /// Get the approximate bits per pixel for this COMRAT.
    ///
    /// For numerically lossless, returns `None` as the bpp varies by image content.
    /// For visually lossless, returns an estimated bpp based on typical ratios.
    /// For target bpp, returns the specified value.
    pub fn bits_per_pixel(&self) -> Option<f32> {
        match self {
            J2KComrat::NumericallyLossless => None,
            J2KComrat::VisuallyLossless(quality) => {
                // Rough estimate: visually lossless typically achieves 2-4 bpp
                // Higher quality factor = higher bpp
                Some(quality.clamp(0.5, 8.0))
            }
            J2KComrat::TargetBpp(bpp) => Some(*bpp),
            J2KComrat::Unknown => None,
        }
    }
}

impl fmt::Display for J2KComrat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_comrat_string())
    }
}

// =============================================================================
// J2KEncodingHints
// =============================================================================

/// Encoding hints for JPEG 2000 compression.
///
/// These hints control how image data is compressed when writing NITF files
/// with JPEG 2000 compression (IC=C8 or IC=CD).
#[derive(Debug, Clone)]
pub struct J2KEncodingHints {
    /// Target compression ratio (e.g., 10.0 for 10:1), None for lossless.
    pub compression_ratio: Option<f64>,

    /// Lossless encoding flag.
    ///
    /// When true, produces numerically lossless compression regardless of
    /// compression_ratio setting.
    pub lossless: bool,

    /// Number of decomposition levels (default 5).
    ///
    /// Determines the number of resolution levels available in the codestream.
    /// More levels = more resolution options but slightly larger file.
    pub decomposition_levels: u8,

    /// Number of quality layers (default 1).
    ///
    /// Multiple quality layers allow progressive quality refinement.
    pub quality_layers: u8,

    /// Use HTJ2K (Part 15) encoding.
    ///
    /// HTJ2K provides faster encoding/decoding at the cost of slightly
    /// reduced compression efficiency.
    pub htj2k: bool,
}

impl Default for J2KEncodingHints {
    fn default() -> Self {
        Self {
            compression_ratio: Some(10.0),
            lossless: false,
            decomposition_levels: 5,
            quality_layers: 1,
            htj2k: false,
        }
    }
}

impl J2KEncodingHints {
    /// Create hints for lossless compression.
    pub fn lossless() -> Self {
        Self {
            compression_ratio: None,
            lossless: true,
            decomposition_levels: 5,
            quality_layers: 1,
            htj2k: false,
        }
    }

    /// Create hints for lossy compression with a target ratio.
    ///
    /// # Arguments
    /// * `ratio` - Target compression ratio (e.g., 10.0 for 10:1)
    pub fn lossy(ratio: f64) -> Self {
        Self {
            compression_ratio: Some(ratio),
            lossless: false,
            decomposition_levels: 5,
            quality_layers: 1,
            htj2k: false,
        }
    }

    /// Create hints for HTJ2K encoding.
    ///
    /// # Arguments
    /// * `lossless` - Whether to use lossless compression
    pub fn htj2k(lossless: bool) -> Self {
        Self {
            compression_ratio: if lossless { None } else { Some(10.0) },
            lossless,
            decomposition_levels: 5,
            quality_layers: 1,
            htj2k: true,
        }
    }
}

// =============================================================================
// COMRAT Generation
// =============================================================================

/// Generate a COMRAT string from encoding hints.
///
/// This function converts encoding hints into the appropriate 4-character
/// COMRAT field value for the NITF image subheader.
///
/// # Arguments
/// * `hints` - The encoding hints specifying compression parameters
///
/// # Returns
/// A 4-character COMRAT string.
///
/// # Conversion Rules
///
/// - If `hints.lossless` is true: Returns "N1.0" (numerically lossless)
/// - If `hints.compression_ratio` is Some(ratio): Converts ratio to bpp
///   assuming 8-bit source (bpp = 8.0 / ratio)
/// - Otherwise: Returns "N1.0" as default (lossless)
///
/// # Examples
///
/// ```ignore
/// use aws_osml_io::jbp::j2k::comrat::{J2KEncodingHints, generate_comrat};
///
/// // Lossless encoding
/// let hints = J2KEncodingHints::lossless();
/// assert_eq!(generate_comrat(&hints), "N1.0");
///
/// // 10:1 compression (8 bits / 10 = 0.8 bpp)
/// let hints = J2KEncodingHints::lossy(10.0);
/// assert_eq!(generate_comrat(&hints), "00.8");
///
/// // 20:1 compression (8 bits / 20 = 0.4 bpp)
/// let hints = J2KEncodingHints::lossy(20.0);
/// assert_eq!(generate_comrat(&hints), "00.4");
/// ```
pub fn generate_comrat(hints: &J2KEncodingHints) -> String {
    if hints.lossless {
        "N1.0".to_string()
    } else if let Some(ratio) = hints.compression_ratio {
        // Convert compression ratio to approximate bpp
        // Assuming 8-bit source, ratio 10:1 = 0.8 bpp
        let bpp = 8.0 / ratio;
        // Clamp to valid COMRAT range and format
        let clamped = bpp.clamp(0.0, 99.9);
        format!("{:04.1}", clamped)
    } else {
        // Default to lossless if no ratio specified
        "N1.0".to_string()
    }
}

/// Generate a J2KComrat enum from encoding hints.
///
/// Similar to `generate_comrat` but returns the enum type instead of a string.
///
/// # Arguments
/// * `hints` - The encoding hints specifying compression parameters
///
/// # Returns
/// The corresponding J2KComrat value.
pub fn hints_to_comrat(hints: &J2KEncodingHints) -> J2KComrat {
    if hints.lossless {
        J2KComrat::NumericallyLossless
    } else if let Some(ratio) = hints.compression_ratio {
        let bpp = (8.0 / ratio) as f32;
        J2KComrat::TargetBpp(bpp.clamp(0.0, 99.9))
    } else {
        J2KComrat::NumericallyLossless
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // J2KComrat::parse tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_numerically_lossless() {
        let comrat = J2KComrat::parse("N1.0").unwrap();
        assert_eq!(comrat, J2KComrat::NumericallyLossless);

        // Other N values should also parse as lossless
        let comrat = J2KComrat::parse("N2.5").unwrap();
        assert_eq!(comrat, J2KComrat::NumericallyLossless);
    }

    #[test]
    fn test_parse_visually_lossless() {
        let comrat = J2KComrat::parse("V1.5").unwrap();
        assert_eq!(comrat, J2KComrat::VisuallyLossless(1.5));

        let comrat = J2KComrat::parse("V9.0").unwrap();
        assert_eq!(comrat, J2KComrat::VisuallyLossless(9.0));
    }

    #[test]
    fn test_parse_target_bpp() {
        let comrat = J2KComrat::parse("00.5").unwrap();
        assert_eq!(comrat, J2KComrat::TargetBpp(0.5));

        let comrat = J2KComrat::parse("01.0").unwrap();
        assert_eq!(comrat, J2KComrat::TargetBpp(1.0));

        let comrat = J2KComrat::parse("10.0").unwrap();
        assert_eq!(comrat, J2KComrat::TargetBpp(10.0));
    }

    #[test]
    fn test_parse_unknown() {
        let comrat = J2KComrat::parse("----").unwrap();
        assert_eq!(comrat, J2KComrat::Unknown);
    }

    #[test]
    fn test_parse_with_whitespace() {
        // Should trim whitespace
        let comrat = J2KComrat::parse(" N1.0 ").unwrap();
        assert_eq!(comrat, J2KComrat::NumericallyLossless);
    }

    #[test]
    fn test_parse_invalid_length() {
        // Too short
        let result = J2KComrat::parse("N01");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CodecError::InvalidFormat(_)));

        // Too long
        let result = J2KComrat::parse("N001.00");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_numeric() {
        // Invalid visually lossless value
        let result = J2KComrat::parse("Vxxx");
        assert!(result.is_err());

        // Invalid target bpp value
        let result = J2KComrat::parse("abcd");
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // J2KComrat::to_comrat_string tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_to_string_numerically_lossless() {
        let comrat = J2KComrat::NumericallyLossless;
        assert_eq!(comrat.to_comrat_string(), "N1.0");
    }

    #[test]
    fn test_to_string_visually_lossless() {
        let comrat = J2KComrat::VisuallyLossless(1.5);
        assert_eq!(comrat.to_comrat_string(), "V1.5");

        let comrat = J2KComrat::VisuallyLossless(9.0);
        assert_eq!(comrat.to_comrat_string(), "V9.0");
    }

    #[test]
    fn test_to_string_target_bpp() {
        let comrat = J2KComrat::TargetBpp(0.5);
        assert_eq!(comrat.to_comrat_string(), "00.5");

        let comrat = J2KComrat::TargetBpp(1.0);
        assert_eq!(comrat.to_comrat_string(), "01.0");

        let comrat = J2KComrat::TargetBpp(10.0);
        assert_eq!(comrat.to_comrat_string(), "10.0");
    }

    #[test]
    fn test_to_string_unknown() {
        let comrat = J2KComrat::Unknown;
        assert_eq!(comrat.to_comrat_string(), "----");
    }

    #[test]
    fn test_to_string_clamping() {
        // Values should be clamped to valid ranges
        let comrat = J2KComrat::TargetBpp(100.0);
        assert_eq!(comrat.to_comrat_string(), "99.9");

        let comrat = J2KComrat::TargetBpp(-1.0);
        assert_eq!(comrat.to_comrat_string(), "00.0");

        // Visually lossless clamped to [0.0, 9.9]
        let comrat = J2KComrat::VisuallyLossless(15.0);
        assert_eq!(comrat.to_comrat_string(), "V9.9");
    }

    // -------------------------------------------------------------------------
    // Round-trip tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_roundtrip_numerically_lossless() {
        let original = J2KComrat::NumericallyLossless;
        let string = original.to_comrat_string();
        let parsed = J2KComrat::parse(&string).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_visually_lossless() {
        let original = J2KComrat::VisuallyLossless(5.5);
        let string = original.to_comrat_string();
        let parsed = J2KComrat::parse(&string).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_target_bpp() {
        let original = J2KComrat::TargetBpp(2.5);
        let string = original.to_comrat_string();
        let parsed = J2KComrat::parse(&string).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_unknown() {
        let original = J2KComrat::Unknown;
        let string = original.to_comrat_string();
        let parsed = J2KComrat::parse(&string).unwrap();
        assert_eq!(original, parsed);
    }

    // -------------------------------------------------------------------------
    // J2KEncodingHints tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_encoding_hints_default() {
        let hints = J2KEncodingHints::default();
        assert_eq!(hints.compression_ratio, Some(10.0));
        assert!(!hints.lossless);
        assert_eq!(hints.decomposition_levels, 5);
        assert_eq!(hints.quality_layers, 1);
        assert!(!hints.htj2k);
    }

    #[test]
    fn test_encoding_hints_lossless() {
        let hints = J2KEncodingHints::lossless();
        assert!(hints.lossless);
        assert!(hints.compression_ratio.is_none());
    }

    #[test]
    fn test_encoding_hints_lossy() {
        let hints = J2KEncodingHints::lossy(20.0);
        assert!(!hints.lossless);
        assert_eq!(hints.compression_ratio, Some(20.0));
    }

    #[test]
    fn test_encoding_hints_htj2k() {
        let hints = J2KEncodingHints::htj2k(true);
        assert!(hints.htj2k);
        assert!(hints.lossless);

        let hints = J2KEncodingHints::htj2k(false);
        assert!(hints.htj2k);
        assert!(!hints.lossless);
    }

    // -------------------------------------------------------------------------
    // generate_comrat tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_generate_comrat_lossless() {
        let hints = J2KEncodingHints::lossless();
        assert_eq!(generate_comrat(&hints), "N1.0");
    }

    #[test]
    fn test_generate_comrat_lossy() {
        // 10:1 compression = 0.8 bpp
        let hints = J2KEncodingHints::lossy(10.0);
        assert_eq!(generate_comrat(&hints), "00.8");

        // 20:1 compression = 0.4 bpp
        let hints = J2KEncodingHints::lossy(20.0);
        assert_eq!(generate_comrat(&hints), "00.4");

        // 5:1 compression = 1.6 bpp
        let hints = J2KEncodingHints::lossy(5.0);
        assert_eq!(generate_comrat(&hints), "01.6");
    }

    #[test]
    fn test_generate_comrat_no_ratio() {
        // No ratio and not lossless should default to lossless
        let hints = J2KEncodingHints {
            compression_ratio: None,
            lossless: false,
            ..Default::default()
        };
        assert_eq!(generate_comrat(&hints), "N1.0");
    }

    // -------------------------------------------------------------------------
    // hints_to_comrat tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_hints_to_comrat_lossless() {
        let hints = J2KEncodingHints::lossless();
        assert_eq!(hints_to_comrat(&hints), J2KComrat::NumericallyLossless);
    }

    #[test]
    fn test_hints_to_comrat_lossy() {
        let hints = J2KEncodingHints::lossy(10.0);
        assert_eq!(hints_to_comrat(&hints), J2KComrat::TargetBpp(0.8));
    }

    // -------------------------------------------------------------------------
    // Edge case tests (for task 4.3)
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_boundary_bpp_values() {
        // Minimum valid bpp
        let comrat = J2KComrat::parse("00.1").unwrap();
        assert_eq!(comrat, J2KComrat::TargetBpp(0.1));

        // Maximum valid bpp
        let comrat = J2KComrat::parse("99.9").unwrap();
        assert_eq!(comrat, J2KComrat::TargetBpp(99.9));

        // Zero bpp
        let comrat = J2KComrat::parse("00.0").unwrap();
        assert_eq!(comrat, J2KComrat::TargetBpp(0.0));
    }

    #[test]
    fn test_parse_empty_string() {
        let result = J2KComrat::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_only_whitespace() {
        let result = J2KComrat::parse("    ");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_lossless() {
        assert!(J2KComrat::NumericallyLossless.is_lossless());
        assert!(!J2KComrat::VisuallyLossless(1.0).is_lossless());
        assert!(!J2KComrat::TargetBpp(0.5).is_lossless());
        assert!(!J2KComrat::Unknown.is_lossless());
    }

    #[test]
    fn test_bits_per_pixel() {
        assert!(J2KComrat::NumericallyLossless.bits_per_pixel().is_none());
        assert_eq!(J2KComrat::TargetBpp(0.5).bits_per_pixel(), Some(0.5));
        // Visually lossless returns clamped quality factor
        assert!(J2KComrat::VisuallyLossless(2.0).bits_per_pixel().is_some());
        assert!(J2KComrat::Unknown.bits_per_pixel().is_none());
    }

    #[test]
    fn test_display_trait() {
        assert_eq!(format!("{}", J2KComrat::NumericallyLossless), "N1.0");
        assert_eq!(format!("{}", J2KComrat::TargetBpp(0.5)), "00.5");
        assert_eq!(format!("{}", J2KComrat::Unknown), "----");
    }
}
