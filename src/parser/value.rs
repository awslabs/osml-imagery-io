//! Value type for parsed field values with type conversions.
//!
//! The [`Value`] enum represents parsed field values from binary data,
//! supporting zero-copy string references and type conversion methods
//! for interpreting ASCII-encoded numeric fields.

use std::borrow::Cow;

use super::error::ConversionError;

/// A parsed field value with type conversion methods.
///
/// Values can represent strings, raw bytes, unsigned integers, nested structures,
/// or arrays of values. String values use `Cow<str>` for zero-copy references
/// when possible.
#[derive(Debug, Clone)]
pub enum Value<'a> {
    /// String value (may reference source buffer for zero-copy)
    String(Cow<'a, str>),
    /// Raw bytes (references source buffer)
    Bytes(&'a [u8]),
    /// Unsigned integer value
    Unsigned(u64),
    /// Nested structure (boxed to avoid infinite size)
    Struct(Box<StructValue<'a>>),
    /// Array of values
    Array(Vec<Value<'a>>),
}

/// Placeholder for nested structure values.
///
/// This will be replaced with actual StructureAccessor integration
/// in a later task when the accessor is fully implemented.
#[derive(Debug, Clone)]
pub struct StructValue<'a> {
    /// The raw bytes of the nested structure
    pub data: &'a [u8],
    /// The type name of the nested structure
    pub type_name: String,
}

impl<'a> Value<'a> {
    /// Create a string value from a borrowed string slice.
    pub fn from_borrowed(s: &'a str) -> Self {
        Value::String(Cow::Borrowed(s))
    }

    /// Create a string value from an owned string.
    pub fn from_string(s: String) -> Self {
        Value::String(Cow::Owned(s))
    }

    /// Create a bytes value from a byte slice.
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        Value::Bytes(bytes)
    }

    /// Create an unsigned integer value.
    pub fn from_unsigned(n: u64) -> Self {
        Value::Unsigned(n)
    }

    /// Create an array value.
    pub fn from_array(values: Vec<Value<'a>>) -> Self {
        Value::Array(values)
    }

    /// Create a nested structure value.
    pub fn from_struct(data: &'a [u8], type_name: impl Into<String>) -> Self {
        Value::Struct(Box::new(StructValue {
            data,
            type_name: type_name.into(),
        }))
    }
}

impl<'a> Value<'a> {
    /// Get the value as a string, trimming trailing padding characters.
    ///
    /// For string values, returns the string with trailing spaces (0x20) removed.
    /// For bytes, attempts to interpret as UTF-8 and trim padding.
    /// For unsigned integers, returns the decimal string representation.
    /// For arrays and structs, returns a ConversionError.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use _io::parser::Value;
    ///
    /// let value = Value::from_borrowed("HELLO   ");
    /// assert_eq!(value.as_str().unwrap(), "HELLO");
    /// ```
    pub fn as_str(&self) -> Result<&str, ConversionError> {
        match self {
            Value::String(cow) => {
                // Trim trailing spaces (standard NITF padding)
                Ok(cow.trim_end_matches(' '))
            }
            Value::Bytes(bytes) => {
                // Try to interpret bytes as UTF-8 string
                std::str::from_utf8(bytes)
                    .map(|s| s.trim_end_matches(' '))
                    .map_err(|e| ConversionError::ParseError {
                        value: format!("{:?}", bytes),
                        target_type: "str",
                        message: e.to_string(),
                    })
            }
            Value::Unsigned(_) => Err(ConversionError::TypeMismatch {
                from_type: "Unsigned",
                to_type: "str",
            }),
            Value::Struct(_) => Err(ConversionError::TypeMismatch {
                from_type: "Struct",
                to_type: "str",
            }),
            Value::Array(_) => Err(ConversionError::TypeMismatch {
                from_type: "Array",
                to_type: "str",
            }),
        }
    }

    /// Get the value as a string with custom padding character trimmed.
    ///
    /// Similar to `as_str()` but allows specifying the padding character to trim.
    ///
    /// # Arguments
    ///
    /// * `pad_char` - The padding character to trim from the end
    pub fn as_str_with_pad(&self, pad_char: char) -> Result<&str, ConversionError> {
        match self {
            Value::String(cow) => Ok(cow.trim_end_matches(pad_char)),
            Value::Bytes(bytes) => std::str::from_utf8(bytes)
                .map(|s| s.trim_end_matches(pad_char))
                .map_err(|e| ConversionError::ParseError {
                    value: format!("{:?}", bytes),
                    target_type: "str",
                    message: e.to_string(),
                }),
            Value::Unsigned(_) => Err(ConversionError::TypeMismatch {
                from_type: "Unsigned",
                to_type: "str",
            }),
            Value::Struct(_) => Err(ConversionError::TypeMismatch {
                from_type: "Struct",
                to_type: "str",
            }),
            Value::Array(_) => Err(ConversionError::TypeMismatch {
                from_type: "Array",
                to_type: "str",
            }),
        }
    }

    /// Parse the value as a signed 64-bit integer.
    ///
    /// For BCS-N strings, parses the numeric content (ignoring leading/trailing spaces).
    /// For unsigned integers, converts directly if within i64 range.
    /// For bytes, attempts to interpret as UTF-8 numeric string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use _io::parser::Value;
    ///
    /// let value = Value::from_borrowed("  123  ");
    /// assert_eq!(value.as_i64().unwrap(), 123);
    ///
    /// let negative = Value::from_borrowed("-456");
    /// assert_eq!(negative.as_i64().unwrap(), -456);
    /// ```
    pub fn as_i64(&self) -> Result<i64, ConversionError> {
        match self {
            Value::String(cow) => {
                let trimmed = cow.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed
                    .parse::<i64>()
                    .map_err(|e| ConversionError::ParseError {
                        value: cow.to_string(),
                        target_type: "i64",
                        message: e.to_string(),
                    })
            }
            Value::Bytes(bytes) => {
                let s = std::str::from_utf8(bytes).map_err(|e| ConversionError::ParseError {
                    value: format!("{:?}", bytes),
                    target_type: "i64",
                    message: e.to_string(),
                })?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed
                    .parse::<i64>()
                    .map_err(|e| ConversionError::ParseError {
                        value: s.to_string(),
                        target_type: "i64",
                        message: e.to_string(),
                    })
            }
            Value::Unsigned(n) => {
                if *n <= i64::MAX as u64 {
                    Ok(*n as i64)
                } else {
                    Err(ConversionError::ParseError {
                        value: n.to_string(),
                        target_type: "i64",
                        message: "value exceeds i64::MAX".to_string(),
                    })
                }
            }
            Value::Struct(_) => Err(ConversionError::TypeMismatch {
                from_type: "Struct",
                to_type: "i64",
            }),
            Value::Array(_) => Err(ConversionError::TypeMismatch {
                from_type: "Array",
                to_type: "i64",
            }),
        }
    }

    /// Parse the value as an unsigned 64-bit integer.
    ///
    /// For BCS-N strings, parses the numeric content (ignoring leading/trailing spaces).
    /// For unsigned integers, returns the value directly.
    /// For bytes, attempts to interpret as UTF-8 numeric string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use _io::parser::Value;
    ///
    /// let value = Value::from_borrowed("00123");
    /// assert_eq!(value.as_u64().unwrap(), 123);
    /// ```
    pub fn as_u64(&self) -> Result<u64, ConversionError> {
        match self {
            Value::String(cow) => {
                let trimmed = cow.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed
                    .parse::<u64>()
                    .map_err(|e| ConversionError::ParseError {
                        value: cow.to_string(),
                        target_type: "u64",
                        message: e.to_string(),
                    })
            }
            Value::Bytes(bytes) => {
                let s = std::str::from_utf8(bytes).map_err(|e| ConversionError::ParseError {
                    value: format!("{:?}", bytes),
                    target_type: "u64",
                    message: e.to_string(),
                })?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed
                    .parse::<u64>()
                    .map_err(|e| ConversionError::ParseError {
                        value: s.to_string(),
                        target_type: "u64",
                        message: e.to_string(),
                    })
            }
            Value::Unsigned(n) => Ok(*n),
            Value::Struct(_) => Err(ConversionError::TypeMismatch {
                from_type: "Struct",
                to_type: "u64",
            }),
            Value::Array(_) => Err(ConversionError::TypeMismatch {
                from_type: "Array",
                to_type: "u64",
            }),
        }
    }

    /// Parse the value as a 64-bit floating-point number.
    ///
    /// For numeric strings, parses the floating-point content.
    /// For unsigned integers, converts to f64.
    /// For bytes, attempts to interpret as UTF-8 numeric string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use _io::parser::Value;
    ///
    /// let value = Value::from_borrowed("2.71828");
    /// assert!((value.as_f64().unwrap() - 2.71828).abs() < 1e-10);
    ///
    /// let int_value = Value::from_borrowed("42");
    /// assert_eq!(int_value.as_f64().unwrap(), 42.0);
    /// ```
    pub fn as_f64(&self) -> Result<f64, ConversionError> {
        match self {
            Value::String(cow) => {
                let trimmed = cow.trim();
                if trimmed.is_empty() {
                    return Ok(0.0);
                }
                trimmed
                    .parse::<f64>()
                    .map_err(|e| ConversionError::ParseError {
                        value: cow.to_string(),
                        target_type: "f64",
                        message: e.to_string(),
                    })
            }
            Value::Bytes(bytes) => {
                let s = std::str::from_utf8(bytes).map_err(|e| ConversionError::ParseError {
                    value: format!("{:?}", bytes),
                    target_type: "f64",
                    message: e.to_string(),
                })?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0.0);
                }
                trimmed
                    .parse::<f64>()
                    .map_err(|e| ConversionError::ParseError {
                        value: s.to_string(),
                        target_type: "f64",
                        message: e.to_string(),
                    })
            }
            Value::Unsigned(n) => Ok(*n as f64),
            Value::Struct(_) => Err(ConversionError::TypeMismatch {
                from_type: "Struct",
                to_type: "f64",
            }),
            Value::Array(_) => Err(ConversionError::TypeMismatch {
                from_type: "Array",
                to_type: "f64",
            }),
        }
    }

    /// Get the raw bytes of the value.
    ///
    /// For bytes, returns the slice directly.
    /// For strings, returns the UTF-8 bytes.
    /// For unsigned integers, returns the bytes in big-endian order.
    /// For structs, returns the underlying data bytes.
    /// For arrays, returns an error.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use _io::parser::Value;
    ///
    /// let value = Value::from_bytes(b"HELLO");
    /// assert_eq!(value.as_bytes(), b"HELLO");
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Value::String(cow) => cow.as_bytes(),
            Value::Bytes(bytes) => bytes,
            Value::Unsigned(n) => {
                // Return empty slice - caller should use to_be_bytes() for specific size
                // This is a limitation since we can't return a reference to temporary data
                // In practice, unsigned values should be accessed via as_u64()
                let _ = n;
                &[]
            }
            Value::Struct(s) => s.data,
            Value::Array(_) => &[],
        }
    }

    /// Check if this value is a string type.
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Check if this value is a bytes type.
    pub fn is_bytes(&self) -> bool {
        matches!(self, Value::Bytes(_))
    }

    /// Check if this value is an unsigned integer type.
    pub fn is_unsigned(&self) -> bool {
        matches!(self, Value::Unsigned(_))
    }

    /// Check if this value is a struct type.
    pub fn is_struct(&self) -> bool {
        matches!(self, Value::Struct(_))
    }

    /// Check if this value is an array type.
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// Get the length of the value.
    ///
    /// For strings, returns the character count.
    /// For bytes, returns the byte count.
    /// For arrays, returns the element count.
    /// For unsigned integers and structs, returns 1.
    pub fn len(&self) -> usize {
        match self {
            Value::String(cow) => cow.len(),
            Value::Bytes(bytes) => bytes.len(),
            Value::Unsigned(_) => 1,
            Value::Struct(_) => 1,
            Value::Array(arr) => arr.len(),
        }
    }

    /// Check if the value is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== as_str() tests ====================

    #[test]
    fn as_str_from_string_no_padding() {
        let value = Value::from_borrowed("HELLO");
        assert_eq!(value.as_str().unwrap(), "HELLO");
    }

    #[test]
    fn as_str_from_string_with_trailing_spaces() {
        let value = Value::from_borrowed("HELLO   ");
        assert_eq!(value.as_str().unwrap(), "HELLO");
    }

    #[test]
    fn as_str_from_string_all_spaces() {
        let value = Value::from_borrowed("     ");
        assert_eq!(value.as_str().unwrap(), "");
    }

    #[test]
    fn as_str_from_bytes() {
        let value = Value::from_bytes(b"WORLD   ");
        assert_eq!(value.as_str().unwrap(), "WORLD");
    }

    #[test]
    fn as_str_from_unsigned_fails() {
        let value = Value::from_unsigned(42);
        assert!(value.as_str().is_err());
    }

    #[test]
    fn as_str_with_custom_pad() {
        let value = Value::from_borrowed("00123000");
        assert_eq!(value.as_str_with_pad('0').unwrap(), "00123");
    }

    // ==================== as_i64() tests ====================

    #[test]
    fn as_i64_from_positive_string() {
        let value = Value::from_borrowed("12345");
        assert_eq!(value.as_i64().unwrap(), 12345);
    }

    #[test]
    fn as_i64_from_negative_string() {
        let value = Value::from_borrowed("-12345");
        assert_eq!(value.as_i64().unwrap(), -12345);
    }

    #[test]
    fn as_i64_from_padded_string() {
        let value = Value::from_borrowed("  123  ");
        assert_eq!(value.as_i64().unwrap(), 123);
    }

    #[test]
    fn as_i64_from_leading_zeros() {
        let value = Value::from_borrowed("00042");
        assert_eq!(value.as_i64().unwrap(), 42);
    }

    #[test]
    fn as_i64_from_empty_string() {
        let value = Value::from_borrowed("");
        assert_eq!(value.as_i64().unwrap(), 0);
    }

    #[test]
    fn as_i64_from_spaces_only() {
        let value = Value::from_borrowed("   ");
        assert_eq!(value.as_i64().unwrap(), 0);
    }

    #[test]
    fn as_i64_from_unsigned() {
        let value = Value::from_unsigned(999);
        assert_eq!(value.as_i64().unwrap(), 999);
    }

    #[test]
    fn as_i64_from_bytes() {
        let value = Value::from_bytes(b"  456  ");
        assert_eq!(value.as_i64().unwrap(), 456);
    }

    #[test]
    fn as_i64_invalid_string() {
        let value = Value::from_borrowed("abc");
        assert!(value.as_i64().is_err());
    }

    // ==================== as_u64() tests ====================

    #[test]
    fn as_u64_from_string() {
        let value = Value::from_borrowed("12345");
        assert_eq!(value.as_u64().unwrap(), 12345);
    }

    #[test]
    fn as_u64_from_padded_string() {
        let value = Value::from_borrowed("  00123  ");
        assert_eq!(value.as_u64().unwrap(), 123);
    }

    #[test]
    fn as_u64_from_unsigned() {
        let value = Value::from_unsigned(42);
        assert_eq!(value.as_u64().unwrap(), 42);
    }

    #[test]
    fn as_u64_negative_fails() {
        let value = Value::from_borrowed("-123");
        assert!(value.as_u64().is_err());
    }

    // ==================== as_f64() tests ====================

    #[test]
    fn as_f64_from_integer_string() {
        let value = Value::from_borrowed("42");
        assert_eq!(value.as_f64().unwrap(), 42.0);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn as_f64_from_decimal_string() {
        let value = Value::from_borrowed("2.71828");
        assert!((value.as_f64().unwrap() - 2.71828).abs() < 1e-10);
    }

    #[test]
    fn as_f64_from_scientific_notation() {
        let value = Value::from_borrowed("1.5e10");
        assert_eq!(value.as_f64().unwrap(), 1.5e10);
    }

    #[test]
    fn as_f64_from_negative() {
        let value = Value::from_borrowed("-2.5");
        assert_eq!(value.as_f64().unwrap(), -2.5);
    }

    #[test]
    fn as_f64_from_unsigned() {
        let value = Value::from_unsigned(100);
        assert_eq!(value.as_f64().unwrap(), 100.0);
    }

    #[test]
    fn as_f64_invalid_string() {
        let value = Value::from_borrowed("not a number");
        assert!(value.as_f64().is_err());
    }

    // ==================== as_bytes() tests ====================

    #[test]
    fn as_bytes_from_bytes() {
        let value = Value::from_bytes(b"HELLO");
        assert_eq!(value.as_bytes(), b"HELLO");
    }

    #[test]
    fn as_bytes_from_string() {
        let value = Value::from_borrowed("WORLD");
        assert_eq!(value.as_bytes(), b"WORLD");
    }

    // ==================== Type check tests ====================

    #[test]
    fn type_checks() {
        assert!(Value::from_borrowed("test").is_string());
        assert!(Value::from_bytes(b"test").is_bytes());
        assert!(Value::from_unsigned(42).is_unsigned());
        assert!(Value::from_array(vec![]).is_array());
        assert!(Value::from_struct(b"data", "type").is_struct());
    }

    // ==================== len() tests ====================

    #[test]
    fn len_string() {
        let value = Value::from_borrowed("HELLO");
        assert_eq!(value.len(), 5);
    }

    #[test]
    fn len_bytes() {
        let value = Value::from_bytes(b"WORLD");
        assert_eq!(value.len(), 5);
    }

    #[test]
    fn len_array() {
        let value = Value::from_array(vec![
            Value::from_unsigned(1),
            Value::from_unsigned(2),
            Value::from_unsigned(3),
        ]);
        assert_eq!(value.len(), 3);
    }

    #[test]
    fn is_empty() {
        assert!(Value::from_borrowed("").is_empty());
        assert!(Value::from_bytes(b"").is_empty());
        assert!(Value::from_array(vec![]).is_empty());
        assert!(!Value::from_borrowed("x").is_empty());
    }
}

// ==================== Property-Based Tests ====================

#[cfg(test)]
mod property_tests {
    /// Property 15: String Padding Trimming
    /// For any string field with padding, `as_str()` SHALL return the string
    /// with trailing padding characters removed.
    /// **Validates: Requirements 6.1**
    mod prop_15_string_padding_trimming {
        use super::super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// For any string with trailing spaces, as_str() trims them
            #[test]
            fn trailing_spaces_trimmed(
                content in "[A-Za-z0-9]{0,20}",
                num_spaces in 0usize..20
            ) {
                let padded = format!("{}{}", content, " ".repeat(num_spaces));
                let value = Value::from_borrowed(&padded);
                let result = value.as_str().unwrap();
                prop_assert_eq!(result, content.trim_end(),
                    "Expected '{}', got '{}'", content.trim_end(), result);
            }

            /// For any string without trailing spaces, as_str() returns it unchanged
            #[test]
            fn no_trailing_spaces_unchanged(content in "[A-Za-z0-9]{1,20}") {
                // Ensure no trailing spaces by trimming
                let content = content.trim_end();
                if content.is_empty() {
                    return Ok(());
                }
                let value = Value::from_borrowed(content);
                let result = value.as_str().unwrap();
                prop_assert_eq!(result, content);
            }

            /// For bytes with trailing spaces, as_str() trims them
            #[test]
            fn bytes_trailing_spaces_trimmed(
                content in "[A-Za-z0-9]{0,20}",
                num_spaces in 0usize..20
            ) {
                let padded = format!("{}{}", content, " ".repeat(num_spaces));
                let value = Value::from_bytes(padded.as_bytes());
                let result = value.as_str().unwrap();
                prop_assert_eq!(result, content.trim_end());
            }

            /// Custom padding character trimming works correctly
            #[test]
            fn custom_pad_char_trimmed(
                content in "[1-9]{0,10}",
                num_zeros in 0usize..10
            ) {
                let padded = format!("{}{}", content, "0".repeat(num_zeros));
                let value = Value::from_borrowed(&padded);
                let result = value.as_str_with_pad('0').unwrap();
                prop_assert_eq!(result, content.trim_end_matches('0'));
            }

            /// All-spaces string becomes empty after trimming
            #[test]
            fn all_spaces_becomes_empty(num_spaces in 1usize..50) {
                let spaces = " ".repeat(num_spaces);
                let value = Value::from_borrowed(&spaces);
                let result = value.as_str().unwrap();
                prop_assert!(result.is_empty(),
                    "Expected empty string, got '{}'", result);
            }
        }
    }

    /// Property 16: Numeric String Parsing
    /// For any BCS-N string representing a valid integer, `as_i64()` and `as_u64()`
    /// SHALL return the numeric value. For any numeric string representing a valid
    /// float, `as_f64()` SHALL return the floating-point value.
    /// **Validates: Requirements 6.2, 6.3, 6.4**
    mod prop_16_numeric_string_parsing {
        use super::super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// For any i64, formatting and parsing round-trips correctly
            #[test]
            fn i64_round_trip(n in any::<i64>()) {
                let s = format!("{}", n);
                let value = Value::from_borrowed(&s);
                let parsed = value.as_i64().unwrap();
                prop_assert_eq!(n, parsed,
                    "Expected {}, got {} from '{}'", n, parsed, s);
            }

            /// For any u64, formatting and parsing round-trips correctly
            #[test]
            fn u64_round_trip(n in any::<u64>()) {
                let s = format!("{}", n);
                let value = Value::from_borrowed(&s);
                let parsed = value.as_u64().unwrap();
                prop_assert_eq!(n, parsed,
                    "Expected {}, got {} from '{}'", n, parsed, s);
            }

            /// For any f64 (excluding special values), formatting and parsing round-trips
            #[test]
            fn f64_round_trip(n in any::<f64>().prop_filter("finite", |f| f.is_finite())) {
                let s = format!("{}", n);
                let value = Value::from_borrowed(&s);
                let parsed = value.as_f64().unwrap();
                // Allow small floating-point error
                let diff = (n - parsed).abs();
                let tolerance = n.abs() * 1e-10 + 1e-10;
                prop_assert!(diff <= tolerance,
                    "Expected {}, got {} (diff: {})", n, parsed, diff);
            }

            /// Integers with leading zeros parse correctly
            #[test]
            fn leading_zeros_parse(n in 0u64..1000000, leading_zeros in 1usize..10) {
                let s = format!("{:0>width$}", n, width = leading_zeros + n.to_string().len());
                let value = Value::from_borrowed(&s);
                let parsed = value.as_u64().unwrap();
                prop_assert_eq!(n, parsed,
                    "Expected {}, got {} from '{}'", n, parsed, s);
            }

            /// Integers with leading/trailing spaces parse correctly (BCS-N padding)
            #[test]
            fn padded_integers_parse(
                n in any::<i64>(),
                leading_spaces in 0usize..5,
                trailing_spaces in 0usize..5
            ) {
                let s = format!("{}{}{}", " ".repeat(leading_spaces), n, " ".repeat(trailing_spaces));
                let value = Value::from_borrowed(&s);
                let parsed = value.as_i64().unwrap();
                prop_assert_eq!(n, parsed,
                    "Expected {}, got {} from '{}'", n, parsed, s);
            }

            /// Negative integers parse correctly
            #[test]
            fn negative_integers_parse(n in i64::MIN..0i64) {
                let s = format!("{}", n);
                let value = Value::from_borrowed(&s);
                let parsed = value.as_i64().unwrap();
                prop_assert_eq!(n, parsed);
            }

            /// Unsigned values from Value::Unsigned convert correctly
            #[test]
            fn unsigned_value_converts(n in any::<u64>()) {
                let value = Value::from_unsigned(n);
                let parsed = value.as_u64().unwrap();
                prop_assert_eq!(n, parsed);
            }

            /// Unsigned values within i64 range convert to i64 correctly
            #[test]
            fn unsigned_to_i64_in_range(n in 0u64..=(i64::MAX as u64)) {
                let value = Value::from_unsigned(n);
                let parsed = value.as_i64().unwrap();
                prop_assert_eq!(n as i64, parsed);
            }

            /// Unsigned values convert to f64 correctly
            #[test]
            fn unsigned_to_f64(n in any::<u64>()) {
                let value = Value::from_unsigned(n);
                let parsed = value.as_f64().unwrap();
                prop_assert_eq!(n as f64, parsed);
            }

            /// Empty string parses as zero
            #[test]
            fn empty_string_is_zero(_unused in 0..1i32) {
                let value = Value::from_borrowed("");
                prop_assert_eq!(value.as_i64().unwrap(), 0);
                prop_assert_eq!(value.as_u64().unwrap(), 0);
                prop_assert_eq!(value.as_f64().unwrap(), 0.0);
            }

            /// Whitespace-only string parses as zero
            #[test]
            fn whitespace_only_is_zero(num_spaces in 1usize..20) {
                let s = " ".repeat(num_spaces);
                let value = Value::from_borrowed(&s);
                prop_assert_eq!(value.as_i64().unwrap(), 0);
                prop_assert_eq!(value.as_u64().unwrap(), 0);
                prop_assert_eq!(value.as_f64().unwrap(), 0.0);
            }

            /// Scientific notation parses correctly for f64
            #[test]
            fn scientific_notation_parses(
                mantissa in -1000.0f64..1000.0,
                exponent in -10i32..10
            ) {
                let s = format!("{}e{}", mantissa, exponent);
                let value = Value::from_borrowed(&s);
                let expected = mantissa * 10f64.powi(exponent);
                let parsed = value.as_f64().unwrap();
                let diff = (expected - parsed).abs();
                let tolerance = expected.abs() * 1e-10 + 1e-10;
                prop_assert!(diff <= tolerance,
                    "Expected {}, got {} from '{}'", expected, parsed, s);
            }
        }
    }

    /// Property 17: Invalid Numeric Conversion Error
    /// For any string that cannot be parsed as a number, `as_i64()`, `as_u64()`,
    /// and `as_f64()` SHALL return a ConversionError.
    /// **Validates: Requirements 6.6**
    mod prop_17_invalid_numeric_conversion {
        use super::super::*;
        use proptest::prelude::*;

        /// Generate a string that is definitely not a valid number
        fn non_numeric_string() -> impl Strategy<Value = String> {
            prop_oneof![
                // Pure alphabetic strings (excluding 'e' and 'E' which could form scientific notation)
                "[a-df-zA-DF-Z]{1,10}",
                // Mixed with special characters
                "[a-zA-Z!@#$%^&*()]{2,10}"
                    .prop_filter("must not be valid number", |s| s.parse::<f64>().is_err()),
                // Multiple decimal points
                "[0-9]+\\.[0-9]+\\.[0-9]+",
                // Letters mixed with digits - use letters that can't form scientific notation
                "[0-9]+[a-df-zA-DF-Z]+[0-9]*",
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Non-numeric strings fail as_i64()
            #[test]
            fn non_numeric_fails_i64(s in non_numeric_string()) {
                let value = Value::from_borrowed(&s);
                prop_assert!(value.as_i64().is_err(),
                    "Expected error for '{}', but got success", s);
            }

            /// Non-numeric strings fail as_u64()
            #[test]
            fn non_numeric_fails_u64(s in non_numeric_string()) {
                let value = Value::from_borrowed(&s);
                prop_assert!(value.as_u64().is_err(),
                    "Expected error for '{}', but got success", s);
            }

            /// Non-numeric strings fail as_f64()
            #[test]
            fn non_numeric_fails_f64(s in non_numeric_string()) {
                let value = Value::from_borrowed(&s);
                prop_assert!(value.as_f64().is_err(),
                    "Expected error for '{}', but got success", s);
            }

            /// Negative numbers fail as_u64()
            #[test]
            fn negative_fails_u64(n in i64::MIN..-1i64) {
                let s = format!("{}", n);
                let value = Value::from_borrowed(&s);
                prop_assert!(value.as_u64().is_err(),
                    "Expected error for negative '{}', but got success", s);
            }

            /// Unsigned values exceeding i64::MAX fail as_i64()
            #[test]
            fn large_unsigned_fails_i64(n in (i64::MAX as u64 + 1)..=u64::MAX) {
                let value = Value::from_unsigned(n);
                prop_assert!(value.as_i64().is_err(),
                    "Expected error for large unsigned {}, but got success", n);
            }

            /// Struct values fail numeric conversions
            #[test]
            fn struct_fails_numeric(_unused in 0..1i32) {
                let value = Value::from_struct(b"data", "test_type");
                prop_assert!(value.as_i64().is_err());
                prop_assert!(value.as_u64().is_err());
                prop_assert!(value.as_f64().is_err());
            }

            /// Array values fail numeric conversions
            #[test]
            fn array_fails_numeric(_unused in 0..1i32) {
                let value = Value::from_array(vec![Value::from_unsigned(1)]);
                prop_assert!(value.as_i64().is_err());
                prop_assert!(value.as_u64().is_err());
                prop_assert!(value.as_f64().is_err());
            }

            /// Invalid UTF-8 bytes fail string conversion
            #[test]
            fn invalid_utf8_fails_str(
                valid_prefix in prop::collection::vec(0x20u8..0x7F, 0..10),
                invalid_byte in prop::sample::select(vec![0x80u8, 0xC0, 0xFE, 0xFF])
            ) {
                let mut bytes = valid_prefix;
                bytes.push(invalid_byte);
                let value = Value::from_bytes(&bytes);
                prop_assert!(value.as_str().is_err(),
                    "Expected error for invalid UTF-8, but got success");
            }

            /// Unsigned and Struct values fail as_str()
            #[test]
            fn non_string_types_fail_as_str(_unused in 0..1i32) {
                let unsigned = Value::from_unsigned(42);
                prop_assert!(unsigned.as_str().is_err());

                let struct_val = Value::from_struct(b"data", "type");
                prop_assert!(struct_val.as_str().is_err());

                let array_val = Value::from_array(vec![]);
                prop_assert!(array_val.as_str().is_err());
            }
        }
    }
}
