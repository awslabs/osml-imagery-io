//! DatasetReader trait for reading datasets.
//!
//! This module defines the interface for reading geospatial datasets
//! through a unified interface.

use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::asset::AssetProvider;
use crate::traits::MetadataProvider;
use crate::types::AssetType;

/// Trait for reading datasets.
///
/// This trait defines the interface for reading geospatial datasets through
/// a unified interface, allowing access to imagery and metadata without
/// knowing format-specific details.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
///
/// # Example
///
/// ```ignore
/// let reader = open_dataset("path/to/file.ntf")?;
/// let keys = reader.get_asset_keys(Some(AssetType::Image), None);
/// for key in keys {
///     let asset = reader.get_asset(&key)?;
///     // Process asset...
/// }
/// reader.close()?;
/// ```
pub trait DatasetReader: Send + Sync {
    /// Returns an AssetProvider for the specified asset key.
    ///
    /// The returned `AssetProvider` enum variant indicates the asset type:
    /// `AssetProvider::Image`, `AssetProvider::Text`, `AssetProvider::Data`,
    /// or `AssetProvider::Graphics`. Use pattern matching or the typed
    /// accessors (`as_image()`, `as_text()`, etc.) to access the specialized
    /// provider trait object.
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    ///
    /// # Errors
    ///
    /// Returns `CodecError::AssetNotFound` if no asset with the given key exists.
    fn get_asset(&self, key: &str) -> Result<AssetProvider, CodecError>;

    /// Returns a list of asset keys matching the filter criteria.
    ///
    /// # Arguments
    ///
    /// * `asset_type` - Optional filter to return only assets of the specified type.
    /// * `roles` - Optional filter to return only assets with any of the specified roles.
    ///
    /// # Returns
    ///
    /// A vector of asset keys matching the filter criteria. If no filters are
    /// provided, returns all asset keys.
    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<&[String]>,
    ) -> Vec<String>;

    /// Returns true if an asset with the given key exists.
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    fn has_asset(&self, key: &str) -> bool;

    /// Returns the dataset-level metadata provider.
    fn metadata(&self) -> Arc<dyn MetadataProvider>;

    /// Releases all resources associated with this reader.
    ///
    /// After calling this method, the reader should not be used.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if resources cannot be released cleanly.
    fn close(&mut self) -> Result<(), CodecError>;
}
