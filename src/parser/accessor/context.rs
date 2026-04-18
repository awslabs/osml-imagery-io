//! Evaluation context building for expression evaluation.
//!
//! This module handles building the context needed for evaluating
//! expressions that reference field values.

use crate::parser::error::AccessError;
use crate::parser::expression::{EvalContext, EvalResult, ExpressionEvaluator};
use crate::parser::types::{FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition};
use crate::parser::value::Value;

/// Calculate the size of a nested type instance.
///
/// This function resolves a TypeRef field by looking up the referenced type
/// in the structure definition's `types` map and calculating its total size
/// by summing the sizes of all its fields.
///
/// # Arguments
/// * `type_name` - Name of the nested type to resolve (e.g., "band_info_type")
/// * `definition` - The parent structure definition containing the types map
/// * `ctx` - The evaluation context with previously parsed field values
/// * `evaluator` - Expression evaluator for size/condition expressions
/// * `data` - The raw data buffer (needed for variable-length nested types)
/// * `offset` - The offset where this nested type instance starts in the data
///
/// # Returns
/// The total size in bytes of the nested type instance.
///
/// # Errors
/// Returns `AccessError::UnknownField` if the type name is not found in the
/// definition's types map.
///
/// # Behavior
/// - Handles conditional fields by evaluating their conditions
/// - Recursively handles nested TypeRef fields within the type
/// - Builds a local context to track field values for expression evaluation
/// - For variable-length nested types (with conditional or expression-sized
///   fields), reads actual data to determine the correct size
fn get_nested_type_size(
    type_name: &str,
    definition: &StructureDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
    data: &[u8],
    offset: usize,
) -> Result<usize, AccessError> {
    let nested_def = definition
        .types
        .get(type_name)
        .ok_or_else(|| AccessError::UnknownField {
            path: format!("type:{}", type_name),
        })?;

    // Build a local context for the nested type
    let mut nested_ctx = ctx.clone();
    let mut total_size = 0;

    for field in &nested_def.fields {
        // Check if this field is conditional
        if let Some(ref condition) = field.condition {
            let result = evaluator.evaluate(condition, &nested_ctx);
            match result {
                Ok(EvalResult::Boolean(false)) => {
                    // Skip this conditional field
                    continue;
                }
                Ok(EvalResult::Boolean(true)) => {
                    // Condition is true, continue to process field
                }
                Err(_) => {
                    // Condition evaluation failed - skip this field
                    // This can happen when referenced fields don't exist yet
                    continue;
                }
                _ => {
                    // Condition didn't evaluate to boolean - skip field
                    continue;
                }
            }
        }

        // Calculate field size - recursively handle TypeRef
        let field_size = get_simple_field_size(
            field,
            &nested_ctx,
            evaluator,
            definition,
            data,
            offset + total_size,
        )?;

        // Read field value and add to context for subsequent fields
        if offset + total_size + field_size <= data.len() {
            let field_data = &data[offset + total_size..offset + total_size + field_size];
            // Add simple values to context for expression evaluation
            if let Ok(value) = read_simple_value(field, field_data) {
                let _ = add_value_to_context_impl(&mut nested_ctx, &field.id, &value);
            }
        }

        // Handle repetitions
        let total_field_size = get_simple_total_field_size(
            field,
            &nested_ctx,
            evaluator,
            definition,
            data,
            offset + total_size,
        )?;

        total_size += total_field_size;
    }

    Ok(total_size)
}

/// Read a simple value from field data for context building.
fn read_simple_value<'a>(
    field: &FieldDefinition,
    data: &'a [u8],
) -> Result<Value<'a>, AccessError> {
    use std::borrow::Cow;

    match &field.field_type {
        FieldType::String => {
            let s = std::str::from_utf8(data).unwrap_or("");
            Ok(Value::String(Cow::Borrowed(s)))
        }
        FieldType::Bytes => Ok(Value::Bytes(data)),
        FieldType::UnsignedInt(bytes) => {
            let n = match bytes {
                1 => data.first().map(|&b| b as u64).unwrap_or(0),
                2 if data.len() >= 2 => u16::from_be_bytes([data[0], data[1]]) as u64,
                3 if data.len() >= 3 => u32::from_be_bytes([0, data[0], data[1], data[2]]) as u64,
                4 if data.len() >= 4 => {
                    u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64
                }
                _ => 0,
            };
            Ok(Value::Unsigned(n))
        }
        FieldType::SignedInt(bytes) => {
            let n = match bytes {
                1 => data.first().map(|&b| b as i8 as i64 as u64).unwrap_or(0),
                2 if data.len() >= 2 => i16::from_be_bytes([data[0], data[1]]) as i64 as u64,
                4 if data.len() >= 4 => {
                    i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64 as u64
                }
                _ => 0,
            };
            Ok(Value::Unsigned(n))
        }
        FieldType::TypeRef(_) => {
            // TypeRef fields are structs, not simple values
            Err(AccessError::UnknownField {
                path: field.id.clone(),
            })
        }
    }
}

/// Get field size using an existing context (avoids recursion).
///
/// This function calculates the size of a single field element based on its
/// type and size specification. It is used during context building to track
/// offsets for subsequent fields.
///
/// # Arguments
/// * `field` - The field definition containing type and size information
/// * `ctx` - The evaluation context with previously parsed field values
/// * `evaluator` - Expression evaluator for size expressions
/// * `definition` - The parent structure definition (required for TypeRef resolution)
/// * `data` - The raw data buffer (required for variable-length nested types)
/// * `base_offset` - The offset where this field starts in the data
///
/// # Returns
/// The size in bytes of a single element of this field.
///
/// # TypeRef Handling
/// When the field type is `FieldType::TypeRef(type_name)`:
/// - Calls `get_nested_type_size()` to calculate the actual size
/// - This enables correct offset calculation for fields after TypeRef arrays
/// - Without this, fields following repeated TypeRef arrays would be inaccessible
///
/// # Errors
/// - `AccessError::UnknownField` if a TypeRef references a non-existent type
/// - `AccessError::ExpressionError` if a size expression fails to evaluate
pub fn get_simple_field_size(
    field: &FieldDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
    definition: &StructureDefinition,
    data: &[u8],
    base_offset: usize,
) -> Result<usize, AccessError> {
    match &field.size {
        SizeSpec::Fixed(size) => {
            if *size == 0 {
                // Size comes from type
                match &field.field_type {
                    FieldType::UnsignedInt(bytes) | FieldType::SignedInt(bytes) => {
                        Ok(*bytes as usize)
                    }
                    FieldType::TypeRef(type_name) => {
                        // Get size from nested type
                        get_nested_type_size(
                            type_name,
                            definition,
                            ctx,
                            evaluator,
                            data,
                            base_offset,
                        )
                    }
                    _ => Ok(0),
                }
            } else {
                Ok(*size)
            }
        }
        SizeSpec::Expression(expr) => {
            let result =
                evaluator
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
///
/// This function calculates the total size of a field, accounting for
/// repetitions. For repeated fields, it multiplies the element size by
/// the repeat count.
///
/// # Arguments
/// * `field` - The field definition containing type, size, and repeat info
/// * `ctx` - The evaluation context with previously parsed field values
/// * `evaluator` - Expression evaluator for size/repeat expressions
/// * `definition` - The parent structure definition (required for TypeRef resolution)
/// * `data` - The raw data buffer (required for variable-length nested types)
/// * `base_offset` - The offset where this field starts in the data
///
/// # Returns
/// The total size in bytes of this field (element_size × repeat_count).
///
/// # TypeRef Handling for Repeated Fields
/// For repeated TypeRef fields (e.g., `band_info` in image subheaders):
/// - If elements have variable sizes (due to conditional fields), each
///   element's size is calculated individually using `get_nested_type_size()`
/// - The sizes are summed to get the accurate total
/// - This is critical for fields like `band_info_type` which contain
///   conditional LUT data that varies per band
///
/// # Errors
/// - `AccessError::ExpressionError` if repeat expression fails to evaluate
/// - Propagates errors from `get_simple_field_size()` and `get_nested_type_size()`
pub fn get_simple_total_field_size(
    field: &FieldDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
    definition: &StructureDefinition,
    data: &[u8],
    base_offset: usize,
) -> Result<usize, AccessError> {
    let element_size = get_simple_field_size(field, ctx, evaluator, definition, data, base_offset)?;

    match &field.repeat {
        None => Ok(element_size),
        Some(RepeatSpec::Count(n)) => Ok(element_size * n),
        Some(RepeatSpec::Expression(expr)) => {
            let result =
                evaluator
                    .evaluate(expr, ctx)
                    .map_err(|e| AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: e.to_string(),
                    })?;

            match result {
                EvalResult::Integer(n) if n >= 0 => {
                    // For TypeRef fields with variable-length elements, calculate each element's size
                    if let FieldType::TypeRef(type_name) = &field.field_type {
                        let mut total = 0;
                        let mut current_offset = base_offset;
                        for _ in 0..(n as usize) {
                            let elem_size = get_nested_type_size(
                                type_name,
                                definition,
                                ctx,
                                evaluator,
                                data,
                                current_offset,
                            )?;
                            total += elem_size;
                            current_offset += elem_size;
                        }
                        Ok(total)
                    } else {
                        Ok(element_size * n as usize)
                    }
                }
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
        // Pass definition, data, and current_offset for TypeRef resolution
        let size =
            match get_simple_field_size(field, &ctx, evaluator, definition, data, current_offset) {
                Ok(s) => s,
                Err(_) => continue, // Skip fields we can't size
            };

        // Read and add to context if within bounds
        if current_offset + size <= data.len() {
            if let Ok(value) = read_field(field, current_offset, size) {
                add_value_to_context_impl(&mut ctx, &field.id, &value)?;
            }
        }

        // Move past this field - use simple calculation with TypeRef support
        let total_size =
            get_simple_total_field_size(field, &ctx, evaluator, definition, data, current_offset)
                .unwrap_or(size);
        current_offset += total_size;
    }

    Ok(ctx)
}
