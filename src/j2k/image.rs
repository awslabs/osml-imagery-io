//! J2KImageAssetProvider — implements ImageAssetProvider for standalone JPEG 2000 images.
//!
//! Provides blocked/tiled access to JPEG 2000 image data. The file buffer is
//! retained as `Arc<[u8]>` with a byte range identifying the raw codestream
//! within it. Tiles are decoded on demand when `get_block()` is called.
//! The tile grid is derived from the SIZ marker.
//!
//! Supports multi-resolution decoding via the `resolution_level` parameter.

use std::ops::Range;
use std::sync::{Arc, OnceLock};

use crate::error::CodecError;
use crate::j2k::codec::{J2KCodec, J2KDecodeParams};
use crate::j2k::markers::{TilePartOffsetTable, scan_sot_markers};
use crate::j2k::metadata::J2KMetadataProvider;
use crate::traits::asset::AssetProvider;
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::types::{AssetType, PixelType};

/// Image asset provider for a standalone JPEG 2000 image.
///
/// Retains the file buffer and a byte range identifying the raw codestream
/// within it. Tiles are decoded on demand when `get_block()` is called.
/// For raw `.j2k` files the range spans the entire buffer; for `.jp2` files
/// it points to the contents of the `jp2c` box, avoiding a copy.
pub struct J2KImageAssetProvider {
    /// Unique key identifying this asset (always "image:0")
    key: String,
    /// Image width in pixels at full resolution
    width: u32,
    /// Image height in pixels at full resolution
    height: u32,
    /// Number of bands (components)
    num_bands: u32,
    /// Pixel data type
    pixel_type: PixelType,
    /// Bits per component sample
    bits_per_pixel: u8,
    /// Tile width in pixels
    tile_width: u32,
    /// Tile height in pixels
    tile_height: u32,
    /// Number of tiles horizontally
    num_tiles_x: u32,
    /// Number of tiles vertically
    num_tiles_y: u32,
    /// Shared file buffer (entire file bytes)
    buffer: Arc<[u8]>,
    /// Byte range within `buffer` that contains the raw J2K codestream
    codestream_range: Range<usize>,
    /// STAC-aligned roles (e.g., "data")
    roles: Vec<String>,
    /// Per-image metadata
    metadata: Arc<J2KMetadataProvider>,
    /// Number of resolution levels available in the codestream
    num_resolution_levels: u32,
    /// J2K codec for decoding tiles
    codec: Arc<dyn J2KCodec>,
    /// Decode header (main header with TLM stripped)
    decode_header: Vec<u8>,
    /// Byte offset of first SOT in the codestream
    first_sot_offset: u64,
    /// Tile-part offset table (lazy if no TLM)
    tile_part_table: OnceLock<TilePartOffsetTable>,
}

impl J2KImageAssetProvider {
    /// Create a new `J2KImageAssetProvider`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: String,
        width: u32,
        height: u32,
        num_bands: u32,
        pixel_type: PixelType,
        bits_per_pixel: u8,
        tile_width: u32,
        tile_height: u32,
        num_tiles_x: u32,
        num_tiles_y: u32,
        buffer: Arc<[u8]>,
        codestream_range: Range<usize>,
        roles: Vec<String>,
        metadata: Arc<J2KMetadataProvider>,
        num_resolution_levels: u32,
        codec: Arc<dyn J2KCodec>,
        decode_header: Vec<u8>,
        first_sot_offset: u64,
        tile_part_table: OnceLock<TilePartOffsetTable>,
    ) -> Self {
        Self {
            key,
            width,
            height,
            num_bands,
            pixel_type,
            bits_per_pixel,
            tile_width,
            tile_height,
            num_tiles_x,
            num_tiles_y,
            buffer,
            codestream_range,
            roles,
            metadata,
            num_resolution_levels,
            codec,
            decode_header,
            first_sot_offset,
            tile_part_table,
        }
    }

    /// Returns a reference to the raw codestream bytes within the shared buffer.
    #[inline]
    fn codestream(&self) -> &[u8] {
        &self.buffer[self.codestream_range.clone()]
    }

    /// Ensure the tile-part offset table is populated, triggering a SOT scan if needed.
    fn ensure_tile_part_table(&self) -> Result<&TilePartOffsetTable, CodecError> {
        if let Some(table) = self.tile_part_table.get() {
            return Ok(table);
        }
        let table = scan_sot_markers(self.codestream(), self.first_sot_offset)?;
        let _ = self.tile_part_table.set(table);
        Ok(self.tile_part_table.get().unwrap())
    }
}

// =============================================================================
// AssetProvider Implementation
// =============================================================================

impl AssetProvider for J2KImageAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.key
    }

    fn description(&self) -> &str {
        "JPEG 2000 image segment"
    }

    fn media_type(&self) -> &str {
        "image/jp2"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Image
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "raw_asset() not supported for J2K; use get_block()".to_string(),
        ))
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// =============================================================================
// ImageAssetProvider Implementation
// =============================================================================

impl ImageAssetProvider for J2KImageAssetProvider {
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        resolution_level < self.num_resolution_levels
            && block_row < self.num_tiles_y
            && block_col < self.num_tiles_x
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Validate resolution level
        if resolution_level >= self.num_resolution_levels {
            return Err(CodecError::InvalidResolutionLevel(resolution_level));
        }

        // Validate block coordinates
        if block_row >= self.num_tiles_y || block_col >= self.num_tiles_x {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Compute tile index (row-major order)
        let tile_index = block_row * self.num_tiles_x + block_col;

        // Decode the tile from the shared buffer (zero-copy slice)
        let params = J2KDecodeParams {
            resolution_level,
            region: None,
        };
        let result = self.codec.decode_tile(self.codestream(), tile_index, &params)?;

        let bps = self.pixel_type.bytes_per_pixel();
        let band_size = (result.width * result.height) as usize * bps;

        // Handle band subsetting
        let requested_bands = match bands {
            Some(b) => b.to_vec(),
            None => (0..self.num_bands).collect(),
        };
        let num_out_bands = requested_bands.len() as u32;

        // If all bands requested in order, return the full buffer
        if requested_bands.len() == self.num_bands as usize
            && requested_bands
                .iter()
                .enumerate()
                .all(|(i, &b)| b == i as u32)
        {
            return Ok((
                result.data,
                [num_out_bands, result.height, result.width],
            ));
        }

        // Band subsetting: extract only requested bands
        let mut output = Vec::with_capacity(requested_bands.len() * band_size);
        for &band in &requested_bands {
            let start = band as usize * band_size;
            let end = start + band_size;
            output.extend_from_slice(&result.data[start..end]);
        }

        Ok((output, [num_out_bands, result.height, result.width]))
    }

    fn num_resolution_levels(&self) -> u32 {
        self.num_resolution_levels
    }

    fn num_bands(&self) -> u32 {
        self.num_bands
    }

    fn num_rows(&self) -> u32 {
        self.height
    }

    fn num_columns(&self) -> u32 {
        self.width
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.tile_width
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.tile_height
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.bits_per_pixel as u32
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        (self.pixel_type.bytes_per_pixel() * 8) as u32
    }

    fn pixel_value_type(&self) -> PixelType {
        self.pixel_type
    }

    fn pad_pixel_value(&self) -> f64 {
        0.0
    }

    fn tile_byte_ranges(&self) -> Option<std::collections::HashMap<(u32, u32), Vec<(u64, u64)>>> {
        let table = self.ensure_tile_part_table().ok()?;
        let base_offset = self.codestream_range.start as u64;
        let mut ranges: std::collections::HashMap<(u32, u32), Vec<(u64, u64)>> =
            std::collections::HashMap::new();
        for entry in table {
            let row = entry.tile_index as u32 / self.num_tiles_x;
            let col = entry.tile_index as u32 % self.num_tiles_x;
            ranges
                .entry((row, col))
                .or_default()
                .push((base_offset + entry.offset, entry.length));
        }
        Some(ranges)
    }

    fn codec_configuration(&self) -> Option<std::collections::HashMap<String, Vec<u8>>> {
        let mut config = std::collections::HashMap::new();
        config.insert("main_header".to_string(), self.decode_header.clone());
        Some(config)
    }
}
