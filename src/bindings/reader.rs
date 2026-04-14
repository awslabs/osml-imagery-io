//! Python bindings for DatasetReader.
//!
//! This module provides the PyDatasetReader wrapper that exposes the
//! DatasetReader trait to Python with context manager support.

use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;

use crate::bindings::{
    PyDataAssetProvider, PyGraphicsAssetProvider, PyImageAssetProvider,
    PyMetadataProvider, PyTextAssetProvider,
};
use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetReader};
use crate::types::AssetType;

/// Provides read access to geospatial datasets.
///
/// A :class:`DatasetReader` exposes the assets and metadata contained in a
/// geospatial dataset (NITF, GeoTIFF, etc.) through a uniform interface.
/// Use :meth:`IO.open` with mode ``"r"`` to obtain an instance. The reader
/// supports the Python context manager protocol, so resources are released
/// automatically when the ``with`` block exits.
///
/// Example::
///
///     from aws.osml.io import IO
///
///     with IO.open(["image.ntf"], "r") as dataset:
///         image_keys = dataset.get_asset_keys(asset_type="image")
///         print(f"Found {len(image_keys)} image assets")
///
///         image = dataset.get_asset(image_keys[0])
///         print(type(image))  # ImageAssetProvider
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
            .ok_or_else(|| CodecError::Io(std::io::Error::other(
                "DatasetReader has been closed",
            )).into())
    }
}


#[pymethods]
impl PyDatasetReader {
    /// Retrieve an asset by its unique key.
    ///
    /// The returned object type depends on the asset's type:
    /// :class:`ImageAssetProvider` for images,
    /// :class:`TextAssetProvider` for text,
    /// :class:`DataAssetProvider` for structured data, or
    /// :class:`GraphicsAssetProvider` for vector graphics.
    ///
    /// :param key: Unique string identifier for the asset.
    /// :type key: str
    /// :returns: An asset provider whose concrete type matches the asset's type.
    /// :rtype: ImageAssetProvider | TextAssetProvider | DataAssetProvider | GraphicsAssetProvider
    /// :raises KeyError: If no asset with the given key exists.
    ///
    /// Example::
    ///
    ///     image = dataset.get_asset("image:0")
    fn get_asset(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        let inner = self.get_inner()?;
        let asset = inner.get_asset(key)?;

        match asset {
            AssetProvider::Image(img) => {
                Ok(PyImageAssetProvider::new(img).into_pyobject(py)?.into_any().unbind())
            }
            AssetProvider::Text(txt) => {
                Ok(PyTextAssetProvider::new(txt).into_pyobject(py)?.into_any().unbind())
            }
            AssetProvider::Data(data) => {
                Ok(PyDataAssetProvider::new(data).into_pyobject(py)?.into_any().unbind())
            }
            AssetProvider::Graphics(gfx) => {
                Ok(PyGraphicsAssetProvider::new(gfx).into_pyobject(py)?.into_any().unbind())
            }
        }
    }

    /// List asset keys, optionally filtered by type or roles.
    ///
    /// :param asset_type: Restrict results to assets of this type
    ///     (e.g., ``"image"``, ``"text"``, ``"data"``, ``"graphics"``).
    /// :type asset_type: str, optional
    /// :param roles: Restrict results to assets that have any of the
    ///     specified roles.
    /// :type roles: list[str], optional
    /// :returns: Asset keys matching the filter criteria.
    /// :rtype: list[str]
    ///
    /// Example::
    ///
    ///     image_keys = dataset.get_asset_keys(asset_type="image")
    ///     text_keys = dataset.get_asset_keys(asset_type="text")
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

    /// Check whether an asset with the given key exists.
    ///
    /// :param key: Unique string identifier for the asset.
    /// :type key: str
    /// :returns: ``True`` if the asset exists, ``False`` otherwise.
    /// :rtype: bool
    fn has_asset(&self, key: &str) -> PyResult<bool> {
        let inner = self.get_inner()?;
        Ok(inner.has_asset(key))
    }

    /// Dataset-level metadata as a :class:`MetadataProvider`.
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
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }
}
