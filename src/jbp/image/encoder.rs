//! Block encoder trait and implementations for NITF image data.
//!
//! This module provides the strategy pattern for encoding image blocks to
//! various compression formats. The [`BlockEncoder`] trait defines the interface,
//! and implementations handle specific compression types.
//!
//! # Supported Compression Types
//!
//! | IC Code | Description | Implementation |
//! |---------|-------------|----------------|
//! | NC | No compression | [`UncompressedBlockEncoder`] |
//! | C3 | JPEG DCT | [`JpegNitfBlockEncoder`] |
//! | M3 | JPEG DCT with mask | [`JpegNitfBlockEncoder`] |
//! | I1 | Downsampled JPEG | [`JpegNitfBlockEncoder`] |
//! | C8 | JPEG 2000 Part 1 | [`Jpeg2000BlockEncoder`] |
//! | CD | JPEG 2000 Part 15 (HTJ2K) | [`Jpeg2000BlockEncoder`] |
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::encoder::{create_block_encoder, BlockEncoder};
//! use osml_io::jbp::image::types::InterleaveMode;
//!
//! let mut encoder = create_block_encoder(
//!     "NC", 64, 64, 3, 8, false, InterleaveMode::B, 32, 32, None, None
//! )?;
//! // Shape is [bands, rows, cols] (CHW format)
//! encoder.encode_block(0, 0, &block_data, [3, 32, 32])?;
//! let encoded = Box::new(encoder).finalize()?;
//! ```

use std::collections::HashSet;

use crate::error::CodecError;
use crate::jbp::image::interleave::from_band_sequential;
use crate::jbp::image::types::InterleaveMode;

#[cfg(feature = "openjpeg")]
use crate::jbp::j2k::{J2KEncodingHints, Jpeg2000BlockEncoder};

#[cfg(feature = "libjpeg-turbo")]
use crate::jbp::jpeg::{JpegBlockEncoder, JpegColorSpace, JpegComrat};

/// Convert a byte buffer from native-endian to big-endian.
///
/// NITF mandates big-endian for uncompressed multi-byte pixel data
/// (JBP Section 4.6.2, requirement JBP-2021.2-013). The internal `Vec<u8>`
/// contract uses native-endian, so this converts at the write boundary.
///
/// For single-byte data (`bytes_per_pixel == 1`) this is a no-op.
#[inline]
pub fn swap_ne_to_be(data: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
    if cfg!(target_endian = "big") || bytes_per_pixel <= 1 {
        return data.to_vec();
    }
    match bytes_per_pixel {
        2 => data
            .chunks_exact(2)
            .flat_map(|c| u16::from_ne_bytes([c[0], c[1]]).to_be_bytes())
            .collect(),
        4 => data
            .chunks_exact(4)
            .flat_map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]).to_be_bytes())
            .collect(),
        8 => data
            .chunks_exact(8)
            .flat_map(|c| {
                u64::from_ne_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
                    .to_be_bytes()
            })
            .collect(),
        _ => data.to_vec(),
    }
}

/// Trait for encoding image blocks to various compression formats.
///
/// This trait is symmetric to `BlockDecoder` and defines the interface for
/// block-based image encoding. Different compression formats implement this
/// trait, allowing the writer to delegate to the appropriate encoder based
/// on the IC field.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent
/// block encoding from multiple threads.
pub trait BlockEncoder: Send + Sync {
    /// Encode a single block of image data.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    /// * `data` - Pixel data in band-sequential format
    /// * `shape` - Shape of the data as [bands, rows, cols] (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if coordinates are out of bounds.
    /// Returns `CodecError::Encode` if data size doesn't match shape.
    fn encode_block(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        shape: [u32; 3],
    ) -> Result<(), CodecError>;

    /// Mark a block as intentionally skipped (for masked images).
    ///
    /// This method is used for masked images where some blocks are intentionally
    /// not encoded. Calling this method marks the block as "handled" so that
    /// `finalize()` won't fail due to missing blocks.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid (0-indexed)
    /// * `block_col` - Column index of the block in the block grid (0-indexed)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if coordinates are out of bounds.
    fn skip_block(&mut self, block_row: u32, block_col: u32) -> Result<(), CodecError>;

    /// Finalize encoding and return the complete encoded image data.
    ///
    /// This method must be called after all blocks have been encoded.
    /// The returned data is ready to be written to the NITF image segment.
    ///
    /// # Errors
    /// Returns `CodecError::Encode` if not all blocks have been encoded.
    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError>;

    /// Get the compression type identifier.
    ///
    /// # Returns
    /// The IC field value (e.g., "NC", "C8").
    fn compression_type(&self) -> &str;

    /// Get the block grid dimensions.
    ///
    /// # Returns
    /// A tuple of (num_block_rows, num_block_cols).
    fn block_grid_size(&self) -> (u32, u32);

    /// Get the output block dimensions in pixels.
    ///
    /// # Returns
    /// A tuple of (block_height, block_width).
    fn block_dimensions(&self) -> (u32, u32);
}

/// Factory function to create the appropriate block encoder based on IC field.
///
/// # Arguments
/// * `ic` - Image compression code (e.g., "NC", "C8", "CD", "C3", "M3", "I1")
/// * `nrows` - Image height in pixels
/// * `ncols` - Image width in pixels
/// * `nbands` - Number of bands
/// * `nbpp` - Bits per pixel
/// * `is_signed` - Whether pixel values are signed (for J2K encoding)
/// * `imode` - Target interleave mode (ignored for J2K, which always uses IMODE=B)
/// * `nppbh` - Block width in pixels
/// * `nppbv` - Block height in pixels
/// * `j2k_hints` - Optional JPEG 2000 encoding hints (required for C8/CD)
/// * `jpeg_comrat` - Optional JPEG COMRAT string for quality (for C3/M3/I1)
///
/// # Returns
/// A boxed `BlockEncoder` implementation appropriate for the compression type.
///
/// # Errors
/// Returns `CodecError::Unsupported` if the compression type is not supported.
///
/// # Supported Compression Types
/// - `NC`: Uncompressed imagery
/// - `C3`: JPEG DCT (requires `libjpeg-turbo` feature)
/// - `M3`: JPEG DCT with mask (requires `libjpeg-turbo` feature)
/// - `I1`: Downsampled JPEG (requires `libjpeg-turbo` feature)
/// - `C8`: JPEG 2000 Part 1 (requires `openjpeg` feature)
/// - `CD`: JPEG 2000 Part 15 HTJ2K (requires `openjpeg` feature)
#[allow(clippy::too_many_arguments)]
pub fn create_block_encoder(
    ic: &str,
    nrows: u32,
    ncols: u32,
    nbands: u32,
    nbpp: u8,
    is_signed: bool,
    imode: InterleaveMode,
    nppbh: u32,
    nppbv: u32,
    #[cfg(feature = "openjpeg")] j2k_hints: Option<&J2KEncodingHints>,
    #[cfg(not(feature = "openjpeg"))] _j2k_hints: Option<&()>,
    #[cfg(feature = "libjpeg-turbo")] jpeg_comrat: Option<&str>,
    #[cfg(not(feature = "libjpeg-turbo"))] _jpeg_comrat: Option<&str>,
) -> Result<Box<dyn BlockEncoder>, CodecError> {
    match ic.trim() {
        "NC" => Ok(Box::new(UncompressedBlockEncoder::new(
            nrows, ncols, nbands, nbpp, imode, nppbh, nppbv,
        ))),
        #[cfg(feature = "libjpeg-turbo")]
        "C3" | "M3" => {
            let encoder = JpegNitfBlockEncoder::new(
                ic.trim(),
                nrows,
                ncols,
                nbands,
                nbpp,
                imode,
                nppbh,
                nppbv,
                jpeg_comrat,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(feature = "libjpeg-turbo")]
        "I1" => {
            // I1 is a single-block downsampled JPEG with dimension constraint
            let encoder = JpegNitfBlockEncoder::new(
                "I1",
                nrows,
                ncols,
                nbands,
                nbpp,
                imode,
                nppbh,
                nppbv,
                jpeg_comrat,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(not(feature = "libjpeg-turbo"))]
        "C3" | "M3" | "I1" => Err(CodecError::Unsupported(format!(
            "JPEG DCT compression (IC='{}') requires the 'libjpeg-turbo' feature to be enabled.",
            ic.trim()
        ))),
        #[cfg(feature = "openjpeg")]
        "C8" => {
            let hints = j2k_hints
                .cloned()
                .unwrap_or_default();
            let encoder = Jpeg2000BlockEncoder::new(
                nrows, ncols, nbands, nbpp, is_signed, nppbh, nppbv, &hints,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(feature = "openjpeg")]
        "CD" => {
            let hints = j2k_hints
                .cloned()
                .map(|mut h| {
                    h.htj2k = true;
                    h
                })
                .unwrap_or_else(|| J2KEncodingHints::htj2k(false));
            let encoder = Jpeg2000BlockEncoder::new(
                nrows, ncols, nbands, nbpp, is_signed, nppbh, nppbv, &hints,
            )?;
            Ok(Box::new(encoder))
        }
        #[cfg(not(feature = "openjpeg"))]
        "C8" | "CD" => Err(CodecError::Unsupported(format!(
            "JPEG 2000 compression (IC='{}') requires the 'openjpeg' feature to be enabled.",
            ic.trim()
        ))),
        _ => Err(CodecError::Unsupported(format!(
            "Unsupported compression type for encoding: '{}'. Supported: NC, C3, M3, I1, C8, CD.",
            ic
        ))),
    }
}
/// Assembles output tiles from input tiles of potentially different sizes.
///
/// When the output tile size differs from the input tile size, this helper
/// reads the necessary input tiles and assembles them into output tiles.
/// The output is always in band-sequential (BSQ) format, ready for encoding.
///
/// # Example
///
/// ```ignore
/// use osml_io::jbp::image::encoder::TileAssembler;
///
/// let assembler = TileAssembler::new(&source_provider, 256, 256);
/// let (grid_rows, grid_cols) = assembler.output_grid_size();
/// for row in 0..grid_rows {
///     for col in 0..grid_cols {
///         let (data, shape) = assembler.get_output_tile(row, col)?;
///         encoder.encode_block(row, col, &data, shape)?;
///     }
/// }
/// ```
pub struct TileAssembler<'a> {
    /// Source image provider
    source: &'a dyn crate::traits::ImageAssetProvider,
    /// Output tile width in pixels
    output_tile_width: u32,
    /// Output tile height in pixels
    output_tile_height: u32,
    /// Source tile width (from provider)
    source_tile_width: u32,
    /// Source tile height (from provider)
    source_tile_height: u32,
    /// Image width in pixels
    image_width: u32,
    /// Image height in pixels
    image_height: u32,
    /// Number of bands
    num_bands: u32,
    /// Bytes per pixel
    bytes_per_pixel: usize,
}

impl<'a> TileAssembler<'a> {
    /// Create a new tile assembler.
    ///
    /// # Arguments
    /// * `source` - Source image provider to read tiles from
    /// * `output_tile_width` - Width of output tiles in pixels
    /// * `output_tile_height` - Height of output tiles in pixels
    pub fn new(
        source: &'a dyn crate::traits::ImageAssetProvider,
        output_tile_width: u32,
        output_tile_height: u32,
    ) -> Self {
        Self {
            source,
            output_tile_width,
            output_tile_height,
            source_tile_width: source.num_pixels_per_block_horizontal(),
            source_tile_height: source.num_pixels_per_block_vertical(),
            image_width: source.num_columns(),
            image_height: source.num_rows(),
            num_bands: source.num_bands(),
            bytes_per_pixel: source.num_bits_per_pixel().div_ceil(8) as usize,
        }
    }

    /// Get the output block grid size.
    ///
    /// # Returns
    /// A tuple of (num_rows, num_cols) representing the number of output tiles
    /// needed to cover the entire image.
    pub fn output_grid_size(&self) -> (u32, u32) {
        let cols = self.image_width.div_ceil(self.output_tile_width);
        let rows = self.image_height.div_ceil(self.output_tile_height);
        (rows, cols)
    }

    /// Get an output tile by assembling from source tiles.
    ///
    /// This method calculates which source tiles overlap with the requested
    /// output tile, reads them, and copies the relevant pixels into the output
    /// buffer in band-sequential format.
    ///
    /// # Arguments
    /// * `output_row` - Output tile row index (0-indexed)
    /// * `output_col` - Output tile column index (0-indexed)
    ///
    /// # Returns
    /// A tuple of (data, shape) where:
    /// - `data` is the pixel data in band-sequential format
    /// - `shape` is `[bands, rows, cols]` describing the tile dimensions (CHW format)
    ///
    /// # Errors
    /// Returns `CodecError::InvalidBlockCoordinates` if coordinates are out of bounds.
    /// Returns `CodecError::Decode` if source tile reading fails.
    pub fn get_output_tile(
        &self,
        output_row: u32,
        output_col: u32,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        let (grid_rows, grid_cols) = self.output_grid_size();
        if output_row >= grid_rows || output_col >= grid_cols {
            return Err(CodecError::InvalidBlockCoordinates(output_row, output_col, 0));
        }

        // Calculate pixel region for this output tile
        let start_x = output_col * self.output_tile_width;
        let start_y = output_row * self.output_tile_height;
        let end_x = (start_x + self.output_tile_width).min(self.image_width);
        let end_y = (start_y + self.output_tile_height).min(self.image_height);
        let tile_width = end_x - start_x;
        let tile_height = end_y - start_y;

        // Determine which source tiles we need
        let src_start_col = start_x / self.source_tile_width;
        let src_end_col = (end_x.saturating_sub(1)) / self.source_tile_width + 1;
        let src_start_row = start_y / self.source_tile_height;
        let src_end_row = (end_y.saturating_sub(1)) / self.source_tile_height + 1;

        // Allocate output buffer (BSQ format: all pixels of band 0, then band 1, etc.)
        let tile_pixels = (tile_width as usize) * (tile_height as usize);
        let mut output = vec![0u8; tile_pixels * (self.num_bands as usize) * self.bytes_per_pixel];

        // Read source tiles and copy relevant pixels
        for src_row in src_start_row..src_end_row {
            for src_col in src_start_col..src_end_col {
                let (src_data, src_shape) = self.source.get_block(src_row, src_col, 0, None)?;

                // Copy overlapping region to output
                self.copy_tile_region(
                    &src_data,
                    src_shape,
                    src_row,
                    src_col,
                    &mut output,
                    start_x,
                    start_y,
                    tile_width,
                    tile_height,
                );
            }
        }

        Ok((output, [self.num_bands, tile_height, tile_width]))
    }

    /// Copy overlapping region from source tile to output buffer.
    ///
    /// This method handles the coordinate translation between source tile
    /// coordinates and output tile coordinates, copying only the pixels
    /// that fall within both regions.
    ///
    /// # Arguments
    /// * `src_data` - Source tile data in BSQ format
    /// * `src_shape` - Shape of source tile as [bands, rows, cols] (CHW format)
    /// * `src_row` - Source tile row index
    /// * `src_col` - Source tile column index
    /// * `output` - Output buffer to write to (BSQ format)
    /// * `out_start_x` - X coordinate of output tile's top-left corner in image space
    /// * `out_start_y` - Y coordinate of output tile's top-left corner in image space
    /// * `out_width` - Width of output tile in pixels
    /// * `out_height` - Height of output tile in pixels
    fn copy_tile_region(
        &self,
        src_data: &[u8],
        src_shape: [u32; 3],
        src_row: u32,
        src_col: u32,
        output: &mut [u8],
        out_start_x: u32,
        out_start_y: u32,
        out_width: u32,
        out_height: u32,
    ) {
        // Shape is now [bands, rows, cols] (CHW format)
        let src_rows = src_shape[1];
        let src_cols = src_shape[2];

        // Calculate source tile's position in image coordinates
        let src_start_x = src_col * self.source_tile_width;
        let src_start_y = src_row * self.source_tile_height;

        // Calculate the overlapping region in image coordinates
        let overlap_start_x = out_start_x.max(src_start_x);
        let overlap_start_y = out_start_y.max(src_start_y);
        let overlap_end_x = (out_start_x + out_width).min(src_start_x + src_cols);
        let overlap_end_y = (out_start_y + out_height).min(src_start_y + src_rows);

        // Check if there's actually an overlap
        if overlap_start_x >= overlap_end_x || overlap_start_y >= overlap_end_y {
            return;
        }

        let overlap_width = overlap_end_x - overlap_start_x;
        let overlap_height = overlap_end_y - overlap_start_y;

        // Calculate offsets within source and output tiles
        let src_offset_x = overlap_start_x - src_start_x;
        let src_offset_y = overlap_start_y - src_start_y;
        let out_offset_x = overlap_start_x - out_start_x;
        let out_offset_y = overlap_start_y - out_start_y;

        let bpp = self.bytes_per_pixel;
        let src_pixels_per_band = (src_rows as usize) * (src_cols as usize);
        let out_pixels_per_band = (out_height as usize) * (out_width as usize);

        // Copy pixel data for each band (BSQ format)
        for band in 0..self.num_bands {
            let src_band_offset = (band as usize) * src_pixels_per_band * bpp;
            let out_band_offset = (band as usize) * out_pixels_per_band * bpp;

            for row in 0..overlap_height {
                let src_row_idx = (src_offset_y + row) as usize;
                let out_row_idx = (out_offset_y + row) as usize;

                let src_row_offset =
                    src_band_offset + src_row_idx * (src_cols as usize) * bpp + (src_offset_x as usize) * bpp;
                let out_row_offset =
                    out_band_offset + out_row_idx * (out_width as usize) * bpp + (out_offset_x as usize) * bpp;

                let copy_bytes = (overlap_width as usize) * bpp;

                output[out_row_offset..out_row_offset + copy_bytes]
                    .copy_from_slice(&src_data[src_row_offset..src_row_offset + copy_bytes]);
            }
        }
    }
}



/// Block encoder for uncompressed NITF imagery (IC=NC).
///
/// Accepts blocks in band-sequential format and converts to the target IMODE.
/// This encoder is symmetric to `UncompressedBlockDecoder` and follows the same
/// block organization patterns.
///
/// # Thread Safety
///
/// This encoder uses interior mutability through standard Rust patterns and is
/// `Send + Sync` safe for use across threads.
pub struct UncompressedBlockEncoder {
    /// Target image height in pixels
    nrows: u32,
    /// Target image width in pixels
    ncols: u32,
    /// Number of bands
    nbands: u32,
    /// Bits per pixel
    nbpp: u8,
    /// Target interleave mode
    imode: InterleaveMode,
    /// Block width in pixels
    nppbh: u32,
    /// Block height in pixels
    nppbv: u32,
    /// Number of blocks per row
    nbpr: u32,
    /// Number of blocks per column
    nbpc: u32,
    /// Accumulated encoded data buffer
    encoded_data: Vec<u8>,
    /// Track which blocks have been encoded (row-major: [block_row][block_col])
    blocks_encoded: Vec<Vec<bool>>,
}

impl UncompressedBlockEncoder {
    /// Create a new uncompressed block encoder.
    ///
    /// # Arguments
    /// * `nrows` - Image height in pixels
    /// * `ncols` - Image width in pixels
    /// * `nbands` - Number of bands
    /// * `nbpp` - Bits per pixel
    /// * `imode` - Target interleave mode
    /// * `nppbh` - Block width in pixels
    /// * `nppbv` - Block height in pixels
    pub fn new(
        nrows: u32,
        ncols: u32,
        nbands: u32,
        nbpp: u8,
        imode: InterleaveMode,
        nppbh: u32,
        nppbv: u32,
    ) -> Self {
        // Calculate block grid size using ceiling division
        let nbpr = ncols.div_ceil(nppbh);
        let nbpc = nrows.div_ceil(nppbv);

        // Pre-allocate space for encoded data
        // NITF stores data in blocks with nominal sizes, so we need to allocate
        // based on block grid * nominal block size, not actual image dimensions
        let bytes_per_pixel = (nbpp as usize).div_ceil(8);
        
        let total_size = match imode {
            InterleaveMode::S => {
                // Band sequential: each band has its own set of blocks
                // Total = nbands * (nbpc * nbpr * nppbv * nppbh * bpp)
                (nbands as usize)
                    * (nbpc as usize)
                    * (nbpr as usize)
                    * (nppbv as usize)
                    * (nppbh as usize)
                    * bytes_per_pixel
            }
            InterleaveMode::B | InterleaveMode::P | InterleaveMode::R => {
                // All bands stored together per block
                // Total = nbpc * nbpr * (nppbv * nppbh * nbands * bpp)
                (nbpc as usize)
                    * (nbpr as usize)
                    * (nppbv as usize)
                    * (nppbh as usize)
                    * (nbands as usize)
                    * bytes_per_pixel
            }
        };

        Self {
            nrows,
            ncols,
            nbands,
            nbpp,
            imode,
            nppbh,
            nppbv,
            nbpr,
            nbpc,
            encoded_data: vec![0u8; total_size],
            blocks_encoded: vec![vec![false; nbpr as usize]; nbpc as usize],
        }
    }

    /// Calculate the number of bytes per pixel.
    fn bytes_per_pixel(&self) -> usize {
        (self.nbpp as usize).div_ceil(8)
    }

    /// Calculate the actual dimensions of a block, handling edge blocks.
    ///
    /// Edge blocks may be smaller than the nominal block size if the image
    /// dimensions are not evenly divisible by the block size.
    fn actual_block_dimensions(&self, block_row: u32, block_col: u32) -> (u32, u32) {
        let start_row = block_row * self.nppbv;
        let start_col = block_col * self.nppbh;

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

    /// Convert BSQ block data to target IMODE and write to output buffer.
    ///
    /// # Arguments
    /// * `block_row` - Row index of the block in the block grid
    /// * `block_col` - Column index of the block in the block grid
    /// * `data` - Pixel data in band-sequential format
    /// * `shape` - Shape of the data as [bands, rows, cols] (CHW format)
    fn write_block_to_buffer(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        shape: [u32; 3],
    ) -> Result<(), CodecError> {
        let bpp = self.bytes_per_pixel();
        // Shape is [bands, rows, cols] (CHW format)
        let block_bands = shape[0];
        let block_rows = shape[1];
        let block_cols = shape[2];

        // NITF mandates big-endian for uncompressed multi-byte pixel data
        // (JBP Section 4.6.2, requirement JBP-2021.2-013). Convert from
        // native-endian (internal contract) to big-endian before writing.
        let be_data = swap_ne_to_be(data, bpp);

        // Convert BSQ input to target IMODE
        let converted_data = from_band_sequential(
            &be_data,
            self.imode,
            block_rows,
            block_cols,
            block_bands,
            bpp,
        )?;

        // Calculate where this block's data goes in the output buffer
        match self.imode {
            InterleaveMode::S => {
                // Band sequential: each band's blocks are stored separately
                // Layout: [Band0_all_blocks, Band1_all_blocks, ...]
                self.write_block_mode_s(
                    block_row,
                    block_col,
                    &converted_data,
                    block_rows,
                    block_cols,
                )?;
            }
            InterleaveMode::B | InterleaveMode::P | InterleaveMode::R => {
                // For B, P, R modes, all bands for a block are stored together
                self.write_block_mode_bpr(
                    block_row,
                    block_col,
                    &converted_data,
                    block_rows,
                    block_cols,
                )?;
            }
        }

        Ok(())
    }

    /// Write block data for IMODE S (band sequential).
    ///
    /// In band sequential mode, each band's blocks are stored separately.
    fn write_block_mode_s(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        block_rows: u32,
        block_cols: u32,
    ) -> Result<(), CodecError> {
        let bpp = self.bytes_per_pixel();
        let blocks_per_band = (self.nbpr as usize) * (self.nbpc as usize);
        let single_band_block_size = (self.nppbh as usize) * (self.nppbv as usize) * bpp;
        let block_index = (block_row as usize) * (self.nbpr as usize) + (block_col as usize);

        let actual_pixels_per_band = (block_rows as usize) * (block_cols as usize);

        for band in 0..self.nbands {
            let band_offset = (band as usize) * blocks_per_band * single_band_block_size;
            let block_offset = band_offset + block_index * single_band_block_size;

            // Source offset in the converted data (which is in BSQ format for mode S)
            let src_band_offset = (band as usize) * actual_pixels_per_band * bpp;

            // Write each row of this band's block data
            for row in 0..block_rows {
                let src_row_offset = src_band_offset + (row as usize) * (block_cols as usize) * bpp;
                let dst_row_offset = block_offset + (row as usize) * (self.nppbh as usize) * bpp;
                let row_bytes = (block_cols as usize) * bpp;

                self.encoded_data[dst_row_offset..dst_row_offset + row_bytes]
                    .copy_from_slice(&data[src_row_offset..src_row_offset + row_bytes]);
            }
        }

        Ok(())
    }

    /// Write block data for IMODE B, P, or R.
    ///
    /// In these modes, all bands for a block are stored together.
    fn write_block_mode_bpr(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        block_rows: u32,
        block_cols: u32,
    ) -> Result<(), CodecError> {
        let bpp = self.bytes_per_pixel();
        let block_size = (self.nppbh as usize) * (self.nppbv as usize) * (self.nbands as usize) * bpp;
        let block_index = (block_row as usize) * (self.nbpr as usize) + (block_col as usize);
        let block_offset = block_index * block_size;

        match self.imode {
            InterleaveMode::B => {
                // Band interleaved by block: all pixels of band 0, then band 1, etc.
                let pixels_per_band = (self.nppbh as usize) * (self.nppbv as usize);
                let actual_pixels_per_band = (block_rows as usize) * (block_cols as usize);

                for band in 0..self.nbands {
                    let dst_band_offset = block_offset + (band as usize) * pixels_per_band * bpp;
                    let src_band_offset = (band as usize) * actual_pixels_per_band * bpp;

                    for row in 0..block_rows {
                        let src_row_offset =
                            src_band_offset + (row as usize) * (block_cols as usize) * bpp;
                        let dst_row_offset =
                            dst_band_offset + (row as usize) * (self.nppbh as usize) * bpp;
                        let row_bytes = (block_cols as usize) * bpp;

                        self.encoded_data[dst_row_offset..dst_row_offset + row_bytes]
                            .copy_from_slice(&data[src_row_offset..src_row_offset + row_bytes]);
                    }
                }
            }
            InterleaveMode::P => {
                // Band interleaved by pixel: R0G0B0, R1G1B1, ...
                let pixel_size = (self.nbands as usize) * bpp;
                let nominal_row_size = (self.nppbh as usize) * pixel_size;

                for row in 0..block_rows {
                    let dst_row_offset = block_offset + (row as usize) * nominal_row_size;
                    let src_row_offset = (row as usize) * (block_cols as usize) * pixel_size;
                    let row_bytes = (block_cols as usize) * pixel_size;

                    self.encoded_data[dst_row_offset..dst_row_offset + row_bytes]
                        .copy_from_slice(&data[src_row_offset..src_row_offset + row_bytes]);
                }
            }
            InterleaveMode::R => {
                // Band interleaved by row: Row0_B0, Row0_B1, Row0_B2, Row1_B0, ...
                let row_size = (self.nppbh as usize) * bpp;
                let nominal_row_group_size = row_size * (self.nbands as usize);

                for row in 0..block_rows {
                    for band in 0..self.nbands {
                        let dst_offset = block_offset
                            + (row as usize) * nominal_row_group_size
                            + (band as usize) * row_size;
                        let src_offset = ((row as usize) * (self.nbands as usize)
                            + (band as usize))
                            * (block_cols as usize)
                            * bpp;
                        let row_bytes = (block_cols as usize) * bpp;

                        self.encoded_data[dst_offset..dst_offset + row_bytes]
                            .copy_from_slice(&data[src_offset..src_offset + row_bytes]);
                    }
                }
            }
            InterleaveMode::S => unreachable!("IMODE S handled separately"),
        }

        Ok(())
    }
}

// Implement Send + Sync for thread safety
// These are automatically derived since all fields are Send + Sync
unsafe impl Send for UncompressedBlockEncoder {}
unsafe impl Sync for UncompressedBlockEncoder {}

impl BlockEncoder for UncompressedBlockEncoder {
    fn encode_block(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        shape: [u32; 3],
    ) -> Result<(), CodecError> {
        // Validate block coordinates
        if block_row >= self.nbpc || block_col >= self.nbpr {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
        }

        // Calculate expected block dimensions
        let (expected_rows, expected_cols) = self.actual_block_dimensions(block_row, block_col);

        // Validate shape matches expected dimensions - shape is [bands, rows, cols] (CHW format)
        if shape[0] != self.nbands || shape[1] != expected_rows || shape[2] != expected_cols {
            return Err(CodecError::Encode(format!(
                "Block shape mismatch: expected [{}, {}, {}], got [{}, {}, {}]",
                self.nbands, expected_rows, expected_cols, shape[0], shape[1], shape[2]
            )));
        }

        // Validate data size matches shape
        let bpp = self.bytes_per_pixel();
        let expected_size =
            (shape[0] as usize) * (shape[1] as usize) * (shape[2] as usize) * bpp;
        if data.len() != expected_size {
            return Err(CodecError::Encode(format!(
                "Block data size mismatch: expected {} bytes, got {}",
                expected_size,
                data.len()
            )));
        }

        // Convert and write to buffer
        self.write_block_to_buffer(block_row, block_col, data, shape)?;
        self.blocks_encoded[block_row as usize][block_col as usize] = true;

        Ok(())
    }

    fn skip_block(&mut self, block_row: u32, block_col: u32) -> Result<(), CodecError> {
        // Validate block coordinates
        if block_row >= self.nbpc || block_col >= self.nbpr {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
        }

        // Mark block as handled (skipped for masked images)
        self.blocks_encoded[block_row as usize][block_col as usize] = true;

        Ok(())
    }

    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError> {
        // Check all blocks have been encoded
        for (row, row_blocks) in self.blocks_encoded.iter().enumerate() {
            for (col, &encoded) in row_blocks.iter().enumerate() {
                if !encoded {
                    return Err(CodecError::Encode(format!(
                        "Incomplete encoding: block ({}, {}) not encoded. Grid size: ({}, {})",
                        row, col, self.nbpc, self.nbpr
                    )));
                }
            }
        }

        Ok(self.encoded_data)
    }

    fn compression_type(&self) -> &str {
        "NC"
    }

    fn block_grid_size(&self) -> (u32, u32) {
        (self.nbpc, self.nbpr)
    }

    fn block_dimensions(&self) -> (u32, u32) {
        (self.nppbv, self.nppbh)
    }
}

// =============================================================================
// JpegNitfBlockEncoder - JPEG DCT encoder for NITF (IC=C3, M3, I1)
// =============================================================================

/// Block encoder for JPEG DCT compressed NITF imagery (IC=C3, M3, I1).
///
/// This encoder wraps the [`JpegBlockEncoder`] and implements the [`BlockEncoder`]
/// trait for integration with the JBP image writer infrastructure.
///
/// # Supported IC Codes
/// - `C3`: JPEG DCT compressed imagery
/// - `M3`: JPEG DCT compressed imagery with block mask
/// - `I1`: Downsampled JPEG (single block ≤2048×2048)
///
/// # Requirements
/// - 2.1: Encode JPEG DCT compressed blocks (IC=C3)
/// - 2.2: Encode 8-bit monochrome JPEG blocks
/// - 2.3: Return error for 12-bit JPEG (not supported)
/// - 2.4: Encode 3-band RGB images (IMODE=P)
/// - 2.5: Convert RGB to YCbCr601 before compression
/// - 2.6: Encode multiband images (IMODE=B or S)
#[cfg(feature = "libjpeg-turbo")]
pub struct JpegNitfBlockEncoder {
    /// The underlying JPEG encoder
    jpeg_encoder: JpegBlockEncoder,
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
    /// Bits per pixel
    nbpp: u8,
    /// Interleave mode
    imode: InterleaveMode,
    /// Compression type (C3, M3, or I1)
    ic: String,
    /// Accumulated encoded data buffer
    encoded_data: Vec<u8>,
    /// Track which blocks have been encoded
    encoded_blocks: HashSet<(u32, u32)>,
    /// Bytes per pixel
    bytes_per_pixel: usize,
}

#[cfg(feature = "libjpeg-turbo")]
impl JpegNitfBlockEncoder {
    /// Create a new JPEG NITF block encoder.
    ///
    /// # Arguments
    /// * `ic` - Image compression code (C3, M3, or I1)
    /// * `nrows` - Image height in pixels
    /// * `ncols` - Image width in pixels
    /// * `nbands` - Number of bands
    /// * `nbpp` - Bits per pixel (8 or 12)
    /// * `imode` - Interleave mode
    /// * `nppbh` - Block width in pixels
    /// * `nppbv` - Block height in pixels
    /// * `comrat` - Optional COMRAT string for quality
    ///
    /// # Returns
    /// A new `JpegNitfBlockEncoder` or an error if parameters are invalid.
    ///
    /// # Requirements
    /// - 2.1: JPEG DCT encoding support
    /// - 4.4: I1 dimension constraint validation (≤2048×2048)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ic: &str,
        nrows: u32,
        ncols: u32,
        nbands: u32,
        nbpp: u8,
        imode: InterleaveMode,
        nppbh: u32,
        nppbv: u32,
        comrat: Option<&str>,
    ) -> Result<Self, CodecError> {
        let ic_trimmed = ic.trim().to_string();

        // Validate I1 dimension constraint (Requirement 4.4)
        if ic_trimmed == "I1" && (nrows > 2048 || ncols > 2048) {
            return Err(CodecError::InvalidFormat(format!(
                "IC=I1 (Downsampled JPEG) requires dimensions ≤2048×2048, got {}×{}",
                ncols, nrows
            )));
        }

        // Parse COMRAT to get quality
        let quality = match comrat {
            Some(c) => JpegComrat::parse(c)?.quality(),
            None => JpegComrat::default().quality(),
        };

        // Determine color space from band count and IMODE
        let color_space = if nbands == 1 {
            JpegColorSpace::Grayscale
        } else if nbands == 3 && imode == InterleaveMode::P {
            // 3-band pixel-interleaved is typically RGB or YCbCr
            // Default to RGB, let the encoder handle YCbCr conversion
            JpegColorSpace::Rgb
        } else {
            // Multiband - encode each band as grayscale
            JpegColorSpace::Grayscale
        };

        // Create the underlying JPEG encoder
        let jpeg_encoder = JpegBlockEncoder::new(
            nbpp,
            nbands as usize,
            nppbh as usize,
            nppbv as usize,
            imode,
            color_space,
            quality,
        )?;

        // Calculate block grid
        let nbpr = ncols.div_ceil(nppbh);
        let nbpc = nrows.div_ceil(nppbv);

        // Calculate bytes per pixel
        let bytes_per_pixel = (nbpp as usize).div_ceil(8);

        Ok(Self {
            jpeg_encoder,
            nrows,
            ncols,
            nbpr,
            nbpc,
            nppbh,
            nppbv,
            nbands,
            nbpp,
            imode,
            ic: ic_trimmed,
            encoded_data: Vec::new(),
            encoded_blocks: HashSet::new(),
            bytes_per_pixel,
        })
    }

    /// Calculate the actual dimensions of a block, handling edge blocks.
    fn actual_block_dimensions(&self, block_row: u32, block_col: u32) -> (u32, u32) {
        let start_row = block_row * self.nppbv;
        let start_col = block_col * self.nppbh;

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
}

#[cfg(feature = "libjpeg-turbo")]
impl BlockEncoder for JpegNitfBlockEncoder {
    fn encode_block(
        &mut self,
        block_row: u32,
        block_col: u32,
        data: &[u8],
        shape: [u32; 3],
    ) -> Result<(), CodecError> {
        // Validate block coordinates
        if block_row >= self.nbpc || block_col >= self.nbpr {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                0,
            ));
        }

        // Validate data size matches shape
        let expected_size =
            (shape[0] as usize) * (shape[1] as usize) * (shape[2] as usize) * self.bytes_per_pixel;
        if data.len() != expected_size {
            return Err(CodecError::Encode(format!(
                "Data size {} doesn't match shape {:?} with {} bytes/pixel (expected {})",
                data.len(),
                shape,
                self.bytes_per_pixel,
                expected_size
            )));
        }

        // Get actual block dimensions for edge blocks
        let (actual_rows, actual_cols) = self.actual_block_dimensions(block_row, block_col);

        // For edge blocks, we may need to handle partial data
        // The input data should already be sized for the actual block dimensions
        if shape[1] != actual_rows || shape[2] != actual_cols {
            return Err(CodecError::Encode(format!(
                "Block shape {:?} doesn't match expected dimensions ({}, {})",
                shape, actual_rows, actual_cols
            )));
        }

        // Encode the block using the JPEG encoder
        let jpeg_data = if self.nbands > 1 && self.imode != InterleaveMode::P {
            // Multiband with IMODE=B or S - encode each band separately
            self.jpeg_encoder.encode_multiband_block(data)?
        } else {
            // Single band or pixel-interleaved RGB
            self.jpeg_encoder.encode_block(data)?
        };

        // Append the JPEG data to the output buffer
        // For NITF JPEG, each block is stored sequentially
        self.encoded_data.extend_from_slice(&jpeg_data);

        // Track encoded block
        self.encoded_blocks.insert((block_row, block_col));

        Ok(())
    }

    fn skip_block(&mut self, block_row: u32, block_col: u32) -> Result<(), CodecError> {
        // Validate block coordinates
        if block_row >= self.nbpc || block_col >= self.nbpr {
            return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
        }

        // Mark block as handled (skipped for masked images like M3)
        self.encoded_blocks.insert((block_row, block_col));

        Ok(())
    }

    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError> {
        // Verify all blocks were encoded
        let expected_blocks = self.nbpc * self.nbpr;
        if self.encoded_blocks.len() != expected_blocks as usize {
            return Err(CodecError::Encode(format!(
                "Incomplete encoding: {} of {} blocks encoded",
                self.encoded_blocks.len(),
                expected_blocks
            )));
        }

        Ok(self.encoded_data)
    }

    fn compression_type(&self) -> &str {
        &self.ic
    }

    fn block_grid_size(&self) -> (u32, u32) {
        (self.nbpc, self.nbpr)
    }

    fn block_dimensions(&self) -> (u32, u32) {
        (self.nppbv, self.nppbh)
    }
}

// Safety: JpegNitfBlockEncoder is thread-safe
// - jpeg_encoder contains only primitive types and JpegCodec (which is Send+Sync)
// - All other fields are primitive types or standard collections
#[cfg(feature = "libjpeg-turbo")]
unsafe impl Send for JpegNitfBlockEncoder {}

#[cfg(feature = "libjpeg-turbo")]
unsafe impl Sync for JpegNitfBlockEncoder {}

#[cfg(test)]
mod tests {
    use super::*;

    mod block_encoder_trait {
        use super::*;

        /// Test that BlockEncoder trait is object-safe by creating a trait object
        #[test]
        fn trait_is_object_safe() {
            // This test verifies that BlockEncoder can be used as a trait object
            // If this compiles, the trait is object-safe
            fn _accepts_trait_object(_encoder: Box<dyn BlockEncoder>) {}
        }

        /// Test that BlockEncoder requires Send + Sync bounds
        #[test]
        fn trait_requires_send_sync() {
            // This test verifies that BlockEncoder implementations must be Send + Sync
            // If this compiles, the bounds are correctly specified
            fn _assert_send_sync<T: Send + Sync>() {}
            fn _check_bounds<T: BlockEncoder>() {
                _assert_send_sync::<T>();
            }
        }
    }

    mod create_block_encoder_tests {
        use super::*;

        #[test]
        #[cfg(feature = "openjpeg")]
        fn c8_returns_encoder() {
            let result = create_block_encoder(
                "C8",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "C8");
        }

        #[test]
        #[cfg(not(feature = "openjpeg"))]
        fn c8_without_feature_returns_error() {
            let result = create_block_encoder(
                "C8",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("openjpeg"));
                }
                _ => panic!("Expected Unsupported error"),
            }
        }

        #[test]
        fn nc_returns_encoder() {
            #[cfg(feature = "openjpeg")]
            let result = create_block_encoder(
                "NC",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            #[cfg(not(feature = "openjpeg"))]
            let result = create_block_encoder(
                "NC",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "NC");
            assert_eq!(encoder.block_grid_size(), (2, 2));
            assert_eq!(encoder.block_dimensions(), (32, 32));
        }

        #[test]
        #[cfg(feature = "openjpeg")]
        fn cd_returns_encoder() {
            let result = create_block_encoder(
                "CD",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            // CD (HTJ2K) may fail if codec doesn't support it
            // OpenJPEG doesn't support HTJ2K, so this should fail
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("HTJ2K"));
                }
                _ => panic!("Expected Unsupported error for HTJ2K"),
            }
        }

        #[test]
        fn unsupported_ic_returns_error() {
            #[cfg(feature = "openjpeg")]
            let result = create_block_encoder(
                "XX",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            #[cfg(not(feature = "openjpeg"))]
            let result = create_block_encoder(
                "XX",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("XX"));
                }
                _ => panic!("Expected Unsupported error"),
            }
        }

        #[test]
        fn nc_with_whitespace_is_trimmed() {
            #[cfg(feature = "openjpeg")]
            let result = create_block_encoder(
                "  NC  ",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            #[cfg(not(feature = "openjpeg"))]
            let result = create_block_encoder(
                "  NC  ",
                64,
                64,
                3,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_ok());
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn c3_returns_encoder() {
            let result = create_block_encoder(
                "C3",
                64,
                64,
                1,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                Some("75.0"),
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "C3");
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn m3_returns_encoder() {
            let result = create_block_encoder(
                "M3",
                64,
                64,
                1,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None, // Default quality
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "M3");
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn i1_returns_encoder() {
            let result = create_block_encoder(
                "I1",
                1024,
                1024,
                1,
                8,
                false,
                InterleaveMode::B,
                1024,
                1024,
                None,
                None,
            );
            assert!(result.is_ok());
            let encoder = result.unwrap();
            assert_eq!(encoder.compression_type(), "I1");
        }

        #[test]
        #[cfg(feature = "libjpeg-turbo")]
        fn i1_dimension_constraint_error() {
            // I1 requires dimensions ≤2048×2048
            let result = create_block_encoder(
                "I1",
                4096,
                4096,
                1,
                8,
                false,
                InterleaveMode::B,
                4096,
                4096,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidFormat(msg)) => {
                    assert!(msg.contains("2048"));
                }
                _ => panic!("Expected InvalidFormat error for I1 dimension constraint"),
            }
        }

        #[test]
        #[cfg(not(feature = "libjpeg-turbo"))]
        fn c3_without_feature_returns_error() {
            let result = create_block_encoder(
                "C3",
                64,
                64,
                1,
                8,
                false,
                InterleaveMode::B,
                32,
                32,
                None,
                None,
            );
            assert!(result.is_err());
            match result {
                Err(CodecError::Unsupported(msg)) => {
                    assert!(msg.contains("libjpeg-turbo"));
                }
                _ => panic!("Expected Unsupported error"),
            }
        }
    }

    mod uncompressed_block_encoder_tests {
        use super::*;

        #[test]
        fn new_calculates_grid_size_correctly() {
            // 64x64 image with 32x32 blocks = 2x2 grid
            let encoder = UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 32);
            assert_eq!(encoder.block_grid_size(), (2, 2));

            // 65x65 image with 32x32 blocks = 3x3 grid (ceiling division)
            let encoder = UncompressedBlockEncoder::new(65, 65, 3, 8, InterleaveMode::B, 32, 32);
            assert_eq!(encoder.block_grid_size(), (3, 3));

            // 100x50 image with 32x32 blocks = 2x4 grid
            let encoder = UncompressedBlockEncoder::new(100, 50, 3, 8, InterleaveMode::B, 32, 32);
            assert_eq!(encoder.block_grid_size(), (4, 2)); // (nbpc, nbpr)
        }

        #[test]
        fn encode_block_validates_coordinates() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Valid coordinates - shape is [bands, rows, cols] (CHW format)
            assert!(encoder.encode_block(0, 0, &data, [1, 32, 32]).is_ok());

            // Invalid row
            let result = encoder.encode_block(2, 0, &data, [1, 32, 32]);
            assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(2, 0, 0))));

            // Invalid column
            let result = encoder.encode_block(0, 2, &data, [1, 32, 32]);
            assert!(matches!(result, Err(CodecError::InvalidBlockCoordinates(0, 2, 0))));
        }

        #[test]
        fn encode_block_validates_data_size() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);

            // Wrong data size
            let data = vec![0u8; 100]; // Should be 32*32 = 1024
            // Shape is [bands, rows, cols] (CHW format)
            let result = encoder.encode_block(0, 0, &data, [1, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    assert!(msg.contains("size mismatch"));
                }
                _ => panic!("Expected Encode error"),
            }
        }

        #[test]
        fn encode_block_validates_shape() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32 * 3];

            // Wrong number of bands in shape - shape is [bands, rows, cols] (CHW format)
            let result = encoder.encode_block(0, 0, &data, [2, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    assert!(msg.contains("shape mismatch"));
                }
                _ => panic!("Expected Encode error"),
            }
        }

        #[test]
        fn finalize_fails_if_blocks_missing() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Only encode one block - shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 32, 32]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    assert!(msg.contains("Incomplete"));
                    assert!(msg.contains("(0, 1)")); // First missing block
                }
                _ => panic!("Expected Encode error"),
            }
        }

        #[test]
        fn finalize_succeeds_when_all_blocks_encoded() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Encode all 4 blocks - shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(0, 1, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(1, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(1, 1, &data, [1, 32, 32]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_ok());
            let encoded = result.unwrap();
            assert_eq!(encoded.len(), 64 * 64); // 64x64 pixels, 1 band, 1 byte/pixel
        }

        #[test]
        fn compression_type_returns_nc() {
            let encoder = UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 32);
            assert_eq!(encoder.compression_type(), "NC");
        }

        #[test]
        fn block_dimensions_returns_correct_values() {
            let encoder = UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 48);
            assert_eq!(encoder.block_dimensions(), (48, 32)); // (height, width)
        }

        #[test]
        fn edge_block_dimensions_calculated_correctly() {
            // 65x65 image with 32x32 blocks
            let encoder = UncompressedBlockEncoder::new(65, 65, 1, 8, InterleaveMode::B, 32, 32);

            // Full block
            let (rows, cols) = encoder.actual_block_dimensions(0, 0);
            assert_eq!((rows, cols), (32, 32));

            // Edge block (right)
            let (rows, cols) = encoder.actual_block_dimensions(0, 2);
            assert_eq!((rows, cols), (32, 1)); // Only 1 column left

            // Edge block (bottom)
            let (rows, cols) = encoder.actual_block_dimensions(2, 0);
            assert_eq!((rows, cols), (1, 32)); // Only 1 row left

            // Corner block
            let (rows, cols) = encoder.actual_block_dimensions(2, 2);
            assert_eq!((rows, cols), (1, 1)); // 1x1 corner
        }

        #[test]
        fn encode_edge_blocks() {
            // 6x6 image with 4x4 blocks = 2x2 grid
            // NITF stores data in nominal block sizes, so total size is 2*2*4*4 = 64 bytes
            let mut encoder = UncompressedBlockEncoder::new(6, 6, 1, 8, InterleaveMode::B, 4, 4);

            // Full block (0,0): 4x4 - shape is [bands, rows, cols] (CHW format)
            let data_full = vec![1u8; 4 * 4];
            encoder.encode_block(0, 0, &data_full, [1, 4, 4]).unwrap();

            // Edge block (0,1): 4x2
            let data_edge_col = vec![2u8; 4 * 2];
            encoder.encode_block(0, 1, &data_edge_col, [1, 4, 2]).unwrap();

            // Edge block (1,0): 2x4
            let data_edge_row = vec![3u8; 2 * 4];
            encoder.encode_block(1, 0, &data_edge_row, [1, 2, 4]).unwrap();

            // Corner block (1,1): 2x2
            let data_corner = vec![4u8; 2 * 2];
            encoder.encode_block(1, 1, &data_corner, [1, 2, 2]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_ok());
            let encoded = result.unwrap();
            // NITF stores data in nominal block sizes: 2x2 blocks * 4x4 pixels = 64 bytes
            assert_eq!(encoded.len(), 2 * 2 * 4 * 4);
        }

        #[test]
        fn encode_16bit_pixels() {
            let mut encoder = UncompressedBlockEncoder::new(4, 4, 1, 16, InterleaveMode::B, 4, 4);
            let data = vec![0u8; 4 * 4 * 2]; // 16 pixels * 2 bytes

            // Shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 4, 4]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_ok());
            let encoded = result.unwrap();
            assert_eq!(encoded.len(), 4 * 4 * 2); // 16 pixels * 2 bytes
        }
    }

    /// Error handling tests for BlockEncoder
    /// Validates Requirements 8.1, 8.2, 8.4
    mod error_handling_tests {
        use super::*;

        /// Test that invalid block coordinates error includes row, col coordinates
        /// Validates: Requirement 8.1
        #[test]
        fn invalid_coordinates_error_includes_row_and_col() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Test row out of bounds (grid is 2x2, so row 5 is invalid)
            // Shape is [bands, rows, cols] (CHW format)
            let result = encoder.encode_block(5, 0, &data, [1, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidBlockCoordinates(row, col, _)) => {
                    assert_eq!(row, 5, "Error should include the invalid row coordinate");
                    assert_eq!(col, 0, "Error should include the column coordinate");
                }
                _ => panic!("Expected InvalidBlockCoordinates error"),
            }

            // Test column out of bounds
            let result = encoder.encode_block(0, 10, &data, [1, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidBlockCoordinates(row, col, _)) => {
                    assert_eq!(row, 0, "Error should include the row coordinate");
                    assert_eq!(col, 10, "Error should include the invalid column coordinate");
                }
                _ => panic!("Expected InvalidBlockCoordinates error"),
            }

            // Test both row and column out of bounds
            let result = encoder.encode_block(100, 200, &data, [1, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidBlockCoordinates(row, col, _)) => {
                    assert_eq!(row, 100, "Error should include the invalid row coordinate");
                    assert_eq!(col, 200, "Error should include the invalid column coordinate");
                }
                _ => panic!("Expected InvalidBlockCoordinates error"),
            }
        }

        /// Test that invalid coordinates error is returned for boundary cases
        /// Validates: Requirement 8.1
        #[test]
        fn invalid_coordinates_at_grid_boundary() {
            // 65x65 image with 32x32 blocks = 3x3 grid (indices 0, 1, 2 valid)
            let mut encoder = UncompressedBlockEncoder::new(65, 65, 1, 8, InterleaveMode::B, 32, 32);
            let (grid_rows, grid_cols) = encoder.block_grid_size();
            assert_eq!((grid_rows, grid_cols), (3, 3));

            let data = vec![0u8; 32 * 32];

            // Index 0 should be valid with full block size
            // Shape is [bands, rows, cols] (CHW format)
            assert!(encoder.encode_block(0, 0, &data, [1, 32, 32]).is_ok());

            // Index 3 should be invalid (one past the boundary)
            let result = encoder.encode_block(3, 0, &data, [1, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidBlockCoordinates(row, col, _)) => {
                    assert_eq!(row, 3);
                    assert_eq!(col, 0);
                }
                _ => panic!("Expected InvalidBlockCoordinates error"),
            }
        }

        /// Test that data size mismatch error includes expected and actual sizes
        /// Validates: Requirement 8.2
        #[test]
        fn data_size_error_includes_expected_and_actual() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);

            // Expected size: 32 * 32 * 1 band * 1 byte = 1024 bytes
            let wrong_data = vec![0u8; 100]; // Too small
            // Shape is [bands, rows, cols] (CHW format)
            let result = encoder.encode_block(0, 0, &wrong_data, [1, 32, 32]);

            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    assert!(
                        msg.contains("1024"),
                        "Error message should include expected size (1024): {}",
                        msg
                    );
                    assert!(
                        msg.contains("100"),
                        "Error message should include actual size (100): {}",
                        msg
                    );
                }
                _ => panic!("Expected Encode error with size information"),
            }
        }

        /// Test data size error with multi-band images
        /// Validates: Requirement 8.2
        #[test]
        fn data_size_error_multiband() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 32);

            // Expected size: 32 * 32 * 3 bands * 1 byte = 3072 bytes
            let wrong_data = vec![0u8; 1024]; // Only enough for 1 band
            // Shape is [bands, rows, cols] (CHW format)
            let result = encoder.encode_block(0, 0, &wrong_data, [3, 32, 32]);

            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    assert!(
                        msg.contains("3072"),
                        "Error message should include expected size (3072): {}",
                        msg
                    );
                    assert!(
                        msg.contains("1024"),
                        "Error message should include actual size (1024): {}",
                        msg
                    );
                }
                _ => panic!("Expected Encode error with size information"),
            }
        }

        /// Test data size error with 16-bit pixels
        /// Validates: Requirement 8.2
        #[test]
        fn data_size_error_16bit_pixels() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 16, InterleaveMode::B, 32, 32);

            // Expected size: 32 * 32 * 1 band * 2 bytes = 2048 bytes
            let wrong_data = vec![0u8; 1024]; // Only half the required size
            // Shape is [bands, rows, cols] (CHW format)
            let result = encoder.encode_block(0, 0, &wrong_data, [1, 32, 32]);

            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    assert!(
                        msg.contains("2048"),
                        "Error message should include expected size (2048): {}",
                        msg
                    );
                    assert!(
                        msg.contains("1024"),
                        "Error message should include actual size (1024): {}",
                        msg
                    );
                }
                _ => panic!("Expected Encode error with size information"),
            }
        }

        /// Test that incomplete encoding error indicates missing blocks
        /// Validates: Requirement 8.4
        #[test]
        fn incomplete_encoding_error_indicates_missing_blocks() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Only encode block (0, 0), leaving (0, 1), (1, 0), (1, 1) missing
            // Shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 32, 32]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    // Error should indicate incomplete encoding
                    assert!(
                        msg.contains("Incomplete") || msg.contains("incomplete"),
                        "Error should indicate incomplete encoding: {}",
                        msg
                    );
                    // Error should mention at least one missing block coordinate
                    assert!(
                        msg.contains("(0, 1)") || msg.contains("(1, 0)") || msg.contains("(1, 1)"),
                        "Error should indicate missing block coordinates: {}",
                        msg
                    );
                }
                _ => panic!("Expected Encode error indicating missing blocks"),
            }
        }

        /// Test incomplete encoding with only some blocks encoded
        /// Validates: Requirement 8.4
        #[test]
        fn incomplete_encoding_partial_blocks() {
            let mut encoder = UncompressedBlockEncoder::new(96, 96, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Grid is 3x3, encode only first row
            // Shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(0, 1, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(0, 2, &data, [1, 32, 32]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    // Should indicate row 1 blocks are missing
                    assert!(
                        msg.contains("(1,") || msg.contains("Incomplete"),
                        "Error should indicate missing blocks in row 1: {}",
                        msg
                    );
                }
                _ => panic!("Expected Encode error indicating missing blocks"),
            }
        }

        /// Test that finalize succeeds when all blocks are encoded
        /// Validates: Requirement 8.4 (inverse case)
        #[test]
        fn finalize_succeeds_all_blocks_encoded() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Encode all 4 blocks in 2x2 grid
            // Shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(0, 1, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(1, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(1, 1, &data, [1, 32, 32]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(result.is_ok(), "finalize should succeed when all blocks are encoded");
        }

        /// Test incomplete encoding error includes grid size information
        /// Validates: Requirement 8.4
        #[test]
        fn incomplete_encoding_error_includes_grid_size() {
            let mut encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            // Don't encode any blocks

            let result = Box::new(encoder).finalize();
            assert!(result.is_err());
            match result {
                Err(CodecError::Encode(msg)) => {
                    // Error should include grid size (2, 2)
                    assert!(
                        msg.contains("(2, 2)") || (msg.contains("2") && msg.contains("Grid")),
                        "Error should include grid size information: {}",
                        msg
                    );
                }
                _ => panic!("Expected Encode error with grid size information"),
            }
        }
    }
}

/// Property-based tests for block encoder
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::jbp::image::decoder::{create_block_decoder, BlockDecoder};
    use crate::jbp::image::facade::ImageSubheaderFacade;
    use crate::traits::ImageAssetProvider;
    use proptest::prelude::*;
    use std::sync::Arc;

    /// Convert big-endian bytes to native-endian (mirrors decoder's swap_be_to_ne)
    fn swap_be_to_ne(data: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
        if cfg!(target_endian = "big") || bytes_per_pixel <= 1 {
            return data.to_vec();
        }
        match bytes_per_pixel {
            2 => data
                .chunks_exact(2)
                .flat_map(|c| u16::from_be_bytes([c[0], c[1]]).to_ne_bytes())
                .collect(),
            4 => data
                .chunks_exact(4)
                .flat_map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]).to_ne_bytes())
                .collect(),
            _ => data.to_vec(),
        }
    }

    /// Generate a valid InterleaveMode
    fn interleave_mode_strategy() -> impl Strategy<Value = InterleaveMode> {
        prop_oneof![
            Just(InterleaveMode::B),
            Just(InterleaveMode::P),
            Just(InterleaveMode::R),
            Just(InterleaveMode::S),
        ]
    }

    /// Generate valid image dimensions for testing (small for speed)
    fn image_dimensions_strategy() -> impl Strategy<Value = (u32, u32, u32, u8)> {
        (
            1u32..=32,  // nrows
            1u32..=32,  // ncols
            1u32..=4,   // nbands
            prop_oneof![Just(8u8), Just(16u8)], // nbpp
        )
    }

    /// Property 3: IMODE Conversion Preserves Pixels
    /// For any valid image data and target IMODE, encoding with BlockEncoder
    /// and then decoding with BlockDecoder SHALL produce byte-identical pixel data.
    /// **Validates: Requirements 2.2, 2.4, 6.4, 7.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn imode_conversion_preserves_pixels(
            (nrows, ncols, nbands, nbpp) in image_dimensions_strategy(),
            imode in interleave_mode_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property 3: IMODE Conversion Preserves Pixels
            let bpp = ((nbpp as usize) + 7) / 8;
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let data_size = total_pixels * bpp;

            // Generate random image data in BSQ format
            let original_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

            // Use single block for simplicity (block size = image size)
            let mut encoder = UncompressedBlockEncoder::new(
                nrows, ncols, nbands, nbpp, imode, ncols, nrows
            );

            // Encode the single block - shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &original_data, [nbands, nrows, ncols]).unwrap();

            // Finalize to get encoded data
            let encoded_data = Box::new(encoder).finalize().unwrap();

            // Now decode using the decoder
            // We need to create a mock subheader facade or use the decoder directly
            // For this test, we'll verify the round-trip by creating a decoder
            // with matching parameters

            // Create decoder with same parameters
            let decoder = create_test_decoder(
                nrows, ncols, 1, 1, ncols, nrows, nbands, nbpp, imode, encoded_data
            );

            // Decode the block
            let (decoded_data, shape) = decoder.decode_block(0, 0, 0, None).unwrap();

            // Verify shape matches - shape is [bands, rows, cols] (CHW format)
            prop_assert_eq!(shape, [nbands, nrows, ncols]);

            // Verify pixel data matches original
            prop_assert_eq!(
                decoded_data, original_data,
                "IMODE {:?} conversion should preserve pixels for {}x{}x{} image with {} bpp",
                imode, nrows, ncols, nbands, nbpp
            );
        }
    }

    /// Helper to create a test decoder without needing ImageSubheaderFacade
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
    ) -> TestUncompressedBlockDecoder {
        TestUncompressedBlockDecoder {
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
            pvtype: crate::jbp::image::types::PixelValueType::UnsignedInt,
            pjust: crate::jbp::image::types::PixelJustification::Right,
            imode,
            ic: "NC".to_string(),
        }
    }

    /// Test decoder struct for property tests (mirrors UncompressedBlockDecoder)
    struct TestUncompressedBlockDecoder {
        image_data: Arc<[u8]>,
        nrows: u32,
        ncols: u32,
        nbpr: u32,
        nbpc: u32,
        nppbh: u32,
        nppbv: u32,
        nbands: u32,
        nbpp: u8,
        abpp: u8,
        pvtype: crate::jbp::image::types::PixelValueType,
        pjust: crate::jbp::image::types::PixelJustification,
        imode: InterleaveMode,
        ic: String,
    }

    impl TestUncompressedBlockDecoder {
        fn bytes_per_pixel(&self) -> usize {
            ((self.nbpp as usize) + 7) / 8
        }

        fn actual_block_dimensions(&self, block_row: u32, block_col: u32) -> (u32, u32) {
            let start_row = block_row * self.nppbv;
            let start_col = block_col * self.nppbh;

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

        fn decode_block(
            &self,
            block_row: u32,
            block_col: u32,
            resolution_level: u32,
            _bands: Option<&[u32]>,
        ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
            // Test decoder only supports resolution level 0
            if resolution_level != 0 {
                return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, resolution_level));
            }
            
            if block_row >= self.nbpc || block_col >= self.nbpr {
                return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
            }

            let (actual_rows, actual_cols) = self.actual_block_dimensions(block_row, block_col);
            let bpp = self.bytes_per_pixel();

            // Read raw block data based on IMODE
            let raw_data = match self.imode {
                InterleaveMode::S => {
                    self.read_block_mode_s(block_row, block_col, actual_rows, actual_cols)?
                }
                _ => self.read_block_mode_bpr(block_row, block_col, actual_rows, actual_cols)?,
            };

            // Convert to band-sequential format if needed
            let bsq_data = if self.imode == InterleaveMode::S || self.imode == InterleaveMode::B {
                raw_data
            } else {
                crate::jbp::image::interleave::to_band_sequential(
                    &raw_data,
                    self.imode,
                    actual_rows,
                    actual_cols,
                    self.nbands,
                    bpp,
                )?
            };

            // Convert from big-endian (NITF on-disk) to native-endian (internal contract)
            let final_data = swap_be_to_ne(&bsq_data, bpp);

            // Return shape as [bands, rows, cols] (CHW format)
            Ok((final_data, [self.nbands, actual_rows, actual_cols]))
        }

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

                for row in 0..actual_rows {
                    let row_offset = block_offset + (row as usize) * (self.nppbh as usize) * bpp;
                    let row_bytes = (actual_cols as usize) * bpp;

                    if row_offset + row_bytes > self.image_data.len() {
                        return Err(CodecError::Decode(format!(
                            "Block data out of bounds: offset {} + {} > {}",
                            row_offset, row_bytes, self.image_data.len()
                        )));
                    }

                    output.extend_from_slice(&self.image_data[row_offset..row_offset + row_bytes]);
                }
            }

            Ok(output)
        }

        fn read_block_mode_bpr(
            &self,
            block_row: u32,
            block_col: u32,
            actual_rows: u32,
            actual_cols: u32,
        ) -> Result<Vec<u8>, CodecError> {
            let bpp = self.bytes_per_pixel();
            let block_size = (self.nppbh as usize) * (self.nppbv as usize) * (self.nbands as usize) * bpp;
            let block_index = (block_row as usize) * (self.nbpr as usize) + (block_col as usize);
            let offset = block_index * block_size;

            let actual_pixels = (actual_rows as usize) * (actual_cols as usize);
            let mut output = Vec::with_capacity(actual_pixels * (self.nbands as usize) * bpp);

            match self.imode {
                InterleaveMode::B => {
                    let pixels_per_band = (self.nppbh as usize) * (self.nppbv as usize);
                    for band in 0..self.nbands {
                        let band_offset = offset + (band as usize) * pixels_per_band * bpp;
                        for row in 0..actual_rows {
                            let row_offset = band_offset + (row as usize) * (self.nppbh as usize) * bpp;
                            let row_bytes = (actual_cols as usize) * bpp;
                            output.extend_from_slice(&self.image_data[row_offset..row_offset + row_bytes]);
                        }
                    }
                }
                InterleaveMode::P => {
                    let pixel_size = (self.nbands as usize) * bpp;
                    for row in 0..actual_rows {
                        for col in 0..actual_cols {
                            let pixel_offset = offset
                                + ((row as usize) * (self.nppbh as usize) + (col as usize)) * pixel_size;
                            output.extend_from_slice(&self.image_data[pixel_offset..pixel_offset + pixel_size]);
                        }
                    }
                }
                InterleaveMode::R => {
                    let row_size = (self.nppbh as usize) * bpp;
                    for row in 0..actual_rows {
                        for band in 0..self.nbands {
                            let row_offset = offset
                                + ((row as usize) * (self.nbands as usize) + (band as usize)) * row_size;
                            let actual_row_bytes = (actual_cols as usize) * bpp;
                            output.extend_from_slice(&self.image_data[row_offset..row_offset + actual_row_bytes]);
                        }
                    }
                }
                InterleaveMode::S => unreachable!("IMODE S handled separately"),
            }

            Ok(output)
        }
    }

    /// Mock ImageAssetProvider for testing TileAssembler
    /// Returns data in BSQ format (same as JBP reader)
    struct MockBsqImageProvider {
        /// Full image data in BSQ format
        image_data: Vec<u8>,
        /// Image dimensions
        nrows: u32,
        ncols: u32,
        nbands: u32,
        /// Block dimensions
        block_width: u32,
        block_height: u32,
        /// Bytes per pixel
        bytes_per_pixel: usize,
    }

    impl MockBsqImageProvider {
        fn new(
            image_data: Vec<u8>,
            nrows: u32,
            ncols: u32,
            nbands: u32,
            block_width: u32,
            block_height: u32,
            bytes_per_pixel: usize,
        ) -> Self {
            Self {
                image_data,
                nrows,
                ncols,
                nbands,
                block_width,
                block_height,
                bytes_per_pixel,
            }
        }

        fn block_grid_size(&self) -> (u32, u32) {
            let rows = (self.nrows + self.block_height - 1) / self.block_height;
            let cols = (self.ncols + self.block_width - 1) / self.block_width;
            (rows, cols)
        }
    }

    impl crate::traits::AssetProvider for MockBsqImageProvider {
        fn key(&self) -> &str { "mock" }
        fn title(&self) -> &str { "Mock Image" }
        fn description(&self) -> &str { "Mock image for testing" }
        fn media_type(&self) -> &str { "application/octet-stream" }
        fn roles(&self) -> &[String] { &[] }
        fn asset_type(&self) -> crate::types::AssetType { crate::types::AssetType::Image }
        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> { Ok(self.image_data.clone()) }
        fn metadata(&self) -> Arc<dyn crate::traits::MetadataProvider> {
            Arc::new(EmptyMetadataProvider)
        }
        fn as_any(&self) -> &dyn std::any::Any { self }
    }

    struct EmptyMetadataProvider;
    impl crate::traits::MetadataProvider for EmptyMetadataProvider {
        fn as_dict(&self, _prefix: Option<&str>) -> std::collections::HashMap<String, serde_json::Value> {
            std::collections::HashMap::new()
        }
        fn raw(&self) -> &[u8] { &[] }
    }

    impl ImageAssetProvider for MockBsqImageProvider {
        fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
            if resolution_level != 0 {
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
            _bands: Option<&[u32]>,
        ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
            if resolution_level != 0 {
                return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, resolution_level));
            }

            let (grid_rows, grid_cols) = self.block_grid_size();
            if block_row >= grid_rows || block_col >= grid_cols {
                return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, resolution_level));
            }

            // Calculate block bounds
            let start_x = block_col * self.block_width;
            let start_y = block_row * self.block_height;
            let end_x = (start_x + self.block_width).min(self.ncols);
            let end_y = (start_y + self.block_height).min(self.nrows);
            let actual_width = end_x - start_x;
            let actual_height = end_y - start_y;

            // Extract block data in BSQ format
            let pixels_per_band_full = (self.nrows as usize) * (self.ncols as usize);
            let block_pixels = (actual_width as usize) * (actual_height as usize);
            let mut block_data = Vec::with_capacity(block_pixels * (self.nbands as usize) * self.bytes_per_pixel);

            for band in 0..self.nbands {
                let band_offset = (band as usize) * pixels_per_band_full * self.bytes_per_pixel;
                for row in start_y..end_y {
                    let row_offset = band_offset + (row as usize) * (self.ncols as usize) * self.bytes_per_pixel;
                    let start_offset = row_offset + (start_x as usize) * self.bytes_per_pixel;
                    let end_offset = start_offset + (actual_width as usize) * self.bytes_per_pixel;
                    block_data.extend_from_slice(&self.image_data[start_offset..end_offset]);
                }
            }

            Ok((block_data, [self.nbands, actual_height, actual_width]))
        }

        fn num_resolution_levels(&self) -> u32 { 1 }
        fn num_bands(&self) -> u32 { self.nbands }
        fn num_rows(&self) -> u32 { self.nrows }
        fn num_columns(&self) -> u32 { self.ncols }
        fn num_pixels_per_block_horizontal(&self) -> u32 { self.block_width }
        fn num_pixels_per_block_vertical(&self) -> u32 { self.block_height }
        fn num_bits_per_pixel(&self) -> u32 { (self.bytes_per_pixel * 8) as u32 }
        fn actual_bits_per_pixel(&self) -> u32 { (self.bytes_per_pixel * 8) as u32 }
        fn pixel_value_type(&self) -> crate::types::PixelType { crate::types::PixelType::UInt8 }
        fn pad_pixel_value(&self) -> f64 { 0.0 }
    }

    /// Strategy for generating tile size combinations
    fn tile_size_strategy() -> impl Strategy<Value = (u32, u32, u32, u32)> {
        (
            2u32..=16,  // source tile width
            2u32..=16,  // source tile height
            2u32..=16,  // output tile width
            2u32..=16,  // output tile height
        )
    }

    /// Strategy for generating image dimensions that work with tile sizes
    fn image_with_tiles_strategy() -> impl Strategy<Value = (u32, u32, u32, u32, u32, u32, u32)> {
        (
            4u32..=32,  // image width
            4u32..=32,  // image height
            1u32..=3,   // nbands
            2u32..=8,   // source tile width
            2u32..=8,   // source tile height
            2u32..=8,   // output tile width
            2u32..=8,   // output tile height
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 2: Tile Size Conversion Preserves Pixels
        /// For any combination of input tile size and output tile size, and any image
        /// dimensions, the pixel values in the assembled output SHALL exactly match
        /// the pixel values from the source.
        /// **Validates: Requirements 5.1, 5.3, 5.4, 5.5, 7.2**
        #[test]
        fn tile_size_conversion_preserves_pixels(
            (image_width, image_height, nbands, src_tile_w, src_tile_h, out_tile_w, out_tile_h)
                in image_with_tiles_strategy()
        ) {
            // Feature: block-encoder-refactor, Property 2: Tile Size Conversion Preserves Pixels
            let bytes_per_pixel = 1usize;
            let total_pixels = (image_width as usize) * (image_height as usize) * (nbands as usize);

            // Generate deterministic image data in BSQ format
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Create mock provider with source tile size
            let provider = MockBsqImageProvider::new(
                original_data.clone(),
                image_height,
                image_width,
                nbands,
                src_tile_w,
                src_tile_h,
                bytes_per_pixel,
            );

            // Create TileAssembler with different output tile size
            let assembler = TileAssembler::new(&provider, out_tile_w, out_tile_h);
            let (grid_rows, grid_cols) = assembler.output_grid_size();

            // Reassemble the full image from output tiles
            let mut reassembled = vec![0u8; total_pixels];
            let pixels_per_band = (image_width as usize) * (image_height as usize);

            for out_row in 0..grid_rows {
                for out_col in 0..grid_cols {
                    let (tile_data, shape) = assembler.get_output_tile(out_row, out_col).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    // Calculate where this tile goes in the full image
                    let start_x = out_col * out_tile_w;
                    let start_y = out_row * out_tile_h;

                    // Copy tile data back to reassembled image (BSQ format)
                    let tile_pixels = (tile_width as usize) * (tile_height as usize);
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels * bytes_per_pixel;
                        let dst_band_offset = (band as usize) * pixels_per_band * bytes_per_pixel;

                        for row in 0..tile_height {
                            let src_row_offset = src_band_offset + (row as usize) * (tile_width as usize) * bytes_per_pixel;
                            let dst_row_offset = dst_band_offset
                                + ((start_y + row) as usize) * (image_width as usize) * bytes_per_pixel
                                + (start_x as usize) * bytes_per_pixel;
                            let row_bytes = (tile_width as usize) * bytes_per_pixel;

                            reassembled[dst_row_offset..dst_row_offset + row_bytes]
                                .copy_from_slice(&tile_data[src_row_offset..src_row_offset + row_bytes]);
                        }
                    }
                }
            }

            // Verify all pixels match
            prop_assert_eq!(
                reassembled, original_data,
                "Tile assembly should preserve all pixels for {}x{} image with {} bands, \
                 source tiles {}x{}, output tiles {}x{}",
                image_width, image_height, nbands, src_tile_w, src_tile_h, out_tile_w, out_tile_h
            );
        }
    }
}

/// Property-based tests for round-trip consistency (Task 9).
/// These tests validate that encoding with JBPDatasetWriter and decoding with
/// JBPDatasetReader produces pixel-perfect results.
#[cfg(test)]
mod round_trip_property_tests {
    use super::*;
    use crate::buffered::{BufferedImageAssetProvider, BufferedMetadataProvider, MemoryImageConfig};
    use crate::jbp::types::NitfFormat;
    use crate::jbp::reader::JBPDatasetReader;
    use crate::jbp::writer::JBPDatasetWriter;
    use crate::traits::{DatasetReader, DatasetWriter, ImageAssetProvider};
    use crate::types::AssetType;
    use proptest::prelude::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Strategy for generating valid image dimensions for round-trip tests.
    /// Uses small dimensions for fast test execution.
    fn round_trip_dimensions_strategy() -> impl Strategy<Value = (u32, u32, u32)> {
        (
            4u32..=32,  // nrows
            4u32..=32,  // ncols
            1u32..=4,   // nbands
        )
    }

    /// Strategy for generating block sizes that are valid for given dimensions.
    fn block_size_strategy() -> impl Strategy<Value = (u32, u32)> {
        (
            2u32..=16,  // block_width
            2u32..=16,  // block_height
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 1: Round-Trip Consistency
        /// For any valid image data (any dimensions, any bit depth, any band count),
        /// encoding with JBPDatasetWriter and then decoding with JBPDatasetReader
        /// SHALL produce byte-identical pixel data with matching dimensions.
        /// **Validates: Requirements 7.1, 7.4**
        #[test]
        fn round_trip_consistency(
            (nrows, ncols, nbands) in round_trip_dimensions_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property 1: Round-Trip Consistency
            let dir = tempdir().unwrap();
            let path = dir.path().join("round_trip_test.ntf");

            // Create test image configuration
            let config = MemoryImageConfig::new(ncols, nrows)
                .with_bands(nbands)
                .with_block_size(ncols, nrows); // Single block for simplicity

            let provider = BufferedImageAssetProvider::new("test_image", config);

            // Generate deterministic test data in BSQ format
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Set the full image data
            provider.set_full_image(&original_data).unwrap();

            // Write the NITF file
            let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
            writer.add_asset(
                "test_image",
                Arc::new(provider),
                "Test Image",
                "Round-trip test",
                &[]
            ).unwrap();
            writer.close().unwrap();

            // Read the file back
            let file_data = std::fs::read(&path).unwrap();
            let reader = JBPDatasetReader::from_bytes(&file_data).unwrap();

            // Verify we have one image asset
            let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
            prop_assert_eq!(asset_keys.len(), 1, "Should have exactly one image asset");

            // Get the image asset
            let asset = reader.get_asset(&asset_keys[0]).unwrap();

            // Downcast to ImageAssetProvider
            let image_provider = asset.as_any()
                .downcast_ref::<crate::jbp::asset::JBPImageAssetProvider>()
                .expect("Asset should be an image provider");

            // Verify dimensions
            prop_assert_eq!(image_provider.num_columns(), ncols, "Width mismatch");
            prop_assert_eq!(image_provider.num_rows(), nrows, "Height mismatch");
            prop_assert_eq!(image_provider.num_bands(), nbands, "Band count mismatch");

            // Read back all blocks and reassemble the image
            let block_width = image_provider.num_pixels_per_block_horizontal();
            let block_height = image_provider.num_pixels_per_block_vertical();
            let grid_cols = (ncols + block_width - 1) / block_width;
            let grid_rows = (nrows + block_height - 1) / block_height;

            // Reassemble the full image from blocks
            let pixels_per_band = (nrows as usize) * (ncols as usize);
            let mut reassembled = vec![0u8; total_pixels];

            for block_row in 0..grid_rows {
                for block_col in 0..grid_cols {
                    let (block_data, shape) = image_provider.get_block(block_row, block_col, 0, None).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    // Calculate where this block goes in the full image
                    let start_x = block_col * block_width;
                    let start_y = block_row * block_height;

                    // Block data is in BSQ format (band-sequential)
                    let tile_pixels_per_band = (tile_width as usize) * (tile_height as usize);
                    
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels_per_band;
                        let dst_band_offset = (band as usize) * pixels_per_band;
                        for row in 0..tile_height {
                            for col in 0..tile_width {
                                let src_offset = src_band_offset + (row as usize) * (tile_width as usize) + (col as usize);
                                
                                let dst_row = start_y + row;
                                let dst_col = start_x + col;
                                let dst_offset = dst_band_offset + (dst_row as usize) * (ncols as usize) + (dst_col as usize);
                                
                                if src_offset < block_data.len() && dst_offset < reassembled.len() {
                                    reassembled[dst_offset] = block_data[src_offset];
                                }
                            }
                        }
                    }
                }
            }

            // Verify pixel data matches original
            prop_assert_eq!(
                reassembled, original_data,
                "Round-trip should preserve all pixels for {}x{}x{} image",
                ncols, nrows, nbands
            );
        }

        /// Property 5: Block Grid Calculation
        /// For any image dimensions (NROWS, NCOLS) and block dimensions (NPPBH, NPPBV),
        /// the block grid size returned by block_grid_size() SHALL equal
        /// (ceil(NROWS/NPPBV), ceil(NCOLS/NPPBH)).
        /// **Validates: Requirements 1.5**
        #[test]
        fn block_grid_calculation(
            (nrows, ncols, nbands) in round_trip_dimensions_strategy(),
            (block_width, block_height) in block_size_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property 5: Block Grid Calculation
            let encoder = UncompressedBlockEncoder::new(
                nrows, ncols, nbands, 8, InterleaveMode::B, block_width, block_height
            );

            let (grid_rows, grid_cols) = encoder.block_grid_size();

            // Expected values using ceiling division
            let expected_rows = (nrows + block_height - 1) / block_height;
            let expected_cols = (ncols + block_width - 1) / block_width;

            prop_assert_eq!(
                grid_rows, expected_rows,
                "Grid rows should be ceil({}/{}) = {}, got {}",
                nrows, block_height, expected_rows, grid_rows
            );
            prop_assert_eq!(
                grid_cols, expected_cols,
                "Grid cols should be ceil({}/{}) = {}, got {}",
                ncols, block_width, expected_cols, grid_cols
            );
        }

        /// Property 4: Edge Block Handling
        /// For any image dimensions that are not evenly divisible by the block size,
        /// edge blocks (partial blocks at right and bottom boundaries) SHALL be
        /// encoded correctly with the actual pixel count, not padded dimensions.
        /// **Validates: Requirements 2.5**
        #[test]
        fn edge_block_handling(
            nrows in 5u32..=32,
            ncols in 5u32..=32,
            nbands in 1u32..=3,
            block_width in 3u32..=8,
            block_height in 3u32..=8,
        ) {
            // Feature: block-encoder-refactor, Property 4: Edge Block Handling
            // Skip cases where dimensions are evenly divisible (no edge blocks)
            prop_assume!(nrows % block_height != 0 || ncols % block_width != 0);

            let dir = tempdir().unwrap();
            let path = dir.path().join("edge_block_test.ntf");

            // Create test image configuration with non-divisible dimensions
            let config = MemoryImageConfig::new(ncols, nrows)
                .with_bands(nbands)
                .with_block_size(block_width, block_height);

            let provider = BufferedImageAssetProvider::new("test_image", config);

            // Generate deterministic test data in BSQ format
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Set the full image data
            provider.set_full_image(&original_data).unwrap();

            // Write the NITF file
            let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
            writer.add_asset(
                "test_image",
                Arc::new(provider),
                "Test Image",
                "Edge block test",
                &[]
            ).unwrap();
            writer.close().unwrap();

            // Read the file back
            let file_data = std::fs::read(&path).unwrap();
            let reader = JBPDatasetReader::from_bytes(&file_data).unwrap();
            let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
            let asset = reader.get_asset(&asset_keys[0]).unwrap();

            let image_provider = asset.as_any()
                .downcast_ref::<crate::jbp::asset::JBPImageAssetProvider>()
                .expect("Asset should be an image provider");

            // Calculate expected grid size
            let expected_grid_cols = (ncols + block_width - 1) / block_width;
            let expected_grid_rows = (nrows + block_height - 1) / block_height;

            // Verify edge blocks have correct dimensions
            // Check right edge block (last column)
            // Shape is [bands, rows, cols] (CHW format)
            if ncols % block_width != 0 {
                let edge_col = expected_grid_cols - 1;
                let expected_edge_width = ncols - (edge_col * block_width);
                
                let (block_data, shape) = image_provider.get_block(0, edge_col, 0, None).unwrap();
                prop_assert_eq!(
                    shape[2], expected_edge_width,
                    "Right edge block width should be {}, got {}",
                    expected_edge_width, shape[2]
                );
            }

            // Check bottom edge block (last row)
            if nrows % block_height != 0 {
                let edge_row = expected_grid_rows - 1;
                let expected_edge_height = nrows - (edge_row * block_height);
                
                let (block_data, shape) = image_provider.get_block(edge_row, 0, 0, None).unwrap();
                prop_assert_eq!(
                    shape[1], expected_edge_height,
                    "Bottom edge block height should be {}, got {}",
                    expected_edge_height, shape[1]
                );
            }

            // Reassemble and verify all pixels
            let pixels_per_band = (nrows as usize) * (ncols as usize);
            let mut reassembled = vec![0u8; total_pixels];

            for block_row in 0..expected_grid_rows {
                for block_col in 0..expected_grid_cols {
                    let (block_data, shape) = image_provider.get_block(block_row, block_col, 0, None).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    let start_x = block_col * block_width;
                    let start_y = block_row * block_height;

                    // Block data is in BSQ format (band-sequential)
                    let tile_pixels_per_band = (tile_height as usize) * (tile_width as usize);
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels_per_band;
                        let dst_band_offset = (band as usize) * pixels_per_band;
                        for row in 0..tile_height {
                            for col in 0..tile_width {
                                let src_offset = src_band_offset + (row as usize) * (tile_width as usize) + (col as usize);
                                
                                let dst_row = start_y + row;
                                let dst_col = start_x + col;
                                let dst_offset = dst_band_offset + (dst_row as usize) * (ncols as usize) + (dst_col as usize);
                                
                                if src_offset < block_data.len() && dst_offset < reassembled.len() {
                                    reassembled[dst_offset] = block_data[src_offset];
                                }
                            }
                        }
                    }
                }
            }

            prop_assert_eq!(
                reassembled, original_data,
                "Edge block handling should preserve all pixels for {}x{}x{} image with {}x{} blocks",
                ncols, nrows, nbands, block_width, block_height
            );
        }

        /// Property: Tile Size Round-Trip
        /// For images with various source tile sizes, writing with different output
        /// tile sizes and reading back SHALL preserve pixel values exactly.
        /// **Validates: Requirements 5.5, 7.2**
        #[test]
        fn tile_size_round_trip(
            (nrows, ncols, nbands) in round_trip_dimensions_strategy(),
            (src_block_w, src_block_h) in block_size_strategy(),
            (out_block_w, out_block_h) in block_size_strategy(),
        ) {
            // Feature: block-encoder-refactor, Property: Tile Size Round-Trip
            let dir = tempdir().unwrap();
            let path = dir.path().join("tile_size_round_trip_test.ntf");

            // Create test image with source tile size
            let config = MemoryImageConfig::new(ncols, nrows)
                .with_bands(nbands)
                .with_block_size(src_block_w, src_block_h);

            let provider = BufferedImageAssetProvider::new("test_image", config);

            // Generate deterministic test data in BSQ format
            let total_pixels = (nrows as usize) * (ncols as usize) * (nbands as usize);
            let original_data: Vec<u8> = (0..total_pixels).map(|i| (i % 256) as u8).collect();

            // Set the full image data
            provider.set_full_image(&original_data).unwrap();

            // Create metadata with output block size hints
            let metadata = BufferedMetadataProvider::new();
            metadata.set("nppbh", &out_block_w.to_string());
            metadata.set("nppbv", &out_block_h.to_string());

            let provider_with_meta = BufferedImageAssetProvider::new("test_image", 
                MemoryImageConfig::new(ncols, nrows)
                    .with_bands(nbands)
                    .with_block_size(src_block_w, src_block_h)
            ).with_metadata(Arc::new(metadata));
            provider_with_meta.set_full_image(&original_data).unwrap();

            // Write the NITF file
            let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
            writer.add_asset(
                "test_image",
                Arc::new(provider_with_meta),
                "Test Image",
                "Tile size round-trip test",
                &[]
            ).unwrap();
            writer.close().unwrap();

            // Read the file back
            let file_data = std::fs::read(&path).unwrap();
            let reader = JBPDatasetReader::from_bytes(&file_data).unwrap();
            let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
            let asset = reader.get_asset(&asset_keys[0]).unwrap();

            let image_provider = asset.as_any()
                .downcast_ref::<crate::jbp::asset::JBPImageAssetProvider>()
                .expect("Asset should be an image provider");

            // Verify dimensions match
            prop_assert_eq!(image_provider.num_columns(), ncols);
            prop_assert_eq!(image_provider.num_rows(), nrows);
            prop_assert_eq!(image_provider.num_bands(), nbands);

            // Reassemble the full image from blocks
            let read_block_width = image_provider.num_pixels_per_block_horizontal();
            let read_block_height = image_provider.num_pixels_per_block_vertical();
            let grid_cols = (ncols + read_block_width - 1) / read_block_width;
            let grid_rows = (nrows + read_block_height - 1) / read_block_height;

            let pixels_per_band = (nrows as usize) * (ncols as usize);
            let mut reassembled = vec![0u8; total_pixels];

            for block_row in 0..grid_rows {
                for block_col in 0..grid_cols {
                    let (block_data, shape) = image_provider.get_block(block_row, block_col, 0, None).unwrap();
                    // Shape is [bands, rows, cols] (CHW format)
                    let tile_bands = shape[0];
                    let tile_height = shape[1];
                    let tile_width = shape[2];

                    let start_x = block_col * read_block_width;
                    let start_y = block_row * read_block_height;

                    // Block data is in BSQ format
                    let tile_pixels_per_band = (tile_height as usize) * (tile_width as usize);
                    for band in 0..tile_bands {
                        let src_band_offset = (band as usize) * tile_pixels_per_band;
                        let dst_band_offset = (band as usize) * pixels_per_band;
                        for row in 0..tile_height {
                            for col in 0..tile_width {
                                let src_offset = src_band_offset + (row as usize) * (tile_width as usize) + (col as usize);
                                
                                let dst_row = start_y + row;
                                let dst_col = start_x + col;
                                let dst_offset = dst_band_offset + (dst_row as usize) * (ncols as usize) + (dst_col as usize);
                                
                                if src_offset < block_data.len() && dst_offset < reassembled.len() {
                                    reassembled[dst_offset] = block_data[src_offset];
                                }
                            }
                        }
                    }
                }
            }

            prop_assert_eq!(
                reassembled, original_data,
                "Tile size round-trip should preserve all pixels for {}x{}x{} image, \
                 source tiles {}x{}, output tiles {}x{}",
                ncols, nrows, nbands, src_block_w, src_block_h, out_block_w, out_block_h
            );
        }
    }
}
