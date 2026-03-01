//! Python bindings for DatasetReader.
//!
//! This module provides the PyDatasetReader wrapper that exposes the
//! DatasetReader trait to Python with context manager support.

use std::sync::Arc;

use pyo3::prelude::*;

use crate::bindings::{
    PyAssetProvider, PyDataAssetProvider, PyGraphicsAssetProvider, PyImageAssetProvider,
    PyMetadataProvider, PyTextAssetProvider,
};
use crate::error::CodecError;
use crate::traits::{
    AssetProvider, DataAssetProvider, DatasetReader, GraphicsAssetProvider, ImageAssetProvider,
    TextAssetProvider,
};
use crate::types::AssetType;

// Import JBP asset providers for downcasting
use crate::jbp::JBPImageAssetProvider;

/// Python wrapper for DatasetReader trait objects.
///
/// This class provides access to geospatial datasets through a unified interface,
/// allowing access to imagery and metadata without knowing format-specific details.
///
/// Supports Python context manager protocol via `__enter__` and `__exit__` methods.
#[pyclass(name = "DatasetReader")]
pub struct PyDatasetReader {
    inner: Option<Box<dyn DatasetReader>>,
}

impl PyDatasetReader {
    /// Creates a new PyDatasetReader wrapping the given trait object.
    pub fn new(inner: Box<dyn DatasetReader>) -> Self {
        Self { inner: Some(inner) }
    }

    /// Returns a reference to the inner DatasetReader, if available.
    fn get_inner(&self) -> PyResult<&dyn DatasetReader> {
        self.inner
            .as_ref()
            .map(|b| b.as_ref())
            .ok_or_else(|| CodecError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "DatasetReader has been closed",
            )).into())
    }
}


#[pymethods]
impl PyDatasetReader {
    /// Returns an AssetProvider for the specified asset key.
    ///
    /// The returned object type depends on the asset type:
    /// - Image assets return ImageAssetProvider
    /// - Text assets return TextAssetProvider
    /// - Data assets return DataAssetProvider
    /// - Graphics assets return GraphicsAssetProvider
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    ///
    /// # Raises
    ///
    /// * KeyError - If no asset with the given key exists.
    fn get_asset(&self, py: Python<'_>, key: &str) -> PyResult<PyObject> {
        let inner = self.get_inner()?;
        let asset = inner.get_asset(key)?;

        // Return the appropriate Python wrapper based on asset type
        match asset.asset_type() {
            AssetType::Image => {
                // Try to downcast to ImageAssetProvider
                if let Some(image_provider) = try_as_image_provider(&asset) {
                    Ok(PyImageAssetProvider::new(image_provider).into_py(py))
                } else {
                    Ok(PyAssetProvider::new(asset).into_py(py))
                }
            }
            AssetType::Text => {
                if let Some(text_provider) = try_as_text_provider(&asset) {
                    Ok(PyTextAssetProvider::new(text_provider).into_py(py))
                } else {
                    Ok(PyAssetProvider::new(asset).into_py(py))
                }
            }
            AssetType::Data => {
                if let Some(data_provider) = try_as_data_provider(&asset) {
                    Ok(PyDataAssetProvider::new(data_provider).into_py(py))
                } else {
                    Ok(PyAssetProvider::new(asset).into_py(py))
                }
            }
            AssetType::Graphics => {
                if let Some(graphics_provider) = try_as_graphics_provider(&asset) {
                    Ok(PyGraphicsAssetProvider::new(graphics_provider).into_py(py))
                } else {
                    Ok(PyAssetProvider::new(asset).into_py(py))
                }
            }
        }
    }

    /// Returns a list of asset keys matching the filter criteria.
    ///
    /// # Arguments
    ///
    /// * `asset_type` - Optional filter to return only assets of the specified type.
    /// * `roles` - Optional filter to return only assets with any of the specified roles.
    ///
    /// # Returns
    ///
    /// A list of asset keys matching the filter criteria.
    #[pyo3(signature = (asset_type=None, roles=None))]
    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<Vec<String>>,
    ) -> PyResult<Vec<String>> {
        let inner = self.get_inner()?;
        let roles_slice = roles.as_deref();
        Ok(inner.get_asset_keys(asset_type, roles_slice))
    }

    /// Returns true if an asset with the given key exists.
    ///
    /// # Arguments
    ///
    /// * `key` - The unique string identifier for the asset.
    fn has_asset(&self, key: &str) -> PyResult<bool> {
        let inner = self.get_inner()?;
        Ok(inner.has_asset(key))
    }

    /// Returns the dataset-level metadata provider.
    #[getter]
    fn metadata(&self) -> PyResult<PyMetadataProvider> {
        let inner = self.get_inner()?;
        Ok(PyMetadataProvider::new(inner.metadata()))
    }

    /// Releases all resources associated with this reader.
    ///
    /// After calling this method, the reader should not be used.
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
    /// Automatically closes the reader when exiting the context.
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

/// Attempts to convert an AssetProvider to an ImageAssetProvider.
///
/// This function checks if the underlying implementation supports the
/// ImageAssetProvider trait by attempting to downcast to known concrete types.
fn try_as_image_provider(
    asset: &Arc<dyn AssetProvider>,
) -> Option<Arc<dyn ImageAssetProvider>> {
    // Try to downcast to JBPImageAssetProvider using as_any
    if asset.asset_type() == AssetType::Image {
        if asset.as_any().downcast_ref::<JBPImageAssetProvider>().is_some() {
            // We need to clone the Arc and return it as ImageAssetProvider
            // Since we can't directly convert Arc<dyn AssetProvider> to Arc<dyn ImageAssetProvider>,
            // we need to create a new Arc from the concrete type
            // This is safe because we've verified the concrete type
            let ptr = Arc::as_ptr(asset);
            // SAFETY: We've verified the concrete type is JBPImageAssetProvider
            // which implements ImageAssetProvider. We increment the ref count.
            unsafe {
                Arc::increment_strong_count(ptr);
                let concrete_ptr = ptr as *const JBPImageAssetProvider;
                Some(Arc::from_raw(concrete_ptr as *const dyn ImageAssetProvider))
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Attempts to convert an AssetProvider to a TextAssetProvider.
/// 
/// Note: JBPTextAssetProvider does not implement TextAssetProvider trait yet,
/// so this always returns None. Text assets are returned as generic AssetProvider.
fn try_as_text_provider(
    _asset: &Arc<dyn AssetProvider>,
) -> Option<Arc<dyn TextAssetProvider>> {
    // JBPTextAssetProvider doesn't implement TextAssetProvider yet
    None
}

/// Attempts to convert an AssetProvider to a DataAssetProvider.
/// 
/// Note: JBPDataAssetProvider does not implement DataAssetProvider trait yet,
/// so this always returns None. Data assets are returned as generic AssetProvider.
fn try_as_data_provider(
    _asset: &Arc<dyn AssetProvider>,
) -> Option<Arc<dyn DataAssetProvider>> {
    // JBPDataAssetProvider doesn't implement DataAssetProvider yet
    None
}

/// Attempts to convert an AssetProvider to a GraphicsAssetProvider.
/// 
/// Note: JBPGraphicsAssetProvider does not implement GraphicsAssetProvider trait yet,
/// so this always returns None. Graphics assets are returned as generic AssetProvider.
fn try_as_graphics_provider(
    _asset: &Arc<dyn AssetProvider>,
) -> Option<Arc<dyn GraphicsAssetProvider>> {
    // JBPGraphicsAssetProvider doesn't implement GraphicsAssetProvider yet
    None
}
