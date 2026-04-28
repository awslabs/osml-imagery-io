//! Callback adapter for Python-defined duck-typed image providers.
//!
//! This module provides `PyCallbackImageAssetProvider`, a Rust struct that wraps
//! an arbitrary Python object implementing the `ImageAssetProvider` interface via
//! duck typing. Immutable properties are cached at construction time; only
//! `get_block()` and `has_block()` cross the GIL boundary at call time.
//!
//! This struct is NOT exposed as a `#[pyclass]` — it is an internal implementation
//! detail used only by the writer's duck-typing fallback.

use std::sync::Arc;

use pyo3::prelude::*;

use crate::bindings::buffered_image::extract_array_bytes;
use crate::bindings::metadata::PyMetadataProvider;
use crate::buffered::BufferedMetadataProvider;
use crate::error::CodecError;
use crate::traits::{AssetMetadata, ImageAssetProvider, MetadataProvider};
use crate::types::PixelType;

/// Required attributes for a duck-typed image provider.
///
/// A Python object must have all of these attributes to be accepted
/// as a duck-typed image provider by `is_duck_typed_image_provider()`.
pub(crate) const REQUIRED_IMAGE_PROVIDER_ATTRS: &[&str] = &[
    "get_block",
    "num_rows",
    "num_columns",
    "num_bands",
    "num_bits_per_pixel",
    "actual_bits_per_pixel",
    "pixel_value_type",
    "num_pixels_per_block_horizontal",
    "num_pixels_per_block_vertical",
    "num_resolution_levels",
    "pad_pixel_value",
    "key",
    "title",
    "description",
];

/// Check if a Python object has all required image provider attributes.
///
/// Returns `true` only if every attribute in `REQUIRED_IMAGE_PROVIDER_ATTRS`
/// is present on the object.
pub(crate) fn is_duck_typed_image_provider(obj: &Bound<'_, PyAny>) -> bool {
    REQUIRED_IMAGE_PROVIDER_ATTRS
        .iter()
        .all(|attr| obj.hasattr(*attr).unwrap_or(false))
}

/// Callback adapter that wraps a Python object implementing the
/// `ImageAssetProvider` interface via duck typing.
///
/// Immutable properties are cached at construction time. Only
/// `get_block()` and `has_block()` cross the GIL boundary at call time.
pub(crate) struct PyCallbackImageAssetProvider {
    /// Reference to the wrapped Python object (Send + Sync via Py<PyAny>).
    py_obj: Py<PyAny>,

    // --- Cached AssetMetadata fields ---
    key: String,
    title: String,
    description: String,

    // --- Cached ImageAssetProvider fields ---
    num_rows: u32,
    num_columns: u32,
    num_bands: u32,
    num_bits_per_pixel: u32,
    actual_bits_per_pixel: u32,
    pixel_value_type: PixelType,
    num_pixels_per_block_horizontal: u32,
    num_pixels_per_block_vertical: u32,
    num_resolution_levels: u32,
    pad_pixel_value: f64,

    /// Whether the Python object has a `has_block` method.
    has_has_block: bool,
    /// Whether the Python object has a `get_metadata` method.
    has_get_metadata: bool,

    /// Stored roles for `AssetMetadata::roles()` return.
    roles: Vec<String>,
}

impl PyCallbackImageAssetProvider {
    /// Create a new adapter by reading all immutable properties from the
    /// Python object. Must be called while the GIL is held.
    ///
    /// Returns an error if any required property is missing or has an
    /// invalid type.
    pub(crate) fn new(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let key: String = Self::read_property(obj, "key")?;
        let title: String = Self::read_property(obj, "title")?;
        let description: String = Self::read_property(obj, "description")?;
        let num_rows: u32 = Self::read_property(obj, "num_rows")?;
        let num_columns: u32 = Self::read_property(obj, "num_columns")?;
        let num_bands: u32 = Self::read_property(obj, "num_bands")?;
        let num_bits_per_pixel: u32 = Self::read_property(obj, "num_bits_per_pixel")?;
        let actual_bits_per_pixel: u32 = Self::read_property(obj, "actual_bits_per_pixel")?;
        let pixel_value_type: PixelType = Self::read_property(obj, "pixel_value_type")?;
        let num_pixels_per_block_horizontal: u32 =
            Self::read_property(obj, "num_pixels_per_block_horizontal")?;
        let num_pixels_per_block_vertical: u32 =
            Self::read_property(obj, "num_pixels_per_block_vertical")?;
        let num_resolution_levels: u32 = Self::read_property(obj, "num_resolution_levels")?;
        let pad_pixel_value: f64 = Self::read_property(obj, "pad_pixel_value")?;

        let has_has_block = obj.hasattr("has_block").unwrap_or(false);
        let has_get_metadata = obj.hasattr("get_metadata").unwrap_or(false);

        Ok(Self {
            py_obj: obj.clone().unbind(),
            key,
            title,
            description,
            num_rows,
            num_columns,
            num_bands,
            num_bits_per_pixel,
            actual_bits_per_pixel,
            pixel_value_type,
            num_pixels_per_block_horizontal,
            num_pixels_per_block_vertical,
            num_resolution_levels,
            pad_pixel_value,
            has_has_block,
            has_get_metadata,
            roles: vec!["data".to_string()],
        })
    }

    /// Helper to read a single property with a descriptive error on failure.
    ///
    /// Uses `Py<PyAny>` intermediate to avoid lifetime issues with
    /// `FromPyObject`'s borrow of the `Bound` reference.
    fn read_property<T>(obj: &Bound<'_, PyAny>, name: &str) -> PyResult<T>
    where
        T: for<'a, 'py> FromPyObject<'a, 'py>,
    {
        let py = obj.py();
        let attr: Py<PyAny> = obj
            .getattr(name)
            .map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(format!(
                    "Python provider missing or invalid property '{}': \
                     expected a compatible type",
                    name
                ))
            })?
            .unbind();
        attr.extract(py).map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "Python provider missing or invalid property '{}': \
                 expected a compatible type",
                name
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// AssetMetadata trait implementation
// ---------------------------------------------------------------------------

impl AssetMetadata for PyCallbackImageAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn media_type(&self) -> &str {
        "application/octet-stream"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        if self.has_get_metadata {
            Python::attach(|py| {
                let result = self
                    .py_obj
                    .call_method0(py, "get_metadata")
                    .ok()
                    .and_then(|obj| {
                        obj.extract::<PyRef<'_, PyMetadataProvider>>(py)
                            .ok()
                            .map(|meta| Arc::clone(meta.inner()))
                    });
                match result {
                    Some(provider) => provider,
                    None => Arc::new(BufferedMetadataProvider::new()) as Arc<dyn MetadataProvider>,
                }
            })
        } else {
            Arc::new(BufferedMetadataProvider::new())
        }
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// ImageAssetProvider trait implementation
// ---------------------------------------------------------------------------

impl ImageAssetProvider for PyCallbackImageAssetProvider {
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        if !self.has_has_block {
            return true;
        }
        Python::attach(|py| {
            self.py_obj
                .call_method(
                    py,
                    "has_block",
                    (block_row, block_col, resolution_level),
                    None,
                )
                .and_then(|r| r.extract::<bool>(py))
                .unwrap_or(true)
        })
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        Python::attach(|py| {
            let bands_arg = bands.map(|b| b.to_vec());
            let result = self
                .py_obj
                .call_method(
                    py,
                    "get_block",
                    (block_row, block_col, resolution_level, bands_arg),
                    None,
                )
                .map_err(|e| CodecError::Python(e.to_string()))?;

            extract_block_data(py, &result, self.pixel_value_type)
        })
    }

    fn num_resolution_levels(&self) -> u32 {
        self.num_resolution_levels
    }

    fn num_bands(&self) -> u32 {
        self.num_bands
    }

    fn num_rows(&self) -> u32 {
        self.num_rows
    }

    fn num_columns(&self) -> u32 {
        self.num_columns
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.num_pixels_per_block_horizontal
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.num_pixels_per_block_vertical
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.num_bits_per_pixel
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        self.actual_bits_per_pixel
    }

    fn pixel_value_type(&self) -> PixelType {
        self.pixel_value_type
    }

    fn pad_pixel_value(&self) -> f64 {
        self.pad_pixel_value
    }
}

// ---------------------------------------------------------------------------
// Block data extraction helper
// ---------------------------------------------------------------------------

/// Extract block data from a Python NumPy array result.
///
/// Validates the array dtype against the expected pixel type, ensures the
/// array is contiguous, extracts raw bytes, and returns the shape.
fn extract_block_data(
    py: Python<'_>,
    result: &Py<PyAny>,
    expected_pixel_type: PixelType,
) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
    // 1. Get dtype string from the array
    let dtype_str: String = result
        .getattr(py, "dtype")
        .and_then(|d| d.getattr(py, "name"))
        .and_then(|n| n.extract(py))
        .map_err(|e| CodecError::Python(format!("Failed to read array dtype: {}", e)))?;

    // 2. Validate dtype matches expected pixel type
    let expected_dtype = expected_pixel_type.to_numpy_dtype();
    if dtype_str != expected_dtype {
        return Err(CodecError::Python(format!(
            "Array dtype '{}' does not match expected pixel type '{}'",
            dtype_str, expected_dtype
        )));
    }

    // 3. Get shape [bands, rows, cols]
    let shape: Vec<u32> = result
        .getattr(py, "shape")
        .and_then(|s| s.extract(py))
        .map_err(|e| CodecError::Python(format!("Failed to read array shape: {}", e)))?;

    if shape.len() != 3 {
        return Err(CodecError::Python(format!(
            "Expected 3D array (bands, rows, cols), got {}D",
            shape.len()
        )));
    }

    // 4. Use numpy.ascontiguousarray() to handle non-contiguous arrays
    let np = py
        .import("numpy")
        .map_err(|e| CodecError::Python(format!("Failed to import numpy: {}", e)))?;
    let contiguous = np
        .call_method1("ascontiguousarray", (result,))
        .map_err(|e| CodecError::Python(format!("Failed to make array contiguous: {}", e)))?;

    // 5. Extract bytes using the shared helper from buffered_image.rs
    let bytes = extract_array_bytes(py, &contiguous.unbind())
        .map_err(|e| CodecError::Python(format!("Failed to extract array bytes: {}", e)))?;

    Ok((bytes, [shape[0], shape[1], shape[2]]))
}
