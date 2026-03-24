//! Streaming mode writing for the structure writer.
//!
//! This module handles sequential field writing where fields must be written
//! in definition order.

use crate::parser::error::WriteError;
use crate::parser::expression::{EvalContext, EvalResult, ExpressionEvaluator};
use crate::parser::types::{FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition};

/// Get the expected field for streaming mode at the given index.
pub fn get_expected_streaming_field(
    definition: &StructureDefinition,
    next_field_index: usize,
) -> Result<FieldDefinition, WriteError> {
    definition
        .fields
        .get(next_field_index)
        .cloned()
        .ok_or_else(|| WriteError::ValidationError {
            path: "".to_string(),
            message: "No more fields expected".to_string(),
        })
}

/// Get the last written field name for error messages.
pub fn get_last_written_field(
    definition: &StructureDefinition,
    next_field_index: usize,
) -> String {
    if next_field_index == 0 {
        "start".to_string()
    } else {
        definition
            .fields
            .get(next_field_index - 1)
            .map(|f| f.id.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Get the size for a field in streaming mode.
pub fn get_streaming_field_size(
    field: &FieldDefinition,
    evaluator: &ExpressionEvaluator,
    ctx: &EvalContext,
) -> Result<usize, WriteError> {
    match &field.size {
        SizeSpec::Fixed(size) => {
            if *size == 0 {
                match &field.field_type {
                    FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                        Ok(*bytes as usize)
                    }
                    // TypeRef fields have variable size determined by the nested
                    // structure's serialized bytes, not by a fixed size spec.
                    FieldType::TypeRef(_) => Ok(0),
                    _ => Err(WriteError::ValidationError {
                        path: field.id.clone(),
                        message: "Cannot determine size for field".to_string(),
                    }),
                }
            } else {
                Ok(*size)
            }
        }
        SizeSpec::Expression(expr) => {
            // Evaluate the expression using written values
            let result = evaluator.evaluate(expr, ctx).map_err(|e| {
                WriteError::ValidationError {
                    path: field.id.clone(),
                    message: format!("Failed to evaluate size expression: {}", e),
                }
            })?;

            match result {
                EvalResult::Integer(n) if n >= 0 => Ok(n as usize),
                _ => Err(WriteError::ValidationError {
                    path: field.id.clone(),
                    message: "Size expression did not evaluate to positive integer".to_string(),
                }),
            }
        }
    }
}

/// Get the repeat count for a repeated field.
pub fn get_repeat_count(
    repeat: &RepeatSpec,
    field_name: &str,
    evaluator: &ExpressionEvaluator,
    ctx: &EvalContext,
) -> Result<usize, WriteError> {
    match repeat {
        RepeatSpec::Count(n) => Ok(*n),
        RepeatSpec::Expression(expr) => {
            let result = evaluator.evaluate(expr, ctx).map_err(|e| {
                WriteError::ValidationError {
                    path: field_name.to_string(),
                    message: format!("Failed to evaluate repeat expression: {}", e),
                }
            })?;

            match result {
                EvalResult::Integer(n) if n >= 0 => Ok(n as usize),
                _ => Err(WriteError::ValidationError {
                    path: field_name.to_string(),
                    message: "Repeat expression did not evaluate to positive integer".to_string(),
                }),
            }
        }
        RepeatSpec::Until(_) | RepeatSpec::Eos => {
            // For until/eos, we can't pre-determine the count
            Err(WriteError::ValidationError {
                path: field_name.to_string(),
                message: "Cannot determine repeat count for until/eos repeats".to_string(),
            })
        }
    }
}
