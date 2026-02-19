//! Python bindings for DatasetWriter.
//!
//! This module provides the PyDatasetWriter wrapper that exposes the
//! DatasetWriter trait to Python with context manager support.

use std::sync::Arc;

use pyo3::prelude::*;

use crate::bindings::{PyAssetProvider, PyMemoryImageAssetProvider, PyMetadataProvider};
use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetWriter};

/// Python wrapper for DatasetWriter trait objects.
///
/// This class provides the ability to write geospatial datasets through a unified
/// interface, allowing creation of imagery files without knowing format-specific
/// encoding details.
///
/// Supports Python context manager protocol via `__enter__` and `__exit__` methods.
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
        self.inner
            .as_mut()
            .ok_or_else(|| {
                CodecError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "DatasetWriter has been closed",
                ))
                .into()
            })
    }
}

#[pymethods]
impl PyDatasetWriter {
    /// Adds an asset to the dataset.
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    /// * `provider` - The AssetProvider containing the asset data.
    /// * `title` - A human-readable title for the asset.
    /// * `description` - A detailed description of the asset.
    /// * `roles` - Semantic roles for the asset (e.g., "data", "thumbnail", "metadata").
    ///
    /// # Raises
    ///
    /// * ValueError - If an asset with the given key already exists.
    #[pyo3(signature = (key, provider, title, description, roles))]
    fn add_asset(
        &mut self,
        key: &str,
        provider: &PyAssetProvider,
        title: &str,
        description: &str,
        roles: Vec<String>,
    ) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        let asset_provider = Arc::clone(provider.inner());
        inner.add_asset(key, asset_provider, title, description, &roles)?;
        Ok(())
    }

    /// Adds an image asset to the dataset using a MemoryImageAssetProvider.
    ///
    /// This method accepts a MemoryImageAssetProvider which contains image
    /// configuration (dimensions, pixel type, blocking, etc.) that will be
    /// used to create the proper NITF image subheader.
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    /// * `provider` - The MemoryImageAssetProvider containing the image data and configuration.
    /// * `title` - A human-readable title for the asset.
    /// * `description` - A detailed description of the asset.
    /// * `roles` - Semantic roles for the asset (e.g., "data", "thumbnail", "metadata").
    ///
    /// # Raises
    ///
    /// * ValueError - If an asset with the given key already exists.
    #[pyo3(signature = (key, provider, title, description, roles))]
    fn add_image_asset(
        &mut self,
        key: &str,
        provider: &PyMemoryImageAssetProvider,
        title: &str,
        description: &str,
        roles: Vec<String>,
    ) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        // Clone the Arc and cast to dyn AssetProvider
        let asset_provider: Arc<dyn AssetProvider> = provider.inner().clone();
        inner.add_asset(key, asset_provider, title, description, &roles)?;
        Ok(())
    }

    /// Sets the dataset-level metadata.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The MetadataProvider containing the metadata to set.
    ///
    /// # Raises
    ///
    /// * IOError - If the metadata cannot be set.
    fn set_metadata(&mut self, metadata: &PyMetadataProvider) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        let metadata_provider = Arc::clone(metadata.inner());
        inner.set_metadata(metadata_provider)?;
        Ok(())
    }

    /// Finalizes the dataset and releases all resources.
    ///
    /// This method flushes all pending data to storage and closes the dataset.
    /// After calling this method, the writer should not be used.
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

    /// Context manager exit point.
    ///
    /// Automatically closes the writer when exiting the context.
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<PyObject>,
        _exc_val: Option<PyObject>,
        _exc_tb: Option<PyObject>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }
}
