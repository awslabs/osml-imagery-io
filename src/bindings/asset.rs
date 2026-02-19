//! Python bindings for AssetProvider.
//!
//! This module provides the PyAssetProvider wrapper that exposes the
//! AssetProvider trait to Python, and BytesAssetProvider for creating
//! assets from raw bytes.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::error::CodecError;
use crate::traits::{AssetProvider, MetadataProvider};
use crate::types::AssetType;

/// Python wrapper for AssetProvider trait objects.
///
/// This class provides access to asset properties and content.
#[pyclass(name = "AssetProvider")]
pub struct PyAssetProvider {
    inner: Arc<dyn AssetProvider>,
}

impl PyAssetProvider {
    /// Creates a new PyAssetProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn AssetProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner AssetProvider.
    pub fn inner(&self) -> &Arc<dyn AssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyAssetProvider {
    /// Returns the unique identifier for this asset within the dataset.
    #[getter]
    fn key(&self) -> &str {
        self.inner.key()
    }

    /// Returns a human-readable title for the asset.
    #[getter]
    fn title(&self) -> &str {
        self.inner.title()
    }

    /// Returns a detailed description of the asset.
    #[getter]
    fn description(&self) -> &str {
        self.inner.description()
    }

    /// Returns the MIME type of the asset content.
    #[getter]
    fn media_type(&self) -> &str {
        self.inner.media_type()
    }

    /// Returns the semantic roles for this asset.
    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles().to_vec()
    }

    /// Returns the asset category.
    #[getter]
    fn asset_type(&self) -> AssetType {
        self.inner.asset_type()
    }

    /// Returns the raw asset bytes as a BytesIO object.
    ///
    /// # Errors
    ///
    /// Raises an IOError if the asset data cannot be read.
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new_bound(py, &bytes);

        // Import io.BytesIO and create instance
        let io_module = py.import_bound("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Returns the asset-level metadata provider.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    /// Creates a new AssetProvider from raw bytes.
    ///
    /// This allows Python users to create assets from raw data for use with
    /// DatasetWriter.add_asset().
    ///
    /// # Arguments
    ///
    /// * `key` - Unique identifier for the asset
    /// * `data` - Raw bytes of the asset content
    /// * `asset_type` - The type of asset (Image, Text, Graphics, Data)
    /// * `title` - Human-readable title (optional, defaults to key)
    /// * `description` - Detailed description (optional, defaults to empty)
    /// * `roles` - Semantic roles (optional, defaults to ["data"])
    /// * `media_type` - MIME type (optional, auto-detected from asset_type)
    ///
    /// # Returns
    ///
    /// A new AssetProvider that can be passed to DatasetWriter.add_asset()
    #[staticmethod]
    #[pyo3(signature = (key, data, asset_type, title=None, description=None, roles=None, media_type=None))]
    fn from_bytes(
        key: &str,
        data: &[u8],
        asset_type: AssetType,
        title: Option<&str>,
        description: Option<&str>,
        roles: Option<Vec<String>>,
        media_type: Option<&str>,
    ) -> Self {
        let provider = BytesAssetProvider::new(
            key.to_string(),
            data.to_vec(),
            asset_type,
            title.map(|s| s.to_string()),
            description.map(|s| s.to_string()),
            roles,
            media_type.map(|s| s.to_string()),
        );
        Self {
            inner: Arc::new(provider),
        }
    }
}

/// A simple asset provider that holds raw bytes in memory.
///
/// This is used to create assets from Python for writing to datasets.
struct BytesAssetProvider {
    key: String,
    title: String,
    description: String,
    media_type: String,
    roles: Vec<String>,
    asset_type: AssetType,
    data: Vec<u8>,
}

impl BytesAssetProvider {
    fn new(
        key: String,
        data: Vec<u8>,
        asset_type: AssetType,
        title: Option<String>,
        description: Option<String>,
        roles: Option<Vec<String>>,
        media_type: Option<String>,
    ) -> Self {
        let default_media_type = match asset_type {
            AssetType::Image => "application/vnd.nitf.image",
            AssetType::Text => "text/plain",
            AssetType::Graphics => "image/cgm",
            AssetType::Data => "application/octet-stream",
        };

        Self {
            title: title.unwrap_or_else(|| key.clone()),
            description: description.unwrap_or_default(),
            media_type: media_type.unwrap_or_else(|| default_media_type.to_string()),
            roles: roles.unwrap_or_else(|| vec!["data".to_string()]),
            key,
            asset_type,
            data,
        }
    }
}

/// Empty metadata provider for BytesAssetProvider
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
}

impl Default for EmptyMetadataProvider {
    fn default() -> Self {
        Self {
            empty_bytes: Vec::new(),
        }
    }
}

impl MetadataProvider for EmptyMetadataProvider {
    fn as_dict(&self, _prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    fn raw(&self) -> &[u8] {
        &self.empty_bytes
    }
}

impl AssetProvider for BytesAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn media_type(&self) -> &str {
        &self.media_type
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(self.data.clone())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        Arc::new(EmptyMetadataProvider::default())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Ensure BytesAssetProvider is Send + Sync
unsafe impl Send for BytesAssetProvider {}
unsafe impl Sync for BytesAssetProvider {}
