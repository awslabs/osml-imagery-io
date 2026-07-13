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

use super::expression::{EvalContext, EvalResult, ExpressionEvaluator, Node};
use super::types::FieldDefinition;
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

/// Lower a scalar `serde_json::Value` to an [`EvalResult`] leaf for the node
/// tree, mirroring [`json_to_write_value`]'s scalar handling. Returns `None` for
/// non-scalar JSON (arrays/objects/null), which the caller handles structurally.
fn json_scalar_to_eval(value: &serde_json::Value) -> Option<EvalResult> {
    match value {
        serde_json::Value::String(s) => Some(EvalResult::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(EvalResult::Integer(i))
            } else {
                n.as_f64().map(EvalResult::Float)
            }
        }
        serde_json::Value::Bool(b) => Some(EvalResult::String(if *b {
            "1".to_string()
        } else {
            "0".to_string()
        })),
        _ => None,
    }
}

/// Build a navigation [`Node`] for a field from its input `serde_json::Value`,
/// so an expression can index an enclosing repeated group
/// (`IMAGE_RECORDS[image_index].NCOLCB`) or dereference a nested struct on the
/// write path exactly as on read.
///
/// A scalar becomes a `Scalar` leaf; an array becomes a `Node::Array` (of struct
/// nodes for a TypeRef field, or scalar leaves otherwise); an object becomes a
/// `Node::Struct`. Struct members are keyed by the nested definition's field ids
/// (resolved case-insensitively from the object), matching how the read path
/// names them, so member derefs resolve identically on both paths. Returns
/// `None` when there is nothing navigable to represent (e.g. a bytes/null value).
fn json_to_node(
    value: &serde_json::Value,
    field_def: &FieldDefinition,
    root_types: &HashMap<String, StructureDefinition>,
) -> Option<Node> {
    match value {
        serde_json::Value::Array(arr) => {
            let nodes: Vec<Node> = arr
                .iter()
                .filter_map(|elem| json_element_to_node(elem, field_def, root_types))
                .collect();
            Some(Node::Array(nodes))
        }
        serde_json::Value::Object(_) => json_element_to_node(value, field_def, root_types),
        _ => json_scalar_to_eval(value).map(Node::Scalar),
    }
}

/// Build a `Node` for a single element/value of `field_def`: a `Struct` for a
/// nested-object TypeRef element, otherwise a scalar leaf.
fn json_element_to_node(
    value: &serde_json::Value,
    field_def: &FieldDefinition,
    root_types: &HashMap<String, StructureDefinition>,
) -> Option<Node> {
    match value {
        serde_json::Value::Object(obj) => {
            let nested_def = match &field_def.field_type {
                FieldType::TypeRef(type_name) => root_types.get(type_name)?,
                _ => return None,
            };
            let mut members = HashMap::new();
            for member in &nested_def.fields {
                let member_lower = member.id.to_lowercase();
                let member_val = obj
                    .iter()
                    .find(|(k, _)| k.to_lowercase() == member_lower)
                    .map(|(_, v)| v);
                if let Some(v) = member_val {
                    if let Some(node) = json_to_node(v, member, root_types) {
                        members.insert(member.id.clone(), node);
                    }
                }
            }
            Some(Node::Struct(members))
        }
        _ => json_scalar_to_eval(value).map(Node::Scalar),
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
        if !writer.is_field_active(field_def)? {
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
                            // is not written until the loop completes). Bound type
                            // parameters, however, are per-element: a parameterized
                            // reference inside a repeat (`foo(_index)`) binds each
                            // element's own `_index`, so seed them per iteration.
                            let inherited = writer.eval_snapshot();
                            let mut bytes_array = Vec::with_capacity(arr.len());
                            for (i, elem) in arr.iter().enumerate() {
                                let elem_inherited =
                                    seed_type_params(field_def, nested_def, &inherited, Some(i))?;
                                let elem_bytes = serialize_nested_value(
                                    elem,
                                    nested_def,
                                    &field_def.id,
                                    i,
                                    root_types,
                                    &elem_inherited,
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
                        // A non-repeated parameterized reference binds its
                        // arguments with no `_index` in scope.
                        let inherited =
                            seed_type_params(field_def, nested_def, &writer.eval_snapshot(), None)?;
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

            // Register the field's array/struct shape as a navigation node so a
            // later expression can index this group (`IMAGE_RECORDS[i].NCOLCB`)
            // or dereference this struct. Scalars are already recorded as leaves
            // by the writer; only intermediary nodes add anything here, and they
            // overwrite the last-element scalar the writer leaves for a repeated
            // field. Done after the write so ordering/validation still runs first.
            if matches!(
                value,
                serde_json::Value::Array(_) | serde_json::Value::Object(_)
            ) {
                if let Some(node) = json_to_node(value, field_def, root_types) {
                    writer.set_node(field_def.id.clone(), node);
                }
            }
        }
    }
    Ok(())
}

/// Build the inherited-scope map for a nested type reference, adding any bound
/// type parameters on top of the enclosing scope snapshot.
///
/// `inherited` is the enclosing scope (from `writer.eval_snapshot()`). For a
/// parameterized reference (`type: foo(expr)`), each argument in
/// `field_def.type_args` is evaluated against that scope — with `index` seeding
/// `_index` when the reference sits inside a repeat — and bound by the nested
/// type's param name, so the child's `size`/`repeat-expr` resolve the parameter.
/// A plain reference (no `type_args`) returns `inherited` unchanged.
fn seed_type_params(
    field_def: &FieldDefinition,
    nested_def: &StructureDefinition,
    inherited: &HashMap<String, Node>,
    index: Option<usize>,
) -> Result<HashMap<String, Node>, WriteError> {
    if field_def.type_args.is_empty() {
        return Ok(inherited.clone());
    }

    // Rebuild a parent EvalContext from the inherited nodes so the argument
    // expressions evaluate against the enclosing fields (including an enclosing
    // array indexed by `ARRAY[i].FIELD`), plus `_index` for a parameterized
    // reference inside a repeat (e.g. `crscov_block(_index)`).
    let mut parent_ctx = EvalContext::new();
    for (name, node) in inherited {
        parent_ctx.insert_node(name.clone(), node.clone());
    }
    if let Some(i) = index {
        parent_ctx = parent_ctx.with_index(i);
    }

    let evaluator = ExpressionEvaluator::new();
    let bound = evaluator
        .bind_type_params(&nested_def.params, &field_def.type_args, &parent_ctx)
        .map_err(|e| WriteError::ValidationError {
            path: field_def.id.clone(),
            message: format!("Failed to bind type parameters: {}", e),
        })?;

    let mut seeded = inherited.clone();
    for (name, value) in bound {
        seeded.insert(name, Node::Scalar(value));
    }
    Ok(seeded)
}

/// Read-side counterpart of [`seed_type_params`]: augment an enclosing-scope
/// map with a parameterized reference's bound arguments for the decode path.
///
/// `parent_scope` is the enclosing accessor's snapshot. Each argument in
/// `field_def.type_args` is evaluated against it (with `index` seeding `_index`
/// inside a repeat) and bound by the nested type's param name, so the sub-
/// accessor built from the struct bytes resolves the parameter while sizing its
/// own fields. Binding failure is best-effort: the un-augmented scope is
/// returned, matching the decode path's lenient "size what resolves" behavior.
/// A plain reference (no `type_args`) returns the scope unchanged.
fn seed_type_params_read(
    field_def: &FieldDefinition,
    nested_def: &StructureDefinition,
    parent_scope: &HashMap<String, Node>,
    index: Option<usize>,
) -> HashMap<String, Node> {
    if field_def.type_args.is_empty() {
        return parent_scope.clone();
    }

    let mut parent_ctx = EvalContext::new();
    for (name, node) in parent_scope {
        parent_ctx.insert_node(name.clone(), node.clone());
    }
    if let Some(i) = index {
        parent_ctx = parent_ctx.with_index(i);
    }

    let evaluator = ExpressionEvaluator::new();
    let mut seeded = parent_scope.clone();
    if let Ok(bound) =
        evaluator.bind_type_params(&nested_def.params, &field_def.type_args, &parent_ctx)
    {
        for (name, value) in bound {
            seeded.insert(name, Node::Scalar(value));
        }
    }
    seeded
}

/// Serialize a single JSON value as a nested structure, returning raw bytes.
fn serialize_nested_value(
    value: &serde_json::Value,
    nested_def: &StructureDefinition,
    parent_field: &str,
    index: usize,
    root_types: &HashMap<String, StructureDefinition>,
    inherited: &HashMap<String, Node>,
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
    inherited: &HashMap<String, Node>,
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
                if let Some(json_value) = value_to_json_seeded(
                    &value,
                    registry,
                    Some(definition),
                    &root_scope,
                    Some(field),
                    None,
                ) {
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
    value_to_json_seeded(value, registry, definition, &HashMap::new(), None, None)
}

/// [`value_to_json`] with an inherited (enclosing-scope) value map.
///
/// `inherited` carries the scalar field values of the scope enclosing `value`,
/// so a nested struct's `size`/`repeat-expr` — including `_root.`/`_parent.`
/// navigators — resolves against enclosing fields when the sub-accessor is
/// built. This is the decode-side mirror of the writer's inherited context. The
/// public [`value_to_json`] seeds it empty (top-level fields need no parent).
///
/// `ref_field` is the field definition that produced this value, threaded so a
/// parameterized reference (`type: foo(expr)`) can bind its arguments into the
/// sub-accessor's scope when a `Value::Struct` is re-parsed. It is `None` for
/// the top-level call and for values not reached through a field.
///
/// `ref_index` is the element index when `value` is one element of a repeated
/// field, so a parameterized element reference (`foo(_index)`) binds the correct
/// per-element `_index`; `None` for a non-repeated reference.
fn value_to_json_seeded(
    value: &Value,
    registry: Option<&StructureRegistry>,
    definition: Option<&StructureDefinition>,
    inherited: &HashMap<String, Node>,
    ref_field: Option<&FieldDefinition>,
    ref_index: Option<usize>,
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
            // Every element shares the same enclosing scope, and each carries the
            // same referencing field — a parameterized element reference binds its
            // per-element `_index` in the Struct arm below.
            let json_arr: Vec<serde_json::Value> = arr
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    value_to_json_seeded(v, registry, definition, inherited, ref_field, Some(i))
                })
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
                // `size`/`repeat-expr` navigators resolve. If this struct was
                // reached through a parameterized reference, bind its arguments
                // (evaluated in the enclosing scope, with the element `_index`
                // when repeated) so a param-sized nested field resolves too.
                let seeded = match ref_field {
                    Some(field) => seed_type_params_read(field, &def, inherited, ref_index),
                    None => inherited.clone(),
                };
                if let Ok(accessor) =
                    StructureAccessor::new_with_inherited(Arc::clone(&def), struct_val.data, seeded)
                {
                    let mut obj = serde_json::Map::new();
                    // The scope visible to this struct's own nested children is
                    // this accessor's snapshot (inherited values plus the fields
                    // it just parsed) — thread it down as their inherited map.
                    let child_inherited = accessor.eval_snapshot();
                    for field_path in accessor.fields() {
                        if let Ok(field_value) = accessor.get(&field_path) {
                            // Find the nested field definition so a parameterized
                            // reference within this struct binds too.
                            let nested_ref = def.fields.iter().find(|f| f.id == field_path);
                            if let Some(json_val) = value_to_json_seeded(
                                &field_value,
                                registry,
                                Some(&def),
                                &child_inherited,
                                nested_ref,
                                None,
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
    /// then a repeated `component_entry` whose `COMPONENT_ID` is sized by the
    /// bare name `LEN.to_i` — a reference out of the nested scope to the TRE
    /// top, resolved via the inherited-scope seeding.
    fn root_navigator_def() -> StructureDefinition {
        // Inner: COMPONENT_ID's width comes from the enclosing LEN field.
        let component_entry = StructureDefinition::new("component_entry").with_field(
            FieldDefinition::new("COMPONENT_ID", FieldType::String)
                .with_size(SizeSpec::expr(
                    ExpressionEvaluator::parse("LEN.to_i").unwrap(),
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
        //   "Unknown field reference: 'LEN'".
        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 1 byte (LEN) + 2 components * 3 bytes (sized by LEN) = 7 bytes.
        assert_eq!(encoded.len(), 7);
        assert_eq!(&encoded, b"3ABCXYZ");

        let decoded = decode_fields(&def, &encoded, None);
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    /// An RSMECA-shaped definition exercising both fixes at once: the inner type
    /// is referenced from a sibling scope **and** repeats a field by a bare name
    /// resolved against the enclosing scope. A fix addressing only one leaves
    /// this broken.
    fn sibling_and_parent_navigator_def() -> StructureDefinition {
        // Innermost: a single byte.
        let map_matrix = StructureDefinition::new("map_matrix_t").with_field(
            FieldDefinition::new("MAP", FieldType::String)
                .with_size(SizeSpec::fixed(1))
                .with_encoding(Encoding::BcsN)
                // Repeat count resolves to the enclosing record's NPAR.
                .with_repeat(RepeatSpec::expr(
                    ExpressionEvaluator::parse("NPAR.to_i").unwrap(),
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
                // MATRIX.MAP repeats NPAR (=3) times.
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

    /// A definition whose conditional field references a value that is never in
    /// the write context, so the `if:` cannot be resolved.
    fn unresolvable_condition_def() -> StructureDefinition {
        StructureDefinition::new("bad_cond").with_field(
            FieldDefinition::new("OPT", FieldType::String)
                .with_size(SizeSpec::fixed(4))
                .with_encoding(Encoding::BcsA)
                .with_condition(ExpressionEvaluator::parse("MISSING.to_i == 1").unwrap()),
        )
    }

    #[test]
    fn write_builder_propagates_unresolvable_condition() {
        // Total-or-error on the write path: `is_field_active` now errors on a
        // condition it cannot resolve instead of treating the field as active,
        // so `encode_fields` surfaces the error rather than silently guessing.
        let def = unresolvable_condition_def();
        let mut fields = HashMap::new();
        fields.insert("OPT".to_string(), serde_json::json!("ABCD"));
        let err = encode_fields(&def, &fields, false).unwrap_err();
        assert!(
            matches!(err, WriteError::ValidationError { .. }),
            "expected ValidationError, got {:?}",
            err
        );
    }

    #[test]
    fn write_builder_ok_false_condition_marks_absent() {
        // `Ok(false)` still means "absent": the field is simply not written and
        // encode succeeds. FLAG=0 makes the OPT condition false.
        let def = StructureDefinition::new("cond_ok")
            .with_field(
                FieldDefinition::new("FLAG", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("OPT", FieldType::String)
                    .with_size(SizeSpec::fixed(4))
                    .with_encoding(Encoding::BcsA)
                    .with_condition(ExpressionEvaluator::parse("FLAG.to_i == 1").unwrap()),
            );
        let mut fields = HashMap::new();
        fields.insert("FLAG".to_string(), serde_json::json!("0"));
        // OPT provided but its condition is false, so it must not be written.
        fields.insert("OPT".to_string(), serde_json::json!("ABCD"));
        let encoded =
            encode_fields(&def, &fields, false).expect("Ok(false) condition should not error");
        assert_eq!(&encoded, b"0");
    }

    /// A SENSRB-shaped definition: a 3-char BCS-A type code selects the width of
    /// the following value field via a `consts:` map subscript
    /// (`widths[CODE].to_i`). This is the exact mechanism SENSRB 12d/13e need.
    fn const_map_sized_def() -> StructureDefinition {
        use crate::parser::expression::Node;
        let mut widths = HashMap::new();
        widths.insert("06a".to_string(), EvalResult::Integer(11));
        widths.insert("06b".to_string(), EvalResult::Integer(12));
        StructureDefinition::new("tre_const_sized")
            .with_const("widths", Node::Map(widths))
            .with_field(
                FieldDefinition::new("CODE", FieldType::String)
                    .with_size(SizeSpec::fixed(3))
                    .with_encoding(Encoding::BcsA),
            )
            .with_field(
                FieldDefinition::new("VALUE", FieldType::String)
                    .with_size(SizeSpec::expr(
                        ExpressionEvaluator::parse("widths[CODE].to_i").unwrap(),
                    ))
                    .with_encoding(Encoding::BcsA),
            )
    }

    #[test]
    fn const_map_lookup_drives_size_round_trip() {
        // Both paths seed the const map into the root scope and resolve
        // `widths[CODE]` to size VALUE. CODE "06b" -> width 12.
        let def = const_map_sized_def();
        let mut fields = HashMap::new();
        fields.insert("CODE".to_string(), serde_json::json!("06b"));
        fields.insert("VALUE".to_string(), serde_json::json!("HELLOWORLD!!"));

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 3 (CODE) + 12 (VALUE, width from widths["06b"]) = 15 bytes.
        assert_eq!(encoded.len(), 15);
        assert_eq!(&encoded[..3], b"06b");

        let decoded = decode_fields(&def, &encoded, None);
        assert_eq!(decoded.get("CODE"), Some(&serde_json::json!("06b")));
        assert_eq!(
            decoded.get("VALUE"),
            Some(&serde_json::json!("HELLOWORLD!!"))
        );

        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    #[test]
    fn const_map_unknown_code_errors_on_encode() {
        // An unknown code has no width in the map: unknown-key is an error, not a
        // silent fallback. Encode surfaces it rather than mis-sizing VALUE.
        let def = const_map_sized_def();
        let mut fields = HashMap::new();
        fields.insert("CODE".to_string(), serde_json::json!("zzz"));
        fields.insert("VALUE".to_string(), serde_json::json!("X"));
        let err = encode_fields(&def, &fields, false).unwrap_err();
        assert!(
            matches!(
                err,
                WriteError::ValidationError { .. } | WriteError::ConversionError { .. }
            ),
            "expected a write error for unknown map key, got {:?}",
            err
        );
    }

    #[test]
    fn field_shadows_const_of_same_name() {
        // A parsed/written field shadows a const of the same name: the size
        // expression `n.to_i` resolves to the FIELD `n` (=2), not the const (=9).
        use crate::parser::expression::Node;
        let def = StructureDefinition::new("shadow")
            .with_const("n", Node::Scalar(EvalResult::Integer(9)))
            .with_field(
                FieldDefinition::new("n", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("DATA", FieldType::String)
                    .with_size(SizeSpec::expr(
                        ExpressionEvaluator::parse("n.to_i").unwrap(),
                    ))
                    .with_encoding(Encoding::BcsA),
            );
        let mut fields = HashMap::new();
        fields.insert("n".to_string(), serde_json::json!("2"));
        fields.insert("DATA".to_string(), serde_json::json!("AB"));

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // n (1 byte) + DATA sized by the FIELD n (=2) = 3 bytes. If the const (9)
        // won, DATA would be 9 bytes wide.
        assert_eq!(encoded.len(), 3);
        assert_eq!(&encoded, b"2AB");

        let decoded = decode_fields(&def, &encoded, None);
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
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

    /// A definition whose nested type is sized by a bound type parameter. The
    /// outer field carries a width `W`; the nested `sized_block(W.to_i)` binds
    /// that value to the param `block_width`, and the block's inner field is
    /// sized `block_width`. This is the Phase-5 mechanism (a parent scalar bound
    /// by name into a nested type) without needing the array-subscript phase.
    fn param_sized_def() -> StructureDefinition {
        use crate::parser::types::ParamDefinition;

        // Nested block: one field sized by the bound `block_width` param.
        let sized_block = StructureDefinition::new("sized_block")
            .with_param(ParamDefinition::new("block_width", Some("s4".to_string())))
            .with_field(
                FieldDefinition::new("PAYLOAD", FieldType::String)
                    .with_size(SizeSpec::expr(
                        ExpressionEvaluator::parse("block_width").unwrap(),
                    ))
                    .with_encoding(Encoding::BcsA),
            );

        StructureDefinition::new("tre_param_sized")
            .with_field(
                FieldDefinition::new("W", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("BLOCK", FieldType::TypeRef("sized_block".to_string()))
                    // type: sized_block(W.to_i)
                    .with_type_args(vec![ExpressionEvaluator::parse("W.to_i").unwrap()]),
            )
            .with_type("sized_block", sized_block)
    }

    #[test]
    fn bound_param_drives_nested_size_round_trip() {
        // The argument `W.to_i` is evaluated in the parent scope and bound to the
        // nested type's `block_width` param, sizing PAYLOAD on both paths.
        let def = param_sized_def();
        let mut fields = HashMap::new();
        fields.insert("W".to_string(), serde_json::json!("4"));
        fields.insert(
            "BLOCK".to_string(),
            serde_json::json!({ "PAYLOAD": "ABCD" }),
        );

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 1 (W) + 4 (PAYLOAD, sized by the bound param) = 5 bytes.
        assert_eq!(encoded.len(), 5);
        assert_eq!(&encoded, b"4ABCD");

        // Decode re-parses the nested block, binding the param so PAYLOAD's size
        // resolves; round-trip is byte-exact.
        let decoded = decode_fields(&def, &encoded, None);
        assert_eq!(
            decoded.get("BLOCK"),
            Some(&serde_json::json!({ "PAYLOAD": "ABCD" }))
        );
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    #[test]
    fn bound_param_is_shadowed_by_local_field() {
        // Params participate in shadowing like any named node: a locally-parsed
        // field of the same name overlays the bound param. Here the nested type
        // both takes a `block_width` param AND declares a local `block_width`
        // field; the local field's value (=2) wins over the bound arg (=4) when
        // sizing PAYLOAD.
        use crate::parser::types::ParamDefinition;

        let sized_block = StructureDefinition::new("shadow_block")
            .with_param(ParamDefinition::new("block_width", Some("s4".to_string())))
            .with_field(
                FieldDefinition::new("block_width", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("PAYLOAD", FieldType::String)
                    .with_size(SizeSpec::expr(
                        ExpressionEvaluator::parse("block_width.to_i").unwrap(),
                    ))
                    .with_encoding(Encoding::BcsA),
            );

        let def = StructureDefinition::new("tre_shadow")
            .with_field(
                FieldDefinition::new("W", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("BLOCK", FieldType::TypeRef("shadow_block".to_string()))
                    .with_type_args(vec![ExpressionEvaluator::parse("W.to_i").unwrap()]),
            )
            .with_type("shadow_block", sized_block);

        let mut fields = HashMap::new();
        fields.insert("W".to_string(), serde_json::json!("4"));
        fields.insert(
            "BLOCK".to_string(),
            serde_json::json!({ "block_width": "2", "PAYLOAD": "XY" }),
        );

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 1 (W) + 1 (block_width field) + 2 (PAYLOAD, sized by the LOCAL field=2,
        // not the bound param=4) = 4 bytes.
        assert_eq!(encoded.len(), 4);
        assert_eq!(&encoded, b"42XY");

        let decoded = decode_fields(&def, &encoded, None);
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }

    /// An RSMDCB-shaped definition exercising the array-subscript operator: an
    /// outer repeated group `RECORDS` (each with a per-image column count `NCOL`),
    /// then a nested per-image block whose inner element count is
    /// `NROW.to_i * RECORDS[image_index].NCOL.to_i` — the value can only be reached
    /// by indexing the *earlier* repeated group by the bound image index. This is
    /// the exact pattern `CRSCOV` needs (Phase 7).
    fn array_subscript_sized_def() -> StructureDefinition {
        use crate::parser::types::ParamDefinition;

        // Each outer record carries a 1-digit column count.
        let record = StructureDefinition::new("record").with_field(
            FieldDefinition::new("NCOL", FieldType::String)
                .with_size(SizeSpec::fixed(1))
                .with_encoding(Encoding::BcsN),
        );

        // Per-image covariance block: emits NROW*RECORDS[image_index].NCOL
        // single-char elements, indexing the enclosing RECORDS group.
        let cov_block = StructureDefinition::new("cov_block")
            .with_param(ParamDefinition::new("image_index", Some("s4".to_string())))
            .with_field(
                FieldDefinition::new("ELEM", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsA)
                    .with_repeat(RepeatSpec::expr(
                        ExpressionEvaluator::parse("NROW.to_i * RECORDS[image_index].NCOL.to_i")
                            .unwrap(),
                    )),
            );

        StructureDefinition::new("tre_rsmdcb_like")
            .with_field(
                FieldDefinition::new("NROW", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("NIMG", FieldType::String)
                    .with_size(SizeSpec::fixed(1))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("RECORDS", FieldType::TypeRef("record".to_string()))
                    .with_repeat(RepeatSpec::expr(
                        ExpressionEvaluator::parse("NIMG.to_i").unwrap(),
                    )),
            )
            .with_field(
                FieldDefinition::new("BLOCKS", FieldType::TypeRef("cov_block".to_string()))
                    .with_repeat(RepeatSpec::expr(
                        ExpressionEvaluator::parse("NIMG.to_i").unwrap(),
                    ))
                    // type: cov_block(_index) — bind the per-image loop counter.
                    .with_type_args(vec![ExpressionEvaluator::parse("_index").unwrap()]),
            )
            .with_type("record", record)
            .with_type("cov_block", cov_block)
    }

    #[test]
    fn array_subscript_drives_nested_repeat_round_trip() {
        // The inner block count is selected by indexing the earlier RECORDS group
        // at the bound image index, on both paths.
        let def = array_subscript_sized_def();
        let mut fields = HashMap::new();
        fields.insert("NROW".to_string(), serde_json::json!("2"));
        fields.insert("NIMG".to_string(), serde_json::json!("2"));
        // Two records: NCOL = 3 and 1.
        fields.insert(
            "RECORDS".to_string(),
            serde_json::json!([{ "NCOL": "3" }, { "NCOL": "1" }]),
        );
        // Block 0: NROW(2)*NCOL(3) = 6 elements; block 1: NROW(2)*NCOL(1) = 2.
        fields.insert(
            "BLOCKS".to_string(),
            serde_json::json!([
                { "ELEM": ["a", "b", "c", "d", "e", "f"] },
                { "ELEM": ["g", "h"] },
            ]),
        );

        let encoded = encode_fields(&def, &fields, false).expect("encode");
        // 1 (NROW) + 1 (NIMG) + 2*1 (RECORDS) + (6 + 2) (BLOCKS) = 12 bytes.
        // Layout: "2" "2" "3" "1" "abcdef" "gh".
        assert_eq!(encoded.len(), 12);
        assert_eq!(&encoded, b"2231abcdefgh");

        // Decode reconstructs the per-image block boundaries by indexing RECORDS,
        // and re-encode is byte-exact — the round trip the `size-eos` blob cannot
        // provide.
        let decoded = decode_fields(&def, &encoded, None);
        assert_eq!(
            decoded.get("BLOCKS"),
            Some(&serde_json::json!([
                { "ELEM": ["a", "b", "c", "d", "e", "f"] },
                { "ELEM": ["g", "h"] },
            ]))
        );
        let reencoded = encode_fields(&def, &decoded, false).expect("re-encode");
        assert_eq!(reencoded, encoded);
    }
}
