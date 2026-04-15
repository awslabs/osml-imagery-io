//! Python bindings for ImageAssetProvider.
//!
//! This module provides the PyImageAssetProvider wrapper that exposes the
//! ImageAssetProvider trait to Python with numpy array support.

use std::sync::Arc;

use numpy::PyArrayMethods;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3::IntoPyObjectExt;

use crate::bindings::PyMetadataProvider;
use crate::traits::ImageAssetProvider;
use crate::types::{AssetType, PixelType};

/// Provides blocked (tiled) access to the pixel data of an image asset.
///
/// Large geospatial images are divided into a regular grid of fixed-size
/// rectangular blocks. ``ImageAssetProvider`` lets you read individual blocks
/// as NumPy arrays without loading the entire image into memory. Use
/// :meth:`DatasetReader.get_asset` to obtain an instance for a specific image
/// asset in the dataset.
///
/// All arrays returned by :meth:`get_block` use a channels-first (CHW) layout
/// with shape ``(bands, rows, cols)``. This matches the convention used by
/// PyTorch and many deep learning pipelines. To convert to the channels-last
/// (HWC) layout expected by OpenCV or Pillow, use
/// ``np.transpose(block, (1, 2, 0))``.
///
/// Example:
///
/// ```python
/// import numpy as np
/// from aws.osml.io import IO
///
/// with IO.open(["image.ntf"], "r") as dataset:
///     image = dataset.get_asset("image:0")
///
///     # Read an RGB composite from a multispectral image
///     rgb = image.get_block(0, 0, resolution_level=0, bands=[3, 2, 1])
///
///     # Convert CHW to HWC for display with matplotlib or Pillow
///     rgb_hwc = np.transpose(rgb, (1, 2, 0))
///
///     # Iterate over all blocks, skipping masked regions
///     grid_rows, grid_cols = image.block_grid_size
///     for row in range(grid_rows):
///         for col in range(grid_cols):
///             if image.has_block(row, col, resolution_level=0):
///                 block = image.get_block(row, col, resolution_level=0)
/// ```
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
        AssetType::Image
    }

    /// Raw asset bytes as a ``BytesIO`` object.
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

    // ========== ImageAssetProvider properties ==========

    /// Number of resolution levels in the image pyramid.
    #[getter]
    fn num_resolution_levels(&self) -> u32 {
        self.inner.num_resolution_levels()
    }

    /// Number of spectral bands.
    #[getter]
    fn num_bands(&self) -> u32 {
        self.inner.num_bands()
    }

    /// Image height at full resolution in pixels.
    #[getter]
    fn num_rows(&self) -> u32 {
        self.inner.num_rows()
    }

    /// Image width at full resolution in pixels.
    #[getter]
    fn num_columns(&self) -> u32 {
        self.inner.num_columns()
    }

    /// Block width in pixels.
    #[getter]
    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.inner.num_pixels_per_block_horizontal()
    }

    /// Block height in pixels.
    #[getter]
    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.inner.num_pixels_per_block_vertical()
    }

    /// Nominal bits per pixel.
    #[getter]
    fn num_bits_per_pixel(&self) -> u32 {
        self.inner.num_bits_per_pixel()
    }

    /// Actual bits per pixel.
    #[getter]
    fn actual_bits_per_pixel(&self) -> u32 {
        self.inner.actual_bits_per_pixel()
    }

    /// Pixel data type.
    #[getter]
    fn pixel_value_type(&self) -> PixelType {
        self.inner.pixel_value_type()
    }

    /// Value used for padding incomplete edge blocks.
    #[getter]
    fn pad_pixel_value(&self) -> f64 {
        self.inner.pad_pixel_value()
    }

    /// Image dimensions as ``(bands, rows, columns)`` in CHW format.
    #[getter]
    fn image_shape(&self) -> (u32, u32, u32) {
        self.inner.image_shape()
    }

    /// Block dimensions as ``(bands, rows, columns)`` in CHW format.
    #[getter]
    fn block_shape(&self) -> (u32, u32, u32) {
        self.inner.block_shape()
    }

    /// Number of blocks in each dimension as ``(rows, cols)``.
    #[getter]
    fn block_grid_size(&self) -> (u32, u32) {
        self.inner.block_grid_size()
    }

    // ========== ImageAssetProvider methods ==========

    /// Check whether a block exists at the given grid coordinates.
    ///
    /// Some formats (notably NITF) support masked (sparse) images where not
    /// every position in the block grid contains data. Use this method to
    /// skip empty regions when iterating over blocks.
    ///
    /// :param block_row: Row index in the block grid.
    /// :type block_row: int
    /// :param block_col: Column index in the block grid.
    /// :type block_col: int
    /// :param resolution_level: Resolution level (0 = full resolution).
    /// :type resolution_level: int
    /// :returns: ``True`` if the block contains data, ``False`` otherwise.
    /// :rtype: bool
    ///
    /// Example:
    ///
    /// ```python
    /// grid_rows, grid_cols = image.block_grid_size
    /// for row in range(grid_rows):
    ///     for col in range(grid_cols):
    ///         if image.has_block(row, col, resolution_level=0):
    ///             block = image.get_block(row, col, resolution_level=0)
    /// ```
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        self.inner.has_block(block_row, block_col, resolution_level)
    }

    /// Read a block of pixel data as a NumPy array.
    ///
    /// Returns an ``ndarray`` with shape ``(bands, rows, cols)`` in
    /// channels-first (CHW) format. The NumPy dtype is selected
    /// automatically based on the image's ``pixel_value_type``.
    ///
    /// :param block_row: Row index in the block grid.
    /// :type block_row: int
    /// :param block_col: Column index in the block grid.
    /// :type block_col: int
    /// :param resolution_level: Resolution level (0 = full resolution).
    /// :type resolution_level: int
    /// :param bands: Zero-based band indices to retrieve. If ``None``,
    ///     all bands are returned.
    /// :type bands: list[int], optional
    /// :returns: Pixel data with shape ``(bands, rows, cols)``.
    /// :rtype: numpy.ndarray
    /// :raises IndexError: If the block coordinates are out of bounds.
    /// :raises ValueError: If the resolution level is invalid.
    ///
    /// Example:
    ///
    /// ```python
    /// # All bands at full resolution
    /// block = image.get_block(0, 0, resolution_level=0)
    ///
    /// # Natural color from a multispectral image (R, G, B)
    /// rgb = image.get_block(0, 0, resolution_level=0, bands=[3, 2, 1])
    ///
    /// # Near-infrared band for vegetation analysis
    /// nir = image.get_block(0, 0, resolution_level=0, bands=[4])
    /// ```
    #[pyo3(signature = (block_row, block_col, resolution_level, bands=None))]
    fn get_block<'py>(
        &self,
        py: Python<'py>,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<Vec<u32>>,
    ) -> PyResult<Py<PyAny>> {
        let bands_slice = bands.as_deref();
        let inner = Arc::clone(&self.inner);
        let (data, shape) = py.detach(|| {
            inner.get_block(block_row, block_col, resolution_level, bands_slice)
        })?;

        let pixel_type = self.inner.pixel_value_type();
        let array = create_numpy_array(py, &data, shape, pixel_type)?;

        Ok(array)
    }

    /// Return per-tile byte ranges relative to the source file.
    ///
    /// Returns a dictionary mapping ``(block_row, block_col)`` tuples to
    /// a list of ``(byte_offset, byte_length)`` tuples, where offsets are
    /// relative to the start of the source file. Each list contains one
    /// entry per tile-part; for most formats this is a single-element list.
    ///
    /// Returns ``None`` for providers without a backing file (e.g. in-memory
    /// images created with :class:`BufferedImageAssetProvider`).
    ///
    /// :returns: Mapping of tile coordinates to byte range lists, or ``None``.
    /// :rtype: dict[tuple[int, int], list[tuple[int, int]]] | None
    fn tile_byte_ranges<'py>(&self, py: Python<'py>) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.tile_byte_ranges() {
            None => Ok(None),
            Some(ranges) => {
                let dict = PyDict::new(py);
                for ((row, col), range_list) in ranges {
                    let py_list: Vec<(u64, u64)> = range_list;
                    dict.set_item((row, col), py_list.into_pyobject(py)?)?;
                }
                Ok(Some(dict.into_any().unbind()))
            }
        }
    }

    /// Return opaque codec configuration for independent tile decoding.
    ///
    /// The returned dictionary contains format-specific key-value pairs
    /// needed to decode tiles independently. For JPEG 2000 images this
    /// includes a ``"main_header"`` key whose value is the raw codestream
    /// main header bytes.
    ///
    /// Returns ``None`` if no configuration is needed (e.g. uncompressed
    /// images).
    ///
    /// :returns: Codec parameters, or ``None``.
    /// :rtype: dict[str, bytes] | None
    fn codec_configuration<'py>(&self, py: Python<'py>) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.codec_configuration() {
            None => Ok(None),
            Some(config) => {
                let dict = PyDict::new(py);
                for (key, value) in config {
                    dict.set_item(key, PyBytes::new(py, &value))?;
                }
                Ok(Some(dict.into_any().unbind()))
            }
        }
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
) -> PyResult<Py<PyAny>> {
    // Shape is [bands, rows, cols] (CHW format)
    let bands = shape[0] as usize;
    let rows = shape[1] as usize;
    let cols = shape[2] as usize;

    match pixel_type {
        PixelType::UInt8 => {
            let array = numpy::PyArray1::<u8>::from_slice(py, data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::Int8 => {
            let typed_data: Vec<i8> = data.iter().map(|&b| b as i8).collect();
            let array = numpy::PyArray1::<i8>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::UInt16 => {
            // All decoders produce native-endian bytes internally
            let typed_data: Vec<u16> = data
                .chunks_exact(2)
                .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
                .collect();
            let array = numpy::PyArray1::<u16>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::Int16 => {
            let typed_data: Vec<i16> = data
                .chunks_exact(2)
                .map(|chunk| i16::from_ne_bytes([chunk[0], chunk[1]]))
                .collect();
            let array = numpy::PyArray1::<i16>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::UInt32 => {
            let typed_data: Vec<u32> = data
                .chunks_exact(4)
                .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            let array = numpy::PyArray1::<u32>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::Int32 => {
            let typed_data: Vec<i32> = data
                .chunks_exact(4)
                .map(|chunk| i32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            let array = numpy::PyArray1::<i32>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::Float32 => {
            let typed_data: Vec<f32> = data
                .chunks_exact(4)
                .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            let array = numpy::PyArray1::<f32>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
        PixelType::Float64 => {
            let typed_data: Vec<f64> = data
                .chunks_exact(8)
                .map(|chunk| {
                    f64::from_ne_bytes([
                        chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6],
                        chunk[7],
                    ])
                })
                .collect();
            let array = numpy::PyArray1::<f64>::from_slice(py, &typed_data);
            let reshaped = array.reshape([bands, rows, cols])?;
            reshaped.into_pyobject(py).map(|a| a.into_any().unbind()).map_err(|e| e.into())
        }
    }
}
