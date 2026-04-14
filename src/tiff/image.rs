//! TIFFImageAssetProvider — implements ImageAssetProvider for a single TIFF IFD.
//!
//! Handles both tiled and stripped layouts, chunky-to-BSQ deinterleaving,
//! and band subsetting.

use std::sync::{Arc, Mutex};

use crate::error::CodecError;
use crate::tiff::ffi::TiffHandle;
use crate::tiff::metadata::TIFFMetadataProvider;
use crate::tiff::tags;
use crate::traits::asset::AssetMetadata;
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::types::PixelType;

// =============================================================================
// Pixel Format Mapping
// =============================================================================

/// Map TIFF `(SampleFormat, BitsPerSample)` to `PixelType`.
///
/// Per TIFF 6.0 spec, absent `SampleFormat` defaults to unsigned integer (1).
/// Returns `CodecError::Unsupported` for unsupported combinations.
pub(crate) fn map_pixel_type(
    sample_format: Option<u16>,
    bits_per_sample: u16,
) -> Result<PixelType, CodecError> {
    let sf = sample_format.unwrap_or(tags::SAMPLE_FORMAT_UINT);

    match (sf, bits_per_sample) {
        (tags::SAMPLE_FORMAT_UINT, 8) => Ok(PixelType::UInt8),
        (tags::SAMPLE_FORMAT_UINT, 16) => Ok(PixelType::UInt16),
        (tags::SAMPLE_FORMAT_UINT, 32) => Ok(PixelType::UInt32),
        (tags::SAMPLE_FORMAT_INT, 8) => Ok(PixelType::Int8),
        (tags::SAMPLE_FORMAT_INT, 16) => Ok(PixelType::Int16),
        (tags::SAMPLE_FORMAT_INT, 32) => Ok(PixelType::Int32),
        (tags::SAMPLE_FORMAT_FLOAT, 32) => Ok(PixelType::Float32),
        (tags::SAMPLE_FORMAT_FLOAT, 64) => Ok(PixelType::Float64),
        _ => Err(CodecError::Unsupported(format!(
            "Unsupported pixel format: SampleFormat={}, BitsPerSample={}",
            sf, bits_per_sample
        ))),
    }
}

// =============================================================================
// TIFFImageAssetProvider
// =============================================================================

/// Image asset provider for a single TIFF IFD.
///
/// Provides blocked/tiled access to pixel data through the `ImageAssetProvider`
/// trait. Supports both tiled and stripped TIFF layouts, chunky-to-BSQ
/// deinterleaving, and band subsetting.
pub struct TIFFImageAssetProvider {
    /// Unique key identifying this asset (e.g., "image:0")
    key: String,
    /// IFD index within the TIFF file
    ifd_index: u16,
    /// Image width in pixels
    width: u32,
    /// Image height in pixels
    height: u32,
    /// Number of bands (SamplesPerPixel)
    bands: u32,
    /// Bits per sample
    bits_per_sample: u32,
    /// Pixel data type
    pixel_type: PixelType,
    /// Whether the IFD is tiled (vs stripped)
    is_tiled: bool,
    /// Block width: TileWidth for tiled, ImageWidth for stripped
    block_width: u32,
    /// Block height: TileLength for tiled, RowsPerStrip for stripped
    block_height: u32,
    /// Planar configuration: CONTIG (1) or SEPARATE (2)
    planar_config: u16,
    /// Compression type (tag 259)
    compression: u16,
    /// Shared libtiff handle (Arc for thread-safe sharing between providers)
    handle: Arc<Mutex<TiffHandle>>,
    /// STAC-aligned roles (e.g., "data", "overview")
    roles: Vec<String>,
    /// Per-IFD metadata
    metadata: Arc<TIFFMetadataProvider>,
}

impl TIFFImageAssetProvider {
    /// Create a new TIFFImageAssetProvider by reading tags from the given IFD.
    ///
    /// The handle's current directory is set to `ifd_index` to read tags.
    /// The caller must hold the mutex lock when calling this.
    pub fn new(
        key: String,
        ifd_index: u16,
        handle: Arc<Mutex<TiffHandle>>,
        metadata: Arc<TIFFMetadataProvider>,
        roles: Vec<String>,
    ) -> Result<Self, CodecError> {
        let guard = handle.lock().map_err(|e| {
            CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
        })?;

        guard.set_directory(ifd_index)?;

        let width = guard.get_field_u32(tags::IMAGE_WIDTH)?;
        let height = guard.get_field_u32(tags::IMAGE_LENGTH)?;
        let bits_per_sample = guard.get_field_u16(tags::BITS_PER_SAMPLE)? as u32;
        let bands = guard.get_field_u16(tags::SAMPLES_PER_PIXEL).unwrap_or(1) as u32;

        let sample_format = guard.get_field_u16(tags::SAMPLE_FORMAT).ok();
        let pixel_type = map_pixel_type(sample_format, bits_per_sample as u16)?;

        let planar_config = guard
            .get_field_u16(tags::PLANAR_CONFIGURATION)
            .unwrap_or(tags::PLANAR_CONFIG_CONTIG);

        let compression = guard
            .get_field_u16(tags::COMPRESSION)
            .unwrap_or(tags::COMPRESSION_NONE);

        let is_tiled = guard.is_tiled();

        let (block_width, block_height) = if is_tiled {
            let tw = guard.get_field_u32(tags::TILE_WIDTH)?;
            let tl = guard.get_field_u32(tags::TILE_LENGTH)?;
            (tw, tl)
        } else {
            let rps = guard.get_field_u32(tags::ROWS_PER_STRIP).unwrap_or(height);
            (width, rps)
        };

        drop(guard);

        Ok(Self {
            key,
            ifd_index,
            width,
            height,
            bands,
            bits_per_sample,
            pixel_type,
            is_tiled,
            block_width,
            block_height,
            planar_config,
            compression,
            handle,
            roles,
            metadata,
        })
    }

    /// Bytes per sample (pixel component).
    fn bytes_per_sample(&self) -> usize {
        (self.bits_per_sample as usize).div_ceil(8)
    }

    /// Number of pixels in a full block.
    fn pixels_per_block(&self) -> usize {
        self.block_width as usize * self.block_height as usize
    }
}

// =============================================================================
// AssetMetadata Implementation
// =============================================================================

impl AssetMetadata for TIFFImageAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.key
    }

    fn description(&self) -> &str {
        "TIFF image segment"
    }

    fn media_type(&self) -> &str {
        "image/tiff"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        // Not meaningful for TIFF — callers should use get_block()
        Err(CodecError::Unsupported(
            "raw_asset() not supported for TIFF; use get_block()".to_string(),
        ))
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }
}

// =============================================================================
// ImageAssetProvider Implementation
// =============================================================================

impl ImageAssetProvider for TIFFImageAssetProvider {
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        if resolution_level > 0 {
            return false;
        }
        let (grid_rows, grid_cols) = self.block_grid_size();
        block_row < grid_rows && block_col < grid_cols
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Validate resolution level
        if resolution_level > 0 {
            return Err(CodecError::InvalidResolutionLevel(resolution_level));
        }

        // Validate block coordinates
        let (grid_rows, grid_cols) = self.block_grid_size();
        if block_row >= grid_rows || block_col >= grid_cols {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        if self.is_tiled {
            self.get_block_tiled(block_row, block_col, bands)
        } else {
            self.get_block_stripped(block_row, bands)
        }
    }

    fn num_resolution_levels(&self) -> u32 {
        1
    }

    fn num_bands(&self) -> u32 {
        self.bands
    }

    fn num_rows(&self) -> u32 {
        self.height
    }

    fn num_columns(&self) -> u32 {
        self.width
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.block_width
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.block_height
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.bits_per_sample
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        self.bits_per_sample
    }

    fn pixel_value_type(&self) -> PixelType {
        self.pixel_type
    }

    fn pad_pixel_value(&self) -> f64 {
        0.0
    }

    fn tile_byte_ranges(&self) -> Option<std::collections::HashMap<(u32, u32), Vec<(u64, u64)>>> {
        let guard = self.handle.lock().ok()?;
        guard.set_directory(self.ifd_index).ok()?;

        let (offset_tag, count_tag, num_entries) = if self.is_tiled {
            (tags::TILE_OFFSETS, tags::TILE_BYTE_COUNTS, guard.number_of_tiles())
        } else {
            (tags::STRIP_OFFSETS, tags::STRIP_BYTE_COUNTS, guard.number_of_strips())
        };

        if num_entries == 0 {
            return None;
        }

        let offsets = guard.get_field_u64_ptr(offset_tag, num_entries).ok()?;
        let counts = guard.get_field_u64_ptr(count_tag, num_entries).ok()?;

        if offsets.len() != counts.len() {
            return None;
        }

        // For contig planar config, tiles are indexed linearly across the grid.
        // For separate planar config, there are bands × grid_tiles entries;
        // we expose only the first band's tiles (band 0) since the tile index
        // maps to a single BSQ array where all bands share the same spatial grid.
        let (grid_rows, grid_cols) = self.block_grid_size();
        let tiles_per_band = (grid_rows * grid_cols) as usize;

        let mut ranges = std::collections::HashMap::with_capacity(tiles_per_band);
        for idx in 0..tiles_per_band {
            if idx >= offsets.len() {
                break;
            }
            let row = idx as u32 / grid_cols;
            let col = idx as u32 % grid_cols;
            // TIFF offsets are already file-relative (no translation needed)
            ranges.insert((row, col), vec![(offsets[idx], counts[idx])]);
        }

        Some(ranges)
    }

    fn codec_configuration(&self) -> Option<std::collections::HashMap<String, Vec<u8>>> {
        // For lossless compression types, no configuration needed
        // COMPRESSION_NONE=1, COMPRESSION_LZW=5, COMPRESSION_DEFLATE=8, COMPRESSION_ADOBE_DEFLATE=32946
        match self.compression {
            1 | 5 | 8 | 32946 => None,
            // For JPEG and other compression types, configuration would be needed
            // but requires additional FFI support to read JPEGTables tag.
            // Return None for now.
            _ => None,
        }
    }
}

// =============================================================================
// Block Reading Implementation
// =============================================================================

impl TIFFImageAssetProvider {
    /// Read a block from a tiled TIFF.
    fn get_block_tiled(
        &self,
        block_row: u32,
        block_col: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        let bps = self.bytes_per_sample();
        let tiles_across = self.width.div_ceil(self.block_width);

        // Actual pixel dimensions for this block (edge blocks may be smaller)
        let actual_rows = std::cmp::min(
            self.block_height,
            self.height.saturating_sub(block_row * self.block_height),
        );
        let actual_cols = std::cmp::min(
            self.block_width,
            self.width.saturating_sub(block_col * self.block_width),
        );

        let guard = self.handle.lock().map_err(|e| {
            CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
        })?;
        guard.set_directory(self.ifd_index)?;

        // For JPEG-compressed YCbCr images, tell libtiff to convert to RGB
        // on decode. This treats the YCbCr↔RGB conversion as an internal
        // codec detail — callers always see RGB pixels.
        if self.compression == tags::COMPRESSION_JPEG && self.bands >= 3 {
            guard.set_field_u32(tags::TIFFTAG_JPEGCOLORMODE, tags::JPEGCOLORMODE_RGB as u32)?;
        }

        let requested_bands = self.resolve_bands(bands);
        let num_out_bands = requested_bands.len() as u32;

        let bsq_data = if self.planar_config == tags::PLANAR_CONFIG_CONTIG {
            // Chunky: one tile contains all bands interleaved
            let tile_index = block_row * tiles_across + block_col;
            let raw = guard.read_encoded_tile(tile_index)?;
            drop(guard);

            deinterleave_chunky_to_bsq(
                &raw,
                self.block_width,
                self.block_height,
                actual_cols,
                actual_rows,
                self.bands,
                bps,
                &requested_bands,
            )
        } else {
            // Planar: separate tile per band
            let tiles_per_band = tiles_across
                * self.height.div_ceil(self.block_height);
            let base_tile = block_row * tiles_across + block_col;

            let mut bsq = Vec::with_capacity(
                num_out_bands as usize * actual_rows as usize * actual_cols as usize * bps,
            );

            for &band in &requested_bands {
                let tile_index = base_tile + band * tiles_per_band;
                let raw = guard.read_encoded_tile(tile_index)?;

                // Extract actual pixels from the (possibly padded) tile
                extract_actual_pixels(
                    &raw,
                    self.block_width,
                    actual_cols,
                    actual_rows,
                    1, // single band per tile in planar mode
                    bps,
                    &mut bsq,
                );
            }
            drop(guard);
            bsq
        };

        Ok((bsq_data, [num_out_bands, actual_rows, actual_cols]))
    }

    /// Read a block from a stripped TIFF.
    fn get_block_stripped(
        &self,
        block_row: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        let bps = self.bytes_per_sample();

        // Actual rows in this strip (last strip may be shorter)
        let actual_rows = std::cmp::min(
            self.block_height,
            self.height.saturating_sub(block_row * self.block_height),
        );
        let actual_cols = self.width;

        let guard = self.handle.lock().map_err(|e| {
            CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
        })?;
        guard.set_directory(self.ifd_index)?;

        // For JPEG-compressed YCbCr images, tell libtiff to convert to RGB
        // on decode. This treats the YCbCr↔RGB conversion as an internal
        // codec detail — callers always see RGB pixels.
        if self.compression == tags::COMPRESSION_JPEG && self.bands >= 3 {
            guard.set_field_u32(tags::TIFFTAG_JPEGCOLORMODE, tags::JPEGCOLORMODE_RGB as u32)?;
        }

        let requested_bands = self.resolve_bands(bands);
        let num_out_bands = requested_bands.len() as u32;

        let bsq_data = if self.planar_config == tags::PLANAR_CONFIG_CONTIG {
            let raw = guard.read_encoded_strip(block_row)?;
            drop(guard);

            // For strips, block_width == actual_cols == ImageWidth, no column padding
            deinterleave_chunky_to_bsq(
                &raw,
                self.block_width,
                self.block_height,
                actual_cols,
                actual_rows,
                self.bands,
                bps,
                &requested_bands,
            )
        } else {
            // Planar: separate strip per band
            let strips_per_band =
                self.height.div_ceil(self.block_height);

            let mut bsq = Vec::with_capacity(
                num_out_bands as usize * actual_rows as usize * actual_cols as usize * bps,
            );

            for &band in &requested_bands {
                let strip_index = band * strips_per_band + block_row;
                let raw = guard.read_encoded_strip(strip_index)?;

                // For strips, no column padding — just take actual_rows worth of data
                let row_bytes = actual_cols as usize * bps;
                let take = actual_rows as usize * row_bytes;
                let take = std::cmp::min(take, raw.len());
                bsq.extend_from_slice(&raw[..take]);
            }
            drop(guard);
            bsq
        };

        Ok((bsq_data, [num_out_bands, actual_rows, actual_cols]))
    }

    /// Resolve the band selection: if None, return all bands [0..N).
    fn resolve_bands(&self, bands: Option<&[u32]>) -> Vec<u32> {
        match bands {
            Some(b) => b.to_vec(),
            None => (0..self.bands).collect(),
        }
    }
}

// =============================================================================
// Deinterleaving Helpers
// =============================================================================

/// Deinterleave chunky (RGBRGB...) data to band-sequential (RRR...GGG...BBB...).
///
/// Handles edge blocks where `actual_cols < block_width` or `actual_rows < block_height`
/// by extracting only the valid pixel region from the padded tile/strip data.
fn deinterleave_chunky_to_bsq(
    raw: &[u8],
    block_width: u32,
    _block_height: u32,
    actual_cols: u32,
    actual_rows: u32,
    total_bands: u32,
    bytes_per_sample: usize,
    requested_bands: &[u32],
) -> Vec<u8> {
    let num_out_bands = requested_bands.len();
    let out_pixels = actual_rows as usize * actual_cols as usize;
    let mut bsq = vec![0u8; num_out_bands * out_pixels * bytes_per_sample];

    let src_pixel_stride = total_bands as usize * bytes_per_sample;
    let src_row_stride = block_width as usize * src_pixel_stride;

    for row in 0..actual_rows as usize {
        for col in 0..actual_cols as usize {
            let src_offset = row * src_row_stride + col * src_pixel_stride;
            let dst_pixel_idx = row * actual_cols as usize + col;

            for (out_band_idx, &band) in requested_bands.iter().enumerate() {
                let src_start = src_offset + band as usize * bytes_per_sample;
                let dst_start =
                    out_band_idx * out_pixels * bytes_per_sample + dst_pixel_idx * bytes_per_sample;

                if src_start + bytes_per_sample <= raw.len()
                    && dst_start + bytes_per_sample <= bsq.len()
                {
                    bsq[dst_start..dst_start + bytes_per_sample]
                        .copy_from_slice(&raw[src_start..src_start + bytes_per_sample]);
                }
            }
        }
    }

    bsq
}

/// Extract actual pixels from a (possibly padded) planar tile.
///
/// In planar mode, each tile contains data for a single band. The tile may be
/// padded to `block_width × block_height`, but we only want `actual_cols × actual_rows`.
fn extract_actual_pixels(
    raw: &[u8],
    block_width: u32,
    actual_cols: u32,
    actual_rows: u32,
    _bands_in_tile: u32,
    bytes_per_sample: usize,
    output: &mut Vec<u8>,
) {
    let src_row_stride = block_width as usize * bytes_per_sample;
    let dst_row_bytes = actual_cols as usize * bytes_per_sample;

    for row in 0..actual_rows as usize {
        let src_start = row * src_row_stride;
        let src_end = src_start + dst_row_bytes;
        if src_end <= raw.len() {
            output.extend_from_slice(&raw[src_start..src_end]);
        } else if src_start < raw.len() {
            // Partial row at end of data
            output.extend_from_slice(&raw[src_start..]);
            // Pad remaining with zeros
            let missing = dst_row_bytes - (raw.len() - src_start);
            output.extend(std::iter::repeat_n(0u8, missing));
        } else {
            // No data for this row, pad with zeros
            output.extend(std::iter::repeat_n(0u8, dst_row_bytes));
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Pixel Format Mapping Tests
    // =========================================================================

    #[test]
    fn test_map_pixel_type_uint8() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_UINT), 8).unwrap(),
            PixelType::UInt8
        );
    }

    #[test]
    fn test_map_pixel_type_uint16() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_UINT), 16).unwrap(),
            PixelType::UInt16
        );
    }

    #[test]
    fn test_map_pixel_type_uint32() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_UINT), 32).unwrap(),
            PixelType::UInt32
        );
    }

    #[test]
    fn test_map_pixel_type_int8() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_INT), 8).unwrap(),
            PixelType::Int8
        );
    }

    #[test]
    fn test_map_pixel_type_int16() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_INT), 16).unwrap(),
            PixelType::Int16
        );
    }

    #[test]
    fn test_map_pixel_type_int32() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_INT), 32).unwrap(),
            PixelType::Int32
        );
    }

    #[test]
    fn test_map_pixel_type_float32() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_FLOAT), 32).unwrap(),
            PixelType::Float32
        );
    }

    #[test]
    fn test_map_pixel_type_float64() {
        assert_eq!(
            map_pixel_type(Some(tags::SAMPLE_FORMAT_FLOAT), 64).unwrap(),
            PixelType::Float64
        );
    }

    #[test]
    fn test_map_pixel_type_absent_defaults_to_uint() {
        // Per TIFF 6.0 spec, absent SampleFormat defaults to unsigned integer
        assert_eq!(map_pixel_type(None, 8).unwrap(), PixelType::UInt8);
        assert_eq!(map_pixel_type(None, 16).unwrap(), PixelType::UInt16);
        assert_eq!(map_pixel_type(None, 32).unwrap(), PixelType::UInt32);
    }

    #[test]
    fn test_map_pixel_type_unsupported_returns_error() {
        // Float with 8 bits is not supported
        let result = map_pixel_type(Some(tags::SAMPLE_FORMAT_FLOAT), 8);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::Unsupported(msg) => {
                assert!(msg.contains("SampleFormat=3"));
                assert!(msg.contains("BitsPerSample=8"));
            }
            other => panic!("Expected Unsupported, got: {:?}", other),
        }
    }

    #[test]
    fn test_map_pixel_type_unsupported_uint64() {
        let result = map_pixel_type(Some(tags::SAMPLE_FORMAT_UINT), 64);
        assert!(result.is_err());
    }

    #[test]
    fn test_map_pixel_type_unsupported_int64() {
        let result = map_pixel_type(Some(tags::SAMPLE_FORMAT_INT), 64);
        assert!(result.is_err());
    }

    // =========================================================================
    // Deinterleaving Tests
    // =========================================================================

    #[test]
    fn test_deinterleave_chunky_to_bsq_3band_2x2() {
        // 2x2 image, 3 bands, 1 byte per sample, chunky layout
        // Input: R0 G0 B0 R1 G1 B1 R2 G2 B2 R3 G3 B3
        let raw = vec![
            1, 2, 3, // pixel (0,0): R=1, G=2, B=3
            4, 5, 6, // pixel (0,1): R=4, G=5, B=6
            7, 8, 9, // pixel (1,0): R=7, G=8, B=9
            10, 11, 12, // pixel (1,1): R=10, G=11, B=12
        ];

        let result = deinterleave_chunky_to_bsq(
            &raw,
            2,                // block_width
            2,                // block_height
            2,                // actual_cols
            2,                // actual_rows
            3,                // total_bands
            1,                // bytes_per_sample
            &[0, 1, 2],      // all bands
        );

        // Expected BSQ: R0 R1 R2 R3 | G0 G1 G2 G3 | B0 B1 B2 B3
        assert_eq!(result, vec![1, 4, 7, 10, 2, 5, 8, 11, 3, 6, 9, 12]);
    }

    #[test]
    fn test_deinterleave_chunky_to_bsq_band_subset() {
        // 2x2 image, 3 bands, request only band 0 and 2
        let raw = vec![
            1, 2, 3,
            4, 5, 6,
            7, 8, 9,
            10, 11, 12,
        ];

        let result = deinterleave_chunky_to_bsq(
            &raw, 2, 2, 2, 2, 3, 1, &[0, 2],
        );

        // Expected: R0 R1 R2 R3 | B0 B1 B2 B3
        assert_eq!(result, vec![1, 4, 7, 10, 3, 6, 9, 12]);
    }

    #[test]
    fn test_deinterleave_chunky_single_band() {
        // 2x2 image, 1 band — no deinterleaving needed
        let raw = vec![10, 20, 30, 40];

        let result = deinterleave_chunky_to_bsq(
            &raw, 2, 2, 2, 2, 1, 1, &[0],
        );

        assert_eq!(result, vec![10, 20, 30, 40]);
    }

    #[test]
    fn test_deinterleave_chunky_16bit() {
        // 1x2 image, 2 bands, 2 bytes per sample (uint16)
        // Pixel (0,0): band0=0x0102, band1=0x0304
        // Pixel (0,1): band0=0x0506, band1=0x0708
        let raw = vec![
            0x01, 0x02, 0x03, 0x04, // pixel (0,0)
            0x05, 0x06, 0x07, 0x08, // pixel (0,1)
        ];

        let result = deinterleave_chunky_to_bsq(
            &raw, 2, 1, 2, 1, 2, 2, &[0, 1],
        );

        // Expected BSQ: band0_px0 band0_px1 | band1_px0 band1_px1
        assert_eq!(
            result,
            vec![0x01, 0x02, 0x05, 0x06, 0x03, 0x04, 0x07, 0x08]
        );
    }

    // =========================================================================
    // Block Coordinate Validation Tests (using real TIFF data)
    // =========================================================================

    /// Build a minimal valid stripped TIFF: 4x4 pixels, 8-bit grayscale,
    /// uncompressed, 2 rows per strip (2 strips total).
    fn make_stripped_tiff() -> Vec<u8> {
        let width: u32 = 4;
        let height: u32 = 4;
        let rows_per_strip: u32 = 2;
        let strip_bytes = width * rows_per_strip;

        let mut buf = Vec::new();

        // TIFF Header
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes()); // IFD at offset 8

        // IFD: 11 entries
        let num_entries: u16 = 11;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        let short_type: u16 = 3;
        let long_type: u16 = 4;

        // Calculate offsets
        let ifd_size = 2 + num_entries as u32 * 12 + 4;
        let strip_offsets_offset = 8 + ifd_size;
        let strip_byte_counts_offset = strip_offsets_offset + 8; // 2 u32 values
        let pixel_data_offset = strip_byte_counts_offset + 8;

        // Tag entries (must be in ascending tag order)
        write_ifd_entry(&mut buf, 256, short_type, 1, width);       // ImageWidth
        write_ifd_entry(&mut buf, 257, short_type, 1, height);      // ImageLength
        write_ifd_entry(&mut buf, 258, short_type, 1, 8);           // BitsPerSample
        write_ifd_entry(&mut buf, 259, short_type, 1, 1);           // Compression=None
        write_ifd_entry(&mut buf, 262, short_type, 1, 1);           // PhotometricInterpretation
        write_ifd_entry(&mut buf, 273, long_type, 2, strip_offsets_offset); // StripOffsets
        write_ifd_entry(&mut buf, 277, short_type, 1, 1);           // SamplesPerPixel
        write_ifd_entry(&mut buf, 278, short_type, 1, rows_per_strip); // RowsPerStrip
        write_ifd_entry(&mut buf, 279, long_type, 2, strip_byte_counts_offset); // StripByteCounts
        write_ifd_entry(&mut buf, 284, short_type, 1, 1);           // PlanarConfiguration=Contig
        write_ifd_entry(&mut buf, 339, short_type, 1, 1);           // SampleFormat=UInt

        // Next IFD offset = 0
        buf.extend_from_slice(&0u32.to_le_bytes());

        // StripOffsets: 2 strips
        buf.extend_from_slice(&pixel_data_offset.to_le_bytes());
        buf.extend_from_slice(&(pixel_data_offset + strip_bytes).to_le_bytes());

        // StripByteCounts: 2 strips
        buf.extend_from_slice(&strip_bytes.to_le_bytes());
        buf.extend_from_slice(&strip_bytes.to_le_bytes());

        // Pixel data: 16 bytes (4x4, 1 byte each)
        // Strip 0: rows 0-1
        buf.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        // Strip 1: rows 2-3
        buf.extend_from_slice(&[9, 10, 11, 12, 13, 14, 15, 16]);

        buf
    }

    fn write_ifd_entry(buf: &mut Vec<u8>, tag: u16, dtype: u16, count: u32, value: u32) {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&dtype.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn test_stripped_provider_dimensions() {
        let data = make_stripped_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        assert_eq!(provider.num_columns(), 4);
        assert_eq!(provider.num_rows(), 4);
        assert_eq!(provider.num_bands(), 1);
        assert_eq!(provider.pixel_value_type(), PixelType::UInt8);
        assert_eq!(provider.num_pixels_per_block_horizontal(), 4);
        assert_eq!(provider.num_pixels_per_block_vertical(), 2);
        assert_eq!(provider.num_resolution_levels(), 1);
        assert_eq!(provider.block_grid_size(), (2, 1));
    }

    #[test]
    fn test_stripped_provider_has_block() {
        let data = make_stripped_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        // Valid blocks
        assert!(provider.has_block(0, 0, 0));
        assert!(provider.has_block(1, 0, 0));

        // Out of bounds
        assert!(!provider.has_block(2, 0, 0));
        assert!(!provider.has_block(0, 1, 0));

        // Invalid resolution level
        assert!(!provider.has_block(0, 0, 1));
    }

    #[test]
    fn test_stripped_provider_get_block_valid() {
        let data = make_stripped_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        // Read strip 0 (rows 0-1)
        let (block_data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 4]);
        assert_eq!(block_data, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        // Read strip 1 (rows 2-3)
        let (block_data, shape) = provider.get_block(1, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 4]);
        assert_eq!(block_data, vec![9, 10, 11, 12, 13, 14, 15, 16]);
    }

    #[test]
    fn test_stripped_provider_get_block_out_of_bounds() {
        let data = make_stripped_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        let result = provider.get_block(2, 0, 0, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::InvalidBlockCoordinates(r, c, l) => {
                assert_eq!(r, 2);
                assert_eq!(c, 0);
                assert_eq!(l, 0);
            }
            other => panic!("Expected InvalidBlockCoordinates, got: {:?}", other),
        }
    }

    #[test]
    fn test_stripped_provider_get_block_invalid_resolution() {
        let data = make_stripped_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        let result = provider.get_block(0, 0, 1, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::InvalidResolutionLevel(l) => assert_eq!(l, 1),
            other => panic!("Expected InvalidResolutionLevel, got: {:?}", other),
        }
    }

    // =========================================================================
    // tile_byte_ranges() Tests
    // =========================================================================

    #[test]
    fn test_stripped_tile_byte_ranges() {
        let data = make_stripped_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        let ranges = provider.tile_byte_ranges();
        assert!(ranges.is_some(), "tile_byte_ranges() should return Some for stripped TIFF");

        let ranges = ranges.unwrap();
        // 2 strips → (0,0) and (1,0)
        assert_eq!(ranges.len(), 2);
        assert!(ranges.contains_key(&(0, 0)));
        assert!(ranges.contains_key(&(1, 0)));

        // Each strip should have a single-element Vec
        assert_eq!(ranges[&(0, 0)].len(), 1);
        assert_eq!(ranges[&(1, 0)].len(), 1);

        // Each strip is 4 pixels wide × 2 rows × 1 byte = 8 bytes
        let (_, len0) = ranges[&(0, 0)][0];
        let (_, len1) = ranges[&(1, 0)][0];
        assert_eq!(len0, 8);
        assert_eq!(len1, 8);

        // Strip 1 offset should be strip 0 offset + strip 0 length
        let (off0, _) = ranges[&(0, 0)][0];
        let (off1, _) = ranges[&(1, 0)][0];
        assert_eq!(off1, off0 + len0);
    }

    /// Build a minimal valid tiled TIFF: 4x4 pixels, 8-bit grayscale,
    /// uncompressed, 2x2 tiles (4 tiles total).
    fn make_tiled_tiff() -> Vec<u8> {
        let width: u32 = 4;
        let height: u32 = 4;
        let tile_width: u32 = 2;
        let tile_height: u32 = 2;
        let tile_bytes = tile_width * tile_height; // 4 bytes per tile

        let mut buf = Vec::new();

        // TIFF Header
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes()); // IFD at offset 8

        // IFD: 12 entries
        let num_entries: u16 = 12;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        let short_type: u16 = 3;
        let long_type: u16 = 4;

        // Calculate offsets
        let ifd_size = 2 + num_entries as u32 * 12 + 4;
        let tile_offsets_offset = 8 + ifd_size;
        let tile_byte_counts_offset = tile_offsets_offset + 16; // 4 u32 values
        let pixel_data_offset = tile_byte_counts_offset + 16;

        // Tag entries (must be in ascending tag order)
        write_ifd_entry(&mut buf, 256, short_type, 1, width);       // ImageWidth
        write_ifd_entry(&mut buf, 257, short_type, 1, height);      // ImageLength
        write_ifd_entry(&mut buf, 258, short_type, 1, 8);           // BitsPerSample
        write_ifd_entry(&mut buf, 259, short_type, 1, 1);           // Compression=None
        write_ifd_entry(&mut buf, 262, short_type, 1, 1);           // PhotometricInterpretation
        write_ifd_entry(&mut buf, 277, short_type, 1, 1);           // SamplesPerPixel
        write_ifd_entry(&mut buf, 284, short_type, 1, 1);           // PlanarConfiguration=Contig
        write_ifd_entry(&mut buf, 322, short_type, 1, tile_width);  // TileWidth
        write_ifd_entry(&mut buf, 323, short_type, 1, tile_height); // TileLength
        write_ifd_entry(&mut buf, 324, long_type, 4, tile_offsets_offset); // TileOffsets
        write_ifd_entry(&mut buf, 325, long_type, 4, tile_byte_counts_offset); // TileByteCounts
        write_ifd_entry(&mut buf, 339, short_type, 1, 1);           // SampleFormat=UInt

        // Next IFD offset = 0
        buf.extend_from_slice(&0u32.to_le_bytes());

        // TileOffsets: 4 tiles (row-major: [0,0], [0,1], [1,0], [1,1])
        for i in 0..4u32 {
            buf.extend_from_slice(&(pixel_data_offset + i * tile_bytes).to_le_bytes());
        }

        // TileByteCounts: 4 tiles
        for _ in 0..4u32 {
            buf.extend_from_slice(&tile_bytes.to_le_bytes());
        }

        // Pixel data: 4 tiles × 4 bytes each = 16 bytes
        // Tile (0,0): top-left 2x2
        buf.extend_from_slice(&[1, 2, 5, 6]);
        // Tile (0,1): top-right 2x2
        buf.extend_from_slice(&[3, 4, 7, 8]);
        // Tile (1,0): bottom-left 2x2
        buf.extend_from_slice(&[9, 10, 13, 14]);
        // Tile (1,1): bottom-right 2x2
        buf.extend_from_slice(&[11, 12, 15, 16]);

        buf
    }

    #[test]
    fn test_tiled_tile_byte_ranges() {
        let data = make_tiled_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        assert!(provider.is_tiled);
        assert_eq!(provider.block_grid_size(), (2, 2));

        let ranges = provider.tile_byte_ranges();
        assert!(ranges.is_some(), "tile_byte_ranges() should return Some for tiled TIFF");

        let ranges = ranges.unwrap();
        // 2x2 grid = 4 tiles
        assert_eq!(ranges.len(), 4);
        assert!(ranges.contains_key(&(0, 0)));
        assert!(ranges.contains_key(&(0, 1)));
        assert!(ranges.contains_key(&(1, 0)));
        assert!(ranges.contains_key(&(1, 1)));

        // Each tile should have a single-element Vec with 2x2 × 1 byte = 4 bytes
        for (_, range_list) in &ranges {
            assert_eq!(range_list.len(), 1);
            assert_eq!(range_list[0].1, 4);
        }

        // Tiles should be contiguous in file
        let off_00 = ranges[&(0, 0)][0].0;
        let off_01 = ranges[&(0, 1)][0].0;
        let off_10 = ranges[&(1, 0)][0].0;
        let off_11 = ranges[&(1, 1)][0].0;
        assert_eq!(off_01, off_00 + 4);
        assert_eq!(off_10, off_01 + 4);
        assert_eq!(off_11, off_10 + 4);
    }

    #[test]
    fn test_tiled_codec_configuration_uncompressed() {
        let data = make_tiled_tiff();
        let handle = Arc::new(Mutex::new(TiffHandle::from_bytes(&data).unwrap()));
        let metadata = Arc::new(
            TIFFMetadataProvider::from_handle(&handle.lock().unwrap(), 0).unwrap(),
        );

        let provider =
            TIFFImageAssetProvider::new("image:0".to_string(), 0, handle, metadata, vec!["data".to_string()])
                .unwrap();

        // Uncompressed TIFF should return None for codec_configuration
        assert!(provider.codec_configuration().is_none());
    }
}
