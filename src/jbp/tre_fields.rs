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
//! use aws_osml_io::jbp::tre_fields;
//! use aws_osml_io::parser::StructureRegistry;
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

use crate::parser::{AccessError, StructureAccessor, StructureRegistry, StructureWriter, WriteError};
use super::tre::{TreEnvelope, TreFieldGroup};

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
    // Normalize the tag: trim whitespace and convert to lowercase
    let normalized_tag = tag.trim().to_lowercase();
    
    // Build the definition name using the pattern TRE_{CETAG}
    let def_name = format!("tre_{}", normalized_tag);
    
    // Look up the definition in the registry
    let definition = match registry.get(&def_name) {
        Some(def) => def,
        None => return Ok(None), // Unknown TRE - no definition exists
    };
    
    // Create and return the accessor
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
    // Normalize the tag: trim whitespace and convert to lowercase
    let normalized_tag = tag.trim().to_lowercase();
    
    // Build the definition name using the pattern TRE_{CETAG}
    let def_name = format!("tre_{}", normalized_tag);
    
    // Check if the definition exists
    registry.get(&def_name).is_some()
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
    // Normalize the tag: trim whitespace and convert to lowercase
    let normalized_tag = group.tag.trim().to_lowercase();
    
    // Build the definition name using the pattern TRE_{CETAG}
    let def_name = format!("tre_{}", normalized_tag);
    
    // Look up the definition in the registry
    let definition = match registry.get(&def_name) {
        Some(def) => def,
        None => return Ok(None), // Unknown TRE - no definition exists
    };
    
    // Create a writer for the TRE structure
    let mut writer = StructureWriter::new_fixed(definition)?;
    
    // Write each field from the group
    for (field_name, value) in &group.fields {
        // Convert field name to lowercase for matching
        let normalized_field = field_name.to_lowercase();
        
        // Convert JSON value to WriteValue
        match value {
            serde_json::Value::String(s) => {
                writer.set(&normalized_field, s.as_str())?;
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    writer.set(&normalized_field, i)?;
                } else if let Some(u) = n.as_u64() {
                    writer.set(&normalized_field, u)?;
                } else if let Some(f) = n.as_f64() {
                    writer.set(&normalized_field, f)?;
                }
            }
            serde_json::Value::Bool(b) => {
                // Convert bool to string "0" or "1"
                writer.set(&normalized_field, if *b { "1" } else { "0" })?;
            }
            _ => {
                // Skip null, arrays, and objects for now
            }
        }
    }
    
    // Finish and return the serialized bytes
    let cedata = writer.finish()?;
    Ok(Some(cedata))
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
    let envelope = TreEnvelope::new(&group.tag, cedata)
        .map_err(|e| SerializeTreError::InvalidTag { 
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
}


#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::parser::{
        Encoding, Expression, FieldDefinition, FieldType, RepeatSpec, SizeSpec,
        StructureDefinition,
    };
    use proptest::prelude::*;

    /// Strategy to generate valid BCS-A alphanumeric strings without trailing spaces
    /// (since BCS-A strings are right-trimmed when parsed)
    fn bcsa_string_no_trailing_space(len: usize) -> impl Strategy<Value = String> {
        // Generate alphanumeric characters (no spaces to avoid trimming issues)
        prop::collection::vec(
            prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()),
            len,
        )
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

        /// Feature: tre-des-support, Property 3 (Extended): Repeated field extraction
        ///
        /// For any TRE with repeated fields, accessing elements using underscore-indexed
        /// naming (e.g., "field_0", "field_1") SHALL return the correct values.
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

            // Verify each repeated element using underscore-indexed naming
            for (i, expected_value) in values.iter().enumerate() {
                let field_path = format!("items_{}", i);
                let extracted = accessor.get(&field_path)
                    .expect(&format!("{} should be accessible", field_path));
                prop_assert_eq!(
                    extracted.as_str().unwrap(),
                    expected_value,
                    "items_{} value should match input", i
                );
            }
        }
    }
}
