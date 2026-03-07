//! Python bindings for BufferedTextAssetProvider.
//!
//! This module provides Python bindings for creating text segments in memory.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::buffered::BufferedTextAssetProvider;
use crate::traits::{AssetProvider, TextAssetProvider};
use crate::types::AssetType;

/// Python wrapper for BufferedTextAssetProvider.
///
/// This class allows creating text segments in memory with configurable
/// content and encoding.
///
/// # Example
///
/// ```python
/// from aws.osml.io import BufferedTextAssetProvider
///
/// # Create a UTF-8 text segment
/// provider = BufferedTextAssetProvider.create(
///     key="text_0",
///     text_content="Hello, World!",
///     encoding="UTF-8",
/// )
///
/// # Access text content
/// print(provider.text)  # "Hello, World!"
/// print(provider.encoding)  # "UTF-8"
/// print(provider.format)  # "U8S"
///
/// # Set optional properties
/// provider = BufferedTextAssetProvider.create(
///     key="text_1",
///     text_content="Sample text",
///     encoding="ASCII",
///     title="My Text Segment",
///     description="A sample text segment",
///     roles=["annotation", "metadata"],
/// )
/// ```
#[pyclass(name = "BufferedTextAssetProvider")]
pub struct PyBufferedTextAssetProvider {
    inner: Arc<BufferedTextAssetProvider>,
}

impl PyBufferedTextAssetProvider {
    /// Returns a reference to the inner provider.
    pub fn inner(&self) -> &Arc<BufferedTextAssetProvider> {
        &self.inner
    }

    /// Returns the inner provider as an Arc<dyn TextAssetProvider>.
    pub fn as_text_provider(&self) -> Arc<dyn TextAssetProvider> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyBufferedTextAssetProvider {
    /// Create a new BufferedTextAssetProvider with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `key` - Unique identifier for this asset
    /// * `text_content` - The text content as a string
    /// * `encoding` - Character encoding: "ASCII", "UTF-8", "ECS", or "MTF" (default: "UTF-8")
    /// * `title` - Human-readable title (optional)
    /// * `description` - Detailed description (optional)
    /// * `roles` - Semantic roles (optional, defaults to ["data"])
    /// * `metadata` - Optional MetadataProvider for additional metadata
    ///
    /// # Returns
    ///
    /// A new BufferedTextAssetProvider instance.
    ///
    /// # Example
    ///
    /// ```python
    /// from aws.osml.io import BufferedTextAssetProvider
    ///
    /// provider = BufferedTextAssetProvider.create(
    ///     key="text_0",
    ///     text_content="Hello, World!",
    ///     encoding="UTF-8",
    ///     title="Sample Text",
    ///     description="A sample text segment",
    /// )
    /// ```
    #[staticmethod]
    #[pyo3(signature = (
        key,
        text_content,
        encoding="UTF-8",
        title=None,
        description=None,
        roles=None,
        metadata=None
    ))]
    fn create(
        key: &str,
        text_content: &str,
        encoding: &str,
        title: Option<&str>,
        description: Option<&str>,
        roles: Option<Vec<String>>,
        metadata: Option<&PyMetadataProvider>,
    ) -> Self {
        let mut provider =
            BufferedTextAssetProvider::new(key, text_content.to_string(), encoding);

        // Apply optional properties
        if let Some(t) = title {
            provider = provider.with_title(t.to_string());
        }

        if let Some(d) = description {
            provider = provider.with_description(d.to_string());
        }

        if let Some(r) = roles {
            provider = provider.with_roles(r);
        }

        if let Some(meta) = metadata {
            provider = provider.with_metadata(meta.inner().clone());
        }

        Self {
            inner: Arc::new(provider),
        }
    }

    // ========== AssetProvider properties ==========

    /// Returns the unique identifier for this asset.
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
    ///
    /// The raw bytes have CR/LF line delimiters as required by NITF.
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

    // ========== TextAssetProvider properties ==========

    /// Returns the decoded text content as a string.
    #[getter]
    fn text(&self) -> PyResult<String> {
        Ok(self.inner.text()?)
    }

    /// Returns the character encoding (e.g., "UTF-8", "ASCII").
    #[getter]
    fn encoding(&self) -> &str {
        self.inner.encoding()
    }

    /// Returns the text format identifier (e.g., "U8S", "STA").
    #[getter]
    fn format(&self) -> &str {
        self.inner.format()
    }
}
