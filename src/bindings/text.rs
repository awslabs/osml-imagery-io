//! Python bindings for TextAssetProvider.
//!
//! This module provides the PyTextAssetProvider wrapper that exposes the
//! TextAssetProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::traits::TextAssetProvider;
use crate::types::AssetType;

/// Python wrapper for TextAssetProvider trait objects.
///
/// This class provides access to text asset properties and content.
#[pyclass(name = "TextAssetProvider")]
pub struct PyTextAssetProvider {
    inner: Arc<dyn TextAssetProvider>,
}

impl PyTextAssetProvider {
    /// Creates a new PyTextAssetProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn TextAssetProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner TextAssetProvider.
    pub fn inner(&self) -> &Arc<dyn TextAssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyTextAssetProvider {
    // AssetProvider methods

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
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new_bound(py, &bytes);

        let io_module = py.import_bound("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Returns the asset-level metadata provider.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    // TextAssetProvider-specific methods

    /// Returns the decoded text content as a string.
    ///
    /// # Errors
    ///
    /// Raises a ValueError if the text cannot be decoded.
    #[getter]
    fn text(&self) -> PyResult<String> {
        Ok(self.inner.text()?)
    }

    /// Returns the character encoding (e.g., "UTF-8", "ASCII").
    #[getter]
    fn encoding(&self) -> &str {
        self.inner.encoding()
    }

    /// Returns the text format identifier.
    #[getter]
    fn format(&self) -> &str {
        self.inner.format()
    }
}
