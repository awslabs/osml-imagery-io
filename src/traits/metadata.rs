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

    /// Returns every value stored under `key`, in file order.
    ///
    /// Most metadata keys hold exactly one value, but some formats let a key repeat
    /// within one container, and then the dictionary surface can only show one of
    /// them. This is the complete view. The return shape is uniform regardless of
    /// count: `[]` when the key is absent, a single element when it appears once,
    /// one element per instance otherwise.
    ///
    /// The invariant `get_value(key) == get_all(key).first()` holds on every
    /// implementation: the dictionary surface projects the *first* value.
    ///
    /// The default implementation reports the single value under `key`, which is
    /// correct for any format that cannot express a repeated key.
    ///
    /// # Format notes
    ///
    /// NITF/NSIF is the format where this matters: a subheader may carry the same
    /// tagged record extension (TRE) several times, each instance a separate record.
    /// STDI-0002 Volume 1 §2 describes a *sequence* of extensions and imposes no
    /// uniqueness requirement on CETAG. TIFF IFD tags and Zarr attributes are unique
    /// by construction, so `get_all` there is just the zero-or-one view of
    /// `get_value`.
    fn get_all(&self, key: &str) -> Vec<serde_json::Value> {
        self.get_value(key).into_iter().collect()
    }
}
