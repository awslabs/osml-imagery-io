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
fn python_to_json(py: Python<'_>, obj: &PyObject) -> PyResult<serde_json::Value> {
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
            arr.push(python_to_json(py, &item.into())?);
        }
        Ok(serde_json::Value::Array(arr))
    } else if let Ok(dict) = bound.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key: String = k.str()?.extract()?;
            map.insert(key, python_to_json(py, &v.into())?);
        }
        Ok(serde_json::Value::Object(map))
    } else {
        // Fallback: convert to string
        let s: String = bound.str()?.extract()?;
        Ok(serde_json::Value::String(s))
    }
}

/// Python wrapper for BufferedMetadataProvider.
///
/// This class extends MetadataProvider and provides a mutable metadata provider 
/// for setting encoding hints and other metadata values programmatically.
///
/// Since BufferedMetadataProvider extends MetadataProvider, it can be used anywhere
/// a MetadataProvider is expected.
///
/// # Example
///
/// ```python
/// from aws.osml.io import BufferedMetadataProvider
///
/// # Create empty provider
/// provider = BufferedMetadataProvider()
///
/// # Set encoding hints (lowercase field names match .ksy parser output)
/// provider.set("imode", "B")
/// provider.set("nppbh", "256")
/// provider.set("nppbv", "256")
///
/// # Get values
/// imode = provider.get("imode")  # Returns "B"
///
/// # Get all as dict (inherited from MetadataProvider)
/// metadata = provider.as_dict()  # Returns {"imode": "B", "nppbh": "256", "nppbv": "256"}
///
/// # Can be used anywhere MetadataProvider is expected
/// writer.set_metadata(provider)
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
    /// Create a new BufferedMetadataProvider.
    ///
    /// # Arguments
    ///
    /// * `source` - Optional existing MetadataProvider to copy from.
    ///   If provided, all key-value pairs from the source will be copied to the new provider.
    ///
    /// # Returns
    ///
    /// A new BufferedMetadataProvider instance.
    ///
    /// # Example
    ///
    /// ```python
    /// # Create empty provider
    /// provider = BufferedMetadataProvider()
    ///
    /// # Create from existing provider (copies all metadata)
    /// copied = BufferedMetadataProvider(source=existing_provider)
    /// ```
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

    /// Set a string value for the given key.
    ///
    /// If the key already exists, its value is replaced.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name.
    /// * `value` - The value to store.
    ///
    /// # Example
    ///
    /// ```python
    /// provider.set("IMODE", "B")
    /// provider.set("NPPBH", "256")
    /// ```
    fn set(&self, key: &str, value: &str) {
        self.inner.set(key, value);
    }

    /// Set a JSON value for the given key.
    ///
    /// Unlike [`set`], which always stores a string, this method accepts any
    /// JSON-compatible Python value (int, float, list, dict, bool, None, str)
    /// and preserves its type. This is needed for GeoTIFF encoding hints that
    /// require numeric or array values.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name.
    /// * `value` - A Python value to store as JSON (int, float, list, str, etc.).
    ///
    /// # Example
    ///
    /// ```python
    /// provider.set_json("GeoProjectedCRS", 32618)
    /// provider.set_json("GeoPixelScale", [0.5, 0.5, 0.0])
    /// ```
    fn set_json(&self, py: Python<'_>, key: &str, value: PyObject) -> PyResult<()> {
        let json_val = python_to_json(py, &value)?;
        self.inner.set_json(key, json_val);
        Ok(())
    }

    /// Get the value for the given key, if it exists.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name to retrieve.
    ///
    /// # Returns
    ///
    /// The value as a string if the key exists, or None if it doesn't.
    ///
    /// # Example
    ///
    /// ```python
    /// imode = provider.get("IMODE")  # Returns "B" or None
    /// ```
    fn get(&self, key: &str) -> Option<String> {
        self.inner.get(key)
    }

    /// Remove a key-value pair.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata field name to remove.
    ///
    /// # Returns
    ///
    /// The previous value if the key existed, or None if it didn't.
    ///
    /// # Example
    ///
    /// ```python
    /// old_value = provider.remove("IMODE")  # Returns previous value or None
    /// ```
    fn remove(&self, key: &str) -> Option<String> {
        self.inner.remove(key)
    }

    /// Clear all stored metadata.
    ///
    /// # Example
    ///
    /// ```python
    /// provider.clear()  # Removes all key-value pairs
    /// ```
    fn clear(&self) {
        self.inner.clear();
    }
}
