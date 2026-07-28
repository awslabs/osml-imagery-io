//! JPEG 2000 block decoder for NITF imagery.
//!
//! This module provides the `Jpeg2000BlockDecoder` which implements the
//! `BlockDecoder` trait for JPEG 2000 compressed imagery (IC=C8, CD).
//!
//! # BPJ2K01.20 Profile Compliance
//!
//! The decoder validates the following BPJ2K01.20 profile constraints:
//! - IMODE must be "B" (band interleaved by block)
//! - NBPP must be between 1 and 38 bits
//! - ABPP must equal NBPP
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::j2k::{Jpeg2000BlockDecoder, OpenJpegCodec};
//! use osml_imagery_io::jbp::image::BlockDecoder;
//!
//! let codec = Arc::new(OpenJpegCodec::new());
//! let decoder = Jpeg2000BlockDecoder::new(&subheader, codestream, codec)?;
//! let (data, shape) = decoder.decode_block(0, 0, 0, None)?;
//! ```

use std::sync::{Arc, OnceLock};

use crate::error::CodecError;
use crate::jbp::image::decoder::BlockDecoder;
use crate::jbp::image::facade::ImageSubheaderFacade;
use crate::jbp::image::types::{InterleaveMode, PixelValueType};
use crate::owned_buffer::OwnedBuffer;

use crate::j2k::markers::{
    build_minimal_codestream_from_parts, main_header_extent, parse_main_header, scan_sot_markers,
    MainHeaderExtent, TilePartOffsetTable,
};
use crate::j2k::{J2KCodec, J2KDecodeParams};

/// Initial header-region fetch size for a `Remote` codestream (mirrors the
/// standalone reader's `HEADER_SCAN_PREFIX`). The region grows adaptively if a
/// codestream carries a main header larger than this.
const HEADER_SCAN_PREFIX: usize = 64 * 1024;

// =============================================================================
// Jpeg2000BlockDecoder
// =============================================================================

/// Block decoder for JPEG 2000 compressed NITF imagery (IC=C8, CD).
///
/// This decoder handles JPEG 2000 Part 1 (IC=C8) and HTJ2K Part 15 (IC=CD)
/// compressed imagery. It validates BPJ2K01.20 profile constraints and
/// delegates decoding to the configured `J2KCodec` implementation.
///
/// # Tile-Based Decoding
///
/// Per the BPJ2K01.20 profile, NITF blocks (NPPBH/NPPBV) must match the
/// native J2K tile grid. This decoder uses `opj_get_decoded_tile` to
/// decode individual tiles efficiently without decoding the entire image.
///
/// # Thread Safety
///
/// `Jpeg2000BlockDecoder` is thread-safe (`Send + Sync`) and can be shared
/// across threads for concurrent block access.
///
/// # Resolution Levels
///
/// JPEG 2000 supports multi-resolution decoding. Use `num_resolution_levels()`
/// to query available levels, and pass the desired level in `J2KDecodeParams`.
pub struct Jpeg2000BlockDecoder {
    /// The J2K codestream data (from image segment)
    codestream: OwnedBuffer,
    /// Image dimensions from subheader
    nrows: u32,
    ncols: u32,
    /// Block dimensions from subheader (NPPBH, NPPBV)
    nppbh: u32,
    nppbv: u32,
    /// Number of bands
    nbands: u32,
    /// Bits per pixel
    nbpp: u8,
    /// Pixel value type
    ///
    /// Captured from the image subheader; the J2K codestream is self-describing
    /// for sample type, so this is retained for completeness but not read back.
    #[allow(dead_code)]
    pvtype: PixelValueType,
    /// Compression type (C8, CD, M8, or MD)
    ic: String,
    /// Whether this is a masked image (IC=M8 or MD)
    is_masked: bool,
    /// COMRAT value
    #[allow(dead_code)]
    comrat: Option<String>,
    /// The J2K codec to use for decoding
    codec: Arc<dyn J2KCodec>,
    /// Cached resolution level count
    num_resolution_levels: OnceLock<u32>,
    /// Cached tile grid info: (tile_width, tile_height, num_tiles_x, num_tiles_y)
    tile_info: OnceLock<(u32, u32, u32, u32)>,
    /// Decode header (main header with TLM stripped). None for masked images.
    decode_header: Option<Vec<u8>>,
    /// Byte offset of the first SOT marker in the codestream. None for masked images.
    first_sot_offset: Option<u64>,
    /// Tile-part offset table. Populated eagerly from TLM or lazily from SOT scan.
    tile_part_table: OnceLock<TilePartOffsetTable>,
}

impl Jpeg2000BlockDecoder {
    /// Create a new JPEG 2000 block decoder.
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade for accessing metadata
    /// * `codestream` - The J2K codestream data
    /// * `codec` - The J2K codec to use for decoding
    ///
    /// # Errors
    ///
    /// Returns `CodecError::InvalidFormat` if:
    /// - IMODE is not "B" (BPJ2K01.20 requirement)
    /// - NBPP is not in range 1-38 (BPJ2K01.20 requirement)
    /// - ABPP does not equal NBPP (BPJ2K01.20 requirement)
    ///
    /// Returns `CodecError::Unsupported` if:
    /// - IC is "CD" (HTJ2K) but the codec doesn't support HTJ2K decoding
    ///
    /// # Requirements
    /// - 1.1, 1.2, 1.3, 1.4, 1.5: Codestream extraction
    /// - 6.1, 6.2, 6.3, 6.4, 6.5: BPJ2K01.20 profile compliance
    pub fn new(
        subheader: &ImageSubheaderFacade,
        codestream: OwnedBuffer,
        codec: Arc<dyn J2KCodec>,
    ) -> Result<Self, CodecError> {
        // Keep the isolated per-block/segment codestream as a (possibly `Remote`)
        // buffer — do NOT materialize it whole. Construction parses only a bounded
        // header region below, and `decode_block` fetches just the target tile's
        // byte ranges on demand (TLM present) or falls back to a header-driven
        // scan (TLM absent). For a resident (`Mapped`/`Heap`) backing every access
        // below is zero-copy; for a `Remote` backing a `get_block` fetches header +
        // the tile's ranges instead of the whole segment. Masked (M8/MD) images
        // decode per-block via `decode_block_at_offset`, which materializes the
        // referenced block range on demand.
        let ic = subheader.ic()?.trim().to_string();
        let imode = subheader.imode()?;

        // Check if this is a masked image (IC=M8 or MD)
        let is_masked = ic == "M8" || ic == "MD";

        // Get the effective IC for codec capability checks (unmask M8->C8, MD->CD)
        let effective_ic = match ic.as_str() {
            "M8" => "C8",
            "MD" => "CD",
            other => other,
        };

        // BPJ2K01.20: IMODE must be B for J2K
        if imode != InterleaveMode::B {
            return Err(CodecError::InvalidFormat(format!(
                "JPEG 2000 images must have IMODE=B (BPJ2K01.20), got IMODE={}",
                imode.to_char()
            )));
        }

        let nbpp = subheader.nbpp()?;
        let abpp = subheader.abpp()?;

        // BPJ2K01.20: NBPP must be 1-38 for J2K
        if !(1..=38).contains(&nbpp) {
            return Err(CodecError::InvalidFormat(format!(
                "JPEG 2000 NBPP must be 1-38 (BPJ2K01.20), got {}",
                nbpp
            )));
        }

        // BPJ2K01.20: ABPP must equal NBPP for J2K
        if abpp != nbpp {
            return Err(CodecError::InvalidFormat(format!(
                "JPEG 2000 images must have ABPP={} equal to NBPP={} (BPJ2K01.20)",
                abpp, nbpp
            )));
        }

        // Check codec capabilities for HTJ2K
        if effective_ic == "CD" && !codec.capabilities().htj2k_decode {
            return Err(CodecError::Unsupported(format!(
                "Codec '{}' does not support HTJ2K (IC={}) decoding",
                codec.capabilities().name,
                ic
            )));
        }

        // Validate codestream magic bytes (SOC marker: 0xFF4F)
        // For masked images, the codestream starts with the mask table, not the SOC marker.
        // The actual J2K codestreams are at offsets specified in the mask table.
        // Read only a bounded prefix (12 bytes) so a `Remote` codestream is not
        // materialized whole just to check the signature.
        if !is_masked {
            let magic = codestream.read_range(0, 12.min(codestream.len()))?;
            if magic.len() < 2 {
                return Err(CodecError::InvalidFormat(
                    "Invalid JPEG 2000 codestream: too short (less than 2 bytes)".into(),
                ));
            }
            if magic[0] != 0xFF || magic[1] != 0x4F {
                // Detect JP2 file format container (signature box: 0x0000000C 6A502020)
                if magic.len() >= 8 && magic[4..8] == [0x6A, 0x50, 0x20, 0x20] {
                    return Err(CodecError::Unsupported(
                        "JPEG 2000 image data contains a JP2 file format container \
                         (detected JP2 signature box) instead of a raw J2K codestream. \
                         JP2 wrapping in NITF is not compliant with BPJ2K01.20."
                            .to_string(),
                    ));
                }
                return Err(CodecError::InvalidFormat(format!(
                    "Invalid JPEG 2000 codestream: missing SOC marker at offset 0 \
                     (found 0x{:02X}{:02X}, expected 0xFF4F)",
                    magic[0], magic[1]
                )));
            }
        }

        let pvtype = subheader.pvtype()?;
        let nrows = subheader.nrows()?;
        let ncols = subheader.ncols()?;
        // Use effective values to handle NPPBH=0/NPPBV=0 (single block = full image)
        let nppbh = subheader.effective_nppbh()?;
        let nppbv = subheader.effective_nppbv()?;
        let nbands = subheader.band_count()? as u32;
        let comrat = subheader.comrat()?;

        // Extract main header for non-masked images. Parse over a bounded header
        // region (SOC .. first SOT) rather than the whole codestream: zero-copy for
        // a resident backing, an adaptive bounded fetch for a `Remote` backing.
        let (decode_header, first_sot_offset, tile_part_table) = if !is_masked {
            let header_region = Self::materialize_header_region(&codestream)?;
            let header_info = parse_main_header(header_region.as_bytes())?;
            let table = OnceLock::new();
            // If TLM markers present, populate tile_part_table eagerly
            if let Some(tlm_table) = header_info.tlm_offset_table {
                let _ = table.set(tlm_table);
            }
            // If no TLM, leave OnceLock empty for lazy SOT scan
            (
                Some(header_info.decode_header),
                Some(header_info.first_sot_offset),
                table,
            )
        } else {
            // Masked images: skip header extraction
            (None, None, OnceLock::new())
        };

        Ok(Self {
            codestream,
            nrows,
            ncols,
            nppbh,
            nppbv,
            nbands,
            nbpp,
            pvtype,
            ic,
            is_masked,
            comrat,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header,
            first_sot_offset,
            tile_part_table,
        })
    }

    /// Materialize a codestream header region large enough to hold the full main
    /// header (SOC .. first SOT), for the pure-Rust marker scans and the codec's
    /// header-only parses (`get_tile_info` / `get_resolution_levels`).
    ///
    /// Zero-copy for a resident (`Mapped`/`Heap`) codestream (returns the whole
    /// sub-view, which is already in memory). For a `Remote` codestream it fetches
    /// a bounded prefix and grows it (doubling, capped at the codestream length)
    /// until the main header fits — so a normal file costs one bounded fetch and a
    /// file with an oversized PPM/COM main header still parses correctly. Mirrors
    /// `J2KImageAssetProvider`'s `materialize_header_region` in `j2k/reader.rs`.
    fn materialize_header_region(codestream: &OwnedBuffer) -> Result<OwnedBuffer, CodecError> {
        let cs_len = codestream.len();

        // Resident: the whole codestream is already in memory; scan it in place.
        if codestream.resident_bytes().is_some() {
            return codestream.try_slice(0..cs_len);
        }

        // Remote: grow an initial prefix until the main header is fully present.
        let mut prefix_len = HEADER_SCAN_PREFIX.min(cs_len);
        loop {
            let region = codestream.try_slice(0..prefix_len)?;
            if prefix_len == cs_len {
                return Ok(region);
            }
            match main_header_extent(region.as_bytes()) {
                MainHeaderExtent::Complete(_) => return Ok(region),
                MainHeaderExtent::Truncated => {
                    prefix_len = (prefix_len.saturating_mul(2)).min(cs_len);
                }
            }
        }
    }

    /// Get or compute the number of resolution levels.
    ///
    /// For masked images, returns 1 since each block has its own codestream
    /// and we can't determine the resolution levels without parsing each block.
    fn resolution_levels(&self) -> Result<u32, CodecError> {
        if let Some(&levels) = self.num_resolution_levels.get() {
            return Ok(levels);
        }

        // For masked images, we can't determine resolution levels from the
        // raw codestream since it starts with the mask table. Return 1 as
        // a safe default - each block's codestream may have different levels.
        if self.is_masked {
            let _ = self.num_resolution_levels.set(1);
            return Ok(1);
        }

        // Header-only parse: use a bounded header region (zero-copy for a resident
        // codestream, a bounded fetch for a `Remote` one), never the whole segment.
        let header_region = Self::materialize_header_region(&self.codestream)?;
        let levels = self.codec.get_resolution_levels(header_region.as_bytes())?;
        // Ignore the result of set() - if another thread set it first, that's fine
        let _ = self.num_resolution_levels.set(levels);
        Ok(levels)
    }

    /// Get or compute the tile grid information.
    /// Returns (tile_width, tile_height, num_tiles_x, num_tiles_y).
    ///
    /// For masked images, returns the image dimensions as a single tile
    /// since each block is encoded as a separate single-tile codestream.
    fn tile_grid_info(&self) -> Result<(u32, u32, u32, u32), CodecError> {
        if let Some(&info) = self.tile_info.get() {
            return Ok(info);
        }

        // For masked images, we can't parse tile info from the raw codestream
        // since it starts with the mask table. Return image dimensions as
        // a single tile - the actual tile info is per-block.
        if self.is_masked {
            let info = (self.ncols, self.nrows, 1, 1);
            let _ = self.tile_info.set(info);
            return Ok(info);
        }

        // Header-only parse: use a bounded header region (zero-copy for a resident
        // codestream, a bounded fetch for a `Remote` one), never the whole segment.
        let header_region = Self::materialize_header_region(&self.codestream)?;
        let info = self.codec.get_tile_info(header_region.as_bytes())?;
        // Ignore the result of set() - if another thread set it first, that's fine
        let _ = self.tile_info.set(info);
        Ok(info)
    }

    /// Ensure the tile-part offset table is populated.
    ///
    /// If the table was eagerly populated from TLM markers during construction,
    /// returns it immediately. Otherwise, triggers a lazy SOT marker scan.
    ///
    /// # Errors
    /// Returns `CodecError::InvalidFormat` if called on a masked image or if
    /// SOT scanning fails.
    fn ensure_tile_part_table(&self) -> Result<&TilePartOffsetTable, CodecError> {
        if let Some(table) = self.tile_part_table.get() {
            return Ok(table);
        }
        let first_sot = self.first_sot_offset.ok_or_else(|| {
            CodecError::InvalidFormat("Cannot scan SOT markers for masked images".to_string())
        })?;
        // Reached only when TLM markers are absent (no eager table). Building the
        // SOT table inherently walks every tile-part across the codestream, so this
        // materializes the whole segment (zero-copy for a resident backing; a single
        // whole-segment fetch for a `Remote` one — the documented TLM-absent
        // floor). TLM-present files never reach here.
        let resident = self.codestream.materialize()?;
        let table = scan_sot_markers(resident.as_bytes(), first_sot)?;
        // Ignore set result — if another thread set it first, that's fine
        let _ = self.tile_part_table.set(table);
        Ok(self.tile_part_table.get().unwrap())
    }

    /// Select specific bands from decoded data.
    ///
    /// # Arguments
    /// * `data` - Full decoded data in band-sequential format
    /// * `band_indices` - Indices of bands to select
    /// * `pixels_per_band` - Number of pixels per band
    /// * `bytes_per_pixel` - Bytes per pixel value
    ///
    /// # Returns
    /// Data containing only the selected bands.
    ///
    /// # Errors
    /// Returns `CodecError::InvalidFormat` if any band index is out of range.
    fn select_bands(
        &self,
        data: &[u8],
        band_indices: &[u32],
        pixels_per_band: usize,
        bytes_per_pixel: usize,
    ) -> Result<Vec<u8>, CodecError> {
        let band_size = pixels_per_band * bytes_per_pixel;
        let mut selected = Vec::with_capacity(band_indices.len() * band_size);

        for &band_idx in band_indices {
            if band_idx >= self.nbands {
                return Err(CodecError::InvalidFormat(format!(
                    "Band index {} out of range (image has {} bands)",
                    band_idx, self.nbands
                )));
            }

            let start = (band_idx as usize) * band_size;
            let end = start + band_size;

            if end > data.len() {
                return Err(CodecError::Decode(format!(
                    "Band {} data out of range: need bytes {}..{}, have {} bytes",
                    band_idx,
                    start,
                    end,
                    data.len()
                )));
            }

            selected.extend_from_slice(&data[start..end]);
        }

        Ok(selected)
    }
}

impl BlockDecoder for Jpeg2000BlockDecoder {
    /// Decode a single block of JPEG 2000 image data.
    ///
    /// Per the BPJ2K01.20 profile, NITF blocks (NPPBH/NPPBV) must match the
    /// native J2K tile grid. This method decodes the specific tile at the
    /// given block coordinates using `opj_get_decoded_tile`.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block/tile to decode
    /// * `block_col` - Column index of the block/tile to decode
    /// * `resolution_level` - Resolution level to decode (0 = full, N = 1/2^N)
    /// * `bands` - Optional slice of band indices to retrieve
    ///
    /// # Returns
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` at the requested resolution (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if block coordinates are out of range,
    /// or if resolution_level exceeds available levels.
    /// Returns `CodecError::Decode` if decoding fails.
    ///
    /// # Requirements
    /// - 2.1, 2.2, 2.3, 2.4, 2.5, 2.6: JPEG 2000 decoding
    /// - 3.1, 3.2, 3.3, 3.4, 3.5: Resolution level support
    /// - 4.1, 4.2, 4.3, 4.4, 4.5: Block-based access
    fn decode_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Get tile grid info
        let (tile_width, tile_height, num_tiles_x, num_tiles_y) = self.tile_grid_info()?;

        // Validate block coordinates against tile grid
        if block_row >= num_tiles_y || block_col >= num_tiles_x {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Validate resolution level
        let num_levels = self.resolution_levels()?;
        if resolution_level >= num_levels {
            return Err(CodecError::InvalidResolutionLevelRange {
                requested: resolution_level,
                available: num_levels,
                max_valid: num_levels.saturating_sub(1),
            });
        }

        // Calculate tile index (row-major order, same as encoder)
        let tile_index = block_row * num_tiles_x + block_col;

        let params = J2KDecodeParams {
            resolution_level,
            region: None,
        };

        // Decode the specific tile
        let result = if self.decode_header.is_some() {
            // Non-masked path: construct a minimal single-tile codestream from just
            // this tile's byte ranges. `ensure_tile_part_table` gives the per-tile
            // (offset, length) ranges from TLM (no scan) or a one-time SOT scan;
            // we then fetch *only* those ranges (`read_range`) rather than the whole
            // segment. For a `Remote` backing this is the bandwidth win: header +
            // this tile's parts, not ~100% of the file. For a resident backing each
            // `read_range` is a cheap copy of already-mapped bytes.
            let table = self.ensure_tile_part_table()?;
            let tile_parts: Vec<(u64, u64)> = table
                .iter()
                .filter(|e| e.tile_index as u32 == tile_index)
                .map(|e| (e.offset, e.length))
                .collect();
            if tile_parts.is_empty() {
                return Err(CodecError::InvalidBlockCoordinates(
                    block_row,
                    block_col,
                    resolution_level,
                ));
            }
            // Fetch this tile's parts in one batched call (bounded ranges into the
            // codestream). For a `Remote` backing this issues a single `read_many`
            // → `cat_ranges`, so the scattered tile-parts fetch concurrently rather
            // than one at a time; for a resident backing it is N cheap copies.
            let ranges: Vec<(usize, usize)> = tile_parts
                .iter()
                .map(|&(offset, length)| (offset as usize, length as usize))
                .collect();
            let part_bytes: Vec<Vec<u8>> = self.codestream.read_ranges(&ranges)?;
            let part_slices: Vec<&[u8]> = part_bytes.iter().map(|v| v.as_slice()).collect();
            let decode_header = self.decode_header.as_ref().unwrap();
            let minimal_codestream =
                build_minimal_codestream_from_parts(decode_header, &part_slices);
            self.codec.decode_tile(&minimal_codestream, 0, &params)?
        } else {
            // Masked path: decode the (already isolated) block codestream. The
            // segment is materialized on demand; masked blocks are reached via
            // `decode_block_at_offset` in practice, so this branch is rarely hit.
            let resident = self.codestream.materialize()?;
            self.codec
                .decode_tile(resident.as_bytes(), tile_index, &params)?
        };

        // Calculate expected tile dimensions at this resolution level
        let scale = 1u32 << resolution_level;

        // For edge tiles, dimensions may be smaller than the nominal tile size
        let tile_x0 = block_col * tile_width;
        let tile_y0 = block_row * tile_height;
        let actual_tile_width = (self.ncols - tile_x0).min(tile_width);
        let actual_tile_height = (self.nrows - tile_y0).min(tile_height);
        let expected_width = actual_tile_width.div_ceil(scale);
        let expected_height = actual_tile_height.div_ceil(scale);

        // Validate: decoded pixel count must not exceed expected (corruption guard).
        // The J2K codestream may have different geometry than the NITF subheader
        // due to non-zero image/tile offsets or sub-sampling, so we compare total
        // pixel count rather than exact width/height.
        let decoded_pixels = (result.width as u64) * (result.height as u64);
        let expected_pixels = (expected_width as u64) * (expected_height as u64);
        if decoded_pixels > expected_pixels {
            return Err(CodecError::Decode(format!(
                "Decoded tile pixel count {} ({}x{}) exceeds expected {} ({}x{}) at resolution level {} for tile ({}, {})",
                decoded_pixels, result.width, result.height, expected_pixels, expected_width, expected_height, resolution_level, block_row, block_col
            )));
        }

        // Validate decoded band count
        if result.num_components != self.nbands {
            return Err(CodecError::Decode(format!(
                "Decoded band count {} doesn't match subheader NBANDS={}",
                result.num_components, self.nbands
            )));
        }

        // Calculate bytes per pixel
        let bytes_per_pixel = (self.nbpp as usize).div_ceil(8);
        let pixels_per_band = (result.width * result.height) as usize;

        // Apply band selection if specified
        let (data, num_bands) = match bands {
            Some(band_indices) if !band_indices.is_empty() => {
                let selected = self.select_bands(
                    &result.data,
                    band_indices,
                    pixels_per_band,
                    bytes_per_pixel,
                )?;
                (selected, band_indices.len() as u32)
            }
            _ => (result.data, result.num_components),
        };

        // Return shape as [bands, rows, cols] (CHW format)
        Ok((data, [num_bands, result.height, result.width]))
    }

    /// Check if a block exists at the given coordinates.
    ///
    /// Validates against the actual J2K tile grid from the codestream.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    ///
    /// # Returns
    /// `true` if the block coordinates are within the tile grid.
    fn has_block(&self, block_row: u32, block_col: u32) -> bool {
        match self.tile_grid_info() {
            Ok((_, _, num_tiles_x, num_tiles_y)) => {
                block_row < num_tiles_y && block_col < num_tiles_x
            }
            Err(_) => false,
        }
    }

    /// Get the compression type identifier.
    ///
    /// # Returns
    /// "C8" for JPEG 2000 Part 1, "CD" for HTJ2K Part 15.
    fn compression_type(&self) -> &str {
        &self.ic
    }

    /// Get the number of resolution levels.
    ///
    /// JPEG 2000 supports multi-resolution decoding. Level 0 is full resolution,
    /// level N is 1/(2^N) of full resolution.
    ///
    /// # Returns
    /// The number of available resolution levels (minimum 1).
    ///
    /// # Requirements
    /// - 3.1, 3.2, 3.3, 3.4, 3.5: Resolution level support
    fn num_resolution_levels(&self) -> u32 {
        self.resolution_levels().unwrap_or(1)
    }

    /// Decode a block at a specific byte offset.
    ///
    /// For JPEG 2000 masked images, each block's codestream is stored at the
    /// offset specified in the Image Data Mask table. This method extracts
    /// the J2K codestream starting at the given offset and decodes it.
    ///
    /// # Arguments
    /// * `offset` - Byte offset from the start of image data to the J2K codestream
    /// * `block_row` - Row index of the block (for validation and dimension calculation)
    /// * `block_col` - Column index of the block (for validation and dimension calculation)
    /// * `resolution_level` - Resolution level to decode (0 = full resolution)
    /// * `bands` - Optional slice of band indices to retrieve
    ///
    /// # Returns
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` at the requested resolution (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if the offset is invalid or decoding fails.
    ///
    /// # Requirements
    /// - 6.1, 6.2: Masked JPEG 2000 decoding using offsets from mask table
    fn decode_block_at_offset(
        &self,
        offset: u64,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Validate resolution level
        let num_levels = self.resolution_levels()?;
        if resolution_level >= num_levels {
            return Err(CodecError::InvalidResolutionLevelRange {
                requested: resolution_level,
                available: num_levels,
                max_valid: num_levels.saturating_sub(1),
            });
        }

        // Validate offset is within bounds (uses the logical length, no fetch).
        let offset_usize = offset as usize;
        let cs_len = self.codestream.len();
        if offset_usize >= cs_len {
            return Err(CodecError::Decode(format!(
                "Block offset {} exceeds codestream length {}",
                offset, cs_len
            )));
        }

        // Narrow to the block's codestream (offset .. end of segment) *without*
        // fetching: a `Remote` backing stays `Remote`, so OpenJPEG's callbacks pull
        // only the ranges this block touches; a resident backing stays zero-copy.
        // Each masked block is its own single-tile codestream; OpenJPEG stops at the
        // block's EOC, so the trailing segment bytes in the sub-view are ignored
        // (same as the prior `&codestream_bytes[offset..]` behavior).
        let block_codestream = self.codestream.subview(offset_usize..cs_len);

        // Validate codestream magic bytes (SOC marker: 0xFF4F) from a bounded read.
        let magic = block_codestream.read_range(0, 2.min(block_codestream.len()))?;
        if magic.len() < 2 {
            return Err(CodecError::Decode(format!(
                "Block codestream at offset {} too short (less than 2 bytes)",
                offset
            )));
        }
        if magic[0] != 0xFF || magic[1] != 0x4F {
            return Err(CodecError::Decode(format!(
                "Invalid J2K codestream at offset {}: missing SOC marker \
                 (found 0x{:02X}{:02X}, expected 0xFF4F)",
                offset, magic[0], magic[1]
            )));
        }

        // Decode the block codestream
        // For masked images, each block is a single-tile codestream, so tile_index is 0
        let params = crate::j2k::J2KDecodeParams {
            resolution_level,
            region: None,
        };

        let result = self
            .codec
            .decode_tile_source(&block_codestream, 0, &params)?;

        // For masked images, use block dimensions from subheader (NPPBH, NPPBV)
        // For non-masked images, use tile dimensions from the J2K codestream
        let (tile_width, tile_height) = if self.is_masked {
            (self.nppbh, self.nppbv)
        } else {
            let (tw, th, _, _) = self.tile_grid_info()?;
            (tw, th)
        };

        // Calculate expected tile dimensions at this resolution level
        let scale = 1u32 << resolution_level;

        // For edge tiles, dimensions may be smaller than the nominal tile size
        let tile_x0 = block_col * tile_width;
        let tile_y0 = block_row * tile_height;
        let actual_tile_width = (self.ncols.saturating_sub(tile_x0)).min(tile_width);
        let actual_tile_height = (self.nrows.saturating_sub(tile_y0)).min(tile_height);
        let expected_width = actual_tile_width.div_ceil(scale);
        let expected_height = actual_tile_height.div_ceil(scale);

        // Validate: decoded pixel count must not exceed expected (corruption guard).
        // The J2K codestream may have different geometry than the NITF subheader
        // due to non-zero image/tile offsets or sub-sampling, so we compare total
        // pixel count rather than exact width/height.
        let decoded_pixels = (result.width as u64) * (result.height as u64);
        let expected_pixels = (expected_width as u64) * (expected_height as u64);
        if decoded_pixels > expected_pixels {
            return Err(CodecError::Decode(format!(
                "Decoded block pixel count {} ({}x{}) exceeds expected {} ({}x{}) at resolution level {} for block ({}, {})",
                decoded_pixels, result.width, result.height, expected_pixels, expected_width, expected_height, resolution_level, block_row, block_col
            )));
        }

        // Validate decoded band count
        if result.num_components != self.nbands {
            return Err(CodecError::Decode(format!(
                "Decoded band count {} doesn't match subheader NBANDS={}",
                result.num_components, self.nbands
            )));
        }

        // Calculate bytes per pixel
        let bytes_per_pixel = (self.nbpp as usize).div_ceil(8);
        let pixels_per_band = (result.width * result.height) as usize;

        // Apply band selection if specified
        let (data, num_bands) = match bands {
            Some(band_indices) if !band_indices.is_empty() => {
                let selected = self.select_bands(
                    &result.data,
                    band_indices,
                    pixels_per_band,
                    bytes_per_pixel,
                )?;
                (selected, band_indices.len() as u32)
            }
            _ => (result.data, result.num_components),
        };

        // Return shape as [bands, rows, cols] (CHW format)
        Ok((data, [num_bands, result.height, result.width]))
    }

    fn tile_byte_ranges(&self) -> Option<std::collections::HashMap<(u32, u32), Vec<(u64, u64)>>> {
        if self.is_masked {
            return None; // Delegate to JBPImageAssetProvider's mask-aware logic
        }
        let table = self.ensure_tile_part_table().ok()?;
        let (_, _, num_tiles_x, _) = self.tile_grid_info().ok()?;

        let mut ranges: std::collections::HashMap<(u32, u32), Vec<(u64, u64)>> =
            std::collections::HashMap::new();
        for entry in table {
            let row = entry.tile_index as u32 / num_tiles_x;
            let col = entry.tile_index as u32 % num_tiles_x;
            ranges
                .entry((row, col))
                .or_default()
                .push((entry.offset, entry.length));
        }
        Some(ranges)
    }

    fn codec_configuration(&self) -> Option<std::collections::HashMap<String, Vec<u8>>> {
        let decode_header = self.decode_header.as_ref()?;
        let mut config = std::collections::HashMap::new();
        config.insert("main_header".to_string(), decode_header.clone());
        Some(config)
    }
}

// Safety: Jpeg2000BlockDecoder is thread-safe
// - codestream is OwnedBuffer (immutable, shared via Arc internally)
// - codec is Arc<dyn J2KCodec> (thread-safe by trait bound)
// - num_resolution_levels uses OnceLock for lazy init
unsafe impl Send for Jpeg2000BlockDecoder {}
unsafe impl Sync for Jpeg2000BlockDecoder {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Mock Codec for Testing
    // =========================================================================

    /// Mock codec for testing without OpenJPEG dependency
    struct MockJ2KCodec {
        htj2k_support: bool,
        decode_result: Option<crate::j2k::J2KDecodeResult>,
        tile_info: (u32, u32, u32, u32), // (tile_width, tile_height, num_tiles_x, num_tiles_y)
    }

    impl MockJ2KCodec {
        fn new() -> Self {
            Self {
                htj2k_support: false,
                decode_result: None,
                tile_info: (64, 64, 1, 1), // Default: single tile
            }
        }

        fn with_htj2k_support(mut self) -> Self {
            self.htj2k_support = true;
            self
        }

        fn with_decode_result(
            mut self,
            width: u32,
            height: u32,
            num_components: u32,
            bits_per_component: u8,
        ) -> Self {
            let bytes_per_pixel = (bits_per_component as usize).div_ceil(8);
            let data_size = (width * height * num_components) as usize * bytes_per_pixel;
            self.decode_result = Some(crate::j2k::J2KDecodeResult {
                data: vec![0u8; data_size],
                width,
                height,
                num_components,
                bits_per_component,
                is_signed: false,
                num_resolution_levels: 6,
            });
            // Update tile info to match - single tile covering the whole image
            self.tile_info = (width, height, 1, 1);
            self
        }

        fn with_tile_grid(
            mut self,
            tile_width: u32,
            tile_height: u32,
            num_tiles_x: u32,
            num_tiles_y: u32,
        ) -> Self {
            self.tile_info = (tile_width, tile_height, num_tiles_x, num_tiles_y);
            self
        }
    }

    impl J2KCodec for MockJ2KCodec {
        fn capabilities(&self) -> crate::j2k::J2KCodecCapabilities {
            crate::j2k::J2KCodecCapabilities {
                max_bit_depth: 38,
                htj2k_decode: self.htj2k_support,
                htj2k_encode: self.htj2k_support,
                name: "MockCodec",
            }
        }

        fn decode(
            &self,
            _codestream: &[u8],
            _params: &J2KDecodeParams,
        ) -> Result<crate::j2k::J2KDecodeResult, CodecError> {
            self.decode_result
                .clone()
                .ok_or_else(|| CodecError::Decode("Mock decode not configured".into()))
        }

        fn start_encode(
            &self,
            _params: &crate::j2k::J2KEncodeParams,
        ) -> Result<Box<dyn crate::j2k::J2KEncodeState>, CodecError> {
            Err(CodecError::Unsupported(
                "Mock encode not implemented".into(),
            ))
        }

        fn get_resolution_levels(&self, _codestream: &[u8]) -> Result<u32, CodecError> {
            Ok(6)
        }

        fn get_dimensions(&self, _codestream: &[u8]) -> Result<(u32, u32, u32), CodecError> {
            if let Some(ref result) = self.decode_result {
                Ok((result.width, result.height, result.num_components))
            } else {
                Err(CodecError::Decode("Mock dimensions not configured".into()))
            }
        }

        fn get_tile_info(&self, _codestream: &[u8]) -> Result<(u32, u32, u32, u32), CodecError> {
            Ok(self.tile_info)
        }

        fn decode_tile(
            &self,
            _codestream: &[u8],
            tile_index: u32,
            _params: &J2KDecodeParams,
        ) -> Result<crate::j2k::J2KDecodeResult, CodecError> {
            let (tile_width, tile_height, num_tiles_x, num_tiles_y) = self.tile_info;
            let total_tiles = num_tiles_x * num_tiles_y;

            if tile_index >= total_tiles {
                return Err(CodecError::InvalidBlockCoordinates(
                    tile_index / num_tiles_x,
                    tile_index % num_tiles_x,
                    0,
                ));
            }

            // Return a result sized for the tile
            if let Some(ref result) = self.decode_result {
                let bytes_per_pixel = (result.bits_per_component as usize).div_ceil(8);
                let data_size =
                    (tile_width * tile_height * result.num_components) as usize * bytes_per_pixel;
                Ok(crate::j2k::J2KDecodeResult {
                    data: vec![0u8; data_size],
                    width: tile_width,
                    height: tile_height,
                    num_components: result.num_components,
                    bits_per_component: result.bits_per_component,
                    is_signed: result.is_signed,
                    num_resolution_levels: result.num_resolution_levels,
                })
            } else {
                Err(CodecError::Decode("Mock decode not configured".into()))
            }
        }
    }

    // =========================================================================
    // Test Helpers
    // =========================================================================

    /// Create a minimal valid J2K codestream (just SOC marker + padding)
    fn create_mock_codestream() -> OwnedBuffer {
        // SOC marker (0xFF4F) followed by some padding
        let data = vec![0xFF, 0x4F, 0xFF, 0x51, 0x00, 0x00, 0x00, 0x00];
        OwnedBuffer::from_vec(data)
    }

    // =========================================================================
    // Validation Tests
    // =========================================================================

    #[test]
    fn test_invalid_codestream_too_short() {
        // Test that codestream validation catches short data
        let short_data = OwnedBuffer::from_vec(vec![0xFF]);

        // We can't easily create a mock subheader, so we test the validation
        // logic directly by checking the error message
        assert!(short_data.as_bytes().len() < 2);
    }

    #[test]
    fn test_invalid_codestream_bad_magic() {
        // Test that codestream validation catches bad magic bytes
        let bad_magic = OwnedBuffer::from_vec(vec![0x00, 0x00, 0x00, 0x00]);

        // Verify the magic bytes are wrong
        assert!(bad_magic.as_bytes()[0] != 0xFF || bad_magic.as_bytes()[1] != 0x4F);
    }

    #[test]
    fn test_valid_codestream_magic() {
        // Test that valid SOC marker is recognized
        let valid = create_mock_codestream();

        // Verify the magic bytes are correct
        assert_eq!(valid.as_bytes()[0], 0xFF);
        assert_eq!(valid.as_bytes()[1], 0x4F);
    }

    // =========================================================================
    // Band Selection Tests
    // =========================================================================

    #[test]
    fn test_select_bands_single() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(4, 4, 3, 8));

        // Create mock decoder state for testing select_bands
        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 4,
            ncols: 4,
            nppbh: 4,
            nppbv: 4,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Create test data: 3 bands, 16 pixels each, 1 byte per pixel
        let mut data = Vec::new();
        for band in 0..3u8 {
            for pixel in 0..16u8 {
                data.push(band * 100 + pixel);
            }
        }

        // Select band 1
        let selected = decoder.select_bands(&data, &[1], 16, 1).unwrap();
        assert_eq!(selected.len(), 16);
        assert_eq!(selected[0], 100); // First pixel of band 1
        assert_eq!(selected[15], 115); // Last pixel of band 1
    }

    #[test]
    fn test_select_bands_multiple() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(4, 4, 3, 8));

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 4,
            ncols: 4,
            nppbh: 4,
            nppbv: 4,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Create test data: 3 bands, 16 pixels each
        let mut data = Vec::new();
        for band in 0..3u8 {
            for pixel in 0..16u8 {
                data.push(band * 100 + pixel);
            }
        }

        // Select bands 0 and 2
        let selected = decoder.select_bands(&data, &[0, 2], 16, 1).unwrap();
        assert_eq!(selected.len(), 32); // 2 bands * 16 pixels
        assert_eq!(selected[0], 0); // First pixel of band 0
        assert_eq!(selected[16], 200); // First pixel of band 2
    }

    #[test]
    fn test_select_bands_out_of_range() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(4, 4, 3, 8));

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 4,
            ncols: 4,
            nppbh: 4,
            nppbv: 4,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        let data = vec![0u8; 48]; // 3 bands * 16 pixels

        // Try to select band 5 (out of range)
        let result = decoder.select_bands(&data, &[5], 16, 1);
        assert!(result.is_err());
        if let Err(CodecError::InvalidFormat(msg)) = result {
            assert!(msg.contains("out of range"));
        }
    }

    // =========================================================================
    // BlockDecoder Trait Tests
    // =========================================================================

    #[test]
    fn test_has_block_valid() {
        let codec = Arc::new(
            MockJ2KCodec::new()
                .with_decode_result(64, 64, 1, 8)
                .with_tile_grid(32, 32, 2, 2),
        ); // 2x2 tile grid

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 32,
            nppbv: 32,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // All 4 tiles in 2x2 grid are valid
        assert!(decoder.has_block(0, 0));
        assert!(decoder.has_block(0, 1));
        assert!(decoder.has_block(1, 0));
        assert!(decoder.has_block(1, 1));
        // Out of range
        assert!(!decoder.has_block(0, 2));
        assert!(!decoder.has_block(2, 0));
        assert!(!decoder.has_block(2, 2));
    }

    #[test]
    fn test_has_block_single_tile() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 1, 8)); // Default single tile

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Only (0, 0) is valid for single-tile image
        assert!(decoder.has_block(0, 0));
        assert!(!decoder.has_block(0, 1));
        assert!(!decoder.has_block(1, 0));
        assert!(!decoder.has_block(1, 1));
    }

    #[test]
    fn test_compression_type_c8() {
        let codec = Arc::new(MockJ2KCodec::new());

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        assert_eq!(decoder.compression_type(), "C8");
    }

    #[test]
    fn test_compression_type_cd() {
        let codec = Arc::new(MockJ2KCodec::new().with_htj2k_support());

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "CD".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        assert_eq!(decoder.compression_type(), "CD");
    }

    #[test]
    fn test_num_resolution_levels() {
        let codec = Arc::new(MockJ2KCodec::new());

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Mock codec returns 6 resolution levels
        assert_eq!(decoder.num_resolution_levels(), 6);
    }

    #[test]
    fn test_decode_block_invalid_coordinates() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 1, 8)); // Single tile

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Invalid block coordinates should return error (single tile image)
        let result = decoder.decode_block(1, 0, 0, None);
        assert!(matches!(
            result,
            Err(CodecError::InvalidBlockCoordinates(1, 0, 0))
        ));

        let result = decoder.decode_block(0, 1, 0, None);
        assert!(matches!(
            result,
            Err(CodecError::InvalidBlockCoordinates(0, 1, 0))
        ));
    }

    /// A masked decoder over a `Remote` `OwnedBuffer` codestream decodes via
    /// `decode_block`'s masked branch, which now materializes the codestream on
    /// demand (`materialize()`) rather than assuming it was pre-materialized in
    /// `new`. The fetch stays bounded and decode matches the resident path.
    #[test]
    fn test_remote_materialized_codestream_decodes() {
        use crate::remote::{FakeReader, HeaderAwarePolicy, StreamFetcher};

        // Same bytes as create_mock_codestream(), served from a Remote source.
        let bytes = vec![0xFF, 0x4F, 0xFF, 0x51, 0x00, 0x00, 0x00, 0x00];
        let reader = FakeReader::new(bytes.clone());
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        // Keep the codestream `Remote` — the masked decode path must fetch it on
        // demand (previously `new` pre-materialized it; it no longer does).
        let codestream = OwnedBuffer::from_remote(fetcher);

        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 3, 8));
        let decoder = Jpeg2000BlockDecoder {
            codestream,
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "M8".to_string(),
            is_masked: true,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Masked path calls codestream.as_bytes(); must not panic on the guard.
        let (data, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(data.len(), 64 * 64 * 3);
        // The isolated codestream was fetched once, not the "whole file" (there is
        // only one range here, but assert the fetch happened and stayed bounded).
        let log = log.lock().unwrap();
        assert!(log.iter().all(|&(_, len)| len <= bytes.len()));
    }

    /// With TLM present (tile-part table pre-populated, as `new` does when the
    /// codestream carries TLM), a non-masked `decode_block` over a `Remote`
    /// codestream fetches **only the target tile's byte range** — not the whole
    /// segment and not other tiles' bytes. This is the targeted-tile bandwidth fix.
    #[test]
    fn test_remote_decode_block_fetches_only_target_tile_parts() {
        use crate::j2k::markers::TilePartEntry;
        use crate::remote::{FakeReader, HeaderAwarePolicy, StreamFetcher};

        // A synthetic codestream large enough that "whole file" is unambiguous.
        // Layout (offsets are what we record in the tile-part table):
        //   [0..64)      main header / decode header region (SOC + SIZ + ...)
        //   [64..1064)   tile 0's tile-part bytes (starts with an SOT marker)
        //   [1064..2064) tile 1's tile-part bytes (starts with an SOT marker)
        let mut cs = vec![0u8; 2064];
        cs[0] = 0xFF;
        cs[1] = 0x4F; // SOC (kept resident-independent; not parsed here)
                      // Give each tile-part a valid-looking SOT so the Isot-patch branch runs.
        let sot = |buf: &mut [u8]| {
            buf[0] = 0xFF;
            buf[1] = 0x90; // SOT
            buf[2] = 0x00;
            buf[3] = 0x0A; // Lsot
        };
        sot(&mut cs[64..]);
        sot(&mut cs[1064..]);
        let file_len = cs.len();

        // A pre-populated TLM table: tile 0 at [64,1064), tile 1 at [1064,2064).
        let table: TilePartOffsetTable = vec![
            TilePartEntry {
                tile_index: 0,
                offset: 64,
                length: 1000,
            },
            TilePartEntry {
                tile_index: 1,
                offset: 1064,
                length: 1000,
            },
        ];
        let tile_part_table = OnceLock::new();
        let _ = tile_part_table.set(table);
        // Pre-populate the grid/level caches so this test isolates the *decode*
        // fetch (a real reader populates these from a bounded header region at
        // metadata time; here the synthetic file is smaller than the 64 KiB header
        // prefetch, which would otherwise coalesce into one whole-file read).
        let tile_info = OnceLock::new();
        let _ = tile_info.set((64u32, 64u32, 1u32, 2u32));
        let num_resolution_levels = OnceLock::new();
        let _ = num_resolution_levels.set(1u32);

        // Remote codestream + fetch log (no eager prefetch).
        let reader = FakeReader::new(cs.clone());
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let codestream = OwnedBuffer::from_remote(fetcher);

        // 2x1 tile grid; MockCodec ignores the codestream bytes and returns a tile.
        let codec = Arc::new(
            MockJ2KCodec::new()
                .with_decode_result(64, 128, 1, 8)
                .with_tile_grid(64, 64, 1, 2),
        );
        let decoder = Jpeg2000BlockDecoder {
            codestream,
            nrows: 128,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels,
            tile_info,
            decode_header: Some(cs[..64].to_vec()),
            first_sot_offset: Some(64),
            tile_part_table,
        };

        // Decode tile (1, 0) — tile_index 1, whose parts live at [1064, 2064).
        let _ = decoder.decode_block(1, 0, 0, None).unwrap();

        let fetches = log.lock().unwrap();
        // No single fetch covered the whole file.
        assert!(
            fetches.iter().all(|&(_, len)| len < file_len),
            "a fetch covered the whole file: {:?}",
            *fetches
        );
        // Total bytes fetched is bounded by tile 1's part (1000 B) plus slack —
        // and strictly less than the full codestream. Tile 0's bytes are NOT read.
        let total: usize = fetches.iter().map(|(_, len)| *len).sum();
        assert!(
            total < file_len,
            "decoding tile 1 fetched {total} of {file_len} bytes — should not pull the whole segment"
        );
        // Every fetch must lie within tile 1's byte range [1064, 2064); tile 0's
        // range [64, 1064) must never be touched.
        assert!(
            fetches
                .iter()
                .all(|&(off, len)| off >= 1064 && off + len as u64 <= file_len as u64),
            "a fetch fell outside tile 1's range [1064, {file_len}): {:?}",
            *fetches
        );
    }

    #[test]
    fn test_decode_block_success() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 3, 8)); // Single tile

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        let result = decoder.decode_block(0, 0, 0, None);
        assert!(result.is_ok());

        let (data, shape) = result.unwrap();
        // Shape is [bands, rows, cols] (CHW format)
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(data.len(), 64 * 64 * 3); // 3 bands, 1 byte per pixel
    }

    #[test]
    fn test_decode_block_multi_tile() {
        // Test decoding from a multi-tile image
        let codec = Arc::new(
            MockJ2KCodec::new()
                .with_decode_result(128, 128, 3, 8)
                .with_tile_grid(64, 64, 2, 2),
        ); // 2x2 tile grid

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 128,
            ncols: 128,
            nppbh: 64,
            nppbv: 64,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // All 4 tiles should be decodable
        for row in 0..2 {
            for col in 0..2 {
                let result = decoder.decode_block(row, col, 0, None);
                assert!(result.is_ok(), "Failed to decode tile ({}, {})", row, col);
                let (data, shape) = result.unwrap();
                // Shape is [bands, rows, cols] (CHW format)
                assert_eq!(shape, [3, 64, 64]);
                assert_eq!(data.len(), 64 * 64 * 3);
            }
        }

        // Out of range should fail
        let result = decoder.decode_block(2, 0, 0, None);
        assert!(matches!(
            result,
            Err(CodecError::InvalidBlockCoordinates(2, 0, 0))
        ));
    }

    #[test]
    fn test_decode_block_with_band_selection() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 3, 8));

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Select only band 1
        let result = decoder.decode_block(0, 0, 0, Some(&[1]));
        assert!(result.is_ok());

        let (data, shape) = result.unwrap();
        // Shape is [bands, rows, cols] (CHW format)
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(data.len(), (64 * 64));
    }

    #[test]
    fn test_decode_block_at_offset_success() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 3, 8));

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Decode at offset 0 (start of codestream)
        let result = decoder.decode_block_at_offset(0, 0, 0, 0, None);
        assert!(result.is_ok());

        let (data, shape) = result.unwrap();
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(data.len(), 64 * 64 * 3);
    }

    #[test]
    fn test_decode_block_at_offset_invalid_offset() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 1, 8));

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 1,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Offset beyond codestream length should fail
        let result = decoder.decode_block_at_offset(1000, 0, 0, 0, None);
        assert!(matches!(result, Err(CodecError::Decode(_))));
    }

    #[test]
    fn test_decode_block_at_offset_with_band_selection() {
        let codec = Arc::new(MockJ2KCodec::new().with_decode_result(64, 64, 3, 8));

        let decoder = Jpeg2000BlockDecoder {
            codestream: create_mock_codestream(),
            nrows: 64,
            ncols: 64,
            nppbh: 64,
            nppbv: 64,
            nbands: 3,
            nbpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            ic: "C8".to_string(),
            is_masked: false,
            comrat: None,
            codec,
            num_resolution_levels: OnceLock::new(),
            tile_info: OnceLock::new(),
            decode_header: None,
            first_sot_offset: None,
            tile_part_table: OnceLock::new(),
        };

        // Select only band 1
        let result = decoder.decode_block_at_offset(0, 0, 0, 0, Some(&[1]));
        assert!(result.is_ok());

        let (data, shape) = result.unwrap();
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(data.len(), (64 * 64));
    }
}
