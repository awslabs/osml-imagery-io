//! Data-driven binary structure parsing and encoding.
//!
//! This module provides classes for reading and writing binary data according
//! to declarative YAML-based structure definitions (``.ksy`` files). Use
//! :class:`StructureRegistry` to load definitions, :class:`StructureAccessor`
//! to read fields from binary data, :class:`StructureWriter` to encode values,
//! and :class:`Value` to interpret parsed field values.

use std::sync::Arc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};

use crate::parser::writer::WriteValue;
use crate::parser::{
    AccessError, ConversionError, LoadError, StructureAccessor, StructureDefinition,
    StructureRegistry, StructureWriter, Value, WriteError,
};

// ==================== Error Conversion ====================

impl From<LoadError> for PyErr {
    fn from(err: LoadError) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}

impl From<AccessError> for PyErr {
    fn from(err: AccessError) -> PyErr {
        match &err {
            AccessError::UnknownField { path } => PyKeyError::new_err(path.clone()),
            AccessError::ConditionalNotPresent { path, .. } => PyKeyError::new_err(path.clone()),
            _ => PyRuntimeError::new_err(err.to_string()),
        }
    }
}

impl From<ConversionError> for PyErr {
    fn from(err: ConversionError) -> PyErr {
        PyTypeError::new_err(err.to_string())
    }
}

impl From<WriteError> for PyErr {
    fn from(err: WriteError) -> PyErr {
        match &err {
            WriteError::MissingRequired { path } => {
                PyValueError::new_err(format!("Missing required field: {}", path))
            }
            WriteError::ValueTooLarge {
                path,
                max_size,
                actual_size,
            } => PyValueError::new_err(format!(
                "Value too large for '{}': max {} bytes, got {}",
                path, max_size, actual_size
            )),
            WriteError::OutOfOrder {
                path,
                expected_after,
            } => PyValueError::new_err(format!(
                "Field '{}' written out of order (expected after '{}')",
                path, expected_after
            )),
            _ => PyRuntimeError::new_err(err.to_string()),
        }
    }
}

// ==================== PyValue ====================

/// Represents a parsed field value from binary structure data.
///
/// A ``Value`` is returned by :class:`StructureAccessor` when you access a
/// field. It holds the raw parsed content and provides type conversion methods
/// — ``as_str()``, ``as_int()``, ``as_float()``, and ``as_bytes()`` — for
/// interpreting ASCII-encoded fields as native Python types. Most binary
/// format fields (e.g. NITF header fields) are stored as fixed-width ASCII
/// strings, so these converters handle trimming and numeric parsing
/// automatically.
///
/// Example:
///
/// ```python
/// value = accessor["NROWS"]
/// num_rows = value.as_int()
/// print(value.as_str())
/// ```
#[pyclass(name = "Value")]
pub struct PyValue {
    /// The underlying value (owned for Python lifetime management)
    inner: OwnedValue,
}

/// Owned version of Value for Python bindings.
#[derive(Debug, Clone)]
enum OwnedValue {
    String(String),
    Bytes(Vec<u8>),
    Unsigned(u64),
    Array(Vec<OwnedValue>),
    Struct { data: Vec<u8>, type_name: String },
}

impl<'a> From<Value<'a>> for OwnedValue {
    fn from(value: Value<'a>) -> Self {
        match value {
            Value::String(cow) => OwnedValue::String(cow.into_owned()),
            Value::Bytes(bytes) => OwnedValue::Bytes(bytes.to_vec()),
            Value::Unsigned(n) => OwnedValue::Unsigned(n),
            Value::Array(arr) => OwnedValue::Array(arr.into_iter().map(OwnedValue::from).collect()),
            Value::Struct(s) => OwnedValue::Struct {
                data: s.data.to_vec(),
                type_name: s.type_name.clone(),
            },
        }
    }
}

impl PyValue {
    /// Create a new PyValue from a parser Value.
    pub fn from_value(value: Value<'_>) -> Self {
        Self {
            inner: OwnedValue::from(value),
        }
    }
}

#[pymethods]
impl PyValue {
    /// Return the value as a string, trimming trailing padding.
    ///
    /// :returns: The string value with trailing spaces removed.
    /// :rtype: str
    /// :raises TypeError: If the value cannot be converted to a string.
    fn as_str(&self) -> PyResult<String> {
        match &self.inner {
            OwnedValue::String(s) => Ok(s.trim_end_matches(' ').to_string()),
            OwnedValue::Bytes(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|e| PyTypeError::new_err(e.to_string()))?;
                Ok(s.trim_end_matches(' ').to_string())
            }
            OwnedValue::Unsigned(n) => Ok(n.to_string()),
            _ => Err(PyTypeError::new_err("Cannot convert to string")),
        }
    }

    /// Parse the value as a signed integer.
    ///
    /// Blank or whitespace-only strings are parsed as ``0``.
    ///
    /// :returns: The parsed integer value.
    /// :rtype: int
    /// :raises TypeError: If the value cannot be converted to an integer.
    fn as_int(&self) -> PyResult<i64> {
        match &self.inner {
            OwnedValue::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed.parse::<i64>().map_err(|e| {
                    PyTypeError::new_err(format!("Cannot parse '{}' as int: {}", s, e))
                })
            }
            OwnedValue::Bytes(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|e| PyTypeError::new_err(e.to_string()))?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed
                    .parse::<i64>()
                    .map_err(|e| PyTypeError::new_err(format!("Cannot parse as int: {}", e)))
            }
            OwnedValue::Unsigned(n) => {
                if *n <= i64::MAX as u64 {
                    Ok(*n as i64)
                } else {
                    Err(PyTypeError::new_err("Value exceeds i64::MAX"))
                }
            }
            _ => Err(PyTypeError::new_err("Cannot convert to int")),
        }
    }

    /// Parse the value as a floating-point number.
    ///
    /// Blank or whitespace-only strings are parsed as ``0.0``.
    ///
    /// :returns: The parsed floating-point value.
    /// :rtype: float
    /// :raises TypeError: If the value cannot be converted to a float.
    fn as_float(&self) -> PyResult<f64> {
        match &self.inner {
            OwnedValue::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0.0);
                }
                trimmed.parse::<f64>().map_err(|e| {
                    PyTypeError::new_err(format!("Cannot parse '{}' as float: {}", s, e))
                })
            }
            OwnedValue::Bytes(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|e| PyTypeError::new_err(e.to_string()))?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0.0);
                }
                trimmed
                    .parse::<f64>()
                    .map_err(|e| PyTypeError::new_err(format!("Cannot parse as float: {}", e)))
            }
            OwnedValue::Unsigned(n) => Ok(*n as f64),
            _ => Err(PyTypeError::new_err("Cannot convert to float")),
        }
    }

    /// Return the raw bytes of the value.
    ///
    /// :returns: The raw byte representation.
    /// :rtype: bytes
    fn as_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        match &self.inner {
            OwnedValue::String(s) => Ok(PyBytes::new(py, s.as_bytes())),
            OwnedValue::Bytes(bytes) => Ok(PyBytes::new(py, bytes)),
            OwnedValue::Unsigned(n) => Ok(PyBytes::new(py, &n.to_be_bytes())),
            OwnedValue::Struct { data, .. } => Ok(PyBytes::new(py, data)),
            OwnedValue::Array(_) => Err(PyTypeError::new_err("Cannot get bytes from array")),
        }
    }

    /// String representation of the value.
    fn __repr__(&self) -> String {
        match &self.inner {
            OwnedValue::String(s) => format!("Value('{}')", s.trim_end_matches(' ')),
            OwnedValue::Bytes(bytes) => format!("Value(<{} bytes>)", bytes.len()),
            OwnedValue::Unsigned(n) => format!("Value({})", n),
            OwnedValue::Array(arr) => format!("Value([{} items])", arr.len()),
            OwnedValue::Struct { type_name, .. } => format!("Value(<struct {}>)", type_name),
        }
    }

    /// Get the length of the value.
    fn __len__(&self) -> usize {
        match &self.inner {
            OwnedValue::String(s) => s.len(),
            OwnedValue::Bytes(bytes) => bytes.len(),
            OwnedValue::Unsigned(_) => 1,
            OwnedValue::Array(arr) => arr.len(),
            OwnedValue::Struct { data, .. } => data.len(),
        }
    }

    /// Return the elements of an array value as a list of :class:`Value` objects.
    ///
    /// :returns: List of ``Value`` objects, one per array element.
    /// :rtype: list[Value]
    /// :raises TypeError: If the value is not an array.
    fn as_array(&self) -> PyResult<Vec<PyValue>> {
        match &self.inner {
            OwnedValue::Array(arr) => Ok(arr
                .iter()
                .map(|elem| PyValue {
                    inner: elem.clone(),
                })
                .collect()),
            _ => Err(PyTypeError::new_err("Value is not an array")),
        }
    }
}

// ==================== PyStructureRegistry ====================

/// Manages loading, caching, and lookup of structure definitions.
///
/// The registry discovers ``.ksy`` structure definition files from one or more
/// search paths and makes them available by name. Built-in definitions for NITF
/// headers, image subheaders, and common TREs are loaded automatically. You can
/// extend the registry by adding custom search paths or registering definitions
/// at runtime. Use :meth:`get` to obtain a :class:`StructureDefinition` for use
/// with :class:`StructureAccessor` or :class:`StructureWriter`.
///
/// Example:
///
/// ```python
/// from aws.osml.io import StructureRegistry
///
/// registry = StructureRegistry()
/// registry.add_search_path("/path/to/my/structures")
/// definition = registry.get("TRE_GEOLOB")
/// for name in registry.list():
///     print(name)
/// ```
#[pyclass(name = "StructureRegistry")]
pub struct PyStructureRegistry {
    inner: StructureRegistry,
}

#[pymethods]
impl PyStructureRegistry {
    /// Create a new registry with default search paths.
    ///
    /// Default search paths include the package data directory
    /// (``data/structures/``) and any paths listed in the
    /// ``OSML_IO_STRUCTURE_PATH`` environment variable.
    #[new]
    fn new() -> Self {
        Self {
            inner: StructureRegistry::new(),
        }
    }

    /// Add a search path with higher priority than existing paths.
    ///
    /// :param path: Directory path to add to the search paths.
    /// :type path: str
    fn add_search_path(&mut self, path: &str) {
        self.inner.add_search_path(path);
    }

    /// Retrieve a structure definition by name.
    ///
    /// :param name: The structure name (e.g., ``"NITF_02.10_FileHeader"``,
    ///     ``"TRE_GEOLOB"``).
    /// :type name: str
    /// :returns: The :class:`StructureDefinition`, or ``None`` if not found.
    /// :rtype: StructureDefinition or None
    fn get(&mut self, name: &str) -> Option<PyStructureDefinition> {
        self.inner.get_mut(name).map(PyStructureDefinition::new)
    }

    /// List all available structure definition names.
    ///
    /// :returns: Names that can be passed to :meth:`get`.
    /// :rtype: list[str]
    fn list(&self) -> Vec<String> {
        self.inner.list()
    }

    /// Reload all definitions from disk.
    ///
    /// Clears the file cache and re-scans search paths. Definitions
    /// registered at runtime via :meth:`register` are preserved.
    ///
    /// :raises RuntimeError: If a search path cannot be read.
    fn reload(&mut self) -> PyResult<()> {
        self.inner
            .reload()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Register a definition at runtime with highest priority.
    ///
    /// Runtime-registered definitions take priority over file-based
    /// definitions with the same name.
    ///
    /// :param name: The name to register the definition under.
    /// :type name: str
    /// :param definition: The :class:`StructureDefinition` to register.
    /// :type definition: StructureDefinition
    fn register(&mut self, name: &str, definition: &PyStructureDefinition) {
        // Clone the definition from the Arc
        let def = (*definition.inner).clone();
        self.inner.register(name, def);
    }

    /// Return the current search paths.
    ///
    /// :returns: Directory paths being searched for ``.ksy`` files.
    /// :rtype: list[str]
    fn search_paths(&self) -> Vec<String> {
        self.inner
            .search_paths()
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "StructureRegistry(paths={}, definitions={})",
            self.inner.search_paths().len(),
            self.inner.list().len()
        )
    }
}

// ==================== PyStructureDefinition ====================

/// A read-only structure definition parsed from a ``.ksy`` file.
///
/// Contains field names and layout information used by
/// :class:`StructureAccessor` and :class:`StructureWriter` to read and write
/// binary data. Obtain instances from :class:`StructureRegistry` via
/// :meth:`StructureRegistry.get`.
///
/// Example:
///
/// ```python
/// definition = registry.get("TRE_GEOLOB")
/// print(definition.id, definition.field_names)
/// ```
#[pyclass(name = "StructureDefinition")]
pub struct PyStructureDefinition {
    inner: Arc<StructureDefinition>,
}

impl PyStructureDefinition {
    pub fn new(inner: Arc<StructureDefinition>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStructureDefinition {
    /// The unique identifier for this structure (e.g., ``"TRE_GEOLOB"``).
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable title from the definition, or ``None`` if not set.
    #[getter]
    fn title(&self) -> Option<&str> {
        self.inner.title.as_deref()
    }

    /// Ordered list of field names defined in this structure.
    #[getter]
    fn field_names(&self) -> Vec<String> {
        self.inner.fields.iter().map(|f| f.id.clone()).collect()
    }

    /// Number of fields in this structure.
    fn __len__(&self) -> usize {
        self.inner.fields.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "StructureDefinition(id='{}', fields={})",
            self.inner.id,
            self.inner.fields.len()
        )
    }
}

// ==================== PyStructureAccessor ====================

/// Provides lazy, dict-like read access to fields in binary data.
///
/// A ``StructureAccessor`` parses fields on demand according to a
/// :class:`StructureDefinition`, caching computed offsets for efficiency.
/// Access fields with bracket notation (``accessor["field_name"]``), use
/// dot-notation paths for nested fields (``"parent.child"``), check for
/// conditional fields with :meth:`has` or the ``in`` operator, and iterate
/// over all accessible paths with :meth:`fields`. Each field access returns
/// a :class:`Value` object.
///
/// Example:
///
/// ```python
/// from aws.osml.io import StructureAccessor
///
/// accessor = StructureAccessor(definition, data)
/// value = accessor["field_name"]
/// if accessor.has("optional_field"):
///     print(accessor["optional_field"].as_str())
/// for path in accessor.fields():
///     print(path, accessor[path])
/// ```
#[pyclass(name = "StructureAccessor")]
pub struct PyStructureAccessor {
    /// The structure definition
    definition: Arc<StructureDefinition>,
    /// Owned copy of the data buffer
    data: Vec<u8>,
}

impl PyStructureAccessor {
    /// Create accessor from definition and data.
    pub fn new(definition: Arc<StructureDefinition>, data: Vec<u8>) -> Self {
        Self { definition, data }
    }

    /// Get the internal accessor for operations.
    fn get_accessor(&self) -> Result<StructureAccessor<'_>, AccessError> {
        StructureAccessor::new(Arc::clone(&self.definition), &self.data)
    }
}

#[pymethods]
impl PyStructureAccessor {
    /// Create a new accessor from a definition and data buffer.
    ///
    /// The data is copied into the accessor for safe access. For very large
    /// datasets, consider using memory-mapped files (``mmap``) as input.
    ///
    /// :param definition: The :class:`StructureDefinition` to use for parsing.
    /// :type definition: StructureDefinition
    /// :param data: The binary data to parse. Accepts ``bytes``, ``bytearray``,
    ///     ``memoryview``, or any object supporting the buffer protocol
    ///     (including ``mmap``).
    /// :type data: bytes-like
    #[new]
    fn py_new(definition: &PyStructureDefinition, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        // Extract bytes from various Python buffer types
        let bytes: Vec<u8> = if let Ok(bytes) = data.extract::<Vec<u8>>() {
            bytes
        } else if let Ok(bytes_obj) = data.cast::<PyBytes>() {
            bytes_obj.as_bytes().to_vec()
        } else if data.hasattr("tobytes")? {
            // Try tobytes() method (works with memoryview)
            let buffer = data.call_method0("tobytes")?;
            buffer.extract::<Vec<u8>>()?
        } else {
            // Try slicing with [:] (works with mmap and other buffer-like objects)
            let py = data.py();
            let slice = pyo3::types::PySlice::full(py);
            let sliced = data.get_item(&slice)?;
            sliced.extract::<Vec<u8>>()?
        };

        Ok(Self::new(Arc::clone(&definition.inner), bytes))
    }

    /// Access a field by path using bracket notation.
    ///
    /// Supports dot-notation paths for nested fields (e.g.,
    /// ``"parent.child"`` or ``"items_0.value"``).
    ///
    /// :param path: Field path to access.
    /// :type path: str
    /// :returns: The parsed field value.
    /// :rtype: Value
    /// :raises KeyError: If the field path does not exist.
    ///
    /// Example:
    ///
    /// ```python
    /// value = accessor["NROWS"]
    /// num_rows = value.as_int()
    /// nested = accessor["parent.child"]
    /// ```
    fn __getitem__(&self, path: &str) -> PyResult<PyValue> {
        let accessor = self.get_accessor()?;
        let value = accessor.get(path)?;
        Ok(PyValue::from_value(value))
    }

    /// Check if a field exists and is accessible.
    ///
    /// Returns ``False`` for conditional fields whose condition is not met.
    ///
    /// :param path: The field path to check.
    /// :type path: str
    /// :returns: ``True`` if the field exists and is accessible.
    /// :rtype: bool
    fn has(&self, path: &str) -> PyResult<bool> {
        let accessor = self.get_accessor()?;
        Ok(accessor.has(path))
    }

    /// Check if a field exists (supports the ``in`` operator).
    fn __contains__(&self, path: &str) -> PyResult<bool> {
        self.has(path)
    }

    /// List all accessible field paths.
    ///
    /// :returns: Field paths that can be passed to bracket notation.
    /// :rtype: list[str]
    fn fields(&self) -> PyResult<Vec<String>> {
        let accessor = self.get_accessor()?;
        Ok(accessor.fields().collect())
    }

    /// Return the raw bytes for a field without interpretation.
    ///
    /// Useful for passing binary data to external decoders.
    ///
    /// :param path: The field path.
    /// :type path: str
    /// :returns: The raw bytes for the field.
    /// :rtype: bytes
    /// :raises KeyError: If the field does not exist.
    /// :raises RuntimeError: If the field is not contiguous in memory.
    fn raw_view<'py>(&self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let accessor = self.get_accessor()?;
        let slice = accessor.raw_slice(path)?;
        Ok(PyBytes::new(py, slice))
    }

    /// The underlying binary data buffer.
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.data)
    }

    /// The :class:`StructureDefinition` used by this accessor.
    #[getter]
    fn definition(&self) -> PyStructureDefinition {
        PyStructureDefinition::new(Arc::clone(&self.definition))
    }

    fn __repr__(&self) -> PyResult<String> {
        let accessor = self.get_accessor()?;
        let field_count = accessor.fields().count();
        Ok(format!(
            "StructureAccessor(definition='{}', fields={}, data_len={})",
            self.definition.id,
            field_count,
            self.data.len()
        ))
    }

    fn __len__(&self) -> usize {
        self.data.len()
    }
}

// ==================== PyStructureWriter ====================

/// Encodes values into binary data according to a :class:`StructureDefinition`.
///
/// A ``StructureWriter`` serializes field values into the correct binary layout
/// defined by a ``.ksy`` structure definition. Fields must be written in
/// definition order. Call :meth:`finish` to retrieve the final encoded bytes.
/// Field values are set using bracket notation or the :meth:`set` method, and
/// accepted types include ``str``, ``int``, ``float``, and ``bytes``.
/// For repeated fields, write elements sequentially with indexed paths
/// (``field_0``, ``field_1``, ...).
///
/// Example:
///
/// ```python
/// from aws.osml.io import StructureWriter
///
/// writer = StructureWriter.new_streaming(definition)
/// writer["field1"] = "value1"
/// writer["field2"] = 42
/// data = writer.finish()
/// ```
#[pyclass(name = "StructureWriter")]
pub struct PyStructureWriter {
    inner: Option<StructureWriter>,
}

impl PyStructureWriter {
    fn get_inner(&self) -> PyResult<&StructureWriter> {
        self.inner
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Writer has been finalized"))
    }

    fn get_inner_mut(&mut self) -> PyResult<&mut StructureWriter> {
        self.inner
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Writer has been finalized"))
    }
}

#[pymethods]
impl PyStructureWriter {
    /// Create a streaming writer.
    ///
    /// Fields must be written in definition order.
    ///
    /// :param definition: The :class:`StructureDefinition` to encode against.
    /// :type definition: StructureDefinition
    /// :returns: A new writer.
    /// :rtype: StructureWriter
    #[staticmethod]
    fn new_streaming(definition: &PyStructureDefinition) -> Self {
        let writer = StructureWriter::new_streaming(Arc::clone(&definition.inner));
        Self {
            inner: Some(writer),
        }
    }

    /// Write a value to a field using bracket notation.
    ///
    /// :param path: The field path.
    /// :type path: str
    /// :param value: The value to write (``str``, ``bytes``, ``int``, or ``float``).
    /// :raises ValueError: If the value is invalid for the field.
    /// :raises RuntimeError: If the writer has been finalized.
    fn __setitem__(&mut self, path: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let writer = self.get_inner_mut()?;
        let write_value = python_to_write_value(value)?;
        writer.set(path, write_value)?;
        Ok(())
    }

    /// Write a value to a field by path.
    ///
    /// :param path: The field path.
    /// :type path: str
    /// :param value: The value to write.
    fn set(&mut self, path: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.__setitem__(path, value)
    }

    /// Check if a field has been written.
    ///
    /// :param path: The field path.
    /// :type path: str
    /// :returns: ``True`` if the field has been written.
    /// :rtype: bool
    fn is_set(&self, path: &str) -> PyResult<bool> {
        let writer = self.get_inner()?;
        Ok(writer.is_set(path))
    }

    /// Finalize the writer and return the encoded bytes.
    ///
    /// After calling ``finish()``, the writer is consumed and cannot be
    /// used again.
    ///
    /// :returns: The encoded binary data.
    /// :rtype: bytes
    /// :raises ValueError: If required fields have not been written.
    /// :raises RuntimeError: If the writer has already been finalized.
    ///
    /// Example:
    ///
    /// ```python
    /// writer = StructureWriter.new_fixed(definition)
    /// writer["FHDR"] = "NITF"
    /// writer["FVER"] = "02.10"
    /// data = writer.finish()
    /// ```
    fn finish<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let writer = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("Writer has been finalized"))?;
        let bytes = writer.finish()?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Return the current buffer contents without validation.
    ///
    /// Useful for debugging or inspecting partial writes.
    ///
    /// :returns: The current buffer contents.
    /// :rtype: bytes
    fn buffer<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let writer = self.get_inner()?;
        Ok(PyBytes::new(py, writer.buffer()))
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(writer) => format!("StructureWriter(buffer_len={})", writer.buffer().len()),
            None => "StructureWriter(<finalized>)".to_string(),
        }
    }
}

/// Convert a Python value to a WriteValue.
fn python_to_write_value(value: &Bound<'_, PyAny>) -> PyResult<WriteValue> {
    // Try string first
    if let Ok(s) = value.extract::<String>() {
        return Ok(WriteValue::String(s));
    }

    // Try integer
    if let Ok(n) = value.extract::<i64>() {
        return Ok(WriteValue::Integer(n));
    }

    // Try unsigned integer (for large values)
    if let Ok(n) = value.extract::<u64>() {
        return Ok(WriteValue::Unsigned(n));
    }

    // Try float
    if let Ok(f) = value.extract::<f64>() {
        return Ok(WriteValue::Float(f));
    }

    // Try list → WriteValue::Array (before Vec<u8> since an empty list also matches Vec<u8>)
    if let Ok(list) = value.cast::<PyList>() {
        let elements: PyResult<Vec<WriteValue>> = list
            .iter()
            .map(|item| python_to_write_value(&item))
            .collect();
        return Ok(WriteValue::Array(elements?));
    }

    // Try tuple → WriteValue::Array
    if let Ok(tup) = value.cast::<PyTuple>() {
        let elements: PyResult<Vec<WriteValue>> = tup
            .iter()
            .map(|item| python_to_write_value(&item))
            .collect();
        return Ok(WriteValue::Array(elements?));
    }

    // Try PyBytes
    if let Ok(bytes_obj) = value.cast::<PyBytes>() {
        return Ok(WriteValue::Bytes(bytes_obj.as_bytes().to_vec()));
    }

    // Try bytes-like (bytearray, memoryview, etc.)
    if let Ok(bytes) = value.extract::<Vec<u8>>() {
        return Ok(WriteValue::Bytes(bytes));
    }

    Err(PyTypeError::new_err(format!(
        "Cannot convert {} to a writable value",
        value.get_type().name()?
    )))
}
