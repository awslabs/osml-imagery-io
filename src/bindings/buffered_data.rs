use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::data::serde_json_value_to_pyobject;
use crate::bindings::PyMetadataProvider;
use crate::buffered::BufferedDataAssetProvider;
use crate::traits::{AssetMetadata, DataAssetProvider};
use crate::types::AssetType;

/// Constructs data assets entirely in memory for DES segment writing.
///
/// Use ``BufferedDataAssetProvider`` to create structured data payloads —
/// XML metadata (SICD/SIDD), JSON, or arbitrary binary — for inclusion in a
/// dataset's Data Extension Segments (DES). The provider implements the same
/// interface as :class:`DataAssetProvider`, so in-memory data assets can be
/// passed to any API that accepts a data asset, including
/// :class:`DatasetWriter`.
///
/// To set DES subheader fields (DESID, DESVER, etc.), attach a
/// :class:`BufferedMetadataProvider` via the ``metadata`` parameter.
///
/// Example:
///
/// ```python
/// from aws.osml.io import BufferedDataAssetProvider, BufferedMetadataProvider
///
/// meta = BufferedMetadataProvider()
/// meta.set("DESID", "XML_DATA_CONTENT")
/// meta.set("DESVER", "01")
///
/// provider = BufferedDataAssetProvider.create(
///     key="des:0",
///     data=b"<SICD>...</SICD>",
///     mime_type="application/xml",
///     title="SICD Metadata",
///     roles=["metadata"],
///     metadata=meta,
/// )
/// ```
#[pyclass(name = "BufferedDataAssetProvider")]
pub struct PyBufferedDataAssetProvider {
    inner: Arc<BufferedDataAssetProvider>,
}

impl PyBufferedDataAssetProvider {
    pub fn inner(&self) -> &Arc<BufferedDataAssetProvider> {
        &self.inner
    }

    pub fn as_data_provider(&self) -> Arc<dyn DataAssetProvider> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyBufferedDataAssetProvider {
    /// Create a new in-memory data asset with the specified content and MIME type.
    ///
    /// :param key: Unique identifier for this asset.
    /// :type key: str
    /// :param data: The raw data payload as bytes.
    /// :type data: bytes
    /// :param mime_type: MIME type of the data content. Defaults to
    ///     ``"application/octet-stream"``.
    /// :type mime_type: str, optional
    /// :param title: Human-readable title for the asset.
    /// :type title: str, optional
    /// :param description: Detailed description of the asset.
    /// :type description: str, optional
    /// :param roles: Semantic roles describing the asset's purpose. Defaults
    ///     to ``["data"]`` if not specified.
    /// :type roles: list[str], optional
    /// :param metadata: Additional metadata to attach (e.g., DESID, DESVER).
    /// :type metadata: MetadataProvider, optional
    /// :returns: A new in-memory data asset.
    /// :rtype: BufferedDataAssetProvider
    #[staticmethod]
    #[pyo3(signature = (
        key,
        data,
        mime_type="application/octet-stream",
        title=None,
        description=None,
        roles=None,
        metadata=None
    ))]
    fn create(
        key: &str,
        data: &[u8],
        mime_type: &str,
        title: Option<&str>,
        description: Option<&str>,
        roles: Option<Vec<String>>,
        metadata: Option<&PyMetadataProvider>,
    ) -> Self {
        let mut provider = BufferedDataAssetProvider::new(key, data.to_vec(), mime_type);

        if let Some(t) = title {
            provider = provider.with_title(t);
        }

        if let Some(d) = description {
            provider = provider.with_description(d);
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

    // ========== AssetMetadata properties ==========

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
        AssetType::Data
    }

    /// Raw asset bytes as a ``BytesIO`` object.
    #[getter]
    fn raw_asset<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new(py, &bytes);

        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// Asset-level metadata as a :class:`MetadataProvider`.
    #[getter]
    fn metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    // ========== DataAssetProvider properties ==========

    /// The MIME type of the data content.
    #[getter]
    fn mime_type(&self) -> &str {
        self.inner.mime_type()
    }

    /// Parse the data content as XML and return an ``ElementTree`` ``Element``.
    ///
    /// :returns: The root element of the parsed XML document.
    /// :rtype: xml.etree.ElementTree.Element
    /// :raises ValueError: If the content is not valid XML.
    fn parse_as_xml(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let xml_string = self.inner.parse_as_xml()?;

        let et_module = py.import("xml.etree.ElementTree")?;
        let fromstring = et_module.getattr("fromstring")?;
        let element = fromstring.call1((xml_string,))?;

        Ok(element.into())
    }

    /// Parse the data content as JSON and return a Python object.
    ///
    /// :returns: The parsed JSON content as a native Python object.
    /// :rtype: dict or list
    /// :raises ValueError: If the content is not valid JSON.
    fn parse_as_json(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let json_value = self.inner.parse_as_json()?;
        serde_json_value_to_pyobject(py, &json_value)
    }
}
