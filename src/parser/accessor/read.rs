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
        (4, Endian::Big) => Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64),
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
        (4, Endian::Big) => Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64),
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

/// Read an IEEE 754 float from bytes with specified endianness.
///
/// IEEE 754 defines the bit layout of the value; the byte serialization order
/// is governed by the containing format. Like [`read_unsigned`], this honors
/// the structure's declared endianness so float fields behave identically to
/// integer fields (NITF/BIIF mandates big-endian, but the parser is generic).
pub fn read_float(bytes: &[u8], size: u8, endian: Endian) -> Result<f64, AccessError> {
    match (size, endian) {
        (4, Endian::Big) => {
            let bits = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(f32::from_bits(bits) as f64)
        }
        (4, Endian::Little) => {
            let bits = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(f32::from_bits(bits) as f64)
        }
        (8, Endian::Big) => {
            let bits = u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok(f64::from_bits(bits))
        }
        (8, Endian::Little) => {
            let bits = u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok(f64::from_bits(bits))
        }
        _ => Err(AccessError::UnknownField {
            path: format!("unsupported float size: {}", size),
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
            // Convert to string (encoding metadata is advisory for reading;
            // real-world producers frequently deviate from spec encodings)
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
        FieldType::Float(byte_size) => {
            if bytes.len() < *byte_size as usize {
                return Err(AccessError::UnexpectedEof {
                    path: field.id.clone(),
                    expected: *byte_size as usize,
                    available: bytes.len(),
                });
            }
            let value = read_float(bytes, *byte_size, endian)?;
            Ok(Value::Float(value))
        }
        FieldType::TypeRef(type_name) => {
            // Create nested structure value
            Ok(Value::from_struct(bytes, type_name.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::SizeSpec;

    fn float_field(id: &str, size: u8) -> FieldDefinition {
        FieldDefinition::new(id, FieldType::Float(size)).with_size(SizeSpec::fixed(size as usize))
    }

    #[test]
    fn read_float_f4_big_endian() {
        // 1.0 == 0x3F800000
        let bytes = [0x3F, 0x80, 0x00, 0x00];
        assert_eq!(read_float(&bytes, 4, Endian::Big).unwrap(), 1.0);
    }

    #[test]
    fn read_float_f4_little_endian() {
        // Same value, byte-reversed
        let bytes = [0x00, 0x00, 0x80, 0x3F];
        assert_eq!(read_float(&bytes, 4, Endian::Little).unwrap(), 1.0);
    }

    #[test]
    fn read_float_f8_round_trips_both_endians() {
        let v = std::f64::consts::PI;
        assert_eq!(read_float(&v.to_be_bytes(), 8, Endian::Big).unwrap(), v);
        assert_eq!(read_float(&v.to_le_bytes(), 8, Endian::Little).unwrap(), v);
    }

    #[test]
    fn read_field_value_float_honors_structure_endian() {
        // Identical bytes, opposite declared endianness -> different values.
        let field = float_field("SCALE", 4);
        let be_bytes = 2.5f32.to_be_bytes();
        let value_be = read_field_value_from_bytes(&field, &be_bytes, Endian::Big).unwrap();
        let value_le = read_field_value_from_bytes(&field, &be_bytes, Endian::Little).unwrap();
        assert_eq!(value_be.as_f64().unwrap(), 2.5);
        // The same bytes read as LE are NOT 2.5 (proves endian is threaded).
        assert_ne!(value_le.as_f64().unwrap(), 2.5);
    }

    #[test]
    fn read_field_value_float_short_data_errors() {
        let field = float_field("SCALE", 4);
        let result = read_field_value_from_bytes(&field, &[0x00, 0x01], Endian::Big);
        assert!(matches!(result, Err(AccessError::UnexpectedEof { .. })));
    }
}
