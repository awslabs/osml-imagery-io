//! In-memory image asset provider for synthetic image generation.
//!
//! This module provides [`MemoryImageAssetProvider`] which implements the
//! [`ImageAssetProvider`] trait for creating synthetic images in memory.
//! It allows setting image parameters and block data programmatically.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::CodecError;
use crate::traits::{AssetProvider, ImageAssetProvider, MetadataProvider};
use crate::types::{AssetType, PixelType};

/// Configuration for a memory image.
#[derive(Clone, Debug)]
pub struct MemoryImageConfig {
    /// Image width in pixels
    pub num_columns: u32,
    /// Image height in pixels
    pub num_rows: u32,
    /// Number of spectral bands
    pub num_bands: u32,
    /// Block width in pixels
    pub block_width: u32,
    /// Block height in pixels
    pub block_height: u32,
    /// Pixel data type
    pub pixel_type: PixelType,
    /// Nominal bits per pixel
    pub bits_per_pixel: u32,
    /// Actual bits per pixel (may be less than nominal)
    pub actual_bits_per_pixel: u32,
    /// Interleave mode (B, P, R, S)
    pub imode: String,
    /// Image representation (MONO, RGB, MULTI, etc.)
    pub irep: String,
}

impl Default for MemoryImageConfig {
    fn default() -> Self {
        Self {
            num_columns: 512,
            num_rows: 512,
            num_bands: 1,
            block_width: 256,
            block_height: 256,
            pixel_type: PixelType::UInt8,
            bits_per_pixel: 8,
            actual_bits_per_pixel: 8,
            imode: "B".to_string(),
            irep: "MONO".to_string(),
        }
    }
}

impl MemoryImageConfig {
    /// Create a new configuration with the given dimensions.
    pub fn new(num_columns: u32, num_rows: u32) -> Self {
        Self {
            num_columns,
            num_rows,
            ..Default::default()
        }
    }

    /// Set the number of bands.
    pub fn with_bands(mut self, num_bands: u32) -> Self {
        self.num_bands = num_bands;
        // Update IREP based on band count
        self.irep = match num_bands {
            1 => "MONO".to_string(),
            3 => "RGB".to_string(),
            _ => "MULTI".to_string(),
        };
        self
    }

    /// Set the block dimensions.
    pub fn with_block_size(mut self, block_width: u32, block_height: u32) -> Self {
        self.block_width = block_width;
        self.block_height = block_height;
        self
    }

    /// Set the pixel type.
    pub fn with_pixel_type(mut self, pixel_type: PixelType) -> Self {
        self.pixel_type = pixel_type;
        self.bits_per_pixel = (pixel_type.bytes_per_pixel() * 8) as u32;
        self.actual_bits_per_pixel = self.bits_per_pixel;
        self
    }

    /// Set the actual bits per pixel (for sub-byte precision).
    pub fn with_actual_bits_per_pixel(mut self, abpp: u32) -> Self {
        self.actual_bits_per_pixel = abpp;
        self
    }

    /// Set the interleave mode.
    pub fn with_imode(mut self, imode: &str) -> Self {
        self.imode = imode.to_string();
        self
    }

    /// Calculate the number of blocks in the horizontal direction.
    pub fn num_blocks_horizontal(&self) -> u32 {
        (self.num_columns + self.block_width - 1) / self.block_width
    }

    /// Calculate the number of blocks in the vertical direction.
    pub fn num_blocks_vertical(&self) -> u32 {
        (self.num_rows + self.block_height - 1) / self.block_height
    }

    /// Calculate the total number of blocks.
    pub fn total_blocks(&self) -> u32 {
        self.num_blocks_horizontal() * self.num_blocks_vertical()
    }
}

/// Empty metadata provider for MemoryImageAssetProvider.
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
}

impl Default for EmptyMetadataProvider {
    fn default() -> Self {
        Self {
            empty_bytes: Vec::new(),
        }
    }
}

impl MetadataProvider for EmptyMetadataProvider {
    fn as_dict(&self, _prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    fn raw(&self) -> &[u8] {
        &self.empty_bytes
    }
}

/// In-memory image asset provider for synthetic image generation.
///
/// This provider stores image data in memory and allows setting block data
/// programmatically. It's useful for creating synthetic test images.
///
/// # Example
///
/// ```ignore
/// use aws_osml_io::memory_image::{MemoryImageAssetProvider, MemoryImageConfig};
/// use aws_osml_io::types::PixelType;
///
/// let config = MemoryImageConfig::new(512, 512)
///     .with_bands(3)
///     .with_block_size(256, 256)
///     .with_pixel_type(PixelType::UInt8);
///
/// let mut provider = MemoryImageAssetProvider::new("image_0", config);
///
/// // Set block data
/// let block_data = vec![128u8; 256 * 256 * 3];
/// provider.set_block(0, 0, &block_data)?;
/// ```
pub struct MemoryImageAssetProvider {
    /// Unique key identifying this asset
    key: String,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles
    roles: Vec<String>,
    /// Image configuration
    config: MemoryImageConfig,
    /// Block data storage (block_row, block_col) -> data
    blocks: RwLock<HashMap<(u32, u32), Vec<u8>>>,
    /// Metadata provider
    metadata: Arc<dyn MetadataProvider>,
}

impl MemoryImageAssetProvider {
    /// Create a new memory image asset provider.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this asset
    /// * `config` - Image configuration
    pub fn new(key: &str, config: MemoryImageConfig) -> Self {
        Self {
            key: key.to_string(),
            title: format!("Synthetic Image {}", key),
            description: format!(
                "{}x{} {}-band {} image",
                config.num_columns, config.num_rows, config.num_bands, config.irep
            ),
            roles: vec!["data".to_string()],
            config,
            blocks: RwLock::new(HashMap::new()),
            metadata: Arc::new(EmptyMetadataProvider::default()),
        }
    }

    /// Create with a custom title and description.
    pub fn with_title(mut self, title: &str, description: &str) -> Self {
        self.title = title.to_string();
        self.description = description.to_string();
        self
    }

    /// Get the image configuration.
    pub fn config(&self) -> &MemoryImageConfig {
        &self.config
    }

    /// Set block data at the given coordinates.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    /// * `data` - Raw pixel data in band-interleaved-by-pixel format
    ///
    /// # Returns
    /// Ok(()) on success, or an error if coordinates are invalid.
    pub fn set_block(&self, block_row: u32, block_col: u32, data: &[u8]) -> Result<(), CodecError> {
        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        if block_row >= num_blocks_v || block_col >= num_blocks_h {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
        }

        let mut blocks = self.blocks.write().map_err(|_| {
            CodecError::Decode("Failed to acquire write lock on blocks".to_string())
        })?;

        blocks.insert((block_row, block_col), data.to_vec());
        Ok(())
    }

    /// Set the full image data from a contiguous array.
    ///
    /// The data should be in band-sequential format (bands, rows, cols).
    /// This method will split the data into blocks automatically.
    ///
    /// # Arguments
    /// * `data` - Raw pixel data in band-sequential format
    pub fn set_full_image(&self, data: &[u8]) -> Result<(), CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let expected_size = (self.config.num_rows as usize)
            * (self.config.num_columns as usize)
            * (self.config.num_bands as usize)
            * bytes_per_pixel;

        if data.len() != expected_size {
            return Err(CodecError::Decode(format!(
                "Data size mismatch: expected {} bytes, got {}",
                expected_size,
                data.len()
            )));
        }

        // Convert from band-sequential to band-interleaved-by-pixel for storage
        let bip_data = self.bsq_to_bip(data)?;

        // Split into blocks
        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        let mut blocks = self.blocks.write().map_err(|_| {
            CodecError::Decode("Failed to acquire write lock on blocks".to_string())
        })?;

        for block_row in 0..num_blocks_v {
            for block_col in 0..num_blocks_h {
                let block_data = self.extract_block(&bip_data, block_row, block_col)?;
                blocks.insert((block_row, block_col), block_data);
            }
        }

        Ok(())
    }

    /// Convert band-sequential data to band-interleaved-by-pixel.
    fn bsq_to_bip(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let num_pixels = (self.config.num_rows as usize) * (self.config.num_columns as usize);
        let num_bands = self.config.num_bands as usize;
        let band_size = num_pixels * bytes_per_pixel;

        let mut bip_data = vec![0u8; data.len()];

        for band in 0..num_bands {
            let band_start = band * band_size;
            for pixel in 0..num_pixels {
                let src_offset = band_start + pixel * bytes_per_pixel;
                let dst_offset = pixel * num_bands * bytes_per_pixel + band * bytes_per_pixel;
                bip_data[dst_offset..dst_offset + bytes_per_pixel]
                    .copy_from_slice(&data[src_offset..src_offset + bytes_per_pixel]);
            }
        }

        Ok(bip_data)
    }

    /// Extract a block from the full BIP image data.
    fn extract_block(&self, bip_data: &[u8], block_row: u32, block_col: u32) -> Result<Vec<u8>, CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let num_bands = self.config.num_bands as usize;
        let pixel_stride = num_bands * bytes_per_pixel;
        let row_stride = (self.config.num_columns as usize) * pixel_stride;

        // Calculate block bounds
        let start_row = (block_row * self.config.block_height) as usize;
        let start_col = (block_col * self.config.block_width) as usize;
        let end_row = (start_row + self.config.block_height as usize).min(self.config.num_rows as usize);
        let end_col = (start_col + self.config.block_width as usize).min(self.config.num_columns as usize);

        let block_rows = end_row - start_row;
        let block_cols = end_col - start_col;
        let block_size = block_rows * block_cols * pixel_stride;

        let mut block_data = Vec::with_capacity(block_size);

        for row in start_row..end_row {
            let row_start = row * row_stride + start_col * pixel_stride;
            let row_end = row_start + (end_col - start_col) * pixel_stride;
            block_data.extend_from_slice(&bip_data[row_start..row_end]);
        }

        Ok(block_data)
    }

    /// Get the raw image data in band-sequential format.
    fn get_raw_bsq(&self) -> Result<Vec<u8>, CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let num_bands = self.config.num_bands as usize;
        let num_rows = self.config.num_rows as usize;
        let num_cols = self.config.num_columns as usize;
        let total_size = num_rows * num_cols * num_bands * bytes_per_pixel;

        // First, assemble the full BIP image
        let mut bip_data = vec![0u8; total_size];
        let pixel_stride = num_bands * bytes_per_pixel;
        let row_stride = num_cols * pixel_stride;

        let blocks = self.blocks.read().map_err(|_| {
            CodecError::Decode("Failed to acquire read lock on blocks".to_string())
        })?;

        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        for block_row in 0..num_blocks_v {
            for block_col in 0..num_blocks_h {
                if let Some(block_data) = blocks.get(&(block_row, block_col)) {
                    // Calculate block bounds
                    let start_row = (block_row * self.config.block_height) as usize;
                    let start_col = (block_col * self.config.block_width) as usize;
                    let end_row = (start_row + self.config.block_height as usize).min(num_rows);
                    let end_col = (start_col + self.config.block_width as usize).min(num_cols);

                    let block_cols = end_col - start_col;
                    let block_row_stride = block_cols * pixel_stride;

                    for (local_row, row) in (start_row..end_row).enumerate() {
                        let src_start = local_row * block_row_stride;
                        let src_end = src_start + block_row_stride;
                        let dst_start = row * row_stride + start_col * pixel_stride;
                        let dst_end = dst_start + block_row_stride;

                        if src_end <= block_data.len() && dst_end <= bip_data.len() {
                            bip_data[dst_start..dst_end].copy_from_slice(&block_data[src_start..src_end]);
                        }
                    }
                }
            }
        }

        // Convert BIP to BSQ
        self.bip_to_bsq(&bip_data)
    }

    /// Convert band-interleaved-by-pixel data to band-sequential.
    fn bip_to_bsq(&self, data: &[u8]) -> Result<Vec<u8>, CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let num_pixels = (self.config.num_rows as usize) * (self.config.num_columns as usize);
        let num_bands = self.config.num_bands as usize;
        let band_size = num_pixels * bytes_per_pixel;

        let mut bsq_data = vec![0u8; data.len()];

        for band in 0..num_bands {
            let band_start = band * band_size;
            for pixel in 0..num_pixels {
                let src_offset = pixel * num_bands * bytes_per_pixel + band * bytes_per_pixel;
                let dst_offset = band_start + pixel * bytes_per_pixel;
                bsq_data[dst_offset..dst_offset + bytes_per_pixel]
                    .copy_from_slice(&data[src_offset..src_offset + bytes_per_pixel]);
            }
        }

        Ok(bsq_data)
    }
}

impl AssetProvider for MemoryImageAssetProvider {
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
        "application/vnd.nitf.image"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Image
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        self.get_raw_bsq()
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ImageAssetProvider for MemoryImageAssetProvider {
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        if resolution_level != 0 {
            return false;
        }

        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        if block_row >= num_blocks_v || block_col >= num_blocks_h {
            return false;
        }

        let blocks = match self.blocks.read() {
            Ok(b) => b,
            Err(_) => return false,
        };

        blocks.contains_key(&(block_row, block_col))
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        if resolution_level != 0 {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, resolution_level));
        }

        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        if block_row >= num_blocks_v || block_col >= num_blocks_h {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, resolution_level));
        }

        let blocks = self.blocks.read().map_err(|_| {
            CodecError::Decode("Failed to acquire read lock on blocks".to_string())
        })?;

        let block_data = blocks.get(&(block_row, block_col)).ok_or_else(|| {
            CodecError::Decode(format!("Block ({}, {}) not found", block_row, block_col))
        })?;

        // Calculate actual block dimensions (may be smaller for edge blocks)
        let start_row = block_row * self.config.block_height;
        let start_col = block_col * self.config.block_width;
        let block_rows = (self.config.block_height).min(self.config.num_rows - start_row);
        let block_cols = (self.config.block_width).min(self.config.num_columns - start_col);

        let num_bands = if let Some(band_indices) = bands {
            band_indices.len() as u32
        } else {
            self.config.num_bands
        };

        // If specific bands requested, extract them
        let output_data = if let Some(band_indices) = bands {
            let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
            let all_bands = self.config.num_bands as usize;
            let num_pixels = (block_rows * block_cols) as usize;
            let mut extracted = Vec::with_capacity(num_pixels * band_indices.len() * bytes_per_pixel);

            for pixel in 0..num_pixels {
                for &band in band_indices {
                    let src_offset = pixel * all_bands * bytes_per_pixel + (band as usize) * bytes_per_pixel;
                    if src_offset + bytes_per_pixel <= block_data.len() {
                        extracted.extend_from_slice(&block_data[src_offset..src_offset + bytes_per_pixel]);
                    }
                }
            }
            extracted
        } else {
            block_data.clone()
        };

        Ok((output_data, [block_rows, block_cols, num_bands]))
    }

    fn num_resolution_levels(&self) -> u32 {
        1
    }

    fn num_bands(&self) -> u32 {
        self.config.num_bands
    }

    fn num_rows(&self) -> u32 {
        self.config.num_rows
    }

    fn num_columns(&self) -> u32 {
        self.config.num_columns
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.config.block_width
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.config.block_height
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.config.bits_per_pixel
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        self.config.actual_bits_per_pixel
    }

    fn pixel_value_type(&self) -> PixelType {
        self.config.pixel_type
    }

    fn pad_pixel_value(&self) -> f64 {
        0.0
    }
}

// Ensure MemoryImageAssetProvider is Send + Sync
unsafe impl Send for MemoryImageAssetProvider {}
unsafe impl Sync for MemoryImageAssetProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MemoryImageConfig::default();
        assert_eq!(config.num_columns, 512);
        assert_eq!(config.num_rows, 512);
        assert_eq!(config.num_bands, 1);
        assert_eq!(config.block_width, 256);
        assert_eq!(config.block_height, 256);
    }

    #[test]
    fn test_config_builder() {
        let config = MemoryImageConfig::new(1024, 768)
            .with_bands(3)
            .with_block_size(128, 128)
            .with_pixel_type(PixelType::UInt16)
            .with_actual_bits_per_pixel(11);

        assert_eq!(config.num_columns, 1024);
        assert_eq!(config.num_rows, 768);
        assert_eq!(config.num_bands, 3);
        assert_eq!(config.block_width, 128);
        assert_eq!(config.block_height, 128);
        assert_eq!(config.pixel_type, PixelType::UInt16);
        assert_eq!(config.bits_per_pixel, 16);
        assert_eq!(config.actual_bits_per_pixel, 11);
        assert_eq!(config.irep, "RGB");
    }

    #[test]
    fn test_block_count() {
        let config = MemoryImageConfig::new(500, 300)
            .with_block_size(256, 256);

        assert_eq!(config.num_blocks_horizontal(), 2);
        assert_eq!(config.num_blocks_vertical(), 2);
        assert_eq!(config.total_blocks(), 4);
    }

    #[test]
    fn test_provider_creation() {
        let config = MemoryImageConfig::new(512, 512);
        let provider = MemoryImageAssetProvider::new("test_image", config);

        assert_eq!(provider.key(), "test_image");
        assert_eq!(provider.asset_type(), AssetType::Image);
        assert_eq!(provider.num_rows(), 512);
        assert_eq!(provider.num_columns(), 512);
    }

    #[test]
    fn test_set_and_get_block() {
        let config = MemoryImageConfig::new(256, 256)
            .with_block_size(256, 256);
        let provider = MemoryImageAssetProvider::new("test", config);

        // Create test data (256x256 pixels, 1 band, 1 byte per pixel)
        let block_data = vec![128u8; 256 * 256];
        provider.set_block(0, 0, &block_data).unwrap();

        assert!(provider.has_block(0, 0, 0));
        
        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [256, 256, 1]);
        assert_eq!(data.len(), 256 * 256);
    }
}
