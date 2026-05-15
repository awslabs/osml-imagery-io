//! BufferedMetadataProvider - A mutable metadata provider for encoding hints.
//!
//! This module provides a simple, thread-safe implementation of the MetadataProvider
//! trait that allows programmatic setting of key-value pairs. It's primarily used
//! for passing encoding hints (IMODE, IC, NPPBH, NPPBV, COMRAT) to the dataset writer.

use std::collections::HashMap;
use std::sync::RwLock;

use serde_json::Value;

use crate::traits::MetadataProvider;

/// A mutable metadata provider that stores JSON key-value pairs.
///
/// This provider allows programmatic setting of metadata values, making it useful
/// for creating assets with custom encoding hints. It implements the MetadataProvider
/// trait, allowing it to be used anywhere a MetadataProvider is expected.
///
/// # Thread Safety
///
/// BufferedMetadataProvider is thread-safe (Send + Sync) and uses RwLock for
/// concurrent read access with exclusive write access.
pub struct BufferedMetadataProvider {
    data: RwLock<HashMap<String, Value>>,
}

impl BufferedMetadataProvider {
    /// Create a new empty BufferedMetadataProvider.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    /// Create a BufferedMetadataProvider initialized from an existing MetadataProvider.
    ///
    /// Copies all key-value pairs from the source provider in a single call.
    pub fn from_provider(source: &dyn MetadataProvider) -> Self {
        Self {
            data: RwLock::new(source.entries(None)),
        }
    }

    /// Set a value for the given key. Accepts any `serde_json::Value`.
    ///
    /// If the key already exists, its value is replaced.
    pub fn set(&self, key: &str, value: Value) {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), value);
    }

    /// Remove a key, returning the previous value if it existed.
    pub fn remove(&self, key: &str) -> Option<Value> {
        let mut data = self.data.write().unwrap();
        data.remove(key)
    }

    /// Bulk insert from a map, overwriting any existing keys.
    pub fn update(&self, entries: HashMap<String, Value>) {
        let mut data = self.data.write().unwrap();
        data.extend(entries);
    }

    /// Clear all stored metadata.
    pub fn clear(&self) {
        let mut data = self.data.write().unwrap();
        data.clear();
    }
}

impl Default for BufferedMetadataProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataProvider for BufferedMetadataProvider {
    fn raw(&self) -> &[u8] {
        &[]
    }

    fn get_value(&self, key: &str) -> Option<Value> {
        let data = self.data.read().unwrap();
        data.get(key).cloned()
    }

    fn contains_key(&self, key: &str) -> bool {
        let data = self.data.read().unwrap();
        data.contains_key(key)
    }

    fn len(&self) -> usize {
        let data = self.data.read().unwrap();
        data.len()
    }

    fn keys(&self) -> Vec<String> {
        let data = self.data.read().unwrap();
        data.keys().cloned().collect()
    }

    fn entries(&self, prefix: Option<&str>) -> HashMap<String, Value> {
        let data = self.data.read().unwrap();

        match prefix {
            None => data.clone(),
            Some(prefix) => data
                .iter()
                .filter(|(key, _)| key.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_creates_empty_provider() {
        let provider = BufferedMetadataProvider::new();
        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
    }

    #[test]
    fn set_and_get_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        assert_eq!(provider.get_value("imode"), Some(json!("B")));
    }

    #[test]
    fn set_overwrites_existing_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("imode", json!("P"));
        assert_eq!(provider.get_value("imode"), Some(json!("P")));
    }

    #[test]
    fn get_value_nonexistent_key_returns_none() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.get_value("nonexistent"), None);
    }

    #[test]
    fn contains_key_reports_correctly() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        assert!(provider.contains_key("imode"));
        assert!(!provider.contains_key("nonexistent"));
    }

    #[test]
    fn len_tracks_entries() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.len(), 0);
        provider.set("a", json!("1"));
        assert_eq!(provider.len(), 1);
        provider.set("b", json!("2"));
        assert_eq!(provider.len(), 2);
        provider.set("a", json!("3")); // overwrite, no length change
        assert_eq!(provider.len(), 2);
    }

    #[test]
    fn is_empty_reflects_state() {
        let provider = BufferedMetadataProvider::new();
        assert!(provider.is_empty());
        provider.set("k", json!("v"));
        assert!(!provider.is_empty());
    }

    #[test]
    fn keys_returns_all_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        let mut keys = provider.keys();
        keys.sort();
        assert_eq!(keys, vec!["ic", "imode"]);
    }

    #[test]
    fn remove_returns_previous_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        let removed = provider.remove("imode");
        assert_eq!(removed, Some(json!("B")));
        assert_eq!(provider.get_value("imode"), None);
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.remove("nonexistent"), None);
    }

    #[test]
    fn update_merges_entries() {
        let provider = BufferedMetadataProvider::new();
        provider.set("existing", json!("keep"));

        let mut new_entries = HashMap::new();
        new_entries.insert("a".to_string(), json!("1"));
        new_entries.insert("b".to_string(), json!("2"));
        provider.update(new_entries);

        assert_eq!(provider.len(), 3);
        assert_eq!(provider.get_value("existing"), Some(json!("keep")));
        assert_eq!(provider.get_value("a"), Some(json!("1")));
        assert_eq!(provider.get_value("b"), Some(json!("2")));
    }

    #[test]
    fn update_overwrites_existing_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));

        let mut new_entries = HashMap::new();
        new_entries.insert("imode".to_string(), json!("P"));
        provider.update(new_entries);

        assert_eq!(provider.get_value("imode"), Some(json!("P")));
    }

    #[test]
    fn clear_removes_all_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        provider.set("nppbh", json!("256"));

        provider.clear();

        assert!(provider.is_empty());
        assert_eq!(provider.len(), 0);
    }

    #[test]
    fn raw_returns_empty_bytes() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn entries_none_returns_all_pairs() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        provider.set("nppbh", json!("256"));

        let dict = provider.entries(None);

        assert_eq!(dict.len(), 3);
        assert_eq!(dict.get("imode"), Some(&json!("B")));
        assert_eq!(dict.get("ic"), Some(&json!("NC")));
        assert_eq!(dict.get("nppbh"), Some(&json!("256")));
    }

    #[test]
    fn entries_with_prefix_filters_correctly() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));
        provider.set("nppbh", json!("256"));
        provider.set("nppbv", json!("256"));
        provider.set("comrat", json!("01.0"));

        let dict = provider.entries(Some("npp"));

        assert_eq!(dict.len(), 2);
        assert!(dict.contains_key("nppbh"));
        assert!(dict.contains_key("nppbv"));
        assert!(!dict.contains_key("imode"));
    }

    #[test]
    fn entries_with_nonmatching_prefix_returns_empty() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", json!("B"));
        provider.set("ic", json!("NC"));

        let dict = provider.entries(Some("xyz"));
        assert!(dict.is_empty());
    }

    #[test]
    fn default_creates_empty_provider() {
        let provider = BufferedMetadataProvider::default();
        assert!(provider.is_empty());
    }

    #[test]
    fn from_provider_copies_all_pairs() {
        let source = BufferedMetadataProvider::new();
        source.set("imode", json!("B"));
        source.set("ic", json!("NC"));
        source.set("nppbh", json!("256"));

        let copied = BufferedMetadataProvider::from_provider(&source);

        assert_eq!(copied.len(), 3);
        assert_eq!(copied.get_value("imode"), Some(json!("B")));
        assert_eq!(copied.get_value("ic"), Some(json!("NC")));
        assert_eq!(copied.get_value("nppbh"), Some(json!("256")));
    }

    #[test]
    fn from_provider_allows_modification_without_affecting_source() {
        let source = BufferedMetadataProvider::new();
        source.set("imode", json!("B"));

        let copied = BufferedMetadataProvider::from_provider(&source);
        copied.set("imode", json!("P"));
        copied.set("new_key", json!("NEW_VALUE"));

        assert_eq!(source.get_value("imode"), Some(json!("B")));
        assert_eq!(source.get_value("new_key"), None);

        assert_eq!(copied.get_value("imode"), Some(json!("P")));
        assert_eq!(copied.get_value("new_key"), Some(json!("NEW_VALUE")));
    }

    #[test]
    fn set_accepts_all_json_types() {
        let provider = BufferedMetadataProvider::new();
        provider.set("str", json!("hello"));
        provider.set("int", json!(42));
        provider.set("float", json!(3.14));
        provider.set("bool", json!(true));
        provider.set("null", Value::Null);
        provider.set("array", json!([1, 2, 3]));
        provider.set("object", json!({"a": "b"}));

        assert_eq!(provider.get_value("str"), Some(json!("hello")));
        assert_eq!(provider.get_value("int"), Some(json!(42)));
        assert_eq!(provider.get_value("float"), Some(json!(3.14)));
        assert_eq!(provider.get_value("bool"), Some(json!(true)));
        assert_eq!(provider.get_value("null"), Some(Value::Null));
        assert_eq!(provider.get_value("array"), Some(json!([1, 2, 3])));
        assert_eq!(provider.get_value("object"), Some(json!({"a": "b"})));
    }
}
