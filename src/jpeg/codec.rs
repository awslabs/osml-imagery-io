//! JPEG DCT codec interface and capabilities.
//!
//! This module defines the core codec types for JPEG DCT compression.

/// JPEG codec for encoding and decoding JPEG DCT compressed imagery.
#[derive(Debug, Clone)]
pub struct JpegCodec {
    /// JPEG quality factor (1-100)
    quality: u8,
}

impl JpegCodec {
    /// Create a new JPEG codec with default quality (75).
    pub fn new() -> Self {
        Self { quality: 75 }
    }

    /// Create a new JPEG codec with the specified quality.
    ///
    /// # Arguments
    /// * `quality` - JPEG quality factor (1-100). Higher values produce
    ///   better quality but larger files.
    pub fn with_quality(quality: u8) -> Self {
        Self {
            quality: quality.clamp(1, 100),
        }
    }

    /// Get the quality setting.
    pub fn quality(&self) -> u8 {
        self.quality
    }

    /// Get the codec capabilities.
    pub fn capabilities(&self) -> JpegCodecCapabilities {
        JpegCodecCapabilities {
            supports_8bit: true,
            supports_12bit: cfg!(feature = "libjpeg-turbo"),
            supports_rgb: true,
            supports_ycbcr: true,
        }
    }
}

impl Default for JpegCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Capabilities of the JPEG codec.
#[derive(Debug, Clone)]
pub struct JpegCodecCapabilities {
    /// Whether 8-bit baseline JPEG is supported.
    pub supports_8bit: bool,
    /// Whether 12-bit extended JPEG is supported.
    pub supports_12bit: bool,
    /// Whether RGB color space is supported.
    pub supports_rgb: bool,
    /// Whether YCbCr color space is supported.
    pub supports_ycbcr: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_codec_new_default_quality() {
        let codec = JpegCodec::new();
        assert_eq!(codec.quality(), 75);
    }

    #[test]
    fn test_jpeg_codec_default_trait() {
        let codec = JpegCodec::default();
        assert_eq!(codec.quality(), 75);
    }

    #[test]
    fn test_jpeg_codec_with_quality() {
        let codec = JpegCodec::with_quality(90);
        assert_eq!(codec.quality(), 90);
    }

    #[test]
    fn test_jpeg_codec_quality_clamped_to_min() {
        let codec = JpegCodec::with_quality(0);
        assert_eq!(codec.quality(), 1);
    }

    #[test]
    fn test_jpeg_codec_quality_clamped_to_max() {
        let codec = JpegCodec::with_quality(255);
        assert_eq!(codec.quality(), 100);
    }

    #[test]
    fn test_jpeg_codec_capabilities_8bit_always_supported() {
        let codec = JpegCodec::new();
        let caps = codec.capabilities();
        assert!(caps.supports_8bit);
    }

    #[test]
    fn test_jpeg_codec_capabilities_rgb_supported() {
        let codec = JpegCodec::new();
        let caps = codec.capabilities();
        assert!(caps.supports_rgb);
    }

    #[test]
    fn test_jpeg_codec_capabilities_ycbcr_supported() {
        let codec = JpegCodec::new();
        let caps = codec.capabilities();
        assert!(caps.supports_ycbcr);
    }
}
