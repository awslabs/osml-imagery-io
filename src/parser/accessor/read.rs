//! Value reading from binary data.
//!
//! This module handles reading and parsing field values from binary buffers,
//! including integer decoding with endianness support.

use std::borrow::Cow;

use crate::parser::error::AccessError;
use crate::parser::types::{Endian, FieldDefinition, FieldType};
use crate::parser::value::Value;

/// Read an unsigned integer from bytes with specified endianness.
pub fn read_unsigned(bytes: &[u8], size: u8, endian: Endian) -> Result<u64, AccessError> {
    match (size, endian) {
        (1, _) => Ok(bytes[0] as u64),
        (2, Endian::Big) => Ok(u16::from_be_bytes([bytes[0], bytes[1]]) as u64),
        (2, Endian::Little) => Ok(u16::from_le_bytes([bytes[0], bytes[1]]) as u64),
        (4, Endian::Big) => {
            Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64)
        }
        (4, Endian::Little) => {
            Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64)
        }
        (8, Endian::Big) => Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        (8, Endian::Little) => Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        _ => Err(AccessError::UnknownField {
            path: format!("unsupported integer size: {}", size),
        }),
    }
}

/// Read a signed integer from bytes with specified endianness.
pub fn read_signed(bytes: &[u8], size: u8, endian: Endian) -> Result<i64, AccessError> {
    match (size, endian) {
        (1, _) => Ok(bytes[0] as i8 as i64),
        (2, Endian::Big) => Ok(i16::from_be_bytes([bytes[0], bytes[1]]) as i64),
        (2, Endian::Little) => Ok(i16::from_le_bytes([bytes[0], bytes[1]]) as i64),
        (4, Endian::Big) => {
            Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64)
        }
        (4, Endian::Little) => {
            Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64)
        }
        (8, Endian::Big) => Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        (8, Endian::Little) => Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])),
        _ => Err(AccessError::UnknownField {
            path: format!("unsupported integer size: {}", size),
        }),
    }
}

/// Read a field value from bytes based on field definition.
pub fn read_field_value_from_bytes<'a>(
    field: &FieldDefinition,
    bytes: &'a [u8],
    endian: Endian,
) -> Result<Value<'a>, AccessError> {
    match &field.field_type {
        FieldType::String => {
            // Validate encoding if specified
            if let Some(encoding) = field.encoding {
                if !encoding.validate(bytes) {
                    return Err(AccessError::EncodingError {
                        path: field.id.clone(),
                        encoding: format!("{:?}", encoding),
                        message: "Invalid characters for encoding".to_string(),
                    });
                }
            }

            // Convert to string
            let s = std::str::from_utf8(bytes).map_err(|e| AccessError::EncodingError {
                path: field.id.clone(),
                encoding: "UTF-8".to_string(),
                message: e.to_string(),
            })?;

            Ok(Value::String(Cow::Borrowed(s)))
        }
        FieldType::Bytes => Ok(Value::Bytes(bytes)),
        FieldType::UnsignedInt(byte_size) => {
            let value = read_unsigned(bytes, *byte_size, endian)?;
            Ok(Value::Unsigned(value))
        }
        FieldType::SignedInt(byte_size) => {
            let value = read_signed(bytes, *byte_size, endian)?;
            // Store as unsigned but preserve sign through conversion
            Ok(Value::Unsigned(value as u64))
        }
        FieldType::TypeRef(type_name) => {
            // Create nested structure value
            Ok(Value::from_struct(bytes, type_name.clone()))
        }
    }
}
