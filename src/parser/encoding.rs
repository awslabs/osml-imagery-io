//! Character set validation for NITF binary formats.
//!
//! This module provides validators for NITF character encodings:
//! - BCS-A (Basic Character Set - Alphanumeric): ASCII 0x20-0x7E
//! - BCS-N (Basic Character Set - Numeric): digits 0-9 and space
//! - ECS-A (Extended Character Set - Alphanumeric): extended range
//!
//! # Examples
//!
//! ```ignore
//! use _io::parser::encoding::{validate_bcs_a, validate_bcs_n, validate_ecs_a};
//!
//! assert!(validate_bcs_a(b"Hello World"));
//! assert!(validate_bcs_n(b"12345"));
//! assert!(validate_ecs_a(b"Extended \xA0 chars"));
//! ```

/// Validate BCS-A (Basic Character Set - Alphanumeric).
///
/// BCS-A allows ASCII printable characters in the range 0x20-0x7E (space through tilde).
/// This is the standard character set for most NITF text fields.
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// `true` if all bytes are valid BCS-A characters, `false` otherwise.
///
/// # Examples
///
/// ```ignore
/// use _io::parser::encoding::validate_bcs_a;
///
/// assert!(validate_bcs_a(b"NITF02.10"));
/// assert!(validate_bcs_a(b"Hello, World!"));
/// assert!(!validate_bcs_a(b"Invalid\x00char"));
/// assert!(!validate_bcs_a(b"\x7F")); // DEL is invalid
/// ```
#[inline]
pub fn validate_bcs_a(data: &[u8]) -> bool {
    data.iter().all(|&b| is_valid_bcs_a_byte(b))
}

/// Check if a single byte is valid BCS-A.
///
/// # Arguments
///
/// * `byte` - The byte to validate
///
/// # Returns
///
/// `true` if the byte is in the range 0x20-0x7E, `false` otherwise.
#[inline]
pub fn is_valid_bcs_a_byte(byte: u8) -> bool {
    (0x20..=0x7E).contains(&byte)
}

/// Validate BCS-N (Basic Character Set - Numeric).
///
/// Per JBP spec (Section 5.2, page 25), BCS-N allows digits (0x30-0x39),
/// plus sign (0x2B), minus sign (0x2D), decimal point (0x2E), slash (0x2F),
/// and space (0x20). This is used for numeric fields in NITF headers that
/// may contain signed values, decimals, or fractions.
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// `true` if all bytes are valid BCS-N characters, `false` otherwise.
///
/// # Examples
///
/// ```ignore
/// use _io::parser::encoding::validate_bcs_n;
///
/// assert!(validate_bcs_n(b"12345"));
/// assert!(validate_bcs_n(b"  123")); // leading spaces allowed
/// assert!(validate_bcs_n(b"+22.77")); // signed decimal allowed
/// assert!(validate_bcs_n(b"-99.99")); // negative decimal allowed
/// assert!(!validate_bcs_n(b"12A45")); // letters not allowed
/// ```
#[inline]
pub fn validate_bcs_n(data: &[u8]) -> bool {
    data.iter().all(|&b| is_valid_bcs_n_byte(b))
}

/// Check if a single byte is valid BCS-N.
///
/// # Arguments
///
/// * `byte` - The byte to validate
///
/// # Returns
///
/// `true` if the byte is a digit (0x30-0x39), plus (0x2B), minus (0x2D),
/// decimal point (0x2E), slash (0x2F), or space (0x20).
#[inline]
pub fn is_valid_bcs_n_byte(byte: u8) -> bool {
    (0x30..=0x39).contains(&byte)
        || byte == 0x20 // space
        || byte == 0x2B // '+'
        || byte == 0x2D // '-'
        || byte == 0x2E // '.'
        || byte == 0x2F // '/'
}

/// Validate BCS-NPI (Basic Character Set - Numeric Positive Integer).
///
/// BCS-NPI allows only digits (0x30-0x39) and space (0x20). This is the
/// restricted numeric subset used for positive integer fields.
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// `true` if all bytes are valid BCS-NPI characters, `false` otherwise.
#[inline]
pub fn validate_bcs_npi(data: &[u8]) -> bool {
    data.iter().all(|&b| is_valid_bcs_npi_byte(b))
}

/// Check if a single byte is valid BCS-NPI.
///
/// # Arguments
///
/// * `byte` - The byte to validate
///
/// # Returns
///
/// `true` if the byte is a digit (0x30-0x39) or space (0x20), `false` otherwise.
#[inline]
pub fn is_valid_bcs_npi_byte(byte: u8) -> bool {
    (0x30..=0x39).contains(&byte) || byte == 0x20
}

/// Validate ECS-A (Extended Character Set - Alphanumeric).
///
/// ECS-A allows a broader range of characters than BCS-A, including extended
/// ASCII characters. Valid bytes are 0x20 and above (excluding control characters).
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// `true` if all bytes are valid ECS-A characters, `false` otherwise.
///
/// # Examples
///
/// ```ignore
/// use _io::parser::encoding::validate_ecs_a;
///
/// assert!(validate_ecs_a(b"Hello World"));
/// assert!(validate_ecs_a(&[0x20, 0x80, 0xFF])); // extended chars allowed
/// assert!(!validate_ecs_a(b"\x00")); // NUL not allowed
/// assert!(!validate_ecs_a(b"\x1F")); // control chars not allowed
/// ```
#[inline]
pub fn validate_ecs_a(data: &[u8]) -> bool {
    data.iter().all(|&b| is_valid_ecs_a_byte(b))
}

/// Check if a single byte is valid ECS-A.
///
/// # Arguments
///
/// * `byte` - The byte to validate
///
/// # Returns
///
/// `true` if the byte is 0x20 or greater, `false` otherwise.
#[inline]
pub fn is_valid_ecs_a_byte(byte: u8) -> bool {
    byte >= 0x20
}

/// Result of character set validation with detailed error information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    /// Whether the validation passed
    pub valid: bool,
    /// Index of the first invalid byte (if any)
    pub first_invalid_index: Option<usize>,
    /// The first invalid byte value (if any)
    pub first_invalid_byte: Option<u8>,
}

impl ValidationResult {
    /// Create a successful validation result.
    pub fn valid() -> Self {
        Self {
            valid: true,
            first_invalid_index: None,
            first_invalid_byte: None,
        }
    }

    /// Create a failed validation result.
    pub fn invalid(index: usize, byte: u8) -> Self {
        Self {
            valid: false,
            first_invalid_index: Some(index),
            first_invalid_byte: Some(byte),
        }
    }
}

/// Validate BCS-A with detailed error information.
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// A `ValidationResult` containing validation status and error details.
pub fn validate_bcs_a_detailed(data: &[u8]) -> ValidationResult {
    for (i, &byte) in data.iter().enumerate() {
        if !is_valid_bcs_a_byte(byte) {
            return ValidationResult::invalid(i, byte);
        }
    }
    ValidationResult::valid()
}

/// Validate BCS-N with detailed error information.
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// A `ValidationResult` containing validation status and error details.
pub fn validate_bcs_n_detailed(data: &[u8]) -> ValidationResult {
    for (i, &byte) in data.iter().enumerate() {
        if !is_valid_bcs_n_byte(byte) {
            return ValidationResult::invalid(i, byte);
        }
    }
    ValidationResult::valid()
}

/// Validate ECS-A with detailed error information.
///
/// # Arguments
///
/// * `data` - The byte slice to validate
///
/// # Returns
///
/// A `ValidationResult` containing validation status and error details.
pub fn validate_ecs_a_detailed(data: &[u8]) -> ValidationResult {
    for (i, &byte) in data.iter().enumerate() {
        if !is_valid_ecs_a_byte(byte) {
            return ValidationResult::invalid(i, byte);
        }
    }
    ValidationResult::valid()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== BCS-A Unit Tests ====================

    #[test]
    fn bcs_a_valid_printable_ascii() {
        // All printable ASCII characters should be valid
        assert!(validate_bcs_a(b" !\"#$%&'()*+,-./"));
        assert!(validate_bcs_a(b"0123456789"));
        assert!(validate_bcs_a(b":;<=>?@"));
        assert!(validate_bcs_a(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
        assert!(validate_bcs_a(b"[\\]^_`"));
        assert!(validate_bcs_a(b"abcdefghijklmnopqrstuvwxyz"));
        assert!(validate_bcs_a(b"{|}~"));
    }

    #[test]
    fn bcs_a_boundary_values() {
        // Boundary tests
        assert!(is_valid_bcs_a_byte(0x20)); // space - lower bound
        assert!(is_valid_bcs_a_byte(0x7E)); // tilde - upper bound
        assert!(!is_valid_bcs_a_byte(0x1F)); // just below lower bound
        assert!(!is_valid_bcs_a_byte(0x7F)); // DEL - just above upper bound
    }

    #[test]
    fn bcs_a_invalid_control_chars() {
        // Control characters should be invalid
        assert!(!validate_bcs_a(b"\x00")); // NUL
        assert!(!validate_bcs_a(b"\x01")); // SOH
        assert!(!validate_bcs_a(b"\x09")); // TAB
        assert!(!validate_bcs_a(b"\x0A")); // LF
        assert!(!validate_bcs_a(b"\x0D")); // CR
        assert!(!validate_bcs_a(b"\x1F")); // US
    }

    #[test]
    fn bcs_a_invalid_extended_ascii() {
        // Extended ASCII should be invalid
        assert!(!validate_bcs_a(&[0x80]));
        assert!(!validate_bcs_a(&[0xFF]));
        assert!(!validate_bcs_a(&[0xA0])); // non-breaking space
    }

    #[test]
    fn bcs_a_empty_slice() {
        // Empty slice should be valid
        assert!(validate_bcs_a(b""));
    }

    #[test]
    fn bcs_a_nitf_header_example() {
        // Typical NITF header values
        assert!(validate_bcs_a(b"NITF02.10"));
        assert!(validate_bcs_a(b"BF01"));
        assert!(validate_bcs_a(b"UNCLAS"));
    }

    // ==================== BCS-N Unit Tests ====================

    #[test]
    fn bcs_n_valid_digits() {
        assert!(validate_bcs_n(b"0123456789"));
        assert!(validate_bcs_n(b"00000"));
        assert!(validate_bcs_n(b"99999"));
    }

    #[test]
    fn bcs_n_valid_with_spaces() {
        assert!(validate_bcs_n(b"   "));
        assert!(validate_bcs_n(b"  123"));
        assert!(validate_bcs_n(b"123  "));
        assert!(validate_bcs_n(b" 1 2 "));
    }

    #[test]
    fn bcs_n_valid_with_signs_and_decimals() {
        assert!(validate_bcs_n(b"+22.7715"));
        assert!(validate_bcs_n(b"-99.99"));
        assert!(validate_bcs_n(b"0002.16"));
        assert!(validate_bcs_n(b"+121.1816"));
        assert!(validate_bcs_n(b"1/2"));
    }

    #[test]
    fn bcs_n_boundary_values() {
        assert!(is_valid_bcs_n_byte(0x20)); // space
        assert!(is_valid_bcs_n_byte(0x2B)); // '+'
        assert!(is_valid_bcs_n_byte(0x2D)); // '-'
        assert!(is_valid_bcs_n_byte(0x2E)); // '.'
        assert!(is_valid_bcs_n_byte(0x2F)); // '/'
        assert!(is_valid_bcs_n_byte(0x30)); // '0'
        assert!(is_valid_bcs_n_byte(0x39)); // '9'
        assert!(!is_valid_bcs_n_byte(0x3A)); // ':' - just above '9'
        assert!(!is_valid_bcs_n_byte(0x2A)); // '*' - just below '+'
    }

    #[test]
    fn bcs_n_invalid_letters() {
        assert!(!validate_bcs_n(b"A"));
        assert!(!validate_bcs_n(b"12A34"));
        assert!(!validate_bcs_n(b"abc"));
        assert!(!validate_bcs_n(b"1.5E3")); // 'E' not allowed
    }

    #[test]
    fn bcs_n_invalid_special_chars() {
        assert!(!validate_bcs_n(b"1,234")); // comma
        assert!(!validate_bcs_n(b"12@34")); // at sign
    }

    #[test]
    fn bcs_n_empty_slice() {
        assert!(validate_bcs_n(b""));
    }

    #[test]
    fn bcs_n_nitf_numeric_fields() {
        // Typical NITF numeric field values
        assert!(validate_bcs_n(b"000001")); // segment count
        assert!(validate_bcs_n(b"00000439")); // length field
        assert!(validate_bcs_n(b"20231215")); // date
    }

    // ==================== BCS-NPI Unit Tests ====================

    #[test]
    fn bcs_npi_valid_digits() {
        assert!(validate_bcs_npi(b"0123456789"));
        assert!(validate_bcs_npi(b"00000"));
        assert!(validate_bcs_npi(b"99999"));
    }

    #[test]
    fn bcs_npi_valid_with_spaces() {
        assert!(validate_bcs_npi(b"   "));
        assert!(validate_bcs_npi(b"  123"));
    }

    #[test]
    fn bcs_npi_rejects_signs_and_decimals() {
        assert!(!validate_bcs_npi(b"-123"));
        assert!(!validate_bcs_npi(b"+123"));
        assert!(!validate_bcs_npi(b"12.34"));
        assert!(!validate_bcs_npi(b"1/2"));
    }

    #[test]
    fn bcs_npi_rejects_letters() {
        assert!(!validate_bcs_npi(b"A"));
        assert!(!validate_bcs_npi(b"12A34"));
    }

    #[test]
    fn bcs_npi_empty_slice() {
        assert!(validate_bcs_npi(b""));
    }

    // ==================== ECS-A Unit Tests ====================

    #[test]
    fn ecs_a_valid_printable_ascii() {
        // All printable ASCII should be valid
        assert!(validate_ecs_a(b"Hello World!"));
        assert!(validate_ecs_a(b"0123456789"));
        assert!(validate_ecs_a(b"~"));
    }

    #[test]
    fn ecs_a_valid_extended_chars() {
        // Extended ASCII should be valid
        assert!(validate_ecs_a(&[0x80]));
        assert!(validate_ecs_a(&[0xA0])); // non-breaking space
        assert!(validate_ecs_a(&[0xFF]));
        assert!(validate_ecs_a(&[0x20, 0x80, 0xC0, 0xFF]));
    }

    #[test]
    fn ecs_a_boundary_values() {
        assert!(is_valid_ecs_a_byte(0x20)); // space - lower bound
        assert!(is_valid_ecs_a_byte(0xFF)); // max byte value
        assert!(!is_valid_ecs_a_byte(0x1F)); // just below lower bound
        assert!(!is_valid_ecs_a_byte(0x00)); // NUL
    }

    #[test]
    fn ecs_a_invalid_control_chars() {
        assert!(!validate_ecs_a(b"\x00"));
        assert!(!validate_ecs_a(b"\x01"));
        assert!(!validate_ecs_a(b"\x1F"));
        assert!(!validate_ecs_a(&[0x00, 0x20])); // mixed
    }

    #[test]
    fn ecs_a_empty_slice() {
        assert!(validate_ecs_a(b""));
    }

    // ==================== Detailed Validation Tests ====================

    #[test]
    fn detailed_bcs_a_valid() {
        let result = validate_bcs_a_detailed(b"Hello");
        assert!(result.valid);
        assert_eq!(result.first_invalid_index, None);
        assert_eq!(result.first_invalid_byte, None);
    }

    #[test]
    fn detailed_bcs_a_invalid() {
        let result = validate_bcs_a_detailed(b"Hello\x00World");
        assert!(!result.valid);
        assert_eq!(result.first_invalid_index, Some(5));
        assert_eq!(result.first_invalid_byte, Some(0x00));
    }

    #[test]
    fn detailed_bcs_n_valid() {
        let result = validate_bcs_n_detailed(b"12345");
        assert!(result.valid);
    }

    #[test]
    fn detailed_bcs_n_invalid() {
        let result = validate_bcs_n_detailed(b"12A45");
        assert!(!result.valid);
        assert_eq!(result.first_invalid_index, Some(2));
        assert_eq!(result.first_invalid_byte, Some(b'A'));
    }

    #[test]
    fn detailed_ecs_a_valid() {
        let result = validate_ecs_a_detailed(&[0x20, 0x80, 0xFF]);
        assert!(result.valid);
    }

    #[test]
    fn detailed_ecs_a_invalid() {
        let result = validate_ecs_a_detailed(&[0x20, 0x1F, 0x80]);
        assert!(!result.valid);
        assert_eq!(result.first_invalid_index, Some(1));
        assert_eq!(result.first_invalid_byte, Some(0x1F));
    }
}

/// Property-based tests for character set validation.
/// These tests verify universal properties across many random inputs.
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Property 5: BCS-A Character Validation
    /// For any byte sequence, the BCS-A validator SHALL return true if and only if
    /// all bytes are in the range 0x20-0x7E (ASCII printable characters).
    /// **Validates: Requirements 2.3**
    mod prop_5_bcs_a_validation {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// For any byte, is_valid_bcs_a_byte returns true iff byte is in 0x20-0x7E
            #[test]
            fn single_byte_validation(byte: u8) {
                let expected = byte >= 0x20 && byte <= 0x7E;
                let actual = is_valid_bcs_a_byte(byte);
                prop_assert_eq!(expected, actual,
                    "Byte 0x{:02X}: expected {}, got {}", byte, expected, actual);
            }

            /// For any byte sequence, validate_bcs_a returns true iff all bytes are valid
            #[test]
            fn slice_validation(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
                let expected = bytes.iter().all(|&b| b >= 0x20 && b <= 0x7E);
                let actual = validate_bcs_a(&bytes);
                prop_assert_eq!(expected, actual);
            }

            /// All valid BCS-A bytes (0x20-0x7E) should pass validation
            #[test]
            fn valid_range_passes(byte in 0x20u8..=0x7E) {
                prop_assert!(is_valid_bcs_a_byte(byte),
                    "Byte 0x{:02X} should be valid BCS-A", byte);
            }

            /// All bytes below 0x20 should fail validation
            #[test]
            fn below_range_fails(byte in 0x00u8..0x20) {
                prop_assert!(!is_valid_bcs_a_byte(byte),
                    "Byte 0x{:02X} should be invalid BCS-A", byte);
            }

            /// All bytes above 0x7E should fail validation
            #[test]
            fn above_range_fails(byte in 0x7Fu8..=0xFF) {
                prop_assert!(!is_valid_bcs_a_byte(byte),
                    "Byte 0x{:02X} should be invalid BCS-A", byte);
            }

            /// A slice of only valid bytes should pass
            #[test]
            fn valid_only_slice_passes(bytes in prop::collection::vec(0x20u8..=0x7E, 0..100)) {
                prop_assert!(validate_bcs_a(&bytes));
            }

            /// A slice with at least one invalid byte should fail
            #[test]
            fn invalid_byte_in_slice_fails(
                valid_bytes in prop::collection::vec(0x20u8..=0x7E, 0..50),
                invalid_byte in prop::sample::select(vec![0x00u8, 0x1F, 0x7F, 0x80, 0xFF]),
                insert_pos in 0usize..=50,
            ) {
                let mut bytes = valid_bytes;
                let pos = insert_pos.min(bytes.len());
                bytes.insert(pos, invalid_byte);
                prop_assert!(!validate_bcs_a(&bytes),
                    "Slice with invalid byte 0x{:02X} should fail", invalid_byte);
            }

            /// Detailed validation should report correct position of first invalid byte
            #[test]
            fn detailed_reports_first_invalid(
                prefix in prop::collection::vec(0x20u8..=0x7E, 0..20),
                invalid_byte in prop::sample::select(vec![0x00u8, 0x1F, 0x7F, 0x80, 0xFF]),
                suffix in prop::collection::vec(any::<u8>(), 0..20),
            ) {
                let mut bytes = prefix.clone();
                bytes.push(invalid_byte);
                bytes.extend(suffix);

                let result = validate_bcs_a_detailed(&bytes);
                prop_assert!(!result.valid);
                prop_assert_eq!(result.first_invalid_index, Some(prefix.len()));
                prop_assert_eq!(result.first_invalid_byte, Some(invalid_byte));
            }
        }
    }

    /// Property 6: BCS-N Character Validation
    /// For any byte sequence, the BCS-N validator SHALL return true if and only if
    /// all bytes are digits (0x30-0x39), plus (0x2B), minus (0x2D), decimal point (0x2E),
    /// slash (0x2F), or space (0x20).
    /// **Validates: Requirements 2.4**
    mod prop_6_bcs_n_validation {
        use super::*;

        /// Generate a valid BCS-N byte (digit, space, plus, minus, decimal, slash)
        fn valid_bcs_n_byte() -> impl Strategy<Value = u8> {
            prop_oneof![
                Just(0x20u8),    // space
                Just(0x2Bu8),    // '+'
                Just(0x2Du8),    // '-'
                Just(0x2Eu8),    // '.'
                Just(0x2Fu8),    // '/'
                0x30u8..=0x39u8, // digits '0'-'9'
            ]
        }

        /// Generate an invalid BCS-N byte
        fn invalid_bcs_n_byte() -> impl Strategy<Value = u8> {
            prop_oneof![
                0x00u8..0x20u8,  // control chars (below space)
                Just(0x21u8),    // '!'
                Just(0x22u8),    // '"'
                Just(0x23u8),    // '#'
                Just(0x24u8),    // '$'
                Just(0x25u8),    // '%'
                Just(0x26u8),    // '&'
                Just(0x27u8),    // '\''
                Just(0x28u8),    // '('
                Just(0x29u8),    // ')'
                Just(0x2Au8),    // '*'
                Just(0x2Cu8),    // ','
                0x3Au8..=0xFFu8, // everything after '9'
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// For any byte, is_valid_bcs_n_byte returns true iff byte is in the valid set
            #[test]
            fn single_byte_validation(byte: u8) {
                let expected = (byte >= 0x30 && byte <= 0x39)
                    || byte == 0x20
                    || byte == 0x2B
                    || byte == 0x2D
                    || byte == 0x2E
                    || byte == 0x2F;
                let actual = is_valid_bcs_n_byte(byte);
                prop_assert_eq!(expected, actual,
                    "Byte 0x{:02X}: expected {}, got {}", byte, expected, actual);
            }

            /// For any byte sequence, validate_bcs_n returns true iff all bytes are valid
            #[test]
            fn slice_validation(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
                let expected = bytes.iter().all(|&b|
                    (b >= 0x30 && b <= 0x39) || b == 0x20 || b == 0x2B || b == 0x2D || b == 0x2E || b == 0x2F
                );
                let actual = validate_bcs_n(&bytes);
                prop_assert_eq!(expected, actual);
            }

            /// All digit bytes (0x30-0x39) should pass validation
            #[test]
            fn digits_pass(byte in 0x30u8..=0x39) {
                prop_assert!(is_valid_bcs_n_byte(byte),
                    "Digit byte 0x{:02X} should be valid BCS-N", byte);
            }

            /// Space (0x20) should pass validation
            #[test]
            fn space_passes(_unused in 0..1i32) {
                prop_assert!(is_valid_bcs_n_byte(0x20));
            }

            /// Plus, minus, decimal point, slash should pass validation
            #[test]
            fn sign_and_decimal_pass(_unused in 0..1i32) {
                prop_assert!(is_valid_bcs_n_byte(0x2B), "'+' should be valid BCS-N");
                prop_assert!(is_valid_bcs_n_byte(0x2D), "'-' should be valid BCS-N");
                prop_assert!(is_valid_bcs_n_byte(0x2E), "'.' should be valid BCS-N");
                prop_assert!(is_valid_bcs_n_byte(0x2F), "'/' should be valid BCS-N");
            }

            /// Letters should fail validation
            #[test]
            fn letters_fail(byte in prop::sample::select(
                (b'A'..=b'Z').chain(b'a'..=b'z').collect::<Vec<_>>()
            )) {
                prop_assert!(!is_valid_bcs_n_byte(byte),
                    "Letter byte 0x{:02X} should be invalid BCS-N", byte);
            }

            /// A slice of only valid bytes should pass
            #[test]
            fn valid_only_slice_passes(bytes in prop::collection::vec(valid_bcs_n_byte(), 0..100)) {
                prop_assert!(validate_bcs_n(&bytes));
            }

            /// A slice with at least one invalid byte should fail
            #[test]
            fn invalid_byte_in_slice_fails(
                valid_bytes in prop::collection::vec(valid_bcs_n_byte(), 0..50),
                invalid_byte in invalid_bcs_n_byte(),
                insert_pos in 0usize..=50,
            ) {
                let mut bytes = valid_bytes;
                let pos = insert_pos.min(bytes.len());
                bytes.insert(pos, invalid_byte);
                prop_assert!(!validate_bcs_n(&bytes),
                    "Slice with invalid byte 0x{:02X} should fail", invalid_byte);
            }

            /// Detailed validation should report correct position of first invalid byte
            #[test]
            fn detailed_reports_first_invalid(
                prefix in prop::collection::vec(valid_bcs_n_byte(), 0..20),
                invalid_byte in invalid_bcs_n_byte(),
                suffix in prop::collection::vec(any::<u8>(), 0..20),
            ) {
                let mut bytes = prefix.clone();
                bytes.push(invalid_byte);
                bytes.extend(suffix);

                let result = validate_bcs_n_detailed(&bytes);
                prop_assert!(!result.valid);
                prop_assert_eq!(result.first_invalid_index, Some(prefix.len()));
                prop_assert_eq!(result.first_invalid_byte, Some(invalid_byte));
            }

            /// Numeric strings should be valid BCS-N
            #[test]
            fn numeric_strings_valid(n in 0u64..999999999) {
                let s = format!("{}", n);
                prop_assert!(validate_bcs_n(s.as_bytes()));
            }

            /// Padded numeric strings (with leading spaces) should be valid BCS-N
            #[test]
            fn padded_numeric_strings_valid(
                n in 0u64..999999,
                padding in 0usize..10,
            ) {
                let s = format!("{:>width$}", n, width = padding + format!("{}", n).len());
                prop_assert!(validate_bcs_n(s.as_bytes()));
            }

            /// Signed decimal strings should be valid BCS-N
            #[test]
            fn signed_decimal_strings_valid(
                sign in prop::sample::select(vec!["+", "-", ""]),
                integer_part in 0u32..9999,
                decimal_part in 0u32..9999,
            ) {
                let s = format!("{}{}.{}", sign, integer_part, decimal_part);
                prop_assert!(validate_bcs_n(s.as_bytes()),
                    "Signed decimal '{}' should be valid BCS-N", s);
            }
        }
    }
}
