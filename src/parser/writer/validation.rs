//! Validation utilities for the structure writer.
//!
//! This module provides encoding validation and padding character handling.

use crate::parser::error::WriteError;
use crate::parser::types::{Encoding, FieldDefinition};

/// Validate bytes against encoding constraints.
///
/// When `strict` is false, numeric fields (BCS-N and BCS-NPI) are validated
/// against BCS-A (printable ASCII 0x20-0x7E) instead of their declared encoding.
/// This tolerates real-world data that deviates from the spec (e.g. signed values
/// in BCS-NPI fields) while still rejecting binary garbage.
pub fn validate_encoding(
    bytes: &[u8],
    encoding: Encoding,
    path: &str,
    strict: bool,
) -> Result<(), WriteError> {
    let effective_encoding = if !strict {
        match encoding {
            Encoding::BcsN | Encoding::BcsNPI => Encoding::BcsA,
            other => other,
        }
    } else {
        encoding
    };

    if !effective_encoding.validate(bytes) {
        let invalid_chars: Vec<u8> = bytes
            .iter()
            .filter(|&&b| !effective_encoding.is_valid_byte(b))
            .copied()
            .collect();

        return Err(WriteError::ValidationError {
            path: path.to_string(),
            message: format!(
                "Invalid characters for {:?} encoding: {:?}",
                encoding, invalid_chars
            ),
        });
    }
    Ok(())
}

/// Get the padding character for a field.
pub fn get_pad_char(field: &FieldDefinition) -> u8 {
    field.pad.unwrap_or_else(|| {
        field.encoding.map(|e| e.default_pad()).unwrap_or(0x20) // Default to space
    })
}
