//! Python bindings for TextAssetProvider.
//!
//! This module provides the PyTextAssetProvider wrapper that exposes the
//! TextAssetProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::IntoPyObjectExt;

use crate::bindings::PyMetadataProvider;
use crate::traits::TextAssetProvider;
use crate::types::AssetType;

/// Provides access to text content stored within a geospatial dataset.
///
/// Geospatial datasets can embed plain text alongside imagery — mission
/// reports, processing notes, annotations, and similar human-readable data.
/// ``TextAssetProvider`` exposes the decoded text through the :attr:`text`
/// property, along with the character :attr:`encoding` and :attr:`format`
/// metadata. Use :meth:`DatasetReader.get_asset` to obtain an instance for
/// a specific text asset in the dataset.
///
/// Example::
///
///     from aws.osml.io import IO
///
///     with IO.open(["image.ntf"], "r") as dataset:
///         for key in dataset.get_asset_keys(asset_type="text"):
///             text_asset = dataset.get_asset(key)
///             print(f"Encoding: {text_asset.encoding}")
///             print(f"Format:   {text_asset.format}")
///             print(f"Text:     {text_asset.text[:200]}...")
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
        AssetType::Text
    }

    /// The raw asset bytes as a ``BytesIO`` object.
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new(py, &bytes);

        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// The asset-level :class:`MetadataProvider`.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    // TextAssetProvider-specific methods

    /// The decoded text content as a string.
    ///
    /// :raises ValueError: If the text cannot be decoded using the asset's
    ///     character encoding.
    #[getter]
    fn text(&self) -> PyResult<String> {
        Ok(self.inner.text()?)
    }

    /// The character encoding of the text content (e.g., ``"UTF-8"``, ``"ASCII"``).
    #[getter]
    fn encoding(&self) -> &str {
        self.inner.encoding()
    }

    /// The text format identifier.
    #[getter]
    fn format(&self) -> &str {
        self.inner.format()
    }
}
