//! Evaluation context building for expression evaluation.
//!
//! This module handles building the context needed for evaluating
//! expressions that reference field values.

use std::collections::HashMap;

use crate::parser::error::AccessError;
use crate::parser::expression::{EvalContext, EvalResult, Expression, ExpressionEvaluator, Node};
use crate::parser::types::{
    Endian, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
};
use crate::parser::value::Value;

use super::read::read_float;

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
    type_args: &[Expression],
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

    // Build a local context for the nested type, then bind any parameterized
    // type-reference arguments: each is evaluated in the *enclosing* context
    // (`ctx`) and seeded by the target type's param name into `nested_ctx`, so a
    // reference like `IMAGE_RECORDS[image_index].NCOLCB` resolves inside the
    // nested scope. `type_args` is empty for a plain (non-parameterized) ref.
    let mut nested_ctx = ctx.clone();
    for (name, value) in evaluator
        .bind_type_params(&nested_def.params, type_args, ctx)
        .map_err(|e| AccessError::ExpressionError {
            path: format!("type:{}", type_name),
            message: e.to_string(),
        })?
    {
        nested_ctx.insert_scalar(name, value);
    }
    let mut total_size = 0;

    for field in &nested_def.fields {
        // Check if this field is conditional
        if let Some(ref condition) = field.condition {
            let result = evaluator.evaluate(condition, &nested_ctx);
            match result {
                Ok(EvalResult::Boolean(false)) => {
                    // Field legitimately absent — `Ok(false)` means "not present".
                    continue;
                }
                Ok(EvalResult::Boolean(true)) => {
                    // Condition is true, continue to process field
                }
                Err(e) => {
                    // Total-or-error: a condition that references an unavailable
                    // value is an error, not a silent skip. Swallowing it here
                    // desyncs the cursor (the field's bytes are still present but
                    // its size is dropped from the running total). Propagate.
                    return Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: e.to_string(),
                    });
                }
                _ => {
                    // A non-boolean condition result is malformed, not "absent".
                    return Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: "Condition did not evaluate to boolean".to_string(),
                    });
                }
            }
        }

        // A count-0 repeated field contributes nothing and must not be probed:
        // for a repeated TypeRef whose element type carries its own
        // data-dependent `repeat-expr`, the single-element probe below errors
        // against absent element data. Letting that `?` propagate would abort
        // sizing of the whole enclosing type (the nested mirror of the
        // empty-`WARP_SETS` decode-abort defect). Skip it and add 0 size.
        if let Some(RepeatSpec::Count(0)) = &field.repeat {
            continue;
        }
        if let Some(RepeatSpec::Expression(expr)) = &field.repeat {
            if let Ok(EvalResult::Integer(0)) = evaluator.evaluate(expr, &nested_ctx) {
                continue;
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
                8 if data.len() >= 8 => u64::from_be_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]),
                _ => 0,
            };
            Ok(Value::Unsigned(n))
        }
        FieldType::SignedInt(bytes) => {
            let n: i64 = match bytes {
                1 => data.first().map(|&b| b as i8 as i64).unwrap_or(0),
                2 if data.len() >= 2 => i16::from_be_bytes([data[0], data[1]]) as i64,
                3 if data.len() >= 3 => {
                    // Sign-extend a 24-bit value into i32 before widening.
                    let raw = u32::from_be_bytes([0, data[0], data[1], data[2]]);
                    ((raw << 8) as i32 >> 8) as i64
                }
                4 if data.len() >= 4 => {
                    i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64
                }
                8 if data.len() >= 8 => i64::from_be_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]),
                _ => 0,
            };
            Ok(Value::Signed(n))
        }
        FieldType::Float(byte_size) => {
            // Best-effort, like the integer arms above: this helper runs during
            // offset probing and may see truncated data, so a short/odd-size read
            // yields 0.0 rather than erroring (the authoritative read paths use
            // read_float and surface UnexpectedEof). Big-endian to match integers.
            if data.len() >= *byte_size as usize {
                Ok(Value::Float(
                    read_float(data, *byte_size, Endian::Big).unwrap_or(0.0),
                ))
            } else {
                Ok(Value::Float(0.0))
            }
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
                            &field.type_args,
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
        SizeSpec::Eos => Ok(data.len().saturating_sub(base_offset)),
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
                                &field.type_args,
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
        Value::Signed(n) => EvalResult::Integer(*n),
        // Floats are usable in expressions: the evaluator supports float
        // comparisons and arithmetic (e.g. `SCALE_FACTOR > 4.5`).
        Value::Float(f) => EvalResult::Float(*f),
        Value::Array(_) => return Ok(()), // Arrays enter the tree in the array-subscript phase
        Value::Struct(_) => return Ok(()), // Structs enter the tree in the array-subscript phase
    };

    ctx.insert_scalar(name, eval_result);
    Ok(())
}

/// Build evaluation context with fields from a structure definition.
pub fn build_context_from_definition<'a, F>(
    definition: &StructureDefinition,
    data: &'a [u8],
    evaluator: &ExpressionEvaluator,
    stop_at: &str,
    seed: &HashMap<String, Node>,
    read_field: F,
) -> Result<EvalContext, AccessError>
where
    F: Fn(&FieldDefinition, usize, usize) -> Result<Value<'a>, AccessError>,
{
    // Pre-seed with inherited (enclosing-scope) values; locally parsed fields
    // overlay them below (local wins).
    let mut ctx = EvalContext::new();
    // Seed compile-time consts first (const map/scalar nodes). The loader stamps
    // the file-level consts onto every nested type, so `definition.consts` is
    // populated at every scope and a nested `MAP[KEY]` subscript resolves. A
    // parsed field of the same name shadows the const (fields overlay below).
    for (name, node) in &definition.consts {
        ctx.insert_node(name.clone(), node.clone());
    }
    for (name, node) in seed {
        ctx.insert_node(name.clone(), node.clone());
    }
    let mut current_offset = 0;

    for field in &definition.fields {
        if field.id == stop_at {
            break;
        }

        // Skip conditional fields that aren't present.
        if let Some(ref condition) = field.condition {
            // Use a temporary context without this field
            let temp_ctx = ctx.clone();
            match evaluator.evaluate(condition, &temp_ctx) {
                Ok(EvalResult::Boolean(false)) => {
                    // Field legitimately absent — `Ok(false)` means "not present".
                    continue;
                }
                Ok(EvalResult::Boolean(true)) => {
                    // Present — fall through and size it.
                }
                Ok(_) => {
                    // A non-boolean condition result is malformed, not "absent".
                    return Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: "Condition did not evaluate to boolean".to_string(),
                    });
                }
                Err(e) => {
                    // Total-or-error: a condition referencing an unavailable value
                    // is an error, not "present". The prior code let any non-false
                    // result (including `Err`) fall through as present, papering
                    // over an unresolvable condition — flip it to propagate.
                    return Err(AccessError::ExpressionError {
                        path: field.id.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        // Sanctioned structural skip (mirrors `ensure_parsed` and
        // `get_nested_type_size`): a count-0 repeated field reads no element and
        // contributes 0 bytes, so it must not be probed. For a repeated TypeRef
        // whose element type carries its own data-dependent `repeat-expr`, the
        // single-element probe below errors against absent element data; with the
        // total-or-error flip that `Err` would now propagate and abort context
        // building. Skip the field structurally *before* any evaluation instead.
        if let Some(RepeatSpec::Count(0)) = &field.repeat {
            continue;
        }
        if let Some(RepeatSpec::Expression(expr)) = &field.repeat {
            if let Ok(EvalResult::Integer(0)) = evaluator.evaluate(expr, &ctx) {
                continue;
            }
        }

        // Get field size - use simple size calculation to avoid recursion.
        // Total-or-error: a field whose size cannot be resolved is a propagated
        // error, not a silently-skipped field (skipping desyncs the read cursor,
        // the mechanism behind the SENSRB `size: 12` bug).
        let size = get_simple_field_size(field, &ctx, evaluator, definition, data, current_offset)?;

        // Read and add to context if within bounds
        if current_offset + size <= data.len() {
            if let Ok(value) = read_field(field, current_offset, size) {
                add_value_to_context_impl(&mut ctx, &field.id, &value)?;
            }
        }

        // Move past this field - use simple calculation with TypeRef support.
        // Total-or-error: propagate a total-size failure rather than falling back
        // to the single-element `size`, which would silently under-advance the
        // cursor for a repeated field.
        let total_size =
            get_simple_total_field_size(field, &ctx, evaluator, definition, data, current_offset)?;
        current_offset += total_size;
    }

    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_field(id: &str, size: u8) -> FieldDefinition {
        FieldDefinition::new(id, FieldType::SignedInt(size))
    }

    #[test]
    fn read_simple_value_signed_negative_s4() {
        // The probing helper must produce a Signed value carrying the sign,
        // not a bit-cast Unsigned.
        let field = signed_field("DELTA", 4);
        let bytes = (-5i32).to_be_bytes();
        let value = read_simple_value(&field, &bytes).unwrap();
        assert!(value.is_signed());
        assert_eq!(value.as_i64().unwrap(), -5);
    }

    #[test]
    fn read_simple_value_signed_s8_not_dropped() {
        // Regression: size 8 previously fell through to 0 in this helper.
        let field = signed_field("BIG", 8);
        let bytes = (-9_000_000_000i64).to_be_bytes();
        let value = read_simple_value(&field, &bytes).unwrap();
        assert_eq!(value.as_i64().unwrap(), -9_000_000_000);
    }

    #[test]
    fn read_simple_value_unsigned_s8_not_dropped() {
        // Parallel gap: the unsigned arm also lacked size 8.
        let field = FieldDefinition::new("BIG_U", FieldType::UnsignedInt(8));
        let n = 0x0102_0304_0506_0708u64;
        let bytes = n.to_be_bytes();
        let value = read_simple_value(&field, &bytes).unwrap();
        assert_eq!(value.as_u64().unwrap(), n);
    }

    #[test]
    fn read_simple_value_signed_s8_feeds_context_as_integer() {
        // The s8 value must reach the expression context as a signed Integer
        // (this is the path that drives offset/size/condition expressions).
        let field = signed_field("BIG", 8);
        let bytes = (-42i64).to_be_bytes();
        let value = read_simple_value(&field, &bytes).unwrap();
        let mut ctx = EvalContext::new();
        add_value_to_context_impl(&mut ctx, "BIG", &value).unwrap();
        match ctx.get_scalar("BIG") {
            Some(EvalResult::Integer(n)) => assert_eq!(*n, -42),
            other => panic!("expected Integer(-42), got {:?}", other),
        }
    }

    /// Build a definition with a single expression-sized field referencing a
    /// name that is never provided, so its `size:` cannot be resolved.
    fn unresolvable_size_def() -> StructureDefinition {
        StructureDefinition::new("bad_size").with_field(
            FieldDefinition::new("F", FieldType::String).with_size(SizeSpec::expr(
                ExpressionEvaluator::parse("MISSING.to_i").unwrap(),
            )),
        )
    }

    #[test]
    fn read_builder_propagates_unresolvable_size() {
        // Total-or-error: a `size:` referencing an unavailable value must error
        // out of the read builder, not silently skip the field (the SENSRB
        // `size: 12` desync mechanism).
        let def = unresolvable_size_def();
        let data = vec![b'X'; 16];
        let evaluator = ExpressionEvaluator::new();
        let seed = HashMap::new();
        let result =
            build_context_from_definition(&def, &data, &evaluator, "", &seed, |field, off, sz| {
                read_simple_value(field, &data[off..off + sz])
            });
        assert!(
            matches!(result, Err(AccessError::ExpressionError { .. })),
            "expected ExpressionError, got {:?}",
            result
        );
    }

    #[test]
    fn read_builder_ok_false_condition_marks_absent() {
        // `Ok(false)` on an `if:` still means the field is legitimately absent:
        // the builder skips it and succeeds (does not error).
        let def = StructureDefinition::new("cond")
            .with_field(
                FieldDefinition::new("FLAG", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1)),
            )
            .with_field(
                FieldDefinition::new("OPT", FieldType::String)
                    .with_size(SizeSpec::Fixed(4))
                    .with_condition(ExpressionEvaluator::parse("FLAG.to_i == 1").unwrap()),
            );
        // FLAG = 0 -> condition false -> OPT absent, no error.
        let data = vec![0u8; 8];
        let evaluator = ExpressionEvaluator::new();
        let seed = HashMap::new();
        let ctx =
            build_context_from_definition(&def, &data, &evaluator, "", &seed, |field, off, sz| {
                read_simple_value(field, &data[off..off + sz])
            })
            .expect("Ok(false) condition should not error");
        // FLAG parsed; OPT skipped as absent.
        assert!(ctx.get_scalar("FLAG").is_some());
        assert!(ctx.get_scalar("OPT").is_none());
    }

    #[test]
    fn read_builder_propagates_unresolvable_condition() {
        // Total-or-error: a condition referencing an unavailable value errors
        // rather than being treated as present (the flipped top-level site).
        let def = StructureDefinition::new("cond_err").with_field(
            FieldDefinition::new("OPT", FieldType::String)
                .with_size(SizeSpec::Fixed(4))
                .with_condition(ExpressionEvaluator::parse("MISSING.to_i == 1").unwrap()),
        );
        let data = vec![b'X'; 8];
        let evaluator = ExpressionEvaluator::new();
        let seed = HashMap::new();
        let result =
            build_context_from_definition(&def, &data, &evaluator, "", &seed, |field, off, sz| {
                read_simple_value(field, &data[off..off + sz])
            });
        assert!(
            matches!(result, Err(AccessError::ExpressionError { .. })),
            "expected ExpressionError, got {:?}",
            result
        );
    }
}
