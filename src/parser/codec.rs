//! Format-agnostic encode/decode of structured field values.
//!
//! This module holds the recursive serialization (`encode_fields`) and
//! deserialization (`decode_fields` / [`value_to_json`]) logic that converts
//! between nested dict/list field values (`serde_json::Value`) and raw bytes,
//! driven purely by a [`StructureDefinition`].
//!
//! The recursion has **no TRE coupling** — it operates only on
//! [`StructureDefinition`], the parser's [`Value`] type, and `serde_json::Value`.
//! The NITF read/write paths (`jbp::tre_fields`, `jbp::metadata`) are thin
//! wrappers that add CETAG/envelope handling on top of these functions, and the
//! public `StructureDefinition::encode`/`decode` bindings call them directly.

use std::collections::HashMap;
use std::sync::Arc;

use super::writer::WriteValue;
use super::{
    FieldType, StructureAccessor, StructureDefinition, StructureRegistry, StructureWriter, Value,
    WriteError,
};

/// Serialize a nested dict of field values to bytes using the given definition.
///
/// Iterates the definition's fields in order, looking up matching values from
/// `fields` (case-insensitive), recursing into nested types and arrays. Scalar
/// fields are written directly; arrays become repeated fields; objects become
/// single nested (TypeRef) fields.
///
/// `strict` controls numeric encoding validation: when `true`, fields are
/// validated against their declared encoding exactly; when `false` (the metadata
/// write default), numeric fields are relaxed to BCS-A.
pub fn encode_fields(
    definition: &StructureDefinition,
    fields: &HashMap<String, serde_json::Value>,
    strict: bool,
) -> Result<Vec<u8>, WriteError> {
    let mut writer = StructureWriter::new(Arc::new(definition.clone()));
    writer.set_strict_encoding(strict);

    // Write fields in definition order by iterating the definition's fields
    // and looking up values from the provided map.
    write_fields_to_writer(&mut writer, definition, fields)?;

    writer.finish()
}

/// Decode a lowercase/uppercase hex string into raw bytes.
///
/// Returns `None` if the string has an odd length or contains a non-hex
/// character. This is the inverse of the hex encoding `value_to_json` applies to
/// `bytes`-typed fields.
fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Some(out)
}

/// Convert a serde_json::Value to a WriteValue for scalar types.
fn json_to_write_value(value: &serde_json::Value) -> Option<WriteValue> {
    match value {
        serde_json::Value::String(s) => Some(WriteValue::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(WriteValue::Integer(i))
            } else if let Some(u) = n.as_u64() {
                Some(WriteValue::Unsigned(u))
            } else {
                n.as_f64().map(WriteValue::Float)
            }
        }
        serde_json::Value::Bool(b) => Some(WriteValue::String(if *b {
            "1".to_string()
        } else {
            "0".to_string()
        })),
        _ => None,
    }
}

/// Serialize a JSON object's fields into a StructureWriter using the given definition.
///
/// This handles scalar fields directly and recurses for arrays and nested objects.
fn write_fields_to_writer(
    writer: &mut StructureWriter,
    definition: &StructureDefinition,
    fields: &HashMap<String, serde_json::Value>,
) -> Result<(), WriteError> {
    for field_def in &definition.fields {
        let field_id_lower = field_def.id.to_lowercase();

        // Find the matching value in the group (case-insensitive)
        let value = fields.iter().find_map(|(name, val)| {
            if name.to_lowercase() == field_id_lower {
                Some(val)
            } else {
                None
            }
        });

        if let Some(value) = value {
            match value {
                serde_json::Value::String(s)
                    if matches!(field_def.field_type, FieldType::Bytes) =>
                {
                    // `bytes`-typed fields carry a hex string (see `value_to_json`).
                    // Hex-decode back to raw bytes so the field round-trips; a
                    // non-hex string is a caller error surfaced as a write error.
                    let bytes = decode_hex(s).ok_or_else(|| WriteError::ValidationError {
                        path: field_def.id.clone(),
                        message: format!(
                            "Field '{}' is a bytes field; value must be a hex string",
                            field_def.id
                        ),
                    })?;
                    writer.set(&field_def.id, WriteValue::Bytes(bytes))?;
                }
                serde_json::Value::String(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::Bool(_) => {
                    if let Some(wv) = json_to_write_value(value) {
                        writer.set(&field_def.id, wv)?;
                    }
                }
                serde_json::Value::Array(arr) => {
                    match &field_def.field_type {
                        FieldType::TypeRef(type_name) => {
                            // Array of nested objects: serialize each element
                            // using a sub-writer for the nested type definition
                            let nested_def = definition.types.get(type_name).ok_or_else(|| {
                                WriteError::ValidationError {
                                    path: field_def.id.clone(),
                                    message: format!(
                                        "Nested type '{}' not found in definition",
                                        type_name
                                    ),
                                }
                            })?;
                            let mut bytes_array = Vec::with_capacity(arr.len());
                            for (i, elem) in arr.iter().enumerate() {
                                let elem_bytes =
                                    serialize_nested_value(elem, nested_def, &field_def.id, i)?;
                                bytes_array.push(WriteValue::Bytes(elem_bytes));
                            }
                            writer.set(&field_def.id, WriteValue::Array(bytes_array))?;
                        }
                        _ => {
                            // Array of scalars: convert each element to WriteValue
                            let write_values: Vec<WriteValue> =
                                arr.iter().filter_map(json_to_write_value).collect();
                            writer.set(&field_def.id, WriteValue::Array(write_values))?;
                        }
                    }
                }
                serde_json::Value::Object(obj) => {
                    // Single nested object (non-repeated TypeRef field)
                    if let FieldType::TypeRef(type_name) = &field_def.field_type {
                        let nested_def = definition.types.get(type_name).ok_or_else(|| {
                            WriteError::ValidationError {
                                path: field_def.id.clone(),
                                message: format!(
                                    "Nested type '{}' not found in definition",
                                    type_name
                                ),
                            }
                        })?;
                        let nested_fields: HashMap<String, serde_json::Value> =
                            obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        let nested_bytes =
                            serialize_nested_fields(nested_def, &nested_fields, &field_def.id)?;
                        writer.set(&field_def.id, WriteValue::Bytes(nested_bytes))?;
                    }
                }
                _ => {
                    // Skip null values
                }
            }
        }
    }
    Ok(())
}

/// Serialize a single JSON value as a nested structure, returning raw bytes.
fn serialize_nested_value(
    value: &serde_json::Value,
    nested_def: &StructureDefinition,
    parent_field: &str,
    index: usize,
) -> Result<Vec<u8>, WriteError> {
    match value {
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, serde_json::Value> =
                obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            serialize_nested_fields(nested_def, &fields, parent_field)
        }
        _ => Err(WriteError::ValidationError {
            path: format!("{}_{}", parent_field, index),
            message: "Expected object value for nested type".to_string(),
        }),
    }
}

/// Serialize a set of fields using a nested StructureDefinition, returning raw bytes.
///
/// Creates a sub-writer for the nested definition and recursively writes all fields.
fn serialize_nested_fields(
    definition: &StructureDefinition,
    fields: &HashMap<String, serde_json::Value>,
    parent_path: &str,
) -> Result<Vec<u8>, WriteError> {
    let mut sub_writer = StructureWriter::new(Arc::new(definition.clone()));
    write_fields_to_writer(&mut sub_writer, definition, fields).map_err(|e| {
        WriteError::ValidationError {
            path: parent_path.to_string(),
            message: format!("Failed to serialize nested structure: {}", e),
        }
    })?;
    sub_writer
        .finish()
        .map_err(|e| WriteError::ValidationError {
            path: parent_path.to_string(),
            message: format!("Failed to finalize nested structure: {}", e),
        })
}

/// Eagerly parse all fields from a structure definition into a HashMap.
///
/// Creates a `StructureAccessor` from the definition and raw bytes, then iterates
/// all fields (respecting conditions and repeated fields) to build the map.
pub fn decode_fields(
    definition: &StructureDefinition,
    raw_bytes: &[u8],
    registry: Option<&StructureRegistry>,
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    if let Ok(accessor) = StructureAccessor::new(Arc::new(definition.clone()), raw_bytes) {
        for field in &definition.fields {
            let field_id = &field.id;
            if field.condition.is_some() && !accessor.has(field_id) {
                continue;
            }
            if let Ok(value) = accessor.get(field_id) {
                if let Some(json_value) = value_to_json(&value, registry, Some(definition)) {
                    result.insert(field_id.clone(), json_value);
                }
            }
        }
    }
    result
}

/// Convert a parsed Value to a serde_json::Value.
///
/// This function handles the conversion of all Value variants to their
/// JSON equivalents:
/// - String → JSON string
/// - Bytes → JSON string (hex-encoded if not valid UTF-8)
/// - Unsigned → JSON number
/// - Array → JSON array
/// - Struct → Resolves to a nested JSON object with named fields when the type
///   can be found in the parent definition's local types, the global registry,
///   or both. Falls back to `{"_type": "...", "_data": "..."}` otherwise.
///
/// # Arguments
/// * `value` - The parsed Value to convert
/// * `registry` - Optional structure registry for resolving Value::Struct types
/// * `definition` - Optional parent structure definition whose `types` map
///   contains local type definitions (e.g., `image_segment_info`, `band_info_type`)
pub fn value_to_json(
    value: &Value,
    registry: Option<&StructureRegistry>,
    definition: Option<&StructureDefinition>,
) -> Option<serde_json::Value> {
    match value {
        Value::String(cow) => {
            // Trim trailing spaces (standard NITF padding)
            let trimmed = cow.trim_end_matches(' ');
            Some(serde_json::Value::String(trimmed.to_string()))
        }
        Value::Bytes(bytes) => {
            // A `bytes`-typed field always decodes to a lowercase hex string, and
            // `encode_fields` hex-decodes the same representation back to raw bytes
            // for `FieldType::Bytes` fields. This symmetry is required for binary
            // fields (e.g. BANDSB `DATA_FLD_*`) to round-trip: the previous
            // UTF-8-first behavior produced a string `encode` could not invert
            // (binary fields hex-expanded to 2x width and tripped `ValueTooLarge`).
            // The cost is that `iminfo` surfaces hex for `bytes`-typed fields — an
            // intentional, minor consequence (see DESIGN_TRE_ROUND_TRIP_TESTING.md
            // Goal 6 / Review Decision 2).
            let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
            Some(serde_json::Value::String(hex))
        }
        Value::Unsigned(n) => Some(serde_json::Value::Number((*n).into())),
        Value::Signed(n) => Some(serde_json::Value::Number((*n).into())),
        Value::Float(f) => serde_json::Number::from_f64(*f).map(serde_json::Value::Number),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr
                .iter()
                .filter_map(|v| value_to_json(v, registry, definition))
                .collect();
            Some(serde_json::Value::Array(json_arr))
        }
        Value::Struct(struct_val) => {
            // Try to resolve the struct type from local types first, then registry.
            // Local types (definition.types) hold types like image_segment_info,
            // band_info_type that are defined within the parent KSY structure.
            let resolved_def: Option<Arc<StructureDefinition>> = definition
                .and_then(|def| def.types.get(&struct_val.type_name))
                .map(|local_def| Arc::new(local_def.clone()))
                .or_else(|| registry.and_then(|reg| reg.get(&struct_val.type_name)));

            if let Some(def) = resolved_def {
                if let Ok(accessor) = StructureAccessor::new(Arc::clone(&def), struct_val.data) {
                    let mut obj = serde_json::Map::new();
                    // Use the resolved definition as the new parent for nested structs
                    for field_path in accessor.fields() {
                        if let Ok(field_value) = accessor.get(&field_path) {
                            if let Some(json_val) =
                                value_to_json(&field_value, registry, Some(&def))
                            {
                                obj.insert(field_path, json_val);
                            }
                        }
                    }
                    return Some(serde_json::Value::Object(obj));
                }
            }

            // Fall back to opaque representation when type not found
            // or accessor creation fails
            let mut obj = serde_json::Map::new();
            obj.insert(
                "_type".to_string(),
                serde_json::Value::String(struct_val.type_name.clone()),
            );
            // Include hex-encoded data for debugging
            let hex: String = struct_val
                .data
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            obj.insert("_data".to_string(), serde_json::Value::String(hex));
            Some(serde_json::Value::Object(obj))
        }
    }
}
