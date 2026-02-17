//! Evaluation context building for expression evaluation.
//!
//! This module handles building the context needed for evaluating
//! expressions that reference field values.

use crate::parser::error::AccessError;
use crate::parser::expression::{EvalContext, EvalResult, ExpressionEvaluator};
use crate::parser::types::{
    FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
};
use crate::parser::value::Value;

/// Get field size using an existing context (avoids recursion).
pub fn get_simple_field_size(
    field: &FieldDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
) -> Result<usize, AccessError> {
    match &field.size {
        SizeSpec::Fixed(size) => {
            if *size == 0 {
                // Size comes from type
                match &field.field_type {
                    FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                        Ok(*bytes as usize)
                    }
                    _ => Ok(0),
                }
            } else {
                Ok(*size)
            }
        }
        SizeSpec::Expression(expr) => {
            let result = evaluator
                .evaluate(expr, ctx)
                .map_err(|e| AccessError::ExpressionError {
                    path: field.id.clone(),
                    message: e.to_string(),
                })?;

            match result {
                EvalResult::Integer(n) if n >= 0 => Ok(n as usize),
                _ => Err(AccessError::ExpressionError {
                    path: field.id.clone(),
                    message: "Size expression did not evaluate to positive integer".to_string(),
                }),
            }
        }
    }
}

/// Get total field size using an existing context (avoids recursion).
pub fn get_simple_total_field_size(
    field: &FieldDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
) -> Result<usize, AccessError> {
    let element_size = get_simple_field_size(field, ctx, evaluator)?;

    match &field.repeat {
        None => Ok(element_size),
        Some(RepeatSpec::Count(n)) => Ok(element_size * n),
        Some(RepeatSpec::Expression(expr)) => {
            let result = evaluator
                .evaluate(expr, ctx)
                .map_err(|e| AccessError::ExpressionError {
                    path: field.id.clone(),
                    message: e.to_string(),
                })?;

            match result {
                EvalResult::Integer(n) if n >= 0 => Ok(element_size * n as usize),
                _ => Err(AccessError::ExpressionError {
                    path: field.id.clone(),
                    message: "Repeat expression did not evaluate to positive integer".to_string(),
                }),
            }
        }
        Some(RepeatSpec::Until(_)) | Some(RepeatSpec::Eos) => {
            // For until/eos, return element size as approximation
            // Actual size will be determined during parsing
            Ok(element_size)
        }
    }
}

/// Add a value to the evaluation context.
pub fn add_value_to_context_impl<'a>(
    ctx: &mut EvalContext,
    name: &str,
    value: &Value<'a>,
) -> Result<(), AccessError> {
    let eval_result = match value {
        Value::String(s) => EvalResult::String(s.to_string()),
        Value::Bytes(b) => EvalResult::Bytes(b.to_vec()),
        Value::Unsigned(n) => EvalResult::Integer(*n as i64),
        Value::Array(_) => return Ok(()), // Arrays not directly usable in expressions
        Value::Struct(_) => return Ok(()), // Structs not directly usable in expressions
    };

    ctx.fields.insert(name.to_string(), eval_result);
    Ok(())
}

/// Build evaluation context with fields from a structure definition.
pub fn build_context_from_definition<'a, F>(
    definition: &StructureDefinition,
    data: &'a [u8],
    evaluator: &ExpressionEvaluator,
    stop_at: &str,
    read_field: F,
) -> Result<EvalContext, AccessError>
where
    F: Fn(&FieldDefinition, usize, usize) -> Result<Value<'a>, AccessError>,
{
    let mut ctx = EvalContext::new();
    let mut current_offset = 0;

    for field in &definition.fields {
        if field.id == stop_at {
            break;
        }

        // Skip conditional fields that aren't present
        if let Some(ref condition) = field.condition {
            // Use a temporary context without this field
            let temp_ctx = ctx.clone();
            let result = evaluator.evaluate(condition, &temp_ctx);
            if let Ok(EvalResult::Boolean(false)) = result {
                continue;
            }
        }

        // Get field size - use simple size calculation to avoid recursion
        let size = match get_simple_field_size(field, &ctx, evaluator) {
            Ok(s) => s,
            Err(_) => continue, // Skip fields we can't size
        };

        // Read and add to context if within bounds
        if current_offset + size <= data.len() {
            if let Ok(value) = read_field(field, current_offset, size) {
                add_value_to_context_impl(&mut ctx, &field.id, &value)?;
            }
        }

        // Move past this field - use simple calculation
        let total_size =
            get_simple_total_field_size(field, &ctx, evaluator).unwrap_or(size);
        current_offset += total_size;
    }

    Ok(ctx)
}
