//! MetadataProvider trait for accessing raw and structured metadata.
//!
//! This module defines the interface for accessing metadata from datasets and assets.

use std::collections::HashMap;

/// Provides access to raw and structured metadata via a dictionary interface.
///
/// Implementations store metadata as key-value pairs where values are JSON-compatible
/// types. The trait supports single-key lookup (`get_value`), existence checks
/// (`contains_key`), key enumeration (`keys`), and bulk export (`entries`).
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait MetadataProvider: Send + Sync {
    /// Returns the raw metadata bytes.
    ///
    /// This provides access to the underlying metadata in its original binary format,
    /// which may be useful for format-specific processing or debugging.
    fn raw(&self) -> &[u8];

    /// Returns the value for a single key, or `None` if absent.
    fn get_value(&self, key: &str) -> Option<serde_json::Value> {
        self.entries(None).remove(key)
    }

    /// Returns `true` if the given key exists in the metadata.
    fn contains_key(&self, key: &str) -> bool {
        self.entries(None).contains_key(key)
    }

    /// Returns the number of metadata entries.
    fn len(&self) -> usize {
        self.entries(None).len()
    }

    /// Returns `true` if there are no metadata entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a list of all metadata keys.
    fn keys(&self) -> Vec<String> {
        self.entries(None).into_keys().collect()
    }

    /// Returns metadata entries, optionally filtered by key prefix.
    ///
    /// - `entries(None)` returns all key-value pairs.
    /// - `entries(Some(prefix))` returns only entries whose key starts with `prefix`.
    fn entries(&self, prefix: Option<&str>) -> HashMap<String, serde_json::Value>;
}
