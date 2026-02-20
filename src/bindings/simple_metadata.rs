//! Python bindings for SimpleMetadataProvider.
//!
//! This module provides the PySimpleMetadataProvider wrapper that exposes the
//! SimpleMetadataProvider to Python, allowing programmatic setting of metadata
//! values for encoding hints.

use std::sync::Arc;

use pyo3::prelude::*;

use crate::bindings::PyMetadataProvider;
use crate::simple_metadata::SimpleMetadataProvider;
use crate::traits::MetadataProvider;

/// Python wrapper for SimpleMetadataProvider.
///
/// This class extends MetadataProvider and provides a mutable metadata provider 
/// for setting encoding hints and other metadata values programmatically.
///
/// Since SimpleMetadataProvider extends MetadataProvider, it can be used anywhere
/// a MetadataProvider is expected.
///
/// # Example
///
/// ```python
/// from aws.osml.io import SimpleMetadataProvider
///
/// # Create empty provider
/// provider = SimpleMetadataProvider()
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
#[pyclass(name = "SimpleMetadataProvider", extends = PyMetadataProvider)]
pub struct PySimpleMetadataProvider {
    inner: Arc<SimpleMetadataProvider>,
}

impl PySimpleMetadataProvider {
    /// Returns a reference to the inner SimpleMetadataProvider.
    pub fn inner(&self) -> &Arc<SimpleMetadataProvider> {
        &self.inner
    }

    /// Returns the inner provider as an Arc<dyn MetadataProvider>.
    pub fn as_metadata_provider(&self) -> Arc<dyn MetadataProvider> {
        self.inner.clone()
    }
}

#[pymethods]
impl PySimpleMetadataProvider {
    /// Create a new SimpleMetadataProvider.
    ///
    /// # Arguments
    ///
    /// * `source` - Optional existing MetadataProvider to copy from.
    ///   If provided, all key-value pairs from the source will be copied to the new provider.
    ///
    /// # Returns
    ///
    /// A new SimpleMetadataProvider instance.
    ///
    /// # Example
    ///
    /// ```python
    /// # Create empty provider
    /// provider = SimpleMetadataProvider()
    ///
    /// # Create from existing provider (copies all metadata)
    /// copied = SimpleMetadataProvider(source=existing_provider)
    /// ```
    #[new]
    #[pyo3(signature = (source=None))]
    fn py_new(source: Option<PyRef<'_, PyMetadataProvider>>) -> (Self, PyMetadataProvider) {
        let simple = match source {
            Some(src) => SimpleMetadataProvider::from_provider(src.inner().as_ref()),
            None => SimpleMetadataProvider::new(),
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
