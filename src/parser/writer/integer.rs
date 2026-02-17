//! Integer encoding with endianness support.
//!
//! This module handles encoding signed and unsigned integers into binary format
//! with configurable byte order (big-endian or little-endian).

use crate::parser::error::WriteError;
use crate::parser::types::Endian;

/// Encode an unsigned integer with specified size and endianness.
pub fn encode_unsigned(
    n: u64,
    byte_size: u8,
    endian: Endian,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    // Check value fits in the specified size
    let max_value = match byte_size {
        1 => u8::MAX as u64,
        2 => u16::MAX as u64,
        4 => u32::MAX as u64,
        8 => u64::MAX,
        _ => {
            return Err(WriteError::ValidationError {
                path: path.to_string(),
                message: format!("Unsupported integer size: {}", byte_size),
            })
        }
    };

    if n > max_value {
        return Err(WriteError::ValueTooLarge {
            path: path.to_string(),
            max_size: byte_size as usize,
            actual_size: 8, // u64 size
        });
    }

    let bytes = match (byte_size, endian) {
        (1, _) => vec![n as u8],
        (2, Endian::Big) => (n as u16).to_be_bytes().to_vec(),
        (2, Endian::Little) => (n as u16).to_le_bytes().to_vec(),
        (4, Endian::Big) => (n as u32).to_be_bytes().to_vec(),
        (4, Endian::Little) => (n as u32).to_le_bytes().to_vec(),
        (8, Endian::Big) => n.to_be_bytes().to_vec(),
        (8, Endian::Little) => n.to_le_bytes().to_vec(),
        _ => {
            return Err(WriteError::ValidationError {
                path: path.to_string(),
                message: format!("Unsupported integer size: {}", byte_size),
            })
        }
    };

    Ok(bytes)
}

/// Encode a signed integer with specified size and endianness.
pub fn encode_signed(
    n: i64,
    byte_size: u8,
    endian: Endian,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    // Check value fits in the specified size
    let (min_value, max_value) = match byte_size {
        1 => (i8::MIN as i64, i8::MAX as i64),
        2 => (i16::MIN as i64, i16::MAX as i64),
        4 => (i32::MIN as i64, i32::MAX as i64),
        8 => (i64::MIN, i64::MAX),
        _ => {
            return Err(WriteError::ValidationError {
                path: path.to_string(),
                message: format!("Unsupported integer size: {}", byte_size),
            })
        }
    };

    if n < min_value || n > max_value {
        return Err(WriteError::ValueTooLarge {
            path: path.to_string(),
            max_size: byte_size as usize,
            actual_size: 8,
        });
    }

    let bytes = match (byte_size, endian) {
        (1, _) => vec![n as i8 as u8],
        (2, Endian::Big) => (n as i16).to_be_bytes().to_vec(),
        (2, Endian::Little) => (n as i16).to_le_bytes().to_vec(),
        (4, Endian::Big) => (n as i32).to_be_bytes().to_vec(),
        (4, Endian::Little) => (n as i32).to_le_bytes().to_vec(),
        (8, Endian::Big) => n.to_be_bytes().to_vec(),
        (8, Endian::Little) => n.to_le_bytes().to_vec(),
        _ => {
            return Err(WriteError::ValidationError {
                path: path.to_string(),
                message: format!("Unsupported integer size: {}", byte_size),
            })
        }
    };

    Ok(bytes)
}
