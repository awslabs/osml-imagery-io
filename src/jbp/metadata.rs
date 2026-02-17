//! Metadata providers for NITF file and segment headers.
//!
//! This module provides metadata provider implementations:
//! - [`JBPFileMetadataProvider`] - File-level metadata from NITF header
//! - [`JBPSegmentMetadataProvider`] - Segment-level metadata from subheaders
//!
//! Both providers implement the [`MetadataProvider`] trait, providing access to
//! parsed field values as dictionaries and raw header bytes.

use std::collections::HashMap;
use std::sync::Arc;

use crate::parser::{StructureAccessor, StructureDefinition, Value};
use crate::traits::MetadataProvider;

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
/// ```
pub struct JBPSegmentMetadataProvider {
    /// Structure definition for field enumeration
    definition: Arc<StructureDefinition>,
    /// Raw subheader bytes
    raw_bytes: Arc<[u8]>,
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
        }
    }

    /// Create from definition and raw bytes directly.
    pub fn from_definition(definition: Arc<StructureDefinition>, raw_bytes: Arc<[u8]>) -> Self {
        Self {
            definition,
            raw_bytes,
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
}
