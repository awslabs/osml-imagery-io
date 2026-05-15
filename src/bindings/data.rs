//! Python bindings for DataAssetProvider.
//!
//! This module provides the PyDataAssetProvider wrapper that exposes the
//! DataAssetProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::traits::DataAssetProvider;
use crate::types::AssetType;

/// Provides access to structured data stored within a geospatial dataset.
///
/// Geospatial datasets can embed structured payloads alongside imagery —
/// XML metadata (such as SICD/SIDD), JSON configuration, overflow TREs,
/// and application-specific data. ``DataAssetProvider`` exposes the raw
/// bytes through :attr:`raw_asset` and the :attr:`mime_type` property
/// indicates the content format. Use :meth:`DatasetReader.get_asset` to
/// obtain an instance for a specific data asset in the dataset.
///
/// Example:
///
/// ```python
/// import json
/// import xml.etree.ElementTree as ET
/// from aws.osml.io import IO
///
/// with IO.open(["sicd_image.ntf"], "r") as dataset:
///     for key in dataset.get_asset_keys(asset_type="data"):
///         data = dataset.get_asset(key)
///         print(f"Data '{key}': mime_type={data.mime_type}")
///
///         raw = data.raw_asset.read()
///         if data.mime_type == "application/xml":
///             root = ET.fromstring(raw)
///             print(f"XML root tag: {root.tag}")
///         elif data.mime_type == "application/json":
///             obj = json.loads(raw)
///             print(f"JSON keys: {list(obj.keys())}")
/// ```
#[pyclass(name = "DataAssetProvider")]
pub struct PyDataAssetProvider {
    inner: Arc<dyn DataAssetProvider>,
}

impl PyDataAssetProvider {
    /// Creates a new PyDataAssetProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn DataAssetProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner DataAssetProvider.
    pub fn inner(&self) -> &Arc<dyn DataAssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyDataAssetProvider {
    // AssetProvider methods

    /// The unique identifier for this asset within the dataset.
    #[getter]
    fn key(&self) -> &str {
        self.inner.key()
    }

    /// A human-readable title for the asset.
    #[getter]
    fn title(&self) -> &str {
        self.inner.title()
    }

    /// A detailed description of the asset.
    #[getter]
    fn description(&self) -> &str {
        self.inner.description()
    }

    /// The MIME type of the asset content.
    #[getter]
    fn media_type(&self) -> &str {
        self.inner.media_type()
    }

    /// The semantic roles for this asset.
    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles().to_vec()
    }

    /// The asset category.
    #[getter]
    fn asset_type(&self) -> AssetType {
        AssetType::Data
    }

    /// The raw asset bytes as a ``BytesIO`` object.
    #[getter]
    fn raw_asset<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new(py, &bytes);

        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// The asset-level :class:`MetadataProvider`.
    #[getter]
    fn metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    // DataAssetProvider-specific methods

    /// The MIME type of the data content (e.g., ``"application/xml"``, ``"application/json"``).
    #[getter]
    fn mime_type(&self) -> &str {
        self.inner.mime_type()
    }
}
