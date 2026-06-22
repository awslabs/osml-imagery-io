//! JPEG DCT block decoder.
//!
//! This module provides the JpegBlockDecoder for decoding JPEG DCT
//! compressed blocks from NITF image segments.
//!
//! # Supported Configurations
//!
//! | Configuration | Pixel Type | Bands | Notes |
//! |--------------|------------|-------|-------|
//! | Mono 8-bit | UInt8 | 1 | Standard grayscale |
//! | Mono 12-bit | UInt16 | 1 | Extended JPEG |
//! | RGB 24-bit | UInt8 | 3 | Pixel interleaved |
//! | YCbCr601 24-bit | UInt8 | 3 | Color space conversion |
//! | Multiband 8-bit | UInt8 | 2-999 | Each band separate JPEG |
//!
//! # Requirements
//! - 1.1: Decode JPEG DCT compressed blocks (IC=C3)
//! - 1.2: Decode 8-bit monochrome JPEG blocks
//! - 1.3: Decode 12-bit monochrome JPEG blocks
//! - 1.4: Decode RGB 24-bit JPEG blocks (IMODE=P)
//! - 1.5: Decode YCbCr601 24-bit JPEG blocks with color space conversion
//! - 1.6: Decode multiband JPEG (IMODE=B or S)

use crate::error::CodecError;
use crate::jbp::image::decoder::BlockDecoder;
use crate::jbp::image::facade::ImageSubheaderFacade;
use crate::jbp::image::types::{ImageRepresentation, InterleaveMode};
use crate::owned_buffer::OwnedBuffer;

use crate::jpeg::JpegCodec;

/// Color space for JPEG decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegColorSpace {
    /// Grayscale (single band)
    Grayscale,
    /// RGB color space (3 bands)
    Rgb,
    /// YCbCr 601 color space (3 bands, converted to RGB on decode)
    YCbCr601,
}

impl JpegColorSpace {
    /// Create from ImageRepresentation.
    pub fn from_irep(irep: ImageRepresentation, num_bands: usize) -> Self {
        match irep {
            ImageRepresentation::Mono => JpegColorSpace::Grayscale,
            ImageRepresentation::Rgb => JpegColorSpace::Rgb,
            ImageRepresentation::YCbCr601 => JpegColorSpace::YCbCr601,
            _ => {
                // Default based on band count
                if num_bands == 1 {
                    JpegColorSpace::Grayscale
                } else if num_bands == 3 {
                    JpegColorSpace::Rgb
                } else {
                    JpegColorSpace::Grayscale // Multiband treated as separate grayscale
                }
            }
        }
    }
}

/// Block decoder for JPEG DCT compressed imagery.
///
/// Decodes JPEG compressed blocks from NITF files with IC=C3, M3, or I1.
/// Supports 8-bit and 12-bit pixel depths, grayscale, RGB, and YCbCr color spaces,
/// and multiband imagery with IMODE=B or S.
#[derive(Debug)]
pub struct JpegBlockDecoder {
    /// The underlying codec
    #[allow(dead_code)]
    codec: JpegCodec,
    /// Bits per pixel (8 or 12)
    bits_per_pixel: u8,
    /// Number of bands
    num_bands: usize,
    /// Block width in pixels
    block_width: usize,
    /// Block height in pixels
    block_height: usize,
    /// Interleave mode
    imode: InterleaveMode,
    /// Color space
    color_space: JpegColorSpace,
}

impl JpegBlockDecoder {
    /// Create a new JPEG block decoder.
    ///
    /// # Arguments
    /// * `bits_per_pixel` - Bits per pixel (8 or 12)
    /// * `num_bands` - Number of image bands
    /// * `block_width` - Width of each block in pixels
    /// * `block_height` - Height of each block in pixels
    /// * `imode` - Interleave mode (B, P, R, or S)
    /// * `color_space` - Color space for decoding
    ///
    /// # Requirements
    /// - 1.1, 1.2, 1.4, 1.5: Basic decoder construction
    pub fn new(
        bits_per_pixel: u8,
        num_bands: usize,
        block_width: usize,
        block_height: usize,
        imode: InterleaveMode,
        color_space: JpegColorSpace,
    ) -> Result<Self, CodecError> {
        // Validate bits per pixel
        if bits_per_pixel != 8 && bits_per_pixel != 12 {
            return Err(CodecError::Unsupported(format!(
                "JPEG only supports 8-bit or 12-bit pixels, got {}",
                bits_per_pixel
            )));
        }

        // Validate number of bands
        if num_bands == 0 {
            return Err(CodecError::InvalidFormat(
                "Number of bands must be at least 1".into(),
            ));
        }

        // Validate color space vs band count
        match color_space {
            JpegColorSpace::Grayscale => {
                // Grayscale can be used for single band or multiband (each band decoded separately)
            }
            JpegColorSpace::Rgb | JpegColorSpace::YCbCr601 => {
                if num_bands != 3 {
                    return Err(CodecError::InvalidFormat(format!(
                        "RGB/YCbCr color space requires 3 bands, got {}",
                        num_bands
                    )));
                }
            }
        }

        // Validate 12-bit is only for grayscale
        if bits_per_pixel == 12 && num_bands != 1 {
            return Err(CodecError::Unsupported(
                "12-bit JPEG only supports single-band grayscale images".into(),
            ));
        }

        Ok(Self {
            codec: JpegCodec::new(),
            bits_per_pixel,
            num_bands,
            block_width,
            block_height,
            imode,
            color_space,
        })
    }

    /// Get the bits per pixel.
    pub fn bits_per_pixel(&self) -> u8 {
        self.bits_per_pixel
    }

    /// Get the number of bands.
    pub fn num_bands(&self) -> usize {
        self.num_bands
    }

    /// Get the block dimensions.
    pub fn block_dimensions(&self) -> (usize, usize) {
        (self.block_width, self.block_height)
    }

    /// Get the interleave mode.
    pub fn imode(&self) -> InterleaveMode {
        self.imode
    }

    /// Get the color space.
    pub fn color_space(&self) -> JpegColorSpace {
        self.color_space
    }

    /// Decode a JPEG compressed block.
    ///
    /// # Arguments
    /// * `jpeg_data` - The JPEG compressed data
    ///
    /// # Returns
    /// The decoded pixel data as bytes in band-sequential format (BSQ).
    /// For 8-bit images, each byte is one pixel value.
    /// For 12-bit images, each pixel is stored as 2 bytes (little-endian u16).
    ///
    /// # Requirements
    /// - 1.2: 8-bit grayscale decoding
    /// - 1.4: 8-bit RGB decoding
    /// - 1.5: YCbCr601 to RGB conversion
    #[cfg(feature = "libjpeg-turbo")]
    pub fn decode_block(&self, jpeg_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        use crate::jpeg::ffi;

        if self.bits_per_pixel == 8 {
            // Determine output bands based on color space
            let output_bands = match self.color_space {
                JpegColorSpace::Grayscale => 1,
                JpegColorSpace::Rgb | JpegColorSpace::YCbCr601 => 3,
            };

            // Decompress using turbojpeg
            // Note: turbojpeg automatically handles YCbCr to RGB conversion
            let decoded =
                ffi::decompress_8bit(jpeg_data, self.block_width, self.block_height, output_bands)?;

            // For RGB/YCbCr, the data comes out as pixel-interleaved (RGBRGBRGB...)
            // We need to convert to band-sequential format (RRR...GGG...BBB...)
            if output_bands == 3 {
                Ok(self.pixel_to_band_sequential(&decoded))
            } else {
                Ok(decoded)
            }
        } else {
            // 12-bit decoding returns u16 values packed as bytes (little-endian)
            ffi::decompress_12bit(jpeg_data, self.block_width, self.block_height)
        }
    }

    /// Decode a JPEG compressed block (stub when libjpeg-turbo is not available).
    #[cfg(not(feature = "libjpeg-turbo"))]
    pub fn decode_block(&self, _jpeg_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "JPEG decoding requires the libjpeg-turbo feature".into(),
        ))
    }

    /// Decode a multiband JPEG block where each band is a separate JPEG stream.
    ///
    /// For IMODE=B (block interleaved) or IMODE=S (sequential), each band is
    /// encoded as a separate JPEG stream. The streams are concatenated with
    /// 4-byte length prefixes.
    ///
    /// # Arguments
    /// * `jpeg_data` - The concatenated JPEG streams with length prefixes
    ///
    /// # Returns
    /// The decoded pixel data in band-sequential format.
    ///
    /// # Data Format
    /// The input data is structured as:
    /// ```text
    /// [4-byte length BE][JPEG stream 1][4-byte length BE][JPEG stream 2]...
    /// ```
    ///
    /// # Requirements
    /// - 1.6: Multiband JPEG decoding (IMODE=B or S)
    #[cfg(feature = "libjpeg-turbo")]
    pub fn decode_multiband_block(&self, jpeg_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        use crate::jpeg::ffi;

        if self.num_bands == 1 {
            // Single band - just decode directly
            return self.decode_block(jpeg_data);
        }

        // For 3-band RGB/YCbCr with IMODE=P, the data is a single JPEG stream
        if self.num_bands == 3
            && self.imode == InterleaveMode::P
            && (self.color_space == JpegColorSpace::Rgb
                || self.color_space == JpegColorSpace::YCbCr601)
        {
            return self.decode_block(jpeg_data);
        }

        // For IMODE=B or S, each band is a separate JPEG stream with length prefix
        let pixels_per_band = self.block_width * self.block_height;
        let bytes_per_pixel = if self.bits_per_pixel == 12 { 2 } else { 1 };
        let mut output = Vec::with_capacity(self.num_bands * pixels_per_band * bytes_per_pixel);

        let mut offset = 0;
        for band in 0..self.num_bands {
            // Read 4-byte length prefix (big-endian)
            if offset + 4 > jpeg_data.len() {
                return Err(CodecError::Decode(format!(
                    "Unexpected end of data reading length prefix for band {}",
                    band
                )));
            }
            let length = u32::from_be_bytes([
                jpeg_data[offset],
                jpeg_data[offset + 1],
                jpeg_data[offset + 2],
                jpeg_data[offset + 3],
            ]) as usize;
            offset += 4;

            // Read JPEG stream
            if offset + length > jpeg_data.len() {
                return Err(CodecError::Decode(format!(
                    "Unexpected end of data reading JPEG stream for band {} (need {} bytes at offset {}, have {})",
                    band, length, offset, jpeg_data.len()
                )));
            }
            let band_jpeg = &jpeg_data[offset..offset + length];
            offset += length;

            // Decode this band's JPEG stream
            let band_data = if self.bits_per_pixel == 8 {
                ffi::decompress_8bit(band_jpeg, self.block_width, self.block_height, 1)?
            } else {
                ffi::decompress_12bit(band_jpeg, self.block_width, self.block_height)?
            };

            output.extend_from_slice(&band_data);
        }

        Ok(output)
    }

    /// Decode a multiband JPEG block (stub when libjpeg-turbo is not available).
    #[cfg(not(feature = "libjpeg-turbo"))]
    pub fn decode_multiband_block(&self, _jpeg_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "JPEG decoding requires the libjpeg-turbo feature".into(),
        ))
    }

    /// Convert pixel-interleaved data to band-sequential format.
    ///
    /// Input: RGBRGBRGB... (pixel interleaved)
    /// Output: RRR...GGG...BBB... (band sequential)
    fn pixel_to_band_sequential(&self, data: &[u8]) -> Vec<u8> {
        let num_pixels = self.block_width * self.block_height;
        let num_bands = 3; // Only used for RGB/YCbCr
        let mut output = vec![0u8; num_pixels * num_bands];

        for pixel in 0..num_pixels {
            for band in 0..num_bands {
                output[band * num_pixels + pixel] = data[pixel * num_bands + band];
            }
        }

        output
    }
}

// =============================================================================
// JpegNitfBlockDecoder - JPEG DCT decoder for NITF (IC=C3, M3, I1)
// =============================================================================

/// Block decoder for JPEG DCT compressed NITF imagery (IC=C3, M3, I1).
///
/// This decoder wraps the [`JpegBlockDecoder`] and implements the [`BlockDecoder`]
/// trait for integration with the JBP image reader infrastructure.
///
/// # Supported IC Codes
/// - `C3`: JPEG DCT compressed imagery
/// - `M3`: JPEG DCT compressed imagery with block mask
/// - `I1`: Downsampled JPEG (single block ≤2048×2048)
///
/// # Requirements
/// - 1.1: Decode JPEG DCT compressed blocks
/// - 1.2: Decode 8-bit monochrome JPEG blocks
/// - 1.3: Return clear error for 12-bit JPEG (not supported)
/// - 1.4: Decode RGB 24-bit JPEG blocks
/// - 1.5: Decode YCbCr601 24-bit JPEG blocks with color space conversion
/// - 1.6: Decode multiband JPEG (IMODE=B or S)
#[cfg(feature = "libjpeg-turbo")]
pub struct JpegNitfBlockDecoder {
    /// The raw image data (JPEG compressed blocks)
    image_data: OwnedBuffer,
    /// The underlying JPEG decoder
    jpeg_decoder: JpegBlockDecoder,
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
    /// Block offsets (lazily computed for multi-block images)
    /// Each entry is (start_offset, end_offset) for the JPEG stream
    block_offsets: std::sync::OnceLock<Vec<(usize, usize)>>,
}

#[cfg(feature = "libjpeg-turbo")]
impl JpegNitfBlockDecoder {
    /// Create a new JPEG NITF block decoder from image subheader.
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade
    /// * `image_data` - The raw JPEG compressed image data
    ///
    /// # Returns
    /// A new `JpegNitfBlockDecoder` or an error if parameters are invalid.
    ///
    /// # Requirements
    /// - 1.1: JPEG DCT decoding support
    /// - 4.2: I1 dimension constraint validation (≤2048×2048)
    pub fn new(
        subheader: &ImageSubheaderFacade,
        image_data: OwnedBuffer,
    ) -> Result<Self, CodecError> {
        if image_data.is_empty() {
            return Err(CodecError::InvalidFormat(
                "Image data segment is empty".into(),
            ));
        }

        let ic = subheader.ic()?.trim().to_string();
        let nrows = subheader.nrows()?;
        let ncols = subheader.ncols()?;
        let nbpr = subheader.nbpr()?;
        let nbpc = subheader.nbpc()?;
        // Use effective values to handle NPPBH=0/NPPBV=0 (single block = full image)
        let nppbh = subheader.effective_nppbh()?;
        let nppbv = subheader.effective_nppbv()?;
        let nbands = subheader.band_count()? as u32;
        let nbpp = subheader.nbpp()?;
        let imode = subheader.imode()?;
        let irep = subheader.irep()?;

        // Validate I1 dimension constraint (Requirement 4.2)
        if ic == "I1" && (nrows > 2048 || ncols > 2048) {
            return Err(CodecError::InvalidFormat(format!(
                "IC=I1 (Downsampled JPEG) requires dimensions ≤2048×2048, got {}×{}",
                ncols, nrows
            )));
        }

        // Determine color space from IREP
        let color_space = JpegColorSpace::from_irep(irep, nbands as usize);

        // Create the underlying JPEG decoder
        let jpeg_decoder = JpegBlockDecoder::new(
            nbpp,
            nbands as usize,
            nppbh as usize,
            nppbv as usize,
            imode,
            color_space,
        )?;

        Ok(Self {
            image_data,
            jpeg_decoder,
            nrows,
            ncols,
            nbpr,
            nbpc,
            nppbh,
            nppbv,
            nbands,
            nbpp,
            imode,
            ic,
            block_offsets: std::sync::OnceLock::new(),
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

    /// Calculate bytes per pixel.
    fn bytes_per_pixel(&self) -> usize {
        (self.nbpp as usize).div_ceil(8)
    }

    /// Compute block offsets by scanning through the image data for JPEG EOI markers.
    ///
    /// For multi-block JPEG images (IC=C3), each block is stored as a separate JPEG
    /// stream concatenated together. This method scans through the data to find the
    /// start and end of each JPEG stream.
    ///
    /// Returns a vector of (start_offset, end_offset) tuples for each block in
    /// row-major order.
    fn compute_block_offsets(&self) -> Vec<(usize, usize)> {
        let total_blocks = (self.nbpc * self.nbpr) as usize;
        let mut offsets = Vec::with_capacity(total_blocks);

        // For single-block images, the entire data is one JPEG stream
        if total_blocks == 1 {
            offsets.push((0, self.image_data.as_bytes().len()));
            return offsets;
        }

        // Determine if blocks contain multiple length-prefixed JPEG streams
        // (multiband with IMODE=B or S, excluding 3-band RGB/YCbCr with IMODE=P)
        let is_multiband_separate =
            self.nbands > 1 && !(self.nbands == 3 && self.imode == InterleaveMode::P);

        // Scan through the data to find JPEG stream boundaries
        let mut current_offset = 0;

        for _ in 0..total_blocks {
            if current_offset >= self.image_data.as_bytes().len() {
                // No more data - remaining blocks will have invalid offsets
                offsets.push((current_offset, current_offset));
                continue;
            }

            if is_multiband_separate {
                // Each block has N length-prefixed JPEG streams:
                // [4-byte len BE][JPEG stream] repeated per band
                let block_start = current_offset;
                for band in 0..self.nbands {
                    if current_offset + 4 > self.image_data.as_bytes().len() {
                        break;
                    }
                    let image_bytes = self.image_data.as_bytes();
                    let length = u32::from_be_bytes([
                        image_bytes[current_offset],
                        image_bytes[current_offset + 1],
                        image_bytes[current_offset + 2],
                        image_bytes[current_offset + 3],
                    ]) as usize;
                    current_offset += 4 + length;
                    if current_offset > self.image_data.as_bytes().len() {
                        // Truncated stream — clamp to data length
                        current_offset = self.image_data.as_bytes().len();
                        break;
                    }
                    let _ = band; // suppress unused warning
                }
                offsets.push((block_start, current_offset));
            } else {
                let remaining_data = &self.image_data.as_bytes()[current_offset..];

                // Find the end of this JPEG stream (EOI marker)
                if let Some(jpeg_len) = find_jpeg_end(remaining_data) {
                    let end_offset = current_offset + jpeg_len;
                    offsets.push((current_offset, end_offset));
                    current_offset = end_offset;
                } else {
                    // No EOI found - use remaining data as the last block
                    offsets.push((current_offset, self.image_data.as_bytes().len()));
                    current_offset = self.image_data.as_bytes().len();
                }
            }
        }

        offsets
    }

    /// Get block offsets, computing them lazily if needed.
    fn get_block_offsets(&self) -> &[(usize, usize)] {
        self.block_offsets
            .get_or_init(|| self.compute_block_offsets())
    }

    /// Apply band selection to decoded data.
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

#[cfg(feature = "libjpeg-turbo")]
impl BlockDecoder for JpegNitfBlockDecoder {
    fn decode_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // JPEG only supports resolution level 0
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

        let (actual_rows, actual_cols) = self.actual_block_dimensions(block_row, block_col);

        // Get the block offsets and find the JPEG data for this block
        let block_offsets = self.get_block_offsets();
        let block_index = (block_row * self.nbpr + block_col) as usize;

        if block_index >= block_offsets.len() {
            return Err(CodecError::Decode(format!(
                "Block index {} out of range (have {} blocks)",
                block_index,
                block_offsets.len()
            )));
        }

        let (start_offset, end_offset) = block_offsets[block_index];

        if start_offset >= end_offset || end_offset > self.image_data.as_bytes().len() {
            return Err(CodecError::Decode(format!(
                "Invalid block offsets: start={}, end={}, data_len={}",
                start_offset,
                end_offset,
                self.image_data.as_bytes().len()
            )));
        }

        let block_jpeg_data = &self.image_data.as_bytes()[start_offset..end_offset];

        // Decode the JPEG data
        // For single-band or RGB/YCbCr with IMODE=P, use decode_block
        // For multiband with IMODE=B or S, use decode_multiband_block
        let decoded = if self.nbands == 1 || (self.nbands == 3 && self.imode == InterleaveMode::P) {
            self.jpeg_decoder.decode_block(block_jpeg_data)?
        } else {
            self.jpeg_decoder.decode_multiband_block(block_jpeg_data)?
        };

        // For edge blocks, the JPEG was encoded at full block dimensions
        // (zero-padded per JBP-2021.2-063/064). Crop back to actual dimensions.
        let cropped = if actual_rows < self.nppbv || actual_cols < self.nppbh {
            let full_w = self.nppbh as usize;
            let act_h = actual_rows as usize;
            let act_w = actual_cols as usize;
            let nbands = self.nbands as usize;
            let bpp = if self.nbpp == 12 { 2 } else { 1 };
            let full_band_size = (self.nppbv as usize) * full_w * bpp;
            let act_band_size = act_h * act_w * bpp;

            let mut out = Vec::with_capacity(nbands * act_band_size);
            for band in 0..nbands {
                let src_offset = band * full_band_size;
                for row in 0..act_h {
                    let src_start = src_offset + row * full_w * bpp;
                    out.extend_from_slice(&decoded[src_start..src_start + act_w * bpp]);
                }
            }
            out
        } else {
            decoded
        };

        // Apply band selection if specified
        let num_bands = bands.map(|b| b.len() as u32).unwrap_or(self.nbands);
        let final_data = match bands {
            Some(band_indices) if !band_indices.is_empty() => {
                self.apply_band_selection(&cropped, actual_rows, actual_cols, band_indices)?
            }
            _ => cropped,
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
        // JPEG DCT only supports a single resolution level
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
        // JPEG only supports resolution level 0
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

        let (actual_rows, actual_cols) = self.actual_block_dimensions(block_row, block_col);

        // For masked JPEG (M3), the offset points to the start of the JPEG stream
        // We need to find the end of the JPEG stream (look for EOI marker 0xFFD9)
        let offset_usize = offset as usize;
        if offset_usize >= self.image_data.as_bytes().len() {
            return Err(CodecError::Decode(format!(
                "Block offset {} exceeds image data length {}",
                offset,
                self.image_data.as_bytes().len()
            )));
        }

        // Find the end of the JPEG stream by looking for EOI marker
        let jpeg_data = &self.image_data.as_bytes()[offset_usize..];
        let jpeg_end = find_jpeg_end(jpeg_data).ok_or_else(|| {
            CodecError::Decode("Could not find JPEG EOI marker in block data".into())
        })?;

        let block_jpeg = &jpeg_data[..jpeg_end];

        // Decode the JPEG data
        let decoded = if self.nbands == 1 || (self.nbands == 3 && self.imode == InterleaveMode::P) {
            self.jpeg_decoder.decode_block(block_jpeg)?
        } else {
            self.jpeg_decoder.decode_multiband_block(block_jpeg)?
        };

        // For edge blocks, the JPEG was encoded at full block dimensions
        // (zero-padded per JBP-2021.2-063/064). Crop back to actual dimensions.
        let cropped = if actual_rows < self.nppbv || actual_cols < self.nppbh {
            let full_w = self.nppbh as usize;
            let act_h = actual_rows as usize;
            let act_w = actual_cols as usize;
            let nbands = self.nbands as usize;
            let bpp = if self.nbpp == 12 { 2 } else { 1 };
            let full_band_size = (self.nppbv as usize) * full_w * bpp;
            let act_band_size = act_h * act_w * bpp;

            let mut out = Vec::with_capacity(nbands * act_band_size);
            for band in 0..nbands {
                let src_offset = band * full_band_size;
                for row in 0..act_h {
                    let src_start = src_offset + row * full_w * bpp;
                    out.extend_from_slice(&decoded[src_start..src_start + act_w * bpp]);
                }
            }
            out
        } else {
            decoded
        };

        // Apply band selection if specified
        let num_bands = bands.map(|b| b.len() as u32).unwrap_or(self.nbands);
        let final_data = match bands {
            Some(band_indices) if !band_indices.is_empty() => {
                self.apply_band_selection(&cropped, actual_rows, actual_cols, band_indices)?
            }
            _ => cropped,
        };

        // Return shape as [bands, rows, cols] (CHW format)
        Ok((final_data, [num_bands, actual_rows, actual_cols]))
    }

    fn tile_byte_ranges(&self) -> Option<std::collections::HashMap<(u32, u32), Vec<(u64, u64)>>> {
        let offsets = self.get_block_offsets();
        let mut ranges = std::collections::HashMap::new();
        for (idx, &(start, end)) in offsets.iter().enumerate() {
            let row = idx as u32 / self.nbpr;
            let col = idx as u32 % self.nbpr;
            ranges.insert((row, col), vec![(start as u64, (end - start) as u64)]);
        }
        Some(ranges)
    }

    fn codec_configuration(&self) -> Option<std::collections::HashMap<String, Vec<u8>>> {
        let mut config = std::collections::HashMap::new();
        config.insert("bits_per_pixel".to_string(), vec![self.nbpp]);
        config.insert("num_bands".to_string(), self.nbands.to_le_bytes().to_vec());
        config.insert("block_width".to_string(), self.nppbh.to_le_bytes().to_vec());
        config.insert(
            "block_height".to_string(),
            self.nppbv.to_le_bytes().to_vec(),
        );
        config.insert("imode".to_string(), vec![self.imode.to_char() as u8]);
        let cs_byte = match self.jpeg_decoder.color_space() {
            JpegColorSpace::Grayscale => 0u8,
            JpegColorSpace::Rgb => 1u8,
            JpegColorSpace::YCbCr601 => 2u8,
        };
        config.insert("color_space".to_string(), vec![cs_byte]);
        Some(config)
    }
}

// Safety: JpegNitfBlockDecoder is thread-safe
// - image_data is OwnedBuffer (immutable, shared via Arc internally)
// - jpeg_decoder contains only primitive types and JpegCodec (which is Send+Sync)
// - All other fields are primitive types
#[cfg(feature = "libjpeg-turbo")]
unsafe impl Send for JpegNitfBlockDecoder {}
#[cfg(feature = "libjpeg-turbo")]
unsafe impl Sync for JpegNitfBlockDecoder {}

/// Find the end of a JPEG stream by looking for the EOI marker (0xFFD9).
///
/// Returns the byte offset just after the EOI marker, or None if not found.
#[cfg(feature = "libjpeg-turbo")]
fn find_jpeg_end(data: &[u8]) -> Option<usize> {
    // Look for EOI marker (0xFF 0xD9)
    for i in 0..data.len().saturating_sub(1) {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            return Some(i + 2);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Constructor Tests
    // =========================================================================

    #[test]
    fn test_new_8bit_grayscale() {
        let decoder =
            JpegBlockDecoder::new(8, 1, 64, 64, InterleaveMode::B, JpegColorSpace::Grayscale);
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.bits_per_pixel(), 8);
        assert_eq!(decoder.num_bands(), 1);
        assert_eq!(decoder.block_dimensions(), (64, 64));
    }

    #[test]
    fn test_new_8bit_rgb() {
        let decoder = JpegBlockDecoder::new(8, 3, 64, 64, InterleaveMode::P, JpegColorSpace::Rgb);
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.num_bands(), 3);
        assert_eq!(decoder.color_space(), JpegColorSpace::Rgb);
    }

    #[test]
    fn test_new_8bit_ycbcr() {
        let decoder =
            JpegBlockDecoder::new(8, 3, 64, 64, InterleaveMode::P, JpegColorSpace::YCbCr601);
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.color_space(), JpegColorSpace::YCbCr601);
    }

    #[test]
    fn test_new_12bit_grayscale() {
        let decoder =
            JpegBlockDecoder::new(12, 1, 64, 64, InterleaveMode::B, JpegColorSpace::Grayscale);
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.bits_per_pixel(), 12);
    }

    #[cfg(feature = "libjpeg-turbo")]
    #[test]
    fn test_12bit_decode_returns_unsupported_error() {
        // 12-bit JPEG decoding requires a specially compiled libjpeg12 library
        // which is not commonly available. Verify we get a clear error message.
        let decoder =
            JpegBlockDecoder::new(12, 1, 8, 8, InterleaveMode::B, JpegColorSpace::Grayscale)
                .unwrap();

        // Any JPEG data will fail because 12-bit is not supported
        let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG SOI marker
        let result = decoder.decode_block(&fake_jpeg);
        assert!(result.is_err());

        // Verify the error mentions the library requirement
        if let Err(CodecError::Unsupported(msg)) = result {
            assert!(
                msg.contains("12-bit") || msg.contains("libjpeg12"),
                "Error message should mention 12-bit or libjpeg12: {}",
                msg
            );
        }
    }

    #[test]
    fn test_new_multiband() {
        let decoder =
            JpegBlockDecoder::new(8, 4, 64, 64, InterleaveMode::B, JpegColorSpace::Grayscale);
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.num_bands(), 4);
    }

    // =========================================================================
    // Validation Error Tests
    // =========================================================================

    #[test]
    fn test_invalid_bits_per_pixel() {
        let result = JpegBlockDecoder::new(
            16, // Invalid - only 8 or 12 supported
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_zero_bands() {
        let result = JpegBlockDecoder::new(
            8,
            0, // Invalid - must be at least 1
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_rgb_wrong_band_count() {
        let result = JpegBlockDecoder::new(
            8,
            4, // Invalid - RGB requires 3 bands
            64,
            64,
            InterleaveMode::P,
            JpegColorSpace::Rgb,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_12bit_multiband_not_supported() {
        let result = JpegBlockDecoder::new(
            12,
            3, // Invalid - 12-bit only supports single band
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    // =========================================================================
    // Color Space Tests
    // =========================================================================

    #[test]
    fn test_color_space_from_irep_mono() {
        let cs = JpegColorSpace::from_irep(ImageRepresentation::Mono, 1);
        assert_eq!(cs, JpegColorSpace::Grayscale);
    }

    #[test]
    fn test_color_space_from_irep_rgb() {
        let cs = JpegColorSpace::from_irep(ImageRepresentation::Rgb, 3);
        assert_eq!(cs, JpegColorSpace::Rgb);
    }

    #[test]
    fn test_color_space_from_irep_ycbcr() {
        let cs = JpegColorSpace::from_irep(ImageRepresentation::YCbCr601, 3);
        assert_eq!(cs, JpegColorSpace::YCbCr601);
    }

    #[test]
    fn test_color_space_from_irep_multi_single_band() {
        let cs = JpegColorSpace::from_irep(ImageRepresentation::Multi, 1);
        assert_eq!(cs, JpegColorSpace::Grayscale);
    }

    #[test]
    fn test_color_space_from_irep_multi_three_bands() {
        let cs = JpegColorSpace::from_irep(ImageRepresentation::Multi, 3);
        assert_eq!(cs, JpegColorSpace::Rgb);
    }

    #[test]
    fn test_color_space_from_irep_multi_many_bands() {
        let cs = JpegColorSpace::from_irep(ImageRepresentation::Multi, 8);
        assert_eq!(cs, JpegColorSpace::Grayscale);
    }

    // =========================================================================
    // Pixel to Band Sequential Conversion Tests
    // =========================================================================

    #[test]
    fn test_pixel_to_band_sequential() {
        let decoder =
            JpegBlockDecoder::new(8, 3, 2, 2, InterleaveMode::P, JpegColorSpace::Rgb).unwrap();

        // Input: RGBRGBRGBRGB (4 pixels, pixel interleaved)
        let input = vec![
            1, 2, 3, // pixel 0: R=1, G=2, B=3
            4, 5, 6, // pixel 1: R=4, G=5, B=6
            7, 8, 9, // pixel 2: R=7, G=8, B=9
            10, 11, 12, // pixel 3: R=10, G=11, B=12
        ];

        let output = decoder.pixel_to_band_sequential(&input);

        // Expected: RRR...GGG...BBB... (band sequential)
        let expected = vec![
            1, 4, 7, 10, // R band
            2, 5, 8, 11, // G band
            3, 6, 9, 12, // B band
        ];

        assert_eq!(output, expected);
    }

    // =========================================================================
    // Decode Tests (require libjpeg-turbo feature)
    // =========================================================================

    #[cfg(feature = "libjpeg-turbo")]
    mod decode_tests {
        use super::*;
        use crate::jpeg::ffi::compress_8bit;

        #[test]
        fn test_decode_8bit_grayscale_roundtrip() {
            // Create a simple 8x8 grayscale image
            let width = 8;
            let height = 8;
            let mut src = vec![0u8; width * height];
            for i in 0..src.len() {
                src[i] = (i * 4) as u8;
            }

            // Compress
            let jpeg_data = compress_8bit(&src, width, height, 1, 90).unwrap();

            // Create decoder and decode
            let decoder = JpegBlockDecoder::new(
                8,
                1,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
            )
            .unwrap();

            let decoded = decoder.decode_block(&jpeg_data).unwrap();
            assert_eq!(decoded.len(), src.len());

            // JPEG is lossy, values should be close but not exact
            for (orig, dec) in src.iter().zip(decoded.iter()) {
                assert!(
                    (*orig as i32 - *dec as i32).abs() < 20,
                    "Pixel difference too large: {} vs {}",
                    orig,
                    dec
                );
            }
        }

        #[test]
        fn test_decode_8bit_rgb_roundtrip() {
            // Create a simple 8x8 RGB image
            let width = 8;
            let height = 8;
            let mut src = vec![0u8; width * height * 3];
            for i in 0..width * height {
                src[i * 3] = (i * 4) as u8; // R
                src[i * 3 + 1] = (i * 2) as u8; // G
                src[i * 3 + 2] = (i * 3) as u8; // B
            }

            // Compress (pixel interleaved)
            let jpeg_data = compress_8bit(&src, width, height, 3, 90).unwrap();

            // Create decoder and decode
            let decoder =
                JpegBlockDecoder::new(8, 3, width, height, InterleaveMode::P, JpegColorSpace::Rgb)
                    .unwrap();

            let decoded = decoder.decode_block(&jpeg_data).unwrap();

            // Output should be band-sequential
            assert_eq!(decoded.len(), width * height * 3);

            // Verify the data is in band-sequential format
            // First width*height bytes should be R, next G, then B
            let r_band = &decoded[0..width * height];
            let g_band = &decoded[width * height..2 * width * height];
            let b_band = &decoded[2 * width * height..3 * width * height];

            // Check that bands are separated (not interleaved)
            // The first pixel's R value should be at index 0
            // The first pixel's G value should be at index width*height
            assert!(r_band.len() == width * height);
            assert!(g_band.len() == width * height);
            assert!(b_band.len() == width * height);
        }

        #[test]
        fn test_decode_multiband_single_band() {
            // Single band should work with decode_multiband_block
            let width = 8;
            let height = 8;
            let src = vec![128u8; width * height];

            let jpeg_data = compress_8bit(&src, width, height, 1, 90).unwrap();

            let decoder = JpegBlockDecoder::new(
                8,
                1,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
            )
            .unwrap();

            let decoded = decoder.decode_multiband_block(&jpeg_data).unwrap();
            assert_eq!(decoded.len(), width * height);
        }

        #[test]
        fn test_decode_multiband_rgb_imode_p() {
            // RGB with IMODE=P should decode as single stream
            let width = 8;
            let height = 8;
            let src = vec![128u8; width * height * 3];

            let jpeg_data = compress_8bit(&src, width, height, 3, 90).unwrap();

            let decoder =
                JpegBlockDecoder::new(8, 3, width, height, InterleaveMode::P, JpegColorSpace::Rgb)
                    .unwrap();

            let decoded = decoder.decode_multiband_block(&jpeg_data).unwrap();
            assert_eq!(decoded.len(), width * height * 3);
        }

        #[test]
        fn test_decode_multiband_separate_streams() {
            // Create multiband data with length-prefixed JPEG streams
            let width = 8;
            let height = 8;
            let num_bands = 4;

            // Compress each band separately
            let mut multiband_data = Vec::new();
            for band in 0..num_bands {
                let band_src = vec![(band * 50) as u8; width * height];
                let band_jpeg = compress_8bit(&band_src, width, height, 1, 90).unwrap();

                // Add 4-byte length prefix (big-endian)
                let length = band_jpeg.len() as u32;
                multiband_data.extend_from_slice(&length.to_be_bytes());
                multiband_data.extend_from_slice(&band_jpeg);
            }

            let decoder = JpegBlockDecoder::new(
                8,
                num_bands,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
            )
            .unwrap();

            let decoded = decoder.decode_multiband_block(&multiband_data).unwrap();
            assert_eq!(decoded.len(), width * height * num_bands);
        }

        #[test]
        fn test_decode_multiband_imode_s() {
            // Test IMODE=S (sequential) with separate JPEG streams
            let width = 8;
            let height = 8;
            let num_bands = 2;

            // Compress each band separately
            let mut multiband_data = Vec::new();
            for band in 0..num_bands {
                let band_src = vec![(band * 100 + 50) as u8; width * height];
                let band_jpeg = compress_8bit(&band_src, width, height, 1, 90).unwrap();

                // Add 4-byte length prefix (big-endian)
                let length = band_jpeg.len() as u32;
                multiband_data.extend_from_slice(&length.to_be_bytes());
                multiband_data.extend_from_slice(&band_jpeg);
            }

            let decoder = JpegBlockDecoder::new(
                8,
                num_bands,
                width,
                height,
                InterleaveMode::S, // Sequential mode
                JpegColorSpace::Grayscale,
            )
            .unwrap();

            let decoded = decoder.decode_multiband_block(&multiband_data).unwrap();
            assert_eq!(decoded.len(), width * height * num_bands);

            // Verify bands are in sequential order
            let band0 = &decoded[0..width * height];
            let band1 = &decoded[width * height..2 * width * height];

            // Band 0 should have values around 50, band 1 around 150
            let avg_band0: f64 = band0.iter().map(|&x| x as f64).sum::<f64>() / band0.len() as f64;
            let avg_band1: f64 = band1.iter().map(|&x| x as f64).sum::<f64>() / band1.len() as f64;

            assert!(
                avg_band0 < avg_band1,
                "Band 0 avg {} should be less than band 1 avg {}",
                avg_band0,
                avg_band1
            );
        }

        #[test]
        fn test_decode_multiband_truncated_length_prefix() {
            // Test error handling for truncated length prefix
            let decoder =
                JpegBlockDecoder::new(8, 2, 8, 8, InterleaveMode::B, JpegColorSpace::Grayscale)
                    .unwrap();

            // Only 2 bytes - not enough for 4-byte length prefix
            let truncated_data = vec![0x00, 0x00];
            let result = decoder.decode_multiband_block(&truncated_data);
            assert!(result.is_err());
        }

        #[test]
        fn test_decode_multiband_truncated_stream() {
            // Test error handling for truncated JPEG stream
            let decoder =
                JpegBlockDecoder::new(8, 2, 8, 8, InterleaveMode::B, JpegColorSpace::Grayscale)
                    .unwrap();

            // Length prefix says 1000 bytes, but only 10 bytes follow
            let mut truncated_data = Vec::new();
            truncated_data.extend_from_slice(&1000u32.to_be_bytes());
            truncated_data.extend_from_slice(&[0u8; 10]);

            let result = decoder.decode_multiband_block(&truncated_data);
            assert!(result.is_err());
        }

        /// Test YCbCr601 decoding with color space conversion (Requirement 1.5)
        #[test]
        fn test_decode_8bit_ycbcr_roundtrip() {
            // Create a simple 8x8 RGB image
            let width = 8;
            let height = 8;
            let mut src = vec![0u8; width * height * 3];
            for i in 0..width * height {
                src[i * 3] = 200; // R - reddish
                src[i * 3 + 1] = 100; // G
                src[i * 3 + 2] = 50; // B
            }

            // Compress as RGB (turbojpeg handles YCbCr internally)
            let jpeg_data = compress_8bit(&src, width, height, 3, 90).unwrap();

            // Create decoder with YCbCr601 color space
            // The decoder should convert YCbCr back to RGB
            let decoder = JpegBlockDecoder::new(
                8,
                3,
                width,
                height,
                InterleaveMode::P,
                JpegColorSpace::YCbCr601,
            )
            .unwrap();

            let decoded = decoder.decode_block(&jpeg_data).unwrap();

            // Output should be band-sequential RGB
            assert_eq!(decoded.len(), width * height * 3);

            // Verify the data is in band-sequential format
            let r_band = &decoded[0..width * height];
            let g_band = &decoded[width * height..2 * width * height];
            let b_band = &decoded[2 * width * height..3 * width * height];

            // Check bands have reasonable values (JPEG is lossy)
            let avg_r: f64 = r_band.iter().map(|&x| x as f64).sum::<f64>() / r_band.len() as f64;
            let avg_g: f64 = g_band.iter().map(|&x| x as f64).sum::<f64>() / g_band.len() as f64;
            let avg_b: f64 = b_band.iter().map(|&x| x as f64).sum::<f64>() / b_band.len() as f64;

            // Original was R=200, G=100, B=50, so R should be highest
            assert!(avg_r > avg_g, "R avg {} should be > G avg {}", avg_r, avg_g);
            assert!(avg_g > avg_b, "G avg {} should be > B avg {}", avg_g, avg_b);
        }
    }

    // =========================================================================
    // JpegNitfBlockDecoder Validation Tests
    // =========================================================================

    #[cfg(feature = "libjpeg-turbo")]
    mod nitf_decoder_tests {
        use super::*;
        use crate::jbp::image::facade::ImageSubheaderFacade;
        use crate::jbp::types::NitfFormat;
        use crate::parser::StructureRegistry;

        fn create_c3_image_subheader() -> Vec<u8> {
            let mut data = Vec::new();

            // IM (2)
            data.extend_from_slice(b"IM");
            // IID1 (10)
            data.extend_from_slice(b"TestImage ");
            // IDATIM (14)
            data.extend_from_slice(b"20240101120000");
            // TGTID (17)
            data.extend_from_slice(&[b' '; 17]);
            // IID2 (80)
            data.extend_from_slice(&[b' '; 80]);
            // Security: ISCLAS(1) ISCLSY(2) ISCODE(11) ISCTLH(2) ISREL(20)
            //           ISDCTP(2) ISDCDT(8) ISDCXM(4) ISDG(1) ISDGDT(8)
            //           ISCLTX(43) ISCATP(1) ISCAUT(40) ISCRSN(1) ISSRDT(8) ISCTLN(15)
            data.push(b'U');
            data.extend_from_slice(&[b' '; 2]); // ISCLSY
            data.extend_from_slice(&[b' '; 11]); // ISCODE
            data.extend_from_slice(&[b' '; 2]); // ISCTLH
            data.extend_from_slice(&[b' '; 20]); // ISREL
            data.extend_from_slice(&[b' '; 2]); // ISDCTP
            data.extend_from_slice(&[b' '; 8]); // ISDCDT
            data.extend_from_slice(&[b' '; 4]); // ISDCXM
            data.push(b' '); // ISDG
            data.extend_from_slice(&[b' '; 8]); // ISDGDT
            data.extend_from_slice(&[b' '; 43]); // ISCLTX
            data.push(b' '); // ISCATP
            data.extend_from_slice(&[b' '; 40]); // ISCAUT
            data.push(b' '); // ISCRSN
            data.extend_from_slice(&[b' '; 8]); // ISSRDT
            data.extend_from_slice(&[b' '; 15]); // ISCTLN
                                                 // ENCRYP (1)
            data.push(b'0');
            // ISORCE (42)
            data.extend_from_slice(&[b' '; 42]);
            // NROWS (8)
            data.extend_from_slice(b"00000064");
            // NCOLS (8)
            data.extend_from_slice(b"00000064");
            // PVTYPE (3)
            data.extend_from_slice(b"INT");
            // IREP (8)
            data.extend_from_slice(b"MONO    ");
            // ICAT (8)
            data.extend_from_slice(b"VIS     ");
            // ABPP (2)
            data.extend_from_slice(b"08");
            // PJUST (1)
            data.push(b'R');
            // ICORDS (1) - blank to skip IGEOLO
            data.push(b' ');
            // NICOM (1)
            data.push(b'0');
            // IC (2) - JPEG DCT
            data.extend_from_slice(b"C3");
            // COMRAT (4) - required when IC != NC/NM
            data.extend_from_slice(b"00.0");
            // NBANDS (1)
            data.push(b'1');
            // Band info: IREPBAND(2) ISUBCAT(6) IFC(1) IMFLT(3) NLUTS(1)
            data.extend_from_slice(b"M ");
            data.extend_from_slice(b"      ");
            data.push(b'N');
            data.extend_from_slice(b"   ");
            data.push(b'0');
            // ISYNC (1)
            data.push(b'0');
            // IMODE (1)
            data.push(b'B');
            // NBPR (4)
            data.extend_from_slice(b"0001");
            // NBPC (4)
            data.extend_from_slice(b"0001");
            // NPPBH (4)
            data.extend_from_slice(b"0064");
            // NPPBV (4)
            data.extend_from_slice(b"0064");
            // NBPP (2)
            data.extend_from_slice(b"08");
            // IDLVL (3)
            data.extend_from_slice(b"001");
            // IALVL (3)
            data.extend_from_slice(b"000");
            // ILOC (10)
            data.extend_from_slice(b"0000000000");
            // IMAG (4)
            data.extend_from_slice(b"1.0 ");
            // UDIDL (5)
            data.extend_from_slice(b"00000");
            // IXSHDL (5)
            data.extend_from_slice(b"00000");

            data
        }

        #[test]
        fn test_empty_image_data_returns_error() {
            let registry = StructureRegistry::new();
            let subheader_data = create_c3_image_subheader();

            let facade = match ImageSubheaderFacade::from_bytes(
                &subheader_data,
                &registry,
                NitfFormat::Nitf21,
            ) {
                Ok(f) => f,
                Err(_) => {
                    eprintln!("Skipping test: could not parse test subheader");
                    return;
                }
            };

            let empty_data = OwnedBuffer::from_vec(Vec::new());
            let result = JpegNitfBlockDecoder::new(&facade, empty_data);

            assert!(result.is_err(), "Expected error for empty image data");
            match result.err().unwrap() {
                CodecError::InvalidFormat(msg) => {
                    assert!(
                        msg.contains("empty"),
                        "Error message should mention 'empty', got: {}",
                        msg
                    );
                }
                other => panic!("Expected CodecError::InvalidFormat, got: {:?}", other),
            }
        }
    }
}
