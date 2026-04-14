//! Asset metadata trait and typed asset provider enum.
//!
//! This module defines:
//! - `AssetMetadata` — the base trait providing common metadata accessors for all asset types.
//! - `AssetProvider` — a typed enum wrapping specialized provider trait objects, replacing
//!   the old `AssetProvider` trait to eliminate `as_any()` downcasting and unsafe pointer casts.

use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::data::DataAssetProvider;
use crate::traits::graphics::GraphicsAssetProvider;
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::traits::text::TextAssetProvider;
use crate::types::AssetType;

/// Base trait providing common metadata for all asset types.
///
/// This trait is the supertrait for all specialized provider traits
/// (`ImageAssetProvider`, `TextAssetProvider`, `DataAssetProvider`,
/// `GraphicsAssetProvider`). It provides access to asset identity,
/// metadata, and raw bytes.
///
/// # Design
///
/// Unlike the previous `AssetProvider` trait, `AssetMetadata` does not include
/// `as_any()` or `asset_type()`. Asset type is determined by the `AssetProvider`
/// enum variant, and downcasting is no longer needed.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait AssetMetadata: Send + Sync {
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

    /// Returns the asset-level metadata provider.
    fn metadata(&self) -> Arc<dyn MetadataProvider>;

    /// Returns the raw asset bytes.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if the asset data cannot be read.
    fn raw_asset(&self) -> Result<Vec<u8>, CodecError>;
}

/// Typed container for asset providers.
///
/// Each variant carries an `Arc` of the corresponding specialized trait object,
/// making the asset type statically known at the call site. This eliminates the
/// need for `as_any()` downcasting or unsafe pointer casts.
///
/// # Examples
///
/// ```ignore
/// match &provider {
///     AssetProvider::Image(img) => { /* use img as &Arc<dyn ImageAssetProvider> */ }
///     AssetProvider::Text(txt) => { /* use txt as &Arc<dyn TextAssetProvider> */ }
///     AssetProvider::Data(data) => { /* use data as &Arc<dyn DataAssetProvider> */ }
///     AssetProvider::Graphics(gfx) => { /* use gfx as &Arc<dyn GraphicsAssetProvider> */ }
/// }
/// ```
pub enum AssetProvider {
    /// Raster image data (satellite imagery, aerial photos, etc.)
    Image(Arc<dyn ImageAssetProvider>),
    /// Text content (embedded text segments, etc.)
    Text(Arc<dyn TextAssetProvider>),
    /// Structured data (XML, JSON metadata, etc.)
    Data(Arc<dyn DataAssetProvider>),
    /// Vector graphics and annotations
    Graphics(Arc<dyn GraphicsAssetProvider>),
}

impl Clone for AssetProvider {
    fn clone(&self) -> Self {
        match self {
            AssetProvider::Image(inner) => AssetProvider::Image(Arc::clone(inner)),
            AssetProvider::Text(inner) => AssetProvider::Text(Arc::clone(inner)),
            AssetProvider::Data(inner) => AssetProvider::Data(Arc::clone(inner)),
            AssetProvider::Graphics(inner) => AssetProvider::Graphics(Arc::clone(inner)),
        }
    }
}

/// Helper macro to delegate a method call to the inner trait object across all variants.
macro_rules! delegate {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        match $self {
            AssetProvider::Image(inner) => inner.$method($($arg),*),
            AssetProvider::Text(inner) => inner.$method($($arg),*),
            AssetProvider::Data(inner) => inner.$method($($arg),*),
            AssetProvider::Graphics(inner) => inner.$method($($arg),*),
        }
    };
}

impl AssetProvider {
    /// Returns the asset type derived from the enum variant.
    pub fn asset_type(&self) -> AssetType {
        match self {
            AssetProvider::Image(_) => AssetType::Image,
            AssetProvider::Text(_) => AssetType::Text,
            AssetProvider::Data(_) => AssetType::Data,
            AssetProvider::Graphics(_) => AssetType::Graphics,
        }
    }

    /// Returns the unique identifier for this asset within the dataset.
    pub fn key(&self) -> &str {
        delegate!(self, key)
    }

    /// Returns a human-readable title for the asset.
    pub fn title(&self) -> &str {
        delegate!(self, title)
    }

    /// Returns a detailed description of the asset.
    pub fn description(&self) -> &str {
        delegate!(self, description)
    }

    /// Returns the MIME type of the asset content.
    pub fn media_type(&self) -> &str {
        delegate!(self, media_type)
    }

    /// Returns the semantic roles for this asset.
    pub fn roles(&self) -> &[String] {
        delegate!(self, roles)
    }

    /// Returns the asset-level metadata provider.
    pub fn metadata(&self) -> Arc<dyn MetadataProvider> {
        delegate!(self, metadata)
    }

    /// Returns the raw asset bytes.
    ///
    /// # Errors
    ///
    /// Returns a `CodecError` if the asset data cannot be read.
    pub fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        delegate!(self, raw_asset)
    }

    /// Returns a reference to the inner `ImageAssetProvider` if this is an `Image` variant.
    pub fn as_image(&self) -> Option<&Arc<dyn ImageAssetProvider>> {
        match self {
            AssetProvider::Image(inner) => Some(inner),
            _ => None,
        }
    }

    /// Returns a reference to the inner `TextAssetProvider` if this is a `Text` variant.
    pub fn as_text(&self) -> Option<&Arc<dyn TextAssetProvider>> {
        match self {
            AssetProvider::Text(inner) => Some(inner),
            _ => None,
        }
    }

    /// Returns a reference to the inner `DataAssetProvider` if this is a `Data` variant.
    pub fn as_data(&self) -> Option<&Arc<dyn DataAssetProvider>> {
        match self {
            AssetProvider::Data(inner) => Some(inner),
            _ => None,
        }
    }

    /// Returns a reference to the inner `GraphicsAssetProvider` if this is a `Graphics` variant.
    pub fn as_graphics(&self) -> Option<&Arc<dyn GraphicsAssetProvider>> {
        match self {
            AssetProvider::Graphics(inner) => Some(inner),
            _ => None,
        }
    }
}
