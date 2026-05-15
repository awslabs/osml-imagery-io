//! Python bindings for MetadataProvider.
//!
//! This module provides the PyMetadataProvider wrapper that exposes the
//! MetadataProvider trait to Python via the `collections.abc.Mapping` protocol.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};

use crate::traits::MetadataProvider;

/// A read-only metadata provider implementing the ``collections.abc.Mapping`` protocol.
///
/// ``MetadataProvider`` exposes metadata as a dictionary-like object. You can
/// access individual fields with bracket notation (``metadata["IC"]``), iterate
/// keys, check membership with ``in``, and convert to a plain ``dict`` via
/// :meth:`entries` or ``dict(metadata)``.
///
/// You typically obtain a ``MetadataProvider`` from a
/// :class:`DatasetReader` or an :class:`AssetProvider` rather than creating
/// one directly.
///
/// Example:
///
/// ```python
/// from aws.osml.io import IO
///
/// with IO.open(["image.ntf"], "r") as dataset:
///     meta = dataset.metadata
///     ic = meta["IC"]                      # KeyError if missing
///     ic = meta.get("IC", "NC")            # default if missing
///     all_meta = meta.entries()            # full dict (single Rust call)
///     security = meta.entries("FS")        # prefix filter
///     for key in meta:
///         print(key, meta[key])
/// ```
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

        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    fn __getitem__<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Py<PyAny>> {
        match self.inner.get_value(key) {
            Some(value) => json_value_to_py(py, &value),
            None => Err(pyo3::exceptions::PyKeyError::new_err(key.to_string())),
        }
    }

    fn __contains__(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let keys = self.inner.keys();
        let py_list = PyList::new(py, &keys)?;
        let iter = py_list.call_method0("__iter__")?;
        Ok(iter.into())
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<String> {
        let total = self.inner.len();
        let keys = self.inner.keys();
        let preview_count = keys.len().min(5);
        let mut parts = Vec::with_capacity(preview_count);
        for key in keys.iter().take(preview_count) {
            if let Some(value) = self.inner.get_value(key) {
                let py_val = json_value_to_py(py, &value)?;
                let repr: String = py_val.bind(py).repr()?.extract()?;
                parts.push(format!("'{}': {}", key, repr));
            }
        }
        let ellipsis = if total > preview_count { ", ..." } else { "" };
        Ok(format!(
            "MetadataProvider({{{}{}}}, {} fields)",
            parts.join(", "),
            ellipsis,
            total
        ))
    }

    /// Retrieve the value for the given key, or a default if absent.
    ///
    /// :param key: The metadata field name.
    /// :type key: str
    /// :param default: Value to return if key is not present (default: None).
    /// :returns: The value for the key, or the default.
    #[pyo3(signature = (key, default=None))]
    fn get<'py>(
        &self,
        py: Python<'py>,
        key: &str,
        default: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        match self.inner.get_value(key) {
            Some(value) => json_value_to_py(py, &value),
            None => Ok(default.unwrap_or_else(|| py.None())),
        }
    }

    /// Return a list of all metadata keys.
    fn keys<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let keys = self.inner.keys();
        let py_list = PyList::new(py, &keys)?;
        Ok(py_list.into())
    }

    /// Return a list of all metadata values.
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let keys = self.inner.keys();
        let py_list = PyList::empty(py);
        for key in &keys {
            if let Some(value) = self.inner.get_value(key) {
                py_list.append(json_value_to_py(py, &value)?)?;
            }
        }
        Ok(py_list.into())
    }

    /// Return a list of (key, value) tuples.
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let keys = self.inner.keys();
        let py_list = PyList::empty(py);
        for key in &keys {
            if let Some(value) = self.inner.get_value(key) {
                let py_val = json_value_to_py(py, &value)?;
                let tuple = PyTuple::new(py, &[key.into_pyobject(py)?.into_any(), py_val.bind(py).clone()])?;
                py_list.append(tuple)?;
            }
        }
        Ok(py_list.into())
    }

    /// Return metadata as a Python dictionary, optionally filtered by key prefix.
    ///
    /// When *name* is provided, only keys that start with that prefix are
    /// included. When omitted, all metadata fields are returned. This is the
    /// fast path for bulk export (single Rust→Python crossing).
    ///
    /// :param name: Key prefix used to filter the returned fields.
    /// :type name: str, optional
    /// :returns: Metadata fields as a dictionary.
    /// :rtype: dict
    #[pyo3(signature = (name=None))]
    fn entries<'py>(&self, py: Python<'py>, name: Option<&str>) -> PyResult<Py<PyAny>> {
        let metadata = self.inner.entries(name);
        let dict = PyDict::new(py);

        for (key, value) in metadata {
            let py_value = json_value_to_py(py, &value)?;
            dict.set_item(key, py_value)?;
        }

        Ok(dict.into())
    }
}

/// Converts a serde_json::Value to a Python object.
pub(crate) fn json_value_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
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
