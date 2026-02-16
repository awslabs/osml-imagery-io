//! DataAssetProvider trait for accessing structured data within a dataset.
//!
//! This module defines the interface for data assets with XML and JSON parsing.

use crate::error::CodecError;
use crate::traits::AssetProvider;

/// Trait for structured data access.
///
/// This trait extends `AssetProvider` to provide data-specific access methods
/// including MIME type information and parsing capabilities for XML and JSON.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait DataAssetProvider: AssetProvider {
    /// Returns the MIME type of the data.
    fn mime_type(&self) -> &str;

    /// Parses the content as XML and returns a serialized XML string.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError::Parse` if the content is not valid XML.
    fn parse_as_xml(&self) -> Result<String, CodecError>;

    /// Parses the content as JSON and returns a JSON value.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError::Parse` if the content is not valid JSON.
    fn parse_as_json(&self) -> Result<serde_json::Value, CodecError>;
}
