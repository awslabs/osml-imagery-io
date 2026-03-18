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

/// Base class for all asset types within a dataset.
///
/// An ``AssetProvider`` represents a single named asset inside a geospatial
/// dataset. Every dataset opened through :class:`IO` contains one or more
/// assets, each identified by a unique key and categorised as image, text,
/// data, or graphics. This class exposes the common properties shared by all
/// asset types — key, title, description, MIME type, roles, and raw bytes —
/// while specialised subclasses such as :class:`ImageAssetProvider`,
/// :class:`TextAssetProvider`, :class:`DataAssetProvider`, and
/// :class:`GraphicsAssetProvider` add format-specific access methods.
///
/// You typically obtain an ``AssetProvider`` by calling
/// :meth:`DatasetReader.get_asset`. To create an asset from raw bytes for
/// writing, use the :meth:`AssetProvider.from_bytes` static method.
///
/// Example::
///
///     from aws.osml.io import IO
///
///     with IO.open(["image.ntf"], "r") as dataset:
///         keys = dataset.get_asset_keys(asset_type="image")
///         asset = dataset.get_asset(keys[0])
///         print(asset.key, asset.title, asset.asset_type)
#[pyclass(name = "AssetProvider")]
pub struct PyAssetProvider {
    inner: Arc<dyn AssetProvider>,
}

impl PyAssetProvider {
    pub fn new(inner: Arc<dyn AssetProvider>) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &Arc<dyn AssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyAssetProvider {
    /// Unique identifier for this asset within the dataset.
    #[getter]
    fn key(&self) -> &str {
        self.inner.key()
    }

    /// Human-readable title for the asset.
    #[getter]
    fn title(&self) -> &str {
        self.inner.title()
    }

    /// Detailed description of the asset.
    #[getter]
    fn description(&self) -> &str {
        self.inner.description()
    }

    /// MIME type of the asset content (e.g. ``"application/vnd.nitf.image"``).
    #[getter]
    fn media_type(&self) -> &str {
        self.inner.media_type()
    }

    /// Semantic roles assigned to this asset (e.g. ``["data"]``, ``["thumbnail"]``).
    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles().to_vec()
    }

    /// Category of this asset: image, text, data, or graphics.
    #[getter]
    fn asset_type(&self) -> AssetType {
        self.inner.asset_type()
    }

    /// Return the raw asset bytes wrapped in a ``BytesIO`` object.
    ///
    /// :returns: The raw bytes of the asset content.
    /// :rtype: io.BytesIO
    /// :raises IOError: If the asset data cannot be read.
    ///
    /// Example::
    ///
    ///     raw = asset.get_raw_asset()
    ///     data = raw.read()
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new_bound(py, &bytes);

        // Import io.BytesIO and create instance
        let io_module = py.import_bound("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Return the :class:`MetadataProvider` for this asset's metadata.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    /// Create a new ``AssetProvider`` from raw bytes.
    ///
    /// Use this factory when you need to build an asset in memory for writing
    /// to a dataset via :meth:`DatasetWriter.add_asset`.
    ///
    /// :param key: Unique identifier for the asset.
    /// :type key: str
    /// :param data: Raw bytes of the asset content.
    /// :type data: bytes
    /// :param asset_type: The type of asset (Image, Text, Graphics, Data).
    /// :type asset_type: AssetType
    /// :param title: Human-readable title. Defaults to *key* when omitted.
    /// :type title: str, optional
    /// :param description: Detailed description. Defaults to empty.
    /// :type description: str, optional
    /// :param roles: Semantic roles. Defaults to ``["data"]``.
    /// :type roles: list[str], optional
    /// :param media_type: MIME type. Auto-detected from *asset_type* when omitted.
    /// :type media_type: str, optional
    /// :returns: A new asset that can be passed to :meth:`DatasetWriter.add_asset`.
    /// :rtype: AssetProvider
    ///
    /// Example::
    ///
    ///     from aws.osml.io import AssetProvider, AssetType
    ///
    ///     asset = AssetProvider.from_bytes(
    ///         key="my_text",
    ///         data=b"Hello, world!",
    ///         asset_type=AssetType.Text,
    ///         title="Greeting",
    ///     )
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
#[derive(Default)]
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
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
