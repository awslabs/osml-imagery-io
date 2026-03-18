//! Python bindings for GraphicsAssetProvider.
//!
//! This module provides the PyGraphicsAssetProvider wrapper that exposes the
//! GraphicsAssetProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::traits::GraphicsAssetProvider;
use crate::types::AssetType;

/// Python wrapper for GraphicsAssetProvider trait objects.
///
/// This class provides access to graphics asset properties and content.
/// Graphics data is accessed through the inherited `get_raw_asset()` method.
#[pyclass(name = "GraphicsAssetProvider")]
pub struct PyGraphicsAssetProvider {
    inner: Arc<dyn GraphicsAssetProvider>,
}

impl PyGraphicsAssetProvider {
    /// Creates a new PyGraphicsAssetProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn GraphicsAssetProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner GraphicsAssetProvider.
    pub fn inner(&self) -> &Arc<dyn GraphicsAssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyGraphicsAssetProvider {
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
        self.inner.asset_type()
    }

    /// The raw graphics bytes as a ``BytesIO`` object.
    ///
    /// Returns the complete vector graphics payload (typically CGM format)
    /// wrapped in a ``BytesIO`` stream. Read the returned object to access
    /// the raw bytes for further processing or rendering.
    ///
    /// :returns: A seekable stream containing the raw graphics bytes.
    /// :rtype: io.BytesIO
    /// :raises IOError: If the graphics data cannot be read from the dataset.
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new_bound(py, &bytes);

        let io_module = py.import_bound("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// The asset-level :class:`MetadataProvider`.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }
}
