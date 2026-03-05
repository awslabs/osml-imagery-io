//! Python bindings for ImageAssetProvider.
//!
//! This module provides the PyImageAssetProvider wrapper that exposes the
//! ImageAssetProvider trait to Python with numpy array support.

use std::sync::Arc;

use numpy::PyArrayMethods;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::PyMetadataProvider;
use crate::traits::ImageAssetProvider;
use crate::types::{AssetType, PixelType};

/// Python wrapper for ImageAssetProvider trait objects.
///
/// This class provides blocked/tiled access to large imagery with numpy array support.
#[pyclass(name = "ImageAssetProvider")]
pub struct PyImageAssetProvider {
    inner: Arc<dyn ImageAssetProvider>,
}

impl PyImageAssetProvider {
    /// Creates a new PyImageAssetProvider wrapping the given trait object.
    pub fn new(inner: Arc<dyn ImageAssetProvider>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner ImageAssetProvider.
    pub fn inner(&self) -> &Arc<dyn ImageAssetProvider> {
        &self.inner
    }
}

#[pymethods]
impl PyImageAssetProvider {
    // ========== AssetProvider properties ==========

    /// Returns the unique identifier for this asset within the dataset.
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

    // ========== ImageAssetProvider methods ==========

    /// Check if a block exists at the given coordinates.
    ///
    /// # Arguments
    ///
    /// * `block_row` - Row index of the block in the block grid
    /// * `block_col` - Column index of the block in the block grid
    /// * `resolution_level` - Resolution level (0 = full resolution)
    ///
    /// # Returns
    ///
    /// True if the block exists, False otherwise.
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        self.inner.has_block(block_row, block_col, resolution_level)
    }

    /// Retrieve block data as a numpy ndarray.
    ///
    /// # Arguments
    ///
    /// * `block_row` - Row index of the block in the block grid
    /// * `block_col` - Column index of the block in the block grid
    /// * `resolution_level` - Resolution level (0 = full resolution)
    /// * `bands` - Optional list of band indices to retrieve. If None, all bands are returned.
    ///
    /// # Returns
    ///
    /// A numpy ndarray with shape (bands, rows, cols) - CHW format - containing the block data.
    ///
    /// # Raises
    ///
    /// * IndexError - If the block coordinates are out of bounds
    /// * ValueError - If the resolution level is invalid
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
        let array = create_numpy_array(py, &data, shape, pixel_type)?;

        Ok(array)
    }
}

/// Creates a numpy array from raw bytes with the appropriate dtype.
/// 
/// The shape is expected in CHW format: [bands, rows, cols].
/// The returned numpy array will also be in CHW format.
pub fn create_numpy_array(
    py: Python<'_>,
    data: &[u8],
    shape: [u32; 3],
    pixel_type: PixelType,
) -> PyResult<PyObject> {
    // Shape is [bands, rows, cols] (CHW format)
    let bands = shape[0] as usize;
    let rows = shape[1] as usize;
    let cols = shape[2] as usize;

    match pixel_type {
        PixelType::UInt8 => {
            let array = numpy::PyArray1::<u8>::from_slice_bound(py, data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::Int8 => {
            let typed_data: Vec<i8> = data.iter().map(|&b| b as i8).collect();
            let array = numpy::PyArray1::<i8>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::UInt16 => {
            // Use native byte order for internal representation
            // The NITF encoder handles conversion to big-endian at the file boundary
            let typed_data: Vec<u16> = data
                .chunks_exact(2)
                .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
                .collect();
            let array = numpy::PyArray1::<u16>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::Int16 => {
            // Use native byte order for internal representation
            let typed_data: Vec<i16> = data
                .chunks_exact(2)
                .map(|chunk| i16::from_ne_bytes([chunk[0], chunk[1]]))
                .collect();
            let array = numpy::PyArray1::<i16>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::UInt32 => {
            // Use native byte order for internal representation
            let typed_data: Vec<u32> = data
                .chunks_exact(4)
                .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            let array = numpy::PyArray1::<u32>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::Int32 => {
            // Use native byte order for internal representation
            let typed_data: Vec<i32> = data
                .chunks_exact(4)
                .map(|chunk| i32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            let array = numpy::PyArray1::<i32>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::Float32 => {
            // Use native byte order for internal representation
            let typed_data: Vec<f32> = data
                .chunks_exact(4)
                .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            let array = numpy::PyArray1::<f32>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
        PixelType::Float64 => {
            // Use native byte order for internal representation
            let typed_data: Vec<f64> = data
                .chunks_exact(8)
                .map(|chunk| {
                    f64::from_ne_bytes([
                        chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6],
                        chunk[7],
                    ])
                })
                .collect();
            let array = numpy::PyArray1::<f64>::from_slice_bound(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            Ok(reshaped.into_py(py))
        }
    }
}
