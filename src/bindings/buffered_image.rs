//! Python bindings for BufferedImageAssetProvider.
//!
//! This module provides Python bindings for creating synthetic images in memory.

use std::sync::Arc;

use numpy::PyReadonlyArrayDyn;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::bindings::{PyImageAssetProvider, PyMetadataProvider};
use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
use crate::traits::{AssetMetadata, ImageAssetProvider};
use crate::types::{AssetType, PixelType};

/// Extract raw bytes from a NumPy array of any supported dtype.
///
/// This function inspects the array's dtype at runtime and extracts the
/// underlying bytes appropriately. Supported dtypes: uint8, int8, uint16,
/// int16, uint32, int32, float32, float64.
///
/// Note: Uses native byte order for internal representation. The NITF
/// encoder handles conversion to big-endian at the file boundary.
pub(crate) fn extract_array_bytes(py: Python<'_>, data: &Py<PyAny>) -> PyResult<Vec<u8>> {
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
            // Single byte - no endianness conversion needed
            Ok(slice.iter().flat_map(|&v: &i8| v.to_ne_bytes()).collect())
        }
        "uint16" => {
            let array: PyReadonlyArrayDyn<'_, u16> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            // Native byte order for internal representation
            Ok(slice.iter().flat_map(|&v: &u16| v.to_ne_bytes()).collect())
        }
        "int16" => {
            let array: PyReadonlyArrayDyn<'_, i16> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v: &i16| v.to_ne_bytes()).collect())
        }
        "uint32" => {
            let array: PyReadonlyArrayDyn<'_, u32> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v: &u32| v.to_ne_bytes()).collect())
        }
        "int32" => {
            let array: PyReadonlyArrayDyn<'_, i32> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v: &i32| v.to_ne_bytes()).collect())
        }
        "float32" => {
            let array: PyReadonlyArrayDyn<'_, f32> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v: &f32| v.to_ne_bytes()).collect())
        }
        "float64" => {
            let array: PyReadonlyArrayDyn<'_, f64> = data.extract(py)?;
            let slice = array.as_slice().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Array must be contiguous: {}", e))
            })?;
            Ok(slice.iter().flat_map(|&v: &f64| v.to_ne_bytes()).collect())
        }
        _ => Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Unsupported array dtype '{}'. Supported: uint8, int8, uint16, int16, uint32, int32, float32, float64",
            dtype_str
        ))),
    }
}

/// Constructs image assets entirely in memory.
///
/// Use ``BufferedImageAssetProvider`` to create synthetic test data, assemble
/// mosaics, or build images from processed results. The provider implements
/// the same interface as :class:`ImageAssetProvider`, so in-memory images can
/// be passed to any API that accepts an image asset, including
/// :class:`DatasetWriter`.
///
/// All pixel arrays use a channels-first (CHW) layout with shape
/// ``(bands, rows, cols)``. This matches the convention used by PyTorch and
/// many deep learning pipelines. To convert to the channels-last (HWC) layout
/// expected by OpenCV or Pillow, use ``np.transpose(array, (1, 2, 0))``.
/// To convert from HWC back to CHW, use ``np.transpose(array, (2, 0, 1))``.
///
/// You can populate the image all at once with :meth:`set_full_image` or set
/// individual blocks with :meth:`set_block` for large or sparse images.
/// Optionally attach a :class:`BufferedMetadataProvider` to supply encoding
/// hints such as compression type (``IC``) and interleave mode (``IMODE``).
///
/// Example:
///
/// ```python
/// import numpy as np
/// from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
///
/// metadata = BufferedMetadataProvider()
/// metadata.set("IC", "NC")
/// metadata.set("IMODE", "B")
///
/// # Create a 512x512 RGB image with 256x256 blocks
/// provider = BufferedImageAssetProvider.create(
///     key="synthetic_image",
///     num_columns=512,
///     num_rows=512,
///     num_bands=3,
///     block_width=256,
///     block_height=256,
///     pixel_type=PixelType.UInt8,
///     metadata=metadata,
/// )
///
/// # Populate the full image at once
/// image_data = np.random.randint(0, 255, (3, 512, 512), dtype=np.uint8)
/// provider.set_full_image(image_data)
///
/// # Or set blocks individually for large/sparse images
/// for row in range(2):
///     for col in range(2):
///         block = np.random.randint(0, 255, (3, 256, 256), dtype=np.uint8)
///         provider.set_block(row, col, block)
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
    /// Create a new in-memory image asset with the specified dimensions and pixel format.
    ///
    /// :param key: Unique identifier for this asset.
    /// :type key: str
    /// :param num_columns: Image width in pixels.
    /// :type num_columns: int, optional
    /// :param num_rows: Image height in pixels.
    /// :type num_rows: int, optional
    /// :param num_bands: Number of spectral bands.
    /// :type num_bands: int, optional
    /// :param block_width: Block width in pixels.
    /// :type block_width: int, optional
    /// :param block_height: Block height in pixels.
    /// :type block_height: int, optional
    /// :param pixel_type: Pixel data type.
    /// :type pixel_type: PixelType, optional
    /// :param actual_bits_per_pixel: Actual bits per pixel, may be less than
    ///     the nominal size. ``None`` uses the full range for the pixel type.
    /// :type actual_bits_per_pixel: int, optional
    /// :param metadata: Encoding hints such as compression type (``IC``) and
    ///     interleave mode (``IMODE``). See :class:`BufferedMetadataProvider`.
    /// :type metadata: MetadataProvider, optional
    /// :param title: Human-readable title. Auto-generated if omitted.
    /// :type title: str, optional
    /// :param description: Detailed description. Auto-generated if omitted.
    /// :type description: str, optional
    /// :returns: A new in-memory image asset.
    /// :rtype: BufferedImageAssetProvider
    ///
    /// Example:
    ///
    /// ```python
    /// from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
    ///
    /// metadata = BufferedMetadataProvider()
    /// metadata.set("IC", "NC")
    /// metadata.set("IMODE", "B")
    ///
    /// provider = BufferedImageAssetProvider.create(
    ///     key="synthetic_image",
    ///     num_columns=512,
    ///     num_rows=512,
    ///     num_bands=3,
    ///     pixel_type=PixelType.UInt8,
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

    /// Create a mutable copy of an existing :class:`ImageAssetProvider`.
    ///
    /// The returned ``BufferedImageAssetProvider`` lazily delegates
    /// :meth:`get_block` calls to the source provider. Only blocks
    /// explicitly set via :meth:`set_block` are stored in memory; all
    /// others are read on demand from the source. This enables
    /// copy-on-write semantics without loading the entire image into
    /// memory.
    ///
    /// Because the returned provider holds a reference to the source,
    /// the source must remain open for the lifetime of the copy. If you
    /// need a fully independent snapshot, iterate over the blocks and
    /// call :meth:`set_block` for each one.
    ///
    /// :param provider: The source image asset to delegate to.
    /// :type provider: ImageAssetProvider
    /// :param key: Optional new key for the copy. If ``None``, the source
    ///     key is reused.
    /// :type key: str, optional
    /// :param block_width: Block width for the copy. If ``None``, uses the
    ///     source block width.
    /// :type block_width: int, optional
    /// :param block_height: Block height for the copy. If ``None``, uses the
    ///     source block height.
    /// :type block_height: int, optional
    /// :param metadata: Metadata for the copy. If ``None``, copies the
    ///     source metadata.
    /// :type metadata: MetadataProvider, optional
    /// :returns: A new mutable provider backed by the source.
    /// :rtype: BufferedImageAssetProvider
    ///
    /// Example:
    ///
    /// ```python
    /// from aws.osml.io import IO, BufferedImageAssetProvider
    ///
    /// with IO.open(["input.ntf"], "r") as reader:
    ///     source = reader.get_asset("image:0")
    ///     copy = BufferedImageAssetProvider.from_provider(source)
    ///     # Override specific blocks or metadata, then write
    /// ```
    #[staticmethod]
    #[pyo3(signature = (provider, key=None, block_width=None, block_height=None, metadata=None))]
    fn from_provider(
        provider: &PyImageAssetProvider,
        key: Option<&str>,
        block_width: Option<u32>,
        block_height: Option<u32>,
        metadata: Option<&PyMetadataProvider>,
    ) -> PyResult<Self> {
        let src = provider.inner();

        let bw = block_width.unwrap_or_else(|| src.num_pixels_per_block_horizontal());
        let bh = block_height.unwrap_or_else(|| src.num_pixels_per_block_vertical());
        let asset_key = key.unwrap_or_else(|| src.key());

        let config = MemoryImageConfig::new(src.num_columns(), src.num_rows())
            .with_bands(src.num_bands())
            .with_block_size(bw, bh)
            .with_pixel_type(src.pixel_value_type())
            .with_actual_bits_per_pixel(src.actual_bits_per_pixel());

        let meta = match metadata {
            Some(m) => m.inner().clone(),
            None => src.metadata(),
        };

        let buffered = BufferedImageAssetProvider::new(asset_key, config)
            .with_title(src.title(), src.description())
            .with_metadata(meta)
            .with_source(Arc::clone(src));

        Ok(Self {
            inner: Arc::new(buffered),
        })
    }

    /// Set the full image data from a NumPy array.
    ///
    /// The array must use channels-first (CHW) layout with shape
    /// ``(bands, rows, cols)``. The dimensions must match the values
    /// specified when the provider was created.
    ///
    /// :param data: Pixel data with shape ``(bands, rows, cols)``. Supported
    ///     dtypes: ``uint8``, ``int8``, ``uint16``, ``int16``, ``uint32``,
    ///     ``int32``, ``float32``, ``float64``. The dtype should match the
    ///     provider's ``pixel_type``.
    /// :type data: numpy.ndarray
    /// :raises ValueError: If the array size does not match the image
    ///     configuration (expected size = bands x rows x cols x bytes_per_pixel).
    /// :raises TypeError: If the array dtype is not supported.
    ///
    /// Example:
    ///
    /// ```python
    /// import numpy as np
    ///
    /// # Create RGB image data in CHW format
    /// image_data = np.zeros((3, 512, 512), dtype=np.uint8)
    /// image_data[0, :, :] = 255  # Red channel
    /// provider.set_full_image(image_data)
    /// ```
    fn set_full_image(&self, py: Python<'_>, data: Py<PyAny>) -> PyResult<()> {
        let bytes = extract_array_bytes(py, &data)?;
        self.inner.set_full_image(&bytes)?;
        Ok(())
    }

    /// Set pixel data for a single block at the given grid coordinates.
    ///
    /// The array must use channels-first (CHW) layout with shape
    /// ``(bands, block_rows, block_cols)``. For large or sparse images,
    /// setting blocks individually avoids loading the full image into memory.
    ///
    /// :param block_row: Row index in the block grid (0-indexed).
    /// :type block_row: int
    /// :param block_col: Column index in the block grid (0-indexed).
    /// :type block_col: int
    /// :param data: Pixel data with shape ``(bands, block_rows, block_cols)``.
    ///     Supported dtypes: ``uint8``, ``int8``, ``uint16``, ``int16``,
    ///     ``uint32``, ``int32``, ``float32``, ``float64``. The dtype should
    ///     match the provider's ``pixel_type``.
    /// :type data: numpy.ndarray
    /// :raises ValueError: If the array is not contiguous or block coordinates
    ///     are out of range.
    /// :raises TypeError: If the array dtype is not supported.
    ///
    /// Example:
    ///
    /// ```python
    /// import numpy as np
    ///
    /// # Set blocks individually for a 1024x1024 image with 256x256 blocks
    /// for row in range(4):
    ///     for col in range(4):
    ///         block = np.random.randint(0, 255, (3, 256, 256), dtype=np.uint8)
    ///         provider.set_block(row, col, block)
    /// ```
    fn set_block(
        &self,
        py: Python<'_>,
        block_row: u32,
        block_col: u32,
        data: Py<PyAny>,
    ) -> PyResult<()> {
        let bytes = extract_array_bytes(py, &data)?;
        self.inner.set_block(block_row, block_col, &bytes)?;
        Ok(())
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

    /// Image representation (MONO, RGB, MULTI, etc.).
    #[getter]
    fn irep(&self) -> String {
        self.inner.config().irep.clone()
    }

    // ========== ImageAssetProvider methods ==========

    /// Check whether a block exists at the given grid coordinates.
    ///
    /// :param block_row: Row index in the block grid.
    /// :type block_row: int
    /// :param block_col: Column index in the block grid.
    /// :type block_col: int
    /// :param resolution_level: Resolution level (0 = full resolution).
    /// :type resolution_level: int
    /// :returns: ``True`` if the block contains data, ``False`` otherwise.
    /// :rtype: bool
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
    ) -> PyResult<Py<PyAny>> {
        let bands_slice = bands.as_deref();
        let (data, shape) =
            self.inner
                .get_block(block_row, block_col, resolution_level, bands_slice)?;

        let pixel_type = self.inner.pixel_value_type();
        let array = crate::bindings::image::create_numpy_array(py, &data, shape, pixel_type)?;

        Ok(array)
    }
}
