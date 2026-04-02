//! Python bindings for BufferedMetadataProvider.
//!
//! This module provides the PyBufferedMetadataProvider wrapper that exposes the
//! BufferedMetadataProvider to Python, allowing programmatic setting of metadata
//! values for encoding hints.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};

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
    } else if let Ok(b) = bound.downcast::<PyBool>() {
        Ok(serde_json::Value::Bool(b.is_true()))
    } else if let Ok(i) = bound.downcast::<PyInt>() {
        let val: i64 = i.extract()?;
        Ok(serde_json::json!(val))
    } else if let Ok(f) = bound.downcast::<PyFloat>() {
        let val: f64 = f.extract()?;
        // Use from_f64 to preserve float representation. serde_json::json!()
        // would coerce whole-number floats like 1.0 to integer, causing
        // downstream type inference to pick TIFF_LONG instead of TIFF_DOUBLE.
        match serde_json::Number::from_f64(val) {
            Some(n) => Ok(serde_json::Value::Number(n)),
            None => {
                // NaN / Infinity cannot be represented in JSON
                Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Cannot convert float {} to JSON",
                    val
                )))
            }
        }
    } else if let Ok(s) = bound.downcast::<PyString>() {
        let val: String = s.extract()?;
        Ok(serde_json::Value::String(val))
    } else if let Ok(list) = bound.downcast::<PyList>() {
        let mut arr = Vec::new();
        for item in list.iter() {
            arr.push(python_to_json(py, &item.unbind())?);
        }
        Ok(serde_json::Value::Array(arr))
    } else if let Ok(dict) = bound.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key: String = k.str()?.extract()?;
            map.insert(key, python_to_json(py, &v.unbind())?);
        }
        Ok(serde_json::Value::Object(map))
    } else {
        // Fallback: convert to string
        let s: String = bound.str()?.extract()?;
        Ok(serde_json::Value::String(s))
    }
}

/// A mutable metadata store for setting encoding hints and format fields.
///
/// ``BufferedMetadataProvider`` is a subclass of :class:`MetadataProvider` that
/// adds write operations (``set``, ``set_json``, ``get``, ``remove``, ``clear``)
/// on top of the read-only :meth:`MetadataProvider.as_dict` interface. Use it to
/// supply encoding hints — compression type, interleave mode, block dimensions,
/// GeoTIFF projection parameters — when constructing images for writing. Because
/// it inherits from :class:`MetadataProvider`, a ``BufferedMetadataProvider`` can
/// be passed anywhere a :class:`MetadataProvider` is expected.
///
/// String values are stored with ``set`` and typed values (int, float, list, dict)
/// are stored with ``set_json``. Use ``set`` for NITF header fields that are
/// always ASCII strings, and ``set_json`` for GeoTIFF tags that require numeric
/// or array values.
///
/// Example::
///
///     from aws.osml.io import BufferedMetadataProvider
///
///     # Create an empty provider and populate encoding hints
///     metadata = BufferedMetadataProvider()
///     metadata.set("IC", "NC")
///     metadata.set("IMODE", "B")
///     metadata.set("NPPBH", "256")
///     metadata.set("NPPBV", "256")
///
///     # Retrieve a value
///     imode = metadata.get("IMODE")  # "B"
///
///     # View all entries as a dict (inherited from MetadataProvider)
///     all_fields = metadata.as_dict()
///
///     # Use set_json for GeoTIFF tags that need typed values
///     metadata.set_json("33550", [0.5, 0.5, 0.0])  # ModelPixelScale
///     metadata.set_json("GeoProjectedCRS", 32618)
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
    /// :returns: A new ``BufferedMetadataProvider`` instance.
    /// :rtype: BufferedMetadataProvider
    ///
    /// Example::
    ///
    ///     # Create an empty provider
    ///     provider = BufferedMetadataProvider()
    ///
    ///     # Create from an existing provider (copies all metadata)
    ///     copied = BufferedMetadataProvider(source=existing_provider)
    #[new]
    #[pyo3(signature = (source=None))]
    fn py_new(source: Option<PyRef<'_, PyMetadataProvider>>) -> (Self, PyMetadataProvider) {
        let simple = match source {
            Some(src) => BufferedMetadataProvider::from_provider(src.inner().as_ref()),
            None => BufferedMetadataProvider::new(),
        };
        let inner = Arc::new(simple);
        
        // Create the base class with the same Arc (as dyn MetadataProvider)
        let base = PyMetadataProvider::new(inner.clone() as Arc<dyn MetadataProvider>);
        
        (Self { inner }, base)
    }

    /// Store a string value for the given key.
    ///
    /// If the key already exists, its value is replaced. Use this method for
    /// NITF header fields and other metadata stored as plain strings. For
    /// values that need to preserve a numeric or structured type, use
    /// :meth:`set_json` instead.
    ///
    /// :param key: The metadata field name.
    /// :type key: str
    /// :param value: The string value to store.
    /// :type value: str
    ///
    /// Example::
    ///
    ///     metadata = BufferedMetadataProvider()
    ///     metadata.set("IC", "NC")
    ///     metadata.set("IMODE", "B")
    ///     metadata.set("NPPBH", "256")
    fn set(&self, key: &str, value: &str) {
        self.inner.set(key, value);
    }

    /// Store a typed value for the given key.
    ///
    /// Unlike :meth:`set`, which always stores a string, this method accepts
    /// any JSON-compatible Python value (int, float, list, dict, bool, None,
    /// str) and preserves its type. Use ``set_json`` for GeoTIFF encoding
    /// hints that require numeric or array values — for example, pixel scale
    /// or projection parameters.
    ///
    /// :param key: The metadata field name.
    /// :type key: str
    /// :param value: A Python value to store (int, float, list, dict, str,
    ///     bool, or None).
    /// :type value: object
    /// :raises ValueError: If the value cannot be represented as JSON
    ///     (e.g. ``float('nan')``).
    ///
    /// Example::
    ///
    ///     metadata = BufferedMetadataProvider()
    ///     metadata.set_json("GeoProjectedCRS", 32618)
    ///     metadata.set_json("33550", [0.5, 0.5, 0.0])  # ModelPixelScale
    fn set_json(&self, py: Python<'_>, key: &str, value: Py<PyAny>) -> PyResult<()> {
        let json_val = python_to_json(py, &value)?;
        self.inner.set_json(key, json_val);
        Ok(())
    }

    /// Retrieve the value for the given key, if it exists.
    ///
    /// :param key: The metadata field name to look up.
    /// :type key: str
    /// :returns: The value as a string, or ``None`` if the key is not present.
    /// :rtype: str or None
    ///
    /// Example::
    ///
    ///     imode = metadata.get("IMODE")  # "B" or None
    fn get(&self, key: &str) -> Option<String> {
        self.inner.get(key)
    }

    /// Remove a key-value pair from the store.
    ///
    /// :param key: The metadata field name to remove.
    /// :type key: str
    /// :returns: The previous value if the key existed, or ``None`` otherwise.
    /// :rtype: str or None
    ///
    /// Example::
    ///
    ///     old_value = metadata.remove("IMODE")  # previous value or None
    fn remove(&self, key: &str) -> Option<String> {
        self.inner.remove(key)
    }

    /// Remove all key-value pairs from the store.
    ///
    /// Example::
    ///
    ///     metadata.clear()
    fn clear(&self) {
        self.inner.clear();
    }
}
