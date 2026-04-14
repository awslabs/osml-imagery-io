//! TextAssetProvider trait for accessing text content within a dataset.
//!
//! This module defines the interface for text assets with encoding information.

use crate::error::CodecError;
use crate::traits::asset::AssetMetadata;

/// Trait for text content access.
///
/// This trait extends `AssetMetadata` to provide text-specific access methods
/// including decoded text content and encoding information.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait TextAssetProvider: AssetMetadata {
    /// Returns the decoded text content as a string.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if the text cannot be decoded.
    fn text(&self) -> Result<String, CodecError>;

    /// Returns the character encoding (e.g., "UTF-8", "ASCII").
    fn encoding(&self) -> &str;

    /// Returns the text format identifier.
    fn format(&self) -> &str;
}
