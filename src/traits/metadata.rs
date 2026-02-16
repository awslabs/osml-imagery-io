//! MetadataProvider trait for accessing raw and structured metadata.
//!
//! This module defines the interface for accessing metadata from datasets and assets.

use std::collections::HashMap;

/// Provides access to raw and structured metadata.
///
/// This trait defines the interface for accessing metadata in both raw byte form
/// and as structured dictionaries. Implementations may support multiple named
/// metadata sections.
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

    /// Returns metadata as a dictionary, optionally filtered by section name.
    ///
    /// # Arguments
    ///
    /// * `name` - Optional section name to filter the returned metadata.
    ///   - If `Some(name)`, returns only the named metadata section.
    ///   - If `None`, returns all metadata sections.
    ///
    /// # Returns
    ///
    /// A `HashMap` where keys are metadata field names and values are JSON-compatible
    /// values that can represent strings, numbers, booleans, arrays, or nested objects.
    fn as_dict(&self, name: Option<&str>) -> HashMap<String, serde_json::Value>;
}
