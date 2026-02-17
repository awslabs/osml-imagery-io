//! Python bindings for the data-driven binary parser.
//!
//! This module provides PyO3 wrappers for the parser infrastructure,
//! exposing StructureRegistry, StructureAccessor, StructureWriter, and Value
//! to Python with Pythonic interfaces.

use std::sync::Arc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::parser::{
    AccessError, ConversionError, LoadError, StructureAccessor, StructureDefinition,
    StructureRegistry, StructureWriter, Value, WriteError,
};
use crate::parser::writer::WriteValue;

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
            WriteError::ValueTooLarge { path, max_size, actual_size } => {
                PyValueError::new_err(format!(
                    "Value too large for '{}': max {} bytes, got {}",
                    path, max_size, actual_size
                ))
            }
            WriteError::OutOfOrder { path, expected_after } => {
                PyValueError::new_err(format!(
                    "Field '{}' written out of order (expected after '{}')",
                    path, expected_after
                ))
            }
            _ => PyRuntimeError::new_err(err.to_string()),
        }
    }
}

// ==================== PyValue ====================

/// Python wrapper for parsed field values.
///
/// Provides type conversion methods for interpreting ASCII-encoded
/// numeric fields as integers or floats.
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
    /// Get the value as a string, trimming trailing padding.
    ///
    /// Returns:
    ///     str: The string value with trailing spaces removed.
    ///
    /// Raises:
    ///     TypeError: If the value cannot be converted to a string.
    fn as_str(&self) -> PyResult<String> {
        match &self.inner {
            OwnedValue::String(s) => Ok(s.trim_end_matches(' ').to_string()),
            OwnedValue::Bytes(bytes) => {
                let s = std::str::from_utf8(bytes)
                    .map_err(|e| PyTypeError::new_err(e.to_string()))?;
                Ok(s.trim_end_matches(' ').to_string())
            }
            OwnedValue::Unsigned(n) => Ok(n.to_string()),
            _ => Err(PyTypeError::new_err("Cannot convert to string")),
        }
    }

    /// Parse the value as a signed integer.
    ///
    /// Returns:
    ///     int: The parsed integer value.
    ///
    /// Raises:
    ///     TypeError: If the value cannot be converted to an integer.
    fn as_int(&self) -> PyResult<i64> {
        match &self.inner {
            OwnedValue::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed.parse::<i64>()
                    .map_err(|e| PyTypeError::new_err(format!("Cannot parse '{}' as int: {}", s, e)))
            }
            OwnedValue::Bytes(bytes) => {
                let s = std::str::from_utf8(bytes)
                    .map_err(|e| PyTypeError::new_err(e.to_string()))?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0);
                }
                trimmed.parse::<i64>()
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
    /// Returns:
    ///     float: The parsed floating-point value.
    ///
    /// Raises:
    ///     TypeError: If the value cannot be converted to a float.
    fn as_float(&self) -> PyResult<f64> {
        match &self.inner {
            OwnedValue::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0.0);
                }
                trimmed.parse::<f64>()
                    .map_err(|e| PyTypeError::new_err(format!("Cannot parse '{}' as float: {}", s, e)))
            }
            OwnedValue::Bytes(bytes) => {
                let s = std::str::from_utf8(bytes)
                    .map_err(|e| PyTypeError::new_err(e.to_string()))?;
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(0.0);
                }
                trimmed.parse::<f64>()
                    .map_err(|e| PyTypeError::new_err(format!("Cannot parse as float: {}", e)))
            }
            OwnedValue::Unsigned(n) => Ok(*n as f64),
            _ => Err(PyTypeError::new_err("Cannot convert to float")),
        }
    }

    /// Get the raw bytes of the value.
    ///
    /// Returns:
    ///     bytes: The raw byte representation.
    fn as_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        match &self.inner {
            OwnedValue::String(s) => Ok(PyBytes::new_bound(py, s.as_bytes())),
            OwnedValue::Bytes(bytes) => Ok(PyBytes::new_bound(py, bytes)),
            OwnedValue::Unsigned(n) => Ok(PyBytes::new_bound(py, &n.to_be_bytes())),
            OwnedValue::Struct { data, .. } => Ok(PyBytes::new_bound(py, data)),
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
}


// ==================== PyStructureRegistry ====================

/// Registry for structure definitions with search path resolution.
///
/// The registry manages loading, caching, and lookup of structure definitions
/// from multiple search paths. Definitions can be loaded from KSY files on disk
/// or registered at runtime.
///
/// Example:
///     >>> registry = StructureRegistry()
///     >>> registry.add_search_path("/custom/structures")
///     >>> definition = registry.get("NITF_02.10_FileHeader")
///     >>> for name in registry.list():
///     ...     print(name)
#[pyclass(name = "StructureRegistry")]
pub struct PyStructureRegistry {
    inner: StructureRegistry,
}

#[pymethods]
impl PyStructureRegistry {
    /// Create a new registry with default search paths.
    ///
    /// Default search paths include:
    /// - Package data directory (data/structures/)
    /// - Paths from OSML_IO_STRUCTURE_PATH environment variable
    #[new]
    fn new() -> Self {
        Self {
            inner: StructureRegistry::new(),
        }
    }

    /// Add a search path (higher priority than existing paths).
    ///
    /// Args:
    ///     path: The directory path to add to the search paths.
    fn add_search_path(&mut self, path: &str) {
        self.inner.add_search_path(path);
    }

    /// Get a structure definition by name.
    ///
    /// Args:
    ///     name: The structure name (e.g., "NITF_02.10_FileHeader", "TRE_GEOLOB").
    ///
    /// Returns:
    ///     The structure definition, or None if not found.
    fn get(&mut self, name: &str) -> Option<PyStructureDefinition> {
        self.inner.get_mut(name).map(PyStructureDefinition::new)
    }

    /// List all available structure names.
    ///
    /// Returns:
    ///     A list of all structure names that can be retrieved via get().
    fn list(&self) -> Vec<String> {
        self.inner.list()
    }

    /// Reload all definitions from disk.
    ///
    /// Clears the file cache and re-scans search paths.
    /// Runtime-registered definitions are preserved.
    fn reload(&mut self) -> PyResult<()> {
        self.inner.reload().map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Register a definition at runtime (highest priority).
    ///
    /// Runtime-registered definitions take priority over file-based
    /// definitions with the same name.
    ///
    /// Args:
    ///     name: The name to register the definition under.
    ///     definition: The structure definition to register.
    fn register(&mut self, name: &str, definition: &PyStructureDefinition) {
        // Clone the definition from the Arc
        let def = (*definition.inner).clone();
        self.inner.register(name, def);
    }

    /// Get the current search paths.
    ///
    /// Returns:
    ///     A list of directory paths being searched.
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

/// A structure definition parsed from a KSY file.
///
/// This is a read-only wrapper around the internal StructureDefinition.
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
    /// The unique identifier for this structure.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable title (if available).
    #[getter]
    fn title(&self) -> Option<&str> {
        self.inner.title.as_deref()
    }

    /// List of field names in this structure.
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

/// Lazy accessor for reading structure fields from binary data.
///
/// Provides dict-like access to parsed field values. Fields are parsed
/// on-demand when accessed, with computed offsets cached for efficiency.
///
/// Example:
///     >>> accessor = StructureAccessor(definition, data)
///     >>> value = accessor["field_name"]
///     >>> if accessor.has("optional_field"):
///     ...     print(accessor["optional_field"].as_str())
///     >>> for path in accessor.fields():
///     ...     print(path, accessor[path])
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
    /// Args:
    ///     definition: The structure definition to use for parsing.
    ///     data: The binary data to parse. Accepts bytes, bytearray, memoryview,
    ///           or any object supporting the buffer protocol (including mmap).
    ///
    /// Returns:
    ///     A new StructureAccessor instance.
    ///
    /// Note:
    ///     The data is copied into the accessor for safe access. For very large
    ///     files, consider using memory-mapped files (mmap) as input.
    #[new]
    fn py_new(definition: &PyStructureDefinition, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        // Extract bytes from various Python buffer types
        let bytes: Vec<u8> = if let Ok(bytes) = data.extract::<Vec<u8>>() {
            bytes
        } else if let Ok(bytes_obj) = data.downcast::<PyBytes>() {
            bytes_obj.as_bytes().to_vec()
        } else if data.hasattr("tobytes")? {
            // Try tobytes() method (works with memoryview)
            let buffer = data.call_method0("tobytes")?;
            buffer.extract::<Vec<u8>>()?
        } else {
            // Try slicing with [:] (works with mmap and other buffer-like objects)
            let py = data.py();
            let slice = pyo3::types::PySlice::full_bound(py);
            let sliced = data.get_item(&slice)?;
            sliced.extract::<Vec<u8>>()?
        };

        Ok(Self::new(Arc::clone(&definition.inner), bytes))
    }

    /// Access a field by path using bracket notation.
    ///
    /// Args:
    ///     path: Field path using dot notation (e.g., "parent.child" or "items_0.value").
    ///
    /// Returns:
    ///     The parsed Value for the field.
    ///
    /// Raises:
    ///     KeyError: If the field path does not exist.
    fn __getitem__(&self, path: &str) -> PyResult<PyValue> {
        let accessor = self.get_accessor()?;
        let value = accessor.get(path)?;
        Ok(PyValue::from_value(value))
    }

    /// Check if a field exists and is accessible.
    ///
    /// Args:
    ///     path: The field path to check.
    ///
    /// Returns:
    ///     True if the field exists and is accessible, False otherwise.
    fn has(&self, path: &str) -> PyResult<bool> {
        let accessor = self.get_accessor()?;
        Ok(accessor.has(path))
    }

    /// Check if a field exists (for 'in' operator).
    fn __contains__(&self, path: &str) -> PyResult<bool> {
        self.has(path)
    }

    /// Iterate over all accessible field paths.
    ///
    /// Returns:
    ///     A list of all field paths that can be accessed.
    fn fields(&self) -> PyResult<Vec<String>> {
        let accessor = self.get_accessor()?;
        Ok(accessor.fields().collect())
    }

    /// Get raw byte slice for a field.
    ///
    /// Returns the raw bytes for a field without interpretation. This is useful
    /// for passing binary data to external decoders.
    ///
    /// Args:
    ///     path: The field path.
    ///
    /// Returns:
    ///     bytes: The raw bytes for the field.
    ///
    /// Raises:
    ///     KeyError: If the field does not exist.
    ///     RuntimeError: If the field is not contiguous in memory (e.g., arrays).
    ///
    /// Note:
    ///     For zero-copy access to the underlying buffer, use field_byte_range()
    ///     to get the offset and length, then slice the original data directly.
    fn raw_view<'py>(&self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyBytes>> {
        let accessor = self.get_accessor()?;
        let slice = accessor.raw_slice(path)?;
        Ok(PyBytes::new_bound(py, slice))
    }

    /// Get byte offset and length for a field.
    ///
    /// Args:
    ///     path: The field path.
    ///
    /// Returns:
    ///     A tuple of (offset, length) for the field.
    ///
    /// Raises:
    ///     KeyError: If the field does not exist.
    fn field_byte_range(&self, path: &str) -> PyResult<(usize, usize)> {
        let accessor = self.get_accessor()?;
        Ok(accessor.field_byte_range(path)?)
    }

    /// Get the underlying data buffer.
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.data)
    }

    /// Get the structure definition.
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

/// Writer for encoding values according to a structure definition.
///
/// Supports both fixed-size mode (out-of-order writes) and streaming mode
/// (sequential writes).
///
/// Example:
///     >>> writer = StructureWriter.new_fixed(definition)
///     >>> writer["field1"] = "value1"
///     >>> writer["field2"] = 42
///     >>> data = writer.finish()
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
    /// Create a writer for fixed-size structures.
    ///
    /// Pre-allocates a buffer of the correct size. Fields can be written
    /// in any order.
    ///
    /// Args:
    ///     definition: The structure definition.
    ///
    /// Returns:
    ///     A new StructureWriter in fixed-size mode.
    #[staticmethod]
    fn new_fixed(definition: &PyStructureDefinition) -> PyResult<Self> {
        let writer = StructureWriter::new_fixed(Arc::clone(&definition.inner))?;
        Ok(Self {
            inner: Some(writer),
        })
    }

    /// Create a streaming writer for variable-size structures.
    ///
    /// Fields must be written in definition order.
    ///
    /// Args:
    ///     definition: The structure definition.
    ///
    /// Returns:
    ///     A new StructureWriter in streaming mode.
    #[staticmethod]
    fn new_streaming(definition: &PyStructureDefinition) -> Self {
        let writer = StructureWriter::new_streaming(Arc::clone(&definition.inner));
        Self {
            inner: Some(writer),
        }
    }

    /// Write a value to a field using bracket notation.
    ///
    /// Args:
    ///     path: The field path.
    ///     value: The value to write (str, bytes, int, or float).
    ///
    /// Raises:
    ///     ValueError: If the value is invalid for the field.
    ///     RuntimeError: If the writer has been finalized.
    fn __setitem__(&mut self, path: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let writer = self.get_inner_mut()?;
        let write_value = python_to_write_value(value)?;
        writer.set(path, write_value)?;
        Ok(())
    }

    /// Write a value to a field.
    ///
    /// Args:
    ///     path: The field path.
    ///     value: The value to write.
    fn set(&mut self, path: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.__setitem__(path, value)
    }

    /// Check if a field has been written.
    ///
    /// Args:
    ///     path: The field path.
    ///
    /// Returns:
    ///     True if the field has been written, False otherwise.
    fn is_set(&self, path: &str) -> PyResult<bool> {
        let writer = self.get_inner()?;
        Ok(writer.is_set(path))
    }

    /// Finalize and return the encoded bytes.
    ///
    /// Returns:
    ///     The encoded binary data.
    ///
    /// Raises:
    ///     ValueError: If required fields have not been written.
    ///     RuntimeError: If the writer has already been finalized.
    fn finish<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let writer = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("Writer has been finalized"))?;
        let bytes = writer.finish()?;
        Ok(PyBytes::new_bound(py, &bytes))
    }

    /// Get the current buffer contents without validation.
    ///
    /// This is useful for debugging or inspecting partial writes.
    ///
    /// Returns:
    ///     The current buffer contents.
    fn buffer<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let writer = self.get_inner()?;
        Ok(PyBytes::new_bound(py, writer.buffer()))
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(writer) => format!(
                "StructureWriter(buffer_len={})",
                writer.buffer().len()
            ),
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

    // Try bytes
    if let Ok(bytes) = value.extract::<Vec<u8>>() {
        return Ok(WriteValue::Bytes(bytes));
    }

    // Try PyBytes
    if let Ok(bytes_obj) = value.downcast::<PyBytes>() {
        return Ok(WriteValue::Bytes(bytes_obj.as_bytes().to_vec()));
    }

    Err(PyTypeError::new_err(format!(
        "Cannot convert {} to a writable value",
        value.get_type().name()?
    )))
}
