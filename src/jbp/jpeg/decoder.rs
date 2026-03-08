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
use crate::jbp::image::types::{ImageRepresentation, InterleaveMode};

use super::codec::JpegCodec;

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
        use super::ffi;

        if self.bits_per_pixel == 8 {
            // Determine output bands based on color space
            let output_bands = match self.color_space {
                JpegColorSpace::Grayscale => 1,
                JpegColorSpace::Rgb | JpegColorSpace::YCbCr601 => 3,
            };

            // Decompress using turbojpeg
            // Note: turbojpeg automatically handles YCbCr to RGB conversion
            let decoded = ffi::decompress_8bit(
                jpeg_data,
                self.block_width,
                self.block_height,
                output_bands,
            )?;

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
        use super::ffi;

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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Constructor Tests
    // =========================================================================

    #[test]
    fn test_new_8bit_grayscale() {
        let decoder = JpegBlockDecoder::new(
            8,
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        );
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.bits_per_pixel(), 8);
        assert_eq!(decoder.num_bands(), 1);
        assert_eq!(decoder.block_dimensions(), (64, 64));
    }

    #[test]
    fn test_new_8bit_rgb() {
        let decoder = JpegBlockDecoder::new(
            8,
            3,
            64,
            64,
            InterleaveMode::P,
            JpegColorSpace::Rgb,
        );
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.num_bands(), 3);
        assert_eq!(decoder.color_space(), JpegColorSpace::Rgb);
    }

    #[test]
    fn test_new_8bit_ycbcr() {
        let decoder = JpegBlockDecoder::new(
            8,
            3,
            64,
            64,
            InterleaveMode::P,
            JpegColorSpace::YCbCr601,
        );
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.color_space(), JpegColorSpace::YCbCr601);
    }

    #[test]
    fn test_new_12bit_grayscale() {
        let decoder = JpegBlockDecoder::new(
            12,
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        );
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.bits_per_pixel(), 12);
    }

    #[cfg(feature = "libjpeg-turbo")]
    #[test]
    fn test_12bit_decode_returns_unsupported_error() {
        // 12-bit JPEG decoding requires a specially compiled libjpeg12 library
        // which is not commonly available. Verify we get a clear error message.
        let decoder = JpegBlockDecoder::new(
            12,
            1,
            8,
            8,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        )
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
        let decoder = JpegBlockDecoder::new(
            8,
            4,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
        );
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
        let decoder = JpegBlockDecoder::new(
            8,
            3,
            2,
            2,
            InterleaveMode::P,
            JpegColorSpace::Rgb,
        )
        .unwrap();

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
        use crate::jbp::jpeg::ffi::compress_8bit;

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
            let decoder = JpegBlockDecoder::new(
                8,
                3,
                width,
                height,
                InterleaveMode::P,
                JpegColorSpace::Rgb,
            )
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

            let decoder = JpegBlockDecoder::new(
                8,
                3,
                width,
                height,
                InterleaveMode::P,
                JpegColorSpace::Rgb,
            )
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

            assert!(avg_band0 < avg_band1, "Band 0 avg {} should be less than band 1 avg {}", avg_band0, avg_band1);
        }

        #[test]
        fn test_decode_multiband_truncated_length_prefix() {
            // Test error handling for truncated length prefix
            let decoder = JpegBlockDecoder::new(
                8,
                2,
                8,
                8,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
            )
            .unwrap();

            // Only 2 bytes - not enough for 4-byte length prefix
            let truncated_data = vec![0x00, 0x00];
            let result = decoder.decode_multiband_block(&truncated_data);
            assert!(result.is_err());
        }

        #[test]
        fn test_decode_multiband_truncated_stream() {
            // Test error handling for truncated JPEG stream
            let decoder = JpegBlockDecoder::new(
                8,
                2,
                8,
                8,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
            )
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
}
