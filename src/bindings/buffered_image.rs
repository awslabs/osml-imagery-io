//! Python bindings for BufferedImageAssetProvider.
//!
//! This module provides Python bindings for creating synthetic images in memory.

use std::sync::Arc;

use numpy::PyReadonlyArrayDyn;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
use crate::traits::{AssetProvider, ImageAssetProvider};
use crate::types::{AssetType, PixelType};

/// Extract raw bytes from a NumPy array of any supported dtype.
///
/// This function inspects the array's dtype at runtime and extracts the
/// underlying bytes appropriately. Supported dtypes: uint8, int8, uint16,
/// int16, uint32, int32, float32, float64.
fn extract_array_bytes(py: Python<'_>, data: &PyObject) -> PyResult<Vec<u8>> {
    // Get the dtype string from the array
    let dtype_str: String = data
        .getattr(py, "dtype")?
        .getattr(py, "name")?
        .extract(py)?;

    match dtype_str.as_str() {
        "uint8" => {
            let array: PyReadonlyArrayDyn<'_, u8> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.to_vec())
        }
        "int8" => {
            let array: PyReadonlyArrayDyn<'_, i8> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        "uint16" => {
            let array: PyReadonlyArrayDyn<'_, u16> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        "int16" => {
            let array: PyReadonlyArrayDyn<'_, i16> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        "uint32" => {
            let array: PyReadonlyArrayDyn<'_, u32> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        "int32" => {
            let array: PyReadonlyArrayDyn<'_, i32> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        "float32" => {
            let array: PyReadonlyArrayDyn<'_, f32> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        "float64" => {
            let array: PyReadonlyArrayDyn<'_, f64> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v| v.to_ne_bytes()).collect())
        }
        _ => Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Unsupported array dtype '{}'. Supported: uint8, int8, uint16, int16, uint32, int32, float32, float64",
            dtype_str
        ))),
    }
}

/// Python wrapper for BufferedImageAssetProvider.
///
/// This class allows creating synthetic images in memory with configurable
/// dimensions, tile sizes, pixel types, and band configurations.
///
/// # Array Format
///
/// All image data uses band-sequential (BSQ) ordering, also known as
/// "channels first" or CHW format. NumPy arrays should have shape
/// `(bands, rows, cols)`. This matches PyTorch's tensor format and
/// provides natural support for per-band operations common in remote sensing.
///
/// For interoperability with OpenCV or Pillow (which use HWC format),
/// use `np.transpose(array, (1, 2, 0))` to convert from CHW to HWC,
/// or `np.transpose(array, (2, 0, 1))` to convert from HWC to CHW.
///
/// # Example
///
/// ```python
/// from aws.osml.io import BufferedImageAssetProvider, PixelType
/// import numpy as np
///
/// # Create a 512x512 RGB image with 256x256 tiles
/// provider = BufferedImageAssetProvider.create(
///     key="synthetic_image",
///     num_columns=512,
///     num_rows=512,
///     num_bands=3,
///     block_width=256,
///     block_height=256,
///     pixel_type=PixelType.UInt8,
/// )
///
/// # Set the full image data - shape is (bands, rows, cols)
/// image_data = np.zeros((3, 512, 512), dtype=np.uint8)
/// provider.set_full_image(image_data)
///
/// # Set a single block - shape is (bands, block_rows, block_cols)
/// block_data = np.ones((3, 256, 256), dtype=np.uint8) * 128
/// provider.set_block(0, 0, block_data)
///
/// # Get a block back - returns shape (bands, block_rows, block_cols)
/// result = provider.get_block(0, 0, 0)
/// assert result.shape == (3, 256, 256)
/// ```
#[pyclass(name = "BufferedImageAssetProvider")]
pub struct PyBufferedImageAssetProvider {
    inner: Arc<BufferedImageAssetProvider>,
}

impl PyBufferedImageAssetProvider {
    /// Returns a reference to the inner provider.
    pub fn inner(&self) -> &Arc<BufferedImageAssetProvider> {
        &self.inner
    }

    /// Returns the inner provider as an Arc<dyn ImageAssetProvider>.
    pub fn as_image_provider(&self) -> Arc<dyn ImageAssetProvider> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyBufferedImageAssetProvider {
    /// Create a new BufferedImageAssetProvider with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `key` - Unique identifier for this asset
    /// * `num_columns` - Image width in pixels (default: 512)
    /// * `num_rows` - Image height in pixels (default: 512)
    /// * `num_bands` - Number of spectral bands (default: 1)
    /// * `block_width` - Block/tile width in pixels (default: 256)
    /// * `block_height` - Block/tile height in pixels (default: 256)
    /// * `pixel_type` - Pixel data type (default: UInt8)
    /// * `actual_bits_per_pixel` - Actual bits per pixel, may be less than nominal (default: None, uses full range)
    /// * `metadata` - Optional MetadataProvider for encoding hints (IMODE, IC, NPPBH, etc.)
    /// * `title` - Human-readable title (default: auto-generated)
    /// * `description` - Detailed description (default: auto-generated)
    ///
    /// # Returns
    ///
    /// A new BufferedImageAssetProvider instance.
    ///
    /// # Example
    ///
    /// ```python
    /// from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
    ///
    /// # Create with encoding hints (lowercase field names match .ksy parser output)
    /// metadata = BufferedMetadataProvider()
    /// metadata.set("imode", "P")  # Pixel interleave mode
    /// metadata.set("nppbh", "256")  # Block width
    ///
    /// provider = BufferedImageAssetProvider.create(
    ///     key="synthetic_image",
    ///     num_columns=512,
    ///     num_rows=512,
    ///     metadata=metadata,
    /// )
    /// ```
    #[staticmethod]
    #[pyo3(signature = (
        key,
        num_columns=512,
        num_rows=512,
        num_bands=1,
        block_width=256,
        block_height=256,
        pixel_type=PixelType::UInt8,
        actual_bits_per_pixel=None,
        metadata=None,
        title=None,
        description=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn create(
        key: &str,
        num_columns: u32,
        num_rows: u32,
        num_bands: u32,
        block_width: u32,
        block_height: u32,
        pixel_type: PixelType,
        actual_bits_per_pixel: Option<u32>,
        metadata: Option<&PyMetadataProvider>,
        title: Option<&str>,
        description: Option<&str>,
    ) -> Self {
        let mut config = MemoryImageConfig::new(num_columns, num_rows)
            .with_bands(num_bands)
            .with_block_size(block_width, block_height)
            .with_pixel_type(pixel_type);

        if let Some(abpp) = actual_bits_per_pixel {
            config = config.with_actual_bits_per_pixel(abpp);
        }

        let mut provider = BufferedImageAssetProvider::new(key, config);

        // Apply metadata if provided
        if let Some(meta) = metadata {
            provider = provider.with_metadata(meta.inner().clone());
        }

        // Apply title and description if provided
        let provider = match (title, description) {
            (Some(t), Some(d)) => provider.with_title(t, d),
            (Some(t), None) => {
                let desc = provider.description().to_string();
                provider.with_title(t, &desc)
            }
            _ => provider,
        };

        Self {
            inner: Arc::new(provider),
        }
    }

    /// Set the full image data from a numpy array.
    ///
    /// The array must be in band-sequential (BSQ) format, also known as
    /// "channels first" or CHW format, with shape `(bands, rows, cols)`.
    ///
    /// # Arguments
    ///
    /// * `data` - NumPy array with shape `(bands, rows, cols)`. Supported dtypes:
    ///   uint8, int8, uint16, int16, uint32, int32, float32, float64.
    ///   The dtype should match the provider's `pixel_type` configuration.
    ///
    /// # Raises
    ///
    /// * `ValueError` - If the array size doesn't match the image configuration
    ///   (expected size = bands × rows × cols × bytes_per_pixel)
    /// * `TypeError` - If the array dtype is not supported
    ///
    /// # Example
    ///
    /// ```python
    /// import numpy as np
    ///
    /// # Create RGB image data in CHW format: (3, 512, 512)
    /// image_data = np.zeros((3, 512, 512), dtype=np.uint8)
    /// image_data[0, :, :] = 255  # Red channel
    /// provider.set_full_image(image_data)
    /// ```
    fn set_full_image(&self, py: Python<'_>, data: PyObject) -> PyResult<()> {
        let bytes = extract_array_bytes(py, &data)?;
        self.inner.set_full_image(&bytes)?;
        Ok(())
    }

    /// Set block data at the given coordinates.
    ///
    /// The array must be in band-sequential (BSQ) format, also known as
    /// "channels first" or CHW format, with shape `(bands, block_rows, block_cols)`.
    ///
    /// # Arguments
    ///
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    /// * `data` - NumPy array with shape `(bands, block_rows, block_cols)`. Supported
    ///   dtypes: uint8, int8, uint16, int16, uint32, int32, float32, float64.
    ///   The dtype should match the provider's `pixel_type` configuration.
    ///
    /// # Raises
    ///
    /// * `ValueError` - If the array is not contiguous or block coordinates are invalid
    /// * `TypeError` - If the array dtype is not supported
    ///
    /// # Example
    ///
    /// ```python
    /// import numpy as np
    ///
    /// # Create block data in CHW format: (3, 256, 256)
    /// block_data = np.ones((3, 256, 256), dtype=np.uint8) * 128
    /// provider.set_block(0, 0, block_data)  # Set top-left block
    /// ```
    fn set_block(&self, py: Python<'_>, block_row: u32, block_col: u32, data: PyObject) -> PyResult<()> {
        let bytes = extract_array_bytes(py, &data)?;
        self.inner.set_block(block_row, block_col, &bytes)?;
        Ok(())
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

    // ========== ImageAssetProvider properties ==========

    /// Returns the number of resolution levels in the image pyramid.
    #[getter]
    fn num_resolution_levels(&self) -> u32 {
        self.inner.num_resolution_levels()
    }

    /// Returns the number of spectral bands.
    #[getter]
    fn num_bands(&self) -> u32 {
        self.inner.num_bands()
    }

    /// Returns the image height at full resolution in pixels.
    #[getter]
    fn num_rows(&self) -> u32 {
        self.inner.num_rows()
    }

    /// Returns the image width at full resolution in pixels.
    #[getter]
    fn num_columns(&self) -> u32 {
        self.inner.num_columns()
    }

    /// Returns the block width in pixels.
    #[getter]
    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.inner.num_pixels_per_block_horizontal()
    }

    /// Returns the block height in pixels.
    #[getter]
    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.inner.num_pixels_per_block_vertical()
    }

    /// Returns the nominal bits per pixel.
    #[getter]
    fn num_bits_per_pixel(&self) -> u32 {
        self.inner.num_bits_per_pixel()
    }

    /// Returns the actual bits per pixel.
    #[getter]
    fn actual_bits_per_pixel(&self) -> u32 {
        self.inner.actual_bits_per_pixel()
    }

    /// Returns the pixel data type.
    #[getter]
    fn pixel_value_type(&self) -> PixelType {
        self.inner.pixel_value_type()
    }

    /// Returns the value used for padding incomplete edge blocks.
    #[getter]
    fn pad_pixel_value(&self) -> f64 {
        self.inner.pad_pixel_value()
    }

    /// Returns the image dimensions as (bands, rows, columns) - CHW format.
    #[getter]
    fn image_shape(&self) -> (u32, u32, u32) {
        self.inner.image_shape()
    }

    /// Returns the block dimensions as (bands, rows, columns) - CHW format.
    #[getter]
    fn block_shape(&self) -> (u32, u32, u32) {
        self.inner.block_shape()
    }

    /// Returns the number of blocks in each dimension as (rows, cols).
    #[getter]
    fn block_grid_size(&self) -> (u32, u32) {
        self.inner.block_grid_size()
    }

    /// Returns the image representation (MONO, RGB, MULTI, etc.).
    #[getter]
    fn irep(&self) -> String {
        self.inner.config().irep.clone()
    }

    // ========== ImageAssetProvider methods ==========

    /// Check if a block exists at the given coordinates.
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        self.inner.has_block(block_row, block_col, resolution_level)
    }

    /// Retrieve block data as a numpy ndarray.
    ///
    /// Returns the block in band-sequential (BSQ) format, also known as
    /// "channels first" or CHW format, with shape `(bands, rows, cols)`.
    ///
    /// # Arguments
    ///
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    /// * `resolution_level` - Resolution level (0 = full resolution)
    /// * `bands` - Optional list of band indices to retrieve (None = all bands)
    ///
    /// # Returns
    ///
    /// NumPy array with shape `(bands, rows, cols)` in CHW format.
    /// The dtype matches the provider's `pixel_type` configuration.
    ///
    /// # Example
    ///
    /// ```python
    /// # Get full block with all bands
    /// block = provider.get_block(0, 0, 0)
    /// print(block.shape)  # (3, 256, 256) for RGB with 256x256 blocks
    ///
    /// # Get only the red channel (band 0)
    /// red_band = provider.get_block(0, 0, 0, bands=[0])
    /// print(red_band.shape)  # (1, 256, 256)
    /// ```
    #[pyo3(signature = (block_row, block_col, resolution_level, bands=None))]
    fn get_block<'py>(
        &self,
        py: Python<'py>,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<Vec<u32>>,
    ) -> PyResult<PyObject> {
        let bands_slice = bands.as_deref();
        let (data, shape) = self
            .inner
            .get_block(block_row, block_col, resolution_level, bands_slice)?;

        let pixel_type = self.inner.pixel_value_type();
        let array = crate::bindings::image::create_numpy_array(py, &data, shape, pixel_type)?;

        Ok(array)
    }
}
