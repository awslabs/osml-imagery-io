//! Python bindings for MetadataProvider.
//!
//! This module provides the PyMetadataProvider wrapper that exposes the
//! MetadataProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

use crate::traits::MetadataProvider;

/// Python wrapper for MetadataProvider trait objects.
///
/// This class provides access to metadata in both raw byte form and as
/// structured dictionaries.
#[pyclass(name = "MetadataProvider")]
pub struct PyMetadataProvider {
    inner: Arc<dyn MetadataProvider>,
}

impl PyMetadataProvider {
    /// Creates a new PyMetadataProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn MetadataProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner MetadataProvider.
    pub fn inner(&self) -> &Arc<dyn MetadataProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyMetadataProvider {
    /// Returns the raw metadata bytes as a BytesIO object.
    ///
    /// This provides access to the underlying metadata in its original binary format.
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> PyResult<PyObject> {
        let bytes = self.inner.raw();
        let py_bytes = PyBytes::new_bound(py, bytes);

        // Import io.BytesIO and create instance
        let io_module = py.import_bound("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Returns metadata as a dictionary, optionally filtered by section name.
    ///
    /// # Arguments
    ///
    /// * `name` - Optional section name to filter the returned metadata.
    ///   - If provided, returns only the named metadata section.
    ///   - If not provided, returns all metadata sections.
    ///
    /// # Returns
    ///
    /// A dictionary where keys are metadata field names and values are
    /// JSON-compatible Python objects.
    #[pyo3(signature = (name=None))]
    fn as_dict<'py>(&self, py: Python<'py>, name: Option<&str>) -> PyResult<PyObject> {
        let metadata = self.inner.as_dict(name);
        let dict = PyDict::new_bound(py);

        for (key, value) in metadata {
            let py_value = json_value_to_py(py, &value)?;
            dict.set_item(key, py_value)?;
        }

        Ok(dict.into())
    }
}

/// Converts a serde_json::Value to a Python object.
fn json_value_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b.to_object(py)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_object(py))
            } else if let Some(u) = n.as_u64() {
                Ok(u.to_object(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_object(py))
            } else {
                // Fallback: convert to string
                Ok(n.to_string().to_object(py))
            }
        }
        serde_json::Value::String(s) => Ok(s.to_object(py)),
        serde_json::Value::Array(arr) => {
            let py_list = PyList::empty_bound(py);
            for item in arr {
                py_list.append(json_value_to_py(py, item)?)?;
            }
            Ok(py_list.into())
        }
        serde_json::Value::Object(obj) => {
            let py_dict = PyDict::new_bound(py);
            for (k, v) in obj {
                py_dict.set_item(k, json_value_to_py(py, v)?)?;
            }
            Ok(py_dict.into())
        }
    }
}
