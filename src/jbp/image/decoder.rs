//! Block decoder trait and implementations for NITF image data.
//!
//! This module provides the strategy pattern for decoding image blocks from
//! various compression formats. The [`BlockDecoder`] trait defines the interface,
//! and implementations handle specific compression types.
//!
//! # Supported Compression Types
//!
//! | IC Code | Description | Implementation |
//! |---------|-------------|----------------|
//! | NC | No compression | [`UncompressedBlockDecoder`] |
//! | NM | No compression with mask | [`UncompressedBlockDecoder`] |
//! | C8 | JPEG 2000 Part 1 | [`Jpeg2000BlockDecoder`] |
//! | CD | JPEG 2000 Part 15 (HTJ2K) | [`Jpeg2000BlockDecoder`] |
//! | M8 | JPEG 2000 with mask | Future |
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::decoder::{create_block_decoder, BlockDecoder};
//! use osml_io::jbp::image::facade::ImageSubheaderFacade;
//!
//! let decoder = create_block_decoder(&facade, image_data)?;
//! let (block_data, shape) = decoder.decode_block(0, 0, 0, None)?;
//! ```

use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::image::facade::ImageSubheaderFacade;
use crate::jbp::image::interleave::to_band_sequential;
use crate::jbp::image::types::{InterleaveMode, PixelJustification, PixelValueType};

#[cfg(feature = "openjpeg")]
use crate::jbp::j2k::{get_j2k_codec, Jpeg2000BlockDecoder};

/// Trait for decoding image blocks from various compression formats.
///
/// This trait defines the interface for block-based image decoding. Different
/// compression formats implement this trait, allowing the image asset provider
/// to delegate to the appropriate decoder based on the IC field.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent
/// block access from multiple threads.
pub trait BlockDecoder: Send + Sync {
    /// Decode a single block of image data.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    /// * `resolution_level` - Resolution level to decode (0 = full resolution, N = 1/2^N)
    /// * `bands` - Optional slice of band indices to retrieve. If `None`, all bands are returned.
    ///
    /// # Returns
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` describing the block dimensions at the requested resolution (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if the block coordinates or resolution level
    /// are out of bounds.
    fn decode_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError>;

    /// Check if a block exists at the given coordinates.
    ///
    /// For uncompressed images, this checks if the coordinates are within
    /// the block grid. For masked images, this also checks the block mask.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    ///
    /// # Returns
    /// `true` if the block exists and contains data, `false` otherwise.
    fn has_block(&self, block_row: u32, block_col: u32) -> bool;

    /// Get the compression type identifier.
    ///
    /// # Returns
    /// The IC field value (e.g., "NC", "NM", "C8").
    fn compression_type(&self) -> &str;

    /// Get the number of resolution levels.
    ///
    /// For uncompressed images, this is always 1.
    /// For JPEG 2000, this depends on the number of decomposition levels.
    ///
    /// # Returns
    /// The number of resolution levels (minimum 1).
    fn num_resolution_levels(&self) -> u32;

    /// Decode a block at a specific byte offset.
    ///
    /// This method is used for masked images where block offsets come from
    /// the Image Data Mask table rather than being calculated from block
    /// coordinates. The offset is relative to the start of the image data
    /// (after the mask table).
    ///
    /// # Arguments
    /// * `offset` - Byte offset from the start of image data to the block
    /// * `block_row` - Row index of the block (for dimension calculation)
    /// * `block_col` - Column index of the block (for dimension calculation)
    /// * `resolution_level` - Resolution level to decode (0 = full resolution)
    /// * `bands` - Optional slice of band indices to retrieve
    ///
    /// # Returns
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` describing the block dimensions (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::Decode` if the offset is invalid or decoding fails.
    ///
    /// # Requirements
    /// - 2.4: Masked block decoding using offsets from mask table
    fn decode_block_at_offset(
        &self,
        offset: u64,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError>;
}

/// Factory function to create the appropriate block decoder based on IC field.
///
/// # Arguments
/// * `subheader` - The image subheader facade for accessing metadata
/// * `image_data` - The raw image data bytes
///
/// # Returns
/// A boxed `BlockDecoder` implementation appropriate for the compression type.
///
/// # Errors
/// Returns `CodecError::Unsupported` if the compression type is not supported.
///
/// # Supported Compression Types
/// - `NC`, `NM`: Uncompressed imagery
/// - `C8`, `M8`: JPEG 2000 Part 1 (requires `openjpeg` feature)
/// - `CD`, `MD`: JPEG 2000 Part 15 HTJ2K (requires `openjpeg` feature)
pub fn create_block_decoder(
    subheader: &ImageSubheaderFacade,
    image_data: Arc<[u8]>,
) -> Result<Box<dyn BlockDecoder>, CodecError> {
    use crate::jbp::image::{is_masked_ic, unmask_ic};
    
    let ic = subheader.ic()?;
    let ic_trimmed = ic.trim();
    
    // For masked IC codes, use the underlying compression type for decoder selection
    let effective_ic = if is_masked_ic(ic_trimmed) {
        unmask_ic(ic_trimmed)
    } else {
        ic_trimmed
    };

    match effective_ic {
        "NC" => {
            let decoder = UncompressedBlockDecoder::new(subheader, image_data)?;
            Ok(Box::new(decoder))
        }
        #[cfg(feature = "openjpeg")]
        "C8" | "CD" => {
            let codec = get_j2k_codec();
            let decoder = Jpeg2000BlockDecoder::new(subheader, image_data, codec)?;
            Ok(Box::new(decoder))
        }
        #[cfg(not(feature = "openjpeg"))]
        "C8" | "CD" => Err(CodecError::Unsupported(format!(
            "JPEG 2000 compression (IC='{}') requires the 'openjpeg' feature to be enabled.",
            ic_trimmed
        ))),
        _ => Err(CodecError::Unsupported(format!(
            "Unsupported compression type: '{}'. Supported: NC, NM, C8, M8, CD, MD.",
            ic_trimmed
        ))),
    }
}

/// Block decoder for uncompressed NITF imagery (IC=NC, NM).
///
/// This decoder handles images with no compression. It reads raw pixel data
/// from the image data buffer and converts it to band-sequential format.
pub struct UncompressedBlockDecoder {
    /// The raw image data
    image_data: Arc<[u8]>,
    /// Number of rows in the image
    nrows: u32,
    /// Number of columns in the image
    ncols: u32,
    /// Number of blocks per row
    nbpr: u32,
    /// Number of blocks per column
    nbpc: u32,
    /// Number of pixels per block horizontal
    nppbh: u32,
    /// Number of pixels per block vertical
    nppbv: u32,
    /// Number of bands
    nbands: u32,
    /// Number of bits per pixel (storage size)
    nbpp: u8,
    /// Actual bits per pixel (significant bits)
    abpp: u8,
    /// Pixel value type
    pvtype: PixelValueType,
    /// Pixel justification
    pjust: PixelJustification,
    /// Interleave mode
    imode: InterleaveMode,
    /// Compression type (NC or NM)
    ic: String,
}

impl UncompressedBlockDecoder {
    /// Create a new uncompressed block decoder from image subheader.
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade
    /// * `image_data` - The raw image data bytes
    ///
    /// # Returns
    /// A new `UncompressedBlockDecoder` or an error if parameters are invalid.
    pub fn new(
        subheader: &ImageSubheaderFacade,
        image_data: Arc<[u8]>,
    ) -> Result<Self, CodecError> {
        let nrows = subheader.nrows()?;
        let ncols = subheader.ncols()?;
        let nbpr = subheader.nbpr()?;
        let nbpc = subheader.nbpc()?;
        let nppbh = subheader.nppbh()?;
        let nppbv = subheader.nppbv()?;
        let nbands = subheader.band_count()? as u32;
        let nbpp = subheader.nbpp()?;
        let abpp = subheader.abpp()?;
        let pvtype = subheader.pvtype()?;
        let pjust = subheader.pjust()?;
        let imode = subheader.imode()?;
        let ic = subheader.ic()?.trim().to_string();

        Ok(Self {
            image_data,
            nrows,
            ncols,
            nbpr,
            nbpc,
            nppbh,
            nppbv,
            nbands,
            nbpp,
            abpp,
            pvtype,
            pjust,
            imode,
            ic,
        })
    }

    /// Calculate the number of bytes per pixel.
    fn bytes_per_pixel(&self) -> usize {
        ((self.nbpp as usize) + 7) / 8
    }

    /// Calculate the size of a single block in bytes.
    fn block_size_bytes(&self) -> usize {
        let bpp = self.bytes_per_pixel();
        (self.nppbh as usize) * (self.nppbv as usize) * (self.nbands as usize) * bpp
    }

    /// Calculate the byte offset for a block based on IMODE.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    ///
    /// # Returns
    /// The byte offset into the image data where the block starts.
    fn block_offset(&self, block_row: u32, block_col: u32) -> u64 {
        let block_size = self.block_size_bytes() as u64;
        let block_index = (block_row as u64) * (self.nbpr as u64) + (block_col as u64);

        match self.imode {
            InterleaveMode::S => {
                // Band sequential: blocks are organized by band first
                // For a single block access, we need to read from multiple locations
                // But for offset calculation, we return the start of the first band's block
                let single_band_block_size = (self.nppbh as u64)
                    * (self.nppbv as u64)
                    * (self.bytes_per_pixel() as u64);
                // Return offset to first band's block
                block_index * single_band_block_size
            }
            InterleaveMode::B | InterleaveMode::P | InterleaveMode::R => {
                // For B, P, R modes, all bands for a block are stored together
                block_index * block_size
            }
        }
    }

    /// Calculate the actual dimensions of a block, handling edge blocks.
    ///
    /// Edge blocks may be smaller than the nominal block size if the image
    /// dimensions are not evenly divisible by the block size.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block
    /// * `block_col` - Column index of the block
    ///
    /// # Returns
    /// A tuple of (actual_rows, actual_cols) for the block.
    fn actual_block_dimensions(&self, block_row: u32, block_col: u32) -> (u32, u32) {
        // Calculate the starting pixel position
        let start_row = block_row * self.nppbv;
        let start_col = block_col * self.nppbh;

        // Calculate actual dimensions (may be smaller for edge blocks)
        let actual_rows = if start_row + self.nppbv > self.nrows {
            self.nrows - start_row
        } else {
            self.nppbv
        };

        let actual_cols = if start_col + self.nppbh > self.ncols {
            self.ncols - start_col
        } else {
            self.nppbh
        };

        (actual_rows, actual_cols)
    }

    /// Read a block for IMODE S (band sequential).
    ///
    /// In band sequential mode, each band's blocks are stored separately.
    /// We need to read from multiple locations and combine them.
    fn read_block_mode_s(
        &self,
        block_row: u32,
        block_col: u32,
        actual_rows: u32,
        actual_cols: u32,
    ) -> Result<Vec<u8>, CodecError> {
        let bpp = self.bytes_per_pixel();
        let blocks_per_band = (self.nbpr as usize) * (self.nbpc as usize);
        let single_band_block_size = (self.nppbh as usize) * (self.nppbv as usize) * bpp;
        let block_index = (block_row as usize) * (self.nbpr as usize) + (block_col as usize);

        let actual_pixels = (actual_rows as usize) * (actual_cols as usize);
        let mut output = Vec::with_capacity(actual_pixels * (self.nbands as usize) * bpp);

        for band in 0..self.nbands {
            let band_offset = (band as usize) * blocks_per_band * single_band_block_size;
            let block_offset = band_offset + block_index * single_band_block_size;

            // Read the block data for this band
            // Handle edge blocks by reading only the actual pixels
            for row in 0..actual_rows {
                let row_offset = block_offset + (row as usize) * (self.nppbh as usize) * bpp;
                let row_bytes = (actual_cols as usize) * bpp;

                if row_offset + row_bytes > self.image_data.len() {
                    return Err(CodecError::Decode(format!(
                        "Block data out of bounds: offset {} + {} > {}",
                        row_offset,
                        row_bytes,
                        self.image_data.len()
                    )));
                }

                output.extend_from_slice(&self.image_data[row_offset..row_offset + row_bytes]);
            }
        }

        Ok(output)
    }

    /// Read a block for IMODE B, P, or R.
    ///
    /// In these modes, all bands for a block are stored together.
    fn read_block_mode_bpr(
        &self,
        block_row: u32,
        block_col: u32,
        actual_rows: u32,
        actual_cols: u32,
    ) -> Result<Vec<u8>, CodecError> {
        let bpp = self.bytes_per_pixel();
        let offset = self.block_offset(block_row, block_col) as usize;
        let nominal_block_size = self.block_size_bytes();

        // For full blocks, we can read directly
        if actual_rows == self.nppbv && actual_cols == self.nppbh {
            if offset + nominal_block_size > self.image_data.len() {
                return Err(CodecError::Decode(format!(
                    "Block data out of bounds: offset {} + {} > {}",
                    offset,
                    nominal_block_size,
                    self.image_data.len()
                )));
            }
            return Ok(self.image_data[offset..offset + nominal_block_size].to_vec());
        }

        // For edge blocks, we need to extract only the valid pixels
        let actual_pixels = (actual_rows as usize) * (actual_cols as usize);
        let mut output = Vec::with_capacity(actual_pixels * (self.nbands as usize) * bpp);

        match self.imode {
            InterleaveMode::B => {
                // Band interleaved by block: all pixels of band 0, then band 1, etc.
                let pixels_per_band = (self.nppbh as usize) * (self.nppbv as usize);
                for band in 0..self.nbands {
                    let band_offset = offset + (band as usize) * pixels_per_band * bpp;
                    for row in 0..actual_rows {
                        let row_offset =
                            band_offset + (row as usize) * (self.nppbh as usize) * bpp;
                        let row_bytes = (actual_cols as usize) * bpp;
                        if row_offset + row_bytes > self.image_data.len() {
                            return Err(CodecError::Decode(format!(
                                "Block data out of bounds at row {}: offset {} + {} > {}",
                                row,
                                row_offset,
                                row_bytes,
                                self.image_data.len()
                            )));
                        }
                        output.extend_from_slice(&self.image_data[row_offset..row_offset + row_bytes]);
                    }
                }
            }
            InterleaveMode::P => {
                // Band interleaved by pixel: R0G0B0, R1G1B1, ...
                let pixel_size = (self.nbands as usize) * bpp;
                for row in 0..actual_rows {
                    for col in 0..actual_cols {
                        let pixel_offset = offset
                            + ((row as usize) * (self.nppbh as usize) + (col as usize)) * pixel_size;
                        if pixel_offset + pixel_size > self.image_data.len() {
                            return Err(CodecError::Decode(format!(
                                "Pixel data out of bounds at ({}, {}): offset {} + {} > {}",
                                row,
                                col,
                                pixel_offset,
                                pixel_size,
                                self.image_data.len()
                            )));
                        }
                        output.extend_from_slice(
                            &self.image_data[pixel_offset..pixel_offset + pixel_size],
                        );
                    }
                }
            }
            InterleaveMode::R => {
                // Band interleaved by row: Row0_B0, Row0_B1, Row0_B2, Row1_B0, ...
                let row_size = (self.nppbh as usize) * bpp;
                for row in 0..actual_rows {
                    for band in 0..self.nbands {
                        let row_offset = offset
                            + ((row as usize) * (self.nbands as usize) + (band as usize)) * row_size;
                        let actual_row_bytes = (actual_cols as usize) * bpp;
                        if row_offset + actual_row_bytes > self.image_data.len() {
                            return Err(CodecError::Decode(format!(
                                "Row data out of bounds at row {}, band {}: offset {} + {} > {}",
                                row,
                                band,
                                row_offset,
                                actual_row_bytes,
                                self.image_data.len()
                            )));
                        }
                        output.extend_from_slice(
                            &self.image_data[row_offset..row_offset + actual_row_bytes],
                        );
                    }
                }
            }
            InterleaveMode::S => unreachable!("IMODE S handled separately"),
        }

        Ok(output)
    }

    /// Apply band selection to block data.
    ///
    /// # Arguments
    /// * `data` - The full block data in band-sequential format
    /// * `actual_rows` - Number of rows in the block
    /// * `actual_cols` - Number of columns in the block
    /// * `bands` - The band indices to select
    ///
    /// # Returns
    /// The filtered block data containing only the selected bands.
    fn apply_band_selection(
        &self,
        data: &[u8],
        actual_rows: u32,
        actual_cols: u32,
        bands: &[u32],
    ) -> Result<Vec<u8>, CodecError> {
        let bpp = self.bytes_per_pixel();
        let pixels_per_band = (actual_rows as usize) * (actual_cols as usize);
        let band_size = pixels_per_band * bpp;

        let mut output = Vec::with_capacity(bands.len() * band_size);

        for &band_idx in bands {
            if band_idx >= self.nbands {
                return Err(CodecError::Decode(format!(
                    "Band index {} out of range (image has {} bands)",
                    band_idx, self.nbands
                )));
            }

            let band_offset = (band_idx as usize) * band_size;
            let band_end = band_offset + band_size;

            if band_end > data.len() {
                return Err(CodecError::Decode(format!(
                    "Band data out of bounds: band {} offset {} + {} > {}",
                    band_idx,
                    band_offset,
                    band_size,
                    data.len()
                )));
            }

            output.extend_from_slice(&data[band_offset..band_end]);
        }

        Ok(output)
    }
}

impl BlockDecoder for UncompressedBlockDecoder {
    fn decode_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Uncompressed images only support resolution level 0
        if resolution_level != 0 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Validate block coordinates
        if block_row >= self.nbpc || block_col >= self.nbpr {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
        }

        // Calculate actual block dimensions (handle edge blocks)
        let (actual_rows, actual_cols) = self.actual_block_dimensions(block_row, block_col);

        // Read raw block data based on IMODE
        let raw_data = match self.imode {
            InterleaveMode::S => {
                self.read_block_mode_s(block_row, block_col, actual_rows, actual_cols)?
            }
            _ => self.read_block_mode_bpr(block_row, block_col, actual_rows, actual_cols)?,
        };

        // Convert to band-sequential format if needed
        let bsq_data = if self.imode == InterleaveMode::S || self.imode == InterleaveMode::B {
            // Already in band-sequential format (S and B have same layout for single block)
            raw_data
        } else {
            // Convert from P or R to band-sequential
            to_band_sequential(
                &raw_data,
                self.imode,
                actual_rows,
                actual_cols,
                self.nbands,
                self.bytes_per_pixel(),
            )?
        };

        // Apply band selection if specified
        let num_bands = bands.map(|b| b.len() as u32).unwrap_or(self.nbands);
        let final_data = match bands {
            Some(band_indices) if !band_indices.is_empty() => {
                self.apply_band_selection(&bsq_data, actual_rows, actual_cols, band_indices)?
            }
            _ => bsq_data,
        };

        // Return shape as [bands, rows, cols] (CHW format)
        Ok((final_data, [num_bands, actual_rows, actual_cols]))
    }

    fn has_block(&self, block_row: u32, block_col: u32) -> bool {
        block_row < self.nbpc && block_col < self.nbpr
    }

    fn compression_type(&self) -> &str {
        &self.ic
    }

    fn num_resolution_levels(&self) -> u32 {
        // Uncompressed images have only one resolution level
        1
    }

    fn decode_block_at_offset(
        &self,
        offset: u64,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Uncompressed images only support resolution level 0
        if resolution_level != 0 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Validate block coordinates for dimension calculation
        if block_row >= self.nbpc || block_col >= self.nbpr {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
        }

        // Calculate actual block dimensions (handle edge blocks)
        let (actual_rows, actual_cols) = self.actual_block_dimensions(block_row, block_col);

        // Calculate expected block size
        let bpp = self.bytes_per_pixel();
        let block_pixels = (actual_rows as usize) * (actual_cols as usize);
        let block_size = block_pixels * (self.nbands as usize) * bpp;

        // Validate offset is within bounds
        let offset_usize = offset as usize;
        if offset_usize + block_size > self.image_data.len() {
            return Err(CodecError::Decode(format!(
                "Block offset {} + size {} exceeds image data length {}",
                offset, block_size, self.image_data.len()
            )));
        }

        // Read raw block data at the specified offset
        let raw_data = self.image_data[offset_usize..offset_usize + block_size].to_vec();

        // Convert to band-sequential format if needed (same as decode_block)
        let bsq_data = if self.imode == InterleaveMode::S || self.imode == InterleaveMode::B {
            // Already in band-sequential format (S and B have same layout for single block)
            raw_data
        } else {
            // Convert from P or R to band-sequential
            to_band_sequential(
                &raw_data,
                self.imode,
                actual_rows,
                actual_cols,
                self.nbands,
                bpp,
            )?
        };

        // Apply band selection if specified
        let num_bands = bands.map(|b| b.len() as u32).unwrap_or(self.nbands);
        let final_data = match bands {
            Some(band_indices) if !band_indices.is_empty() => {
                self.apply_band_selection(&bsq_data, actual_rows, actual_cols, band_indices)?
            }
            _ => bsq_data,
        };

        // Return shape as [bands, rows, cols] (CHW format)
        Ok((final_data, [num_bands, actual_rows, actual_cols]))
    }
}

// Expose internal types for testing
#[cfg(test)]
pub(crate) use self::UncompressedBlockDecoder as TestUncompressedBlockDecoder;


#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a simple UncompressedBlockDecoder for testing
    /// without needing a full ImageSubheaderFacade
    fn create_test_decoder(
        nrows: u32,
        ncols: u32,
        nbpr: u32,
        nbpc: u32,
        nppbh: u32,
        nppbv: u32,
        nbands: u32,
        nbpp: u8,
        imode: InterleaveMode,
        image_data: Vec<u8>,
    ) -> UncompressedBlockDecoder {
        UncompressedBlockDecoder {
            image_data: Arc::from(image_data),
            nrows,
            ncols,
            nbpr,
            nbpc,
            nppbh,
            nppbv,
            nbands,
            nbpp,
            abpp: nbpp,
            pvtype: PixelValueType::UnsignedInt,
            pjust: PixelJustification::Right,
            imode,
            ic: "NC".to_string(),
        }
    }

    /// Create test image data with known pixel values
    /// Each pixel value encodes its position: row * 100 + col + band * 10000
    fn create_test_image_data_bsq(
        nrows: u32,
        ncols: u32,
        nbands: u32,
        bytes_per_pixel: usize,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        for band in 0..nbands {
            for row in 0..nrows {
                for col in 0..ncols {
                    let value = (band * 10000 + row * 100 + col) as u32;
                    match bytes_per_pixel {
                        1 => data.push((value % 256) as u8),
                        2 => data.extend_from_slice(&(value as u16).to_be_bytes()),
                        4 => data.extend_from_slice(&value.to_be_bytes()),
                        _ => panic!("Unsupported bytes_per_pixel"),
                    }
                }
            }
        }
        data
    }

    /// Create test image data in BIP format (band interleaved by pixel)
    fn create_test_image_data_bip(
        nrows: u32,
        ncols: u32,
        nbands: u32,
        bytes_per_pixel: usize,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        for row in 0..nrows {
            for col in 0..ncols {
                for band in 0..nbands {
                    let value = (band * 10000 + row * 100 + col) as u32;
                    match bytes_per_pixel {
                        1 => data.push((value % 256) as u8),
                        2 => data.extend_from_slice(&(value as u16).to_be_bytes()),
                        4 => data.extend_from_slice(&value.to_be_bytes()),
                        _ => panic!("Unsupported bytes_per_pixel"),
                    }
                }
            }
        }
        data
    }

    mod has_block_tests {
        use super::*;

        #[test]
        fn valid_block_coordinates() {
            let decoder = create_test_decoder(
                64, 64,  // nrows, ncols
                2, 2,    // nbpr, nbpc (2x2 block grid)
                32, 32,  // nppbh, nppbv
                1,       // nbands
                8,       // nbpp
                InterleaveMode::B,
                vec![0u8; 64 * 64],
            );

            assert!(decoder.has_block(0, 0));
            assert!(decoder.has_block(0, 1));
            assert!(decoder.has_block(1, 0));
            assert!(decoder.has_block(1, 1));
        }

        #[test]
        fn invalid_block_coordinates() {
            let decoder = create_test_decoder(
                64, 64,
                2, 2,
                32, 32,
                1,
                8,
                InterleaveMode::B,
                vec![0u8; 64 * 64],
            );

            assert!(!decoder.has_block(2, 0));
            assert!(!decoder.has_block(0, 2));
            assert!(!decoder.has_block(2, 2));
            assert!(!decoder.has_block(100, 100));
        }
    }

    mod decode_block_tests {
        use super::*;

        #[test]
        fn decode_single_block_single_band() {
            // 4x4 image, single block, single band
            let data: Vec<u8> = (0..16).collect();
            let decoder = create_test_decoder(
                4, 4,    // nrows, ncols
                1, 1,    // nbpr, nbpc
                4, 4,    // nppbh, nppbv
                1,       // nbands
                8,       // nbpp
                InterleaveMode::B,
                data.clone(),
            );

            let (block_data, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            // Shape is [bands, rows, cols] (CHW format)
            assert_eq!(shape, [1, 4, 4]);
            assert_eq!(block_data, data);
        }

        #[test]
        fn decode_block_invalid_coordinates() {
            let decoder = create_test_decoder(
                4, 4,
                1, 1,
                4, 4,
                1,
                8,
                InterleaveMode::B,
                vec![0u8; 16],
            );

            let result = decoder.decode_block(1, 0, 0, None);
            assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(1, 0, 0))));

            let result = decoder.decode_block(0, 1, 0, None);
            assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(0, 1, 0))));
        }

        #[test]
        fn decode_block_with_band_selection() {
            // 4x4 image, 3 bands, BSQ format
            let data = create_test_image_data_bsq(4, 4, 3, 1);
            let decoder = create_test_decoder(
                4, 4,
                1, 1,
                4, 4,
                3,
                8,
                InterleaveMode::B,
                data,
            );

            // Select only band 1
            let (block_data, shape) = decoder.decode_block(0, 0, 0, Some(&[1])).unwrap();
            // Shape is [bands, rows, cols] (CHW format)
            assert_eq!(shape, [1, 4, 4]);
            assert_eq!(block_data.len(), 16); // 4x4x1 band

            // Select bands 0 and 2
            let (block_data, shape) = decoder.decode_block(0, 0, 0, Some(&[0, 2])).unwrap();
            assert_eq!(shape, [2, 4, 4]);
            assert_eq!(block_data.len(), 32); // 4x4x2 bands
        }

        #[test]
        fn decode_edge_block() {
            // 6x6 image with 4x4 blocks = 2x2 block grid
            // For IMODE B, data is organized by blocks, with all bands per block
            // Block layout:
            //   Block(0,0): rows 0-3, cols 0-3
            //   Block(0,1): rows 0-3, cols 4-5 (edge - only 2 cols)
            //   Block(1,0): rows 4-5, cols 0-3 (edge - only 2 rows)
            //   Block(1,1): rows 4-5, cols 4-5 (corner - 2x2)
            
            // Create data organized by blocks for IMODE B
            // Each block contains all its pixels sequentially
            let mut data = Vec::new();
            
            // Block (0,0): 4x4 = 16 pixels
            for row in 0..4u8 {
                for col in 0..4u8 {
                    data.push(row * 10 + col);
                }
            }
            // Block (0,1): 4x2 = 8 pixels (edge block, but stored as 4x4 with padding)
            // Actually for edge blocks, we need to store the full block size
            for row in 0..4u8 {
                for col in 0..4u8 {
                    if col < 2 {
                        data.push(row * 10 + col + 4); // col offset by 4
                    } else {
                        data.push(0); // padding
                    }
                }
            }
            // Block (1,0): 2x4 = 8 pixels (edge block)
            for row in 0..4u8 {
                for col in 0..4u8 {
                    if row < 2 {
                        data.push((row + 4) * 10 + col); // row offset by 4
                    } else {
                        data.push(0); // padding
                    }
                }
            }
            // Block (1,1): 2x2 = 4 pixels (corner block)
            for row in 0..4u8 {
                for col in 0..4u8 {
                    if row < 2 && col < 2 {
                        data.push((row + 4) * 10 + col + 4);
                    } else {
                        data.push(0); // padding
                    }
                }
            }
            
            let decoder = create_test_decoder(
                6, 6,
                2, 2,    // 2x2 block grid
                4, 4,    // 4x4 block size
                1,
                8,
                InterleaveMode::B,
                data,
            );

            // Top-left block: full 4x4
            let (block_data, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            // Shape is [bands, rows, cols] (CHW format)
            assert_eq!(shape, [1, 4, 4]);
            assert_eq!(block_data.len(), 16);

            // Top-right block: 4 rows x 2 cols (edge)
            let (block_data, shape) = decoder.decode_block(0, 1, 0, None).unwrap();
            assert_eq!(shape, [1, 4, 2]);
            assert_eq!(block_data.len(), 8);

            // Bottom-left block: 2 rows x 4 cols (edge)
            let (block_data, shape) = decoder.decode_block(1, 0, 0, None).unwrap();
            assert_eq!(shape, [1, 2, 4]);
            assert_eq!(block_data.len(), 8);

            // Bottom-right block: 2 rows x 2 cols (corner)
            let (block_data, shape) = decoder.decode_block(1, 1, 0, None).unwrap();
            assert_eq!(shape, [1, 2, 2]);
            assert_eq!(block_data.len(), 4);
        }
    }

    mod decode_block_at_offset_tests {
        use super::*;

        #[test]
        fn decode_block_at_offset_single_block() {
            // 4x4 image, single block, single band
            let data: Vec<u8> = (0..16).collect();
            let decoder = create_test_decoder(
                4, 4,    // nrows, ncols
                1, 1,    // nbpr, nbpc
                4, 4,    // nppbh, nppbv
                1,       // nbands
                8,       // nbpp
                InterleaveMode::B,
                data.clone(),
            );

            // Decode at offset 0
            let (block_data, shape) = decoder.decode_block_at_offset(0, 0, 0, 0, None).unwrap();
            assert_eq!(shape, [1, 4, 4]);
            assert_eq!(block_data, data);
        }

        #[test]
        fn decode_block_at_offset_multi_band() {
            // 4x4 image, single block, 3 bands
            let data = create_test_image_data_bsq(4, 4, 3, 1);
            let decoder = create_test_decoder(
                4, 4,
                1, 1,
                4, 4,
                3,
                8,
                InterleaveMode::B,
                data.clone(),
            );

            // Decode at offset 0
            let (block_data, shape) = decoder.decode_block_at_offset(0, 0, 0, 0, None).unwrap();
            assert_eq!(shape, [3, 4, 4]);
            assert_eq!(block_data.len(), 48); // 4x4x3 bands
        }

        #[test]
        fn decode_block_at_offset_with_band_selection() {
            // 4x4 image, single block, 3 bands
            let data = create_test_image_data_bsq(4, 4, 3, 1);
            let decoder = create_test_decoder(
                4, 4,
                1, 1,
                4, 4,
                3,
                8,
                InterleaveMode::B,
                data,
            );

            // Select only band 1
            let (block_data, shape) = decoder.decode_block_at_offset(0, 0, 0, 0, Some(&[1])).unwrap();
            assert_eq!(shape, [1, 4, 4]);
            assert_eq!(block_data.len(), 16);
        }

        #[test]
        fn decode_block_at_offset_invalid_resolution() {
            let decoder = create_test_decoder(
                4, 4, 1, 1, 4, 4, 1, 8,
                InterleaveMode::B,
                vec![0u8; 16],
            );

            // Uncompressed images only support resolution level 0
            let result = decoder.decode_block_at_offset(0, 0, 0, 1, None);
            assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(0, 0, 1))));
        }

        #[test]
        fn decode_block_at_offset_out_of_bounds() {
            let decoder = create_test_decoder(
                4, 4, 1, 1, 4, 4, 1, 8,
                InterleaveMode::B,
                vec![0u8; 16],
            );

            // Offset beyond data length
            let result = decoder.decode_block_at_offset(100, 0, 0, 0, None);
            assert!(matches!(result, Err(CodecError::Decode(_))));
        }

        #[test]
        fn decode_block_at_offset_nonzero_offset() {
            // Create data with two blocks worth of data
            // First block: all zeros, Second block: all ones
            let mut data = vec![0u8; 16]; // First block
            data.extend(vec![1u8; 16]);   // Second block
            
            let decoder = create_test_decoder(
                4, 8,    // 4 rows, 8 cols
                2, 1,    // 2 blocks per row, 1 block per col
                4, 4,    // 4x4 block size
                1,       // 1 band
                8,       // 8 bits per pixel
                InterleaveMode::B,
                data,
            );

            // Decode second block at offset 16
            let (block_data, shape) = decoder.decode_block_at_offset(16, 0, 1, 0, None).unwrap();
            assert_eq!(shape, [1, 4, 4]);
            assert_eq!(block_data.len(), 16);
            // All pixels should be 1
            assert!(block_data.iter().all(|&b| b == 1));
        }
    }

    mod compression_type_tests {
        use super::*;

        #[test]
        fn compression_type_nc() {
            let decoder = create_test_decoder(
                4, 4, 1, 1, 4, 4, 1, 8,
                InterleaveMode::B,
                vec![0u8; 16],
            );
            assert_eq!(decoder.compression_type(), "NC");
        }

        #[test]
        fn num_resolution_levels() {
            let decoder = create_test_decoder(
                4, 4, 1, 1, 4, 4, 1, 8,
                InterleaveMode::B,
                vec![0u8; 16],
            );
            assert_eq!(decoder.num_resolution_levels(), 1);
        }
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Generate valid InterleaveMode
    fn interleave_mode_strategy() -> impl Strategy<Value = InterleaveMode> {
        prop_oneof![
            Just(InterleaveMode::B),
            Just(InterleaveMode::P),
            Just(InterleaveMode::R),
            Just(InterleaveMode::S),
        ]
    }

    /// Generate valid image dimensions for testing (small for speed)
    fn image_params_strategy() -> impl Strategy<Value = (u32, u32, u32, u32, u32)> {
        (
            1u32..=4,   // nppbh (block width)
            1u32..=4,   // nppbv (block height)
            1u32..=3,   // nbpr (blocks per row)
            1u32..=3,   // nbpc (blocks per column)
            1u32..=4,   // nbands
        )
    }

    /// Create test decoder with generated parameters
    fn create_decoder_with_data(
        nppbh: u32,
        nppbv: u32,
        nbpr: u32,
        nbpc: u32,
        nbands: u32,
        imode: InterleaveMode,
    ) -> (UncompressedBlockDecoder, Vec<Vec<Vec<Vec<u8>>>>) {
        let nrows = nbpc * nppbv;
        let ncols = nbpr * nppbh;
        
        // Create expected values organized as [block_row][block_col][band][pixel_in_block]
        // This makes verification easier
        let mut expected: Vec<Vec<Vec<Vec<u8>>>> = Vec::new();
        
        // Create data in the appropriate format based on IMODE
        let data = match imode {
            InterleaveMode::S => {
                // Band sequential: all blocks of band 0, then all blocks of band 1, etc.
                let mut d = Vec::new();
                for band in 0..nbands {
                    for block_row in 0..nbpc {
                        for block_col in 0..nbpr {
                            for row in 0..nppbv {
                                for col in 0..nppbh {
                                    let val = ((band * 100 + block_row * 40 + block_col * 10 + row * 4 + col) % 256) as u8;
                                    d.push(val);
                                }
                            }
                        }
                    }
                }
                
                // Build expected values
                for block_row in 0..nbpc {
                    let mut br = Vec::new();
                    for block_col in 0..nbpr {
                        let mut bc = Vec::new();
                        for band in 0..nbands {
                            let mut bb = Vec::new();
                            for row in 0..nppbv {
                                for col in 0..nppbh {
                                    let val = ((band * 100 + block_row * 40 + block_col * 10 + row * 4 + col) % 256) as u8;
                                    bb.push(val);
                                }
                            }
                            bc.push(bb);
                        }
                        br.push(bc);
                    }
                    expected.push(br);
                }
                
                d
            }
            InterleaveMode::B => {
                // Band interleaved by block: for each block, all bands together
                let mut d = Vec::new();
                for block_row in 0..nbpc {
                    let mut br = Vec::new();
                    for block_col in 0..nbpr {
                        let mut bc = Vec::new();
                        for band in 0..nbands {
                            let mut bb = Vec::new();
                            for row in 0..nppbv {
                                for col in 0..nppbh {
                                    let val = ((band * 100 + block_row * 40 + block_col * 10 + row * 4 + col) % 256) as u8;
                                    d.push(val);
                                    bb.push(val);
                                }
                            }
                            bc.push(bb);
                        }
                        br.push(bc);
                    }
                    expected.push(br);
                }
                d
            }
            InterleaveMode::P => {
                // Band interleaved by pixel: for each block, pixels with all bands interleaved
                let mut d = Vec::new();
                for block_row in 0..nbpc {
                    let mut br = Vec::new();
                    for block_col in 0..nbpr {
                        let mut bc: Vec<Vec<u8>> = (0..nbands).map(|_| Vec::new()).collect();
                        for row in 0..nppbv {
                            for col in 0..nppbh {
                                for band in 0..nbands {
                                    let val = ((band * 100 + block_row * 40 + block_col * 10 + row * 4 + col) % 256) as u8;
                                    d.push(val);
                                    bc[band as usize].push(val);
                                }
                            }
                        }
                        br.push(bc);
                    }
                    expected.push(br);
                }
                d
            }
            InterleaveMode::R => {
                // Band interleaved by row: for each block, rows with bands interleaved
                let mut d = Vec::new();
                for block_row in 0..nbpc {
                    let mut br = Vec::new();
                    for block_col in 0..nbpr {
                        let mut bc: Vec<Vec<u8>> = (0..nbands).map(|_| Vec::new()).collect();
                        for row in 0..nppbv {
                            for band in 0..nbands {
                                for col in 0..nppbh {
                                    let val = ((band * 100 + block_row * 40 + block_col * 10 + row * 4 + col) % 256) as u8;
                                    d.push(val);
                                    bc[band as usize].push(val);
                                }
                            }
                        }
                        br.push(bc);
                    }
                    expected.push(br);
                }
                d
            }
        };

        let decoder = UncompressedBlockDecoder {
            image_data: Arc::from(data.clone()),
            nrows,
            ncols,
            nbpr,
            nbpc,
            nppbh,
            nppbv,
            nbands,
            nbpp: 8,
            abpp: 8,
            pvtype: PixelValueType::UnsignedInt,
            pjust: PixelJustification::Right,
            imode,
            ic: "NC".to_string(),
        };

        (decoder, expected)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 6: Block Access Returns Correct Data
        /// For any valid block coordinates and image data, the data returned by
        /// get_block() SHALL contain the correct pixel values for that block region.
        /// **Validates: Requirements 6.1, 6.2, 6.5**
        #[test]
        fn block_access_returns_correct_data(
            (nppbh, nppbv, nbpr, nbpc, nbands) in image_params_strategy(),
            imode in interleave_mode_strategy(),
            block_row in 0u32..3,
            block_col in 0u32..3,
        ) {
            // Skip if block coordinates are out of range
            if block_row >= nbpc || block_col >= nbpr {
                return Ok(());
            }

            let (decoder, expected) = create_decoder_with_data(
                nppbh, nppbv, nbpr, nbpc, nbands, imode
            );

            let result = decoder.decode_block(block_row, block_col, 0, None);
            prop_assert!(result.is_ok(), "decode_block should succeed for valid coordinates");

            let (block_data, shape) = result.unwrap();
            
            // Verify shape is correct - shape is [bands, rows, cols] (CHW format)
            prop_assert_eq!(shape[0], nbands, "Band count should match");
            prop_assert_eq!(shape[1], nppbv, "Block rows should match nppbv");
            prop_assert_eq!(shape[2], nppbh, "Block cols should match nppbh");
            
            // Verify data size matches shape
            let expected_size = (shape[0] * shape[1] * shape[2]) as usize;
            prop_assert_eq!(
                block_data.len(), expected_size,
                "Data size should match shape: {} vs {}x{}x{}",
                block_data.len(), shape[0], shape[1], shape[2]
            );

            // Verify pixel values are correct (output is in BSQ format)
            let expected_block = &expected[block_row as usize][block_col as usize];
            
            for band in 0..nbands {
                let band_offset = (band * nppbv * nppbh) as usize;
                let expected_band = &expected_block[band as usize];
                
                for i in 0..expected_band.len() {
                    let actual = block_data[band_offset + i];
                    let expected_val = expected_band[i];
                    
                    prop_assert_eq!(
                        actual, expected_val,
                        "Pixel mismatch at band={}, pixel={} for block ({}, {}), imode={:?}",
                        band, i, block_row, block_col, imode
                    );
                }
            }
        }

        /// Property 7: Invalid Block Coordinates Return Error
        /// For any block coordinates outside the valid range, get_block() SHALL
        /// return an InvalidBlockCoordinates error.
        /// **Validates: Requirements 6.3, 6.4**
        #[test]
        fn invalid_block_coordinates_return_error(
            (nppbh, nppbv, nbpr, nbpc, nbands) in image_params_strategy(),
            imode in interleave_mode_strategy(),
            extra_row in 0u32..5,
            extra_col in 0u32..5,
        ) {
            let (decoder, _expected) = create_decoder_with_data(
                nppbh, nppbv, nbpr, nbpc, nbands, imode
            );

            // Test row out of bounds
            let invalid_row = nbpc + extra_row;
            let result = decoder.decode_block(invalid_row, 0, 0, None);
            prop_assert!(
                matches!(result, Err(CodecError::InvalidBlockCoordinates(r, 0, 0)) if r == invalid_row),
                "Should return InvalidBlockCoordinates for row {} >= nbpc {}",
                invalid_row, nbpc
            );

            // Test col out of bounds
            let invalid_col = nbpr + extra_col;
            let result = decoder.decode_block(0, invalid_col, 0, None);
            prop_assert!(
                matches!(result, Err(CodecError::InvalidBlockCoordinates(0, c, 0)) if c == invalid_col),
                "Should return InvalidBlockCoordinates for col {} >= nbpr {}",
                invalid_col, nbpr
            );

            // Test both out of bounds
            let result = decoder.decode_block(invalid_row, invalid_col, 0, None);
            prop_assert!(
                matches!(result, Err(CodecError::InvalidBlockCoordinates(r, c, 0)) if r == invalid_row && c == invalid_col),
                "Should return InvalidBlockCoordinates for ({}, {}) >= ({}, {})",
                invalid_row, invalid_col, nbpc, nbpr
            );
        }

        /// Property 4: Pixel Data Round-Trip per IMODE
        /// For any valid image pixel data and interleave mode (B, P, R, S),
        /// writing the pixel data and then reading it back with the same IMODE
        /// SHALL produce byte-identical output.
        /// **Validates: Requirements 5.1-5.6, 10.1-10.8, 17.2**
        #[test]
        fn pixel_data_round_trip_per_imode(
            (nppbh, nppbv, nbpr, nbpc, nbands) in image_params_strategy(),
            imode in interleave_mode_strategy(),
        ) {
            let (decoder, expected) = create_decoder_with_data(
                nppbh, nppbv, nbpr, nbpc, nbands, imode
            );

            // Read all blocks and verify they match expected values
            for block_row in 0..nbpc {
                for block_col in 0..nbpr {
                    let result = decoder.decode_block(block_row, block_col, 0, None);
                    prop_assert!(result.is_ok(), "decode_block should succeed");

                    let (block_data, shape) = result.unwrap();
                    
                    // Verify shape - shape is [bands, rows, cols] (CHW format)
                    prop_assert_eq!(shape[0], nbands, "Band count should match");
                    prop_assert_eq!(shape[1], nppbv, "Block rows should match");
                    prop_assert_eq!(shape[2], nppbh, "Block cols should match");

                    // Verify data matches expected (output is always BSQ)
                    let expected_block = &expected[block_row as usize][block_col as usize];
                    
                    for band in 0..nbands {
                        let band_offset = (band * nppbv * nppbh) as usize;
                        let expected_band = &expected_block[band as usize];
                        
                        for i in 0..expected_band.len() {
                            prop_assert_eq!(
                                block_data[band_offset + i],
                                expected_band[i],
                                "Pixel mismatch at block ({}, {}), band {}, pixel {} for imode {:?}",
                                block_row, block_col, band, i, imode
                            );
                        }
                    }
                }
            }
        }
    }
}
