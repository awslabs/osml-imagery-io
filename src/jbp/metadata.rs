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
//! through the `with_tres()` constructor. TRE fields are exposed as nested
//! dictionaries keyed by CETAG (e.g., `{"GEOLOB": {"ARV": "...", "BRV": "..."}}`).

use std::collections::HashMap;
use std::sync::Arc;

use crate::owned_buffer::OwnedBuffer;
use crate::parser::{StructureAccessor, StructureDefinition, StructureRegistry, Value};
use crate::traits::MetadataProvider;

use super::tre::TreEnvelope;
use super::tre_fields;

/// Metadata provider for NITF file header fields.
///
/// Provides access to file-level metadata from the NITF header through the
/// [`MetadataProvider`] trait. Fields are eagerly parsed at construction into a
/// cached HashMap for O(1) access.
///
/// # Example
///
/// ```ignore
/// let provider = JBPFileMetadataProvider::new(accessor, raw_bytes);
///
/// // Get all fields
/// let all_fields = provider.entries(None);
///
/// // Get only security fields (starting with "FS")
/// let security_fields = provider.entries(Some("FS"));
/// ```
pub struct JBPFileMetadataProvider {
    tags: HashMap<String, serde_json::Value>,
    raw_bytes: OwnedBuffer,
}

impl JBPFileMetadataProvider {
    /// Create from definition and raw bytes directly.
    ///
    /// Eagerly parses all fields into a cached HashMap. The definition is consumed
    /// during construction and not retained.
    pub fn from_definition(definition: Arc<StructureDefinition>, raw_bytes: OwnedBuffer) -> Self {
        let tags = parse_fields_from_definition(&definition, raw_bytes.as_bytes(), None);
        Self { tags, raw_bytes }
    }
}

impl MetadataProvider for JBPFileMetadataProvider {
    fn raw(&self) -> &[u8] {
        self.raw_bytes.as_bytes()
    }

    fn get_value(&self, key: &str) -> Option<serde_json::Value> {
        self.tags.get(key).cloned()
    }

    fn contains_key(&self, key: &str) -> bool {
        self.tags.contains_key(key)
    }

    fn len(&self) -> usize {
        self.tags.len()
    }

    fn keys(&self) -> Vec<String> {
        self.tags.keys().cloned().collect()
    }

    fn entries(&self, prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        match prefix {
            None => self.tags.clone(),
            Some(prefix) => self
                .tags
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

/// Metadata provider for NITF segment subheader fields.
///
/// Provides access to segment-level metadata from subheaders through the
/// [`MetadataProvider`] trait. Fields (including TRE entries) are eagerly parsed
/// at construction into a cached HashMap for O(1) access.
///
/// # TRE Support
///
/// When created with `with_tres()`, TRE fields are parsed eagerly and stored as
/// top-level keys (CETAG) mapped to nested dictionaries.
/// Unknown TREs produce a raw representation: `{"_raw": "<hex>", "_length": N}`.
///
/// # Example
///
/// ```ignore
/// let provider = JBPSegmentMetadataProvider::with_tres(
///     definition, raw_bytes, tre_envelopes, registry
/// );
///
/// // Get all fields (O(n) clone)
/// let all_fields = provider.entries(None);
///
/// // Get only GEOLOB TRE fields
/// let geolob_fields = provider.entries(Some("GEOLOB"));
/// ```
pub struct JBPSegmentMetadataProvider {
    tags: HashMap<String, serde_json::Value>,
    raw_bytes: OwnedBuffer,
}

impl JBPSegmentMetadataProvider {
    /// Create from definition and raw bytes directly.
    ///
    /// Eagerly parses all subheader fields into a cached HashMap.
    pub fn from_definition(definition: Arc<StructureDefinition>, raw_bytes: OwnedBuffer) -> Self {
        let tags = parse_fields_from_definition(&definition, raw_bytes.as_bytes(), None);
        Self { tags, raw_bytes }
    }

    /// Create with TRE support.
    ///
    /// Eagerly parses all subheader fields and TRE entries into a cached HashMap.
    /// The definition, registry, and TRE envelopes are consumed during construction
    /// and not retained.
    pub fn with_tres(
        definition: Arc<StructureDefinition>,
        raw_bytes: OwnedBuffer,
        tre_envelopes: Vec<TreEnvelope>,
        registry: Arc<StructureRegistry>,
    ) -> Self {
        let mut tags =
            parse_fields_from_definition(&definition, raw_bytes.as_bytes(), Some(&registry));
        parse_tre_entries(&mut tags, &tre_envelopes, &registry);
        Self { tags, raw_bytes }
    }
}

impl MetadataProvider for JBPSegmentMetadataProvider {
    fn raw(&self) -> &[u8] {
        self.raw_bytes.as_bytes()
    }

    fn get_value(&self, key: &str) -> Option<serde_json::Value> {
        self.tags.get(key).cloned()
    }

    fn contains_key(&self, key: &str) -> bool {
        self.tags.contains_key(key)
    }

    fn len(&self) -> usize {
        self.tags.len()
    }

    fn keys(&self) -> Vec<String> {
        self.tags.keys().cloned().collect()
    }

    fn entries(&self, prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        match prefix {
            None => self.tags.clone(),
            Some(prefix) => self
                .tags
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

/// Eagerly parse all fields from a structure definition into a HashMap.
///
/// Creates a `StructureAccessor` from the definition and raw bytes, then iterates
/// all fields (respecting conditions and repeated fields) to build the cached map.
fn parse_fields_from_definition(
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

/// Parse TRE envelopes into an existing tags HashMap.
///
/// Each TRE is stored as a top-level key (trimmed CETAG) mapped to either a nested
/// dictionary of parsed fields, or a raw representation if the TRE definition is unknown.
fn parse_tre_entries(
    tags: &mut HashMap<String, serde_json::Value>,
    tre_envelopes: &[TreEnvelope],
    registry: &StructureRegistry,
) {
    for envelope in tre_envelopes {
        let tag = envelope.tag.trim();
        match tre_fields::create_accessor(registry, tag, &envelope.data) {
            Ok(Some(tre_accessor)) => {
                let tre_def = tre_accessor.definition.clone();
                let mut tre_dict = serde_json::Map::new();
                for field_path in tre_accessor.fields() {
                    if let Ok(value) = tre_accessor.get(&field_path) {
                        if let Some(json_value) =
                            value_to_json(&value, Some(registry), Some(&tre_def))
                        {
                            tre_dict.insert(field_path, json_value);
                        }
                    }
                }
                tags.insert(tag.to_string(), serde_json::Value::Object(tre_dict));
            }
            Ok(None) | Err(_) => {
                let mut raw_dict = serde_json::Map::new();
                let hex: String = envelope.data.iter().map(|b| format!("{:02x}", b)).collect();
                raw_dict.insert("_raw".to_string(), serde_json::Value::String(hex));
                raw_dict.insert(
                    "_length".to_string(),
                    serde_json::Value::Number(envelope.data.len().into()),
                );
                tags.insert(tag.to_string(), serde_json::Value::Object(raw_dict));
            }
        }
    }
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
fn value_to_json(
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
            // Try to interpret as UTF-8 string first
            match std::str::from_utf8(bytes) {
                Ok(s) => Some(serde_json::Value::String(
                    s.trim_end_matches(' ').to_string(),
                )),
                Err(_) => {
                    // Fall back to hex encoding for binary data
                    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                    Some(serde_json::Value::String(hex))
                }
            }
        }
        Value::Unsigned(n) => Some(serde_json::Value::Number((*n).into())),
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
    fn create_test_data() -> OwnedBuffer {
        // FHDR(4) + FVER(5) + FSCLAS(1) + FSCLSY(2) = 12 bytes
        OwnedBuffer::from_vec(b"NITF02.10U  ".to_vec())
    }

    #[test]
    fn file_metadata_provider_raw_returns_bytes() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes.clone());

        assert_eq!(provider.raw(), raw_bytes.as_bytes());
    }

    #[test]
    fn file_metadata_provider_entries_all_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

        let dict = provider.entries(None);

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
    fn file_metadata_provider_entries_with_prefix() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

        // Filter by "FS" prefix (security fields)
        let dict = provider.entries(Some("FS"));

        // Should only have security fields
        assert!(dict.contains_key("FSCLAS"));
        assert!(dict.contains_key("FSCLSY"));
        assert!(!dict.contains_key("FHDR"));
        assert!(!dict.contains_key("FVER"));
    }

    #[test]
    fn file_metadata_provider_entries_with_nonmatching_prefix() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes);

        // Filter by prefix that matches nothing
        let dict = provider.entries(Some("XYZ"));

        assert!(dict.is_empty());
    }

    #[test]
    fn segment_metadata_provider_raw_returns_bytes() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes.clone());

        assert_eq!(provider.raw(), raw_bytes.as_bytes());
    }

    #[test]
    fn segment_metadata_provider_entries_all_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

        let dict = provider.entries(None);

        // Should have all 4 fields
        assert!(dict.contains_key("FHDR"));
        assert!(dict.contains_key("FVER"));
        assert!(dict.contains_key("FSCLAS"));
        assert!(dict.contains_key("FSCLSY"));
    }

    #[test]
    fn segment_metadata_provider_entries_with_prefix() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

        // Filter by "F" prefix
        let dict = provider.entries(Some("F"));

        // All fields start with F in our test
        assert_eq!(dict.len(), 4);
    }

    #[test]
    fn value_to_json_string() {
        let value = Value::from_borrowed("HELLO   ");
        let json = value_to_json(&value, None, None).unwrap();
        assert_eq!(json, serde_json::json!("HELLO"));
    }

    #[test]
    fn value_to_json_unsigned() {
        let value = Value::from_unsigned(42);
        let json = value_to_json(&value, None, None).unwrap();
        assert_eq!(json, serde_json::json!(42));
    }

    #[test]
    fn value_to_json_bytes_utf8() {
        let value = Value::from_bytes(b"WORLD   ");
        let json = value_to_json(&value, None, None).unwrap();
        assert_eq!(json, serde_json::json!("WORLD"));
    }

    #[test]
    fn value_to_json_bytes_binary() {
        let value = Value::from_bytes(&[0xFF, 0x00, 0xAB]);
        let json = value_to_json(&value, None, None).unwrap();
        assert_eq!(json, serde_json::json!("ff00ab"));
    }

    #[test]
    fn value_to_json_array() {
        let value = Value::from_array(vec![
            Value::from_unsigned(1),
            Value::from_unsigned(2),
            Value::from_unsigned(3),
        ]);
        let json = value_to_json(&value, None, None).unwrap();
        assert_eq!(json, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn value_to_json_struct() {
        let value = Value::from_struct(b"data", "TestType");
        let json = value_to_json(&value, None, None).unwrap();

        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("_type"), Some(&serde_json::json!("TestType")));
        assert_eq!(obj.get("_data"), Some(&serde_json::json!("64617461")));
    }

    #[test]
    fn value_to_json_struct_resolved_via_local_types() {
        // Simulate a parent definition with a local type (like image_segment_info
        // in the file header). The struct type is NOT in the registry — it's in
        // the parent definition's `types` map.
        let inner_def = StructureDefinition::new("my_inner_type")
            .with_field(
                FieldDefinition::new("ALPHA", FieldType::String).with_size(SizeSpec::Fixed(3)),
            )
            .with_field(
                FieldDefinition::new("BETA", FieldType::String).with_size(SizeSpec::Fixed(4)),
            );

        let mut parent_def = StructureDefinition::new("ParentHeader");
        parent_def
            .types
            .insert("my_inner_type".to_string(), inner_def);

        // Build raw bytes for the inner struct: ALPHA(3) + BETA(4) = 7 bytes
        let raw = b"ABCDEFG";
        let value = Value::from_struct(raw.as_slice(), "my_inner_type");

        // With no registry but with parent definition, should resolve via local types
        let json = value_to_json(&value, None, Some(&parent_def)).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("ALPHA"), Some(&serde_json::json!("ABC")));
        assert_eq!(obj.get("BETA"), Some(&serde_json::json!("DEFG")));
        // Should NOT have _type/_data fallback keys
        assert!(obj.get("_type").is_none());
        assert!(obj.get("_data").is_none());
    }

    #[test]
    fn value_to_json_array_of_structs_resolved_via_local_types() {
        // Simulate repeated struct fields (like IMAGE_INFO array in file header)
        let inner_def = StructureDefinition::new("segment_info")
            .with_field(
                FieldDefinition::new("SUBHDR_LEN", FieldType::String).with_size(SizeSpec::Fixed(6)),
            )
            .with_field(
                FieldDefinition::new("DATA_LEN", FieldType::String).with_size(SizeSpec::Fixed(10)),
            );

        let mut parent_def = StructureDefinition::new("FileHeader");
        parent_def
            .types
            .insert("segment_info".to_string(), inner_def);

        // Build an array of 2 struct values, each 16 bytes
        let arr = Value::from_array(vec![
            Value::from_struct(b"000439000012345", "segment_info"),
            Value::from_struct(b"000439000067890", "segment_info"),
        ]);

        let json = value_to_json(&arr, None, Some(&parent_def)).unwrap();
        let json_arr = json.as_array().unwrap();
        assert_eq!(json_arr.len(), 2);

        // First element should be resolved
        let first = json_arr[0].as_object().unwrap();
        assert_eq!(first.get("SUBHDR_LEN"), Some(&serde_json::json!("000439")));
        assert!(first.get("_type").is_none());

        // Second element should also be resolved
        let second = json_arr[1].as_object().unwrap();
        assert_eq!(second.get("SUBHDR_LEN"), Some(&serde_json::json!("000439")));
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

        let dict = provider.entries(None);

        // Should have subheader fields
        assert!(dict.contains_key("FHDR"));
        assert!(dict.contains_key("FVER"));

        // Should have GEOLOB as a nested dictionary
        assert!(dict.contains_key("GEOLOB"));
        let geolob = dict.get("GEOLOB").unwrap().as_object().unwrap();
        assert_eq!(geolob.get("arv"), Some(&serde_json::json!("000360000")));
        assert_eq!(geolob.get("brv"), Some(&serde_json::json!("000360000")));
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

        // Filter by GEOLOB prefix - should only get TRE entry
        let dict = provider.entries(Some("GEOLOB"));

        assert!(dict.contains_key("GEOLOB"));
        let geolob = dict.get("GEOLOB").unwrap().as_object().unwrap();
        assert!(geolob.contains_key("arv"));
        assert!(geolob.contains_key("brv"));
        assert!(!dict.contains_key("FHDR"));
        assert!(!dict.contains_key("FVER"));
        assert_eq!(dict.len(), 1);
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

        let dict = provider.entries(None);

        // Should have subheader fields
        assert!(dict.contains_key("FHDR"));

        // Unknown TRE should have raw representation
        assert!(dict.contains_key("UNKNWN"));
        let unknwn = dict.get("UNKNWN").unwrap().as_object().unwrap();
        assert_eq!(unknwn.get("_raw"), Some(&serde_json::json!("0102030405")));
        assert_eq!(unknwn.get("_length"), Some(&serde_json::json!(5)));
    }

    #[test]
    fn segment_provider_without_tres_has_no_tre_fields() {
        let definition = create_test_definition();
        let raw_bytes = create_test_data();

        // Create provider without TRE support
        let provider = JBPSegmentMetadataProvider::from_definition(definition, raw_bytes);

        let dict = provider.entries(None);

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
        let tre2_def = StructureDefinition::new("tre_test").with_field(
            FieldDefinition::new("value", FieldType::String).with_size(SizeSpec::Fixed(5)),
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

        let dict = provider.entries(None);

        // Should have nested dicts for both TREs
        assert!(dict.contains_key("GEOLOB"));
        let geolob = dict.get("GEOLOB").unwrap().as_object().unwrap();
        assert_eq!(geolob.get("arv"), Some(&serde_json::json!("000360000")));
        assert_eq!(geolob.get("brv"), Some(&serde_json::json!("000360000")));

        assert!(dict.contains_key("TEST"));
        let test_tre = dict.get("TEST").unwrap().as_object().unwrap();
        assert_eq!(test_tre.get("value"), Some(&serde_json::json!("HELLO")));
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
    fn create_raw_bytes(values: &[String]) -> OwnedBuffer {
        let mut bytes = Vec::new();
        for value in values {
            let mut field_bytes = value.as_bytes().to_vec();
            // Pad to 10 bytes with spaces
            field_bytes.resize(10, b' ');
            bytes.extend_from_slice(&field_bytes);
        }
        OwnedBuffer::from_vec(bytes)
    }

    /// Property 8: Metadata Prefix Filtering
    /// For any MetadataProvider and any prefix string, `entries(Some(prefix))` SHALL
    /// return only entries whose keys start with the given prefix, and `entries(None)`
    /// SHALL return all entries. TRE entries appear as nested dicts under their CETAG
    /// key, and repeated fields appear as JSON arrays.
    /// **Validates: Requirements 9.3, 9.4**
    mod prop_8_metadata_prefix_filtering {
        use super::*;
        use crate::parser::{Encoding, RepeatSpec};

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// entries(None) returns all fields
            #[test]
            fn entries_none_returns_all_fields(
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

                let dict = provider.entries(None);

                // Should have all fields
                prop_assert_eq!(dict.len(), num_fields,
                    "Expected {} fields, got {}", num_fields, dict.len());

                for name in &field_names {
                    prop_assert!(dict.contains_key(name),
                        "Missing field: {}", name);
                }
            }

            /// entries(Some(prefix)) returns only matching fields
            #[test]
            fn entries_with_prefix_filters_correctly(
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
                let dict = provider.entries(Some("FS"));

                // Should only have matching fields
                prop_assert_eq!(dict.len(), num_matching,
                    "Expected {} matching fields, got {}", num_matching, dict.len());

                for name in dict.keys() {
                    prop_assert!(name.starts_with("FS"),
                        "Field '{}' should start with 'FS'", name);
                }
            }

            /// entries with non-matching prefix returns empty
            #[test]
            fn entries_nonmatching_prefix_returns_empty(
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
                let dict = provider.entries(Some("XYZ"));

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
                let dict = provider.entries(Some("ABC"));
                prop_assert_eq!(dict.len(), num_upper);

                // Filter by "abc" should get nothing (case-sensitive)
                let dict_lower = provider.entries(Some("abc"));
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
                let dict = provider.entries(Some("IM"));

                prop_assert_eq!(dict.len(), num_matching);
                for name in dict.keys() {
                    prop_assert!(name.starts_with("IM"));
                }
            }

            /// Feature: metadata-restructure, Property 8: Metadata Prefix Filtering
            ///
            /// Prefix filtering with TREs returns nested dicts for matching CETAGs.
            /// TRE entries are top-level keys mapped to nested dicts, not dot-separated.
            ///
            /// **Validates: Requirements 9.3, 9.4**
            #[test]
            fn prefix_filter_returns_nested_tre_dicts(
                field1_value in prop::collection::vec(
                    prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), 8
                ).prop_map(|c| c.into_iter().collect::<String>()),
                field2_value in prop::collection::vec(
                    prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), 6
                ).prop_map(|c| c.into_iter().collect::<String>()),
            ) {
                // Create subheader with fields that don't start with "GEO"
                let subheader_def = Arc::new(
                    StructureDefinition::new("TestSubheader")
                        .with_field(
                            FieldDefinition::new("HEADER", FieldType::String)
                                .with_size(SizeSpec::Fixed(10)),
                        )
                );
                let subheader_bytes = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());

                // Create TRE definition and envelope
                let tre_def = StructureDefinition::new("tre_geolob")
                    .with_field(
                        FieldDefinition::new("field1", FieldType::String)
                            .with_size(SizeSpec::Fixed(8))
                            .with_encoding(Encoding::BcsA),
                    )
                    .with_field(
                        FieldDefinition::new("field2", FieldType::String)
                            .with_size(SizeSpec::Fixed(6))
                            .with_encoding(Encoding::BcsA),
                    );

                let mut cedata = Vec::new();
                cedata.extend(field1_value.as_bytes());
                cedata.extend(field2_value.as_bytes());

                let tre_envelope = TreEnvelope {
                    tag: "GEOLOB".to_string(),
                    data: cedata,
                };

                let mut registry = StructureRegistry::new();
                registry.register("tre_geolob", tre_def);

                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    vec![tre_envelope],
                    Arc::new(registry),
                );

                // Filter by "GEOLOB" — should return only the nested TRE dict
                let dict = provider.entries(Some("GEOLOB"));

                prop_assert_eq!(dict.len(), 1,
                    "Should have exactly 1 entry for GEOLOB. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>());
                prop_assert!(dict.contains_key("GEOLOB"),
                    "Should contain 'GEOLOB' key");

                let geolob = dict.get("GEOLOB").unwrap();
                prop_assert!(geolob.is_object(),
                    "GEOLOB should be a nested dict, got: {:?}", geolob);

                let nested = geolob.as_object().unwrap();
                prop_assert!(nested.contains_key("field1"),
                    "Nested dict should contain 'field1'");
                prop_assert!(nested.contains_key("field2"),
                    "Nested dict should contain 'field2'");

                // No dot-separated keys
                for key in dict.keys() {
                    prop_assert!(!key.contains('.'),
                        "No dot-separated keys should appear, got: {}", key);
                }
            }

            /// Feature: metadata-restructure, Property 8: Metadata Prefix Filtering
            ///
            /// Prefix filtering with repeated fields returns arrays under base name.
            ///
            /// **Validates: Requirements 9.3, 9.4**
            #[test]
            fn prefix_filter_returns_arrays_for_repeated_fields(
                count in 1u8..5,
                elem_values in prop::collection::vec(
                    prop::collection::vec(
                        prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), 5
                    ).prop_map(|c| c.into_iter().collect::<String>()),
                    5,
                ),
            ) {
                let values: Vec<String> = elem_values.into_iter().take(count as usize).collect();
                let elem_size = 5usize;

                // Build a subheader with: HEADER(10) + NREP(1 byte uint) + ITEMS(repeated, 5 each)
                let repeat_expr = crate::parser::ExpressionEvaluator::parse("NREP").unwrap();
                let def = Arc::new(
                    StructureDefinition::new("TestSubheaderRepeat")
                        .with_field(
                            FieldDefinition::new("HEADER", FieldType::String)
                                .with_size(SizeSpec::Fixed(10)),
                        )
                        .with_field(
                            FieldDefinition::new("NREP", FieldType::UnsignedInt(1))
                                .with_size(SizeSpec::Fixed(1)),
                        )
                        .with_field(
                            FieldDefinition::new("ITEMS", FieldType::String)
                                .with_size(SizeSpec::Fixed(elem_size))
                                .with_encoding(Encoding::BcsA)
                                .with_repeat(RepeatSpec::Expression(repeat_expr)),
                        )
                );

                // Build raw bytes
                let mut bytes = Vec::new();
                let mut header = b"TESTHEAD".to_vec();
                header.resize(10, b' ');
                bytes.extend_from_slice(&header);
                bytes.push(count);
                for val in &values {
                    let mut elem = val.as_bytes().to_vec();
                    elem.resize(elem_size, b' ');
                    bytes.extend_from_slice(&elem);
                }
                let raw = OwnedBuffer::from_vec(bytes);

                let provider = JBPSegmentMetadataProvider::from_definition(def, raw);

                // Filter by "ITEMS" prefix — should return only the ITEMS array
                let dict = provider.entries(Some("ITEMS"));

                prop_assert_eq!(dict.len(), 1,
                    "Should have exactly 1 entry for ITEMS. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>());
                prop_assert!(dict.contains_key("ITEMS"),
                    "Should contain 'ITEMS' key");

                let items = dict.get("ITEMS").unwrap();
                prop_assert!(items.is_array(),
                    "ITEMS should be a JSON array, got: {:?}", items);

                let arr = items.as_array().unwrap();
                prop_assert_eq!(arr.len(), count as usize,
                    "Array length should equal count {}", count);

                // No _N-suffixed keys
                for key in dict.keys() {
                    prop_assert!(!key.starts_with("ITEMS_"),
                        "No _N-suffixed keys should appear, got: {}", key);
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

                prop_assert_eq!(returned_raw, raw_bytes.as_bytes(),
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

                prop_assert_eq!(returned_raw, raw_bytes.as_bytes(),
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
                let raw_bytes = OwnedBuffer::from_vec(bytes.clone());
                let provider = JBPFileMetadataProvider::from_definition(definition, raw_bytes.clone());

                let returned_raw = provider.raw();

                prop_assert_eq!(returned_raw, raw_bytes.as_bytes(),
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
    /// For any segment with TREs, calling `entries(None)` SHALL return each CETAG
    /// as a top-level key mapped to a nested dictionary of that TRE's fields.
    /// Calling `entries("GEOLOB")` SHALL return only the `"GEOLOB"` key with its
    /// nested dictionary. Unknown TREs SHALL get raw representation.
    /// **Validates: Requirements 9.3, 9.4**
    mod prop_7_metadata_interface_tre_access {
        use super::*;
        use crate::parser::Encoding;

        /// Strategy to generate valid BCS-A field values (alphanumeric, fixed size)
        fn field_value_strategy(size: usize) -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), size)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Create a simple TRE definition with two fixed-size string fields
        fn create_tre_definition(
            name: &str,
            field1_size: usize,
            field2_size: usize,
        ) -> StructureDefinition {
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
            Arc::new(StructureDefinition::new("TestSubheader").with_field(
                FieldDefinition::new("HEADER", FieldType::String).with_size(SizeSpec::Fixed(10)),
            ))
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: metadata-restructure, Property 7: Metadata Interface TRE Access
            ///
            /// entries(None) returns TRE as nested dictionary under CETAG key
            ///
            /// **Validates: Requirements 9.3, 9.4**
            #[test]
            fn entries_none_includes_tre_as_nested_dict(
                field1_value in field_value_strategy(8),
                field2_value in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());

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

                let dict = provider.entries(None);

                // Should have subheader field
                prop_assert!(dict.contains_key("HEADER"),
                    "Should contain subheader field HEADER");

                // Should have GEOLOB as a top-level key mapped to a nested dict
                prop_assert!(dict.contains_key("GEOLOB"),
                    "Should contain GEOLOB as a top-level key");

                let geolob = dict.get("GEOLOB").unwrap();
                prop_assert!(geolob.is_object(),
                    "GEOLOB should be a nested dictionary, got: {:?}", geolob);

                let nested = geolob.as_object().unwrap();

                // Verify nested dict contains field1 and field2 with correct values
                prop_assert_eq!(
                    nested.get("field1"),
                    Some(&serde_json::json!(field1_value)),
                    "GEOLOB.field1 should have correct value"
                );
                prop_assert_eq!(
                    nested.get("field2"),
                    Some(&serde_json::json!(field2_value)),
                    "GEOLOB.field2 should have correct value"
                );

                // No dot-separated keys should appear
                for key in dict.keys() {
                    prop_assert!(!key.contains('.'),
                        "No dot-separated keys should appear, got: {}", key);
                }
            }

            /// Feature: metadata-restructure, Property 7: Metadata Interface TRE Access
            ///
            /// entries(Some("CETAG")) returns only that CETAG's nested dict
            ///
            /// **Validates: Requirements 9.3, 9.4**
            #[test]
            fn entries_with_cetag_prefix_filters_to_nested_dict(
                field1_value in field_value_strategy(8),
                field2_value in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());

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
                let dict = provider.entries(Some("GEOLOB"));

                // Should NOT have subheader field
                prop_assert!(!dict.contains_key("HEADER"),
                    "Should NOT contain subheader field when filtering by CETAG");

                // Should have exactly 1 entry: the GEOLOB nested dict
                prop_assert_eq!(dict.len(), 1,
                    "Should have exactly 1 entry (the GEOLOB nested dict)");

                // The GEOLOB key should map to a nested dict with field1 and field2
                prop_assert!(dict.contains_key("GEOLOB"),
                    "Should contain 'GEOLOB' key");
                let geolob = dict.get("GEOLOB").unwrap();
                prop_assert!(geolob.is_object(),
                    "GEOLOB should be a nested dict");
                let nested = geolob.as_object().unwrap();
                prop_assert!(nested.contains_key("field1"),
                    "Nested dict should contain 'field1'");
                prop_assert!(nested.contains_key("field2"),
                    "Nested dict should contain 'field2'");
            }

            /// Feature: metadata-restructure, Property 7: Metadata Interface TRE Access
            ///
            /// Multiple TREs appear as separate nested dicts and are filterable
            ///
            /// **Validates: Requirements 9.3, 9.4**
            #[test]
            fn multiple_tres_as_nested_dicts_and_filterable(
                tre1_field1 in field_value_strategy(8),
                tre1_field2 in field_value_strategy(6),
                tre2_field1 in field_value_strategy(8),
                tre2_field2 in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());

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

                // entries(None) should return subheader + 2 TRE nested dicts
                let all_dict = provider.entries(None);
                prop_assert!(all_dict.contains_key("HEADER"));
                prop_assert!(all_dict.contains_key("GEOLOB"));
                prop_assert!(all_dict.contains_key("SENSRB"));
                prop_assert_eq!(all_dict.len(), 3, // 1 subheader + 2 TRE entries
                    "Should have 3 total entries");

                // Each TRE entry is a nested dict
                let geolob = all_dict.get("GEOLOB").unwrap().as_object().unwrap();
                prop_assert_eq!(geolob.get("field1"), Some(&serde_json::json!(tre1_field1)));
                prop_assert_eq!(geolob.get("field2"), Some(&serde_json::json!(tre1_field2)));

                let sensrb = all_dict.get("SENSRB").unwrap().as_object().unwrap();
                prop_assert_eq!(sensrb.get("field1"), Some(&serde_json::json!(tre2_field1)));
                prop_assert_eq!(sensrb.get("field2"), Some(&serde_json::json!(tre2_field2)));

                // entries(Some("GEOLOB")) should return only GEOLOB nested dict
                let geolob_dict = provider.entries(Some("GEOLOB"));
                prop_assert_eq!(geolob_dict.len(), 1);
                prop_assert!(geolob_dict.contains_key("GEOLOB"));

                // entries(Some("SENSRB")) should return only SENSRB nested dict
                let sensrb_dict = provider.entries(Some("SENSRB"));
                prop_assert_eq!(sensrb_dict.len(), 1);
                prop_assert!(sensrb_dict.contains_key("SENSRB"));
            }

            /// Feature: metadata-restructure, Property 7: Metadata Interface TRE Access
            ///
            /// Unknown TREs get raw representation with _raw and _length
            ///
            /// **Validates: Requirements 9.3, 9.4**
            #[test]
            fn unknown_tres_get_raw_representation(
                known_field1 in field_value_strategy(8),
                known_field2 in field_value_strategy(6),
            ) {
                // Create subheader
                let subheader_def = create_subheader_definition();
                let subheader_bytes = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());

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

                let dict = provider.entries(None);

                // Should have known TRE as nested dict
                prop_assert!(dict.contains_key("GEOLOB"));
                let geolob = dict.get("GEOLOB").unwrap().as_object().unwrap();
                prop_assert!(geolob.contains_key("field1"));
                prop_assert!(geolob.contains_key("field2"));

                // Unknown TRE should have raw representation
                prop_assert!(dict.contains_key("UNKNWN"),
                    "Unknown TRE should be present with raw representation");
                let unknwn = dict.get("UNKNWN").unwrap().as_object().unwrap();
                prop_assert_eq!(unknwn.get("_raw"),
                    Some(&serde_json::json!("0102030405")),
                    "Unknown TRE should have _raw hex string");
                prop_assert_eq!(unknwn.get("_length"),
                    Some(&serde_json::json!(5)),
                    "Unknown TRE should have _length matching data size");

                // Total: 1 subheader + 1 known TRE nested dict + 1 unknown TRE raw = 3
                prop_assert_eq!(dict.len(), 3);
            }
        }
    }

    /// Feature: metadata-restructure, Property 1: TRE Fields Appear as Nested Dictionaries
    ///
    /// For any JBPSegmentMetadataProvider with one or more TREs that have definitions
    /// in the StructureRegistry, calling entries(None) shall produce a dictionary where
    /// each CETAG is a top-level key mapped to a nested dictionary containing that TRE's
    /// field names as keys and their parsed values as values. No dot-separated
    /// "CETAG.field" keys shall appear.
    ///
    /// **Validates: Requirements 1.1, 1.2, 1.3**
    mod prop_1_tre_fields_nested_dicts {
        use super::*;
        use crate::parser::Encoding;

        /// Strategy to generate a unique uppercase CETAG (3-6 chars, letters only)
        fn cetag_strategy() -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['A'..='Z'].into()), 3..=6)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Strategy to generate a lowercase field name (3-8 chars)
        fn field_name_strategy() -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['a'..='z'].into()), 3..=8)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Strategy to generate a BCS-A field value of a given size
        fn field_value_strategy(size: usize) -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), size)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Represents a single TRE with its definition and data for testing
        #[derive(Debug, Clone)]
        struct TestTre {
            cetag: String,
            field_names: Vec<String>,
            field_values: Vec<String>,
            field_size: usize,
        }

        /// Strategy to generate a single TRE with 2-5 unique fields
        fn test_tre_strategy() -> impl Strategy<Value = TestTre> {
            let field_size = 5usize;
            (
                cetag_strategy(),
                prop::collection::vec(field_name_strategy(), 2..=5),
            )
                .prop_flat_map(move |(cetag, raw_names)| {
                    // Deduplicate field names by appending index
                    let field_names: Vec<String> = raw_names
                        .iter()
                        .enumerate()
                        .map(|(i, n)| format!("{}_{}", n, i))
                        .collect();
                    let num_fields = field_names.len();
                    let values =
                        prop::collection::vec(field_value_strategy(field_size), num_fields);
                    (Just(cetag), Just(field_names), values)
                })
                .prop_map(move |(cetag, field_names, field_values)| TestTre {
                    cetag,
                    field_names,
                    field_values,
                    field_size,
                })
        }

        /// Strategy to generate 1-3 TREs with unique CETAGs
        fn test_tres_strategy() -> impl Strategy<Value = Vec<TestTre>> {
            prop::collection::vec(test_tre_strategy(), 1..=3).prop_map(|tres| {
                // Ensure unique CETAGs by appending index
                tres.into_iter()
                    .enumerate()
                    .map(|(i, mut tre)| {
                        tre.cetag = format!("{}{}", &tre.cetag[..tre.cetag.len().min(4)], i);
                        tre
                    })
                    .collect()
            })
        }

        /// Build a StructureDefinition for a TestTre
        fn build_tre_definition(tre: &TestTre) -> StructureDefinition {
            let mut def = StructureDefinition::new(format!("tre_{}", tre.cetag.to_lowercase()));
            for name in &tre.field_names {
                def = def.with_field(
                    FieldDefinition::new(name.clone(), FieldType::String)
                        .with_size(SizeSpec::Fixed(tre.field_size))
                        .with_encoding(Encoding::BcsA),
                );
            }
            def
        }

        /// Build raw CEDATA bytes for a TestTre
        fn build_tre_data(tre: &TestTre) -> Vec<u8> {
            let mut data = Vec::new();
            for value in &tre.field_values {
                let mut field_bytes = value.as_bytes().to_vec();
                field_bytes.resize(tre.field_size, b' ');
                data.extend_from_slice(&field_bytes);
            }
            data
        }

        /// Create a minimal subheader definition and data
        fn create_subheader() -> (Arc<StructureDefinition>, OwnedBuffer) {
            let def = Arc::new(StructureDefinition::new("TestSubheader").with_field(
                FieldDefinition::new("HEADER", FieldType::String).with_size(SizeSpec::Fixed(10)),
            ));
            let data = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());
            (def, data)
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: metadata-restructure, Property 1: TRE Fields Appear as Nested Dictionaries
            ///
            /// **Validates: Requirements 1.1, 1.2, 1.3**
            #[test]
            fn tre_fields_appear_as_nested_dicts(tres in test_tres_strategy()) {
                let (subheader_def, subheader_bytes) = create_subheader();

                // Build registry and envelopes from generated TREs
                let mut registry = StructureRegistry::new();
                let mut envelopes = Vec::new();

                for tre in &tres {
                    let def_name = format!("tre_{}", tre.cetag.to_lowercase());
                    registry.register(&def_name, build_tre_definition(tre));

                    envelopes.push(TreEnvelope {
                        tag: tre.cetag.clone(),
                        data: build_tre_data(tre),
                    });
                }

                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    envelopes,
                    Arc::new(registry),
                );

                let dict = provider.entries(None);

                // 1. Each CETAG is a top-level key
                for tre in &tres {
                    prop_assert!(
                        dict.contains_key(&tre.cetag),
                        "CETAG '{}' should be a top-level key in dict. Keys: {:?}",
                        tre.cetag,
                        dict.keys().collect::<Vec<_>>()
                    );
                }

                // 2. Each CETAG maps to a nested dictionary (serde_json::Value::Object)
                for tre in &tres {
                    let value = dict.get(&tre.cetag).unwrap();
                    prop_assert!(
                        value.is_object(),
                        "CETAG '{}' should map to a JSON object, got: {:?}",
                        tre.cetag,
                        value
                    );

                    // 3. The nested dict contains the TRE's field names as keys
                    let nested = value.as_object().unwrap();
                    for (i, field_name) in tre.field_names.iter().enumerate() {
                        prop_assert!(
                            nested.contains_key(field_name),
                            "Nested dict for '{}' should contain field '{}'. Keys: {:?}",
                            tre.cetag,
                            field_name,
                            nested.keys().collect::<Vec<_>>()
                        );

                        // Verify the value matches what we generated
                        let expected_value = &tre.field_values[i];
                        let actual = nested.get(field_name).unwrap();
                        prop_assert_eq!(
                            actual,
                            &serde_json::json!(expected_value),
                            "Field '{}.{}' value mismatch",
                            tre.cetag,
                            field_name
                        );
                    }
                }

                // 4. No dot-separated keys like "CETAG.field" appear in the top-level dict
                for key in dict.keys() {
                    for tre in &tres {
                        let dot_prefix = format!("{}.", tre.cetag);
                        prop_assert!(
                            !key.starts_with(&dot_prefix),
                            "Top-level dict should not contain dot-separated key '{}'. \
                             TRE fields should be nested under the CETAG key.",
                            key
                        );
                    }
                }
            }
        }
    }

    /// Feature: metadata-restructure, Property 2: Unknown TREs Get Raw Representation
    ///
    /// For any JBPSegmentMetadataProvider with TREs that have no definition in the
    /// StructureRegistry, calling entries(None) shall produce a dictionary where each
    /// unknown CETAG is a top-level key mapped to an object containing "_raw"
    /// (hex-encoded bytes) and "_length" (byte count) fields, and _length shall equal
    /// the actual byte length of the TRE data.
    ///
    /// **Validates: Requirements 1.5**
    mod prop_2_unknown_tres_raw_representation {
        use super::*;

        /// Strategy to generate a unique uppercase CETAG for unknown TREs
        fn unknown_cetag_strategy() -> impl Strategy<Value = String> {
            (0u32..100).prop_map(|i| format!("UNKWN{}", i))
        }

        /// Strategy to generate 1-3 unknown TREs with unique CETAGs
        fn unknown_tres_strategy() -> impl Strategy<Value = Vec<(String, Vec<u8>)>> {
            prop::collection::vec(
                (
                    unknown_cetag_strategy(),
                    prop::collection::vec(any::<u8>(), 1..=50),
                ),
                1..=3,
            )
            .prop_map(|tres| {
                // Ensure unique CETAGs by appending index
                tres.into_iter()
                    .enumerate()
                    .map(|(i, (_, data))| (format!("UNKW{:02}", i), data))
                    .collect()
            })
        }

        /// Create a minimal subheader definition and data
        fn create_subheader() -> (Arc<StructureDefinition>, OwnedBuffer) {
            let def = Arc::new(StructureDefinition::new("TestSubheader").with_field(
                FieldDefinition::new("HEADER", FieldType::String).with_size(SizeSpec::Fixed(10)),
            ));
            let data = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());
            (def, data)
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: metadata-restructure, Property 2: Unknown TREs Get Raw Representation
            ///
            /// **Validates: Requirements 1.5**
            #[test]
            fn unknown_tres_get_raw_representation(tres in unknown_tres_strategy()) {
                let (subheader_def, subheader_bytes) = create_subheader();

                // Build envelopes from generated unknown TREs
                let envelopes: Vec<TreEnvelope> = tres
                    .iter()
                    .map(|(tag, data)| TreEnvelope {
                        tag: tag.clone(),
                        data: data.clone(),
                    })
                    .collect();

                // Use an empty registry — no TRE definitions registered,
                // so all TREs are "unknown"
                let registry = StructureRegistry::new();

                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    envelopes,
                    Arc::new(registry),
                );

                let dict = provider.entries(None);

                for (tag, data) in &tres {
                    // 1. Each unknown CETAG is a top-level key
                    prop_assert!(
                        dict.contains_key(tag),
                        "Unknown CETAG '{}' should be a top-level key. Keys: {:?}",
                        tag,
                        dict.keys().collect::<Vec<_>>()
                    );

                    // 2. The value is a JSON object with "_raw" and "_length" keys
                    let value = dict.get(tag).unwrap();
                    prop_assert!(
                        value.is_object(),
                        "Unknown CETAG '{}' should map to a JSON object, got: {:?}",
                        tag,
                        value
                    );

                    let obj = value.as_object().unwrap();
                    prop_assert!(
                        obj.contains_key("_raw"),
                        "Object for '{}' should contain '_raw' key. Keys: {:?}",
                        tag,
                        obj.keys().collect::<Vec<_>>()
                    );
                    prop_assert!(
                        obj.contains_key("_length"),
                        "Object for '{}' should contain '_length' key. Keys: {:?}",
                        tag,
                        obj.keys().collect::<Vec<_>>()
                    );

                    // 3. "_raw" is a hex string matching the input bytes
                    let expected_hex: String =
                        data.iter().map(|b| format!("{:02x}", b)).collect();
                    let raw_value = obj.get("_raw").unwrap();
                    prop_assert_eq!(
                        raw_value,
                        &serde_json::json!(expected_hex),
                        "CETAG '{}' _raw mismatch",
                        tag
                    );

                    // 4. "_length" is a number equal to the byte length of the TRE data
                    let length_value = obj.get("_length").unwrap();
                    prop_assert_eq!(
                        length_value,
                        &serde_json::json!(data.len()),
                        "CETAG '{}' _length should equal {} but got {:?}",
                        tag,
                        data.len(),
                        length_value
                    );
                }
            }
        }
    }

    /// Feature: metadata-restructure, Property 3: Repeated Fields Appear as JSON Arrays
    ///
    /// For any MetadataProvider whose underlying structure definition contains repeated
    /// fields, calling entries() shall produce a dictionary where each repeated field
    /// appears under its base name mapped to a JSON array of element values. No
    /// _N-suffixed keys shall appear for repeated fields.
    ///
    /// **Validates: Requirements 2.1, 2.2, 2.4, 4.1, 4.2, 8.1, 8.2**
    mod prop_3_repeated_fields_as_json_arrays {
        use super::*;
        use crate::parser::{Encoding, Expression, ExpressionEvaluator, RepeatSpec};

        /// Strategy to generate BCS-A field values of a given size (uppercase + digits)
        fn field_value_strategy(size: usize) -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), size)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Build a subheader definition with a count field and a repeated field.
        /// The repeated field uses RepeatSpec::Expression referencing the count field.
        fn create_subheader_with_repeated(
            count_field: &str,
            repeated_field: &str,
            elem_size: usize,
        ) -> StructureDefinition {
            let repeat_expr = ExpressionEvaluator::parse(count_field).unwrap();

            StructureDefinition::new("TestSubheaderRepeat")
                .with_field(
                    FieldDefinition::new("HEADER", FieldType::String)
                        .with_size(SizeSpec::Fixed(10)),
                )
                .with_field(
                    FieldDefinition::new(count_field, FieldType::UnsignedInt(1))
                        .with_size(SizeSpec::Fixed(1)),
                )
                .with_field(
                    FieldDefinition::new(repeated_field, FieldType::String)
                        .with_size(SizeSpec::Fixed(elem_size))
                        .with_encoding(Encoding::BcsA)
                        .with_repeat(RepeatSpec::Expression(repeat_expr)),
                )
        }

        /// Build raw bytes for a subheader with a count field and repeated string elements.
        fn build_subheader_bytes(
            header_value: &str,
            count: u8,
            elem_values: &[String],
            elem_size: usize,
        ) -> OwnedBuffer {
            let mut bytes = Vec::new();
            // HEADER field: 10 bytes, space-padded
            let mut header = header_value.as_bytes().to_vec();
            header.resize(10, b' ');
            bytes.extend_from_slice(&header);
            // Count field: 1 byte unsigned int
            bytes.push(count);
            // Repeated elements: each elem_size bytes, space-padded
            for val in elem_values {
                let mut elem = val.as_bytes().to_vec();
                elem.resize(elem_size, b' ');
                bytes.extend_from_slice(&elem);
            }
            OwnedBuffer::from_vec(bytes)
        }

        /// Build a TRE definition with a count field (BCS-N string) and a repeated field.
        fn create_tre_with_repeated(elem_size: usize) -> StructureDefinition {
            let repeat_expr = Expression::MethodCall {
                target: Box::new(Expression::FieldRef("count".to_string())),
                method: "to_i".to_string(),
            };

            StructureDefinition::new("tre_reptest")
                .with_field(
                    FieldDefinition::new("scalar", FieldType::String)
                        .with_size(SizeSpec::Fixed(4))
                        .with_encoding(Encoding::BcsA),
                )
                .with_field(
                    FieldDefinition::new("count", FieldType::String)
                        .with_size(SizeSpec::Fixed(2))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("items", FieldType::String)
                        .with_size(SizeSpec::Fixed(elem_size))
                        .with_encoding(Encoding::BcsA)
                        .with_repeat(RepeatSpec::Expression(repeat_expr)),
                )
        }

        /// Build raw CEDATA bytes for a TRE with scalar + count + repeated items.
        fn build_tre_data(
            scalar_value: &str,
            count: usize,
            elem_values: &[String],
            elem_size: usize,
        ) -> Vec<u8> {
            let mut data = Vec::new();
            // scalar: 4 bytes, space-padded
            let mut scalar = scalar_value.as_bytes().to_vec();
            scalar.resize(4, b' ');
            data.extend_from_slice(&scalar);
            // count: 2 bytes BCS-N
            data.extend(format!("{:02}", count).as_bytes());
            // repeated items
            for val in elem_values {
                let mut elem = val.as_bytes().to_vec();
                elem.resize(elem_size, b' ');
                data.extend_from_slice(&elem);
            }
            data
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: metadata-restructure, Property 3: Repeated Fields Appear as JSON Arrays
            ///
            /// Subheader repeated fields appear as JSON arrays under the base name
            /// (JBPSegmentMetadataProvider).
            ///
            /// **Validates: Requirements 2.1, 2.2, 8.1**
            #[test]
            fn segment_subheader_repeated_fields_as_arrays(
                count in 1u8..6,
                elem_values in prop::collection::vec(field_value_strategy(5), 6),
            ) {
                let count = count;
                let values: Vec<String> = elem_values.into_iter().take(count as usize).collect();
                let elem_size = 5usize;

                let def = Arc::new(create_subheader_with_repeated("NREP", "ITEMS", elem_size));
                let raw = build_subheader_bytes("TESTHEAD", count, &values, elem_size);

                let provider = JBPSegmentMetadataProvider::from_definition(def, raw);
                let dict = provider.entries(None);

                // 1. The repeated field appears under its base name
                prop_assert!(
                    dict.contains_key("ITEMS"),
                    "Dict should contain 'ITEMS' key. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );

                // 2. The value is a JSON array
                let items_value = dict.get("ITEMS").unwrap();
                prop_assert!(
                    items_value.is_array(),
                    "'ITEMS' should be a JSON array, got: {:?}",
                    items_value
                );

                // 3. The array length matches the count
                let arr = items_value.as_array().unwrap();
                prop_assert_eq!(
                    arr.len(),
                    count as usize,
                    "Array length should equal count {}",
                    count
                );

                // 4. Each element matches the generated value
                for (i, expected) in values.iter().enumerate() {
                    prop_assert_eq!(
                        &arr[i],
                        &serde_json::json!(expected),
                        "ITEMS[{}] value mismatch",
                        i
                    );
                }

                // 5. No _N-suffixed keys appear
                for key in dict.keys() {
                    prop_assert!(
                        !key.starts_with("ITEMS_"),
                        "No _N-suffixed keys should appear, got: {}",
                        key
                    );
                }
            }

            /// Feature: metadata-restructure, Property 3: Repeated Fields Appear as JSON Arrays
            ///
            /// File-header repeated fields appear as JSON arrays under the base name
            /// (JBPFileMetadataProvider).
            ///
            /// **Validates: Requirements 2.1, 2.2, 8.2**
            #[test]
            fn file_header_repeated_fields_as_arrays(
                count in 1u8..6,
                elem_values in prop::collection::vec(field_value_strategy(5), 6),
            ) {
                let count = count;
                let values: Vec<String> = elem_values.into_iter().take(count as usize).collect();
                let elem_size = 5usize;

                let def = Arc::new(create_subheader_with_repeated("NREP", "ITEMS", elem_size));
                let raw = build_subheader_bytes("TESTHEAD", count, &values, elem_size);

                let provider = JBPFileMetadataProvider::from_definition(def, raw);
                let dict = provider.entries(None);

                // 1. The repeated field appears under its base name as a JSON array
                prop_assert!(
                    dict.contains_key("ITEMS"),
                    "Dict should contain 'ITEMS' key. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );

                let items_value = dict.get("ITEMS").unwrap();
                prop_assert!(
                    items_value.is_array(),
                    "'ITEMS' should be a JSON array, got: {:?}",
                    items_value
                );

                let arr = items_value.as_array().unwrap();
                prop_assert_eq!(
                    arr.len(),
                    count as usize,
                    "Array length should equal count {}",
                    count
                );

                // 2. No _N-suffixed keys appear
                for key in dict.keys() {
                    prop_assert!(
                        !key.starts_with("ITEMS_"),
                        "No _N-suffixed keys should appear, got: {}",
                        key
                    );
                }
            }

            /// Feature: metadata-restructure, Property 3: Repeated Fields Appear as JSON Arrays
            ///
            /// TRE repeated fields appear as JSON arrays within the nested TRE dictionary.
            ///
            /// **Validates: Requirements 2.4, 4.1, 4.2**
            #[test]
            fn tre_repeated_fields_as_arrays(
                count in 1usize..6,
                scalar_value in field_value_strategy(4),
                elem_values in prop::collection::vec(field_value_strategy(5), 6),
            ) {
                let values: Vec<String> = elem_values.into_iter().take(count).collect();
                let elem_size = 5usize;

                // Build TRE definition with repeated field
                let tre_def = create_tre_with_repeated(elem_size);
                let cedata = build_tre_data(&scalar_value, count, &values, elem_size);

                // Register the TRE definition
                let mut registry = StructureRegistry::new();
                registry.register("tre_reptest", tre_def);

                // Create a minimal subheader
                let subheader_def = Arc::new(
                    StructureDefinition::new("TestSubheader")
                        .with_field(
                            FieldDefinition::new("HEADER", FieldType::String)
                                .with_size(SizeSpec::Fixed(10)),
                        ),
                );
                let subheader_bytes = OwnedBuffer::from_vec(b"TESTHEAD  ".to_vec());

                let tre_envelope = TreEnvelope {
                    tag: "REPTEST".to_string(),
                    data: cedata,
                };

                let provider = JBPSegmentMetadataProvider::with_tres(
                    subheader_def,
                    subheader_bytes,
                    vec![tre_envelope],
                    Arc::new(registry),
                );

                let dict = provider.entries(None);

                // 1. The CETAG is a top-level key mapped to a nested dict
                prop_assert!(
                    dict.contains_key("REPTEST"),
                    "Dict should contain 'REPTEST' key. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );

                let tre_dict = dict.get("REPTEST").unwrap();
                prop_assert!(
                    tre_dict.is_object(),
                    "'REPTEST' should be a JSON object, got: {:?}",
                    tre_dict
                );

                let nested = tre_dict.as_object().unwrap();

                // 2. The scalar field is present as a direct value
                prop_assert!(
                    nested.contains_key("scalar"),
                    "Nested dict should contain 'scalar'. Keys: {:?}",
                    nested.keys().collect::<Vec<_>>()
                );
                prop_assert_eq!(
                    nested.get("scalar").unwrap(),
                    &serde_json::json!(scalar_value),
                    "scalar value mismatch"
                );

                // 3. The repeated field appears under its base name as a JSON array
                prop_assert!(
                    nested.contains_key("items"),
                    "Nested dict should contain 'items'. Keys: {:?}",
                    nested.keys().collect::<Vec<_>>()
                );

                let items_value = nested.get("items").unwrap();
                prop_assert!(
                    items_value.is_array(),
                    "'items' should be a JSON array, got: {:?}",
                    items_value
                );

                // 4. The array length matches the count
                let arr = items_value.as_array().unwrap();
                prop_assert_eq!(
                    arr.len(),
                    count,
                    "Array length should equal count {}",
                    count
                );

                // 5. Each element matches the generated value
                for (i, expected) in values.iter().enumerate() {
                    prop_assert_eq!(
                        &arr[i],
                        &serde_json::json!(expected),
                        "items[{}] value mismatch",
                        i
                    );
                }

                // 6. No _N-suffixed keys appear in the nested TRE dict
                for key in nested.keys() {
                    prop_assert!(
                        !key.starts_with("items_"),
                        "No _N-suffixed keys should appear in TRE dict, got: {}",
                        key
                    );
                }
            }
        }
    }

    /// Feature: metadata-restructure, Property 4: Prefix Filter Returns Only Matching Entries
    ///
    /// For any JBPSegmentMetadataProvider and any prefix string, entries(Some(prefix))
    /// shall return only entries whose top-level keys start with the given prefix. For
    /// TRE entries, the CETAG itself is the top-level key. For subheader fields, the
    /// field name is the top-level key. A non-matching prefix shall produce an empty
    /// dictionary.
    ///
    /// **Validates: Requirements 1.4, 7.1, 7.2, 7.3**
    mod prop_4_prefix_filter_nested_output {
        use super::*;
        use crate::parser::Encoding;

        /// Strategy to generate a BCS-A field value of a given size
        fn field_value_strategy(size: usize) -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), size)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Create a simple TRE definition with two fixed-size string fields
        fn create_tre_definition(name: &str) -> StructureDefinition {
            StructureDefinition::new(name)
                .with_field(
                    FieldDefinition::new("field1", FieldType::String)
                        .with_size(SizeSpec::Fixed(8))
                        .with_encoding(Encoding::BcsA),
                )
                .with_field(
                    FieldDefinition::new("field2", FieldType::String)
                        .with_size(SizeSpec::Fixed(6))
                        .with_encoding(Encoding::BcsA),
                )
        }

        /// Create a subheader definition with fields that have distinct prefixes
        fn create_subheader_definition() -> Arc<StructureDefinition> {
            Arc::new(
                StructureDefinition::new("TestSubheader")
                    .with_field(
                        FieldDefinition::new("HEADER", FieldType::String)
                            .with_size(SizeSpec::Fixed(10)),
                    )
                    .with_field(
                        FieldDefinition::new("HEADTYPE", FieldType::String)
                            .with_size(SizeSpec::Fixed(4)),
                    )
                    .with_field(
                        FieldDefinition::new("FSCLAS", FieldType::String)
                            .with_size(SizeSpec::Fixed(1)),
                    )
                    .with_field(
                        FieldDefinition::new("FSCLSY", FieldType::String)
                            .with_size(SizeSpec::Fixed(2)),
                    ),
            )
        }

        /// Create subheader raw bytes matching the definition above
        fn create_subheader_bytes() -> OwnedBuffer {
            // HEADER(10) + HEADTYPE(4) + FSCLAS(1) + FSCLSY(2) = 17 bytes
            OwnedBuffer::from_vec(b"TESTHEAD  IMG U  ".to_vec())
        }

        /// Build a provider with subheader fields and two known TREs (GEOLOB, SENSRB)
        fn build_provider(
            geolob_f1: &str,
            geolob_f2: &str,
            sensrb_f1: &str,
            sensrb_f2: &str,
        ) -> JBPSegmentMetadataProvider {
            let subheader_def = create_subheader_definition();
            let subheader_bytes = create_subheader_bytes();

            let mut registry = StructureRegistry::new();
            registry.register("tre_geolob", create_tre_definition("tre_geolob"));
            registry.register("tre_sensrb", create_tre_definition("tre_sensrb"));

            let mut geolob_data = Vec::new();
            let mut f1 = geolob_f1.as_bytes().to_vec();
            f1.resize(8, b' ');
            geolob_data.extend_from_slice(&f1);
            let mut f2 = geolob_f2.as_bytes().to_vec();
            f2.resize(6, b' ');
            geolob_data.extend_from_slice(&f2);

            let mut sensrb_data = Vec::new();
            let mut sf1 = sensrb_f1.as_bytes().to_vec();
            sf1.resize(8, b' ');
            sensrb_data.extend_from_slice(&sf1);
            let mut sf2 = sensrb_f2.as_bytes().to_vec();
            sf2.resize(6, b' ');
            sensrb_data.extend_from_slice(&sf2);

            let envelopes = vec![
                TreEnvelope {
                    tag: "GEOLOB".to_string(),
                    data: geolob_data,
                },
                TreEnvelope {
                    tag: "SENSRB".to_string(),
                    data: sensrb_data,
                },
            ];

            JBPSegmentMetadataProvider::with_tres(
                subheader_def,
                subheader_bytes,
                envelopes,
                Arc::new(registry),
            )
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: metadata-restructure, Property 4: Prefix Filter Returns Only Matching Entries
            ///
            /// Prefix matching a CETAG exactly returns only that TRE's nested dict.
            ///
            /// **Validates: Requirements 1.4, 7.1**
            #[test]
            fn prefix_matching_cetag_returns_nested_tre(
                gf1 in field_value_strategy(8),
                gf2 in field_value_strategy(6),
                sf1 in field_value_strategy(8),
                sf2 in field_value_strategy(6),
            ) {
                let provider = build_provider(&gf1, &gf2, &sf1, &sf2);

                // Filter by exact CETAG "GEOLOB"
                let dict = provider.entries(Some("GEOLOB"));

                // Should contain the GEOLOB key with nested dict
                prop_assert!(
                    dict.contains_key("GEOLOB"),
                    "Should contain 'GEOLOB' key. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );

                let geolob = dict.get("GEOLOB").unwrap();
                prop_assert!(
                    geolob.is_object(),
                    "'GEOLOB' should be a nested dict, got: {:?}",
                    geolob
                );

                // Should NOT contain SENSRB or subheader fields
                prop_assert!(
                    !dict.contains_key("SENSRB"),
                    "Should not contain 'SENSRB' when filtering by 'GEOLOB'"
                );
                prop_assert!(
                    !dict.contains_key("HEADER"),
                    "Should not contain subheader field 'HEADER'"
                );
                prop_assert!(
                    !dict.contains_key("FSCLAS"),
                    "Should not contain subheader field 'FSCLAS'"
                );
            }

            /// Feature: metadata-restructure, Property 4: Prefix Filter Returns Only Matching Entries
            ///
            /// Prefix matching subheader fields returns only those fields, no TREs.
            ///
            /// **Validates: Requirements 7.2**
            #[test]
            fn prefix_matching_subheader_fields_only(
                gf1 in field_value_strategy(8),
                gf2 in field_value_strategy(6),
                sf1 in field_value_strategy(8),
                sf2 in field_value_strategy(6),
            ) {
                let provider = build_provider(&gf1, &gf2, &sf1, &sf2);

                // Filter by "FS" prefix — matches FSCLAS and FSCLSY
                let dict = provider.entries(Some("FS"));

                // Should contain only subheader fields starting with "FS"
                prop_assert!(
                    dict.contains_key("FSCLAS"),
                    "Should contain 'FSCLAS'. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );
                prop_assert!(
                    dict.contains_key("FSCLSY"),
                    "Should contain 'FSCLSY'. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );

                // Should NOT contain TRE entries or non-matching subheader fields
                prop_assert!(
                    !dict.contains_key("GEOLOB"),
                    "Should not contain TRE 'GEOLOB' when filtering by 'FS'"
                );
                prop_assert!(
                    !dict.contains_key("SENSRB"),
                    "Should not contain TRE 'SENSRB' when filtering by 'FS'"
                );
                prop_assert!(
                    !dict.contains_key("HEADER"),
                    "Should not contain 'HEADER' when filtering by 'FS'"
                );

                // All returned keys must start with "FS"
                for key in dict.keys() {
                    prop_assert!(
                        key.starts_with("FS"),
                        "All keys should start with 'FS', got: {}",
                        key
                    );
                }
            }

            /// Feature: metadata-restructure, Property 4: Prefix Filter Returns Only Matching Entries
            ///
            /// Non-matching prefix returns an empty dictionary.
            ///
            /// **Validates: Requirements 7.3**
            #[test]
            fn nonmatching_prefix_returns_empty(
                gf1 in field_value_strategy(8),
                gf2 in field_value_strategy(6),
                sf1 in field_value_strategy(8),
                sf2 in field_value_strategy(6),
            ) {
                let provider = build_provider(&gf1, &gf2, &sf1, &sf2);

                // Filter by prefix that matches nothing
                let dict = provider.entries(Some("ZZZZZ"));

                prop_assert!(
                    dict.is_empty(),
                    "Non-matching prefix should return empty dict, got {} entries: {:?}",
                    dict.len(),
                    dict.keys().collect::<Vec<_>>()
                );
            }

            /// Feature: metadata-restructure, Property 4: Prefix Filter Returns Only Matching Entries
            ///
            /// A prefix that is a prefix of a CETAG includes that TRE entry because
            /// tag.starts_with(prefix) is true (e.g., "GEO" matches "GEOLOB").
            ///
            /// **Validates: Requirements 1.4, 7.1**
            #[test]
            fn partial_prefix_of_cetag_includes_tre(
                gf1 in field_value_strategy(8),
                gf2 in field_value_strategy(6),
                sf1 in field_value_strategy(8),
                sf2 in field_value_strategy(6),
            ) {
                let provider = build_provider(&gf1, &gf2, &sf1, &sf2);

                // "GEO" is a prefix of "GEOLOB" — tag.starts_with(prefix) is true
                let dict = provider.entries(Some("GEO"));

                prop_assert!(
                    dict.contains_key("GEOLOB"),
                    "'GEO' prefix should match 'GEOLOB' TRE. Keys: {:?}",
                    dict.keys().collect::<Vec<_>>()
                );

                // SENSRB should not match "GEO"
                prop_assert!(
                    !dict.contains_key("SENSRB"),
                    "'GEO' prefix should not match 'SENSRB'"
                );

                // Verify the GEOLOB entry is a nested dict with correct values
                let geolob = dict.get("GEOLOB").unwrap();
                prop_assert!(
                    geolob.is_object(),
                    "'GEOLOB' should be a nested dict"
                );
            }

            /// Feature: metadata-restructure, Property 4: Prefix Filter Returns Only Matching Entries
            ///
            /// For any random prefix, all returned top-level keys satisfy simple
            /// `key.starts_with(prefix)` matching (no bidirectional TRE matching).
            ///
            /// **Validates: Requirements 1.4, 7.1, 7.2, 7.3**
            #[test]
            fn all_returned_keys_satisfy_prefix_filter(
                gf1 in field_value_strategy(8),
                gf2 in field_value_strategy(6),
                sf1 in field_value_strategy(8),
                sf2 in field_value_strategy(6),
                prefix in "[A-Z]{1,8}",
            ) {
                let provider = build_provider(&gf1, &gf2, &sf1, &sf2);

                let dict = provider.entries(Some(&prefix));

                for key in dict.keys() {
                    prop_assert!(
                        key.starts_with(&prefix),
                        "Key '{}' should start with prefix '{}'",
                        key,
                        prefix
                    );
                }
            }
        }
    }

    /// Feature: metadata-restructure, Property 6: Struct Resolution Produces Nested Dictionaries
    ///
    /// For any Value::Struct whose type_name exists in the StructureRegistry, calling
    /// value_to_json(value, Some(registry)) shall produce a serde_json::Value::Object
    /// with keys matching the field IDs from the type definition and values recursively
    /// converted. Nested Value::Struct fields within the resolved struct shall also be
    /// recursively resolved. Repeated fields within the resolved struct shall appear as
    /// JSON arrays.
    ///
    /// **Validates: Requirements 3.1, 3.3, 3.4, 6.3**
    mod prop_6_struct_resolution_nested_dicts {
        use super::*;
        use crate::parser::{Encoding, Expression, RepeatSpec};

        /// Strategy to generate a BCS-A field value of a given size (uppercase + digits)
        fn field_value_strategy(size: usize) -> impl Strategy<Value = String> {
            prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), size)
                .prop_map(|chars| chars.into_iter().collect::<String>())
        }

        /// Build raw bytes for a flat struct with N string fields of given size.
        fn build_flat_struct_bytes(values: &[String], field_size: usize) -> Vec<u8> {
            let mut data = Vec::new();
            for val in values {
                let mut field_bytes = val.as_bytes().to_vec();
                field_bytes.resize(field_size, b' ');
                data.extend_from_slice(&field_bytes);
            }
            data
        }

        /// Build a StructureDefinition with N string fields of given size.
        fn build_flat_struct_def(
            type_id: &str,
            field_names: &[&str],
            field_size: usize,
        ) -> StructureDefinition {
            let mut def = StructureDefinition::new(type_id);
            for name in field_names {
                def = def.with_field(
                    FieldDefinition::new(*name, FieldType::String)
                        .with_size(SizeSpec::Fixed(field_size))
                        .with_encoding(Encoding::BcsA),
                );
            }
            def
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Feature: metadata-restructure, Property 6: Struct Resolution Produces Nested Dictionaries
            ///
            /// A Value::Struct with a known type resolves to a JSON object with correct keys and values.
            ///
            /// **Validates: Requirements 3.1, 6.3**
            #[test]
            fn struct_resolves_to_nested_dict(
                val_a in field_value_strategy(6),
                val_b in field_value_strategy(6),
                val_c in field_value_strategy(6),
            ) {
                let field_size = 6usize;
                let field_names = ["alpha", "bravo", "charlie"];
                let values = [val_a.clone(), val_b.clone(), val_c.clone()];

                // Build the struct definition and register it
                let def = build_flat_struct_def("my_struct_type", &field_names, field_size);
                let mut registry = StructureRegistry::new();
                registry.register("my_struct_type", def);

                // Build raw data for the struct
                let raw = build_flat_struct_bytes(&values, field_size);

                // Create Value::Struct
                let value = Value::from_struct(&raw, "my_struct_type");

                // Call value_to_json with registry
                let json = value_to_json(&value, Some(&registry), None);
                prop_assert!(json.is_some(), "value_to_json should return Some");

                let json = json.unwrap();

                // 1. Returns serde_json::Value::Object
                prop_assert!(
                    json.is_object(),
                    "Resolved struct should be a JSON object, got: {:?}",
                    json
                );

                let obj = json.as_object().unwrap();

                // 2. Object keys match field IDs from the type definition
                for name in &field_names {
                    prop_assert!(
                        obj.contains_key(*name),
                        "Object should contain key '{}'. Keys: {:?}",
                        name,
                        obj.keys().collect::<Vec<_>>()
                    );
                }

                // 3. Values are correctly converted (strings trimmed)
                for (i, name) in field_names.iter().enumerate() {
                    prop_assert_eq!(
                        obj.get(*name).unwrap(),
                        &serde_json::json!(values[i]),
                        "Field '{}' value mismatch",
                        name
                    );
                }

                // Should NOT have _type/_data fallback keys
                prop_assert!(
                    !obj.contains_key("_type"),
                    "Resolved struct should not have '_type' key"
                );
                prop_assert!(
                    !obj.contains_key("_data"),
                    "Resolved struct should not have '_data' key"
                );
            }

            /// Feature: metadata-restructure, Property 6: Struct Resolution Produces Nested Dictionaries
            ///
            /// Nested Value::Struct fields are recursively resolved when both types are in the registry.
            ///
            /// **Validates: Requirements 3.3, 6.3**
            #[test]
            fn nested_structs_recursively_resolved(
                outer_val in field_value_strategy(5),
                inner_val_a in field_value_strategy(4),
                inner_val_b in field_value_strategy(4),
            ) {
                // Inner struct: two 4-byte string fields
                let inner_def = build_flat_struct_def("inner_type", &["x", "y"], 4);
                let inner_bytes = build_flat_struct_bytes(
                    &[inner_val_a.clone(), inner_val_b.clone()],
                    4,
                );
                let inner_size = inner_bytes.len(); // 8 bytes

                // Outer struct: one 5-byte string field + one TypeRef field (inner struct)
                let outer_def = StructureDefinition::new("outer_type")
                    .with_field(
                        FieldDefinition::new("label", FieldType::String)
                            .with_size(SizeSpec::Fixed(5))
                            .with_encoding(Encoding::BcsA),
                    )
                    .with_field(
                        FieldDefinition::new("nested", FieldType::TypeRef("inner_type".to_string()))
                            .with_size(SizeSpec::Fixed(inner_size)),
                    );

                let mut registry = StructureRegistry::new();
                registry.register("inner_type", inner_def);
                registry.register("outer_type", outer_def);

                // Build outer raw bytes: label(5) + nested(inner_size)
                let mut outer_bytes = Vec::new();
                let mut label_bytes = outer_val.as_bytes().to_vec();
                label_bytes.resize(5, b' ');
                outer_bytes.extend_from_slice(&label_bytes);
                outer_bytes.extend_from_slice(&inner_bytes);

                let value = Value::from_struct(&outer_bytes, "outer_type");
                let json = value_to_json(&value, Some(&registry), None);
                prop_assert!(json.is_some(), "value_to_json should return Some");

                let json = json.unwrap();
                prop_assert!(json.is_object(), "Outer struct should be a JSON object");

                let obj = json.as_object().unwrap();

                // Outer label field is present and correct
                prop_assert_eq!(
                    obj.get("label").unwrap(),
                    &serde_json::json!(outer_val),
                    "Outer 'label' value mismatch"
                );

                // Nested field is present and is itself a JSON object (recursively resolved)
                let nested = obj.get("nested");
                prop_assert!(
                    nested.is_some(),
                    "Outer struct should contain 'nested' key"
                );

                let nested = nested.unwrap();
                prop_assert!(
                    nested.is_object(),
                    "Nested struct should be a JSON object, got: {:?}",
                    nested
                );

                let nested_obj = nested.as_object().unwrap();
                prop_assert_eq!(
                    nested_obj.get("x").unwrap(),
                    &serde_json::json!(inner_val_a),
                    "Nested field 'x' value mismatch"
                );
                prop_assert_eq!(
                    nested_obj.get("y").unwrap(),
                    &serde_json::json!(inner_val_b),
                    "Nested field 'y' value mismatch"
                );
            }

            /// Feature: metadata-restructure, Property 6: Struct Resolution Produces Nested Dictionaries
            ///
            /// Repeated fields within a resolved struct appear as JSON arrays.
            ///
            /// **Validates: Requirements 3.4, 6.3**
            #[test]
            fn struct_with_repeated_fields_produces_arrays(
                scalar_val in field_value_strategy(4),
                count in 1usize..5,
                elem_values in prop::collection::vec(field_value_strategy(3), 5),
            ) {
                let elems: Vec<String> = elem_values.into_iter().take(count).collect();
                let elem_size = 3usize;

                // Build a struct definition with: scalar(4) + count(2 BCS-N) + items(repeated, 3 each)
                let repeat_expr = Expression::MethodCall {
                    target: Box::new(Expression::FieldRef("count".to_string())),
                    method: "to_i".to_string(),
                };

                let struct_def = StructureDefinition::new("struct_with_repeat")
                    .with_field(
                        FieldDefinition::new("scalar", FieldType::String)
                            .with_size(SizeSpec::Fixed(4))
                            .with_encoding(Encoding::BcsA),
                    )
                    .with_field(
                        FieldDefinition::new("count", FieldType::String)
                            .with_size(SizeSpec::Fixed(2))
                            .with_encoding(Encoding::BcsN),
                    )
                    .with_field(
                        FieldDefinition::new("items", FieldType::String)
                            .with_size(SizeSpec::Fixed(elem_size))
                            .with_encoding(Encoding::BcsA)
                            .with_repeat(RepeatSpec::Expression(repeat_expr)),
                    );

                let mut registry = StructureRegistry::new();
                registry.register("struct_with_repeat", struct_def);

                // Build raw bytes: scalar(4) + count(2) + items(count * elem_size)
                let mut raw = Vec::new();
                let mut scalar_bytes = scalar_val.as_bytes().to_vec();
                scalar_bytes.resize(4, b' ');
                raw.extend_from_slice(&scalar_bytes);
                raw.extend(format!("{:02}", count).as_bytes());
                for val in &elems {
                    let mut elem_bytes = val.as_bytes().to_vec();
                    elem_bytes.resize(elem_size, b' ');
                    raw.extend_from_slice(&elem_bytes);
                }

                let value = Value::from_struct(&raw, "struct_with_repeat");
                let json = value_to_json(&value, Some(&registry), None);
                prop_assert!(json.is_some(), "value_to_json should return Some");

                let json = json.unwrap();
                prop_assert!(json.is_object(), "Struct should be a JSON object");

                let obj = json.as_object().unwrap();

                // Scalar field is present
                prop_assert_eq!(
                    obj.get("scalar").unwrap(),
                    &serde_json::json!(scalar_val),
                    "scalar value mismatch"
                );

                // Repeated field appears as a JSON array
                let items = obj.get("items");
                prop_assert!(
                    items.is_some(),
                    "Object should contain 'items' key. Keys: {:?}",
                    obj.keys().collect::<Vec<_>>()
                );

                let items = items.unwrap();
                prop_assert!(
                    items.is_array(),
                    "'items' should be a JSON array, got: {:?}",
                    items
                );

                let arr = items.as_array().unwrap();
                prop_assert_eq!(
                    arr.len(),
                    count,
                    "Array length should equal count {}",
                    count
                );

                for (i, expected) in elems.iter().enumerate() {
                    prop_assert_eq!(
                        &arr[i],
                        &serde_json::json!(expected),
                        "items[{}] value mismatch",
                        i
                    );
                }
            }

            /// Feature: metadata-restructure, Property 6: Struct Resolution Produces Nested Dictionaries
            ///
            /// A Value::Struct whose type_name is NOT in the registry falls back to
            /// {"_type": "...", "_data": "..."} representation.
            ///
            /// **Validates: Requirements 3.1, 6.3**
            #[test]
            fn unregistered_struct_falls_back_to_opaque(
                data_bytes in prop::collection::vec(any::<u8>(), 1..=30),
                type_name in "[a-z_]{3,12}",
            ) {
                // Empty registry — no types registered
                let registry = StructureRegistry::new();

                let value = Value::from_struct(&data_bytes, type_name.clone());
                let json = value_to_json(&value, Some(&registry), None);
                prop_assert!(json.is_some(), "value_to_json should return Some");

                let json = json.unwrap();
                prop_assert!(
                    json.is_object(),
                    "Fallback struct should be a JSON object, got: {:?}",
                    json
                );

                let obj = json.as_object().unwrap();

                // Should have _type and _data keys
                prop_assert!(
                    obj.contains_key("_type"),
                    "Fallback should contain '_type' key. Keys: {:?}",
                    obj.keys().collect::<Vec<_>>()
                );
                prop_assert!(
                    obj.contains_key("_data"),
                    "Fallback should contain '_data' key. Keys: {:?}",
                    obj.keys().collect::<Vec<_>>()
                );

                // _type matches the type_name
                prop_assert_eq!(
                    obj.get("_type").unwrap(),
                    &serde_json::json!(type_name),
                    "_type should match the struct's type_name"
                );

                // _data is hex-encoded bytes
                let expected_hex: String =
                    data_bytes.iter().map(|b| format!("{:02x}", b)).collect();
                prop_assert_eq!(
                    obj.get("_data").unwrap(),
                    &serde_json::json!(expected_hex),
                    "_data should be hex-encoded bytes"
                );

                // Should NOT have resolved field keys
                prop_assert!(
                    !obj.keys().any(|k| k != "_type" && k != "_data"),
                    "Fallback should only have '_type' and '_data' keys, got: {:?}",
                    obj.keys().collect::<Vec<_>>()
                );
            }
        }
    }
}
