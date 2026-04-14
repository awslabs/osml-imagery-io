//! DatasetWriter trait for writing datasets.
//!
//! This module defines the interface for writing geospatial datasets
//! through a unified interface.

use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::asset::AssetProvider;
use crate::traits::MetadataProvider;

/// Trait for writing datasets.
///
/// This trait defines the interface for writing geospatial datasets through
/// a unified interface, allowing creation of imagery files without knowing
/// format-specific encoding details.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
///
/// # Example
///
/// ```ignore
/// let mut writer = open_dataset_for_write("path/to/output.ntf")?;
/// writer.add_asset("image_0", image_provider, "Main Image", "Primary imagery", &["data".to_string()])?;
/// writer.set_metadata(metadata_provider)?;
/// writer.close()?;
/// ```
pub trait DatasetWriter: Send + Sync {
    /// Adds an asset to the dataset.
    ///
    /// The `provider` is an `AssetProvider` enum whose variant indicates the
    /// asset type (`Image`, `Text`, `Data`, or `Graphics`). Writers can match
    /// on the variant to extract the specialized provider trait object directly.
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    /// * `provider` - The `AssetProvider` enum containing the asset data.
    /// * `title` - A human-readable title for the asset.
    /// * `description` - A detailed description of the asset.
    /// * `roles` - Semantic roles for the asset (e.g., "data", "thumbnail", "metadata").
    ///
    /// # Errors
    ///
    /// Returns `CodecError::DuplicateKey` if an asset with the given key already exists.
    fn add_asset(
        &mut self,
        key: &str,
        provider: AssetProvider,
        title: &str,
        description: &str,
        roles: &[String],
    ) -> Result<(), CodecError>;

    /// Sets the dataset-level metadata.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The MetadataProvider containing the metadata to set.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if the metadata cannot be set.
    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError>;

    /// Finalizes the dataset and releases all resources.
    ///
    /// This method flushes all pending data to storage and closes the dataset.
    /// After calling this method, the writer should not be used.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if the dataset cannot be finalized or resources
    /// cannot be released cleanly.
    fn close(&mut self) -> Result<(), CodecError>;
}
