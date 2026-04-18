//! TRE field access using runtime-loaded definitions.
//!
//! This module provides helper functions for accessing TRE (Tagged Record Extension)
//! fields using the data-driven parser infrastructure. TRE definitions are loaded
//! from `.ksy` files at runtime via the Structure Registry, enabling new TRE types
//! to be supported without code changes.
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::tre_fields;
//! use osml_imagery_io::parser::StructureRegistry;
//!
//! let registry = StructureRegistry::new();
//! let cedata = &[/* TRE CEDATA bytes */];
//!
//! // Check if a TRE definition exists
//! if tre_fields::has_definition(&registry, "GEOLOB") {
//!     // Create an accessor for the TRE CEDATA
//!     if let Ok(Some(accessor)) = tre_fields::create_accessor(&registry, "GEOLOB", cedata) {
//!         // Access fields using the accessor
//!         if let Ok(value) = accessor.get("ARV") {
//!             println!("ARV: {:?}", value);
//!         }
//!     }
//! }
//! ```

use std::sync::Arc;

use super::tre::{TreEnvelope, TreFieldGroup};
use crate::parser::writer::WriteValue;
use crate::parser::{
    AccessError, FieldType, StructureAccessor, StructureDefinition, StructureRegistry,
    StructureWriter, WriteError,
};

/// Look up a TRE definition from the registry by CETAG.
///
/// Normalizes the tag (trim whitespace, lowercase) and prepends `tre_` to form
/// the registry key. Returns `None` if no definition exists for this tag.
fn lookup_definition(registry: &StructureRegistry, tag: &str) -> Option<Arc<StructureDefinition>> {
    let normalized_tag = tag.trim().to_lowercase();
    let def_name = format!("tre_{}", normalized_tag);
    registry.get(&def_name)
}

/// Create a StructureAccessor for a TRE's CEDATA.
///
/// Looks up the TRE definition from the registry using the pattern `tre_{cetag_lowercase}`
/// and creates an accessor for parsing the CEDATA bytes.
///
/// # Arguments
///
/// * `registry` - The structure registry containing TRE definitions
/// * `tag` - The 6-character CETAG identifying the TRE type (will be trimmed and lowercased)
/// * `cedata` - The raw CEDATA bytes to parse
///
/// # Returns
///
/// * `Ok(Some(accessor))` - If a definition exists and the accessor was created successfully
/// * `Ok(None)` - If no definition exists for this TRE tag (unknown TRE)
/// * `Err(AccessError)` - If the definition exists but accessor creation failed
///
/// # Example
///
/// ```ignore
/// let accessor = tre_fields::create_accessor(&registry, "GEOLOB", cedata)?;
/// if let Some(acc) = accessor {
///     let arv = acc.get("arv")?;
/// }
/// ```
pub fn create_accessor<'a>(
    registry: &StructureRegistry,
    tag: &str,
    cedata: &'a [u8],
) -> Result<Option<StructureAccessor<'a>>, AccessError> {
    let definition = match lookup_definition(registry, tag) {
        Some(def) => def,
        None => return Ok(None),
    };

    let accessor = StructureAccessor::new(Arc::clone(&definition), cedata)?;
    Ok(Some(accessor))
}

/// Check if a TRE definition exists in the registry.
///
/// # Arguments
///
/// * `registry` - The structure registry containing TRE definitions
/// * `tag` - The 6-character CETAG identifying the TRE type (will be trimmed and lowercased)
///
/// # Returns
///
/// `true` if a definition exists for this TRE tag, `false` otherwise.
///
/// # Example
///
/// ```ignore
/// if tre_fields::has_definition(&registry, "GEOLOB") {
///     println!("GEOLOB TRE is supported");
/// }
/// ```
pub fn has_definition(registry: &StructureRegistry, tag: &str) -> bool {
    lookup_definition(registry, tag).is_some()
}

/// Serialize a TRE field group to CEDATA bytes.
///
/// Uses the TRE definition from the registry to serialize field values to binary.
/// Field names in the group should match the field IDs in the definition (case-insensitive).
///
/// # Arguments
///
/// * `registry` - The structure registry containing TRE definitions
/// * `group` - The TRE field group containing field values to serialize
///
/// # Returns
///
/// * `Ok(Some(cedata))` - If a definition exists and serialization succeeded
/// * `Ok(None)` - If no definition exists for this TRE tag (unknown TRE)
/// * `Err(WriteError)` - If serialization failed (missing required fields, invalid values, etc.)
///
/// # Example
///
/// ```ignore
/// let mut group = TreFieldGroup::new("GEOLOB");
/// group.insert("arv", json!("000360000"));
/// group.insert("brv", json!("000360000"));
///
/// if let Some(cedata) = serialize_tre_fields(&registry, &group)? {
///     let envelope = TreEnvelope::new("GEOLOB", cedata)?;
/// }
/// ```
///
/// # Requirements
///
/// _Requirements: 8.1, 8.2, 8.3_
pub fn serialize_tre_fields(
    registry: &StructureRegistry,
    group: &TreFieldGroup,
) -> Result<Option<Vec<u8>>, WriteError> {
    let definition = match lookup_definition(registry, &group.tag) {
        Some(def) => def,
        None => return Ok(None),
    };

    let mut writer = StructureWriter::new(Arc::clone(&definition));

    // Write fields in definition order by iterating the definition's fields
    // and looking up values from the group
    write_fields_to_writer(&mut writer, &definition, &group.fields)?;

    // Finish and return the serialized bytes
    let cedata = writer.finish()?;
    Ok(Some(cedata))
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
            } else if let Some(f) = n.as_f64() {
                Some(WriteValue::Float(f))
            } else {
                None
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
    fields: &std::collections::HashMap<String, serde_json::Value>,
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
                        let nested_fields: std::collections::HashMap<String, serde_json::Value> =
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
            let fields: std::collections::HashMap<String, serde_json::Value> =
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
    fields: &std::collections::HashMap<String, serde_json::Value>,
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

/// Serialize a TRE field group to a TreEnvelope.
///
/// This is a convenience function that combines `serialize_tre_fields` with
/// `TreEnvelope::new` to create a complete TRE envelope.
///
/// # Arguments
///
/// * `registry` - The structure registry containing TRE definitions
/// * `group` - The TRE field group containing field values to serialize
///
/// # Returns
///
/// * `Ok(Some(envelope))` - If a definition exists and serialization succeeded
/// * `Ok(None)` - If no definition exists for this TRE tag (unknown TRE)
/// * `Err` - If serialization or envelope creation failed
///
/// # Example
///
/// ```ignore
/// let mut group = TreFieldGroup::new("GEOLOB");
/// group.insert("arv", json!("000360000"));
///
/// if let Some(envelope) = serialize_tre_to_envelope(&registry, &group)? {
///     // Use the envelope...
/// }
/// ```
pub fn serialize_tre_to_envelope(
    registry: &StructureRegistry,
    group: &TreFieldGroup,
) -> Result<Option<TreEnvelope>, SerializeTreError> {
    // Serialize the fields to CEDATA
    let cedata = match serialize_tre_fields(registry, group)? {
        Some(data) => data,
        None => return Ok(None),
    };

    // Create the envelope
    let envelope =
        TreEnvelope::new(&group.tag, cedata).map_err(|e| SerializeTreError::InvalidTag {
            tag: group.tag.clone(),
            source: e,
        })?;

    Ok(Some(envelope))
}

/// Serialize multiple TRE field groups to TreEnvelopes.
///
/// Iterates over all groups and serializes those with known definitions.
/// Unknown TREs (those without definitions) are skipped.
///
/// # Arguments
///
/// * `registry` - The structure registry containing TRE definitions
/// * `groups` - A map of CETAG to TreFieldGroup
///
/// # Returns
///
/// A vector of TreEnvelopes for all successfully serialized TREs.
/// Unknown TREs are silently skipped.
///
/// # Example
///
/// ```ignore
/// let groups = parse_tre_fields_from_metadata(&metadata);
/// let envelopes = serialize_tre_groups_to_envelopes(&registry, &groups)?;
/// ```
pub fn serialize_tre_groups_to_envelopes(
    registry: &StructureRegistry,
    groups: &std::collections::HashMap<String, TreFieldGroup>,
) -> Result<Vec<TreEnvelope>, SerializeTreError> {
    let mut envelopes = Vec::new();

    for group in groups.values() {
        if let Some(envelope) = serialize_tre_to_envelope(registry, group)? {
            envelopes.push(envelope);
        }
    }

    Ok(envelopes)
}

/// Error type for TRE serialization operations.
#[derive(Debug, thiserror::Error)]
pub enum SerializeTreError {
    /// Error writing TRE fields
    #[error("Failed to serialize TRE fields: {0}")]
    WriteError(#[from] WriteError),

    /// Invalid CETAG format
    #[error("Invalid CETAG '{tag}': {source}")]
    InvalidTag {
        tag: String,
        #[source]
        source: super::error::JBPError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Encoding, FieldDefinition, FieldType, SizeSpec, StructureDefinition};

    /// Create a simple GEOLOB-like TRE definition for testing
    fn create_test_geolob_definition() -> StructureDefinition {
        StructureDefinition::new("tre_geolob")
            .with_title("Geographic Location TRE")
            .with_field(
                FieldDefinition::new("ARV", FieldType::String)
                    .with_size(SizeSpec::Fixed(9))
                    .with_encoding(Encoding::BcsN)
                    .with_doc("Longitude density"),
            )
            .with_field(
                FieldDefinition::new("BRV", FieldType::String)
                    .with_size(SizeSpec::Fixed(9))
                    .with_encoding(Encoding::BcsN)
                    .with_doc("Latitude density"),
            )
            .with_field(
                FieldDefinition::new("LSO", FieldType::String)
                    .with_size(SizeSpec::Fixed(15))
                    .with_encoding(Encoding::BcsN)
                    .with_doc("Longitude of reference origin"),
            )
            .with_field(
                FieldDefinition::new("PSO", FieldType::String)
                    .with_size(SizeSpec::Fixed(15))
                    .with_encoding(Encoding::BcsN)
                    .with_doc("Latitude of reference origin"),
            )
    }

    #[test]
    fn has_definition_returns_true_for_known_tre() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        assert!(has_definition(&registry, "GEOLOB"));
    }

    #[test]
    fn has_definition_returns_false_for_unknown_tre() {
        let registry = StructureRegistry::new();

        assert!(!has_definition(&registry, "UNKNOWN"));
    }

    #[test]
    fn has_definition_handles_whitespace_in_tag() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        // Tag with trailing spaces (common in NITF)
        assert!(has_definition(&registry, "GEOLOB"));
        assert!(has_definition(&registry, "GEOLOB "));
        assert!(has_definition(&registry, " GEOLOB"));
        assert!(has_definition(&registry, " GEOLOB "));
    }

    #[test]
    fn has_definition_is_case_insensitive() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        assert!(has_definition(&registry, "GEOLOB"));
        assert!(has_definition(&registry, "geolob"));
        assert!(has_definition(&registry, "GeoLob"));
    }

    #[test]
    fn create_accessor_returns_none_for_unknown_tre() {
        let registry = StructureRegistry::new();
        let cedata = b"test data";

        let result = create_accessor(&registry, "UNKNOWN", cedata).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn create_accessor_returns_accessor_for_known_tre() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        // Create CEDATA matching the GEOLOB structure (9 + 9 + 15 + 15 = 48 bytes)
        let cedata = b"000360000000360000+000.000000000+00.0000000000";

        let result = create_accessor(&registry, "GEOLOB", cedata).unwrap();
        assert!(result.is_some());

        let accessor = result.unwrap();
        assert_eq!(accessor.definition().id, "tre_geolob");
    }

    #[test]
    fn create_accessor_handles_whitespace_in_tag() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        let cedata = b"000360000000360000+000.000000000+00.0000000000";

        // Tag with trailing spaces
        let result = create_accessor(&registry, "GEOLOB ", cedata).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn create_accessor_is_case_insensitive() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        let cedata = b"000360000000360000+000.000000000+00.0000000000";

        // Lowercase tag
        let result = create_accessor(&registry, "geolob", cedata).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn create_accessor_can_read_fields() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_geolob_definition());

        // Create CEDATA with known values
        // ARV: "000360000" (9 bytes)
        // BRV: "000360000" (9 bytes)
        // LSO: "+000.000000000" (15 bytes)
        // PSO: "+00.0000000000" (15 bytes)
        let cedata = b"000360000000360000+000.000000000+00.0000000000";

        let accessor = create_accessor(&registry, "GEOLOB", cedata)
            .unwrap()
            .unwrap();

        // Read the ARV field
        let arv = accessor.get("ARV").unwrap();
        assert_eq!(arv.as_str().unwrap(), "000360000");

        // Read the BRV field
        let brv = accessor.get("BRV").unwrap();
        assert_eq!(brv.as_str().unwrap(), "000360000");
    }

    /// Create a TRE definition with a repeated scalar field (like RPC00B coefficients)
    fn create_test_repeated_scalar_definition() -> StructureDefinition {
        use crate::parser::{Expression, RepeatSpec};

        StructureDefinition::new("tre_rpctest")
            .with_title("RPC-like TRE with repeated scalars")
            .with_field(
                FieldDefinition::new("COUNT", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("COEFFS", FieldType::String)
                    .with_size(SizeSpec::Fixed(6))
                    .with_encoding(Encoding::BcsA)
                    .with_repeat(RepeatSpec::Expression(Expression::MethodCall {
                        target: Box::new(Expression::FieldRef("COUNT".to_string())),
                        method: "to_i".to_string(),
                    })),
            )
    }

    #[test]
    fn serialize_scalar_array_roundtrip() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_rpctest", create_test_repeated_scalar_definition());

        // Build CEDATA: COUNT=03, then 3 x 6-byte coefficients
        let cedata = b"03COEF01COEF02COEF03";

        // Read with accessor
        let accessor = create_accessor(&registry, "RPCTEST", cedata)
            .unwrap()
            .unwrap();
        let count_val = accessor.get("COUNT").unwrap();
        assert_eq!(count_val.as_str().unwrap(), "03");
        let coeffs = accessor.get("COEFFS").unwrap();
        match coeffs {
            crate::parser::Value::Array(arr) => assert_eq!(arr.len(), 3),
            _ => panic!("Expected array"),
        }

        // Build a TreFieldGroup with the array
        let mut group = TreFieldGroup::new("RPCTEST");
        group.insert("COUNT", serde_json::json!("03"));
        group.insert("COEFFS", serde_json::json!(["COEF01", "COEF02", "COEF03"]));

        // Serialize
        let result = serialize_tre_fields(&registry, &group).unwrap();
        assert!(result.is_some());
        let serialized = result.unwrap();

        // Verify byte-identical output
        assert_eq!(serialized, cedata.to_vec());
    }

    /// Create a TRE definition with nested repeated types (like J2KLRA layers)
    fn create_test_nested_type_definition() -> StructureDefinition {
        use crate::parser::{Expression, RepeatSpec};

        let layer_info = StructureDefinition::new("layer_info")
            .with_field(
                FieldDefinition::new("LAYER_ID", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("BITRATE", FieldType::String)
                    .with_size(SizeSpec::Fixed(9))
                    .with_encoding(Encoding::BcsA),
            );

        StructureDefinition::new("tre_j2ktest")
            .with_title("J2KLRA-like TRE with nested types")
            .with_field(
                FieldDefinition::new("NLAYERS", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("LAYERS", FieldType::TypeRef("layer_info".to_string()))
                    .with_size(SizeSpec::Fixed(12)) // 3 + 9 = 12 bytes per layer
                    .with_repeat(RepeatSpec::Expression(Expression::MethodCall {
                        target: Box::new(Expression::FieldRef("NLAYERS".to_string())),
                        method: "to_i".to_string(),
                    })),
            )
            .with_type("layer_info", layer_info)
    }

    #[test]
    fn serialize_nested_object_array_roundtrip() {
        let mut registry = StructureRegistry::new();
        registry.register("tre_j2ktest", create_test_nested_type_definition());

        // Build CEDATA: NLAYERS=02, then 2 x layer_info (3 + 9 = 12 bytes each)
        let cedata = b"002001RATE00001002RATE00002";

        // Read with accessor to verify parsing works
        let accessor = create_accessor(&registry, "J2KTEST", cedata)
            .unwrap()
            .unwrap();
        let nlayers = accessor.get("NLAYERS").unwrap();
        assert_eq!(nlayers.as_str().unwrap(), "002");

        // Build a TreFieldGroup with nested objects
        let mut group = TreFieldGroup::new("J2KTEST");
        group.insert("NLAYERS", serde_json::json!("002"));
        group.insert(
            "LAYERS",
            serde_json::json!([
                {"LAYER_ID": "001", "BITRATE": "RATE00001"},
                {"LAYER_ID": "002", "BITRATE": "RATE00002"}
            ]),
        );

        // Serialize
        let result = serialize_tre_fields(&registry, &group).unwrap();
        assert!(result.is_some());
        let serialized = result.unwrap();

        // Verify byte-identical output
        assert_eq!(serialized, cedata.to_vec());
    }

    /// Create a TRE definition with deeply nested repeats (like HISTOA events with IPCOMS)
    fn create_test_deeply_nested_definition() -> StructureDefinition {
        use crate::parser::{Expression, RepeatSpec};

        let ipcom_type = StructureDefinition::new("ipcom_entry").with_field(
            FieldDefinition::new("COMMENT", FieldType::String)
                .with_size(SizeSpec::Fixed(10))
                .with_encoding(Encoding::BcsA),
        );

        let event_type = StructureDefinition::new("processing_event")
            .with_field(
                FieldDefinition::new("PDATE", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_encoding(Encoding::BcsA),
            )
            .with_field(
                FieldDefinition::new("NIPCOM", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("IPCOMS", FieldType::TypeRef("ipcom_entry".to_string()))
                    .with_size(SizeSpec::Fixed(10))
                    .with_repeat(RepeatSpec::Expression(Expression::MethodCall {
                        target: Box::new(Expression::FieldRef("NIPCOM".to_string())),
                        method: "to_i".to_string(),
                    })),
            )
            .with_type("ipcom_entry", ipcom_type);

        StructureDefinition::new("tre_histtest")
            .with_title("HISTOA-like TRE with deeply nested repeats")
            .with_field(
                FieldDefinition::new("NEVENTS", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("EVENTS", FieldType::TypeRef("processing_event".to_string()))
                    .with_size(SizeSpec::Expression(Expression::Literal(
                        crate::parser::Literal::Integer(0),
                    )))
                    .with_repeat(RepeatSpec::Expression(Expression::MethodCall {
                        target: Box::new(Expression::FieldRef("NEVENTS".to_string())),
                        method: "to_i".to_string(),
                    })),
            )
            .with_type("processing_event", event_type)
    }

    #[test]
    fn serialize_deeply_nested_roundtrip() {
        let mut registry = StructureRegistry::new();
        let def = create_test_deeply_nested_definition();
        registry.register("tre_histtest", def);

        // Build CEDATA:
        // NEVENTS=01
        // Event 0: PDATE=20240101, NIPCOM=02, IPCOM[0]=COMMENT001, IPCOM[1]=COMMENT002
        let cedata = b"0120240101\
02\
COMMENT001\
COMMENT002";

        // Build a TreFieldGroup with deeply nested objects
        let mut group = TreFieldGroup::new("HISTTEST");
        group.insert("NEVENTS", serde_json::json!("01"));
        group.insert(
            "EVENTS",
            serde_json::json!([
                {
                    "PDATE": "20240101",
                    "NIPCOM": "02",
                    "IPCOMS": [
                        {"COMMENT": "COMMENT001"},
                        {"COMMENT": "COMMENT002"}
                    ]
                }
            ]),
        );

        // Serialize
        let result = serialize_tre_fields(&registry, &group).unwrap();
        assert!(result.is_some());
        let serialized = result.unwrap();

        // Verify byte-identical output
        assert_eq!(
            std::str::from_utf8(&serialized).unwrap(),
            std::str::from_utf8(cedata).unwrap()
        );
    }

    #[test]
    fn serialize_unknown_tre_returns_none() {
        let registry = StructureRegistry::new();
        let group = TreFieldGroup::new("UNKNOWN");
        let result = serialize_tre_fields(&registry, &group).unwrap();
        assert!(result.is_none());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::parser::{
        Encoding, Expression, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
    };
    use proptest::prelude::*;

    /// Strategy to generate valid BCS-A alphanumeric strings without trailing spaces
    /// (since BCS-A strings are right-trimmed when parsed)
    fn bcsa_string_no_trailing_space(len: usize) -> impl Strategy<Value = String> {
        // Generate alphanumeric characters (no spaces to avoid trimming issues)
        prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), len)
            .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    /// Create a simple TRE definition with fixed-size string fields for testing
    fn create_simple_tre_definition(field_sizes: &[(String, usize)]) -> StructureDefinition {
        let mut def = StructureDefinition::new("tre_test");
        for (name, size) in field_sizes {
            def = def.with_field(
                FieldDefinition::new(name.clone(), FieldType::String)
                    .with_size(SizeSpec::Fixed(*size))
                    .with_encoding(Encoding::BcsA),
            );
        }
        def
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Feature: tre-des-support, Property 3: Known TRE Field Extraction
        ///
        /// For any TRE with a known definition, parsing the CEDATA SHALL produce
        /// a field map where each field path returns the correct value according
        /// to the definition.
        ///
        /// **Validates: Requirements 2.1, 2.2, 2.4, 2.5**
        #[test]
        fn prop_3_known_tre_field_extraction(
            field1_value in bcsa_string_no_trailing_space(10),
            field2_value in bcsa_string_no_trailing_space(8),
            field3_value in bcsa_string_no_trailing_space(15),
        ) {
            // Create a TRE definition with known field sizes
            let field_sizes = vec![
                ("field1".to_string(), 10usize),
                ("field2".to_string(), 8usize),
                ("field3".to_string(), 15usize),
            ];
            let definition = create_simple_tre_definition(&field_sizes);

            // Register the definition
            let mut registry = StructureRegistry::new();
            registry.register("tre_test", definition);

            // Build CEDATA from the field values
            let mut cedata = Vec::new();
            cedata.extend(field1_value.as_bytes());
            cedata.extend(field2_value.as_bytes());
            cedata.extend(field3_value.as_bytes());

            // Create accessor using tre_fields module
            let accessor = create_accessor(&registry, "TEST", &cedata)
                .expect("Should not error")
                .expect("Definition should exist");

            // Verify each field can be extracted and matches the input value
            let extracted_field1 = accessor.get("field1")
                .expect("field1 should be accessible");
            prop_assert_eq!(
                extracted_field1.as_str().unwrap(),
                &field1_value,
                "field1 value should match input"
            );

            let extracted_field2 = accessor.get("field2")
                .expect("field2 should be accessible");
            prop_assert_eq!(
                extracted_field2.as_str().unwrap(),
                &field2_value,
                "field2 value should match input"
            );

            let extracted_field3 = accessor.get("field3")
                .expect("field3 should be accessible");
            prop_assert_eq!(
                extracted_field3.as_str().unwrap(),
                &field3_value,
                "field3 value should match input"
            );
        }

        /// Feature: metadata-restructure, Property 3 (Extended): Repeated field extraction
        ///
        /// For any TRE with repeated fields, accessing the field SHALL return a
        /// Value::Array, and indexing into that array SHALL return the correct values.
        ///
        /// **Validates: Requirements 2.4, 2.5**
        #[test]
        fn prop_3_repeated_field_extraction(
            values in prop::collection::vec(bcsa_string_no_trailing_space(5), 1..=5),
        ) {
            // Create a TRE definition with a repeated field
            let count = values.len();

            // Create repeat expression that references the count field
            let repeat_expr = Expression::MethodCall {
                target: Box::new(Expression::FieldRef("count".to_string())),
                method: "to_i".to_string(),
            };

            let def = StructureDefinition::new("tre_repeat_test")
                .with_field(
                    FieldDefinition::new("count", FieldType::String)
                        .with_size(SizeSpec::Fixed(3))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("items", FieldType::String)
                        .with_size(SizeSpec::Fixed(5))
                        .with_encoding(Encoding::BcsA)
                        .with_repeat(RepeatSpec::Expression(repeat_expr)),
                );

            // Register the definition
            let mut registry = StructureRegistry::new();
            registry.register("tre_repeat_test", def);

            // Build CEDATA: count field (3 bytes) + repeated items (5 bytes each)
            let mut cedata = Vec::new();
            cedata.extend(format!("{:03}", count).as_bytes());
            for value in &values {
                cedata.extend(value.as_bytes());
            }

            // Create accessor
            let accessor = create_accessor(&registry, "REPEAT_TEST", &cedata)
                .expect("Should not error")
                .expect("Definition should exist");

            // Verify count field
            let extracted_count = accessor.get("count")
                .expect("count should be accessible");
            prop_assert_eq!(
                extracted_count.as_str().unwrap(),
                &format!("{:03}", count),
                "count value should match"
            );

            // Verify repeated field returns Value::Array
            let items_value = accessor.get("items")
                .expect("items should be accessible");
            match items_value {
                crate::parser::Value::Array(arr) => {
                    prop_assert_eq!(arr.len(), count, "Array length should match count");
                    for (i, expected_value) in values.iter().enumerate() {
                        prop_assert_eq!(
                            arr[i].as_str().unwrap(),
                            expected_value,
                            "items[{}] value should match input", i
                        );
                    }
                }
                other => {
                    prop_assert!(false, "Expected Value::Array, got {:?}", other);
                }
            }
        }
    }
}
