//! DataAssetProvider trait for accessing structured data within a dataset.
//!
//! This module defines the interface for data assets with MIME type information.

use crate::traits::asset::AssetMetadata;

/// Trait for structured data access.
///
/// This trait extends `AssetMetadata` to provide data-specific access methods
/// including MIME type information. Use `raw_asset()` from `AssetMetadata` to
/// obtain the raw bytes for decoding with your preferred XML/JSON library.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait DataAssetProvider: AssetMetadata {
    /// Returns the MIME type of the data.
    fn mime_type(&self) -> &str;
}
