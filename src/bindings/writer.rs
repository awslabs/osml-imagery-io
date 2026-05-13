//! Python bindings for DatasetWriter.
//!
//! This module provides the PyDatasetWriter wrapper that exposes the
//! DatasetWriter trait to Python with context manager support.

use std::sync::Arc;

use pyo3::prelude::*;

use crate::bindings::callback_provider::{
    is_duck_typed_image_provider, PyCallbackImageAssetProvider,
};
use crate::bindings::{
    PyAssetProvider, PyBufferedImageAssetProvider, PyBufferedTextAssetProvider,
    PyDataAssetProvider, PyGraphicsAssetProvider, PyImageAssetProvider, PyMetadataProvider,
    PyTextAssetProvider,
};
use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetWriter};

/// Provides write access to geospatial datasets.
///
/// A :class:`DatasetWriter` creates a new geospatial dataset (NITF, GeoTIFF,
/// etc.) and populates it with assets and metadata. Use :meth:`IO.open` with
/// mode ``"w"`` and a format name to obtain an instance. The writer handles
/// format-specific encoding details so you can focus on the content. It
/// supports the Python context manager protocol, so resources are flushed and
/// released automatically when the ``with`` block exits.
///
/// Example:
///
/// ```python
/// from aws.osml.io import IO, BufferedMetadataProvider
///
/// metadata = BufferedMetadataProvider()
/// metadata.set("IC", "NC")
///
/// with IO.open(["output.ntf"], "w", "nitf") as writer:
///     writer.metadata = metadata
///     writer.add_asset(
///         "image:0", image_provider,
///         "Primary Image", "RGB scene", ["data"],
///     )
/// ```
#[pyclass(name = "DatasetWriter")]
pub struct PyDatasetWriter {
    inner: Option<Box<dyn DatasetWriter>>,
}

impl PyDatasetWriter {
    /// Creates a new PyDatasetWriter wrapping the given trait object.
    pub fn new(inner: Box<dyn DatasetWriter>) -> Self {
        Self { inner: Some(inner) }
    }

    /// Returns a mutable reference to the inner DatasetWriter, if available.
    fn get_inner_mut(&mut self) -> PyResult<&mut Box<dyn DatasetWriter>> {
        self.inner.as_mut().ok_or_else(|| {
            CodecError::Io(std::io::Error::other("DatasetWriter has been closed")).into()
        })
    }
}

#[pymethods]
impl PyDatasetWriter {
    /// Add an asset to the dataset.
    ///
    /// Each asset is identified by a unique key and backed by an
    /// :class:`AssetProvider` (or any subtype such as
    /// :class:`BufferedImageAssetProvider`).
    ///
    /// :param key: Unique string identifier for the asset.
    /// :type key: str
    /// :param provider: The asset data to add.
    /// :type provider: AssetProvider | ImageAssetProvider | BufferedImageAssetProvider
    /// :param title: Human-readable title for the asset.
    /// :type title: str
    /// :param description: Detailed description of the asset.
    /// :type description: str
    /// :param roles: Semantic roles (e.g., ``"data"``, ``"thumbnail"``).
    /// :type roles: list[str]
    /// :raises ValueError: If an asset with the given key already exists.
    /// :raises TypeError: If *provider* is not a valid asset provider type.
    ///
    /// Example:
    ///
    /// ```python
    /// writer.add_asset(
    ///     "image:0", image_provider,
    ///     "Primary Image", "RGB scene", ["data"],
    /// )
    /// ```
    #[pyo3(signature = (key, provider, title, description, roles))]
    fn add_asset(
        &mut self,
        _py: Python<'_>,
        key: &str,
        provider: &Bound<'_, PyAny>,
        title: &str,
        description: &str,
        roles: Vec<String>,
    ) -> PyResult<()> {
        let inner = self.get_inner_mut()?;

        // Extract the AssetProvider enum from whichever Python type was passed
        let enum_provider = if let Ok(img) = provider.extract::<PyRef<PyImageAssetProvider>>() {
            AssetProvider::Image(img.inner().clone())
        } else if let Ok(buf) = provider.extract::<PyRef<PyBufferedImageAssetProvider>>() {
            AssetProvider::Image(buf.inner().clone())
        } else if let Ok(txt) = provider.extract::<PyRef<PyTextAssetProvider>>() {
            AssetProvider::Text(txt.inner().clone())
        } else if let Ok(buf_txt) = provider.extract::<PyRef<PyBufferedTextAssetProvider>>() {
            AssetProvider::Text(buf_txt.as_text_provider())
        } else if let Ok(data) = provider.extract::<PyRef<PyDataAssetProvider>>() {
            AssetProvider::Data(data.inner().clone())
        } else if let Ok(gfx) = provider.extract::<PyRef<PyGraphicsAssetProvider>>() {
            AssetProvider::Graphics(gfx.inner().clone())
        } else if let Ok(asset) = provider.extract::<PyRef<PyAssetProvider>>() {
            // PyAssetProvider wraps an AssetProvider enum internally
            asset.inner().clone()
        } else if is_duck_typed_image_provider(provider) {
            // Duck-typing fallback for Python-defined image providers
            let adapter = PyCallbackImageAssetProvider::new(_py, provider)?;
            AssetProvider::Image(Arc::new(adapter))
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "provider must be an AssetProvider, ImageAssetProvider, \
                 BufferedImageAssetProvider, TextAssetProvider, \
                 BufferedTextAssetProvider, DataAssetProvider, \
                 GraphicsAssetProvider, or a duck-typed image provider \
                 with required methods (get_block, num_rows, etc.)",
            ));
        };

        inner.add_asset(key, enum_provider, title, description, &roles)?;
        Ok(())
    }

    /// Set the dataset-level metadata.
    ///
    /// Assign a :class:`MetadataProvider` (or :class:`BufferedMetadataProvider`)
    /// containing file-level fields for the output file. For NITF, this populates
    /// the file header (security markings, originator, etc.). For TIFF, this has
    /// no effect — IFD tags and encoding hints are sourced from each asset
    /// provider's metadata instead.
    ///
    /// :raises IOError: If the metadata cannot be applied to the dataset.
    #[setter]
    fn metadata(&mut self, metadata: &PyMetadataProvider) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        let metadata_provider = Arc::clone(metadata.inner());
        inner.set_metadata(metadata_provider)?;
        Ok(())
    }

    /// Flush pending data and release all resources.
    ///
    /// After calling this method the writer should not be used. When using
    /// the context manager (``with`` statement), ``close`` is called
    /// automatically on exit.
    ///
    /// :raises IOError: If flushing data to storage fails.
    fn close(&mut self) -> PyResult<()> {
        if let Some(mut inner) = self.inner.take() {
            inner.close()?;
        }
        Ok(())
    }

    /// Context manager entry point.
    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Enable or disable strict encoding validation for metadata fields.
    ///
    /// When *strict* is ``True``, numeric TRE fields are validated against
    /// their exact declared encoding (e.g. BCS-NPI rejects ``+``, ``-``,
    /// ``.``). When ``False`` (the default), numeric fields accept any
    /// printable ASCII, tolerating real-world deviations from the spec.
    ///
    /// :param strict: Whether to enforce strict encoding validation.
    /// :type strict: bool
    ///
    /// Example:
    ///
    /// ```python
    /// with IO.open("output.ntf", "w", "nitf") as writer:
    ///     writer.strict_encoding = True  # enforce spec-exact validation
    /// ```
    #[setter]
    fn strict_encoding(&mut self, strict: bool) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        inner.set_strict_encoding(strict);
        Ok(())
    }

    /// Context manager exit point.
    ///
    /// Automatically closes the writer when exiting the context.
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }
}
