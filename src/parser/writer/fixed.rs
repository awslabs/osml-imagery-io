//! Fixed-size mode writing for the structure writer.
//!
//! This module handles writing fields to a pre-allocated fixed-size buffer,
//! allowing out-of-order field writes.

use std::collections::HashMap;

use crate::parser::error::WriteError;
use crate::parser::types::{FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition};

/// Calculate the fixed layout for a structure definition.
///
/// Returns the total size and a map of field offsets.
pub fn calculate_fixed_layout(
    definition: &StructureDefinition,
) -> Result<(usize, HashMap<String, (usize, usize)>), WriteError> {
    let mut offset = 0;
    let mut field_offsets = HashMap::new();

    for field in &definition.fields {
        let size = get_fixed_field_size(field)?;

        // Handle repeated fields
        if let Some(ref repeat) = field.repeat {
            match repeat {
                RepeatSpec::Count(n) => {
                    // Store offset for each indexed element
                    for i in 0..*n {
                        let indexed_name = format!("{}_{}", field.id, i);
                        field_offsets.insert(indexed_name, (offset + i * size, size));
                    }
                    offset += size * n;
                }
                _ => {
                    // Expression-based, until, or eos repeats can't be pre-calculated
                    return Err(WriteError::ValidationError {
                        path: field.id.clone(),
                        message: "Variable-length repeats not supported in fixed-size mode"
                            .to_string(),
                    });
                }
            }
        } else {
            field_offsets.insert(field.id.clone(), (offset, size));
            offset += size;
        }
    }

    Ok((offset, field_offsets))
}

/// Get the fixed size of a field.
pub fn get_fixed_field_size(field: &FieldDefinition) -> Result<usize, WriteError> {
    match &field.size {
        SizeSpec::Fixed(size) => {
            if *size == 0 {
                // Size comes from type
                match &field.field_type {
                    FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                        Ok(*bytes as usize)
                    }
                    _ => Err(WriteError::ValidationError {
                        path: field.id.clone(),
                        message: "Cannot determine fixed size for field".to_string(),
                    }),
                }
            } else {
                Ok(*size)
            }
        }
        SizeSpec::Expression(_) => Err(WriteError::ValidationError {
            path: field.id.clone(),
            message: "Expression-based sizes not supported in fixed-size mode".to_string(),
        }),
    }
}
