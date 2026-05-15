//! JPEGMetadataProvider — implements MetadataProvider for JPEG metadata.
//!
//! Provides dataset-level metadata extracted from the JPEG SOF marker:
//! width, height, num_components, bits_per_pixel (always 8), and color_space.
//! Entries are stored as `HashMap<String, serde_json::Value>`.
//!
//! - `entries(None)` → all entries
//! - `entries(Some(prefix))` → entries whose key starts with `prefix`
//! - `raw()` → empty slice (no single raw binary representation)

use std::collections::HashMap;

use serde_json::Value;

use crate::traits::metadata::MetadataProvider;

/// Metadata provider for JPEG image data.
///
/// Stores metadata entries as a flat `HashMap<String, serde_json::Value>`.
/// The entries include:
///
/// - `"width"` — image width in pixels
/// - `"height"` — image height in pixels
/// - `"num_components"` — number of image components (1 for grayscale, 3 for RGB)
/// - `"bits_per_pixel"` — bits per pixel (always 8 for baseline JPEG)
/// - `"color_space"` — `"Grayscale"` or `"RGB"`
pub struct JPEGMetadataProvider {
    entries: HashMap<String, Value>,
}

impl JPEGMetadataProvider {
    /// Create a new `JPEGMetadataProvider` from pre-built metadata entries.
    ///
    /// The caller (typically `JPEGDatasetReader`) is responsible for parsing
    /// the SOF marker and building the entries map.
    pub fn new(entries: HashMap<String, Value>) -> Self {
        Self { entries }
    }
}

impl MetadataProvider for JPEGMetadataProvider {
    fn raw(&self) -> &[u8] {
        &[]
    }

    fn entries(&self, name: Option<&str>) -> HashMap<String, Value> {
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

    fn sample_provider() -> JPEGMetadataProvider {
        let mut entries = HashMap::new();
        entries.insert("width".to_string(), json!(1024));
        entries.insert("height".to_string(), json!(768));
        entries.insert("num_components".to_string(), json!(3));
        entries.insert("bits_per_pixel".to_string(), json!(8));
        entries.insert("color_space".to_string(), json!("RGB"));
        JPEGMetadataProvider::new(entries)
    }

    #[test]
    fn test_entries_none_returns_all_entries() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        assert_eq!(dict.len(), 5);
        assert_eq!(dict.get("width").and_then(|v| v.as_u64()), Some(1024));
        assert_eq!(dict.get("height").and_then(|v| v.as_u64()), Some(768));
        assert_eq!(dict.get("num_components").and_then(|v| v.as_u64()), Some(3));
        assert_eq!(dict.get("bits_per_pixel").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(
            dict.get("color_space").and_then(|v| v.as_str()),
            Some("RGB")
        );
    }

    #[test]
    fn test_entries_prefix_filters_correctly() {
        let provider = sample_provider();
        let filtered = provider.entries(Some("num_"));
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("num_components"));
    }

    #[test]
    fn test_entries_prefix_bits() {
        let provider = sample_provider();
        let filtered = provider.entries(Some("bits"));
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("bits_per_pixel"));
    }

    #[test]
    fn test_entries_empty_prefix_returns_all() {
        let provider = sample_provider();
        assert_eq!(provider.entries(Some("")), provider.entries(None));
    }

    #[test]
    fn test_entries_unknown_prefix_returns_empty() {
        let provider = sample_provider();
        assert!(provider.entries(Some("zzz")).is_empty());
    }

    #[test]
    fn test_raw_returns_empty() {
        let provider = sample_provider();
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn test_empty_provider() {
        let provider = JPEGMetadataProvider::new(HashMap::new());
        assert!(provider.entries(None).is_empty());
        assert!(provider.entries(Some("any")).is_empty());
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn test_grayscale_color_space() {
        let mut entries = HashMap::new();
        entries.insert("color_space".to_string(), json!("Grayscale"));
        let provider = JPEGMetadataProvider::new(entries);
        let dict = provider.entries(None);
        assert_eq!(
            dict.get("color_space").and_then(|v| v.as_str()),
            Some("Grayscale")
        );
    }
}
