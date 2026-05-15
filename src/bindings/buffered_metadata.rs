//! Python bindings for BufferedMetadataProvider.
//!
//! This module provides the PyBufferedMetadataProvider wrapper that exposes the
//! BufferedMetadataProvider to Python via the `collections.abc.MutableMapping` protocol.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};

use crate::bindings::metadata::json_value_to_py;
use crate::bindings::PyMetadataProvider;
use crate::buffered::BufferedMetadataProvider;
use crate::traits::MetadataProvider;

/// Convert a Python object to a serde_json::Value.
///
/// Handles None, bool, int, float, str, list, and dict. Floats are always
/// stored with fractional representation (via `Number::from_f64`) so that
/// values like `1.0` are not silently coerced to integers by serde_json.
fn python_to_json(py: Python<'_>, obj: &Py<PyAny>) -> PyResult<serde_json::Value> {
    let bound = obj.bind(py);
    if bound.is_none() {
        Ok(serde_json::Value::Null)
    } else if let Ok(b) = bound.cast::<PyBool>() {
        Ok(serde_json::Value::Bool(b.is_true()))
    } else if let Ok(i) = bound.cast::<PyInt>() {
        let val: i64 = i.extract()?;
        Ok(serde_json::json!(val))
    } else if let Ok(f) = bound.cast::<PyFloat>() {
        let val: f64 = f.extract()?;
        // Use from_f64 to preserve float representation. serde_json::json!()
        // would coerce whole-number floats like 1.0 to integer, causing
        // downstream type inference to pick TIFF_LONG instead of TIFF_DOUBLE.
        match serde_json::Number::from_f64(val) {
            Some(n) => Ok(serde_json::Value::Number(n)),
            None => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Cannot convert float {} to JSON",
                val
            ))),
        }
    } else if let Ok(s) = bound.cast::<PyString>() {
        let val: String = s.extract()?;
        Ok(serde_json::Value::String(val))
    } else if let Ok(list) = bound.cast::<PyList>() {
        let mut arr = Vec::new();
        for item in list.iter() {
            arr.push(python_to_json(py, &item.unbind())?);
        }
        Ok(serde_json::Value::Array(arr))
    } else if let Ok(dict) = bound.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key: String = k.str()?.extract()?;
            map.insert(key, python_to_json(py, &v.unbind())?);
        }
        Ok(serde_json::Value::Object(map))
    } else {
        let s: String = bound.str()?.extract()?;
        Ok(serde_json::Value::String(s))
    }
}

/// A mutable metadata provider implementing ``collections.abc.MutableMapping``.
///
/// ``BufferedMetadataProvider`` extends :class:`MetadataProvider` with write
/// operations, giving it full dictionary semantics. Use bracket notation to
/// set any native Python type (str, int, float, list, dict, bool, None) and
/// ``del`` to remove keys.
///
/// Example:
///
/// ```python
/// from aws.osml.io import BufferedMetadataProvider
///
/// metadata = BufferedMetadataProvider()
/// metadata["IC"] = "NC"
/// metadata["IMODE"] = "B"
/// metadata["33550"] = [0.5, 0.5, 0.0]     # list
/// metadata["GeoProjectedCRS"] = 32618      # int
///
/// del metadata["IC"]
/// metadata.update({"NPPBH": "256", "NPPBV": "256"})
/// metadata.clear()
/// ```
#[pyclass(name = "BufferedMetadataProvider", extends = PyMetadataProvider)]
pub struct PyBufferedMetadataProvider {
    inner: Arc<BufferedMetadataProvider>,
}

impl PyBufferedMetadataProvider {
    /// Returns a reference to the inner BufferedMetadataProvider.
    pub fn inner(&self) -> &Arc<BufferedMetadataProvider> {
        &self.inner
    }

    /// Returns the inner provider as an Arc<dyn MetadataProvider>.
    pub fn as_metadata_provider(&self) -> Arc<dyn MetadataProvider> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyBufferedMetadataProvider {
    /// Create a new ``BufferedMetadataProvider``.
    ///
    /// :param source: An existing :class:`MetadataProvider` to copy entries from.
    ///     If provided, all key-value pairs are copied into the new provider.
    /// :type source: MetadataProvider or None
    #[new]
    #[pyo3(signature = (source=None))]
    fn py_new(source: Option<PyRef<'_, PyMetadataProvider>>) -> (Self, PyMetadataProvider) {
        let simple = match source {
            Some(src) => BufferedMetadataProvider::from_provider(src.inner().as_ref()),
            None => BufferedMetadataProvider::new(),
        };
        let inner = Arc::new(simple);

        let base = PyMetadataProvider::new(inner.clone() as Arc<dyn MetadataProvider>);

        (Self { inner }, base)
    }

    fn __setitem__(&self, py: Python<'_>, key: &str, value: Py<PyAny>) -> PyResult<()> {
        let json_val = python_to_json(py, &value)?;
        self.inner.set(key, json_val);
        Ok(())
    }

    fn __delitem__(&self, key: &str) -> PyResult<()> {
        match self.inner.remove(key) {
            Some(_) => Ok(()),
            None => Err(pyo3::exceptions::PyKeyError::new_err(key.to_string())),
        }
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
            "BufferedMetadataProvider({{{}{}}}, {} fields)",
            parts.join(", "),
            ellipsis,
            total
        ))
    }

    /// Bulk update from a Python dict.
    #[pyo3(name = "update")]
    fn py_update(&self, py: Python<'_>, mapping: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut entries = HashMap::new();
        for (k, v) in mapping.iter() {
            let key: String = k.str()?.extract()?;
            let json_val = python_to_json(py, &v.unbind())?;
            entries.insert(key, json_val);
        }
        self.inner.update(entries);
        Ok(())
    }

    /// Remove all key-value pairs.
    #[pyo3(name = "clear")]
    fn py_clear(&self) {
        self.inner.clear();
    }
}
