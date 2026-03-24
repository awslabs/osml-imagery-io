//! Value encoding for the structure writer.
//!
//! This module handles encoding various value types (strings, bytes, integers)
//! into binary format according to field definitions.

use crate::parser::error::WriteError;
use crate::parser::types::{Endian, FieldDefinition, FieldType};

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
pub fn encode_string(
    s: &str,
    field: &FieldDefinition,
    size: usize,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    let bytes = s.as_bytes();

    // Check size constraint
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

    // Create output buffer with padding
    let mut result = vec![get_pad_char(field); size];
    result[..bytes.len()].copy_from_slice(bytes);

    Ok(result)
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
