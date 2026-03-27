//! J2KMetadataProvider — implements MetadataProvider for JPEG 2000 metadata.
//!
//! Provides dataset-level metadata extracted from the J2K codestream SIZ marker:
//! width, height, num_components, bits_per_component, is_signed, tile dimensions,
//! and compression_type. Entries are stored as `HashMap<String, serde_json::Value>`.
//!
//! - `as_dict(None)` → all entries
//! - `as_dict(Some(prefix))` → entries whose key starts with `prefix`
//! - `raw()` → empty slice (no single raw binary representation)

use std::collections::HashMap;

use serde_json::Value;

use crate::traits::metadata::MetadataProvider;

/// Metadata provider for JPEG 2000 codestream data.
///
/// Stores metadata entries as a flat `HashMap<String, serde_json::Value>`.
/// The entries include:
///
/// - `"width"` — image width in pixels
/// - `"height"` — image height in pixels
/// - `"num_components"` — number of image components (bands)
/// - `"bits_per_component"` — bits per component sample
/// - `"is_signed"` — whether component samples are signed
/// - `"tile_width"` — tile width in pixels
/// - `"tile_height"` — tile height in pixels
/// - `"num_tiles_x"` — number of tiles horizontally
/// - `"num_tiles_y"` — number of tiles vertically
/// - `"compression_type"` — `"j2k"` or `"jp2"`
pub struct J2KMetadataProvider {
    entries: HashMap<String, Value>,
}

impl J2KMetadataProvider {
    /// Create a new `J2KMetadataProvider` from pre-built metadata entries.
    ///
    /// The caller (typically `J2KDatasetReader`) is responsible for parsing
    /// the SIZ marker and building the entries map.
    pub fn new(entries: HashMap<String, Value>) -> Self {
        Self { entries }
    }
}

impl MetadataProvider for J2KMetadataProvider {
    fn raw(&self) -> &[u8] {
        &[]
    }

    fn as_dict(&self, name: Option<&str>) -> HashMap<String, Value> {
        match name {
            None => self.entries.clone(),
            Some(prefix) => self
                .entries
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: build a provider with typical J2K metadata entries.
    fn sample_provider() -> J2KMetadataProvider {
        let mut entries = HashMap::new();
        entries.insert("width".to_string(), json!(1024));
        entries.insert("height".to_string(), json!(768));
        entries.insert("num_components".to_string(), json!(3));
        entries.insert("bits_per_component".to_string(), json!(8));
        entries.insert("is_signed".to_string(), json!(false));
        entries.insert("tile_width".to_string(), json!(512));
        entries.insert("tile_height".to_string(), json!(512));
        entries.insert("num_tiles_x".to_string(), json!(2));
        entries.insert("num_tiles_y".to_string(), json!(2));
        entries.insert("compression_type".to_string(), json!("j2k"));
        J2KMetadataProvider::new(entries)
    }

    #[test]
    fn test_as_dict_none_returns_all_entries() {
        let provider = sample_provider();
        let dict = provider.as_dict(None);
        assert_eq!(dict.len(), 10);
        assert_eq!(dict.get("width").and_then(|v| v.as_u64()), Some(1024));
        assert_eq!(dict.get("height").and_then(|v| v.as_u64()), Some(768));
        assert_eq!(dict.get("num_components").and_then(|v| v.as_u64()), Some(3));
        assert_eq!(
            dict.get("bits_per_component").and_then(|v| v.as_u64()),
            Some(8)
        );
        assert_eq!(
            dict.get("is_signed").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            dict.get("compression_type").and_then(|v| v.as_str()),
            Some("j2k")
        );
    }

    #[test]
    fn test_as_dict_prefix_filters_correctly() {
        let provider = sample_provider();
        let filtered = provider.as_dict(Some("tile"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("tile_width"));
        assert!(filtered.contains_key("tile_height"));
    }

    #[test]
    fn test_as_dict_prefix_num() {
        let provider = sample_provider();
        let filtered = provider.as_dict(Some("num_"));
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains_key("num_components"));
        assert!(filtered.contains_key("num_tiles_x"));
        assert!(filtered.contains_key("num_tiles_y"));
    }

    #[test]
    fn test_as_dict_empty_prefix_returns_all() {
        let provider = sample_provider();
        assert_eq!(provider.as_dict(Some("")), provider.as_dict(None));
    }

    #[test]
    fn test_as_dict_unknown_prefix_returns_empty() {
        let provider = sample_provider();
        assert!(provider.as_dict(Some("zzz")).is_empty());
    }

    #[test]
    fn test_raw_returns_empty() {
        let provider = sample_provider();
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn test_empty_provider() {
        let provider = J2KMetadataProvider::new(HashMap::new());
        assert!(provider.as_dict(None).is_empty());
        assert!(provider.as_dict(Some("any")).is_empty());
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn test_jp2_compression_type() {
        let mut entries = HashMap::new();
        entries.insert("compression_type".to_string(), json!("jp2"));
        let provider = J2KMetadataProvider::new(entries);
        let dict = provider.as_dict(None);
        assert_eq!(
            dict.get("compression_type").and_then(|v| v.as_str()),
            Some("jp2")
        );
    }
}
