//! In-memory image asset provider for synthetic image generation.
//!
//! This module provides [`BufferedImageAssetProvider`] which implements the
//! [`ImageAssetProvider`] trait for creating synthetic images in memory.
//! It allows setting image parameters and block data programmatically.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::assembly::{pad_pixel_bytes, TileAssembler};
use crate::error::CodecError;
use crate::traits::{AssetMetadata, ImageAssetProvider, MetadataProvider};
use crate::types::PixelType;

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

    /// Set the nominal bits per pixel (NBPP — storage container size).
    ///
    /// For sub-byte imagery, this is the packed bit depth (1, 2, or 4).
    /// For multi-byte imagery where ABPP < NBPP (e.g. 11-bit in 16-bit
    /// container), set this to the container size and use
    /// `with_actual_bits_per_pixel` for the significant bits.
    ///
    /// Does not modify `actual_bits_per_pixel` — call these in any order.
    pub fn with_bits_per_pixel(mut self, nbpp: u32) -> Self {
        self.bits_per_pixel = nbpp;
        self
    }

    /// Set the actual bits per pixel (ABPP — significant bits).
    ///
    /// Does not modify `bits_per_pixel` — call these in any order.
    pub fn with_actual_bits_per_pixel(mut self, abpp: u32) -> Self {
        self.actual_bits_per_pixel = abpp;
        self
    }

    /// Calculate the number of blocks in the horizontal direction.
    pub fn num_blocks_horizontal(&self) -> u32 {
        self.num_columns.div_ceil(self.block_width)
    }

    /// Calculate the number of blocks in the vertical direction.
    pub fn num_blocks_vertical(&self) -> u32 {
        self.num_rows.div_ceil(self.block_height)
    }

    /// Calculate the total number of blocks.
    pub fn total_blocks(&self) -> u32 {
        self.num_blocks_horizontal() * self.num_blocks_vertical()
    }
}

/// Empty metadata provider for BufferedImageAssetProvider.
#[derive(Default)]
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
}

impl MetadataProvider for EmptyMetadataProvider {
    fn entries(&self, _prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
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
/// use osml_imagery_io::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
/// use osml_imagery_io::types::PixelType;
///
/// let config = MemoryImageConfig::new(512, 512)
///     .with_bands(3)
///     .with_block_size(256, 256)
///     .with_pixel_type(PixelType::UInt8);
///
/// let mut provider = BufferedImageAssetProvider::new("image_0", config);
///
/// // Set block data
/// let block_data = vec![128u8; 256 * 256 * 3];
/// provider.set_block(0, 0, &block_data)?;
/// ```
pub struct BufferedImageAssetProvider {
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
    /// Set of block coordinates that have been provided via set_block()
    /// Used for sparse image support with masked IC values
    provided_blocks: RwLock<HashSet<(u32, u32)>>,
    /// Metadata provider
    metadata: Arc<dyn MetadataProvider>,
    /// Optional source provider for lazy delegation.
    /// When present, `get_block` checks local overrides first, then falls
    /// back to this source. This enables copy-on-write semantics: only
    /// blocks explicitly set via `set_block` are stored in memory; all
    /// others are read on demand from the source.
    source: Option<Arc<dyn ImageAssetProvider>>,
}

impl BufferedImageAssetProvider {
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
            provided_blocks: RwLock::new(HashSet::new()),
            metadata: Arc::new(EmptyMetadataProvider::default()),
            source: None,
        }
    }

    /// Create with a custom title and description.
    pub fn with_title(mut self, title: &str, description: &str) -> Self {
        self.title = title.to_string();
        self.description = description.to_string();
        self
    }

    /// Create with a custom metadata provider.
    ///
    /// This allows attaching encoding hints and other metadata to the asset.
    /// The metadata will be accessible via the `metadata()` method and can be
    /// used by writers to control format-specific encoding options.
    ///
    /// # Arguments
    /// * `metadata` - The metadata provider to attach to this asset
    ///
    /// # Example
    ///
    /// ```ignore
    /// use osml_imagery_io::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
    /// use osml_imagery_io::BufferedMetadataProvider;
    ///
    /// let metadata = BufferedMetadataProvider::new();
    /// metadata.set("imode", serde_json::json!("P"));
    /// metadata.set("nppbh", serde_json::json!("256"));
    ///
    /// let config = MemoryImageConfig::new(512, 512);
    /// let provider = BufferedImageAssetProvider::new("image_0", config)
    ///     .with_metadata(Arc::new(metadata));
    /// ```
    pub fn with_metadata(mut self, metadata: Arc<dyn MetadataProvider>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set a source provider for lazy block delegation.
    ///
    /// When a source is set, `get_block` checks local overrides first
    /// (blocks set via `set_block`), then falls back to the source provider.
    /// This enables copy-on-write semantics without loading the entire image
    /// into memory.
    ///
    /// # Arguments
    /// * `source` - The source provider to delegate unmodified block reads to
    pub fn with_source(mut self, source: Arc<dyn ImageAssetProvider>) -> Self {
        self.source = Some(source);
        self
    }

    /// Get the image configuration.
    pub fn config(&self) -> &MemoryImageConfig {
        &self.config
    }

    /// Get the set of block coordinates that have been provided via set_block().
    ///
    /// This is useful for determining which blocks have data when writing
    /// masked images with sparse block data.
    ///
    /// # Returns
    /// A HashSet containing (row, col) tuples for all provided blocks.
    pub fn provided_blocks(&self) -> HashSet<(u32, u32)> {
        self.provided_blocks
            .read()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Set block data at the given coordinates.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    /// * `data` - Raw pixel data in band-sequential (BSQ) format
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

        let mut provided = self.provided_blocks.write().map_err(|_| {
            CodecError::Decode("Failed to acquire write lock on provided_blocks".to_string())
        })?;

        blocks.insert((block_row, block_col), data.to_vec());
        provided.insert((block_row, block_col));
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

        // Split into blocks (data is already in BSQ format)
        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        let mut blocks = self.blocks.write().map_err(|_| {
            CodecError::Decode("Failed to acquire write lock on blocks".to_string())
        })?;

        let mut provided = self.provided_blocks.write().map_err(|_| {
            CodecError::Decode("Failed to acquire write lock on provided_blocks".to_string())
        })?;

        for block_row in 0..num_blocks_v {
            for block_col in 0..num_blocks_h {
                let block_data = self.extract_block_bsq(data, block_row, block_col)?;
                blocks.insert((block_row, block_col), block_data);
                provided.insert((block_row, block_col));
            }
        }

        Ok(())
    }

    /// Extract a block from the full BSQ image data, returning BSQ block data.
    fn extract_block_bsq(
        &self,
        bsq_data: &[u8],
        block_row: u32,
        block_col: u32,
    ) -> Result<Vec<u8>, CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let num_bands = self.config.num_bands as usize;
        let num_rows = self.config.num_rows as usize;
        let num_cols = self.config.num_columns as usize;
        let band_size = num_rows * num_cols * bytes_per_pixel;

        // Calculate block bounds
        let start_row = (block_row * self.config.block_height) as usize;
        let start_col = (block_col * self.config.block_width) as usize;
        let end_row = (start_row + self.config.block_height as usize).min(num_rows);
        let end_col = (start_col + self.config.block_width as usize).min(num_cols);

        let block_rows = end_row - start_row;
        let block_cols = end_col - start_col;
        let block_band_size = block_rows * block_cols * bytes_per_pixel;
        let block_size = num_bands * block_band_size;

        let mut block_data = vec![0u8; block_size];

        // Extract each band
        for band in 0..num_bands {
            let src_band_start = band * band_size;
            let dst_band_start = band * block_band_size;

            for (local_row, row) in (start_row..end_row).enumerate() {
                let src_row_start =
                    src_band_start + row * num_cols * bytes_per_pixel + start_col * bytes_per_pixel;
                let src_row_end = src_row_start + (end_col - start_col) * bytes_per_pixel;
                let dst_row_start = dst_band_start + local_row * block_cols * bytes_per_pixel;
                let dst_row_end = dst_row_start + (end_col - start_col) * bytes_per_pixel;

                block_data[dst_row_start..dst_row_end]
                    .copy_from_slice(&bsq_data[src_row_start..src_row_end]);
            }
        }

        Ok(block_data)
    }

    /// Get the raw image data in band-sequential format.
    fn get_raw_bsq(&self) -> Result<Vec<u8>, CodecError> {
        let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
        let num_bands = self.config.num_bands as usize;
        let num_rows = self.config.num_rows as usize;
        let num_cols = self.config.num_columns as usize;
        let band_size = num_rows * num_cols * bytes_per_pixel;
        let total_size = num_bands * band_size;

        let mut bsq_data = vec![0u8; total_size];

        let blocks = self
            .blocks
            .read()
            .map_err(|_| CodecError::Decode("Failed to acquire read lock on blocks".to_string()))?;

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

                    let block_rows = end_row - start_row;
                    let block_cols = end_col - start_col;
                    let block_band_size = block_rows * block_cols * bytes_per_pixel;

                    // Copy each band from block to full image
                    for band in 0..num_bands {
                        let src_band_start = band * block_band_size;
                        let dst_band_start = band * band_size;

                        for (local_row, row) in (start_row..end_row).enumerate() {
                            let src_row_start =
                                src_band_start + local_row * block_cols * bytes_per_pixel;
                            let src_row_end = src_row_start + block_cols * bytes_per_pixel;
                            let dst_row_start = dst_band_start
                                + row * num_cols * bytes_per_pixel
                                + start_col * bytes_per_pixel;
                            let dst_row_end = dst_row_start + block_cols * bytes_per_pixel;

                            if src_row_end <= block_data.len() && dst_row_end <= bsq_data.len() {
                                bsq_data[dst_row_start..dst_row_end]
                                    .copy_from_slice(&block_data[src_row_start..src_row_end]);
                            }
                        }
                    }
                }
            }
        }

        Ok(bsq_data)
    }
}

impl AssetMetadata for BufferedImageAssetProvider {
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

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        self.get_raw_bsq()
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }
}

impl ImageAssetProvider for BufferedImageAssetProvider {
    fn has_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
    ) -> Result<bool, CodecError> {
        if resolution_level != 0 {
            return Ok(false);
        }

        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        if block_row >= num_blocks_v || block_col >= num_blocks_h {
            return Ok(false);
        }

        // Check local overrides first
        let provided = self
            .provided_blocks
            .read()
            .map_err(|_| CodecError::Decode("poisoned lock on provided_blocks".into()))?;

        if provided.contains(&(block_row, block_col)) {
            return Ok(true);
        }

        // Fall back to source provider
        if let Some(ref source) = self.source {
            let src_bw = source.num_pixels_per_block_horizontal();
            let src_bh = source.num_pixels_per_block_vertical();
            if src_bw == self.config.block_width && src_bh == self.config.block_height {
                return source.has_block(block_row, block_col, resolution_level);
            }
            // Grids differ — use geometry check against source extent
            let assembler = TileAssembler::new(
                source.as_ref(),
                self.config.block_width,
                self.config.block_height,
            );
            return Ok(assembler.has_block(block_row, block_col));
        }

        Ok(false)
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        if resolution_level != 0 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        let num_blocks_h = self.config.num_blocks_horizontal();
        let num_blocks_v = self.config.num_blocks_vertical();

        if block_row >= num_blocks_v || block_col >= num_blocks_h {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Check local overrides first
        let has_local = {
            let blocks = self.blocks.read().map_err(|_| {
                CodecError::Decode("Failed to acquire read lock on blocks".to_string())
            })?;
            blocks.contains_key(&(block_row, block_col))
        };

        if has_local {
            // Serve from local block storage
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

            // If specific bands requested, extract them from BSQ data
            let output_data = if let Some(band_indices) = bands {
                let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
                let block_band_size = (block_rows * block_cols) as usize * bytes_per_pixel;
                let mut extracted = Vec::with_capacity(band_indices.len() * block_band_size);

                for &band in band_indices {
                    let src_band_start = (band as usize) * block_band_size;
                    let src_band_end = src_band_start + block_band_size;
                    if src_band_end <= block_data.len() {
                        extracted.extend_from_slice(&block_data[src_band_start..src_band_end]);
                    }
                }
                extracted
            } else {
                block_data.clone()
            };

            // Return shape as [bands, rows, cols] (CHW format)
            Ok((output_data, [num_bands, block_rows, block_cols]))
        } else if let Some(ref source) = self.source {
            let src_bw = source.num_pixels_per_block_horizontal();
            let src_bh = source.num_pixels_per_block_vertical();
            let (src_grid_rows, src_grid_cols) = source.block_grid_size();
            let block_within_source =
                block_row < src_grid_rows && block_col < src_grid_cols;

            let (all_band_data, shape) =
                if src_bw == self.config.block_width
                    && src_bh == self.config.block_height
                    && block_within_source
                {
                    source.get_block(block_row, block_col, resolution_level, None)?
                } else {
                    let assembler = TileAssembler::new(
                        source.as_ref(),
                        self.config.block_width,
                        self.config.block_height,
                    );

                    if !assembler.has_block(block_row, block_col) {
                        // Block is entirely outside source extent — return pad-filled buffer
                        let block_rows = self
                            .config
                            .block_height
                            .min(self.config.num_rows - block_row * self.config.block_height);
                        let block_cols = self
                            .config
                            .block_width
                            .min(self.config.num_columns - block_col * self.config.block_width);
                        let pad = pad_pixel_bytes(self.pad_pixel_value(), self.config.pixel_type);
                        let buf_size = (block_rows as usize)
                            * (block_cols as usize)
                            * (self.config.num_bands as usize)
                            * pad.len();
                        let mut buf = vec![0u8; buf_size];
                        if pad.iter().any(|&b| b != 0) {
                            for chunk in buf.chunks_exact_mut(pad.len()) {
                                chunk.copy_from_slice(&pad);
                            }
                        }
                        (buf, [self.config.num_bands, block_rows, block_cols])
                    } else {
                        let (tile_data, tile_shape) =
                            assembler.get_output_tile(block_row, block_col)?;
                        let [_bands, tile_rows, tile_cols] = tile_shape;

                        // Expected block dimensions for this position
                        let block_rows = self
                            .config
                            .block_height
                            .min(self.config.num_rows - block_row * self.config.block_height);
                        let block_cols = self
                            .config
                            .block_width
                            .min(self.config.num_columns - block_col * self.config.block_width);

                        if tile_rows == block_rows && tile_cols == block_cols {
                            (tile_data, tile_shape)
                        } else {
                            // Partial overlap — pad to expected block dimensions
                            let pad =
                                pad_pixel_bytes(self.pad_pixel_value(), self.config.pixel_type);
                            let bpp = pad.len();
                            let out_size = (block_rows as usize)
                                * (block_cols as usize)
                                * (self.config.num_bands as usize)
                                * bpp;
                            let mut buf = vec![0u8; out_size];
                            if pad.iter().any(|&b| b != 0) {
                                for chunk in buf.chunks_exact_mut(bpp) {
                                    chunk.copy_from_slice(&pad);
                                }
                            }
                            let out_band_size =
                                (block_rows as usize) * (block_cols as usize) * bpp;
                            let tile_band_size =
                                (tile_rows as usize) * (tile_cols as usize) * bpp;
                            for band in 0..self.config.num_bands as usize {
                                for row in 0..tile_rows as usize {
                                    let src_off = band * tile_band_size
                                        + row * (tile_cols as usize) * bpp;
                                    let dst_off = band * out_band_size
                                        + row * (block_cols as usize) * bpp;
                                    let copy_bytes = (tile_cols as usize) * bpp;
                                    buf[dst_off..dst_off + copy_bytes]
                                        .copy_from_slice(&tile_data[src_off..src_off + copy_bytes]);
                                }
                            }
                            (buf, [self.config.num_bands, block_rows, block_cols])
                        }
                    }
                };

            // Apply band subsetting if requested
            if let Some(band_indices) = bands {
                let [_, rows, cols] = shape;
                let bytes_per_pixel = self.config.pixel_type.bytes_per_pixel();
                let block_band_size = (rows as usize) * (cols as usize) * bytes_per_pixel;
                let mut extracted =
                    Vec::with_capacity(band_indices.len() * block_band_size);
                for &band in band_indices {
                    let src_band_start = (band as usize) * block_band_size;
                    let src_band_end = src_band_start + block_band_size;
                    if src_band_end <= all_band_data.len() {
                        extracted.extend_from_slice(&all_band_data[src_band_start..src_band_end]);
                    }
                }
                Ok((extracted, [band_indices.len() as u32, rows, cols]))
            } else {
                Ok((all_band_data, shape))
            }
        } else {
            Err(CodecError::Decode(format!(
                "Block ({}, {}) not found",
                block_row, block_col
            )))
        }
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

// Ensure BufferedImageAssetProvider is Send + Sync
unsafe impl Send for BufferedImageAssetProvider {}
unsafe impl Sync for BufferedImageAssetProvider {}

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
        let config = MemoryImageConfig::new(500, 300).with_block_size(256, 256);

        assert_eq!(config.num_blocks_horizontal(), 2);
        assert_eq!(config.num_blocks_vertical(), 2);
        assert_eq!(config.total_blocks(), 4);
    }

    #[test]
    fn test_provider_creation() {
        let config = MemoryImageConfig::new(512, 512);
        let provider = BufferedImageAssetProvider::new("test_image", config);

        assert_eq!(provider.key(), "test_image");
        assert_eq!(provider.num_rows(), 512);
        assert_eq!(provider.num_columns(), 512);
    }

    #[test]
    fn test_set_and_get_block() {
        let config = MemoryImageConfig::new(256, 256).with_block_size(256, 256);
        let provider = BufferedImageAssetProvider::new("test", config);

        // Create test data (1 band, 256x256 pixels, 1 byte per pixel) in BSQ format
        let block_data = vec![128u8; 256 * 256];
        provider.set_block(0, 0, &block_data).unwrap();

        assert!(provider.has_block(0, 0, 0).unwrap());

        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        // Shape is now [bands, rows, cols] (CHW format)
        assert_eq!(shape, [1, 256, 256]);
        assert_eq!(data.len(), 256 * 256);
    }

    #[test]
    fn test_with_metadata() {
        use crate::buffered::metadata::BufferedMetadataProvider;

        // Create a metadata provider with encoding hints
        let metadata = BufferedMetadataProvider::new();
        metadata.set("imode", serde_json::json!("P"));
        metadata.set("nppbh", serde_json::json!("256"));

        let config = MemoryImageConfig::new(512, 512);
        let provider =
            BufferedImageAssetProvider::new("test_image", config).with_metadata(Arc::new(metadata));

        // Verify metadata is accessible
        let meta = provider.metadata();
        let dict = meta.entries(None);
        assert_eq!(dict.get("imode"), Some(&serde_json::json!("P")));
        assert_eq!(dict.get("nppbh"), Some(&serde_json::json!("256")));
    }

    #[test]
    fn test_default_metadata_is_empty() {
        let config = MemoryImageConfig::new(512, 512);
        let provider = BufferedImageAssetProvider::new("test_image", config);

        // Default metadata should be empty
        let meta = provider.metadata();
        let dict = meta.entries(None);
        assert!(dict.is_empty());
    }
}

/// Property-based tests for BufferedImageAssetProvider metadata round-trip.
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::buffered::metadata::BufferedMetadataProvider;
    use proptest::prelude::*;

    /// Strategy for generating valid metadata keys (NITF field names).
    fn valid_metadata_key() -> impl Strategy<Value = String> {
        // NITF field names are typically uppercase alphanumeric, 1-10 chars
        "[A-Z][A-Z0-9]{0,9}".prop_map(|s| s.to_string())
    }

    /// Strategy for generating valid metadata values.
    fn valid_metadata_value() -> impl Strategy<Value = String> {
        // Values can be alphanumeric with some special chars, 1-20 chars
        "[A-Za-z0-9._-]{1,20}".prop_map(|s| s.to_string())
    }

    proptest! {
        /// Property 5: BufferedImageAssetProvider Metadata Round-Trip
        ///
        /// For any BufferedMetadataProvider M with key-value pairs, if a
        /// BufferedImageAssetProvider is created with M, then calling
        /// metadata().entries(None) on the provider SHALL return the same
        /// key-value pairs as M.entries(None).
        ///
        /// **Validates: Requirements 2.2**
        #[test]
        fn property_metadata_round_trip(
            pairs in prop::collection::vec((valid_metadata_key(), valid_metadata_value()), 1..10)
        ) {
            // Create metadata provider with random key-value pairs
            let metadata = BufferedMetadataProvider::new();
            for (key, value) in &pairs {
                metadata.set(key, serde_json::json!(value));
            }

            // Get the original dict before attaching to provider
            let original_dict = metadata.entries(None);

            // Create BufferedImageAssetProvider with the metadata
            let config = MemoryImageConfig::new(256, 256);
            let provider = BufferedImageAssetProvider::new("test_image", config)
                .with_metadata(Arc::new(metadata));

            // Get metadata back from provider
            let retrieved_dict = provider.metadata().entries(None);

            // Verify all key-value pairs are preserved
            prop_assert_eq!(
                original_dict.len(),
                retrieved_dict.len(),
                "Metadata dict should have same number of entries"
            );

            for (key, value) in &original_dict {
                prop_assert!(
                    retrieved_dict.contains_key(key),
                    "Retrieved metadata should contain key: {}", key
                );
                prop_assert_eq!(
                    retrieved_dict.get(key),
                    Some(value),
                    "Value for key {} should match", key
                );
            }
        }

        /// Property 5b: Multiple metadata providers are independent
        ///
        /// Creating multiple BufferedImageAssetProviders with different metadata
        /// should not affect each other's metadata.
        #[test]
        fn property_metadata_independence(
            pairs1 in prop::collection::vec((valid_metadata_key(), valid_metadata_value()), 1..5),
            pairs2 in prop::collection::vec((valid_metadata_key(), valid_metadata_value()), 1..5)
        ) {
            // Create first metadata provider
            let metadata1 = BufferedMetadataProvider::new();
            for (key, value) in &pairs1 {
                metadata1.set(key, serde_json::json!(value));
            }
            let original_dict1 = metadata1.entries(None);

            // Create second metadata provider
            let metadata2 = BufferedMetadataProvider::new();
            for (key, value) in &pairs2 {
                metadata2.set(key, serde_json::json!(value));
            }
            let original_dict2 = metadata2.entries(None);

            // Create two providers with different metadata
            let config = MemoryImageConfig::new(256, 256);
            let provider1 = BufferedImageAssetProvider::new("image1", config.clone())
                .with_metadata(Arc::new(metadata1));
            let provider2 = BufferedImageAssetProvider::new("image2", config)
                .with_metadata(Arc::new(metadata2));

            // Verify each provider has its own metadata
            let retrieved1 = provider1.metadata().entries(None);
            let retrieved2 = provider2.metadata().entries(None);

            prop_assert_eq!(
                original_dict1.len(),
                retrieved1.len(),
                "Provider 1 should have correct metadata count"
            );
            prop_assert_eq!(
                original_dict2.len(),
                retrieved2.len(),
                "Provider 2 should have correct metadata count"
            );

            // Verify values match for each provider
            for (key, value) in &original_dict1 {
                prop_assert_eq!(
                    retrieved1.get(key),
                    Some(value),
                    "Provider 1 value for key {} should match", key
                );
            }
            for (key, value) in &original_dict2 {
                prop_assert_eq!(
                    retrieved2.get(key),
                    Some(value),
                    "Provider 2 value for key {} should match", key
                );
            }
        }
    }
}

#[cfg(test)]
mod retiling_tests {
    use super::*;

    fn make_source(
        width: u32,
        height: u32,
        block_w: u32,
        block_h: u32,
        bands: u32,
        pixel_type: PixelType,
    ) -> BufferedImageAssetProvider {
        let config = MemoryImageConfig::new(width, height)
            .with_bands(bands)
            .with_block_size(block_w, block_h)
            .with_pixel_type(pixel_type);
        let provider = BufferedImageAssetProvider::new("source", config);

        let bpp = pixel_type.bytes_per_pixel();
        let num_blocks_h = width.div_ceil(block_w);
        let num_blocks_v = height.div_ceil(block_h);
        for br in 0..num_blocks_v {
            for bc in 0..num_blocks_h {
                let start_row = br * block_h;
                let start_col = bc * block_w;
                let end_row = (start_row + block_h).min(height);
                let end_col = (start_col + block_w).min(width);
                let brows = end_row - start_row;
                let bcols = end_col - start_col;
                let pixels = (brows as usize) * (bcols as usize);
                let mut data = vec![0u8; pixels * (bands as usize) * bpp];
                for band in 0..bands as usize {
                    for row in 0..brows as usize {
                        for col in 0..bcols as usize {
                            let img_x = start_col as usize + col;
                            let img_y = start_row as usize + row;
                            let val =
                                ((img_y * width as usize + img_x + band * 37) % 256) as u8;
                            let offset = band * pixels * bpp + row * (bcols as usize) * bpp
                                + col * bpp;
                            data[offset] = val;
                        }
                    }
                }
                provider.set_block(br, bc, &data).unwrap();
            }
        }
        provider
    }

    fn build_reference(width: u32, height: u32, bands: u32, bpp: usize) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let mut reference = vec![0u8; w * h * (bands as usize) * bpp];
        for band in 0..bands as usize {
            for row in 0..h {
                for col in 0..w {
                    let val = ((row * w + col + band * 37) % 256) as u8;
                    let offset = band * w * h * bpp + row * w * bpp + col * bpp;
                    reference[offset] = val;
                }
            }
        }
        reference
    }

    #[test]
    fn matching_grids_delegates_directly() {
        let source = Arc::new(make_source(16, 16, 4, 4, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(1)
            .with_block_size(4, 4)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source.clone());

        for row in 0..4u32 {
            for col in 0..4u32 {
                let (data, shape) = buffered.get_block(row, col, 0, None).unwrap();
                let (expected, expected_shape) = source.get_block(row, col, 0, None).unwrap();
                assert_eq!(shape, expected_shape);
                assert_eq!(data, expected);
            }
        }
    }

    #[test]
    fn mismatched_grids_assembles_correctly() {
        // Source: 4x4 blocks on 16x16 image
        // Buffered: 8x8 blocks on 16x16 image
        let source = Arc::new(make_source(16, 16, 4, 4, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(1)
            .with_block_size(8, 8)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let reference = build_reference(16, 16, 1, 1);

        for row in 0..2u32 {
            for col in 0..2u32 {
                let (data, shape) = buffered.get_block(row, col, 0, None).unwrap();
                assert_eq!(shape, [1, 8, 8]);
                for r in 0..8usize {
                    for c in 0..8usize {
                        let img_x = col as usize * 8 + c;
                        let img_y = row as usize * 8 + r;
                        let ref_offset = img_y * 16 + img_x;
                        let tile_offset = r * 8 + c;
                        assert_eq!(
                            data[tile_offset], reference[ref_offset],
                            "Mismatch at block ({}, {}), pixel ({}, {})",
                            row, col, r, c
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn mismatched_grids_smaller_output() {
        // Source: 8x8 blocks on 16x16 image
        // Buffered: 4x4 blocks on 16x16 image
        let source = Arc::new(make_source(16, 16, 8, 8, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(1)
            .with_block_size(4, 4)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let reference = build_reference(16, 16, 1, 1);

        for row in 0..4u32 {
            for col in 0..4u32 {
                let (data, shape) = buffered.get_block(row, col, 0, None).unwrap();
                assert_eq!(shape, [1, 4, 4]);
                for r in 0..4usize {
                    for c in 0..4usize {
                        let img_x = col as usize * 4 + c;
                        let img_y = row as usize * 4 + r;
                        let ref_offset = img_y * 16 + img_x;
                        let tile_offset = r * 4 + c;
                        assert_eq!(data[tile_offset], reference[ref_offset]);
                    }
                }
            }
        }
    }

    #[test]
    fn buffered_larger_than_source_full_outside() {
        // Source: 8x8 image, 4x4 blocks
        // Buffered: 16x16 image, 4x4 blocks → blocks beyond (2,2) are outside source
        let source = Arc::new(make_source(8, 8, 4, 4, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(1)
            .with_block_size(4, 4)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        // Block (3, 0) starts at y=12, source is only 8 tall → fully outside
        let (data, shape) = buffered.get_block(3, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 4, 4]);
        assert!(data.iter().all(|&b| b == 0)); // pad value is 0.0 → all zeros
    }

    #[test]
    fn buffered_larger_than_source_partial_overlap() {
        // Source: 10x10 image, 10x10 block (single block)
        // Buffered: 16x16 image, 8x8 blocks
        // Block (1,1) covers pixels (8,8)→(16,16) but source only has (0,0)→(10,10)
        // So overlap is (8,8)→(10,10) = 2x2 pixels, rest is pad
        let source = Arc::new(make_source(10, 10, 10, 10, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(1)
            .with_block_size(8, 8)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let reference = build_reference(10, 10, 1, 1);

        let (data, shape) = buffered.get_block(1, 1, 0, None).unwrap();
        assert_eq!(shape, [1, 8, 8]);

        // First 2 rows, 2 cols should have real data; rest should be zero (pad)
        for r in 0..8usize {
            for c in 0..8usize {
                let tile_offset = r * 8 + c;
                let img_x = 8 + c;
                let img_y = 8 + r;
                if img_x < 10 && img_y < 10 {
                    let ref_offset = img_y * 10 + img_x;
                    assert_eq!(
                        data[tile_offset], reference[ref_offset],
                        "Mismatch at pixel ({}, {})",
                        r, c
                    );
                } else {
                    assert_eq!(data[tile_offset], 0, "Expected pad at ({}, {})", r, c);
                }
            }
        }
    }

    #[test]
    fn buffered_larger_full_overlap() {
        // Source: 16x16 image, 8x8 blocks
        // Buffered: 16x16 image, 4x4 blocks (all blocks fully inside source)
        let source = Arc::new(make_source(16, 16, 8, 8, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(1)
            .with_block_size(4, 4)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let reference = build_reference(16, 16, 1, 1);

        // All blocks should be fully within source → no padding needed
        for row in 0..4u32 {
            for col in 0..4u32 {
                let (data, shape) = buffered.get_block(row, col, 0, None).unwrap();
                assert_eq!(shape, [1, 4, 4]);
                for r in 0..4usize {
                    for c in 0..4usize {
                        let img_x = col as usize * 4 + c;
                        let img_y = row as usize * 4 + r;
                        let ref_offset = img_y * 16 + img_x;
                        let tile_offset = r * 4 + c;
                        assert_eq!(data[tile_offset], reference[ref_offset]);
                    }
                }
            }
        }
    }

    #[test]
    fn nonzero_pad_value_uint16() {
        // Source: 4x4 image, 4x4 block
        // Buffered: 8x8 image, 4x4 blocks, UInt16 with pad=0 (default)
        // Block (1,1) is fully outside → all zeros
        let source = Arc::new(make_source(4, 4, 4, 4, 1, PixelType::UInt16));
        let config = MemoryImageConfig::new(8, 8)
            .with_bands(1)
            .with_block_size(4, 4)
            .with_pixel_type(PixelType::UInt16);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let (data, shape) = buffered.get_block(1, 1, 0, None).unwrap();
        assert_eq!(shape, [1, 4, 4]);
        assert_eq!(data.len(), 4 * 4 * 2); // 16 pixels × 2 bytes
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn band_subsetting_after_assembly() {
        // Source: 8x8 image, 4x4 blocks, 3 bands
        // Buffered: 8x8 image, 8x8 blocks (mismatch), 3 bands
        let source = Arc::new(make_source(8, 8, 4, 4, 3, PixelType::UInt8));
        let config = MemoryImageConfig::new(8, 8)
            .with_bands(3)
            .with_block_size(8, 8)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        // Request only band 1
        let (data, shape) = buffered.get_block(0, 0, 0, Some(&[1])).unwrap();
        assert_eq!(shape, [1, 8, 8]);
        assert_eq!(data.len(), 8 * 8);

        // Verify it matches band 1 from full assembly
        let (full_data, _) = buffered.get_block(0, 0, 0, None).unwrap();
        let band_size = 8 * 8;
        let band1_data = &full_data[band_size..2 * band_size];
        assert_eq!(data, band1_data);
    }

    #[test]
    fn local_override_takes_priority() {
        // Source: 8x8 image, 4x4 blocks
        // Buffered: 8x8 image, 8x8 blocks (mismatch)
        let source = Arc::new(make_source(8, 8, 4, 4, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(8, 8)
            .with_bands(1)
            .with_block_size(8, 8)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        // Set a local override for block (0, 0)
        let override_data = vec![255u8; 8 * 8];
        buffered.set_block(0, 0, &override_data).unwrap();

        let (data, shape) = buffered.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 8, 8]);
        assert_eq!(data, override_data);
    }

    #[test]
    fn multi_band_retiling() {
        // Source: 8x8 image, 4x4 blocks, 3 bands
        // Buffered: 8x8 image, 8x8 blocks, 3 bands
        let source = Arc::new(make_source(8, 8, 4, 4, 3, PixelType::UInt8));
        let config = MemoryImageConfig::new(8, 8)
            .with_bands(3)
            .with_block_size(8, 8)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let reference = build_reference(8, 8, 3, 1);

        let (data, shape) = buffered.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 8, 8]);
        assert_eq!(data, reference);
    }

    #[test]
    fn edge_blocks_non_divisible() {
        // Source: 10x10 image, 10x10 block (single block)
        // Buffered: 10x10 image, 4x4 blocks → edge blocks are 2 pixels wide/tall
        let source = Arc::new(make_source(10, 10, 10, 10, 1, PixelType::UInt8));
        let config = MemoryImageConfig::new(10, 10)
            .with_bands(1)
            .with_block_size(4, 4)
            .with_pixel_type(PixelType::UInt8);
        let buffered = BufferedImageAssetProvider::new("buffered", config)
            .with_source(source);

        let reference = build_reference(10, 10, 1, 1);

        // Bottom-right block (2, 2) covers pixels (8,8)→(10,10) = 2x2
        let (data, shape) = buffered.get_block(2, 2, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 2]);
        for r in 0..2usize {
            for c in 0..2usize {
                let img_x = 8 + c;
                let img_y = 8 + r;
                let ref_offset = img_y * 10 + img_x;
                let tile_offset = r * 2 + c;
                assert_eq!(data[tile_offset], reference[ref_offset]);
            }
        }
    }
}
