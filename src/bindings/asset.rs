//! Python bindings for AssetProvider.
//!
//! This module provides the PyAssetProvider wrapper that exposes the
//! AssetProvider enum to Python, and specialized Bytes*AssetProvider adapters
//! for creating assets from raw bytes.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::error::CodecError;
use crate::traits::{
    AssetMetadata, AssetProvider, DataAssetProvider, GraphicsAssetProvider, MetadataProvider,
    TextAssetProvider,
};
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
/// Example:
///
/// ```python
/// from aws.osml.io import IO
///
/// with IO.open(["image.ntf"], "r") as dataset:
///     keys = dataset.get_asset_keys(asset_type="image")
///     asset = dataset.get_asset(keys[0])
///     print(asset.key, asset.title, asset.asset_type)
/// ```
#[pyclass(name = "AssetProvider")]
pub struct PyAssetProvider {
    inner: AssetProvider,
}

impl PyAssetProvider {
    pub fn new(inner: AssetProvider) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &AssetProvider {
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

    /// The raw asset bytes as a ``BytesIO`` object.
    ///
    /// :returns: The raw bytes of the asset content.
    /// :rtype: io.BytesIO
    /// :raises IOError: If the asset data cannot be read.
    ///
    /// Example:
    ///
    /// ```python
    /// data = asset.raw_asset.read()
    /// ```
    #[getter]
    fn raw_asset<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new(py, &bytes);

        // Import io.BytesIO and create instance
        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// The :class:`MetadataProvider` for this asset's metadata.
    #[getter]
    fn metadata(&self) -> PyMetadataProvider {
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
    /// :raises ValueError: If *asset_type* is ``AssetType.Image`` — use
    ///     :class:`BufferedImageAssetProvider` instead.
    ///
    /// Example:
    ///
    /// ```python
    /// from aws.osml.io import AssetProvider, AssetType
    ///
    /// asset = AssetProvider.from_bytes(
    ///     key="my_text",
    ///     data=b"Hello, world!",
    ///     asset_type=AssetType.Text,
    ///     title="Greeting",
    /// )
    /// ```
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
    ) -> PyResult<Self> {
        let title_str = title.unwrap_or(key).to_string();
        let desc_str = description.unwrap_or_default().to_string();
        let roles_vec = roles.unwrap_or_else(|| vec!["data".to_string()]);

        let enum_provider = match asset_type {
            AssetType::Image => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Cannot create an image asset from raw bytes. \
                     Use BufferedImageAssetProvider instead.",
                ));
            }
            AssetType::Text => {
                let default_media = "text/plain";
                let media = media_type.unwrap_or(default_media).to_string();
                let provider = BytesTextAssetProvider {
                    key: key.to_string(),
                    title: title_str,
                    description: desc_str,
                    media_type: media,
                    roles: roles_vec,
                    data: data.to_vec(),
                };
                AssetProvider::Text(Arc::new(provider))
            }
            AssetType::Data => {
                let default_media = "application/octet-stream";
                let media = media_type.unwrap_or(default_media).to_string();
                let provider = BytesDataAssetProvider {
                    key: key.to_string(),
                    title: title_str,
                    description: desc_str,
                    media_type: media,
                    roles: roles_vec,
                    data: data.to_vec(),
                };
                AssetProvider::Data(Arc::new(provider))
            }
            AssetType::Graphics => {
                let default_media = "image/cgm";
                let media = media_type.unwrap_or(default_media).to_string();
                let provider = BytesGraphicsAssetProvider {
                    key: key.to_string(),
                    title: title_str,
                    description: desc_str,
                    media_type: media,
                    roles: roles_vec,
                    data: data.to_vec(),
                };
                AssetProvider::Graphics(Arc::new(provider))
            }
        };

        Ok(Self {
            inner: enum_provider,
        })
    }
}

// =============================================================================
// Empty metadata provider shared by all Bytes*AssetProvider adapters
// =============================================================================

/// Empty metadata provider for Bytes*AssetProvider adapters.
#[derive(Default)]
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
}

impl MetadataProvider for EmptyMetadataProvider {
    fn entries(&self, _prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    fn raw(&self) -> &[u8] {
        &self.empty_bytes
    }
}

// =============================================================================
// BytesTextAssetProvider — implements AssetMetadata + TextAssetProvider
// =============================================================================

/// A text asset provider backed by raw bytes in memory.
struct BytesTextAssetProvider {
    key: String,
    title: String,
    description: String,
    media_type: String,
    roles: Vec<String>,
    data: Vec<u8>,
}

impl AssetMetadata for BytesTextAssetProvider {
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

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        Arc::new(EmptyMetadataProvider::default())
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(self.data.clone())
    }
}

impl TextAssetProvider for BytesTextAssetProvider {
    fn text(&self) -> Result<String, CodecError> {
        String::from_utf8(self.data.clone())
            .map_err(|e| CodecError::Decode(format!("Invalid UTF-8 in text asset: {}", e)))
    }

    fn encoding(&self) -> &str {
        "UTF-8"
    }

    fn format(&self) -> &str {
        "text/plain"
    }
}

// =============================================================================
// BytesDataAssetProvider — implements AssetMetadata + DataAssetProvider
// =============================================================================

/// A data asset provider backed by raw bytes in memory.
struct BytesDataAssetProvider {
    key: String,
    title: String,
    description: String,
    media_type: String,
    roles: Vec<String>,
    data: Vec<u8>,
}

impl AssetMetadata for BytesDataAssetProvider {
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

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        Arc::new(EmptyMetadataProvider::default())
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(self.data.clone())
    }
}

impl DataAssetProvider for BytesDataAssetProvider {
    fn mime_type(&self) -> &str {
        &self.media_type
    }
}

// =============================================================================
// BytesGraphicsAssetProvider — implements AssetMetadata + GraphicsAssetProvider
// =============================================================================

/// A graphics asset provider backed by raw bytes in memory.
struct BytesGraphicsAssetProvider {
    key: String,
    title: String,
    description: String,
    media_type: String,
    roles: Vec<String>,
    data: Vec<u8>,
}

impl AssetMetadata for BytesGraphicsAssetProvider {
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

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        Arc::new(EmptyMetadataProvider::default())
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(self.data.clone())
    }
}

impl GraphicsAssetProvider for BytesGraphicsAssetProvider {}
