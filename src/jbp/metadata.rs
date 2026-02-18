//! Metadata providers for NITF file and segment headers.
//!
//! This module provides metadata provider implementations:
//! - [`JBPFileMetadataProvider`] - File-level metadata from NITF header
//! - [`JBPSegmentMetadataProvider`] - Segment-level metadata from subheaders
//!
//! Both providers implement the [`MetadataProvider`] trait, providing access to
//! parsed field values as dictionaries and raw header bytes.
//!
//! # TRE Support
//!
//! [`JBPSegmentMetadataProvider`] supports TRE (Tagged Record Extension) metadata
//! through the `with_tres()` constructor. TRE fields are exposed in the dictionary
//! interface with CETAG-prefixed keys (e.g., "GEOLOB.ARV").

use std::collections::HashMap;
use std::sync::Arc;

use crate::parser::{StructureAccessor, StructureDefinition, StructureRegistry, Value};
use crate::traits::MetadataProvider;

use super::tre::TreEnvelope;
use super::tre_fields;

/// Metadata provider for NITF file header fields.
///
/// Provides access to file-level metadata from the NITF header through the
/// [`MetadataProvider`] trait. Fields can be accessed as a dictionary with
/// optional prefix-based filtering.
///
/// # Example
///
/// ```ignore
/// let provider = JBPFileMetadataProvider::new(accessor, raw_bytes);
///
/// // Get all fields
/// let all_fields = provider.as_dict(None);
///
/// // Get only security fields (starting with "FS")
/// let security_fields = provider.as_dict(Some("FS"));
/// ```
pub struct JBPFileMetadataProvider {
    /// Structure definition for field enumeration
    definition: Arc<StructureDefinition>,
    /// Raw header bytes
    raw_bytes: Arc<[u8]>,
}

impl JBPFileMetadataProvider {
    /// Create a new file metadata provider.
    ///
    /// # Arguments
    /// * `accessor` - Structure accessor for the parsed file header
    /// * `raw_bytes` - Raw bytes of the file header
    pub fn new(accessor: &StructureAccessor, raw_bytes: Arc<[u8]>) -> Self {
        Self {
            definition: accessor.definition.clone(),
            raw_bytes,
        }
    }

    /// Create from definition and raw bytes directly.
    ///
    /// This is useful when you have the definition but don't need to keep
    /// the accessor around.
    pub fn from_definition(definition: Arc<StructureDefinition>, raw_bytes: Arc<[u8]>) -> Self {
        Self {
            definition,
            raw_bytes,
        }
    }
}


impl MetadataProvider for JBPFileMetadataProvider {
    /// Returns the raw file header bytes.
    fn raw(&self) -> &[u8] {
        &self.raw_bytes
    }

    /// Returns file header fields as a dictionary.
    ///
    /// # Arguments
    /// * `name` - Optional prefix to filter fields. If `Some(prefix)`, only fields
    ///   whose names start with the prefix are returned. If `None`, all fields
    ///   are returned.
    ///
    /// # Returns
    /// A HashMap of field names to JSON values.
    fn as_dict(&self, name: Option<&str>) -> HashMap<String, serde_json::Value> {
        let mut result = HashMap::new();

        // Create a temporary accessor to read values
        if let Ok(accessor) = StructureAccessor::new(self.definition.clone(), &self.raw_bytes) {
            // Iterate over all accessible field paths
            for field_path in accessor.fields() {
                // Apply prefix filter if specified
                if let Some(prefix) = name {
                    if !field_path.starts_with(prefix) {
                        continue;
                    }
                }

                // Try to get the field value and convert to JSON
                if let Ok(value) = accessor.get(&field_path) {
                    if let Some(json_value) = value_to_json(&value) {
                        result.insert(field_path, json_value);
                    }
                }
            }
        }

        result
    }
}

// Ensure JBPFileMetadataProvider is Send + Sync
// Arc<[u8]> and Arc<StructureDefinition> are both Send + Sync
unsafe impl Send for JBPFileMetadataProvider {}
unsafe impl Sync for JBPFileMetadataProvider {}

/// Metadata provider for NITF segment subheader fields.
///
/// Provides access to segment-level metadata from subheaders through the
/// [`MetadataProvider`] trait. Works identically to [`JBPFileMetadataProvider`]
/// but for segment subheaders instead of file headers.
///
/// # TRE Support
///
/// When created with `with_tres()`, this provider also exposes TRE fields in the
/// dictionary interface. TRE fields are prefixed with their CETAG (e.g., "GEOLOB.ARV").
/// Unknown TREs (those without definitions in the registry) are skipped in metadata
/// output but preserved for round-trip operations.
///
/// # Example
///
/// ```ignore
/// let provider = JBPSegmentMetadataProvider::new(accessor, raw_bytes);
///
/// // Get all subheader fields
/// let all_fields = provider.as_dict(None);
///
/// // Get only image-specific fields (starting with "I")
/// let image_fields = provider.as_dict(Some("I"));
///
/// // With TRE support:
/// let provider = JBPSegmentMetadataProvider::with_tres(
///     definition, raw_bytes, tre_envelopes, registry
/// );
///
/// // Get only GEOLOB TRE fields
/// let geolob_fields = provider.as_dict(Some("GEOLOB"));
/// ```
pub struct JBPSegmentMetadataProvider {
    /// Structure definition for field enumeration
    definition: Arc<StructureDefinition>,
    /// Raw subheader bytes
    raw_bytes: Arc<[u8]>,
    /// TRE envelopes from this segment (inline + overflow)
    tre_envelopes: Vec<TreEnvelope>,
    /// Structure registry for TRE definitions
    registry: Option<Arc<StructureRegistry>>,
}

impl JBPSegmentMetadataProvider {
    /// Create a new segment metadata provider.
    ///
    /// # Arguments
    /// * `accessor` - Structure accessor for the parsed segment subheader
    /// * `raw_bytes` - Raw bytes of the segment subheader
    pub fn new(accessor: &StructureAccessor, raw_bytes: Arc<[u8]>) -> Self {
        Self {
            definition: accessor.definition.clone(),
            raw_bytes,
            tre_envelopes: Vec::new(),
            registry: None,
        }
    }

    /// Create from definition and raw bytes directly.
    pub fn from_definition(definition: Arc<StructureDefinition>, raw_bytes: Arc<[u8]>) -> Self {
        Self {
            definition,
            raw_bytes,
            tre_envelopes: Vec::new(),
            registry: None,
        }
    }

    /// Create with TRE support.
    ///
    /// This constructor enables TRE field access through the metadata interface.
    /// TRE fields are exposed with CETAG-prefixed keys (e.g., "GEOLOB.ARV").
    ///
    /// # Arguments
    /// * `definition` - Structure definition for the segment subheader
    /// * `raw_bytes` - Raw bytes of the segment subheader
    /// * `tre_envelopes` - TRE envelopes from this segment (inline + overflow)
    /// * `registry` - Structure registry for TRE definitions
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = JBPSegmentMetadataProvider::with_tres(
    ///     definition,
    ///     raw_bytes,
    ///     tre_envelopes,
    ///     registry,
    /// );
    ///
    /// // Access TRE fields
    /// let dict = provider.as_dict(Some("GEOLOB"));
    /// ```
    pub fn with_tres(
        definition: Arc<StructureDefinition>,
        raw_bytes: Arc<[u8]>,
        tre_envelopes: Vec<TreEnvelope>,
        registry: Arc<StructureRegistry>,
    ) -> Self {
        Self {
            definition,
            raw_bytes,
            tre_envelopes,
            registry: Some(registry),
        }
    }
}

impl MetadataProvider for JBPSegmentMetadataProvider {
    /// Returns the raw subheader bytes.
    fn raw(&self) -> &[u8] {
        &self.raw_bytes
    }

    /// Returns subheader fields as a dictionary.
    ///
    /// # Arguments
    /// * `name` - Optional prefix to filter fields. If `Some(prefix)`, only fields
    ///   whose names start with the prefix are returned. If `None`, all fields
    ///   are returned.
    ///
    /// # TRE Fields
    ///
    /// When TRE support is enabled (via `with_tres()`), TRE fields are included
    /// in the result with CETAG-prefixed keys (e.g., "GEOLOB.ARV"). The prefix
    /// filter applies to TRE fields as well - for example, `as_dict(Some("GEOLOB"))`
    /// returns only fields from the GEOLOB TRE.
    ///
    /// Unknown TREs (those without definitions in the registry) are skipped.
    ///
    /// # Returns
    /// A HashMap of field names to JSON values.
    fn as_dict(&self, name: Option<&str>) -> HashMap<String, serde_json::Value> {
        let mut result = HashMap::new();

        // Create a temporary accessor to read subheader values
        if let Ok(accessor) = StructureAccessor::new(self.definition.clone(), &self.raw_bytes) {
            // Iterate over all accessible field paths
            for field_path in accessor.fields() {
                // Apply prefix filter if specified
                if let Some(prefix) = name {
                    if !field_path.starts_with(prefix) {
                        continue;
                    }
                }

                // Try to get the field value and convert to JSON
                if let Ok(value) = accessor.get(&field_path) {
                    if let Some(json_value) = value_to_json(&value) {
                        result.insert(field_path, json_value);
                    }
                }
            }
        }

        // Add TRE fields if registry is available
        if let Some(ref registry) = self.registry {
            for envelope in &self.tre_envelopes {
                // Normalize the tag (trim whitespace)
                let tag = envelope.tag.trim();

                // Skip if prefix filter doesn't match the CETAG
                if let Some(prefix) = name {
                    // Check if the prefix could match this TRE's fields
                    // Either the prefix is the CETAG itself, or starts with "CETAG."
                    if !tag.starts_with(prefix) && !prefix.starts_with(tag) {
                        continue;
                    }
                }

                // Try to create an accessor for this TRE
                if let Ok(Some(tre_accessor)) =
                    tre_fields::create_accessor(registry, tag, &envelope.data)
                {
                    // Iterate over all TRE fields
                    for field_path in tre_accessor.fields() {
                        // Build the full path with CETAG prefix
                        let full_path = format!("{}.{}", tag, field_path);

                        // Apply prefix filter
                        if let Some(prefix) = name {
                            if !full_path.starts_with(prefix) {
                                continue;
                            }
                        }

                        // Try to get the field value and convert to JSON
                        if let Ok(value) = tre_accessor.get(&field_path) {
                            if let Some(json_value) = value_to_json(&value) {
                                result.insert(full_path, json_value);
                            }
                        }
                    }
                }
                // Unknown TREs (no definition) are silently skipped
            }
        }

        result
    }
}

// Ensure JBPSegmentMetadataProvider is Send + Sync
unsafe impl Send for JBPSegmentMetadataProvider {}
unsafe impl Sync for JBPSegmentMetadataProvider {}


/// Convert a parsed Value to a serde_json::Value.
///
/// This function handles the conversion of all Value variants to their
/// JSON equivalents:
/// - String → JSON string
/// - Bytes → JSON string (hex-encoded if not valid UTF-8)
/// - Unsigned → JSON number
/// - Array → JSON array
/// - Struct → JSON object with type_name and data fields
fn value_to_json(value: &Value) -> Option<serde_json::Value> {
    match value {
        Value::String(cow) => {
            // Trim trailing spaces (standard NITF padding)
            let trimmed = cow.trim_end_matches(' ');
            Some(serde_json::Value::String(trimmed.to_string()))
        }
        Value::Bytes(bytes) => {
            // Try to interpret as UTF-8 string first
            match std::str::from_utf8(bytes) {
                Ok(s) => Some(serde_json::Value::String(s.trim_end_matches(' ').to_string())),
                Err(_) => {
                    // Fall back to hex encoding for binary data
                    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                    Some(serde_json::Value::String(hex))
                }
            }
        }
        Value::Unsigned(n) => Some(serde_json::Value::Number((*n).into())),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> =
                arr.iter().filter_map(value_to_json).collect();
            Some(serde_json::Value::Array(json_arr))
        }
        Value::Struct(struct_val) => {
            // For nested structures, return an object with type info
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
    use crate::parser::{FieldDefinition, FieldType, SizeSpec, StructureDefinition};

    /// Create a simple test structure definition with a few fields.
    fn create_test_definition() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("TestHeader")
                .with_field(
                    FieldDefinition::new("FHDR", FieldType::String)
                        .with_size(SizeSpec::Fixed(4))
                        .with_doc("File type header"),
                )
                .with_field(
                    FieldDefinition::new("FVER", FieldType::String)
                        .with_size(SizeSpec::Fixed(5))
                        .with_doc("File version"),
                )
                .with_field(
                    FieldDefinition::new("FSCLAS", FieldType::String)
                        .with_size(SizeSpec::Fixed(1))
                        .with_doc("File security classification"),
                )
                .with_field(
                    FieldDefinition::new("FSCLSY", FieldType::String)
                        .with_size(SizeSpec::Fixed(2))
                        .with_doc("File security classification system"),
                ),
        )
    }

    /// Create test data matching the test definition.
    fn create_test_data() -> Arc<[u8]> {
        // FHDR(4) + FVER(5) + FSCLAS(1) + FSCLSY(2) = 12 bytes
        Arc::from(b"NITF02.10U  ".as_slice())
    }

    #[test]
    fn file_metadata_provider_raw_returns_bytes() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes.clone());

        assert_eq!(provider.raw(), raw_bytes.as_ref());
    }

    #[test]
    fn file_metadata_provider_as_dict_all_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

        let dict = provider.as_dict(None);

        // Should have all 4 fields
        assert!(dict.contains_key("FHDR"));
        assert!(dict.contains_key("FVER"));
        assert!(dict.contains_key("FSCLAS"));
        assert!(dict.contains_key("FSCLSY"));

        // Check values
        assert_eq!(dict.get("FHDR"), Some(&serde_json::json!("NITF")));
        assert_eq!(dict.get("FVER"), Some(&serde_json::json!("02.10")));
        assert_eq!(dict.get("FSCLAS"), Some(&serde_json::json!("U")));
    }

    #[test]
    fn file_metadata_provider_as_dict_with_prefix() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

        // Filter by "FS" prefix (security fields)
        let dict = provider.as_dict(Some("FS"));

        // Should only have security fields
        assert!(dict.contains_key("FSCLAS"));
        assert!(dict.contains_key("FSCLSY"));
        assert!(!dict.contains_key("FHDR"));
        assert!(!dict.contains_key("FVER"));
    }

    #[test]
    fn file_metadata_provider_as_dict_with_nonmatching_prefix() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

        // Filter by prefix that matches nothing
        let dict = provider.as_dict(Some("XYZ"));

        assert!(dict.is_empty());
    }

    #[test]
    fn segment_metadata_provider_raw_returns_bytes() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes.clone());

        assert_eq!(provider.raw(), raw_bytes.as_ref());
    }

    #[test]
    fn segment_metadata_provider_as_dict_all_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

        let dict = provider.as_dict(None);

        // Should have all 4 fields
        assert!(dict.contains_key("FHDR"));
        assert!(dict.contains_key("FVER"));
        assert!(dict.contains_key("FSCLAS"));
        assert!(dict.contains_key("FSCLSY"));
    }

    #[test]
    fn segment_metadata_provider_as_dict_with_prefix() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

        // Filter by "F" prefix
        let dict = provider.as_dict(Some("F"));

        // All fields start with F in our test
        assert_eq!(dict.len(), 4);
    }

    #[test]
    fn value_to_json_string() {
        let value = Value::from_str("HELLO   ");
        let json = value_to_json(&value).unwrap();
        assert_eq!(json, serde_json::json!("HELLO"));
    }

    #[test]
    fn value_to_json_unsigned() {
        let value = Value::from_unsigned(42);
        let json = value_to_json(&value).unwrap();
        assert_eq!(json, serde_json::json!(42));
    }

    #[test]
    fn value_to_json_bytes_utf8() {
        let value = Value::from_bytes(b"WORLD   ");
        let json = value_to_json(&value).unwrap();
        assert_eq!(json, serde_json::json!("WORLD"));
    }

    #[test]
    fn value_to_json_bytes_binary() {
        let value = Value::from_bytes(&[0xFF, 0x00, 0xAB]);
        let json = value_to_json(&value).unwrap();
        assert_eq!(json, serde_json::json!("ff00ab"));
    }

    #[test]
    fn value_to_json_array() {
        let value = Value::from_array(vec![
            Value::from_unsigned(1),
            Value::from_unsigned(2),
            Value::from_unsigned(3),
        ]);
        let json = value_to_json(&value).unwrap();
        assert_eq!(json, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn value_to_json_struct() {
        let value = Value::from_struct(b"data", "TestType");
        let json = value_to_json(&value).unwrap();
        
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("_type"), Some(&serde_json::json!("TestType")));
        assert_eq!(obj.get("_data"), Some(&serde_json::json!("64617461")));
    }

    // TRE support tests

    /// Create a simple TRE definition for testing
    fn create_test_tre_definition() -> StructureDefinition {
        use crate::parser::Encoding;
        StructureDefinition::new("tre_geolob")
            .with_title("Geographic Location TRE")
            .with_field(
                FieldDefinition::new("arv", FieldType::String)
                    .with_size(SizeSpec::Fixed(9))
                    .with_encoding(Encoding::BcsN)
                    .with_doc("Longitude density"),
            )
            .with_field(
                FieldDefinition::new("brv", FieldType::String)
                    .with_size(SizeSpec::Fixed(9))
                    .with_encoding(Encoding::BcsN)
                    .with_doc("Latitude density"),
            )
    }

    #[test]
    fn segment_provider_with_tres_includes_tre_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();

        // Create a TRE envelope with GEOLOB data
        // ARV: "000360000" (9 bytes), BRV: "000360000" (9 bytes)
        let tre_data = b"000360000000360000".to_vec();
        let tre_envelope = TreEnvelope {
            tag: "GEOLOB".to_string(),
            data: tre_data,
        };

        // Create registry with TRE definition
        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_tre_definition());

        let provider = JBPSegmentMetadataProvider::with_tres(
            definition,
            raw_bytes,
            vec![tre_envelope],
            Arc::new(registry),
        );

        let dict = provider.as_dict(None);

        // Should have subheader fields
        assert!(dict.contains_key("FHDR"));
        assert!(dict.contains_key("FVER"));

        // Should have TRE fields with CETAG prefix
        assert!(dict.contains_key("GEOLOB.arv"));
        assert!(dict.contains_key("GEOLOB.brv"));
        assert_eq!(dict.get("GEOLOB.arv"), Some(&serde_json::json!("000360000")));
        assert_eq!(dict.get("GEOLOB.brv"), Some(&serde_json::json!("000360000")));
    }

    #[test]
    fn segment_provider_with_tres_filters_by_cetag() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();

        let tre_data = b"000360000000360000".to_vec();
        let tre_envelope = TreEnvelope {
            tag: "GEOLOB".to_string(),
            data: tre_data,
        };

        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_tre_definition());

        let provider = JBPSegmentMetadataProvider::with_tres(
            definition,
            raw_bytes,
            vec![tre_envelope],
            Arc::new(registry),
        );

        // Filter by GEOLOB prefix - should only get TRE fields
        let dict = provider.as_dict(Some("GEOLOB"));

        assert!(dict.contains_key("GEOLOB.arv"));
        assert!(dict.contains_key("GEOLOB.brv"));
        assert!(!dict.contains_key("FHDR"));
        assert!(!dict.contains_key("FVER"));
        assert_eq!(dict.len(), 2);
    }

    #[test]
    fn segment_provider_skips_unknown_tres() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();

        // Create a TRE with unknown tag (no definition in registry)
        let unknown_tre = TreEnvelope {
            tag: "UNKNWN".to_string(),
            data: vec![1, 2, 3, 4, 5],
        };

        // Empty registry - no TRE definitions
        let registry = StructureRegistry::new();

        let provider = JBPSegmentMetadataProvider::with_tres(
            definition,
            raw_bytes,
            vec![unknown_tre],
            Arc::new(registry),
        );

        let dict = provider.as_dict(None);

        // Should have subheader fields but no TRE fields
        assert!(dict.contains_key("FHDR"));
        assert!(!dict.contains_key("UNKNWN."));
        // No keys should start with UNKNWN
        for key in dict.keys() {
            assert!(!key.starts_with("UNKNWN"), "Unexpected TRE field: {}", key);
        }
    }

    #[test]
    fn segment_provider_without_tres_has_no_tre_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();

        // Create provider without TRE support
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

        let dict = provider.as_dict(None);

        // Should only have subheader fields
        assert!(dict.contains_key("FHDR"));
        assert_eq!(dict.len(), 4); // Only the 4 subheader fields
    }

    #[test]
    fn segment_provider_handles_multiple_tres() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();

        // Create two TRE envelopes
        let tre1 = TreEnvelope {
            tag: "GEOLOB".to_string(),
            data: b"000360000000360000".to_vec(),
        };

        // Create a second TRE definition
        let tre2_def = StructureDefinition::new("tre_test")
            .with_field(
                FieldDefinition::new("value", FieldType::String)
                    .with_size(SizeSpec::Fixed(5)),
            );

        let tre2 = TreEnvelope {
            tag: "TEST  ".to_string(),
            data: b"HELLO".to_vec(),
        };

        let mut registry = StructureRegistry::new();
        registry.register("tre_geolob", create_test_tre_definition());
        registry.register("tre_test", tre2_def);

        let provider = JBPSegmentMetadataProvider::with_tres(
            definition,
            raw_bytes,
            vec![tre1, tre2],
            Arc::new(registry),
        );

        let dict = provider.as_dict(None);

        // Should have fields from both TREs
        assert!(dict.contains_key("GEOLOB.arv"));
        assert!(dict.contains_key("TEST.value"));
        assert_eq!(dict.get("TEST.value"), Some(&serde_json::json!("HELLO")));
    }
}


/// Property-based tests for metadata providers.
///
/// These tests verify the correctness properties defined in the design document:
/// - Property 8: Metadata Prefix Filtering
/// - Property 9: Raw Metadata Identity
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::parser::{FieldDefinition, FieldType, SizeSpec, StructureDefinition};
    use proptest::prelude::*;

    /// Create a structure definition with the given field names.
    fn create_definition_with_fields(field_names: &[String]) -> Arc<StructureDefinition> {
        let mut def = StructureDefinition::new("TestStruct");
        for name in field_names {
            def = def.with_field(
                FieldDefinition::new(name.clone(), FieldType::String)
                    .with_size(SizeSpec::Fixed(10)),
            );
        }
        Arc::new(def)
    }

    /// Create raw bytes for the given field values (each 10 bytes, space-padded).
    fn create_raw_bytes(values: &[String]) -> Arc<[u8]> {
        let mut bytes = Vec::new();
        for value in values {
            let mut field_bytes = value.as_bytes().to_vec();
            // Pad to 10 bytes with spaces
            field_bytes.resize(10, b' ');
            bytes.extend_from_slice(&field_bytes);
        }
        Arc::from(bytes)
    }

    /// Property 8: Metadata Prefix Filtering
    /// For any MetadataProvider and any prefix string, `as_dict(Some(prefix))` SHALL
    /// return only entries whose keys start with the given prefix, and `as_dict(None)`
    /// SHALL return all entries.
    /// **Validates: Requirements 5.2, 5.3, 6.5**
    mod prop_8_metadata_prefix_filtering {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// as_dict(None) returns all fields
            #[test]
            fn as_dict_none_returns_all_fields(
                num_fields in 1usize..5,
            ) {
                // Generate unique field names
                let field_names: Vec<String> = (0..num_fields)
                    .map(|i| format!("FIELD{}", i))
                    .collect();
                let field_values: Vec<String> = (0..num_fields)
                    .map(|i| format!("VALUE{}", i))
                    .collect();

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

                let dict = provider.as_dict(None);

                // Should have all fields
                prop_assert_eq!(dict.len(), num_fields,
                    "Expected {} fields, got {}", num_fields, dict.len());

                for name in &field_names {
                    prop_assert!(dict.contains_key(name),
                        "Missing field: {}", name);
                }
            }

            /// as_dict(Some(prefix)) returns only matching fields
            #[test]
            fn as_dict_with_prefix_filters_correctly(
                num_matching in 1usize..4,
                num_nonmatching in 1usize..4,
            ) {
                // Create fields with "FS" prefix (matching) and "OT" prefix (non-matching)
                let mut field_names = Vec::new();
                let mut field_values = Vec::new();

                for i in 0..num_matching {
                    field_names.push(format!("FS{}", i));
                    field_values.push(format!("MATCH{}", i));
                }
                for i in 0..num_nonmatching {
                    field_names.push(format!("OT{}", i));
                    field_values.push(format!("OTHER{}", i));
                }

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

                // Filter by "FS" prefix
                let dict = provider.as_dict(Some("FS"));

                // Should only have matching fields
                prop_assert_eq!(dict.len(), num_matching,
                    "Expected {} matching fields, got {}", num_matching, dict.len());

                for (name, _) in &dict {
                    prop_assert!(name.starts_with("FS"),
                        "Field '{}' should start with 'FS'", name);
                }
            }

            /// as_dict with non-matching prefix returns empty
            #[test]
            fn as_dict_nonmatching_prefix_returns_empty(
                num_fields in 1usize..5,
            ) {
                let field_names: Vec<String> = (0..num_fields)
                    .map(|i| format!("FIELD{}", i))
                    .collect();
                let field_values: Vec<String> = (0..num_fields)
                    .map(|i| format!("VALUE{}", i))
                    .collect();

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

                // Filter by prefix that matches nothing
                let dict = provider.as_dict(Some("XYZ"));

                prop_assert!(dict.is_empty(),
                    "Expected empty dict, got {} entries", dict.len());
            }

            /// Prefix filtering is case-sensitive
            #[test]
            fn prefix_filtering_is_case_sensitive(
                num_upper in 1usize..3,
                num_lower in 1usize..3,
            ) {
                let mut field_names = Vec::new();
                let mut field_values = Vec::new();

                // Uppercase fields
                for i in 0..num_upper {
                    field_names.push(format!("ABC{}", i));
                    field_values.push(format!("UPPER{}", i));
                }
                // We can't actually create lowercase field names in NITF,
                // but we can test that the prefix match is exact
                for i in 0..num_lower {
                    field_names.push(format!("DEF{}", i));
                    field_values.push(format!("OTHER{}", i));
                }

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

                // Filter by "ABC" should only get uppercase
                let dict = provider.as_dict(Some("ABC"));
                prop_assert_eq!(dict.len(), num_upper);

                // Filter by "abc" should get nothing (case-sensitive)
                let dict_lower = provider.as_dict(Some("abc"));
                prop_assert!(dict_lower.is_empty());
            }

            /// Segment metadata provider has same prefix filtering behavior
            #[test]
            fn segment_provider_prefix_filtering(
                num_matching in 1usize..4,
                num_nonmatching in 1usize..4,
            ) {
                let mut field_names = Vec::new();
                let mut field_values = Vec::new();

                for i in 0..num_matching {
                    field_names.push(format!("IM{}", i));
                    field_values.push(format!("IMAGE{}", i));
                }
                for i in 0..num_nonmatching {
                    field_names.push(format!("TX{}", i));
                    field_values.push(format!("TEXT{}", i));
                }

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

                // Filter by "IM" prefix
                let dict = provider.as_dict(Some("IM"));

                prop_assert_eq!(dict.len(), num_matching);
                for (name, _) in &dict {
                    prop_assert!(name.starts_with("IM"));
                }
            }
        }
    }

    /// Property 9: Raw Metadata Identity
    /// For any MetadataProvider, the bytes returned by `raw()` SHALL be identical
    /// to the original header/subheader bytes from the file.
    /// **Validates: Requirements 5.4**
    mod prop_9_raw_metadata_identity {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// raw() returns exact input bytes for file metadata
            #[test]
            fn file_provider_raw_returns_exact_bytes(
                num_fields in 1usize..5,
            ) {
                let field_names: Vec<String> = (0..num_fields)
                    .map(|i| format!("FIELD{}", i))
                    .collect();
                let field_values: Vec<String> = (0..num_fields)
                    .map(|i| format!("VALUE{}", i))
                    .collect();

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes.clone());

                let returned_raw = provider.raw();

                prop_assert_eq!(returned_raw, raw_bytes.as_ref(),
                    "raw() should return exact input bytes");
            }

            /// raw() returns exact input bytes for segment metadata
            #[test]
            fn segment_provider_raw_returns_exact_bytes(
                num_fields in 1usize..5,
            ) {
                let field_names: Vec<String> = (0..num_fields)
                    .map(|i| format!("FIELD{}", i))
                    .collect();
                let field_values: Vec<String> = (0..num_fields)
                    .map(|i| format!("VALUE{}", i))
                    .collect();

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes.clone());

                let returned_raw = provider.raw();

                prop_assert_eq!(returned_raw, raw_bytes.as_ref(),
                    "raw() should return exact input bytes");
            }

            /// raw() preserves arbitrary byte content
            #[test]
            fn raw_preserves_arbitrary_bytes(
                bytes in proptest::collection::vec(any::<u8>(), 10..100),
            ) {
                // Create a simple definition that can handle arbitrary bytes
                let definition = Arc::new(
                    StructureDefinition::new("BinaryData")
                        .with_field(
                            FieldDefinition::new("DATA", FieldType::Bytes)
                                .with_size(SizeSpec::Fixed(bytes.len())),
                        ),
                );
                let raw_bytes: Arc<[u8]> = Arc::from(bytes.as_slice());
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes.clone());

                let returned_raw = provider.raw();

                prop_assert_eq!(returned_raw, raw_bytes.as_ref(),
                    "raw() should preserve arbitrary byte content");
            }

            /// raw() length matches input length
            #[test]
            fn raw_length_matches_input(
                num_fields in 1usize..10,
            ) {
                let field_names: Vec<String> = (0..num_fields)
                    .map(|i| format!("F{}", i))
                    .collect();
                let field_values: Vec<String> = (0..num_fields)
                    .map(|i| format!("V{}", i))
                    .collect();

                let definition = create_definition_with_fields(&field_names);
                let raw_bytes = create_raw_bytes(&field_values);
                let expected_len = raw_bytes.len();

                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

                prop_assert_eq!(provider.raw().len(), expected_len,
                    "raw() length should match input length");
            }
        }
    }

    /// Property 7: Metadata Interface TRE Access
    /// For any segment with TREs, calling `as_dict(None)` SHALL return all TRE fields
    /// with CETAG-prefixed keys, and calling `as_dict("GEOLOB")` SHALL return only
    /// fields from the GEOLOB TRE.
    /// **Validates: Requirements 18.1, 18.2, 18.3, 18.4**
    mod prop_7_metadata_interface_tre_access {
        use super::*;
        use crate::parser::Encoding;

        /// Strategy to generate valid CETAG strings (1-6 uppercase alphanumeric)
        fn valid_cetag_strategy() -> impl Strategy<Value = String> {
            prop::collection::vec(
                prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()),
                1..=6,
            )
            .prop_map(|chars| {
                let s: String = chars.into_iter().collect();
                format!("{:<6}", s) // Pad to 6 chars with spaces
            })
        }

        /// Strategy to generate valid BCS-A field values (alphanumeric, fixed size)
        fn field_value_strategy(size: usize) -> impl Strategy<Value = String> {
            prop::collection::vec(
                prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()),
                size,
            )
            .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Create a simple TRE definition with two fixed-size string fields
        fn create_tre_definition(name: &str, field1_size: usize, field2_size: usize) -> StructureDefinition {
            StructureDefinition::new(name)
                .with_field(
                    FieldDefinition::new("field1", FieldType::String)
                        .with_size(SizeSpec::Fixed(field1_size))
                        .with_encoding(Encoding::BcsA),
                )
                .with_field(
                    FieldDefinition::new("field2", FieldType::String)
                        .with_size(SizeSpec::Fixed(field2_size))
                        .with_encoding(Encoding::BcsA),
                )
        }

        /// Create a simple subheader definition
        fn create_subheader_definition() -> Arc<StructureDefinition> {
            Arc::new(
                StructureDefinition::new("TestSubheader")
                    .with_field(
                        FieldDefinition::new("HEADER", FieldType::String)
                            .with_size(SizeSpec::Fixed(10)),
                    )
            )
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: tre-des-support, Property 7: Metadata Interface TRE Access
            ///
            /// as_dict(None) returns all TRE fields with CETAG-prefixed keys
            ///
            /// **Validates: Requirements 18.1, 18.2, 18.3, 18.4**
            #[test]
            fn as_dict_none_includes_all_tre_fields(
                field1_value in field_value_strategy(8),
                field2_value in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes: Arc<[u8]> = Arc::from(b"TESTHEAD  ".as_slice());

                // Create TRE definition and envelope
                let tre_def = create_tre_definition("tre_geolob", 8, 6);
                let mut cedata = Vec::new();
                cedata.extend(field1_value.as_bytes());
                cedata.extend(field2_value.as_bytes());

                let tre_envelope = TreEnvelope {
                    tag: "GEOLOB".to_string(),
                    data: cedata,
                };

                // Create registry with TRE definition
                let mut registry = StructureRegistry::new();
                registry.register("tre_geolob", tre_def);

                // Create provider with TRE support
                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    vec![tre_envelope],
                    Arc::new(registry),
                );

                let dict = provider.as_dict(None);

                // Should have subheader field
                prop_assert!(dict.contains_key("HEADER"),
                    "Should contain subheader field HEADER");

                // Should have TRE fields with CETAG prefix
                prop_assert!(dict.contains_key("GEOLOB.field1"),
                    "Should contain TRE field GEOLOB.field1");
                prop_assert!(dict.contains_key("GEOLOB.field2"),
                    "Should contain TRE field GEOLOB.field2");

                // Verify TRE field values
                prop_assert_eq!(
                    dict.get("GEOLOB.field1"),
                    Some(&serde_json::json!(field1_value)),
                    "GEOLOB.field1 should have correct value"
                );
                prop_assert_eq!(
                    dict.get("GEOLOB.field2"),
                    Some(&serde_json::json!(field2_value)),
                    "GEOLOB.field2 should have correct value"
                );
            }

            /// Feature: tre-des-support, Property 7: Metadata Interface TRE Access
            ///
            /// as_dict(Some("CETAG")) returns only fields from that TRE
            ///
            /// **Validates: Requirements 18.3, 18.4**
            #[test]
            fn as_dict_with_cetag_prefix_filters_to_tre(
                field1_value in field_value_strategy(8),
                field2_value in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes: Arc<[u8]> = Arc::from(b"TESTHEAD  ".as_slice());

                // Create TRE definition and envelope
                let tre_def = create_tre_definition("tre_geolob", 8, 6);
                let mut cedata = Vec::new();
                cedata.extend(field1_value.as_bytes());
                cedata.extend(field2_value.as_bytes());

                let tre_envelope = TreEnvelope {
                    tag: "GEOLOB".to_string(),
                    data: cedata,
                };

                // Create registry with TRE definition
                let mut registry = StructureRegistry::new();
                registry.register("tre_geolob", tre_def);

                // Create provider with TRE support
                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    vec![tre_envelope],
                    Arc::new(registry),
                );

                // Filter by GEOLOB prefix
                let dict = provider.as_dict(Some("GEOLOB"));

                // Should NOT have subheader field
                prop_assert!(!dict.contains_key("HEADER"),
                    "Should NOT contain subheader field when filtering by CETAG");

                // Should have only TRE fields
                prop_assert_eq!(dict.len(), 2,
                    "Should have exactly 2 TRE fields");

                // All keys should start with GEOLOB
                for key in dict.keys() {
                    prop_assert!(key.starts_with("GEOLOB."),
                        "All keys should start with 'GEOLOB.', got: {}", key);
                }
            }

            /// Feature: tre-des-support, Property 7: Metadata Interface TRE Access
            ///
            /// Multiple TREs are all accessible and filterable
            ///
            /// **Validates: Requirements 18.1, 18.2, 18.3, 18.4**
            #[test]
            fn multiple_tres_accessible_and_filterable(
                tre1_field1 in field_value_strategy(8),
                tre1_field2 in field_value_strategy(6),
                tre2_field1 in field_value_strategy(8),
                tre2_field2 in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes: Arc<[u8]> = Arc::from(b"TESTHEAD  ".as_slice());

                // Create two TRE definitions
                let tre1_def = create_tre_definition("tre_geolob", 8, 6);
                let tre2_def = create_tre_definition("tre_sensrb", 8, 6);

                // Create TRE envelopes
                let mut cedata1 = Vec::new();
                cedata1.extend(tre1_field1.as_bytes());
                cedata1.extend(tre1_field2.as_bytes());
                let tre1 = TreEnvelope {
                    tag: "GEOLOB".to_string(),
                    data: cedata1,
                };

                let mut cedata2 = Vec::new();
                cedata2.extend(tre2_field1.as_bytes());
                cedata2.extend(tre2_field2.as_bytes());
                let tre2 = TreEnvelope {
                    tag: "SENSRB".to_string(),
                    data: cedata2,
                };

                // Create registry with both TRE definitions
                let mut registry = StructureRegistry::new();
                registry.register("tre_geolob", tre1_def);
                registry.register("tre_sensrb", tre2_def);

                // Create provider with both TREs
                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    vec![tre1, tre2],
                    Arc::new(registry),
                );

                // as_dict(None) should return all fields
                let all_dict = provider.as_dict(None);
                prop_assert!(all_dict.contains_key("HEADER"));
                prop_assert!(all_dict.contains_key("GEOLOB.field1"));
                prop_assert!(all_dict.contains_key("SENSRB.field1"));
                prop_assert_eq!(all_dict.len(), 5, // 1 subheader + 2*2 TRE fields
                    "Should have 5 total fields");

                // as_dict(Some("GEOLOB")) should return only GEOLOB fields
                let geolob_dict = provider.as_dict(Some("GEOLOB"));
                prop_assert_eq!(geolob_dict.len(), 2);
                for key in geolob_dict.keys() {
                    prop_assert!(key.starts_with("GEOLOB."));
                }

                // as_dict(Some("SENSRB")) should return only SENSRB fields
                let sensrb_dict = provider.as_dict(Some("SENSRB"));
                prop_assert_eq!(sensrb_dict.len(), 2);
                for key in sensrb_dict.keys() {
                    prop_assert!(key.starts_with("SENSRB."));
                }
            }

            /// Feature: tre-des-support, Property 7: Metadata Interface TRE Access
            ///
            /// Unknown TREs are skipped in metadata output
            ///
            /// **Validates: Requirements 18.3, 18.4**
            #[test]
            fn unknown_tres_skipped_in_output(
                known_field1 in field_value_strategy(8),
                known_field2 in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes: Arc<[u8]> = Arc::from(b"TESTHEAD  ".as_slice());

                // Create known TRE
                let tre_def = create_tre_definition("tre_geolob", 8, 6);
                let mut cedata = Vec::new();
                cedata.extend(known_field1.as_bytes());
                cedata.extend(known_field2.as_bytes());
                let known_tre = TreEnvelope {
                    tag: "GEOLOB".to_string(),
                    data: cedata,
                };

                // Create unknown TRE (no definition in registry)
                let unknown_tre = TreEnvelope {
                    tag: "UNKNWN".to_string(),
                    data: vec![1, 2, 3, 4, 5],
                };

                // Create registry with only known TRE definition
                let mut registry = StructureRegistry::new();
                registry.register("tre_geolob", tre_def);

                // Create provider with both TREs
                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    vec![known_tre, unknown_tre],
                    Arc::new(registry),
                );

                let dict = provider.as_dict(None);

                // Should have known TRE fields
                prop_assert!(dict.contains_key("GEOLOB.field1"));
                prop_assert!(dict.contains_key("GEOLOB.field2"));

                // Should NOT have unknown TRE fields
                for key in dict.keys() {
                    prop_assert!(!key.starts_with("UNKNWN"),
                        "Should not contain unknown TRE fields, got: {}", key);
                }

                // Total should be 1 subheader + 2 known TRE fields = 3
                prop_assert_eq!(dict.len(), 3);
            }
        }
    }
}
