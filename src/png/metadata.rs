//! PNGMetadataProvider — implements MetadataProvider for PNG chunk metadata.
//!
//! Provides per-image metadata extracted from PNG ancillary chunks (tEXt, iTXt,
//! zTXt, tIME, gAMA, pHYs, PLTE) and dataset-level metadata (width, height,
//! bit_depth, color_type). Entries are stored as `HashMap<String, serde_json::Value>`.
//!
//! - `entries(None)` → all entries
//! - `entries(Some(prefix))` → entries whose key starts with `prefix`
//! - `raw()` → empty slice (PNG has no single raw binary representation)

use std::collections::HashMap;

use serde_json::Value;

use crate::traits::metadata::MetadataProvider;

/// Metadata provider for PNG chunk data.
///
/// Stores PNG metadata entries as a flat `HashMap<String, serde_json::Value>`.
/// The entries include:
///
/// - **Text chunks** (tEXt, iTXt, zTXt): keyword → string value
/// - **tIME**: `"tIME"` → ISO 8601 string
/// - **gAMA**: `"gAMA"` → float (gamma value)
/// - **pHYs**: `"pHYs"` → JSON object `{"x": ..., "y": ..., "unit": ...}`
/// - **PLTE**: `"PLTE"` → JSON array of `[R, G, B]` triples
/// - **Dataset-level**: `"width"`, `"height"`, `"bit_depth"`, `"color_type"`
pub struct PNGMetadataProvider {
    entries: HashMap<String, Value>,
}

impl PNGMetadataProvider {
    /// Create a new `PNGMetadataProvider` from pre-built metadata entries.
    ///
    /// The caller (typically `PNGDatasetReader`) is responsible for extracting
    /// chunk data from the `png` crate's `Reader` and building the entries map.
    pub fn new(entries: HashMap<String, Value>) -> Self {
        Self { entries }
    }
}

impl MetadataProvider for PNGMetadataProvider {
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

    /// Helper: build a provider with typical PNG metadata entries.
    fn sample_provider() -> PNGMetadataProvider {
        let mut entries = HashMap::new();
        // Dataset-level metadata
        entries.insert("width".to_string(), json!(256));
        entries.insert("height".to_string(), json!(128));
        entries.insert("bit_depth".to_string(), json!(8));
        entries.insert("color_type".to_string(), json!("RGB"));
        // Text chunks
        entries.insert("Author".to_string(), json!("Test User"));
        entries.insert("Description".to_string(), json!("A test image"));
        // Ancillary chunks
        entries.insert("tIME".to_string(), json!("2025-01-15T12:30:00Z"));
        entries.insert("gAMA".to_string(), json!(2.2));
        entries.insert("pHYs".to_string(), json!({"x": 3780, "y": 3780, "unit": 1}));
        entries.insert(
            "PLTE".to_string(),
            json!([[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
        );
        PNGMetadataProvider::new(entries)
    }

    // =========================================================================
    // entries(None) tests (Req 4.9)
    // =========================================================================

    #[test]
    fn test_entries_none_returns_all_entries() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        assert_eq!(dict.len(), 10);
        assert_eq!(dict.get("width").and_then(|v| v.as_u64()), Some(256));
        assert_eq!(dict.get("height").and_then(|v| v.as_u64()), Some(128));
        assert_eq!(dict.get("bit_depth").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(dict.get("color_type").and_then(|v| v.as_str()), Some("RGB"));
        assert_eq!(
            dict.get("Author").and_then(|v| v.as_str()),
            Some("Test User")
        );
    }

    // =========================================================================
    // Prefix filter tests (Req 4.8)
    // =========================================================================

    #[test]
    fn test_entries_prefix_filters_correctly() {
        let provider = sample_provider();
        // "t" prefix should match "tIME" only (not "width", "height", etc.)
        let filtered = provider.entries(Some("t"));
        assert!(filtered.contains_key("tIME"));
        assert!(!filtered.contains_key("width"));
        assert!(!filtered.contains_key("Author"));
    }

    #[test]
    fn test_entries_prefix_matches_dataset_level() {
        let provider = sample_provider();
        // "bit" prefix should match "bit_depth"
        let filtered = provider.entries(Some("bit"));
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("bit_depth"));
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
        assert!(provider.entries(Some("nonexistent")).is_empty());
    }

    // =========================================================================
    // Dataset-level metadata tests (Req 4.10)
    // =========================================================================

    #[test]
    fn test_dataset_level_metadata_present() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        assert!(dict.contains_key("width"));
        assert!(dict.contains_key("height"));
        assert!(dict.contains_key("bit_depth"));
        assert!(dict.contains_key("color_type"));
    }

    // =========================================================================
    // Text chunk tests (Req 4.2, 4.3, 4.4)
    // =========================================================================

    #[test]
    fn test_text_chunks_in_dict() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        assert_eq!(
            dict.get("Author").and_then(|v| v.as_str()),
            Some("Test User")
        );
        assert_eq!(
            dict.get("Description").and_then(|v| v.as_str()),
            Some("A test image")
        );
    }

    // =========================================================================
    // Ancillary chunk tests (Req 4.5, 4.6, 4.7)
    // =========================================================================

    #[test]
    fn test_time_chunk() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        assert_eq!(
            dict.get("tIME").and_then(|v| v.as_str()),
            Some("2025-01-15T12:30:00Z")
        );
    }

    #[test]
    fn test_gama_chunk() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        assert_eq!(dict.get("gAMA").and_then(|v| v.as_f64()), Some(2.2));
    }

    #[test]
    fn test_phys_chunk() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        let phys = dict.get("pHYs").unwrap();
        assert_eq!(phys.get("x").and_then(|v| v.as_u64()), Some(3780));
        assert_eq!(phys.get("y").and_then(|v| v.as_u64()), Some(3780));
        assert_eq!(phys.get("unit").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn test_plte_chunk() {
        let provider = sample_provider();
        let dict = provider.entries(None);
        let plte = dict.get("PLTE").unwrap();
        let arr = plte.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], json!([255, 0, 0]));
        assert_eq!(arr[1], json!([0, 255, 0]));
        assert_eq!(arr[2], json!([0, 0, 255]));
    }

    // =========================================================================
    // raw() test
    // =========================================================================

    #[test]
    fn test_raw_returns_empty() {
        let provider = sample_provider();
        assert!(provider.raw().is_empty());
    }

    // =========================================================================
    // Empty provider test
    // =========================================================================

    #[test]
    fn test_empty_provider() {
        let provider = PNGMetadataProvider::new(HashMap::new());
        assert!(provider.entries(None).is_empty());
        assert!(provider.entries(Some("any")).is_empty());
        assert!(provider.raw().is_empty());
    }

    // =========================================================================
    // Duplicate keyword overwrite test
    // =========================================================================

    #[test]
    fn test_duplicate_keyword_last_wins() {
        // Simulates the behavior where later text chunks overwrite earlier ones
        let mut entries = HashMap::new();
        entries.insert("Author".to_string(), json!("First"));
        // Overwrite with second value (HashMap semantics)
        entries.insert("Author".to_string(), json!("Second"));
        let provider = PNGMetadataProvider::new(entries);
        let dict = provider.entries(None);
        assert_eq!(dict.get("Author").and_then(|v| v.as_str()), Some("Second"));
    }
}
