//! COMRAT parsing for JPEG DCT compression.
//!
//! The COMRAT field in NITF specifies the compression rate/quality for
//! lossy compression. For JPEG DCT, this is a quality factor.

use crate::error::CodecError;

/// JPEG COMRAT (Compression Rate) specification.
///
/// For JPEG DCT compression, COMRAT specifies a quality factor
/// in the format "nn.n" where higher values mean higher quality.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum JpegComrat {
    /// Quality factor (0-100 mapped from COMRAT 00.0-99.9)
    Quality(u8),
    /// Default quality (75)
    #[default]
    Default,
}

impl JpegComrat {
    /// Parse a COMRAT string for JPEG compression.
    ///
    /// # Arguments
    /// * `comrat` - The COMRAT field value (e.g., "75.0")
    ///
    /// # Returns
    /// The parsed COMRAT or an error if invalid.
    pub fn parse(comrat: &str) -> Result<Self, CodecError> {
        let trimmed = comrat.trim();

        // Empty or whitespace means default
        if trimmed.is_empty() {
            return Ok(Self::Default);
        }

        // Parse as floating point quality factor
        // Format: "nn.n" where nn.n is 00.0 to 99.9
        match trimmed.parse::<f32>() {
            Ok(value) if (0.0..=99.9).contains(&value) => {
                // Map 0.0-99.9 to quality 1-100
                // 00.0 -> quality 1 (lowest)
                // 99.9 -> quality 100 (highest)
                let quality = ((value / 99.9) * 99.0 + 1.0).round() as u8;
                Ok(Self::Quality(quality.clamp(1, 100)))
            }
            Ok(value) => Err(CodecError::InvalidFormat(format!(
                "COMRAT value {} out of range (0.0-99.9)",
                value
            ))),
            Err(_) => Err(CodecError::InvalidFormat(format!(
                "Invalid COMRAT format: '{}'",
                comrat
            ))),
        }
    }

    /// Convert to a COMRAT string for writing.
    pub fn to_comrat_string(&self) -> String {
        match self {
            Self::Quality(q) => {
                // Map quality 1-100 back to 00.0-99.9
                let value = ((*q as f32 - 1.0) / 99.0) * 99.9;
                format!("{:04.1}", value)
            }
            Self::Default => "    ".to_string(), // 4 spaces for default
        }
    }

    /// Get the JPEG quality value (1-100).
    pub fn quality(&self) -> u8 {
        match self {
            Self::Quality(q) => *q,
            Self::Default => 75,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Valid COMRAT values ("00.0" to "99.9") - Requirement 5.3
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_minimum_quality() {
        // "00.0" should map to quality 1 (lowest)
        let comrat = JpegComrat::parse("00.0").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
        assert_eq!(comrat.quality(), 1);
    }

    #[test]
    fn test_parse_maximum_quality() {
        // "99.9" should map to quality 100 (highest)
        let comrat = JpegComrat::parse("99.9").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
        assert_eq!(comrat.quality(), 100);
    }

    #[test]
    fn test_parse_mid_range_quality() {
        // "50.0" should map to approximately quality 50
        let comrat = JpegComrat::parse("50.0").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
        let q = comrat.quality();
        assert!(q >= 45 && q <= 55, "Expected quality ~50, got {}", q);
    }

    #[test]
    fn test_parse_quality_75() {
        // "75.0" should map to approximately quality 75
        let comrat = JpegComrat::parse("75.0").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
        let q = comrat.quality();
        assert!(q >= 70 && q <= 80, "Expected quality ~75, got {}", q);
    }

    #[test]
    fn test_parse_quality_25() {
        // "25.0" should map to approximately quality 25
        let comrat = JpegComrat::parse("25.0").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
        let q = comrat.quality();
        assert!(q >= 20 && q <= 30, "Expected quality ~25, got {}", q);
    }

    #[test]
    fn test_parse_fractional_values() {
        // Test fractional COMRAT values
        let comrat = JpegComrat::parse("33.3").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));

        let comrat = JpegComrat::parse("66.6").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));

        let comrat = JpegComrat::parse("12.5").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
    }

    #[test]
    fn test_parse_quality_monotonic() {
        // Higher COMRAT values should produce higher quality
        let low = JpegComrat::parse("10.0").unwrap().quality();
        let mid = JpegComrat::parse("50.0").unwrap().quality();
        let high = JpegComrat::parse("90.0").unwrap().quality();

        assert!(
            low < mid,
            "10.0 quality {} should be < 50.0 quality {}",
            low,
            mid
        );
        assert!(
            mid < high,
            "50.0 quality {} should be < 90.0 quality {}",
            mid,
            high
        );
    }

    // -------------------------------------------------------------------------
    // Default quality mapping - Requirement 5.4
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_empty_string_default() {
        let comrat = JpegComrat::parse("").unwrap();
        assert_eq!(comrat, JpegComrat::Default);
        assert_eq!(comrat.quality(), 75);
    }

    #[test]
    fn test_parse_whitespace_only_default() {
        let comrat = JpegComrat::parse("   ").unwrap();
        assert_eq!(comrat, JpegComrat::Default);
        assert_eq!(comrat.quality(), 75);
    }

    #[test]
    fn test_parse_tab_whitespace_default() {
        let comrat = JpegComrat::parse("\t").unwrap();
        assert_eq!(comrat, JpegComrat::Default);
        assert_eq!(comrat.quality(), 75);
    }

    #[test]
    fn test_default_trait_quality() {
        let comrat = JpegComrat::default();
        assert_eq!(comrat, JpegComrat::Default);
        assert_eq!(comrat.quality(), 75);
    }

    #[test]
    fn test_default_to_comrat_string() {
        let comrat = JpegComrat::Default;
        let s = comrat.to_comrat_string();
        assert_eq!(s, "    "); // 4 spaces
        assert_eq!(s.len(), 4);
    }

    // -------------------------------------------------------------------------
    // Edge cases and invalid inputs - Requirements 5.1, 5.2, 5.3
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_out_of_range_high() {
        // Values > 99.9 should fail
        let result = JpegComrat::parse("100.0");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodecError::InvalidFormat(_)));
    }

    #[test]
    fn test_parse_out_of_range_negative() {
        // Negative values should fail
        let result = JpegComrat::parse("-1.0");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodecError::InvalidFormat(_)));
    }

    #[test]
    fn test_parse_non_numeric() {
        // Non-numeric strings should fail
        let result = JpegComrat::parse("abc");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodecError::InvalidFormat(_)));
    }

    #[test]
    fn test_parse_mixed_alphanumeric() {
        let result = JpegComrat::parse("12ab");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_special_characters() {
        let result = JpegComrat::parse("!@#$");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_integer_without_decimal() {
        // Integer values should parse (e.g., "50" parses as 50.0)
        let comrat = JpegComrat::parse("50").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
    }

    #[test]
    fn test_parse_leading_whitespace() {
        let comrat = JpegComrat::parse("  75.0").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
    }

    #[test]
    fn test_parse_trailing_whitespace() {
        let comrat = JpegComrat::parse("75.0  ").unwrap();
        assert!(matches!(comrat, JpegComrat::Quality(_)));
    }

    #[test]
    fn test_parse_boundary_99_9() {
        // Exactly 99.9 should work
        let comrat = JpegComrat::parse("99.9").unwrap();
        assert_eq!(comrat.quality(), 100);
    }

    #[test]
    fn test_parse_boundary_0_0() {
        // Exactly 0.0 should work
        let comrat = JpegComrat::parse("0.0").unwrap();
        assert_eq!(comrat.quality(), 1);
    }

    #[test]
    fn test_parse_boundary_just_over() {
        // 99.91 should fail (out of range)
        let result = JpegComrat::parse("99.91");
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // Roundtrip tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_roundtrip_quality_values() {
        for q in [1u8, 25, 50, 75, 100] {
            let original = JpegComrat::Quality(q);
            let comrat_str = original.to_comrat_string();
            let parsed = JpegComrat::parse(&comrat_str).unwrap();
            // Allow small rounding differences due to float conversion
            assert!(
                (parsed.quality() as i16 - q as i16).abs() <= 1,
                "Roundtrip failed for quality {}: got {}",
                q,
                parsed.quality()
            );
        }
    }

    #[test]
    fn test_roundtrip_default() {
        let original = JpegComrat::Default;
        let comrat_str = original.to_comrat_string();
        let parsed = JpegComrat::parse(&comrat_str).unwrap();
        assert_eq!(parsed, JpegComrat::Default);
    }

    #[test]
    fn test_to_comrat_string_format() {
        // Quality values should produce "nn.n" format (4 chars)
        let comrat = JpegComrat::Quality(50);
        let s = comrat.to_comrat_string();
        assert_eq!(s.len(), 4);
        assert!(s.contains('.'));
    }

    #[test]
    fn test_to_comrat_string_min_quality() {
        let comrat = JpegComrat::Quality(1);
        let s = comrat.to_comrat_string();
        assert_eq!(s.len(), 4);
        // Should be close to "00.0"
        let parsed: f32 = s.parse().unwrap();
        assert!(parsed >= 0.0 && parsed <= 5.0);
    }

    #[test]
    fn test_to_comrat_string_max_quality() {
        let comrat = JpegComrat::Quality(100);
        let s = comrat.to_comrat_string();
        assert_eq!(s.len(), 4);
        // Should be close to "99.9"
        let parsed: f32 = s.parse().unwrap();
        assert!(parsed >= 95.0 && parsed <= 99.9);
    }

    // -------------------------------------------------------------------------
    // PartialEq tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_equality_quality() {
        assert_eq!(JpegComrat::Quality(50), JpegComrat::Quality(50));
        assert_ne!(JpegComrat::Quality(50), JpegComrat::Quality(51));
    }

    #[test]
    fn test_equality_default() {
        assert_eq!(JpegComrat::Default, JpegComrat::Default);
        assert_ne!(JpegComrat::Default, JpegComrat::Quality(75));
    }
}
