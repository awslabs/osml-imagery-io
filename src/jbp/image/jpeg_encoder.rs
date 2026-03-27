//! JPEG DCT block encoder.
//!
//! This module provides the JpegBlockEncoder for encoding image blocks
//! to JPEG DCT format for NITF image segments.
//!
//! # Supported Configurations
//!
//! | Configuration | Pixel Type | Bands | Notes |
//! |--------------|------------|-------|-------|
//! | Mono 8-bit | UInt8 | 1 | Standard grayscale |
//! | Mono 12-bit | UInt16 | 1 | Not supported (returns error) |
//! | RGB 24-bit | UInt8 | 3 | Pixel interleaved |
//! | YCbCr601 24-bit | UInt8 | 3 | Color space conversion |
//! | Multiband 8-bit | UInt8 | 2-999 | Each band separate JPEG |
//!
//! # Requirements
//! - 2.1: Encode JPEG DCT compressed blocks (IC=C3)
//! - 2.2: Encode 8-bit monochrome JPEG blocks
//! - 2.3: Return error for 12-bit JPEG requests
//! - 2.4: Encode 3-band RGB images (IMODE=P)
//! - 2.5: Convert RGB to YCbCr601 before compression
//! - 2.6: Encode multiband images (IMODE=B or S)

use crate::error::CodecError;
use crate::jbp::image::types::InterleaveMode;

use crate::jpeg::JpegCodec;
use crate::jbp::image::jpeg_decoder::JpegColorSpace;

/// Block encoder for JPEG DCT compressed imagery.
///
/// Encodes image blocks to JPEG format for NITF files with IC=C3, M3, or I1.
#[derive(Debug)]
pub struct JpegBlockEncoder {
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
    /// JPEG quality (1-100)
    quality: u8,
}

impl JpegBlockEncoder {
    /// Create a new JPEG block encoder.
    ///
    /// # Arguments
    /// * `bits_per_pixel` - Bits per pixel (8 or 12)
    /// * `num_bands` - Number of image bands
    /// * `block_width` - Width of each block in pixels
    /// * `block_height` - Height of each block in pixels
    /// * `imode` - Interleave mode (B, P, R, or S)
    /// * `color_space` - Color space for encoding
    /// * `quality` - JPEG quality factor (1-100)
    ///
    /// # Requirements
    /// - 2.1, 2.2, 2.4: Basic encoder construction
    pub fn new(
        bits_per_pixel: u8,
        num_bands: usize,
        block_width: usize,
        block_height: usize,
        imode: InterleaveMode,
        color_space: JpegColorSpace,
        quality: u8,
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

        // Validate quality
        if quality == 0 || quality > 100 {
            return Err(CodecError::InvalidFormat(format!(
                "Quality must be 1-100, got {}",
                quality
            )));
        }

        // Validate color space vs band count
        match color_space {
            JpegColorSpace::Grayscale => {
                // Grayscale can be used for single band or multiband (each band encoded separately)
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
            codec: JpegCodec::with_quality(quality),
            bits_per_pixel,
            num_bands,
            block_width,
            block_height,
            imode,
            color_space,
            quality,
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

    /// Get the quality setting.
    pub fn quality(&self) -> u8 {
        self.quality
    }

    /// Encode a block to JPEG format.
    ///
    /// # Arguments
    /// * `pixel_data` - The raw pixel data in band-sequential format (BSQ).
    ///   For 8-bit images, each byte is one pixel value.
    ///   For 12-bit images, each pixel is stored as 2 bytes (little-endian u16).
    ///
    /// # Returns
    /// The JPEG compressed data.
    ///
    /// # Requirements
    /// - 2.2: 8-bit grayscale encoding
    /// - 2.3: 12-bit returns error (not supported)
    /// - 2.4: 8-bit RGB encoding
    /// - 2.5: YCbCr601 color space handling
    #[cfg(feature = "libjpeg-turbo")]
    pub fn encode_block(&self, pixel_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        use crate::jpeg::ffi;

        // Validate input size
        let expected_size = self.expected_input_size();
        if pixel_data.len() != expected_size {
            return Err(CodecError::Encode(format!(
                "Input buffer size {} doesn't match expected size {} ({}x{}x{} bands, {} bits)",
                pixel_data.len(),
                expected_size,
                self.block_width,
                self.block_height,
                self.num_bands,
                self.bits_per_pixel
            )));
        }

        if self.bits_per_pixel == 12 {
            // 12-bit encoding is not supported
            return Err(CodecError::Unsupported(
                "12-bit JPEG encoding is not supported. \
                 Consider using JPEG 2000 (IC=C8) or uncompressed format (IC=NC) instead."
                    .into(),
            ));
        }

        // 8-bit encoding
        match self.color_space {
            JpegColorSpace::Grayscale => {
                // Single band grayscale - data is already in correct format
                ffi::compress_8bit(
                    pixel_data,
                    self.block_width,
                    self.block_height,
                    1,
                    self.quality,
                )
            }
            JpegColorSpace::Rgb | JpegColorSpace::YCbCr601 => {
                // RGB/YCbCr - need to convert from BSQ to pixel-interleaved
                let interleaved = self.band_sequential_to_pixel(pixel_data);
                // turbojpeg handles YCbCr conversion internally
                ffi::compress_8bit(
                    &interleaved,
                    self.block_width,
                    self.block_height,
                    3,
                    self.quality,
                )
            }
        }
    }

    /// Encode a block to JPEG format (stub when libjpeg-turbo is not available).
    #[cfg(not(feature = "libjpeg-turbo"))]
    pub fn encode_block(&self, _pixel_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "JPEG encoding requires the libjpeg-turbo feature".into(),
        ))
    }

    /// Encode a multiband block where each band is encoded as a separate JPEG stream.
    ///
    /// For IMODE=B (block interleaved) or IMODE=S (sequential), each band is
    /// encoded as a separate JPEG stream. The streams are concatenated with
    /// 4-byte length prefixes (big-endian).
    ///
    /// # Arguments
    /// * `pixel_data` - The raw pixel data in band-sequential format (BSQ).
    ///
    /// # Returns
    /// The concatenated JPEG streams with length prefixes.
    ///
    /// # Data Format
    /// The output data is structured as:
    /// ```text
    /// [4-byte length BE][JPEG stream 1][4-byte length BE][JPEG stream 2]...
    /// ```
    ///
    /// # Requirements
    /// - 2.6: Multiband JPEG encoding (IMODE=B or S)
    #[cfg(feature = "libjpeg-turbo")]
    pub fn encode_multiband_block(&self, pixel_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        use crate::jpeg::ffi;

        if self.num_bands == 1 {
            // Single band - just encode directly
            return self.encode_block(pixel_data);
        }

        // For 3-band RGB/YCbCr with IMODE=P, encode as a single JPEG stream
        if self.num_bands == 3
            && self.imode == InterleaveMode::P
            && (self.color_space == JpegColorSpace::Rgb
                || self.color_space == JpegColorSpace::YCbCr601)
        {
            return self.encode_block(pixel_data);
        }

        // Validate input size
        let expected_size = self.expected_input_size();
        if pixel_data.len() != expected_size {
            return Err(CodecError::Encode(format!(
                "Input buffer size {} doesn't match expected size {} ({}x{}x{} bands, {} bits)",
                pixel_data.len(),
                expected_size,
                self.block_width,
                self.block_height,
                self.num_bands,
                self.bits_per_pixel
            )));
        }

        if self.bits_per_pixel == 12 {
            return Err(CodecError::Unsupported(
                "12-bit JPEG encoding is not supported. \
                 Consider using JPEG 2000 (IC=C8) or uncompressed format (IC=NC) instead."
                    .into(),
            ));
        }

        // For IMODE=B or S, encode each band as a separate JPEG stream
        let pixels_per_band = self.block_width * self.block_height;
        let mut output = Vec::new();

        for band in 0..self.num_bands {
            // Extract this band's data
            let band_start = band * pixels_per_band;
            let band_end = band_start + pixels_per_band;
            let band_data = &pixel_data[band_start..band_end];

            // Compress this band as grayscale
            let band_jpeg = ffi::compress_8bit(
                band_data,
                self.block_width,
                self.block_height,
                1,
                self.quality,
            )?;

            // Add 4-byte length prefix (big-endian)
            let length = band_jpeg.len() as u32;
            output.extend_from_slice(&length.to_be_bytes());
            output.extend_from_slice(&band_jpeg);
        }

        Ok(output)
    }

    /// Encode a multiband block (stub when libjpeg-turbo is not available).
    #[cfg(not(feature = "libjpeg-turbo"))]
    pub fn encode_multiband_block(&self, _pixel_data: &[u8]) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "JPEG encoding requires the libjpeg-turbo feature".into(),
        ))
    }

    /// Calculate the expected input buffer size.
    fn expected_input_size(&self) -> usize {
        let pixels_per_band = self.block_width * self.block_height;
        let bytes_per_pixel = if self.bits_per_pixel == 12 { 2 } else { 1 };
        pixels_per_band * self.num_bands * bytes_per_pixel
    }

    /// Convert band-sequential data to pixel-interleaved format.
    ///
    /// Input: RRR...GGG...BBB... (band sequential)
    /// Output: RGBRGBRGB... (pixel interleaved)
    fn band_sequential_to_pixel(&self, data: &[u8]) -> Vec<u8> {
        let num_pixels = self.block_width * self.block_height;
        let num_bands = 3; // Only used for RGB/YCbCr
        let mut output = vec![0u8; num_pixels * num_bands];

        for pixel in 0..num_pixels {
            for band in 0..num_bands {
                output[pixel * num_bands + band] = data[band * num_pixels + pixel];
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
        let encoder = JpegBlockEncoder::new(
            8,
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            75,
        );
        assert!(encoder.is_ok());
        let encoder = encoder.unwrap();
        assert_eq!(encoder.bits_per_pixel(), 8);
        assert_eq!(encoder.num_bands(), 1);
        assert_eq!(encoder.block_dimensions(), (64, 64));
        assert_eq!(encoder.quality(), 75);
    }

    #[test]
    fn test_new_8bit_rgb() {
        let encoder = JpegBlockEncoder::new(
            8,
            3,
            64,
            64,
            InterleaveMode::P,
            JpegColorSpace::Rgb,
            90,
        );
        assert!(encoder.is_ok());
        let encoder = encoder.unwrap();
        assert_eq!(encoder.num_bands(), 3);
        assert_eq!(encoder.color_space(), JpegColorSpace::Rgb);
        assert_eq!(encoder.imode(), InterleaveMode::P);
    }

    #[test]
    fn test_new_8bit_ycbcr() {
        let encoder = JpegBlockEncoder::new(
            8,
            3,
            64,
            64,
            InterleaveMode::P,
            JpegColorSpace::YCbCr601,
            85,
        );
        assert!(encoder.is_ok());
        let encoder = encoder.unwrap();
        assert_eq!(encoder.color_space(), JpegColorSpace::YCbCr601);
    }

    #[test]
    fn test_new_12bit_grayscale() {
        // 12-bit encoder can be created, but encoding will fail
        let encoder = JpegBlockEncoder::new(
            12,
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            75,
        );
        assert!(encoder.is_ok());
        let encoder = encoder.unwrap();
        assert_eq!(encoder.bits_per_pixel(), 12);
    }

    #[test]
    fn test_new_multiband() {
        let encoder = JpegBlockEncoder::new(
            8,
            4,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            75,
        );
        assert!(encoder.is_ok());
        let encoder = encoder.unwrap();
        assert_eq!(encoder.num_bands(), 4);
    }

    // =========================================================================
    // Validation Error Tests
    // =========================================================================

    #[test]
    fn test_invalid_bits_per_pixel() {
        let result = JpegBlockEncoder::new(
            16, // Invalid - only 8 or 12 supported
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            75,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_zero_bands() {
        let result = JpegBlockEncoder::new(
            8,
            0, // Invalid - must be at least 1
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            75,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_invalid_quality_zero() {
        let result = JpegBlockEncoder::new(
            8,
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            0, // Invalid - must be 1-100
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_invalid_quality_over_100() {
        let result = JpegBlockEncoder::new(
            8,
            1,
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            101, // Invalid - must be 1-100
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_rgb_wrong_band_count() {
        let result = JpegBlockEncoder::new(
            8,
            4, // Invalid - RGB requires 3 bands
            64,
            64,
            InterleaveMode::P,
            JpegColorSpace::Rgb,
            75,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_12bit_multiband_not_supported() {
        let result = JpegBlockEncoder::new(
            12,
            3, // Invalid - 12-bit only supports single band
            64,
            64,
            InterleaveMode::B,
            JpegColorSpace::Grayscale,
            75,
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    // =========================================================================
    // Band Sequential to Pixel Conversion Tests
    // =========================================================================

    #[test]
    fn test_band_sequential_to_pixel() {
        let encoder = JpegBlockEncoder::new(
            8,
            3,
            2,
            2,
            InterleaveMode::P,
            JpegColorSpace::Rgb,
            75,
        )
        .unwrap();

        // Input: RRR...GGG...BBB... (band sequential)
        let input = vec![
            1, 4, 7, 10, // R band
            2, 5, 8, 11, // G band
            3, 6, 9, 12, // B band
        ];

        let output = encoder.band_sequential_to_pixel(&input);

        // Expected: RGBRGBRGBRGB (pixel interleaved)
        let expected = vec![
            1, 2, 3, // pixel 0: R=1, G=2, B=3
            4, 5, 6, // pixel 1: R=4, G=5, B=6
            7, 8, 9, // pixel 2: R=7, G=8, B=9
            10, 11, 12, // pixel 3: R=10, G=11, B=12
        ];

        assert_eq!(output, expected);
    }

    // =========================================================================
    // Encode Tests (require libjpeg-turbo feature)
    // =========================================================================

    #[cfg(feature = "libjpeg-turbo")]
    mod encode_tests {
        use super::*;
        use crate::jpeg::ffi::decompress_8bit;

        #[test]
        fn test_encode_8bit_grayscale_roundtrip() {
            // Create a simple 8x8 grayscale image
            let width = 8;
            let height = 8;
            let mut src = vec![0u8; width * height];
            for i in 0..src.len() {
                src[i] = (i * 4) as u8;
            }

            // Create encoder and encode
            let encoder = JpegBlockEncoder::new(
                8,
                1,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                90,
            )
            .unwrap();

            let jpeg_data = encoder.encode_block(&src).unwrap();
            assert!(!jpeg_data.is_empty());

            // Decompress and verify
            let decoded = decompress_8bit(&jpeg_data, width, height, 1).unwrap();
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
        fn test_encode_8bit_rgb_roundtrip() {
            // Create a simple 8x8 RGB image in BSQ format
            let width = 8;
            let height = 8;
            let num_pixels = width * height;
            let mut src = vec![0u8; num_pixels * 3];

            // Fill with BSQ data (RRR...GGG...BBB...)
            for i in 0..num_pixels {
                src[i] = (i * 4) as u8; // R band
                src[num_pixels + i] = (i * 2) as u8; // G band
                src[2 * num_pixels + i] = (i * 3) as u8; // B band
            }

            // Create encoder and encode
            let encoder = JpegBlockEncoder::new(
                8,
                3,
                width,
                height,
                InterleaveMode::P,
                JpegColorSpace::Rgb,
                90,
            )
            .unwrap();

            let jpeg_data = encoder.encode_block(&src).unwrap();
            assert!(!jpeg_data.is_empty());

            // Decompress (returns pixel-interleaved)
            let decoded = decompress_8bit(&jpeg_data, width, height, 3).unwrap();
            assert_eq!(decoded.len(), num_pixels * 3);
        }

        #[test]
        fn test_encode_8bit_ycbcr_roundtrip() {
            // Create a simple 8x8 RGB image in BSQ format
            let width = 8;
            let height = 8;
            let num_pixels = width * height;
            let mut src = vec![0u8; num_pixels * 3];

            // Fill with BSQ data - reddish color
            for i in 0..num_pixels {
                src[i] = 200; // R band
                src[num_pixels + i] = 100; // G band
                src[2 * num_pixels + i] = 50; // B band
            }

            // Create encoder with YCbCr color space
            let encoder = JpegBlockEncoder::new(
                8,
                3,
                width,
                height,
                InterleaveMode::P,
                JpegColorSpace::YCbCr601,
                90,
            )
            .unwrap();

            let jpeg_data = encoder.encode_block(&src).unwrap();
            assert!(!jpeg_data.is_empty());

            // Decompress (turbojpeg handles YCbCr to RGB conversion)
            let decoded = decompress_8bit(&jpeg_data, width, height, 3).unwrap();
            assert_eq!(decoded.len(), num_pixels * 3);
        }

        #[test]
        fn test_encode_12bit_returns_unsupported_error() {
            let encoder = JpegBlockEncoder::new(
                12,
                1,
                8,
                8,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                75,
            )
            .unwrap();

            // 12-bit data (8x8 pixels * 2 bytes per pixel)
            let src = vec![0u8; 8 * 8 * 2];
            let result = encoder.encode_block(&src);

            assert!(result.is_err());
            if let Err(CodecError::Unsupported(msg)) = result {
                assert!(
                    msg.contains("12-bit") || msg.contains("not supported"),
                    "Error message should mention 12-bit: {}",
                    msg
                );
            } else {
                panic!("Expected Unsupported error");
            }
        }

        #[test]
        fn test_encode_wrong_buffer_size() {
            let encoder = JpegBlockEncoder::new(
                8,
                1,
                8,
                8,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                75,
            )
            .unwrap();

            // Wrong size - should be 64 bytes
            let src = vec![0u8; 32];
            let result = encoder.encode_block(&src);

            assert!(result.is_err());
            assert!(matches!(result, Err(CodecError::Encode(_))));
        }

        #[test]
        fn test_encode_multiband_single_band() {
            // Single band should work with encode_multiband_block
            let width = 8;
            let height = 8;
            let src = vec![128u8; width * height];

            let encoder = JpegBlockEncoder::new(
                8,
                1,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                90,
            )
            .unwrap();

            let jpeg_data = encoder.encode_multiband_block(&src).unwrap();
            assert!(!jpeg_data.is_empty());
        }

        #[test]
        fn test_encode_multiband_rgb_imode_p() {
            // RGB with IMODE=P should encode as single stream
            let width = 8;
            let height = 8;
            let num_pixels = width * height;
            let src = vec![128u8; num_pixels * 3];

            let encoder = JpegBlockEncoder::new(
                8,
                3,
                width,
                height,
                InterleaveMode::P,
                JpegColorSpace::Rgb,
                90,
            )
            .unwrap();

            let jpeg_data = encoder.encode_multiband_block(&src).unwrap();
            assert!(!jpeg_data.is_empty());
        }

        #[test]
        fn test_encode_multiband_separate_streams() {
            // Create multiband data (4 bands)
            let width = 8;
            let height = 8;
            let num_bands = 4;
            let pixels_per_band = width * height;

            let mut src = vec![0u8; pixels_per_band * num_bands];
            for band in 0..num_bands {
                for i in 0..pixels_per_band {
                    src[band * pixels_per_band + i] = (band * 50) as u8;
                }
            }

            let encoder = JpegBlockEncoder::new(
                8,
                num_bands,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                90,
            )
            .unwrap();

            let multiband_data = encoder.encode_multiband_block(&src).unwrap();

            // Verify the output has length prefixes
            // Each band should have a 4-byte length prefix followed by JPEG data
            let mut offset = 0;
            for _band in 0..num_bands {
                assert!(offset + 4 <= multiband_data.len(), "Missing length prefix");
                let length = u32::from_be_bytes([
                    multiband_data[offset],
                    multiband_data[offset + 1],
                    multiband_data[offset + 2],
                    multiband_data[offset + 3],
                ]) as usize;
                offset += 4;

                assert!(offset + length <= multiband_data.len(), "Truncated JPEG stream");
                // Verify JPEG magic bytes (SOI marker)
                assert_eq!(multiband_data[offset], 0xFF);
                assert_eq!(multiband_data[offset + 1], 0xD8);
                offset += length;
            }
        }

        #[test]
        fn test_encode_multiband_imode_s() {
            // Test IMODE=S (sequential) with separate JPEG streams
            let width = 8;
            let height = 8;
            let num_bands = 2;
            let pixels_per_band = width * height;

            let mut src = vec![0u8; pixels_per_band * num_bands];
            for band in 0..num_bands {
                for i in 0..pixels_per_band {
                    src[band * pixels_per_band + i] = (band * 100 + 50) as u8;
                }
            }

            let encoder = JpegBlockEncoder::new(
                8,
                num_bands,
                width,
                height,
                InterleaveMode::S, // Sequential mode
                JpegColorSpace::Grayscale,
                90,
            )
            .unwrap();

            let multiband_data = encoder.encode_multiband_block(&src).unwrap();
            assert!(!multiband_data.is_empty());

            // Verify we can parse the length-prefixed streams
            let mut offset = 0;
            for _band in 0..num_bands {
                let length = u32::from_be_bytes([
                    multiband_data[offset],
                    multiband_data[offset + 1],
                    multiband_data[offset + 2],
                    multiband_data[offset + 3],
                ]) as usize;
                offset += 4 + length;
            }
            assert_eq!(offset, multiband_data.len());
        }

        #[test]
        fn test_encode_quality_affects_size() {
            // Higher quality should produce larger files
            let width = 16;
            let height = 16;
            let mut src = vec![0u8; width * height];
            for i in 0..src.len() {
                src[i] = ((i * 7) % 256) as u8; // Some variation
            }

            let encoder_low = JpegBlockEncoder::new(
                8,
                1,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                10, // Low quality
            )
            .unwrap();

            let encoder_high = JpegBlockEncoder::new(
                8,
                1,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                95, // High quality
            )
            .unwrap();

            let jpeg_low = encoder_low.encode_block(&src).unwrap();
            let jpeg_high = encoder_high.encode_block(&src).unwrap();

            // High quality should generally produce larger files
            // (though this isn't always guaranteed for very small images)
            assert!(
                jpeg_high.len() >= jpeg_low.len(),
                "High quality {} should be >= low quality {}",
                jpeg_high.len(),
                jpeg_low.len()
            );
        }

        /// Test encoder/decoder roundtrip for multiband data
        #[test]
        fn test_encode_decode_multiband_roundtrip() {
            use crate::jbp::image::jpeg_decoder::JpegBlockDecoder;

            let width = 8;
            let height = 8;
            let num_bands = 4;
            let pixels_per_band = width * height;

            // Create source data with distinct values per band
            let mut src = vec![0u8; pixels_per_band * num_bands];
            for band in 0..num_bands {
                for i in 0..pixels_per_band {
                    src[band * pixels_per_band + i] = (band * 50 + 25) as u8;
                }
            }

            // Encode
            let encoder = JpegBlockEncoder::new(
                8,
                num_bands,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
                95,
            )
            .unwrap();

            let encoded = encoder.encode_multiband_block(&src).unwrap();

            // Decode
            let decoder = JpegBlockDecoder::new(
                8,
                num_bands,
                width,
                height,
                InterleaveMode::B,
                JpegColorSpace::Grayscale,
            )
            .unwrap();

            let decoded = decoder.decode_multiband_block(&encoded).unwrap();
            assert_eq!(decoded.len(), src.len());

            // Verify each band's average is close to original
            for band in 0..num_bands {
                let orig_band = &src[band * pixels_per_band..(band + 1) * pixels_per_band];
                let dec_band = &decoded[band * pixels_per_band..(band + 1) * pixels_per_band];

                let orig_avg: f64 =
                    orig_band.iter().map(|&x| x as f64).sum::<f64>() / pixels_per_band as f64;
                let dec_avg: f64 =
                    dec_band.iter().map(|&x| x as f64).sum::<f64>() / pixels_per_band as f64;

                assert!(
                    (orig_avg - dec_avg).abs() < 10.0,
                    "Band {} avg mismatch: {} vs {}",
                    band,
                    orig_avg,
                    dec_avg
                );
            }
        }
    }
}
