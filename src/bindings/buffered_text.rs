//! Python bindings for BufferedTextAssetProvider.
//!
//! This module provides Python bindings for creating text segments in memory.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::IntoPyObjectExt;

use crate::bindings::PyMetadataProvider;
use crate::buffered::BufferedTextAssetProvider;
use crate::traits::{AssetProvider, TextAssetProvider};
use crate::types::AssetType;

/// Constructs text assets entirely in memory.
///
/// Use ``BufferedTextAssetProvider`` to create text content for inclusion in a
/// dataset — mission reports, annotations, processing notes, and similar
/// human-readable data. The provider implements the same interface as
/// :class:`TextAssetProvider`, so in-memory text assets can be passed to any
/// API that accepts a text asset, including :class:`DatasetWriter`.
///
/// Supported character encodings are ``"UTF-8"``, ``"ASCII"``, ``"ECS"``,
/// and ``"MTF"``. You can optionally attach a title, description, and
/// semantic roles to describe the text asset's purpose within the dataset.
///
/// Example::
///
///     from aws.osml.io import BufferedTextAssetProvider
///
///     # Create a UTF-8 text asset
///     provider = BufferedTextAssetProvider.create(
///         key="text_0",
///         text_content="Hello, World!",
///         encoding="UTF-8",
///     )
///
///     # Access text content and metadata
///     print(provider.text)      # "Hello, World!"
///     print(provider.encoding)  # "UTF-8"
///     print(provider.format)    # "U8S"
///
///     # Create a text asset with title, description, and roles
///     provider = BufferedTextAssetProvider.create(
///         key="text_segment_0",
///         text_content="Mission report content...",
///         encoding="UTF-8",
///         title="Mission Report",
///         description="Operational text",
///         roles=["data", "annotation"],
///     )
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
    /// Create a new in-memory text asset with the specified content and encoding.
    ///
    /// :param key: Unique identifier for this asset.
    /// :type key: str
    /// :param text_content: The text content as a string.
    /// :type text_content: str
    /// :param encoding: Character encoding. Supported values are ``"UTF-8"``,
    ///     ``"ASCII"``, ``"ECS"``, and ``"MTF"``. Defaults to ``"UTF-8"``.
    /// :type encoding: str, optional
    /// :param title: Human-readable title for the asset.
    /// :type title: str, optional
    /// :param description: Detailed description of the asset.
    /// :type description: str, optional
    /// :param roles: Semantic roles describing the asset's purpose. Defaults
    ///     to ``["data"]`` if not specified.
    /// :type roles: list[str], optional
    /// :param metadata: Additional metadata to attach to the asset.
    /// :type metadata: MetadataProvider, optional
    /// :returns: A new in-memory text asset.
    /// :rtype: BufferedTextAssetProvider
    ///
    /// Example::
    ///
    ///     from aws.osml.io import BufferedTextAssetProvider
    ///
    ///     provider = BufferedTextAssetProvider.create(
    ///         key="text_0",
    ///         text_content="Hello, World!",
    ///         encoding="UTF-8",
    ///         title="Sample Text",
    ///         description="A sample text asset",
    ///     )
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

    /// Unique identifier for this asset within the dataset.
    #[getter]
    fn key(&self) -> &str {
        self.inner.key()
    }

    /// Human-readable title for the asset.
    #[getter]
    fn title(&self) -> &str {
        self.inner.title()
    }

    /// Detailed description of the asset.
    #[getter]
    fn description(&self) -> &str {
        self.inner.description()
    }

    /// MIME type of the asset content.
    #[getter]
    fn media_type(&self) -> &str {
        self.inner.media_type()
    }

    /// Semantic roles for this asset.
    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles().to_vec()
    }

    /// Asset category.
    #[getter]
    fn asset_type(&self) -> AssetType {
        self.inner.asset_type()
    }

    /// Raw asset bytes as a ``BytesIO`` object.
    ///
    /// The raw bytes have CR/LF line delimiters as required by NITF.
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new(py, &bytes);

        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Asset-level metadata as a :class:`MetadataProvider`.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    // ========== TextAssetProvider properties ==========

    /// The decoded text content as a string.
    #[getter]
    fn text(&self) -> PyResult<String> {
        Ok(self.inner.text()?)
    }

    /// The character encoding of the text content (e.g., ``"UTF-8"``, ``"ASCII"``).
    #[getter]
    fn encoding(&self) -> &str {
        self.inner.encoding()
    }

    /// The text format identifier (e.g., ``"U8S"``, ``"STA"``).
    #[getter]
    fn format(&self) -> &str {
        self.inner.format()
    }
}
