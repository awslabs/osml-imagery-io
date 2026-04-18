//! JPEGDatasetReader — implements DatasetReader for standalone JPEG files.
//!
//! Opens a `.jpg` or `.jpeg` file from a byte slice, validates the SOI marker
//! (0xFFD8), parses the SOF marker to extract metadata (dimensions, bands,
//! color space), and exposes a single image asset keyed as `"image:0"`.
//! Pixel decoding is deferred to `get_block()` time on the ImageAssetProvider.
//!
//! The entire input buffer is stored once as `Arc<[u8]>` that is shared with
//! the image asset provider for on-demand decoding.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use crate::error::CodecError;
use crate::jpeg::ffi::TjDecompressor;
use crate::jpeg::image::JPEGImageAssetProvider;
use crate::jpeg::metadata::JPEGMetadataProvider;
use crate::traits::asset::AssetMetadata;
use crate::traits::asset::AssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::traits::reader::DatasetReader;
use crate::types::AssetType;

use super::sys::TJSAMP_GRAY;

/// JPEG SOI (Start of Image) marker.
const JPEG_SOI: [u8; 2] = [0xFF, 0xD8];

/// JPEG dataset reader implementing the `DatasetReader` trait.
///
/// Owns a single image asset provider and dataset-level metadata.
/// Metadata is extracted eagerly during `from_bytes`; pixel decoding
/// is deferred to `get_block()` calls on the image asset provider.
pub struct JPEGDatasetReader {
    image_asset: Option<Arc<JPEGImageAssetProvider>>,
    metadata: Arc<JPEGMetadataProvider>,
}

impl std::fmt::Debug for JPEGDatasetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JPEGDatasetReader")
            .field("has_image", &self.image_asset.is_some())
            .finish()
    }
}

impl JPEGDatasetReader {
    /// Construct from a raw byte slice.
    ///
    /// Validates the SOI marker (0xFFD8), then uses libjpeg-turbo to parse
    /// the JPEG header for metadata (dimensions, bands, color space). Does
    /// NOT decode pixel data — that is deferred to `get_block()` calls on
    /// the `ImageAssetProvider`.
    ///
    /// The input is copied once into an `Arc<[u8]>` that is shared with the
    /// image asset provider.
    pub fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        // Validate minimum length
        if data.len() < 2 {
            return Err(CodecError::InvalidFormat(
                "Not a valid JPEG file: too short".to_string(),
            ));
        }

        // Validate SOI marker
        if data[..2] != JPEG_SOI {
            return Err(CodecError::InvalidFormat(
                "Not a valid JPEG file: invalid signature".to_string(),
            ));
        }

        // After SOI, the next byte must be 0xFF (start of a marker segment).
        // This catches truncated files that have SOI but no actual JPEG structure.
        if data.len() < 4 || data[2] != 0xFF {
            return Err(CodecError::InvalidFormat(
                "Not a valid JPEG file: no marker segment after SOI".to_string(),
            ));
        }

        // Use libjpeg-turbo to parse the header for dimensions and color info
        let decompressor = TjDecompressor::new().map_err(|_| {
            CodecError::InvalidFormat("Not a valid JPEG file: failed to parse header".to_string())
        })?;

        let (width, height, subsamp, _colorspace) =
            decompressor.get_header(data).map_err(|_| {
                CodecError::InvalidFormat(
                    "Not a valid JPEG file: failed to parse header".to_string(),
                )
            })?;

        // Determine band count and color space from subsampling
        let (num_bands, color_space) = if subsamp == TJSAMP_GRAY {
            (1u32, "Grayscale")
        } else {
            (3u32, "RGB")
        };

        // Build metadata entries
        let mut entries = HashMap::new();
        entries.insert("width".to_string(), json!(width as u64));
        entries.insert("height".to_string(), json!(height as u64));
        entries.insert("num_components".to_string(), json!(num_bands));
        entries.insert("bits_per_pixel".to_string(), json!(8));
        entries.insert("color_space".to_string(), json!(color_space));

        let metadata = Arc::new(JPEGMetadataProvider::new(entries));

        // Single allocation: wrap the entire input buffer in an Arc
        let buffer: Arc<[u8]> = Arc::from(data);

        let image_asset = JPEGImageAssetProvider::new(
            "image:0".to_string(),
            width as u32,
            height as u32,
            num_bands,
            buffer,
            vec!["data".to_string()],
            metadata.clone(),
        );

        Ok(Self {
            image_asset: Some(Arc::new(image_asset)),
            metadata,
        })
    }
}

// =============================================================================
// DatasetReader Implementation
// =============================================================================

impl DatasetReader for JPEGDatasetReader {
    fn get_asset(&self, key: &str) -> Result<AssetProvider, CodecError> {
        match &self.image_asset {
            Some(asset) if asset.key() == key => Ok(AssetProvider::Image(
                asset.clone() as Arc<dyn crate::traits::image::ImageAssetProvider>
            )),
            _ => Err(CodecError::AssetNotFound(key.to_string())),
        }
    }

    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<&[String]>,
    ) -> Vec<String> {
        match asset_type {
            None | Some(AssetType::Image) => match &self.image_asset {
                Some(asset) => {
                    if let Some(requested) = roles {
                        let asset_roles = asset.roles();
                        if requested.iter().any(|r| asset_roles.contains(r)) {
                            vec!["image:0".to_string()]
                        } else {
                            Vec::new()
                        }
                    } else {
                        vec!["image:0".to_string()]
                    }
                }
                None => Vec::new(),
            },
            Some(AssetType::Text) | Some(AssetType::Graphics) | Some(AssetType::Data) => Vec::new(),
        }
    }

    fn has_asset(&self, key: &str) -> bool {
        match &self.image_asset {
            Some(asset) => asset.key() == key,
            None => false,
        }
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn close(&mut self) -> Result<(), CodecError> {
        self.image_asset = None;
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jpeg::ffi::compress_8bit;
    use crate::traits::image::ImageAssetProvider;
    use crate::types::PixelType;

    // =========================================================================
    // Signature validation tests
    // =========================================================================

    #[test]
    fn test_from_bytes_empty_data() {
        let result = JPEGDatasetReader::from_bytes(&[]);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("too short"), "got: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_single_byte() {
        let result = JPEGDatasetReader::from_bytes(&[0xFF]);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("too short"), "got: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_invalid_signature() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let result = JPEGDatasetReader::from_bytes(&data);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("invalid signature"), "got: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_soi_only_no_valid_header() {
        // SOI marker but no valid JPEG structure after it
        let data = vec![0xFF, 0xD8, 0x00, 0x00];
        let result = JPEGDatasetReader::from_bytes(&data);
        assert!(result.is_err());
    }

    // =========================================================================
    // Helper: create a valid JPEG from pixel data
    // =========================================================================

    fn make_jpeg(width: usize, height: usize, num_bands: usize, quality: u8) -> (Vec<u8>, Vec<u8>) {
        // Create pixel-interleaved source data
        let mut src = vec![0u8; width * height * num_bands];
        for i in 0..src.len() {
            src[i] = (i * 7 % 256) as u8;
        }
        let jpeg_data = compress_8bit(&src, width, height, num_bands, quality).unwrap();
        (jpeg_data, src)
    }

    // =========================================================================
    // DatasetReader trait tests
    // =========================================================================

    #[test]
    fn test_roundtrip_grayscale() {
        let (jpeg_data, _src) = make_jpeg(16, 16, 1, 95);

        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        assert!(reader.has_asset("image:0"));
        assert!(!reader.has_asset("nonexistent"));

        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image:0"]);
        assert!(reader
            .get_asset_keys(Some(AssetType::Text), None)
            .is_empty());

        // Check metadata
        let meta = reader.metadata();
        let dict = meta.as_dict(None);
        assert_eq!(dict.get("width").and_then(|v| v.as_u64()), Some(16));
        assert_eq!(dict.get("height").and_then(|v| v.as_u64()), Some(16));
        assert_eq!(dict.get("num_components").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(dict.get("bits_per_pixel").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(
            dict.get("color_space").and_then(|v| v.as_str()),
            Some("Grayscale")
        );

        // Decode and verify shape
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");
        assert_eq!(image.num_columns(), 16);
        assert_eq!(image.num_rows(), 16);
        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.pixel_value_type(), PixelType::UInt8);
        assert_eq!(image.num_bits_per_pixel(), 8);
        assert_eq!(image.num_resolution_levels(), 1);

        let (data, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 16, 16]);
        assert_eq!(data.len(), 16 * 16);
    }

    #[test]
    fn test_roundtrip_rgb() {
        let (jpeg_data, _src) = make_jpeg(16, 16, 3, 95);

        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();

        let meta = reader.metadata();
        let dict = meta.as_dict(None);
        assert_eq!(dict.get("num_components").and_then(|v| v.as_u64()), Some(3));
        assert_eq!(
            dict.get("color_space").and_then(|v| v.as_str()),
            Some("RGB")
        );

        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");
        assert_eq!(image.num_bands(), 3);

        let (data, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 16, 16]);
        assert_eq!(data.len(), 3 * 16 * 16);
    }

    #[test]
    fn test_get_asset_invalid_key() {
        let (jpeg_data, _) = make_jpeg(8, 8, 1, 90);
        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();

        match reader.get_asset("nonexistent") {
            Err(CodecError::AssetNotFound(key)) => assert_eq!(key, "nonexistent"),
            Ok(_) => panic!("Expected AssetNotFound, got Ok"),
            Err(e) => panic!("Expected AssetNotFound, got: {}", e),
        }
    }

    #[test]
    fn test_close_clears_assets() {
        let (jpeg_data, _) = make_jpeg(8, 8, 1, 90);
        let mut reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();

        assert!(reader.has_asset("image:0"));
        reader.close().unwrap();
        assert!(!reader.has_asset("image:0"));
        assert!(reader.get_asset("image:0").is_err());
        assert!(reader
            .get_asset_keys(Some(AssetType::Image), None)
            .is_empty());
    }

    #[test]
    fn test_invalid_block_coordinates() {
        let (jpeg_data, _) = make_jpeg(8, 8, 1, 90);
        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        let err = image.get_block(1, 0, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(1, 0, 0)));

        let err = image.get_block(0, 1, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(0, 1, 0)));
    }

    #[test]
    fn test_invalid_resolution_level() {
        let (jpeg_data, _) = make_jpeg(8, 8, 1, 90);
        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        let err = image.get_block(0, 0, 1, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidResolutionLevel(1)));
    }

    #[test]
    fn test_has_block() {
        let (jpeg_data, _) = make_jpeg(8, 8, 1, 90);
        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert!(image.has_block(0, 0, 0));
        assert!(!image.has_block(1, 0, 0));
        assert!(!image.has_block(0, 1, 0));
        assert!(!image.has_block(0, 0, 1));
    }

    #[test]
    fn test_band_subset_rgb() {
        let (jpeg_data, _) = make_jpeg(8, 8, 3, 95);
        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        // Get all bands first for reference
        let (all_data, _) = image.get_block(0, 0, 0, None).unwrap();
        let band_size = 8 * 8;

        // Request only band 0
        let (data, shape) = image.get_block(0, 0, 0, Some(&[0])).unwrap();
        assert_eq!(shape, [1, 8, 8]);
        assert_eq!(data, &all_data[..band_size]);

        // Request bands in reverse order
        let (data, shape) = image.get_block(0, 0, 0, Some(&[2, 0])).unwrap();
        assert_eq!(shape, [2, 8, 8]);
        assert_eq!(&data[..band_size], &all_data[band_size * 2..band_size * 3]);
        assert_eq!(&data[band_size..], &all_data[..band_size]);
    }

    #[test]
    fn test_lossy_roundtrip_values_close() {
        // Verify that JPEG lossy compression produces values close to the original
        let width = 16;
        let height = 16;
        let num_bands = 1;

        // Create uniform-ish source data (128 everywhere)
        let src: Vec<u8> = vec![128; width * height * num_bands];
        let jpeg_data = compress_8bit(&src, width, height, num_bands, 95).unwrap();

        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        let (data, _) = image.get_block(0, 0, 0, None).unwrap();
        for (i, &pixel) in data.iter().enumerate() {
            assert!(
                (pixel as i32 - 128).abs() < 10,
                "Pixel {} differs too much: {} vs 128",
                i,
                pixel
            );
        }
    }

    #[test]
    fn test_bsq_format_rgb() {
        // Verify that the output is actually in BSQ format (not interleaved)
        // Create a JPEG where R=200, G=100, B=50 everywhere
        let width = 8;
        let height = 8;
        let npix = width * height;

        // Interleaved input for compress_8bit
        let mut src = Vec::with_capacity(npix * 3);
        for _ in 0..npix {
            src.push(200); // R
            src.push(100); // G
            src.push(50); // B
        }
        let jpeg_data = compress_8bit(&src, width, height, 3, 100).unwrap();

        let reader = JPEGDatasetReader::from_bytes(&jpeg_data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        let (data, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 8, 8]);

        // In BSQ format: first npix bytes = R band, next npix = G band, last npix = B band
        // JPEG is lossy so values won't be exact, but bands should be clearly separated
        let r_band = &data[..npix];
        let g_band = &data[npix..npix * 2];
        let b_band = &data[npix * 2..];

        // R band should be close to 200, G close to 100, B close to 50
        let r_avg: f64 = r_band.iter().map(|&v| v as f64).sum::<f64>() / npix as f64;
        let g_avg: f64 = g_band.iter().map(|&v| v as f64).sum::<f64>() / npix as f64;
        let b_avg: f64 = b_band.iter().map(|&v| v as f64).sum::<f64>() / npix as f64;

        assert!((r_avg - 200.0).abs() < 20.0, "R avg: {}", r_avg);
        assert!((g_avg - 100.0).abs() < 20.0, "G avg: {}", g_avg);
        assert!((b_avg - 50.0).abs() < 20.0, "B avg: {}", b_avg);
    }
}
