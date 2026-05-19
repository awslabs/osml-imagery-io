//! Overview asset wrapper that re-keys an image asset as an overview.
//!
//! `OverviewAssetWrapper` wraps any `ImageAssetProvider` and overrides its
//! key and roles to present it as an overview asset (e.g., `image:0:overview:1`
//! with role `"overview"`). All other methods delegate to the inner provider.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::{AssetMetadata, ImageAssetProvider, MetadataProvider};
use crate::types::PixelType;

/// Wraps an `ImageAssetProvider` with a new key and overview role.
///
/// This wrapper is used when merging R-set files into a composite reader.
/// The inner provider retains its original `tile_byte_ranges()` pointing
/// to its own source file. Only the key and roles are changed.
pub struct OverviewAssetWrapper {
    /// The new asset key (e.g., `"image:0:overview:1"`)
    key: String,
    /// The overview roles
    roles: Vec<String>,
    /// The wrapped provider
    inner: Arc<dyn ImageAssetProvider>,
}

impl OverviewAssetWrapper {
    /// Create a new overview wrapper.
    ///
    /// # Arguments
    /// * `key` - The new asset key (e.g., `"image:0:overview:1"`)
    /// * `inner` - The wrapped image asset provider
    pub fn new(key: String, inner: Arc<dyn ImageAssetProvider>) -> Self {
        Self {
            key,
            roles: vec!["overview".to_string()],
            inner,
        }
    }
}

impl AssetMetadata for OverviewAssetWrapper {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        self.inner.title()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn media_type(&self) -> &str {
        self.inner.media_type()
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        self.inner.raw_asset()
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.inner.metadata()
    }
}

impl ImageAssetProvider for OverviewAssetWrapper {
    fn has_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
    ) -> Result<bool, CodecError> {
        self.inner.has_block(block_row, block_col, resolution_level)
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        self.inner
            .get_block(block_row, block_col, resolution_level, bands)
    }

    fn num_resolution_levels(&self) -> u32 {
        self.inner.num_resolution_levels()
    }

    fn num_bands(&self) -> u32 {
        self.inner.num_bands()
    }

    fn num_rows(&self) -> u32 {
        self.inner.num_rows()
    }

    fn num_columns(&self) -> u32 {
        self.inner.num_columns()
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.inner.num_pixels_per_block_horizontal()
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.inner.num_pixels_per_block_vertical()
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.inner.num_bits_per_pixel()
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        self.inner.actual_bits_per_pixel()
    }

    fn pixel_value_type(&self) -> PixelType {
        self.inner.pixel_value_type()
    }

    fn pad_pixel_value(&self) -> f64 {
        self.inner.pad_pixel_value()
    }

    fn tile_byte_ranges(&self) -> Option<HashMap<(u32, u32), Vec<(u64, u64)>>> {
        self.inner.tile_byte_ranges()
    }

    fn codec_configuration(&self) -> Option<HashMap<String, Vec<u8>>> {
        self.inner.codec_configuration()
    }
}
