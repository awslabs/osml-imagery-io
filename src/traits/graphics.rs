//! GraphicsAssetProvider trait for accessing vector graphics within a dataset.
//!
//! This module defines the interface for graphics assets such as vector overlays
//! and annotations.

use crate::traits::AssetProvider;

/// Trait for vector graphics access.
///
/// This trait extends `AssetProvider` to provide graphics-specific access.
/// Graphics data (vector graphics, annotations, overlays) is accessed through
/// the base `raw_asset()` method inherited from `AssetProvider`.
///
/// # Graphics Data Access
///
/// The raw graphics data can be retrieved using the inherited `raw_asset()` method
/// from `AssetProvider`. The format of the graphics data depends on the specific
/// implementation and can be determined from the `media_type()` method.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait GraphicsAssetProvider: AssetProvider {
    // Graphics-specific methods to be defined by format implementations.
    // Base access is through AssetProvider::raw_asset().
}
