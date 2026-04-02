//! Python bindings for MetadataProvider.
//!
//! This module provides the PyMetadataProvider wrapper that exposes the
//! MetadataProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use pyo3::IntoPyObjectExt;

use crate::traits::MetadataProvider;

/// Provides access to key-value metadata associated with a dataset or asset.
///
/// Every dataset and asset exposes metadata through a ``MetadataProvider``.
/// You can retrieve metadata as a Python dictionary via :meth:`as_dict`, with
/// an optional prefix filter to select a group of related fields, or obtain
/// the underlying bytes in their original binary format via the :attr:`raw`
/// property. The dictionary values are native Python types — ``str``, ``int``,
/// ``list``, or nested ``dict`` — depending on how the field is defined in the
/// format's structure definition.
///
/// You typically obtain a ``MetadataProvider`` from a
/// :class:`DatasetReader` or an :class:`AssetProvider` rather than creating
/// one directly.
///
/// Example::
///
///     from aws.osml.io import IO
///
///     with IO.open(["image.ntf"], "r") as dataset:
///         # All dataset-level metadata
///         all_meta = dataset.metadata.as_dict()
///
///         # Only fields whose key starts with "FS" (file security)
///         security = dataset.metadata.as_dict("FS")
#[pyclass(name = "MetadataProvider", subclass)]
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
    /// The underlying metadata in its original binary format, as a ``BytesIO`` object.
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw();
        let py_bytes = PyBytes::new(py, bytes);

        // Import io.BytesIO and create instance
        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Return metadata as a Python dictionary, optionally filtered by key prefix.
    ///
    /// When *name* is provided, only keys that start with that prefix are
    /// included. When omitted, all metadata fields are returned.
    ///
    /// :param name: Key prefix used to filter the returned fields.
    /// :type name: str, optional
    /// :returns: Metadata fields as a dictionary mapping string keys to
    ///     native Python values (``str``, ``int``, ``list``, or ``dict``).
    /// :rtype: dict
    ///
    /// Example::
    ///
    ///     all_meta = provider.as_dict()
    ///     security = provider.as_dict("FS")
    #[pyo3(signature = (name=None))]
    fn as_dict<'py>(&self, py: Python<'py>, name: Option<&str>) -> PyResult<Py<PyAny>> {
        let metadata = self.inner.as_dict(name);
        let dict = PyDict::new(py);

        for (key, value) in metadata {
            let py_value = json_value_to_py(py, &value)?;
            dict.set_item(key, py_value)?;
        }

        Ok(dict.into())
    }
}

/// Converts a serde_json::Value to a Python object.
fn json_value_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else if let Some(u) = n.as_u64() {
                Ok(u.into_pyobject(py)?.into_any().unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)?.into_any().unbind())
            } else {
                // Fallback: convert to string
                Ok(n.to_string().into_pyobject(py)?.into_any().unbind())
            }
        }
        serde_json::Value::String(s) => Ok(s.into_pyobject(py)?.into_any().unbind()),
        serde_json::Value::Array(arr) => {
            let py_list = PyList::empty(py);
            for item in arr {
                py_list.append(json_value_to_py(py, item)?)?;
            }
            Ok(py_list.into())
        }
        serde_json::Value::Object(obj) => {
            let py_dict = PyDict::new(py);
            for (k, v) in obj {
                py_dict.set_item(k, json_value_to_py(py, v)?)?;
            }
            Ok(py_dict.into())
        }
    }
}
