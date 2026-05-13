//! Value encoding for the structure writer.
//!
//! This module handles encoding various value types (strings, bytes, integers)
//! into binary format according to field definitions.

use crate::parser::error::WriteError;
use crate::parser::types::{Encoding, Endian, FieldDefinition, FieldType};

use super::integer::{encode_signed, encode_unsigned};
use super::validation::{get_pad_char, validate_encoding};
use super::WriteValue;

/// Encode a value according to field definition.
pub fn encode_value(
    value: &WriteValue,
    field: &FieldDefinition,
    size: usize,
    endian: Endian,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    match (&field.field_type, value) {
        (FieldType::String, WriteValue::String(s)) => encode_string(s, field, size, path),
        (FieldType::String, WriteValue::Integer(n)) => {
            // Convert integer to string
            let s = n.to_string();
            encode_string(&s, field, size, path)
        }
        (FieldType::String, WriteValue::Unsigned(n)) => {
            // Convert unsigned to string
            let s = n.to_string();
            encode_string(&s, field, size, path)
        }
        (FieldType::String, WriteValue::Float(f)) => {
            // Convert float to string
            let s = f.to_string();
            encode_string(&s, field, size, path)
        }
        (FieldType::Bytes, WriteValue::Bytes(bytes)) => encode_bytes(bytes, size, path),
        (FieldType::Bytes, WriteValue::String(s)) => encode_bytes(s.as_bytes(), size, path),
        (FieldType::UnsignedInt(byte_size), WriteValue::Unsigned(n)) => {
            encode_unsigned(*n, *byte_size, endian, path)
        }
        (FieldType::UnsignedInt(byte_size), WriteValue::Integer(n)) => {
            if *n < 0 {
                return Err(WriteError::ValidationError {
                    path: path.to_string(),
                    message: "Cannot write negative value to unsigned field".to_string(),
                });
            }
            encode_unsigned(*n as u64, *byte_size, endian, path)
        }
        (FieldType::SignedInt(byte_size), WriteValue::Integer(n)) => {
            encode_signed(*n, *byte_size, endian, path)
        }
        (FieldType::SignedInt(byte_size), WriteValue::Unsigned(n)) => {
            if *n > i64::MAX as u64 {
                return Err(WriteError::ValidationError {
                    path: path.to_string(),
                    message: "Value too large for signed field".to_string(),
                });
            }
            encode_signed(*n as i64, *byte_size, endian, path)
        }
        // TypeRef fields accept raw bytes (pre-serialized nested structures).
        // The bytes are already the correct size from the sub-writer, so we
        // return them directly without size validation or padding.
        (FieldType::TypeRef(_), WriteValue::Bytes(bytes)) => Ok(bytes.clone()),
        _ => Err(WriteError::ConversionError {
            path: path.to_string(),
            message: format!(
                "Cannot convert {:?} to {:?}",
                std::mem::discriminant(value),
                field.field_type
            ),
        }),
    }
}

/// Encode a string value with padding and validation.
///
/// For BCS-N and BCS-NPI fields, values that are too short are left-padded
/// with zeros (numeric convention), and values that are too long are
/// auto-formatted by parsing as a number and reformatting to fit. For other
/// encodings, short values are right-padded with spaces.
pub fn encode_string(
    s: &str,
    field: &FieldDefinition,
    size: usize,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    let is_numeric = matches!(field.encoding, Some(Encoding::BcsN | Encoding::BcsNPI));

    let formatted: String;
    let value = if is_numeric {
        formatted = format_numeric_to_fit(s, size, field.encoding.unwrap(), path)?;
        &formatted
    } else {
        s
    };

    let bytes = value.as_bytes();

    if bytes.len() > size {
        return Err(WriteError::ValueTooLarge {
            path: path.to_string(),
            max_size: size,
            actual_size: bytes.len(),
        });
    }

    // Validate encoding if specified
    if let Some(encoding) = field.encoding {
        validate_encoding(bytes, encoding, path)?;
    }

    // Pad: numeric fields left-pad with '0', text fields right-pad with ' '
    let mut result = vec![get_pad_char(field); size];
    if is_numeric {
        let offset = size - bytes.len();
        result[offset..].copy_from_slice(bytes);
    } else {
        result[..bytes.len()].copy_from_slice(bytes);
    }

    Ok(result)
}

/// Format a numeric string to fit a fixed-width field.
///
/// Handles three cases:
/// 1. Value already fits → returned as-is
/// 2. Value is too long → parse as number, reformat to fit the field width
/// 3. Value cannot fit even after reformatting → returns ValueTooLarge error
fn format_numeric_to_fit(
    s: &str,
    size: usize,
    encoding: Encoding,
    path: &str,
) -> Result<String, WriteError> {
    if s.len() <= size {
        return Ok(s.to_string());
    }

    // For BCS-NPI (positive integers only), parse as integer
    if encoding == Encoding::BcsNPI {
        let n: u64 = s.trim().parse().map_err(|_| WriteError::ValidationError {
            path: path.to_string(),
            message: format!(
                "Value '{}' is {} bytes (field is {} bytes) and cannot be parsed as integer for auto-formatting",
                s, s.len(), size
            ),
        })?;
        let formatted = format!("{:0>width$}", n, width = size);
        if formatted.len() > size {
            return Err(WriteError::ValueTooLarge {
                path: path.to_string(),
                max_size: size,
                actual_size: formatted.len(),
            });
        }
        return Ok(formatted);
    }

    // BCS-N: may contain sign, decimal point, digits
    let trimmed = s.trim();

    // Detect sign prefix
    let (sign, magnitude) = if trimmed.starts_with('+') || trimmed.starts_with('-') {
        (&trimmed[..1], &trimmed[1..])
    } else {
        ("", trimmed)
    };

    // Parse as float if it contains a decimal point, otherwise as integer
    if magnitude.contains('.') {
        let val: f64 = trimmed.parse().map_err(|_| WriteError::ValidationError {
            path: path.to_string(),
            message: format!(
                "Value '{}' is {} bytes (field is {} bytes) and cannot be parsed as number for auto-formatting",
                s, s.len(), size
            ),
        })?;

        // Determine decimal places: available width minus sign and integer digits and dot
        let int_part = val.abs().trunc() as u64;
        let int_digits = if int_part == 0 {
            1
        } else {
            int_part.ilog10() as usize + 1
        };
        let sign_len = sign.len();
        // field_width = sign + int_digits + '.' + decimal_digits
        let available_for_decimals = size.saturating_sub(sign_len + int_digits + 1);

        if available_for_decimals == 0 {
            // No room for decimals — try formatting as integer
            let formatted = if sign.is_empty() {
                format!("{:0>width$}", int_part, width = size)
            } else {
                format!("{}{:0>width$}", sign, int_part, width = size - sign_len)
            };
            if formatted.len() > size {
                return Err(WriteError::ValueTooLarge {
                    path: path.to_string(),
                    max_size: size,
                    actual_size: s.len(),
                });
            }
            return Ok(formatted);
        }

        let formatted = if sign == "+" || sign == "-" {
            format!(
                "{}{:0>width$.prec$}",
                sign,
                val.abs(),
                width = size - sign_len,
                prec = available_for_decimals
            )
        } else {
            format!(
                "{:0>width$.prec$}",
                val,
                width = size,
                prec = available_for_decimals
            )
        };

        if formatted.len() > size {
            return Err(WriteError::ValueTooLarge {
                path: path.to_string(),
                max_size: size,
                actual_size: s.len(),
            });
        }
        Ok(formatted)
    } else {
        // Integer (no decimal point) in a BCS-N field
        let n: i64 = trimmed.parse().map_err(|_| WriteError::ValidationError {
            path: path.to_string(),
            message: format!(
                "Value '{}' is {} bytes (field is {} bytes) and cannot be parsed as number for auto-formatting",
                s, s.len(), size
            ),
        })?;
        let formatted = if n < 0 {
            format!("-{:0>width$}", n.unsigned_abs(), width = size - 1)
        } else if sign == "+" {
            format!("+{:0>width$}", n as u64, width = size - 1)
        } else {
            format!("{:0>width$}", n, width = size)
        };
        if formatted.len() > size {
            return Err(WriteError::ValueTooLarge {
                path: path.to_string(),
                max_size: size,
                actual_size: s.len(),
            });
        }
        Ok(formatted)
    }
}

/// Encode raw bytes with size validation.
pub fn encode_bytes(bytes: &[u8], size: usize, path: &str) -> Result<Vec<u8>, WriteError> {
    if bytes.len() > size {
        return Err(WriteError::ValueTooLarge {
            path: path.to_string(),
            max_size: size,
            actual_size: bytes.len(),
        });
    }

    // Pad with zeros if needed
    let mut result = vec![0u8; size];
    result[..bytes.len()].copy_from_slice(bytes);

    Ok(result)
}
