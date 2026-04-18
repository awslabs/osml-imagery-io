//! Python bindings for DataAssetProvider.
//!
//! This module provides the PyDataAssetProvider wrapper that exposes the
//! DataAssetProvider trait to Python.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::IntoPyObjectExt;

use crate::bindings::PyMetadataProvider;
use crate::traits::DataAssetProvider;
use crate::types::AssetType;

/// Provides access to structured data stored within a geospatial dataset.
///
/// Geospatial datasets can embed structured payloads alongside imagery —
/// XML metadata (such as SICD/SIDD), JSON configuration, overflow TREs,
/// and application-specific data. ``DataAssetProvider`` exposes the raw
/// bytes through :meth:`get_raw_asset` and offers convenience methods to
/// parse the content directly as XML (:meth:`parse_as_xml`) or JSON
/// (:meth:`parse_as_json`). The :attr:`mime_type` property indicates the
/// content format. Use :meth:`DatasetReader.get_asset` to obtain an
/// instance for a specific data asset in the dataset.
///
/// Example:
///
/// ```python
/// from aws.osml.io import IO
///
/// with IO.open(["sicd_image.ntf"], "r") as dataset:
///     for key in dataset.get_asset_keys(asset_type="data"):
///         data = dataset.get_asset(key)
///         print(f"Data '{key}': mime_type={data.mime_type}")
///
///         if data.mime_type == "application/xml":
///             xml_tree = data.parse_as_xml()
///             print(f"XML root tag: {xml_tree.tag}")
///         elif data.mime_type == "application/json":
///             obj = data.parse_as_json()
///             print(f"JSON keys: {list(obj.keys())}")
/// ```
#[pyclass(name = "DataAssetProvider")]
pub struct PyDataAssetProvider {
    inner: Arc<dyn DataAssetProvider>,
}

impl PyDataAssetProvider {
    /// Creates a new PyDataAssetProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn DataAssetProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner DataAssetProvider.
    pub fn inner(&self) -> &Arc<dyn DataAssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyDataAssetProvider {
    // AssetProvider methods

    /// The unique identifier for this asset within the dataset.
    #[getter]
    fn key(&self) -> &str {
        self.inner.key()
    }

    /// A human-readable title for the asset.
    #[getter]
    fn title(&self) -> &str {
        self.inner.title()
    }

    /// A detailed description of the asset.
    #[getter]
    fn description(&self) -> &str {
        self.inner.description()
    }

    /// The MIME type of the asset content.
    #[getter]
    fn media_type(&self) -> &str {
        self.inner.media_type()
    }

    /// The semantic roles for this asset.
    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles().to_vec()
    }

    /// The asset category.
    #[getter]
    fn asset_type(&self) -> AssetType {
        AssetType::Data
    }

    /// The raw asset bytes as a ``BytesIO`` object.
    fn get_raw_asset<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        let bytes = self.inner.raw_asset()?;
        let py_bytes = PyBytes::new(py, &bytes);

        let io_module = py.import("io")?;
        let bytes_io_class = io_module.getattr("BytesIO")?;
        let bytes_io = bytes_io_class.call1((py_bytes,))?;

        Ok(bytes_io.into())
    }

    /// The asset-level :class:`MetadataProvider`.
    fn get_metadata(&self) -> PyMetadataProvider {
        PyMetadataProvider::new(self.inner.metadata())
    }

    // DataAssetProvider-specific methods

    /// The MIME type of the data content (e.g., ``"application/xml"``, ``"application/json"``).
    #[getter]
    fn mime_type(&self) -> &str {
        self.inner.mime_type()
    }

    /// Parse the data content as XML and return an ``ElementTree`` ``Element``.
    ///
    /// Decodes the raw bytes and parses them as XML using Python's
    /// ``xml.etree.ElementTree`` module. This is useful for reading
    /// SICD/SIDD metadata or other XML payloads embedded in the dataset.
    ///
    /// :returns: The root element of the parsed XML document.
    /// :rtype: xml.etree.ElementTree.Element
    /// :raises ValueError: If the content is not valid XML.
    ///
    /// Example:
    ///
    /// ```python
    /// data = dataset.get_asset(key)
    /// if data.mime_type == "application/xml":
    ///     root = data.parse_as_xml()
    ///     print(f"XML root tag: {root.tag}")
    ///     for child in root:
    ///         print(f"  {child.tag}")
    /// ```
    fn parse_as_xml(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let xml_string = self.inner.parse_as_xml()?;

        // Import xml.etree.ElementTree and parse the XML string
        let et_module = py.import("xml.etree.ElementTree")?;
        let fromstring = et_module.getattr("fromstring")?;
        let element = fromstring.call1((xml_string,))?;

        Ok(element.into())
    }

    /// Parse the data content as JSON and return a Python object.
    ///
    /// Decodes the raw bytes and parses them as JSON. The returned value
    /// is a native Python object — typically a ``dict`` for JSON objects
    /// or a ``list`` for JSON arrays.
    ///
    /// :returns: The parsed JSON content as a native Python object.
    /// :rtype: dict or list
    /// :raises ValueError: If the content is not valid JSON.
    ///
    /// Example:
    ///
    /// ```python
    /// data = dataset.get_asset(key)
    /// if data.mime_type == "application/json":
    ///     obj = data.parse_as_json()
    ///     print(f"Keys: {list(obj.keys())}")
    /// ```
    fn parse_as_json(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let json_value = self.inner.parse_as_json()?;
        serde_json_value_to_pyobject(py, &json_value)
    }
}

/// Converts a serde_json::Value to a Py<PyAny>.
fn serde_json_value_to_pyobject(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
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
                Ok(py.None())
            }
        }
        serde_json::Value::String(s) => Ok(s.into_pyobject(py)?.into_any().unbind()),
        serde_json::Value::Array(arr) => {
            let py_list = pyo3::types::PyList::empty(py);
            for item in arr {
                py_list.append(serde_json_value_to_pyobject(py, item)?)?;
            }
            Ok(py_list.into())
        }
        serde_json::Value::Object(obj) => {
            let py_dict = pyo3::types::PyDict::new(py);
            for (k, v) in obj {
                py_dict.set_item(k, serde_json_value_to_pyobject(py, v)?)?;
            }
            Ok(py_dict.into())
        }
    }
}
