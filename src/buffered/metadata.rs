//! BufferedMetadataProvider - A mutable metadata provider for encoding hints.
//!
//! This module provides a simple, thread-safe implementation of the MetadataProvider
//! trait that allows programmatic setting of key-value pairs. It's primarily used
//! for passing encoding hints (IMODE, IC, NPPBH, NPPBV, COMRAT) to the dataset writer.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::traits::MetadataProvider;

/// A mutable metadata provider that stores string key-value pairs.
///
/// This provider allows programmatic setting of metadata values, making it useful
/// for creating assets with custom encoding hints. It implements the MetadataProvider
/// trait, allowing it to be used anywhere a MetadataProvider is expected.
///
/// # Thread Safety
///
/// BufferedMetadataProvider is thread-safe (Send + Sync) and uses RwLock for
/// concurrent read access with exclusive write access.
///
/// # Example
///
/// ```ignore
/// use osml_io::BufferedMetadataProvider;
///
/// let provider = BufferedMetadataProvider::new();
/// provider.set("imode", "B");
/// provider.set("nppbh", "256");
///
/// assert_eq!(provider.get("imode"), Some("B".to_string()));
///
/// let dict = provider.as_dict(None);
/// assert!(dict.contains_key("imode"));
/// ```
pub struct BufferedMetadataProvider {
    /// Thread-safe storage for metadata key-value pairs.
    data: RwLock<HashMap<String, serde_json::Value>>,
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
    /// This copies all key-value pairs from the source provider, allowing users
    /// to duplicate metadata and selectively update fields.
    ///
    /// # Arguments
    ///
    /// * `source` - The MetadataProvider to copy key-value pairs from.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Copy metadata from an existing provider and modify
    /// let new_provider = BufferedMetadataProvider::from_provider(&existing_provider);
    /// new_provider.set("imode", "P"); // Override imode
    /// ```
    pub fn from_provider(source: &dyn MetadataProvider) -> Self {
        let source_dict = source.as_dict(None);
        Self {
            data: RwLock::new(source_dict),
        }
    }

    /// Set a string value for the given key.
    ///
    /// If the key already exists, its value is replaced.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name.
    /// * `value` - The value to store.
    pub fn set(&self, key: &str, value: &str) {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), serde_json::Value::String(value.to_string()));
    }

    /// Set a raw `serde_json::Value` for the given key.
    ///
    /// Unlike [`set`], which always stores a JSON string, this method stores
    /// the value as-is, preserving its JSON type (number, array, object, etc.).
    /// This is needed for GeoTIFF encoding hints that require numeric or array values.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name.
    /// * `value` - The JSON value to store.
    pub fn set_json(&self, key: &str, value: serde_json::Value) {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), value);
    }

    /// Get the value for the given key, if it exists.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name to retrieve.
    ///
    /// # Returns
    ///
    /// The value as a String if the key exists and contains a string value,
    /// or None if the key doesn't exist.
    pub fn get(&self, key: &str) -> Option<String> {
        let data = self.data.read().unwrap();
        data.get(key).and_then(|v| {
            match v {
                serde_json::Value::String(s) => Some(s.clone()),
                // For non-string values, convert to string representation
                other => Some(other.to_string()),
            }
        })
    }

    /// Remove a key-value pair.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name to remove.
    ///
    /// # Returns
    ///
    /// The previous value if the key existed, or None if it didn't.
    pub fn remove(&self, key: &str) -> Option<String> {
        let mut data = self.data.write().unwrap();
        data.remove(key).and_then(|v| {
            match v {
                serde_json::Value::String(s) => Some(s),
                other => Some(other.to_string()),
            }
        })
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
    /// Returns empty bytes (BufferedMetadataProvider has no raw representation).
    fn raw(&self) -> &[u8] {
        &[]
    }

    /// Returns metadata as a dictionary, optionally filtered by prefix.
    ///
    /// # Arguments
    ///
    /// * `name` - Optional prefix to filter fields. If `Some(prefix)`, only fields
    ///   whose names start with the prefix are returned. If `None`, all fields
    ///   are returned.
    ///
    /// # Returns
    ///
    /// A HashMap of field names to JSON values.
    fn as_dict(&self, name: Option<&str>) -> HashMap<String, serde_json::Value> {
        let data = self.data.read().unwrap();
        
        match name {
            None => data.clone(),
            Some(prefix) => {
                data.iter()
                    .filter(|(key, _)| key.starts_with(prefix))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_provider() {
        let provider = BufferedMetadataProvider::new();
        let dict = provider.as_dict(None);
        assert!(dict.is_empty());
    }

    #[test]
    fn set_and_get_single_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        assert_eq!(provider.get("imode"), Some("B".to_string()));
    }

    #[test]
    fn set_overwrites_existing_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        provider.set("imode", "P");
        assert_eq!(provider.get("imode"), Some("P".to_string()));
    }

    #[test]
    fn get_nonexistent_key_returns_none() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.get("nonexistent"), None);
    }

    #[test]
    fn remove_returns_previous_value() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        let removed = provider.remove("imode");
        assert_eq!(removed, Some("B".to_string()));
        assert_eq!(provider.get("imode"), None);
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let provider = BufferedMetadataProvider::new();
        assert_eq!(provider.remove("nonexistent"), None);
    }

    #[test]
    fn clear_removes_all_keys() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        provider.set("ic", "NC");
        provider.set("nppbh", "256");
        
        provider.clear();
        
        let dict = provider.as_dict(None);
        assert!(dict.is_empty());
    }

    #[test]
    fn raw_returns_empty_bytes() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn as_dict_none_returns_all_pairs() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        provider.set("ic", "NC");
        provider.set("nppbh", "256");
        
        let dict = provider.as_dict(None);
        
        assert_eq!(dict.len(), 3);
        assert_eq!(dict.get("imode"), Some(&serde_json::json!("B")));
        assert_eq!(dict.get("ic"), Some(&serde_json::json!("NC")));
        assert_eq!(dict.get("nppbh"), Some(&serde_json::json!("256")));
    }

    #[test]
    fn as_dict_with_prefix_filters_correctly() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        provider.set("ic", "NC");
        provider.set("nppbh", "256");
        provider.set("nppbv", "256");
        provider.set("comrat", "01.0");
        
        // Filter by "npp" prefix
        let dict = provider.as_dict(Some("npp"));
        
        assert_eq!(dict.len(), 2);
        assert!(dict.contains_key("nppbh"));
        assert!(dict.contains_key("nppbv"));
        assert!(!dict.contains_key("imode"));
        assert!(!dict.contains_key("ic"));
        assert!(!dict.contains_key("comrat"));
    }

    #[test]
    fn as_dict_with_nonmatching_prefix_returns_empty() {
        let provider = BufferedMetadataProvider::new();
        provider.set("imode", "B");
        provider.set("ic", "NC");
        
        let dict = provider.as_dict(Some("xyz"));
        
        assert!(dict.is_empty());
    }

    #[test]
    fn default_creates_empty_provider() {
        let provider = BufferedMetadataProvider::default();
        let dict = provider.as_dict(None);
        assert!(dict.is_empty());
    }

    #[test]
    fn from_provider_copies_all_pairs() {
        // Create a source provider with some values
        let source = BufferedMetadataProvider::new();
        source.set("imode", "B");
        source.set("ic", "NC");
        source.set("nppbh", "256");

        // Create a new provider from the source
        let copied = BufferedMetadataProvider::from_provider(&source);

        // Verify all pairs were copied
        let dict = copied.as_dict(None);
        assert_eq!(dict.len(), 3);
        assert_eq!(dict.get("imode"), Some(&serde_json::json!("B")));
        assert_eq!(dict.get("ic"), Some(&serde_json::json!("NC")));
        assert_eq!(dict.get("nppbh"), Some(&serde_json::json!("256")));
    }

    #[test]
    fn from_provider_allows_modification_without_affecting_source() {
        let source = BufferedMetadataProvider::new();
        source.set("imode", "B");

        let copied = BufferedMetadataProvider::from_provider(&source);
        copied.set("imode", "P");
        copied.set("new_key", "NEW_VALUE");

        // Source should be unchanged
        assert_eq!(source.get("imode"), Some("B".to_string()));
        assert_eq!(source.get("new_key"), None);

        // Copied should have modifications
        assert_eq!(copied.get("imode"), Some("P".to_string()));
        assert_eq!(copied.get("new_key"), Some("NEW_VALUE".to_string()));
    }
}
