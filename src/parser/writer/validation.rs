//! Validation utilities for the structure writer.
//!
//! This module provides encoding validation and padding character handling.

use crate::parser::error::WriteError;
use crate::parser::types::{Encoding, FieldDefinition};

/// Validate bytes against encoding constraints.
pub fn validate_encoding(bytes: &[u8], encoding: Encoding, path: &str) -> Result<(), WriteError> {
    if !encoding.validate(bytes) {
        let invalid_chars: Vec<u8> = bytes
            .iter()
            .filter(|&&b| !encoding.is_valid_byte(b))
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
        field
            .encoding
            .map(|e| e.default_pad())
            .unwrap_or(0x20) // Default to space
    })
}
