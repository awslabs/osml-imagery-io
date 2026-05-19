//! PNGImageAssetProvider — implements ImageAssetProvider for PNG images.
//!
//! Provides blocked access to decoded PNG pixel data. Since PNG is a
//! single non-tiled image, the block grid is always 1×1 and there is
//! exactly 1 resolution level.
//!
//! The constructor accepts pre-decoded pixel data already in BSQ format.
//! The `PNGDatasetReader` handles actual decoding and BSQ conversion
//! before constructing this provider.

use std::sync::Arc;

use crate::error::CodecError;
use crate::png::metadata::PNGMetadataProvider;
use crate::traits::asset::AssetMetadata;
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::types::PixelType;

/// Image asset provider for a decoded PNG image.
///
/// Stores the full decoded image in BSQ (band-sequential) format and
/// serves it as a single 1×1 block. Supports band subsetting via
/// `get_block`.
pub struct PNGImageAssetProvider {
    /// Unique key identifying this asset (e.g., "image:0")
    key: String,
    /// Image width in pixels
    width: u32,
    /// Image height in pixels
    height: u32,
    /// Number of bands
    num_bands: u32,
    /// Pixel data type
    pixel_type: PixelType,
    /// Original PNG bit depth (1, 2, 4, 8, or 16)
    bit_depth: u8,
    /// Decoded pixels in BSQ format: all band-0 pixels, then band-1, etc.
    pixels: Vec<u8>,
    /// STAC-aligned roles (e.g., "data")
    roles: Vec<String>,
    /// Per-image metadata
    metadata: Arc<PNGMetadataProvider>,
}

impl PNGImageAssetProvider {
    /// Create a new `PNGImageAssetProvider` from pre-decoded BSQ pixel data.
    ///
    /// # Arguments
    ///
    /// * `key` - Asset key (e.g., "image:0")
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `num_bands` - Number of bands
    /// * `pixel_type` - Pixel data type (UInt8 or UInt16)
    /// * `bit_depth` - Original PNG bit depth (1, 2, 4, 8, or 16)
    /// * `pixels` - Decoded pixel data in BSQ format
    /// * `roles` - STAC-aligned roles (e.g., vec!["data"])
    /// * `metadata` - PNG metadata provider
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: String,
        width: u32,
        height: u32,
        num_bands: u32,
        pixel_type: PixelType,
        bit_depth: u8,
        pixels: Vec<u8>,
        roles: Vec<String>,
        metadata: Arc<PNGMetadataProvider>,
    ) -> Self {
        Self {
            key,
            width,
            height,
            num_bands,
            pixel_type,
            bit_depth,
            pixels,
            roles,
            metadata,
        }
    }

    /// Bytes per sample for the stored pixel type.
    fn bytes_per_sample(&self) -> usize {
        self.pixel_type.bytes_per_pixel()
    }

    /// Resolve the band selection: if None, return all bands [0..N).
    fn resolve_bands(&self, bands: Option<&[u32]>) -> Vec<u32> {
        match bands {
            Some(b) => b.to_vec(),
            None => (0..self.num_bands).collect(),
        }
    }
}

// =============================================================================
// AssetProvider Implementation
// =============================================================================

impl AssetMetadata for PNGImageAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.key
    }

    fn description(&self) -> &str {
        "PNG image segment"
    }

    fn media_type(&self) -> &str {
        "image/png"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "raw_asset() not supported for PNG; use get_block()".to_string(),
        ))
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }
}

// =============================================================================
// ImageAssetProvider Implementation
// =============================================================================

impl ImageAssetProvider for PNGImageAssetProvider {
    fn has_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
    ) -> Result<bool, CodecError> {
        Ok(resolution_level == 0 && block_row == 0 && block_col == 0)
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
        if block_row != 0 || block_col != 0 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        let requested_bands = self.resolve_bands(bands);
        let num_out_bands = requested_bands.len() as u32;
        let bps = self.bytes_per_sample();
        let band_size = self.width as usize * self.height as usize * bps;

        // If all bands requested in order, return the full buffer
        if requested_bands.len() == self.num_bands as usize
            && requested_bands
                .iter()
                .enumerate()
                .all(|(i, &b)| b == i as u32)
        {
            return Ok((
                self.pixels.clone(),
                [num_out_bands, self.height, self.width],
            ));
        }

        // Band subsetting: extract only requested bands
        let mut output = Vec::with_capacity(requested_bands.len() * band_size);
        for &band in &requested_bands {
            let start = band as usize * band_size;
            let end = start + band_size;
            output.extend_from_slice(&self.pixels[start..end]);
        }

        Ok((output, [num_out_bands, self.height, self.width]))
    }

    fn num_resolution_levels(&self) -> u32 {
        1
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
        self.width
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.height
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.bit_depth as u32
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        // After unpacking, actual storage is always 8 or 16 bits
        (self.bytes_per_sample() * 8) as u32
    }

    fn pixel_value_type(&self) -> PixelType {
        self.pixel_type
    }

    fn pad_pixel_value(&self) -> f64 {
        0.0
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper: create a metadata provider with minimal entries.
    fn make_metadata(
        width: u32,
        height: u32,
        bit_depth: u8,
        color_type: &str,
    ) -> Arc<PNGMetadataProvider> {
        let mut entries = HashMap::new();
        entries.insert("width".to_string(), serde_json::json!(width));
        entries.insert("height".to_string(), serde_json::json!(height));
        entries.insert("bit_depth".to_string(), serde_json::json!(bit_depth));
        entries.insert("color_type".to_string(), serde_json::json!(color_type));
        Arc::new(PNGMetadataProvider::new(entries))
    }

    // =========================================================================
    // Grayscale 8-bit (Req 3.8)
    // =========================================================================

    #[test]
    fn test_grayscale_8bit() {
        // 2x3 grayscale image, 1 band, UInt8
        let width = 3;
        let height = 2;
        let pixels: Vec<u8> = vec![10, 20, 30, 40, 50, 60]; // BSQ: 6 pixels
        let metadata = make_metadata(width, height, 8, "Grayscale");

        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            width,
            height,
            1,
            PixelType::UInt8,
            8,
            pixels.clone(),
            vec!["data".to_string()],
            metadata,
        );

        assert_eq!(provider.num_columns(), 3);
        assert_eq!(provider.num_rows(), 2);
        assert_eq!(provider.num_bands(), 1);
        assert_eq!(provider.pixel_value_type(), PixelType::UInt8);
        assert_eq!(provider.num_bits_per_pixel(), 8);
        assert_eq!(provider.actual_bits_per_pixel(), 8);
        assert_eq!(provider.num_resolution_levels(), 1);
        assert_eq!(provider.block_grid_size(), (1, 1));
        assert_eq!(provider.num_pixels_per_block_horizontal(), 3);
        assert_eq!(provider.num_pixels_per_block_vertical(), 2);
        assert_eq!(provider.media_type(), "image/png");

        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 3]);
        assert_eq!(data, pixels);
    }

    // =========================================================================
    // RGB 16-bit (Req 3.11)
    // =========================================================================

    #[test]
    fn test_rgb_16bit() {
        // 2x2 RGB 16-bit image, 3 bands, UInt16
        let width = 2;
        let height = 2;
        let num_bands = 3;
        let bps = 2; // bytes per sample for UInt16

        // BSQ: band0 (4 pixels * 2 bytes), band1, band2
        let mut pixels = Vec::new();
        // Band 0 (R): 100, 200, 300, 400
        for &v in &[100u16, 200, 300, 400] {
            pixels.extend_from_slice(&v.to_ne_bytes());
        }
        // Band 1 (G): 500, 600, 700, 800
        for &v in &[500u16, 600, 700, 800] {
            pixels.extend_from_slice(&v.to_ne_bytes());
        }
        // Band 2 (B): 900, 1000, 1100, 1200
        for &v in &[900u16, 1000, 1100, 1200] {
            pixels.extend_from_slice(&v.to_ne_bytes());
        }

        let metadata = make_metadata(width, height, 16, "RGB");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            width,
            height,
            num_bands,
            PixelType::UInt16,
            16,
            pixels.clone(),
            vec!["data".to_string()],
            metadata,
        );

        assert_eq!(provider.num_columns(), 2);
        assert_eq!(provider.num_rows(), 2);
        assert_eq!(provider.num_bands(), 3);
        assert_eq!(provider.pixel_value_type(), PixelType::UInt16);
        assert_eq!(provider.num_bits_per_pixel(), 16);
        assert_eq!(provider.actual_bits_per_pixel(), 16);

        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 2, 2]);
        assert_eq!(data.len(), num_bands as usize * 4 * bps);
        assert_eq!(data, pixels);
    }

    // =========================================================================
    // Out-of-bounds block (Req 3.5)
    // =========================================================================

    #[test]
    fn test_get_block_out_of_bounds() {
        let metadata = make_metadata(2, 2, 8, "Grayscale");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            2,
            2,
            1,
            PixelType::UInt8,
            8,
            vec![0; 4],
            vec!["data".to_string()],
            metadata,
        );

        // block_row=1 is out of bounds for a 1×1 grid
        let err = provider.get_block(1, 0, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(1, 0, 0)));

        // block_col=1 is out of bounds
        let err = provider.get_block(0, 1, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(0, 1, 0)));

        // Both out of bounds
        let err = provider.get_block(1, 1, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(1, 1, 0)));
    }

    // =========================================================================
    // Invalid resolution level (Req 3.6)
    // =========================================================================

    #[test]
    fn test_get_block_invalid_resolution() {
        let metadata = make_metadata(2, 2, 8, "Grayscale");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            2,
            2,
            1,
            PixelType::UInt8,
            8,
            vec![0; 4],
            vec!["data".to_string()],
            metadata,
        );

        let err = provider.get_block(0, 0, 1, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidResolutionLevel(1)));

        let err = provider.get_block(0, 0, 5, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidResolutionLevel(5)));
    }

    // =========================================================================
    // Band subsetting (Req 3.17)
    // =========================================================================

    #[test]
    fn test_band_subset() {
        // 2x2 RGBA image, 4 bands, UInt8
        let width = 2;
        let height = 2;
        // BSQ: band0=[1,2,3,4], band1=[5,6,7,8], band2=[9,10,11,12], band3=[13,14,15,16]
        let mut pixels = Vec::new();
        for band in 0..4u8 {
            for px in 0..4u8 {
                pixels.push(band * 4 + px + 1);
            }
        }

        let metadata = make_metadata(width, height, 8, "RGBA");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            width,
            height,
            4,
            PixelType::UInt8,
            8,
            pixels,
            vec!["data".to_string()],
            metadata,
        );

        // Request only bands 0 and 2 (R and B)
        let (data, shape) = provider.get_block(0, 0, 0, Some(&[0, 2])).unwrap();
        assert_eq!(shape, [2, 2, 2]);
        // Band 0: [1,2,3,4], Band 2: [9,10,11,12]
        assert_eq!(data, vec![1, 2, 3, 4, 9, 10, 11, 12]);

        // Request single band (band 3 = alpha)
        let (data, shape) = provider.get_block(0, 0, 0, Some(&[3])).unwrap();
        assert_eq!(shape, [1, 2, 2]);
        assert_eq!(data, vec![13, 14, 15, 16]);

        // Request bands in reverse order
        let (data, shape) = provider.get_block(0, 0, 0, Some(&[2, 0])).unwrap();
        assert_eq!(shape, [2, 2, 2]);
        assert_eq!(data, vec![9, 10, 11, 12, 1, 2, 3, 4]);
    }

    // =========================================================================
    // Indexed returns raw indices (Req 3.18)
    // =========================================================================

    #[test]
    fn test_indexed_returns_raw_indices() {
        // 3x2 indexed image: raw palette indices, single band UInt8
        let width = 3;
        let height = 2;
        // Palette indices (not RGB values)
        let indices: Vec<u8> = vec![0, 1, 2, 3, 4, 5];

        let metadata = make_metadata(width, height, 8, "Indexed");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            width,
            height,
            1,
            PixelType::UInt8,
            8,
            indices.clone(),
            vec!["data".to_string()],
            metadata,
        );

        assert_eq!(provider.num_bands(), 1);
        assert_eq!(provider.pixel_value_type(), PixelType::UInt8);

        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 3]);
        // Raw palette indices are returned, not expanded RGB
        assert_eq!(data, indices);
    }

    // =========================================================================
    // has_block tests
    // =========================================================================

    #[test]
    fn test_has_block() {
        let metadata = make_metadata(2, 2, 8, "Grayscale");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            2,
            2,
            1,
            PixelType::UInt8,
            8,
            vec![0; 4],
            vec!["data".to_string()],
            metadata,
        );

        assert!(provider.has_block(0, 0, 0).unwrap());
        assert!(!provider.has_block(1, 0, 0).unwrap());
        assert!(!provider.has_block(0, 1, 0).unwrap());
        assert!(!provider.has_block(0, 0, 1).unwrap());
    }

    // =========================================================================
    // Sub-byte bit depth (Req 3.16) — stored as UInt8 after unpacking
    // =========================================================================

    #[test]
    fn test_sub_byte_grayscale_4bit() {
        // 4x1 grayscale 4-bit image, unpacked to UInt8
        // Values: 0, 5, 10, 15 (valid 4-bit range 0-15)
        let pixels: Vec<u8> = vec![0, 5, 10, 15];
        let metadata = make_metadata(4, 1, 4, "Grayscale");

        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            4,
            1,
            1,
            PixelType::UInt8,
            4,
            pixels.clone(),
            vec!["data".to_string()],
            metadata,
        );

        assert_eq!(provider.num_bands(), 1);
        assert_eq!(provider.pixel_value_type(), PixelType::UInt8);
        assert_eq!(provider.num_bits_per_pixel(), 4);
        assert_eq!(provider.actual_bits_per_pixel(), 8); // stored as UInt8

        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 1, 4]);
        assert_eq!(data, pixels);
    }

    // =========================================================================
    // AssetProvider trait methods
    // =========================================================================

    #[test]
    fn test_asset_provider_methods() {
        let metadata = make_metadata(2, 2, 8, "RGB");
        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            2,
            2,
            3,
            PixelType::UInt8,
            8,
            vec![0; 12],
            vec!["data".to_string()],
            metadata,
        );

        assert_eq!(provider.key(), "image:0");
        assert_eq!(provider.title(), "image:0");
        assert_eq!(provider.description(), "PNG image segment");
        assert_eq!(provider.media_type(), "image/png");
        assert_eq!(provider.roles(), &["data".to_string()]);
        assert!(provider.raw_asset().is_err());
    }

    // =========================================================================
    // GrayscaleAlpha 8-bit (Req 3.14)
    // =========================================================================

    #[test]
    fn test_grayscale_alpha_8bit() {
        // 2x2 GrayscaleAlpha, 2 bands, UInt8
        // BSQ: band0=[100,150,200,250], band1=[255,128,64,0]
        let pixels: Vec<u8> = vec![100, 150, 200, 250, 255, 128, 64, 0];
        let metadata = make_metadata(2, 2, 8, "GrayscaleAlpha");

        let provider = PNGImageAssetProvider::new(
            "image:0".to_string(),
            2,
            2,
            2,
            PixelType::UInt8,
            8,
            pixels.clone(),
            vec!["data".to_string()],
            metadata,
        );

        assert_eq!(provider.num_bands(), 2);
        assert_eq!(provider.pixel_value_type(), PixelType::UInt8);

        let (data, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [2, 2, 2]);
        assert_eq!(data, pixels);
    }
}
