//! DateTime parsing utilities for NITF date/time strings.
//!
//! This module provides utilities for parsing NITF FDT (File Date Time) format
//! strings into structured datetime components.
//!
//! # Format
//!
//! NITF datetime strings use the format `CCYYMMDDhhmmss` (14 characters):
//! - CCYY: 4-digit year
//! - MM: 2-digit month (01-12) or "--" for unknown
//! - DD: 2-digit day (01-31) or "--" for unknown
//! - hh: 2-digit hour (00-23) or "--" for unknown
//! - mm: 2-digit minute (00-59) or "--" for unknown
//! - ss: 2-digit second (00-59) or "--" for unknown
//!
//! # Example
//!
//! ```
//! use aws_osml_io::jbp::datetime::{parse_nitf_datetime, NitfDateTime};
//!
//! // Parse a complete datetime
//! let dt = parse_nitf_datetime("20231215143022").unwrap();
//! assert_eq!(dt.year, 2023);
//! assert_eq!(dt.month, Some(12));
//! assert_eq!(dt.day, Some(15));
//!
//! // Parse a partial datetime with unknown components
//! let dt = parse_nitf_datetime("2023--15------").unwrap();
//! assert_eq!(dt.year, 2023);
//! assert_eq!(dt.month, None);
//! assert_eq!(dt.day, Some(15));
//! ```

use thiserror::Error;

/// Error parsing NITF datetime strings.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DateTimeParseError {
    /// The datetime string has an invalid length (expected 14 characters).
    #[error("Invalid datetime length: expected 14, got {0}")]
    InvalidLength(usize),

    /// The year component is invalid.
    #[error("Invalid year: {0}")]
    InvalidYear(String),

    /// The month component is invalid (expected 01-12 or "--").
    #[error("Invalid month: {0} (expected 01-12 or --)")]
    InvalidMonth(String),

    /// The day component is invalid (expected 01-31 or "--").
    #[error("Invalid day: {0} (expected 01-31 or --)")]
    InvalidDay(String),

    /// The hour component is invalid (expected 00-23 or "--").
    #[error("Invalid hour: {0} (expected 00-23 or --)")]
    InvalidHour(String),

    /// The minute component is invalid (expected 00-59 or "--").
    #[error("Invalid minute: {0} (expected 00-59 or --)")]
    InvalidMinute(String),

    /// The second component is invalid (expected 00-59 or "--").
    #[error("Invalid second: {0} (expected 00-59 or --)")]
    InvalidSecond(String),
}

/// Parsed NITF datetime with optional components.
///
/// NITF datetime strings can have unknown components represented by "--".
/// This struct captures the parsed values, with `None` for unknown components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NitfDateTime {
    /// 4-digit year (always required)
    pub year: u16,
    /// Month (1-12), or None if unknown
    pub month: Option<u8>,
    /// Day of month (1-31), or None if unknown
    pub day: Option<u8>,
    /// Hour (0-23), or None if unknown
    pub hour: Option<u8>,
    /// Minute (0-59), or None if unknown
    pub minute: Option<u8>,
    /// Second (0-59), or None if unknown
    pub second: Option<u8>,
}

impl NitfDateTime {
    /// Create a new NitfDateTime with all components specified.
    pub fn new(
        year: u16,
        month: Option<u8>,
        day: Option<u8>,
        hour: Option<u8>,
        minute: Option<u8>,
        second: Option<u8>,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Check if all datetime components are present (no unknown values).
    pub fn is_complete(&self) -> bool {
        self.month.is_some()
            && self.day.is_some()
            && self.hour.is_some()
            && self.minute.is_some()
            && self.second.is_some()
    }

    /// Convert to ISO 8601 string representation.
    ///
    /// Unknown components are represented with appropriate placeholders:
    /// - Complete: "2023-12-15T14:30:22"
    /// - Partial: "2023-??-15T??:??:??"
    pub fn to_iso8601(&self) -> String {
        let month_str = self
            .month
            .map(|m| format!("{:02}", m))
            .unwrap_or_else(|| "??".to_string());
        let day_str = self
            .day
            .map(|d| format!("{:02}", d))
            .unwrap_or_else(|| "??".to_string());
        let hour_str = self
            .hour
            .map(|h| format!("{:02}", h))
            .unwrap_or_else(|| "??".to_string());
        let minute_str = self
            .minute
            .map(|m| format!("{:02}", m))
            .unwrap_or_else(|| "??".to_string());
        let second_str = self
            .second
            .map(|s| format!("{:02}", s))
            .unwrap_or_else(|| "??".to_string());

        format!(
            "{:04}-{}-{}T{}:{}:{}",
            self.year, month_str, day_str, hour_str, minute_str, second_str
        )
    }

    /// Convert to NITF FDT format string (CCYYMMDDhhmmss).
    ///
    /// Unknown components are represented as "--".
    pub fn to_nitf_string(&self) -> String {
        let month_str = self
            .month
            .map(|m| format!("{:02}", m))
            .unwrap_or_else(|| "--".to_string());
        let day_str = self
            .day
            .map(|d| format!("{:02}", d))
            .unwrap_or_else(|| "--".to_string());
        let hour_str = self
            .hour
            .map(|h| format!("{:02}", h))
            .unwrap_or_else(|| "--".to_string());
        let minute_str = self
            .minute
            .map(|m| format!("{:02}", m))
            .unwrap_or_else(|| "--".to_string());
        let second_str = self
            .second
            .map(|s| format!("{:02}", s))
            .unwrap_or_else(|| "--".to_string());

        format!(
            "{:04}{}{}{}{}{}",
            self.year, month_str, day_str, hour_str, minute_str, second_str
        )
    }
}

/// Parse a NITF FDT (File Date Time) format string.
///
/// # Format
///
/// The format is `CCYYMMDDhhmmss` (14 characters):
/// - CCYY: 4-digit year (required, must be valid number)
/// - MM: 2-digit month (01-12) or "--" for unknown
/// - DD: 2-digit day (01-31) or "--" for unknown
/// - hh: 2-digit hour (00-23) or "--" for unknown
/// - mm: 2-digit minute (00-59) or "--" for unknown
/// - ss: 2-digit second (00-59) or "--" for unknown
///
/// # Arguments
///
/// * `fdt` - The NITF datetime string to parse
///
/// # Returns
///
/// A `NitfDateTime` struct with parsed components, or a `DateTimeParseError` if invalid.
///
/// # Example
///
/// ```
/// use aws_osml_io::jbp::datetime::parse_nitf_datetime;
///
/// let dt = parse_nitf_datetime("20231215143022").unwrap();
/// assert_eq!(dt.year, 2023);
/// assert_eq!(dt.month, Some(12));
/// ```
pub fn parse_nitf_datetime(fdt: &str) -> Result<NitfDateTime, DateTimeParseError> {
    // Check length
    if fdt.len() != 14 {
        return Err(DateTimeParseError::InvalidLength(fdt.len()));
    }

    // Parse year (always required, positions 0-3)
    let year_str = &fdt[0..4];
    let year: u16 = year_str
        .parse()
        .map_err(|_| DateTimeParseError::InvalidYear(year_str.to_string()))?;

    // Parse month (positions 4-5)
    let month_str = &fdt[4..6];
    let month = parse_optional_component(month_str, 1, 12)
        .map_err(|_| DateTimeParseError::InvalidMonth(month_str.to_string()))?;

    // Parse day (positions 6-7)
    let day_str = &fdt[6..8];
    let day = parse_optional_component(day_str, 1, 31)
        .map_err(|_| DateTimeParseError::InvalidDay(day_str.to_string()))?;

    // Parse hour (positions 8-9)
    let hour_str = &fdt[8..10];
    let hour = parse_optional_component(hour_str, 0, 23)
        .map_err(|_| DateTimeParseError::InvalidHour(hour_str.to_string()))?;

    // Parse minute (positions 10-11)
    let minute_str = &fdt[10..12];
    let minute = parse_optional_component(minute_str, 0, 59)
        .map_err(|_| DateTimeParseError::InvalidMinute(minute_str.to_string()))?;

    // Parse second (positions 12-13)
    let second_str = &fdt[12..14];
    let second = parse_optional_component(second_str, 0, 59)
        .map_err(|_| DateTimeParseError::InvalidSecond(second_str.to_string()))?;

    Ok(NitfDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

/// Parse an optional 2-character component.
///
/// Returns `Ok(None)` if the component is "--" (unknown).
/// Returns `Ok(Some(value))` if the component is a valid number within range.
/// Returns `Err(())` if the component is invalid.
fn parse_optional_component(s: &str, min: u8, max: u8) -> Result<Option<u8>, ()> {
    if s == "--" {
        return Ok(None);
    }

    let value: u8 = s.parse().map_err(|_| ())?;
    if value < min || value > max {
        return Err(());
    }

    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Unit Tests ====================

    #[test]
    fn parse_complete_datetime() {
        let dt = parse_nitf_datetime("20231215143022").unwrap();
        assert_eq!(dt.year, 2023);
        assert_eq!(dt.month, Some(12));
        assert_eq!(dt.day, Some(15));
        assert_eq!(dt.hour, Some(14));
        assert_eq!(dt.minute, Some(30));
        assert_eq!(dt.second, Some(22));
        assert!(dt.is_complete());
    }

    #[test]
    fn parse_datetime_with_unknown_month() {
        let dt = parse_nitf_datetime("2023--15143022").unwrap();
        assert_eq!(dt.year, 2023);
        assert_eq!(dt.month, None);
        assert_eq!(dt.day, Some(15));
        assert_eq!(dt.hour, Some(14));
        assert_eq!(dt.minute, Some(30));
        assert_eq!(dt.second, Some(22));
        assert!(!dt.is_complete());
    }

    #[test]
    fn parse_datetime_with_unknown_day() {
        let dt = parse_nitf_datetime("202312--143022").unwrap();
        assert_eq!(dt.year, 2023);
        assert_eq!(dt.month, Some(12));
        assert_eq!(dt.day, None);
        assert!(!dt.is_complete());
    }

    #[test]
    fn parse_datetime_with_unknown_time() {
        let dt = parse_nitf_datetime("20231215------").unwrap();
        assert_eq!(dt.year, 2023);
        assert_eq!(dt.month, Some(12));
        assert_eq!(dt.day, Some(15));
        assert_eq!(dt.hour, None);
        assert_eq!(dt.minute, None);
        assert_eq!(dt.second, None);
        assert!(!dt.is_complete());
    }

    #[test]
    fn parse_datetime_all_unknown_except_year() {
        let dt = parse_nitf_datetime("2023----------").unwrap();
        assert_eq!(dt.year, 2023);
        assert_eq!(dt.month, None);
        assert_eq!(dt.day, None);
        assert_eq!(dt.hour, None);
        assert_eq!(dt.minute, None);
        assert_eq!(dt.second, None);
        assert!(!dt.is_complete());
    }

    #[test]
    fn parse_datetime_boundary_values() {
        // Minimum valid values
        let dt = parse_nitf_datetime("00000101000000").unwrap();
        assert_eq!(dt.year, 0);
        assert_eq!(dt.month, Some(1));
        assert_eq!(dt.day, Some(1));
        assert_eq!(dt.hour, Some(0));
        assert_eq!(dt.minute, Some(0));
        assert_eq!(dt.second, Some(0));

        // Maximum valid values
        let dt = parse_nitf_datetime("99991231235959").unwrap();
        assert_eq!(dt.year, 9999);
        assert_eq!(dt.month, Some(12));
        assert_eq!(dt.day, Some(31));
        assert_eq!(dt.hour, Some(23));
        assert_eq!(dt.minute, Some(59));
        assert_eq!(dt.second, Some(59));
    }

    #[test]
    fn parse_datetime_invalid_length_short() {
        let result = parse_nitf_datetime("2023121514302");
        assert!(matches!(result, Err(DateTimeParseError::InvalidLength(13))));
    }

    #[test]
    fn parse_datetime_invalid_length_long() {
        let result = parse_nitf_datetime("202312151430220");
        assert!(matches!(result, Err(DateTimeParseError::InvalidLength(15))));
    }

    #[test]
    fn parse_datetime_invalid_length_empty() {
        let result = parse_nitf_datetime("");
        assert!(matches!(result, Err(DateTimeParseError::InvalidLength(0))));
    }

    #[test]
    fn parse_datetime_invalid_year() {
        let result = parse_nitf_datetime("ABCD1215143022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidYear(_))));
    }

    #[test]
    fn parse_datetime_invalid_month_too_high() {
        let result = parse_nitf_datetime("20231315143022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidMonth(_))));
    }

    #[test]
    fn parse_datetime_invalid_month_zero() {
        let result = parse_nitf_datetime("20230015143022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidMonth(_))));
    }

    #[test]
    fn parse_datetime_invalid_month_letters() {
        let result = parse_nitf_datetime("2023AB15143022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidMonth(_))));
    }

    #[test]
    fn parse_datetime_invalid_day_too_high() {
        let result = parse_nitf_datetime("20231232143022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidDay(_))));
    }

    #[test]
    fn parse_datetime_invalid_day_zero() {
        let result = parse_nitf_datetime("20231200143022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidDay(_))));
    }

    #[test]
    fn parse_datetime_invalid_hour() {
        let result = parse_nitf_datetime("20231215243022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidHour(_))));
    }

    #[test]
    fn parse_datetime_invalid_minute() {
        let result = parse_nitf_datetime("20231215146022");
        assert!(matches!(result, Err(DateTimeParseError::InvalidMinute(_))));
    }

    #[test]
    fn parse_datetime_invalid_second() {
        let result = parse_nitf_datetime("20231215143060");
        assert!(matches!(result, Err(DateTimeParseError::InvalidSecond(_))));
    }

    #[test]
    fn nitf_datetime_to_iso8601_complete() {
        let dt = NitfDateTime::new(2023, Some(12), Some(15), Some(14), Some(30), Some(22));
        assert_eq!(dt.to_iso8601(), "2023-12-15T14:30:22");
    }

    #[test]
    fn nitf_datetime_to_iso8601_partial() {
        let dt = NitfDateTime::new(2023, None, Some(15), None, None, None);
        assert_eq!(dt.to_iso8601(), "2023-??-15T??:??:??");
    }

    #[test]
    fn nitf_datetime_to_iso8601_all_unknown() {
        let dt = NitfDateTime::new(2023, None, None, None, None, None);
        assert_eq!(dt.to_iso8601(), "2023-??-??T??:??:??");
    }

    #[test]
    fn nitf_datetime_to_nitf_string_complete() {
        let dt = NitfDateTime::new(2023, Some(12), Some(15), Some(14), Some(30), Some(22));
        assert_eq!(dt.to_nitf_string(), "20231215143022");
    }

    #[test]
    fn nitf_datetime_to_nitf_string_partial() {
        let dt = NitfDateTime::new(2023, None, Some(15), None, None, None);
        assert_eq!(dt.to_nitf_string(), "2023--15------");
    }

    #[test]
    fn nitf_datetime_to_nitf_string_all_unknown() {
        let dt = NitfDateTime::new(2023, None, None, None, None, None);
        assert_eq!(dt.to_nitf_string(), "2023----------");
    }

    #[test]
    fn nitf_datetime_is_complete_true() {
        let dt = NitfDateTime::new(2023, Some(12), Some(15), Some(14), Some(30), Some(22));
        assert!(dt.is_complete());
    }

    #[test]
    fn nitf_datetime_is_complete_false_missing_month() {
        let dt = NitfDateTime::new(2023, None, Some(15), Some(14), Some(30), Some(22));
        assert!(!dt.is_complete());
    }

    #[test]
    fn nitf_datetime_is_complete_false_missing_second() {
        let dt = NitfDateTime::new(2023, Some(12), Some(15), Some(14), Some(30), None);
        assert!(!dt.is_complete());
    }

    #[test]
    fn datetime_parse_error_display() {
        assert_eq!(
            DateTimeParseError::InvalidLength(10).to_string(),
            "Invalid datetime length: expected 14, got 10"
        );
        assert_eq!(
            DateTimeParseError::InvalidYear("ABCD".to_string()).to_string(),
            "Invalid year: ABCD"
        );
        assert_eq!(
            DateTimeParseError::InvalidMonth("13".to_string()).to_string(),
            "Invalid month: 13 (expected 01-12 or --)"
        );
        assert_eq!(
            DateTimeParseError::InvalidDay("32".to_string()).to_string(),
            "Invalid day: 32 (expected 01-31 or --)"
        );
        assert_eq!(
            DateTimeParseError::InvalidHour("24".to_string()).to_string(),
            "Invalid hour: 24 (expected 00-23 or --)"
        );
        assert_eq!(
            DateTimeParseError::InvalidMinute("60".to_string()).to_string(),
            "Invalid minute: 60 (expected 00-59 or --)"
        );
        assert_eq!(
            DateTimeParseError::InvalidSecond("60".to_string()).to_string(),
            "Invalid second: 60 (expected 00-59 or --)"
        );
    }

    #[test]
    fn round_trip_complete_datetime() {
        let original = "20231215143022";
        let dt = parse_nitf_datetime(original).unwrap();
        let round_tripped = dt.to_nitf_string();
        assert_eq!(original, round_tripped);
    }

    #[test]
    fn round_trip_partial_datetime() {
        let original = "2023--15------";
        let dt = parse_nitf_datetime(original).unwrap();
        let round_tripped = dt.to_nitf_string();
        assert_eq!(original, round_tripped);
    }

    #[test]
    fn round_trip_all_unknown() {
        let original = "2023----------";
        let dt = parse_nitf_datetime(original).unwrap();
        let round_tripped = dt.to_nitf_string();
        assert_eq!(original, round_tripped);
    }
}


#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy to generate valid year values (0-9999)
    fn valid_year() -> impl Strategy<Value = u16> {
        0u16..=9999u16
    }

    /// Strategy to generate valid month values (1-12)
    fn valid_month() -> impl Strategy<Value = u8> {
        1u8..=12u8
    }

    /// Strategy to generate valid day values (1-31)
    fn valid_day() -> impl Strategy<Value = u8> {
        1u8..=31u8
    }

    /// Strategy to generate valid hour values (0-23)
    fn valid_hour() -> impl Strategy<Value = u8> {
        0u8..=23u8
    }

    /// Strategy to generate valid minute values (0-59)
    fn valid_minute() -> impl Strategy<Value = u8> {
        0u8..=59u8
    }

    /// Strategy to generate valid second values (0-59)
    fn valid_second() -> impl Strategy<Value = u8> {
        0u8..=59u8
    }

    /// Strategy to generate an optional component (Some(value) or None)
    fn optional_component<S: Strategy<Value = u8>>(
        value_strategy: S,
    ) -> impl Strategy<Value = Option<u8>> {
        prop_oneof![
            Just(None),
            value_strategy.prop_map(Some),
        ]
    }

    /// Strategy to generate a complete valid NITF datetime string
    fn valid_complete_datetime_string() -> impl Strategy<Value = String> {
        (
            valid_year(),
            valid_month(),
            valid_day(),
            valid_hour(),
            valid_minute(),
            valid_second(),
        )
            .prop_map(|(year, month, day, hour, minute, second)| {
                format!(
                    "{:04}{:02}{:02}{:02}{:02}{:02}",
                    year, month, day, hour, minute, second
                )
            })
    }

    /// Strategy to generate a valid NITF datetime string with optional unknown components
    fn valid_datetime_string_with_unknowns() -> impl Strategy<Value = String> {
        (
            valid_year(),
            optional_component(valid_month()),
            optional_component(valid_day()),
            optional_component(valid_hour()),
            optional_component(valid_minute()),
            optional_component(valid_second()),
        )
            .prop_map(|(year, month, day, hour, minute, second)| {
                let month_str = month.map(|m| format!("{:02}", m)).unwrap_or_else(|| "--".to_string());
                let day_str = day.map(|d| format!("{:02}", d)).unwrap_or_else(|| "--".to_string());
                let hour_str = hour.map(|h| format!("{:02}", h)).unwrap_or_else(|| "--".to_string());
                let minute_str = minute.map(|m| format!("{:02}", m)).unwrap_or_else(|| "--".to_string());
                let second_str = second.map(|s| format!("{:02}", s)).unwrap_or_else(|| "--".to_string());
                format!(
                    "{:04}{}{}{}{}{}",
                    year, month_str, day_str, hour_str, minute_str, second_str
                )
            })
    }

    /// Strategy to generate invalid datetime strings
    fn invalid_datetime_string() -> impl Strategy<Value = String> {
        prop_oneof![
            // Wrong length (too short)
            "[0-9]{1,13}".prop_filter("must be shorter than 14", |s| s.len() < 14),
            // Wrong length (too long)
            "[0-9]{15,20}",
            // Invalid year (non-numeric)
            "[A-Za-z]{4}[0-9]{10}",
            // Invalid month (out of range)
            "[0-9]{4}(00|13|14|15|99)[0-9]{8}",
            // Invalid day (out of range)
            "[0-9]{6}(00|32|33|99)[0-9]{6}",
            // Invalid hour (out of range)
            "[0-9]{8}(24|25|99)[0-9]{4}",
            // Invalid minute (out of range)
            "[0-9]{10}(60|61|99)[0-9]{2}",
            // Invalid second (out of range)
            "[0-9]{12}(60|61|99)",
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Feature: jbp-dataset-integration
        /// Property 17: DateTime Parsing Round-Trip
        /// *For any* valid NITF datetime string (CCYYMMDDhhmmss format), parsing with
        /// DateTime_Parser and converting back to string SHALL produce an equivalent representation.
        /// **Validates: Requirements 16.1, 16.2**
        #[test]
        fn prop_datetime_parsing_round_trip(datetime_str in valid_complete_datetime_string()) {
            let parsed = parse_nitf_datetime(&datetime_str).expect("Should parse valid datetime");
            let round_tripped = parsed.to_nitf_string();
            prop_assert_eq!(
                datetime_str,
                round_tripped,
                "Round-trip should produce identical string"
            );
        }

        /// Feature: jbp-dataset-integration
        /// Property 17 (extended): DateTime Parsing Round-Trip with Unknown Components
        /// *For any* valid NITF datetime string with "--" for unknown components,
        /// parsing and converting back SHALL produce an equivalent representation.
        /// **Validates: Requirements 16.1, 16.2, 16.3**
        #[test]
        fn prop_datetime_parsing_round_trip_with_unknowns(datetime_str in valid_datetime_string_with_unknowns()) {
            let parsed = parse_nitf_datetime(&datetime_str).expect("Should parse valid datetime");
            let round_tripped = parsed.to_nitf_string();
            prop_assert_eq!(
                datetime_str,
                round_tripped,
                "Round-trip should produce identical string even with unknown components"
            );
        }

        /// Feature: jbp-dataset-integration
        /// Property 18: DateTime Partial Date Handling
        /// *For any* datetime string with "--" components, DateTime_Parser SHALL return
        /// a NitfDateTime with None for the unknown components.
        /// **Validates: Requirements 16.3**
        #[test]
        fn prop_datetime_partial_date_handling(
            year in valid_year(),
            month in optional_component(valid_month()),
            day in optional_component(valid_day()),
            hour in optional_component(valid_hour()),
            minute in optional_component(valid_minute()),
            second in optional_component(valid_second()),
        ) {
            let month_str = month.map(|m| format!("{:02}", m)).unwrap_or_else(|| "--".to_string());
            let day_str = day.map(|d| format!("{:02}", d)).unwrap_or_else(|| "--".to_string());
            let hour_str = hour.map(|h| format!("{:02}", h)).unwrap_or_else(|| "--".to_string());
            let minute_str = minute.map(|m| format!("{:02}", m)).unwrap_or_else(|| "--".to_string());
            let second_str = second.map(|s| format!("{:02}", s)).unwrap_or_else(|| "--".to_string());

            let datetime_str = format!(
                "{:04}{}{}{}{}{}",
                year, month_str, day_str, hour_str, minute_str, second_str
            );

            let parsed = parse_nitf_datetime(&datetime_str).expect("Should parse valid datetime");

            prop_assert_eq!(parsed.year, year, "Year should match");
            prop_assert_eq!(parsed.month, month, "Month should match (None for unknown)");
            prop_assert_eq!(parsed.day, day, "Day should match (None for unknown)");
            prop_assert_eq!(parsed.hour, hour, "Hour should match (None for unknown)");
            prop_assert_eq!(parsed.minute, minute, "Minute should match (None for unknown)");
            prop_assert_eq!(parsed.second, second, "Second should match (None for unknown)");
        }

        /// Feature: jbp-dataset-integration
        /// Property 19: DateTime Invalid Input Error
        /// *For any* string that is not a valid NITF datetime format (wrong length,
        /// invalid characters, out-of-range values), DateTime_Parser SHALL return a DateTimeParseError.
        /// **Validates: Requirements 16.4**
        #[test]
        fn prop_datetime_invalid_input_error(invalid_str in invalid_datetime_string()) {
            let result = parse_nitf_datetime(&invalid_str);
            prop_assert!(
                result.is_err(),
                "Invalid datetime string '{}' should produce an error, but got: {:?}",
                invalid_str,
                result
            );
        }

        /// Feature: jbp-dataset-integration
        /// Property 17 (ISO 8601): DateTime ISO 8601 Conversion Consistency
        /// *For any* valid complete NITF datetime, the ISO 8601 representation SHALL
        /// contain the same date/time components.
        /// **Validates: Requirements 16.1, 16.2**
        #[test]
        fn prop_datetime_iso8601_consistency(
            year in valid_year(),
            month in valid_month(),
            day in valid_day(),
            hour in valid_hour(),
            minute in valid_minute(),
            second in valid_second(),
        ) {
            let dt = NitfDateTime::new(year, Some(month), Some(day), Some(hour), Some(minute), Some(second));
            let iso = dt.to_iso8601();

            // Parse the ISO string to verify components
            let expected = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                year, month, day, hour, minute, second
            );
            prop_assert_eq!(iso, expected, "ISO 8601 format should match expected");
        }

        /// Feature: jbp-dataset-integration
        /// Property 18 (is_complete): DateTime Completeness Check
        /// *For any* NitfDateTime, is_complete() SHALL return true if and only if
        /// all optional components are Some.
        /// **Validates: Requirements 16.3**
        #[test]
        fn prop_datetime_is_complete_consistency(
            year in valid_year(),
            month in optional_component(valid_month()),
            day in optional_component(valid_day()),
            hour in optional_component(valid_hour()),
            minute in optional_component(valid_minute()),
            second in optional_component(valid_second()),
        ) {
            let dt = NitfDateTime::new(year, month, day, hour, minute, second);
            let expected_complete = month.is_some()
                && day.is_some()
                && hour.is_some()
                && minute.is_some()
                && second.is_some();

            prop_assert_eq!(
                dt.is_complete(),
                expected_complete,
                "is_complete() should return true iff all components are Some"
            );
        }
    }
}
