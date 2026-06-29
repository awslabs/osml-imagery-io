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

use super::expression::EvalResult;
use super::writer::WriteValue;
use super::{
    FieldType, StructureAccessor, StructureDefinition, StructureRegistry, StructureWriter, Value,
    WriteError,
};

/// Collect every nested type definition into a single flat map keyed by type
/// name.
///
/// These structures use one type namespace per top-level definition: in
/// practice all types are declared flat under the root `types:` map, and type
/// names are unique within a structure. The builder API nonetheless allows a
/// type to be declared lexically inside another type's `types`, so this walks
/// the tree and lifts every type to one map. A nested TypeRef then resolves
/// against this flat map regardless of where the type was declared or how deep
/// the reference sits. Name collisions (which the formats don't produce) resolve
/// last-writer-wins, an accepted simplification of full lexical scoping.
fn flatten_types(definition: &StructureDefinition, out: &mut HashMap<String, StructureDefinition>) {
    for (name, nested) in &definition.types {
        flatten_types(nested, out);
        out.insert(name.clone(), nested.clone());
    }
}

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

    // Write fields in definition order by iterating the definition's fields and
    // looking up values from the provided map. `root_types` is the structure's
    // flattened type map: every nested TypeRef — at any depth — resolves against
    // this one map (see `flatten_types`).
    let mut root_types = HashMap::new();
    flatten_types(definition, &mut root_types);
    write_fields_to_writer(&mut writer, definition, fields, &root_types)?;

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
    root_types: &HashMap<String, StructureDefinition>,
) -> Result<(), WriteError> {
    for field_def in &definition.fields {
        // Gate writes on the writer's own notion of field presence rather than on
        // whether a key happens to be in the input map. A field whose `if:`
        // condition is currently false is skipped regardless of what the map
        // contains — this lets callers over-provide values (every maybe-present
        // field) and have `encode` emit exactly the fields the definition says
        // are present, matching what `decode` produces. A field that is active
        // but absent from the map stays unwritten and is surfaced by
        // `finish()` as `MissingRequired`.
        if !writer.is_field_active(field_def) {
            continue;
        }

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
                            // Array of nested objects: serialize each element with
                            // a sub-writer for the nested type. Types are flat —
                            // resolve against the structure's single `root_types`
                            // map regardless of nesting depth.
                            let nested_def = root_types.get(type_name).ok_or_else(|| {
                                WriteError::ValidationError {
                                    path: field_def.id.clone(),
                                    message: format!(
                                        "Nested type '{}' not found in definition",
                                        type_name
                                    ),
                                }
                            })?;
                            // Snapshot the enclosing scope once: every element
                            // shares the same parent scope (the array field itself
                            // is not written until the loop completes).
                            let inherited = writer.eval_snapshot();
                            let mut bytes_array = Vec::with_capacity(arr.len());
                            for (i, elem) in arr.iter().enumerate() {
                                let elem_bytes = serialize_nested_value(
                                    elem,
                                    nested_def,
                                    &field_def.id,
                                    i,
                                    root_types,
                                    &inherited,
                                )?;
                                bytes_array.push(WriteValue::Bytes(elem_bytes));
                            }
                            writer.set(&field_def.id, WriteValue::Array(bytes_array))?;
                        }
                        FieldType::Bytes => {
                            // Repeated `bytes` field: each element is a hex string
                            // (the same representation `value_to_json` emits per
                            // element). Hex-decode each back to raw bytes, mirroring
                            // the scalar Bytes branch above.
                            let mut write_values = Vec::with_capacity(arr.len());
                            for elem in arr {
                                let bytes = match elem {
                                    serde_json::Value::String(s) => {
                                        decode_hex(s).ok_or_else(|| WriteError::ValidationError {
                                            path: field_def.id.clone(),
                                            message: format!(
                                                "Field '{}' is a bytes field; each value must be a hex string",
                                                field_def.id
                                            ),
                                        })?
                                    }
                                    _ => {
                                        return Err(WriteError::ValidationError {
                                            path: field_def.id.clone(),
                                            message: format!(
                                                "Field '{}' is a bytes field; each value must be a hex string",
                                                field_def.id
                                            ),
                                        })
                                    }
                                };
                                write_values.push(WriteValue::Bytes(bytes));
                            }
                            writer.set(&field_def.id, WriteValue::Array(write_values))?;
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
                        // Types are flat — resolve against `root_types`.
                        let nested_def = root_types.get(type_name).ok_or_else(|| {
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
                        let inherited = writer.eval_snapshot();
                        let nested_bytes = serialize_nested_fields(
                            nested_def,
                            &nested_fields,
                            &field_def.id,
                            root_types,
                            &inherited,
                        )?;
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
    root_types: &HashMap<String, StructureDefinition>,
    inherited: &HashMap<String, EvalResult>,
) -> Result<Vec<u8>, WriteError> {
    match value {
        serde_json::Value::Object(obj) => {
            let fields: HashMap<String, serde_json::Value> =
                obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            serialize_nested_fields(nested_def, &fields, parent_field, root_types, inherited)
        }
        _ => Err(WriteError::ValidationError {
            path: format!("{}_{}", parent_field, index),
            message: "Expected object value for nested type".to_string(),
        }),
    }
}

/// Serialize a set of fields using a nested StructureDefinition, returning raw bytes.
///
/// Creates a sub-writer for the nested definition and recursively writes all
/// fields. `root_types` is the structure's flat type map (these structures never
/// nest `types:` blocks, so every TypeRef resolves against it at any depth).
/// `inherited` is a snapshot of the enclosing scope's scalar values, seeded into
/// the sub-writer so nested `size`/`repeat-expr` expressions — including
/// `_root.`/`_parent.` navigators — can reference enclosing fields. The
/// sub-writer is discarded when this function returns, so its locally-written
/// values never leak back to the parent (the "pop").
fn serialize_nested_fields(
    definition: &StructureDefinition,
    fields: &HashMap<String, serde_json::Value>,
    parent_path: &str,
    root_types: &HashMap<String, StructureDefinition>,
    inherited: &HashMap<String, EvalResult>,
) -> Result<Vec<u8>, WriteError> {
    let mut sub_writer = StructureWriter::new(Arc::new(definition.clone()));
    sub_writer.set_inherited_context(inherited.clone());
    write_fields_to_writer(&mut sub_writer, definition, fields, root_types).map_err(|e| {
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
        // The root scope's scalar fields are inherited by any nested struct at
        // this level, so a nested `size`/`repeat-expr` referencing a root field
        // (e.g. `_root.LEN`) resolves.
        let root_scope = accessor.eval_snapshot();
        for field in &definition.fields {
            let field_id = &field.id;
            if field.condition.is_some() && !accessor.has(field_id) {
                continue;
            }
            if let Ok(value) = accessor.get(field_id) {
                if let Some(json_value) =
                    value_to_json_seeded(&value, registry, Some(definition), &root_scope)
                {
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
    value_to_json_seeded(value, registry, definition, &HashMap::new())
}

/// [`value_to_json`] with an inherited (enclosing-scope) value map.
///
/// `inherited` carries the scalar field values of the scope enclosing `value`,
/// so a nested struct's `size`/`repeat-expr` — including `_root.`/`_parent.`
/// navigators — resolves against enclosing fields when the sub-accessor is
/// built. This is the decode-side mirror of the writer's inherited context. The
/// public [`value_to_json`] seeds it empty (top-level fields need no parent).
fn value_to_json_seeded(
    value: &Value,
    registry: Option<&StructureRegistry>,
    definition: Option<&StructureDefinition>,
    inherited: &HashMap<String, EvalResult>,
) -> Option<serde_json::Value> {
    match value {
        Value::String(cow) => {
            // Return the field's bytes verbatim — no trailing-space trim. The
            // padding is part of the on-disk value (BCS-A fields are left
            // justified and right-padded with spaces per JBP §4.6.4), so
            // returning it faithfully makes `encode(decode(bytes)) == bytes` and
            // preserves spec-significant all-spaces sentinels (e.g. ACCPOB
            // `UNIAAH`, whose spaces gate a conditional). Inspection callers that
            // want trimmed display call `.strip()` themselves.
            Some(serde_json::Value::String(cow.to_string()))
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
            // Every element shares the same enclosing scope.
            let json_arr: Vec<serde_json::Value> = arr
                .iter()
                .filter_map(|v| value_to_json_seeded(v, registry, definition, inherited))
                .collect();
            Some(serde_json::Value::Array(json_arr))
        }
        Value::Struct(struct_val) => {
            // Resolve the struct type from local types first, then registry.
            // `definition.types` is the structure's single flat type map (these
            // structures never nest `types:` blocks), so it holds every type at
            // every depth — e.g. both `acpo_record` and the `accuracy_point`
            // that `acpo_record` references.
            let resolved_def: Option<Arc<StructureDefinition>> = definition
                .and_then(|def| def.types.get(&struct_val.type_name))
                .map(|local_def| {
                    // Propagate the flattened type map into the resolved
                    // definition so the sub-accessor below can size and resolve
                    // sibling types its own `types` map would otherwise miss.
                    // Without this, a second-level nested TypeRef (e.g.
                    // `accuracy_point` inside `acpo_record`) fails to resolve on
                    // decode, mirroring the encode-side sibling-scope gap.
                    let mut with_types = local_def.clone();
                    if let Some(parent) = definition {
                        let mut flat = HashMap::new();
                        flatten_types(parent, &mut flat);
                        with_types.types = flat;
                    }
                    Arc::new(with_types)
                })
                .or_else(|| registry.and_then(|reg| reg.get(&struct_val.type_name)));

            if let Some(def) = resolved_def {
                // Seed the sub-accessor with the enclosing scope so nested
                // `size`/`repeat-expr` navigators resolve.
                if let Ok(accessor) = StructureAccessor::new_with_inherited(
                    Arc::clone(&def),
                    struct_val.data,
                    inherited.clone(),
                ) {
                    let mut obj = serde_json::Map::new();
                    // The scope visible to this struct's own nested children is
                    // this accessor's snapshot (inherited values plus the fields
                    // it just parsed) — thread it down as their inherited map.
                    let child_inherited = accessor.eval_snapshot();
                    for field_path in accessor.fields() {
                        if let Ok(field_value) = accessor.get(&field_path) {
                            if let Some(json_val) = value_to_json_seeded(
                                &field_value,
                                registry,
                                Some(&def),
                                &child_inherited,
                            ) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::expression::ExpressionEvaluator;
    use crate::parser::types::{Encoding, FieldDefinition, RepeatSpec, SizeSpec};

    /// A definition with a count field and a repeated 2-byte `bytes` field whose
    /// count is fixed at 3. Exercises the repeated-`bytes` hex round-trip.
    fn repeated_bytes_def() -> StructureDefinition {
        StructureDefinition::new("repeated_bytes")
            .with_field(
                FieldDefinition::new("N", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("DATA", FieldType::Bytes)
                    .with_size(SizeSpec::fixed(2))
                    .with_repeat(RepeatSpec::count(3)),
            )
    }

    #[test]
    fn repeated_bytes_round_trip_via_hex() {
        let def = repeated_bytes_def();
        let mut fields = HashMap::new();
        fields.insert("N".to_string(), serde_json::json!("3"));
        // Each repeated `bytes` element is a 2-byte hex string (4 hex chars),
        // mirroring what `value_to_json` emits per element.
        fields.insert(
            "DATA".to_string(),
            serde_json::json!(["00ff", "1234", "abcd"]),
        );

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 1 byte (N) + 3 * 2 bytes (DATA) = 7 bytes.
        assert_eq!(encoded.len(), 7);

        let decoded = decode_fields(&def, &encoded, None);
        assert_eq!(
            decoded.get("DATA"),
            Some(&serde_json::json!(["00ff", "1234", "abcd"]))
        );

        // Re-encoding the decoded dict reproduces the same bytes (idempotent).
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    /// A *flat* definition modeled on ACCPOB: the TRE declares both
    /// `acpo_record` and `accuracy_point` under its single `types` map, and
    /// `acpo_record` references `accuracy_point` from its own (empty) scope.
    /// Resolving the inner `POINTS` TypeRef therefore requires falling back to
    /// the enclosing TRE-level `types` map. Returns (outer, acpo_record,
    /// accuracy_point) wired into one flat definition.
    fn sibling_scope_def() -> StructureDefinition {
        // Inner-inner: a 2-byte point.
        let accuracy_point = StructureDefinition::new("accuracy_point")
            .with_field(
                FieldDefinition::new("LAT", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("LON", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            );

        // Inner: a record whose POINTS field references the SIBLING type
        // `accuracy_point`. Note `acpo_record.types` is intentionally empty —
        // the reference must resolve via the enclosing map.
        let acpo_record = StructureDefinition::new("acpo_record").with_field(
            FieldDefinition::new("POINTS", FieldType::TypeRef("accuracy_point".to_string()))
                .with_repeat(RepeatSpec::count(2)),
        );

        // Outer: ACPO_DATA is a single `acpo_record`. Both nested types are
        // declared flat under the TRE-level `types` map.
        StructureDefinition::new("tre_accpob_like")
            .with_field(FieldDefinition::new(
                "ACPO_DATA",
                FieldType::TypeRef("acpo_record".to_string()),
            ))
            .with_type("acpo_record", acpo_record)
            .with_type("accuracy_point", accuracy_point)
    }

    #[test]
    fn sibling_scope_nested_typeref_resolves() {
        let def = sibling_scope_def();
        let mut fields = HashMap::new();
        fields.insert(
            "ACPO_DATA".to_string(),
            serde_json::json!({
                "POINTS": [
                    {"LAT": "1", "LON": "2"},
                    {"LAT": "3", "LON": "4"},
                ]
            }),
        );

        // Before the flat-types resolution, this raised:
        //   "Nested type 'accuracy_point' not found in definition".
        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 2 points * 2 bytes each = 4 bytes.
        assert_eq!(encoded.len(), 4);
        assert_eq!(&encoded, b"1234");

        // Decode resolves the second-level sibling type and re-encode reproduces
        // the same bytes (the decode path has the same flat-types resolution).
        let decoded = decode_fields(&def, &encoded, None);
        assert_eq!(
            decoded.get("ACPO_DATA"),
            Some(&serde_json::json!({
                "POINTS": [
                    {"LAT": "1", "LON": "2"},
                    {"LAT": "3", "LON": "4"},
                ]
            }))
        );
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    /// A MITOCA-shaped definition: the TRE declares a root-level `LEN` field,
    /// then a repeated `component_entry` whose `COMPONENT_ID` is sized by
    /// `_root.LEN.to_i` — a navigator out of the nested scope to the TRE top.
    fn root_navigator_def() -> StructureDefinition {
        // Inner: COMPONENT_ID's width comes from the root LEN field.
        let component_entry = StructureDefinition::new("component_entry").with_field(
            FieldDefinition::new("COMPONENT_ID", FieldType::String)
                .with_size(SizeSpec::expr(
                    ExpressionEvaluator::parse("_root.LEN.to_i").unwrap(),
                ))
                .with_encoding(Encoding::BcsA),
        );

        StructureDefinition::new("tre_mitoca_like")
            .with_field(
                FieldDefinition::new("LEN", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new(
                    "COMPONENTS",
                    FieldType::TypeRef("component_entry".to_string()),
                )
                .with_repeat(RepeatSpec::count(2)),
            )
            .with_type("component_entry", component_entry)
    }

    #[test]
    fn root_navigator_in_nested_size_resolves() {
        let def = root_navigator_def();
        let mut fields = HashMap::new();
        fields.insert("LEN".to_string(), serde_json::json!("3"));
        fields.insert(
            "COMPONENTS".to_string(),
            serde_json::json!([
                {"COMPONENT_ID": "ABC"},
                {"COMPONENT_ID": "XYZ"},
            ]),
        );

        // Before seeding inherited values, this raised:
        //   "Unknown field reference: '_root.LEN'".
        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 1 byte (LEN) + 2 components * 3 bytes (sized by _root.LEN) = 7 bytes.
        assert_eq!(encoded.len(), 7);
        assert_eq!(&encoded, b"3ABCXYZ");

        let decoded = decode_fields(&def, &encoded, None);
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    /// An RSMECA-shaped definition exercising both fixes at once: the inner type
    /// is referenced from a sibling scope (Phase 1) **and** repeats a field by a
    /// `_parent` navigator (Phase 2). A fix addressing only one leaves this
    /// broken.
    fn sibling_and_parent_navigator_def() -> StructureDefinition {
        // Innermost: a single byte.
        let map_matrix = StructureDefinition::new("map_matrix_t").with_field(
            FieldDefinition::new("MAP", FieldType::String)
                .with_size(SizeSpec::fixed(1))
                .with_encoding(Encoding::BcsN)
                // Repeat count navigates to the enclosing record's NPAR.
                .with_repeat(RepeatSpec::expr(
                    ExpressionEvaluator::parse("_parent.NPAR.to_i").unwrap(),
                )),
        );

        // Middle: declares NPAR, then references the SIBLING type `map_matrix_t`.
        let comp = StructureDefinition::new("comp_t")
            .with_field(
                FieldDefinition::new("NPAR", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(FieldDefinition::new(
                "MATRIX",
                FieldType::TypeRef("map_matrix_t".to_string()),
            ));

        StructureDefinition::new("tre_rsmeca_like")
            .with_field(FieldDefinition::new(
                "COMP",
                FieldType::TypeRef("comp_t".to_string()),
            ))
            .with_type("comp_t", comp)
            .with_type("map_matrix_t", map_matrix)
    }

    #[test]
    fn sibling_typeref_and_parent_navigator_compose() {
        let def = sibling_and_parent_navigator_def();
        let mut fields = HashMap::new();
        fields.insert(
            "COMP".to_string(),
            serde_json::json!({
                "NPAR": "3",
                // MATRIX.MAP repeats _parent.NPAR (=3) times.
                "MATRIX": {"MAP": ["1", "2", "3"]},
            }),
        );

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // NPAR (1 byte) + 3 * MAP (1 byte each) = 4 bytes.
        assert_eq!(encoded.len(), 4);
        assert_eq!(&encoded, b"3123");

        let decoded = decode_fields(&def, &encoded, None);
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    /// An ACCPOB-shaped definition: a BCS-A discriminator field (`UNIAAH`) whose
    /// all-spaces value is a spec-significant sentinel gating a conditional
    /// (`AAH`, present only when `UNIAAH != "   "`). Used to prove faithful decode
    /// preserves the padding so the round trip does not flip the conditional.
    fn sentinel_conditional_def() -> StructureDefinition {
        StructureDefinition::new("tre_accpob_like")
            .with_field(
                FieldDefinition::new("UNIAAH", FieldType::String)
                    .with_size(SizeSpec::fixed(3))
                    .with_encoding(Encoding::BcsA),
            )
            .with_field(
                FieldDefinition::new("AAH", FieldType::String)
                    .with_size(SizeSpec::fixed(5))
                    .with_encoding(Encoding::BcsN)
                    .with_condition(ExpressionEvaluator::parse("UNIAAH != \"   \"").unwrap()),
            )
    }

    #[test]
    fn sentinel_all_spaces_round_trips_faithfully() {
        // Faithful bytes: UNIAAH = "   " (all spaces) means AAH is not present,
        // so the encoded form is just the 3-byte discriminator.
        let def = sentinel_conditional_def();
        let raw = b"   ".to_vec();

        // Decode must preserve the all-spaces sentinel, not trim it to "".
        let decoded = decode_fields(&def, &raw, None);
        assert_eq!(decoded.get("UNIAAH"), Some(&serde_json::json!("   ")));
        // The conditional stays inactive, so AAH is absent.
        assert!(!decoded.contains_key("AAH"));

        // encode(decode(bytes)) == bytes, with no canonicalization cycle.
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, raw);
    }

    #[test]
    fn sentinel_present_round_trips_faithfully() {
        // When UNIAAH carries a real value, AAH is present. The BCS-N AAH field
        // is left-padded with zeros ("00123"), and faithful decode keeps it
        // exactly so the round trip is byte-exact.
        let def = sentinel_conditional_def();
        let raw = b"M  00123".to_vec();

        let decoded = decode_fields(&def, &raw, None);
        // BCS-A discriminator keeps its trailing-space padding verbatim.
        assert_eq!(decoded.get("UNIAAH"), Some(&serde_json::json!("M  ")));
        // BCS-N field keeps its leading zeros verbatim (no integer normalization).
        assert_eq!(decoded.get("AAH"), Some(&serde_json::json!("00123")));

        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, raw);
    }

    #[test]
    fn repeated_bytes_rejects_non_hex_element() {
        let def = repeated_bytes_def();
        let mut fields = HashMap::new();
        fields.insert("N".to_string(), serde_json::json!("3"));
        // "zz" is not valid hex.
        fields.insert(
            "DATA".to_string(),
            serde_json::json!(["00ff", "zz12", "abcd"]),
        );
        let err = encode_fields(&def, &fields, false).unwrap_err();
        assert!(matches!(err, WriteError::ValidationError { .. }));
    }
}
