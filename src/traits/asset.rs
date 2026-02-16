//! AssetProvider trait for accessing assets within a dataset.
//!
//! This module defines the base interface for all asset types.

use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::MetadataProvider;
use crate::types::AssetType;

/// Base trait for all asset types.
///
/// This trait defines the common interface for accessing assets within a dataset.
/// Assets are identified by unique string keys and can be categorized by type.
/// All assets provide access to raw bytes and metadata.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait AssetProvider: Send + Sync {
    /// Returns the unique identifier for this asset within the dataset.
    fn key(&self) -> &str;

    /// Returns a human-readable title for the asset.
    fn title(&self) -> &str;

    /// Returns a detailed description of the asset.
    fn description(&self) -> &str;

    /// Returns the MIME type of the asset content.
    fn media_type(&self) -> &str;

    /// Returns the semantic roles for this asset.
    ///
    /// Roles describe the purpose of the asset (e.g., "data", "thumbnail", "metadata").
    fn roles(&self) -> &[String];

    /// Returns the asset category.
    fn asset_type(&self) -> AssetType;

    /// Returns the raw asset bytes.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if the asset data cannot be read.
    fn raw_asset(&self) -> Result<Vec<u8>, CodecError>;

    /// Returns the asset-level metadata provider.
    fn metadata(&self) -> Arc<dyn MetadataProvider>;
}
