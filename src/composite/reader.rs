//! Composite dataset reader that merges assets from multiple readers.
//!
//! `CompositeDatasetReader` wraps a base `DatasetReader` and adds overview
//! assets from R-set files. The base reader's assets are exposed unchanged;
//! overview assets are re-keyed as `image:0:overview:N` with role `"overview"`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetReader, ImageAssetProvider, MetadataProvider};
use crate::types::AssetType;

use super::wrapper::OverviewAssetWrapper;

/// A dataset reader that merges a base reader with overview assets from R-set files.
///
/// The base reader provides all original assets. Overview assets are added
/// from separate readers (one per R-set file), re-keyed as `image:0:overview:N`.
pub struct CompositeDatasetReader {
    /// The base reader (first path)
    base: Box<dyn DatasetReader>,
    /// Overview assets keyed by their new key (e.g., "image:0:overview:1")
    overviews: HashMap<String, Arc<OverviewAssetWrapper>>,
    /// Sorted overview keys for deterministic iteration
    overview_keys: Vec<String>,
    /// Cache for AssetProvider enum wrappers
    cache: RwLock<HashMap<String, AssetProvider>>,
}

impl CompositeDatasetReader {
    /// Create a new composite reader from a base reader and overview entries.
    ///
    /// # Arguments
    /// * `base` - The base dataset reader (from the first/primary file)
    /// * `overview_entries` - Vec of (overview_level, image_asset_provider) pairs
    ///   from R-set files. The level comes from the `.rN` filename.
    pub fn new(
        base: Box<dyn DatasetReader>,
        overview_entries: Vec<(u32, Arc<dyn ImageAssetProvider>)>,
    ) -> Self {
        let mut overviews = HashMap::new();
        let mut overview_keys = Vec::new();

        for (level, provider) in overview_entries {
            let key = format!("image:0:overview:{}", level);
            let wrapper = Arc::new(OverviewAssetWrapper::new(key.clone(), provider));
            overviews.insert(key.clone(), wrapper);
            overview_keys.push(key);
        }

        // Sort keys by overview level for deterministic ordering
        overview_keys.sort_by_key(|k| {
            k.rsplit(':')
                .next()
                .and_then(|n| n.parse::<u32>().ok())
                .unwrap_or(0)
        });

        Self {
            base,
            overviews,
            overview_keys,
            cache: RwLock::new(HashMap::new()),
        }
    }
}

impl DatasetReader for CompositeDatasetReader {
    fn get_asset(&self, key: &str) -> Result<AssetProvider, CodecError> {
        // Check overview assets first
        if let Some(wrapper) = self.overviews.get(key) {
            // Check cache
            {
                let cache = self.cache.read().unwrap();
                if let Some(cached) = cache.get(key) {
                    return Ok(cached.clone());
                }
            }
            let asset = AssetProvider::Image(wrapper.clone() as Arc<dyn ImageAssetProvider>);
            let mut cache = self.cache.write().unwrap();
            cache.insert(key.to_string(), asset.clone());
            return Ok(asset);
        }

        // Delegate to base reader
        self.base.get_asset(key)
    }

    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<&[String]>,
    ) -> Vec<String> {
        let mut keys = self.base.get_asset_keys(asset_type, roles);

        // Add overview keys if they match the filter
        let type_matches = asset_type.is_none() || asset_type == Some(AssetType::Image);
        let roles_match = match roles {
            None => true,
            Some(r) => r.iter().any(|role| role == "overview"),
        };

        if type_matches && roles_match {
            keys.extend(self.overview_keys.iter().cloned());
        }

        keys
    }

    fn has_asset(&self, key: &str) -> bool {
        self.overviews.contains_key(key) || self.base.has_asset(key)
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.base.metadata()
    }

    fn close(&mut self) -> Result<(), CodecError> {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        self.base.close()
    }
}

// SAFETY: CompositeDatasetReader is Send + Sync because:
// - base: Box<dyn DatasetReader> is Send + Sync (trait bound)
// - overviews: HashMap is Send + Sync (values are Arc)
// - overview_keys: Vec<String> is Send + Sync
// - cache: RwLock<HashMap> is Send + Sync
unsafe impl Send for CompositeDatasetReader {}
unsafe impl Sync for CompositeDatasetReader {}
