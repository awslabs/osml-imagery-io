//! Uncompressed (IC=NC) block encoder for NITF image data.
//!
//! This module implements the [`UncompressedBlockEncoder`] which handles images
//! with no compression. It accepts blocks in band-sequential format and converts
//! them to the target interleave mode, performing endian swaps as needed.

use crate::error::CodecError;
use crate::jbp::image::interleave::from_band_sequential;
use crate::jbp::image::types::InterleaveMode;

use super::encoder::{swap_ne_to_be, BlockEncoder};

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
        let packed_band_size = ((nppbh as usize) * (nppbv as usize) * (nbpp as usize)).div_ceil(8);
        let packed_block_size = packed_band_size * (nbands as usize);
        let total_blocks = (nbpc as usize) * (nbpr as usize);
        let total_size = total_blocks * packed_block_size;

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

    /// Calculate the number of bytes per pixel in decoded (container) form.
    fn bytes_per_pixel(&self) -> usize {
        (self.nbpp as usize).div_ceil(8)
    }

    /// Whether pixels are stored as a bit-packed bitstream on disk.
    ///
    /// Returns `true` for any NBPP that is not a multiple of 8 (e.g., 1, 2, 4, 12).
    #[allow(clippy::manual_is_multiple_of)]
    fn is_bit_packed(&self) -> bool {
        self.nbpp % 8 != 0
    }

    /// On-disk size of one band's data within a block.
    fn packed_band_size_bytes(&self) -> usize {
        ((self.nppbh as usize) * (self.nppbv as usize) * (self.nbpp as usize)).div_ceil(8)
    }

    /// On-disk size of one complete block (all bands).
    fn packed_block_size_bytes(&self) -> usize {
        self.packed_band_size_bytes() * (self.nbands as usize)
    }

    /// Pack pixel data from container representation into a bit-packed bitstream.
    ///
    /// Input is `bpp` bytes per pixel (big-endian) as produced by the native-to-BE
    /// swap. For NBPP <= 8: 1 byte per pixel. For NBPP 9-16: 2 bytes per pixel
    /// (big-endian u16). Only the lowest `nbpp` bits of each pixel value are packed.
    fn pack_bitstream(&self, pixels: &[u8]) -> Vec<u8> {
        let nbpp = self.nbpp as usize;
        let bpp = self.bytes_per_pixel();
        let num_pixels = pixels.len() / bpp;
        let total_bits = num_pixels * nbpp;
        let packed_len = total_bits.div_ceil(8);
        let mut packed = vec![0u8; packed_len];
        let mask: u32 = (1u32 << nbpp) - 1;

        let mut bit_offset = 0usize;
        for i in 0..num_pixels {
            let value: u32 = match bpp {
                1 => pixels[i] as u32,
                2 => u16::from_be_bytes([pixels[i * 2], pixels[i * 2 + 1]]) as u32,
                4 => u32::from_be_bytes([
                    pixels[i * 4],
                    pixels[i * 4 + 1],
                    pixels[i * 4 + 2],
                    pixels[i * 4 + 3],
                ]),
                _ => unreachable!("NBPP > 32 not supported for bit-packed"),
            };
            let value = value & mask;

            // Write nbpp bits into the packed buffer (MSB-first)
            let mut bits_remaining = nbpp;
            let mut current_bit = bit_offset;
            let mut val_bits_left = value;

            while bits_remaining > 0 {
                let byte_idx = current_bit / 8;
                let bit_within_byte = current_bit % 8;
                let bits_available = 8 - bit_within_byte;
                let bits_to_write = bits_remaining.min(bits_available);

                let shift = bits_remaining - bits_to_write;
                let fragment = ((val_bits_left >> shift) & ((1u32 << bits_to_write) - 1)) as u8;

                packed[byte_idx] |= fragment << (bits_available - bits_to_write);
                bits_remaining -= bits_to_write;
                val_bits_left &= (1u32 << shift) - 1;
                current_bit += bits_to_write;
            }

            bit_offset += nbpp;
        }

        packed
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
        let blocks_per_band = (self.nbpr as usize) * (self.nbpc as usize);
        let single_band_block_size = self.packed_band_size_bytes();
        let block_index = (block_row as usize) * (self.nbpr as usize) + (block_col as usize);
        let actual_pixels_per_band = (block_rows as usize) * (block_cols as usize);

        if self.is_bit_packed() {
            let bpp = self.bytes_per_pixel();
            let band_bytes = actual_pixels_per_band * bpp;
            for band in 0..self.nbands {
                let band_offset = (band as usize) * blocks_per_band * single_band_block_size;
                let block_offset = band_offset + block_index * single_band_block_size;
                let src_band_offset = (band as usize) * band_bytes;
                let band_pixels = &data[src_band_offset..src_band_offset + band_bytes];
                let packed = self.pack_bitstream(band_pixels);
                self.encoded_data[block_offset..block_offset + packed.len()]
                    .copy_from_slice(&packed);
            }
        } else {
            let bpp = self.bytes_per_pixel();
            for band in 0..self.nbands {
                let band_offset = (band as usize) * blocks_per_band * single_band_block_size;
                let block_offset = band_offset + block_index * single_band_block_size;
                let src_band_offset = (band as usize) * actual_pixels_per_band * bpp;

                for row in 0..block_rows {
                    let src_row_offset =
                        src_band_offset + (row as usize) * (block_cols as usize) * bpp;
                    let dst_row_offset =
                        block_offset + (row as usize) * (self.nppbh as usize) * bpp;
                    let row_bytes = (block_cols as usize) * bpp;

                    self.encoded_data[dst_row_offset..dst_row_offset + row_bytes]
                        .copy_from_slice(&data[src_row_offset..src_row_offset + row_bytes]);
                }
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
        let block_size = self.packed_block_size_bytes();
        let block_index = (block_row as usize) * (self.nbpr as usize) + (block_col as usize);
        let block_offset = block_index * block_size;

        if self.is_bit_packed() {
            let bpp = self.bytes_per_pixel();
            let actual_pixels_per_band = (block_rows as usize) * (block_cols as usize);
            let band_bytes = actual_pixels_per_band * bpp;
            let packed_band_size = self.packed_band_size_bytes();

            for band in 0..self.nbands as usize {
                let src_band_offset = band * band_bytes;
                let band_pixels = &data[src_band_offset..src_band_offset + band_bytes];
                let packed = self.pack_bitstream(band_pixels);
                let dst_band_offset = block_offset + band * packed_band_size;
                self.encoded_data[dst_band_offset..dst_band_offset + packed.len()]
                    .copy_from_slice(&packed);
            }

            return Ok(());
        }

        let bpp = self.bytes_per_pixel();

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
        let expected_size = (shape[0] as usize) * (shape[1] as usize) * (shape[2] as usize) * bpp;
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

#[cfg(test)]
mod tests {
    use super::*;

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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Valid coordinates - shape is [bands, rows, cols] (CHW format)
            assert!(encoder.encode_block(0, 0, &data, [1, 32, 32]).is_ok());

            // Invalid row
            let result = encoder.encode_block(2, 0, &data, [1, 32, 32]);
            assert!(matches!(
                result,
                Err(CodecError::InvalidBlockCoordinates(2, 0, 0))
            ));

            // Invalid column
            let result = encoder.encode_block(0, 2, &data, [1, 32, 32]);
            assert!(matches!(
                result,
                Err(CodecError::InvalidBlockCoordinates(0, 2, 0))
            ));
        }

        #[test]
        fn encode_block_validates_data_size() {
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);

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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 32);
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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
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
            encoder
                .encode_block(0, 1, &data_edge_col, [1, 4, 2])
                .unwrap();

            // Edge block (1,0): 2x4
            let data_edge_row = vec![3u8; 2 * 4];
            encoder
                .encode_block(1, 0, &data_edge_row, [1, 2, 4])
                .unwrap();

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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
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
                    assert_eq!(
                        col, 10,
                        "Error should include the invalid column coordinate"
                    );
                }
                _ => panic!("Expected InvalidBlockCoordinates error"),
            }

            // Test both row and column out of bounds
            let result = encoder.encode_block(100, 200, &data, [1, 32, 32]);
            assert!(result.is_err());
            match result {
                Err(CodecError::InvalidBlockCoordinates(row, col, _)) => {
                    assert_eq!(row, 100, "Error should include the invalid row coordinate");
                    assert_eq!(
                        col, 200,
                        "Error should include the invalid column coordinate"
                    );
                }
                _ => panic!("Expected InvalidBlockCoordinates error"),
            }
        }

        /// Test that invalid coordinates error is returned for boundary cases
        /// Validates: Requirement 8.1
        #[test]
        fn invalid_coordinates_at_grid_boundary() {
            // 65x65 image with 32x32 blocks = 3x3 grid (indices 0, 1, 2 valid)
            let mut encoder =
                UncompressedBlockEncoder::new(65, 65, 1, 8, InterleaveMode::B, 32, 32);
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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);

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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 3, 8, InterleaveMode::B, 32, 32);

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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 16, InterleaveMode::B, 32, 32);

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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
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
            let mut encoder =
                UncompressedBlockEncoder::new(96, 96, 1, 8, InterleaveMode::B, 32, 32);
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
            let mut encoder =
                UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
            let data = vec![0u8; 32 * 32];

            // Encode all 4 blocks in 2x2 grid
            // Shape is [bands, rows, cols] (CHW format)
            encoder.encode_block(0, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(0, 1, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(1, 0, &data, [1, 32, 32]).unwrap();
            encoder.encode_block(1, 1, &data, [1, 32, 32]).unwrap();

            let result = Box::new(encoder).finalize();
            assert!(
                result.is_ok(),
                "finalize should succeed when all blocks are encoded"
            );
        }

        /// Test incomplete encoding error includes grid size information
        /// Validates: Requirement 8.4
        #[test]
        fn incomplete_encoding_error_includes_grid_size() {
            let encoder = UncompressedBlockEncoder::new(64, 64, 1, 8, InterleaveMode::B, 32, 32);
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

    mod sub_byte_encode_tests {
        use super::*;
        use crate::jbp::image::decoder::BlockDecoder;
        use crate::jbp::image::nc_decoder::UncompressedBlockDecoder;
        use crate::jbp::image::types::{PixelJustification, PixelValueType};
        use crate::owned_buffer::OwnedBuffer;

        #[test]
        fn encode_1bpp_single_band_roundtrip() {
            // 8x4 block, 1 band, 1 bpp, IMODE=B
            let pixels: Vec<u8> = vec![
                1, 0, 1, 0, 1, 0, 1, 0, // row 0
                1, 1, 0, 0, 1, 1, 0, 0, // row 1
                1, 1, 1, 1, 0, 0, 0, 0, // row 2
                0, 0, 0, 0, 1, 1, 1, 1, // row 3
            ];

            let mut encoder = UncompressedBlockEncoder::new(4, 8, 1, 1, InterleaveMode::B, 8, 4);
            encoder.encode_block(0, 0, &pixels, [1, 4, 8]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // Verify packed size: 32 pixels * 1 bit = 4 bytes
            assert_eq!(encoded.len(), 4);

            // Decode and verify round-trip
            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                4,
                8,
                1,
                1,
                8,
                4,
                1,
                1,
                1,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::B,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [1, 4, 8]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_1bpp_two_bands_roundtrip() {
            // 4x2 block, 2 bands, 1 bpp, IMODE=B
            let pixels: Vec<u8> = vec![
                1, 1, 1, 1, 0, 0, 0, 0, // band 0
                0, 0, 0, 0, 1, 1, 1, 1, // band 1
            ];

            let mut encoder = UncompressedBlockEncoder::new(2, 4, 2, 1, InterleaveMode::B, 4, 2);
            encoder.encode_block(0, 0, &pixels, [2, 2, 4]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // Verify packed size: 8 pixels/band * 1 bit * 2 bands = 2 bytes
            assert_eq!(encoded.len(), 2);

            // Decode and verify round-trip
            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                4,
                1,
                1,
                4,
                2,
                2,
                1,
                1,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::B,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [2, 2, 4]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_1bpp_imode_s_roundtrip() {
            // 4x2 block, 2 bands, 1 bpp, IMODE=S
            let pixels: Vec<u8> = vec![
                1, 1, 0, 0, 1, 1, 0, 0, // band 0
                0, 0, 1, 1, 0, 0, 1, 1, // band 1
            ];

            let mut encoder = UncompressedBlockEncoder::new(2, 4, 2, 1, InterleaveMode::S, 4, 2);
            encoder.encode_block(0, 0, &pixels, [2, 2, 4]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // Verify packed size: same as B (2 bytes)
            assert_eq!(encoded.len(), 2);

            // Decode and verify round-trip
            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                4,
                1,
                1,
                4,
                2,
                2,
                1,
                1,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::S,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [2, 2, 4]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_2bpp_roundtrip() {
            // 4x2 block, 1 band, 2 bpp
            let pixels: Vec<u8> = vec![0, 1, 2, 3, 3, 2, 1, 0];

            let mut encoder = UncompressedBlockEncoder::new(2, 4, 1, 2, InterleaveMode::B, 4, 2);
            encoder.encode_block(0, 0, &pixels, [1, 2, 4]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // Verify packed size: 8 pixels * 2 bits = 2 bytes
            assert_eq!(encoded.len(), 2);

            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                4,
                1,
                1,
                4,
                2,
                1,
                2,
                2,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::B,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [1, 2, 4]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_4bpp_roundtrip() {
            // 2x2 block, 1 band, 4 bpp
            let pixels: Vec<u8> = vec![0x0A, 0x0B, 0x0C, 0x0D];

            let mut encoder = UncompressedBlockEncoder::new(2, 2, 1, 4, InterleaveMode::B, 2, 2);
            encoder.encode_block(0, 0, &pixels, [1, 2, 2]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // Verify packed size: 4 pixels * 4 bits = 2 bytes
            assert_eq!(encoded.len(), 2);

            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                2,
                1,
                1,
                2,
                2,
                1,
                4,
                4,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::B,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [1, 2, 2]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_buffer_size_matches_luinband2() {
            // LUinBand2.ntf scenario: 35x18 block, 2 bands, 1 bpp
            let encoder = UncompressedBlockEncoder::new(18, 35, 2, 1, InterleaveMode::B, 35, 18);
            let (grid_rows, grid_cols) = encoder.block_grid_size();
            assert_eq!((grid_rows, grid_cols), (1, 1));

            // Expected packed size: ceil(35*18*1/8) * 2 = 79 * 2 = 158
            let result = Box::new(encoder).finalize();
            // finalize will fail because no blocks encoded, but we can check allocated size
            assert!(result.is_err());
            // Instead, create and encode to check size
            let mut encoder =
                UncompressedBlockEncoder::new(18, 35, 2, 1, InterleaveMode::B, 35, 18);
            let pixels = vec![0u8; 35 * 18 * 2]; // 2 bands, 1 byte per pixel input
            encoder.encode_block(0, 0, &pixels, [2, 18, 35]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();
            assert_eq!(encoded.len(), 158);
        }

        #[test]
        fn encode_12bpp_single_band_roundtrip() {
            // 2x2 block, 1 band, 12 bpp, IMODE=B
            // Input: 4 pixels as native-endian u16, values [0x123, 0x456, 0x789, 0xABC]
            let mut pixels = Vec::new();
            for val in [0x123u16, 0x456, 0x789, 0xABC] {
                pixels.extend_from_slice(&val.to_ne_bytes());
            }

            let mut encoder = UncompressedBlockEncoder::new(2, 2, 1, 12, InterleaveMode::B, 2, 2);
            encoder.encode_block(0, 0, &pixels, [1, 2, 2]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // Verify packed size: 4 pixels * 12 bits = 48 bits = 6 bytes
            assert_eq!(encoded.len(), 6);

            // Decode and verify round-trip
            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                2,
                1,
                1,
                2,
                2,
                1,
                12,
                12,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::B,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [1, 2, 2]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_12bpp_two_bands_roundtrip() {
            // 2x2 block, 2 bands, 12 bpp, IMODE=B
            let values: Vec<u16> = vec![
                0x100, 0x200, 0x300, 0x400, // band 0
                0xFFF, 0x001, 0x800, 0x555, // band 1
            ];
            let mut pixels = Vec::new();
            for val in &values {
                pixels.extend_from_slice(&val.to_ne_bytes());
            }

            let mut encoder = UncompressedBlockEncoder::new(2, 2, 2, 12, InterleaveMode::B, 2, 2);
            encoder.encode_block(0, 0, &pixels, [2, 2, 2]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            // packed_band_size = ceil(4*12/8) = 6 bytes; 2 bands = 12 bytes
            assert_eq!(encoded.len(), 12);

            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                2,
                1,
                1,
                2,
                2,
                2,
                12,
                12,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::B,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [2, 2, 2]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_12bpp_imode_s_roundtrip() {
            // 2x2 block, 2 bands, 12 bpp, IMODE=S
            let values: Vec<u16> = vec![
                0x111, 0x222, 0x333, 0x444, // band 0
                0xAAA, 0xBBB, 0xCCC, 0xDDD, // band 1
            ];
            let mut pixels = Vec::new();
            for val in &values {
                pixels.extend_from_slice(&val.to_ne_bytes());
            }

            let mut encoder = UncompressedBlockEncoder::new(2, 2, 2, 12, InterleaveMode::S, 2, 2);
            encoder.encode_block(0, 0, &pixels, [2, 2, 2]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();

            assert_eq!(encoded.len(), 12);

            let decoder = UncompressedBlockDecoder::from_raw_params(
                OwnedBuffer::from_vec(encoded),
                2,
                2,
                1,
                1,
                2,
                2,
                2,
                12,
                12,
                PixelValueType::UnsignedInt,
                PixelJustification::Right,
                InterleaveMode::S,
                "NC".to_string(),
            );
            let (decoded, shape) = decoder.decode_block(0, 0, 0, None).unwrap();
            assert_eq!(shape, [2, 2, 2]);
            assert_eq!(decoded, pixels);
        }

        #[test]
        fn encode_12bpp_buffer_size() {
            // 4x4 block, 1 band, 12 bpp
            // packed_band_size = ceil(16*12/8) = 24 bytes
            let encoder = UncompressedBlockEncoder::new(4, 4, 1, 12, InterleaveMode::B, 4, 4);
            assert_eq!(encoder.block_grid_size(), (1, 1));

            let mut encoder = UncompressedBlockEncoder::new(4, 4, 1, 12, InterleaveMode::B, 4, 4);
            let pixels = vec![0u8; 4 * 4 * 2]; // 16 pixels * 2 bytes/pixel
            encoder.encode_block(0, 0, &pixels, [1, 4, 4]).unwrap();
            let encoded = Box::new(encoder).finalize().unwrap();
            assert_eq!(encoded.len(), 24);
        }
    }
}
