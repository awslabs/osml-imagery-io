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

/// Warn that an assignment collapsed a repeated key to a single value.
///
/// Legal — ``d[k] = v`` means *k maps to v afterwards* — but it is exactly the
/// silent narrowing that made repeated NITF extensions unreachable in the first
/// place, so it is worth surfacing. Raised from the binding layer rather than the
/// Rust core so it is catchable with :func:`warnings.catch_warnings` and testable
/// with ``pytest.warns``.
fn warn_instances_discarded(py: Python<'_>, key: &str, discarded: usize) -> PyResult<()> {
    let warnings = py.import("warnings")?;
    let message = format!(
        "'{}' held {} instances; assignment replaces the whole slot, \
         so {} were discarded. Use set_all() to write several instances.",
        key,
        discarded,
        discarded - 1
    );
    warnings.call_method1(
        "warn",
        (
            message,
            py.get_type::<pyo3::exceptions::PyUserWarning>(),
            2u8,
        ),
    )?;
    Ok(())
}

/// A mutable metadata provider implementing ``collections.abc.MutableMapping``.
///
/// ``BufferedMetadataProvider`` extends :class:`MetadataProvider` with write
/// operations, giving it full dictionary semantics. Use bracket notation to
/// set any native Python type (str, int, float, list, dict, bool, None) and
/// ``del`` to remove keys.
///
/// A key may hold several ordered values in formats that allow it: :meth:`set_all`
/// and :meth:`append` author more than one, and the inherited
/// :meth:`MetadataProvider.get_all` reads them all back. ``metadata[key]`` remains
/// the first value, and assignment is a *whole-slot* operation:
/// ``metadata[key] = {...}`` discards any other values (with a
/// :class:`UserWarning` if there were any).
///
/// In NITF/NSIF this is how you write a repeated tagged record extension — one
/// instance per dict, under the CETAG.
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
/// metadata.set_all("PIAPEA", [{"LASTNME": "DURHAM"}, {"LASTNME": "DAILEY"}])
/// metadata.append("PIAPEA", {"LASTNME": "WEBB"})
/// metadata.get_all("PIAPEA")                  # three instances, in order
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
    /// Build the two-layer initializer: the base wraps the same provider as the
    /// subclass, so the inherited accessors (`get_all`, `keys`, `__getitem__`)
    /// read the live buffer rather than a copy.
    fn initializer(inner: Arc<BufferedMetadataProvider>) -> PyClassInitializer<Self> {
        let base = PyMetadataProvider::new(inner.clone() as Arc<dyn MetadataProvider>);
        PyClassInitializer::from(base).add_subclass(Self { inner })
    }

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
    ///     If provided, all key-value pairs are copied into the new provider,
    ///     including every instance of a repeated extension. See
    ///     :meth:`from_provider`.
    /// :type source: MetadataProvider or None
    #[new]
    #[pyo3(signature = (source=None))]
    fn py_new(source: Option<PyRef<'_, PyMetadataProvider>>) -> PyClassInitializer<Self> {
        let simple = match source {
            Some(src) => BufferedMetadataProvider::from_provider(src.inner().as_ref()),
            None => BufferedMetadataProvider::new(),
        };
        Self::initializer(Arc::new(simple))
    }

    /// Copy every key out of *source* into a new writable provider.
    ///
    /// This is the duplicate-safe copy path: every key is copied through
    /// :meth:`MetadataProvider.get_all`, so repeated values survive with no special
    /// case and no ordering requirement.
    ///
    /// Doing it by hand with ``update(src.entries())`` is lossy — ``entries()``
    /// carries only the first value of each key, and being bulk ``__setitem__`` it
    /// obeys the whole-slot rule, so it would discard the very instances you meant to
    /// copy.
    ///
    /// :param source: The provider to copy from.
    /// :type source: MetadataProvider
    /// :rtype: BufferedMetadataProvider
    ///
    /// Example:
    ///
    /// ```python
    /// with IO.open(["in.ntf"], "r") as dataset:
    ///     image = dataset.get_asset("image:0")
    ///     out = BufferedMetadataProvider.from_provider(image.metadata)
    ///     out.get_all("PIAPEA")          # every instance survived the copy
    /// ```
    #[staticmethod]
    fn from_provider(py: Python<'_>, source: PyRef<'_, PyMetadataProvider>) -> PyResult<Py<Self>> {
        let copied = BufferedMetadataProvider::from_provider(source.inner().as_ref());
        Py::new(py, Self::initializer(Arc::new(copied)))
    }

    fn __setitem__(&self, py: Python<'_>, key: &str, value: Py<PyAny>) -> PyResult<()> {
        let json_val = python_to_json(py, &value)?;
        let discarded = self.inner.set(key, json_val);
        if discarded > 1 {
            warn_instances_discarded(py, key, discarded)?;
        }
        Ok(())
    }

    /// Replace every value stored under *key* with *instances*, in order.
    ///
    /// Order is preserved exactly as given and is the order the writer emits. For
    /// NITF that matters whenever a repeated extension's instances form a sequence:
    /// ``CSEPHA`` is defined in time-sequence order (STDI-0002 Vol 1 App D) and
    /// ``BCHIPA`` instances form a UUID-linked series (Vol 1 App AR).
    ///
    /// Passing an empty list removes the key.
    ///
    /// :param key: The metadata key — a CETAG such as ``"PIAPEA"`` for NITF records.
    /// :type key: str
    /// :param instances: One entry per instance, in the order to write them.
    /// :type instances: list
    fn set_all(&self, py: Python<'_>, tag: &str, instances: Vec<Py<PyAny>>) -> PyResult<()> {
        let mut values = Vec::with_capacity(instances.len());
        for instance in &instances {
            values.push(python_to_json(py, instance)?);
        }
        self.inner.set_all(tag, values);
        Ok(())
    }

    /// Append one more value under *key*, keeping the instances already stored.
    ///
    /// This is the mapping-level counterpart of ``list.append``: it grows the slot
    /// rather than replacing it, so it never discards anything.
    ///
    /// :param key: The metadata key — a CETAG such as ``"PIAPEA"`` for NITF records.
    /// :type key: str
    /// :param instance: The value to append; a dict of fields for a NITF record.
    /// :type instance: object
    fn append(&self, py: Python<'_>, tag: &str, instance: Py<PyAny>) -> PyResult<()> {
        let value = python_to_json(py, &instance)?;
        self.inner.append(tag, value);
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
    ///
    /// This is bulk ``__setitem__`` and obeys the same whole-slot rule: a key that
    /// held several extension instances keeps only the value given here, and warns.
    #[pyo3(name = "update")]
    fn py_update(&self, py: Python<'_>, mapping: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut entries = HashMap::new();
        for (k, v) in mapping.iter() {
            let key: String = k.str()?.extract()?;
            let json_val = python_to_json(py, &v.unbind())?;
            entries.insert(key, json_val);
        }
        for (key, discarded) in self.inner.update(entries) {
            warn_instances_discarded(py, &key, discarded)?;
        }
        Ok(())
    }

    /// Remove all key-value pairs.
    #[pyo3(name = "clear")]
    fn py_clear(&self) {
        self.inner.clear();
    }
}
