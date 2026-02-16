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

/// Python wrapper for DataAssetProvider trait objects.
///
/// This class provides access to data asset properties and parsing methods.
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

    // DataAssetProvider-specific methods

    /// Returns the MIME type of the data.
    fn get_mime_type(&self) -> &str {
        self.inner.mime_type()
    }

    /// Parses the content as XML and returns a string representation.
    ///
    /// # Errors
    ///
    /// Raises a ValueError if the content is not valid XML.
    fn parse_as_xml(&self) -> PyResult<String> {
        Ok(self.inner.parse_as_xml()?)
    }

    /// Parses the content as JSON and returns a Python dictionary.
    ///
    /// # Errors
    ///
    /// Raises a ValueError if the content is not valid JSON.
    fn parse_as_json(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json_value = self.inner.parse_as_json()?;
        serde_json_value_to_pyobject(py, &json_value)
    }
}

/// Converts a serde_json::Value to a PyObject.
fn serde_json_value_to_pyobject(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b.into_py(py)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_py(py))
            } else if let Some(u) = n.as_u64() {
                Ok(u.into_py(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_py(py))
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(s) => Ok(s.into_py(py)),
        serde_json::Value::Array(arr) => {
            let py_list = pyo3::types::PyList::empty_bound(py);
            for item in arr {
                py_list.append(serde_json_value_to_pyobject(py, item)?)?;
            }
            Ok(py_list.into())
        }
        serde_json::Value::Object(obj) => {
            let py_dict = pyo3::types::PyDict::new_bound(py);
            for (k, v) in obj {
                py_dict.set_item(k, serde_json_value_to_pyobject(py, v)?)?;
            }
            Ok(py_dict.into())
        }
    }
}
