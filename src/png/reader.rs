//! PNGDatasetReader — implements DatasetReader for PNG files.
//!
//! Opens a PNG from a byte slice, validates the signature, decodes the full
//! image eagerly using the `png` crate, and exposes a single image asset
//! keyed as `"image:0"`. Dataset-level metadata includes width,
//! height, bit_depth, color_type, and any ancillary chunk data.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::error::CodecError;
use crate::png::image::PNGImageAssetProvider;
use crate::png::metadata::PNGMetadataProvider;
use crate::traits::asset::AssetProvider;
use crate::traits::asset::AssetMetadata;
use crate::traits::metadata::MetadataProvider;
use crate::traits::reader::DatasetReader;
use crate::types::{AssetType, PixelType};

/// The 8-byte PNG file signature.
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// PNG dataset reader implementing the `DatasetReader` trait.
///
/// Owns a single image asset provider and dataset-level metadata.
/// The full image is eagerly decoded during `from_bytes`.
pub struct PNGDatasetReader {
    image_asset: Option<Arc<PNGImageAssetProvider>>,
    metadata: Arc<PNGMetadataProvider>,
}

impl std::fmt::Debug for PNGDatasetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PNGDatasetReader")
            .field("has_image", &self.image_asset.is_some())
            .finish()
    }
}

impl PNGDatasetReader {
    /// Construct from a raw byte slice.
    ///
    /// Validates the PNG signature (first 8 bytes), decodes the full image
    /// eagerly, extracts metadata from ancillary chunks, and builds the
    /// image asset provider with BSQ pixel data.
    pub fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        // Validate PNG signature
        if data.len() < 8 || data[..8] != PNG_SIGNATURE {
            return Err(CodecError::InvalidFormat(
                "Not a valid PNG file: invalid signature".to_string(),
            ));
        }

        let cursor = Cursor::new(data);
        let mut decoder = png::Decoder::new(cursor);

        // For indexed images we want raw palette indices (no EXPAND).
        // For sub-byte grayscale we want EXPAND to unpack to 1 byte/sample.
        // We'll peek at the header first to decide the transformation.
        // Default: IDENTITY (no transforms) — we handle sub-byte expansion
        // after peeking at the header info.
        decoder.set_transformations(png::Transformations::IDENTITY);

        let mut reader = decoder
            .read_info()
            .map_err(|e| CodecError::Decode(format!("PNG decode error: {}", e)))?;

        let info = reader.info();
        let width = info.width;
        let height = info.height;
        let color_type = info.color_type;
        let bit_depth = info.bit_depth;

        // Determine if we need to re-decode with EXPAND for sub-byte
        // non-indexed images.
        let need_expand = matches!(
            bit_depth,
            png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four
        ) && color_type != png::ColorType::Indexed;

        // Extract metadata from the info before we potentially re-decode
        let metadata_entries = Self::extract_metadata(info);

        // If we need EXPAND, re-create the decoder with the transform
        let (output_info, raw_pixels) = if need_expand {
            let cursor2 = Cursor::new(data);
            let mut decoder2 = png::Decoder::new(cursor2);
            decoder2.set_transformations(png::Transformations::EXPAND);
            let mut reader2 = decoder2
                .read_info()
                .map_err(|e| CodecError::Decode(format!("PNG decode error: {}", e)))?;

            let buf_size = reader2
                .output_buffer_size()
                .ok_or_else(|| CodecError::Decode("PNG image too large to decode".to_string()))?;
            let mut buf = vec![0u8; buf_size];
            let out_info = reader2
                .next_frame(&mut buf)
                .map_err(|e| CodecError::Decode(format!("PNG decode error: {}", e)))?;
            buf.truncate(out_info.buffer_size());
            (out_info, buf)
        } else {
            let buf_size = reader
                .output_buffer_size()
                .ok_or_else(|| CodecError::Decode("PNG image too large to decode".to_string()))?;
            let mut buf = vec![0u8; buf_size];
            let out_info = reader
                .next_frame(&mut buf)
                .map_err(|e| CodecError::Decode(format!("PNG decode error: {}", e)))?;
            buf.truncate(out_info.buffer_size());
            (out_info, buf)
        };

        // Determine pixel type, num_bands, and bit_depth value
        let (pixel_type, num_bands, bit_depth_val) =
            Self::classify_output(&output_info, color_type, bit_depth)?;

        // Convert interleaved row-major pixels to BSQ format
        let bsq_pixels = Self::interleaved_to_bsq(
            &raw_pixels,
            width,
            height,
            num_bands,
            pixel_type,
        );

        let metadata = Arc::new(PNGMetadataProvider::new(metadata_entries));

        let image_asset = PNGImageAssetProvider::new(
            "image:0".to_string(),
            width,
            height,
            num_bands,
            pixel_type,
            bit_depth_val,
            bsq_pixels,
            vec!["data".to_string()],
            metadata.clone(),
        );

        Ok(Self {
            image_asset: Some(Arc::new(image_asset)),
            metadata,
        })
    }

    /// Classify the decoded output into PixelType, num_bands, and bit_depth value.
    fn classify_output(
        output_info: &png::OutputInfo,
        _original_color_type: png::ColorType,
        original_bit_depth: png::BitDepth,
    ) -> Result<(PixelType, u32, u8), CodecError> {
        let out_bit_depth = output_info.bit_depth;
        let out_color_type = output_info.color_type;

        let pixel_type = match out_bit_depth {
            png::BitDepth::Eight => PixelType::UInt8,
            png::BitDepth::Sixteen => PixelType::UInt16,
            // Sub-byte after EXPAND should be 8-bit
            png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four => PixelType::UInt8,
        };

        let num_bands = match out_color_type {
            png::ColorType::Grayscale => 1,
            png::ColorType::Rgb => 3,
            png::ColorType::Indexed => 1, // raw palette indices
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgba => 4,
        };

        // Report the original bit depth for metadata
        let bit_depth_val = match original_bit_depth {
            png::BitDepth::One => 1,
            png::BitDepth::Two => 2,
            png::BitDepth::Four => 4,
            png::BitDepth::Eight => 8,
            png::BitDepth::Sixteen => 16,
        };

        Ok((pixel_type, num_bands, bit_depth_val))
    }

    /// Extract metadata from PNG info into a HashMap.
    fn extract_metadata(info: &png::Info) -> HashMap<String, Value> {
        let mut entries = HashMap::new();

        // Dataset-level metadata
        entries.insert("width".to_string(), json!(info.width));
        entries.insert("height".to_string(), json!(info.height));

        let bit_depth_val: u8 = match info.bit_depth {
            png::BitDepth::One => 1,
            png::BitDepth::Two => 2,
            png::BitDepth::Four => 4,
            png::BitDepth::Eight => 8,
            png::BitDepth::Sixteen => 16,
        };
        entries.insert("bit_depth".to_string(), json!(bit_depth_val));

        let color_type_str = match info.color_type {
            png::ColorType::Grayscale => "Grayscale",
            png::ColorType::Rgb => "RGB",
            png::ColorType::Indexed => "Indexed",
            png::ColorType::GrayscaleAlpha => "GrayscaleAlpha",
            png::ColorType::Rgba => "RGBA",
        };
        entries.insert("color_type".to_string(), json!(color_type_str));

        // tEXt chunks
        for chunk in &info.uncompressed_latin1_text {
            entries.insert(chunk.keyword.clone(), json!(chunk.text));
        }

        // zTXt chunks
        for chunk in &info.compressed_latin1_text {
            if let Ok(text) = chunk.get_text() {
                entries.insert(chunk.keyword.clone(), json!(text));
            }
        }

        // iTXt chunks
        for chunk in &info.utf8_text {
            if let Ok(text) = chunk.get_text() {
                entries.insert(chunk.keyword.clone(), json!(text));
            }
        }

        // gAMA chunk
        if let Some(gamma) = info.source_gamma {
            entries.insert("gAMA".to_string(), json!(gamma.into_value()));
        }

        // pHYs chunk
        if let Some(ref dims) = info.pixel_dims {
            let unit_val: u8 = match dims.unit {
                png::Unit::Meter => 1,
                png::Unit::Unspecified => 0,
            };
            entries.insert(
                "pHYs".to_string(),
                json!({
                    "x": dims.xppu,
                    "y": dims.yppu,
                    "unit": unit_val,
                }),
            );
        }

        // PLTE chunk
        if let Some(ref palette) = info.palette {
            let triples: Vec<Value> = palette
                .chunks(3)
                .map(|rgb| json!([rgb[0], rgb[1], rgb[2]]))
                .collect();
            entries.insert("PLTE".to_string(), json!(triples));
        }

        entries
    }

    /// Convert interleaved row-major pixels to BSQ (band-sequential) format.
    ///
    /// Input: `[R0G0B0 R1G1B1 ...]` per row (from png crate)
    /// Output: all band-0 pixels, then band-1, etc.
    ///
    /// For UInt16, the png crate outputs big-endian 2-byte samples.
    /// We convert to native-endian during the BSQ conversion.
    fn interleaved_to_bsq(
        interleaved: &[u8],
        width: u32,
        height: u32,
        num_bands: u32,
        pixel_type: PixelType,
    ) -> Vec<u8> {
        let num_pixels = (width as usize) * (height as usize);
        let bps = pixel_type.bytes_per_pixel(); // bytes per sample
        let total_bytes = num_pixels * num_bands as usize * bps;

        if num_bands == 1 {
            // Single band: no interleaving to undo, just handle endianness
            if bps == 2 {
                return Self::convert_be_to_ne_u16(interleaved, num_pixels);
            }
            return interleaved.to_vec();
        }

        let mut bsq = vec![0u8; total_bytes];
        let band_size = num_pixels * bps; // bytes per band in output

        if bps == 1 {
            // UInt8: simple byte shuffle
            let samples_per_row = width as usize * num_bands as usize;
            for row in 0..height as usize {
                let row_start = row * samples_per_row;
                for col in 0..width as usize {
                    let pixel_offset = row_start + col * num_bands as usize;
                    let linear_idx = row * width as usize + col;
                    for band in 0..num_bands as usize {
                        bsq[band * num_pixels + linear_idx] =
                            interleaved[pixel_offset + band];
                    }
                }
            }
        } else {
            // UInt16: 2 bytes per sample, big-endian from png crate → native endian
            let samples_per_row = width as usize * num_bands as usize;
            for row in 0..height as usize {
                let row_byte_start = row * samples_per_row * 2;
                for col in 0..width as usize {
                    let pixel_byte_offset =
                        row_byte_start + col * num_bands as usize * 2;
                    let linear_idx = row * width as usize + col;
                    for band in 0..num_bands as usize {
                        let src = pixel_byte_offset + band * 2;
                        let be_val =
                            u16::from_be_bytes([interleaved[src], interleaved[src + 1]]);
                        let ne_bytes = be_val.to_ne_bytes();
                        let dst = band * band_size + linear_idx * 2;
                        bsq[dst] = ne_bytes[0];
                        bsq[dst + 1] = ne_bytes[1];
                    }
                }
            }
        }

        bsq
    }

    /// Convert a buffer of big-endian u16 samples to native-endian.
    fn convert_be_to_ne_u16(data: &[u8], num_samples: usize) -> Vec<u8> {
        let mut out = vec![0u8; num_samples * 2];
        for i in 0..num_samples {
            let be_val = u16::from_be_bytes([data[i * 2], data[i * 2 + 1]]);
            let ne = be_val.to_ne_bytes();
            out[i * 2] = ne[0];
            out[i * 2 + 1] = ne[1];
        }
        out
    }
}

// =============================================================================
// DatasetReader Implementation
// =============================================================================

impl DatasetReader for PNGDatasetReader {
    fn get_asset(&self, key: &str) -> Result<AssetProvider, CodecError> {
        match &self.image_asset {
            Some(asset) if asset.key() == key => {
                Ok(AssetProvider::Image(asset.clone() as Arc<dyn crate::traits::image::ImageAssetProvider>))
            }
            _ => Err(CodecError::AssetNotFound(key.to_string())),
        }
    }

    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<&[String]>,
    ) -> Vec<String> {
        match asset_type {
            None | Some(AssetType::Image) => {
                match &self.image_asset {
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
                }
            }
            Some(AssetType::Text) | Some(AssetType::Graphics) | Some(AssetType::Data) => {
                Vec::new()
            }
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
    use crate::traits::image::ImageAssetProvider;

    /// Helper: create a minimal valid PNG in memory using the `png` crate encoder.
    fn make_png(
        width: u32,
        height: u32,
        color_type: png::ColorType,
        bit_depth: png::BitDepth,
        pixels: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, width, height);
            encoder.set_color(color_type);
            encoder.set_depth(bit_depth);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        buf
    }

    /// Helper: create a simple 2x2 grayscale 8-bit PNG.
    fn make_gray_2x2() -> Vec<u8> {
        // Interleaved (trivial for 1 band): row0=[10,20], row1=[30,40]
        make_png(2, 2, png::ColorType::Grayscale, png::BitDepth::Eight, &[10, 20, 30, 40])
    }

    // =========================================================================
    // test_from_bytes_valid_png (Req 2.2)
    // =========================================================================

    #[test]
    fn test_from_bytes_valid_png() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();

        assert!(reader.image_asset.is_some());
        assert!(reader.has_asset("image:0"));
        assert!(!reader.has_asset("image:1"));

        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image:0"]);
    }

    // =========================================================================
    // test_from_bytes_invalid_signature (Req 2.3, 2.4)
    // =========================================================================

    #[test]
    fn test_from_bytes_invalid_signature() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let result = PNGDatasetReader::from_bytes(&data);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("invalid signature"));
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_from_bytes_empty_data (Req 2.3)
    // =========================================================================

    #[test]
    fn test_from_bytes_empty_data() {
        let result = PNGDatasetReader::from_bytes(&[]);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("invalid signature"));
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_from_bytes_truncated (Req 2.3)
    // =========================================================================

    #[test]
    fn test_from_bytes_truncated() {
        // Valid signature but truncated data
        let mut data = PNG_SIGNATURE.to_vec();
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // partial IHDR length
        let result = PNGDatasetReader::from_bytes(&data);
        assert!(result.is_err());
        match result {
            Err(CodecError::Decode(msg)) => {
                assert!(msg.contains("PNG decode error"));
            }
            other => panic!("Expected Decode error, got: {:?}", other),
        }
    }

    // =========================================================================
    // test_get_asset_valid_key (Req 2.5)
    // =========================================================================

    #[test]
    fn test_get_asset_valid_key() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0");
        assert!(asset.is_ok());
        assert_eq!(asset.unwrap().key(), "image:0");
    }

    // =========================================================================
    // test_get_asset_invalid_key (Req 2.5)
    // =========================================================================

    #[test]
    fn test_get_asset_invalid_key() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let result = reader.get_asset("nonexistent");
        match result {
            Err(CodecError::AssetNotFound(key)) => assert_eq!(key, "nonexistent"),
            other => panic!("Expected AssetNotFound, got: {}", other.is_ok()),
        }
    }

    // =========================================================================
    // test_get_asset_keys_image (Req 2.6)
    // =========================================================================

    #[test]
    fn test_get_asset_keys_image() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image:0"]);

        // None also returns image keys
        let keys_none = reader.get_asset_keys(None, None);
        assert_eq!(keys_none, vec!["image:0"]);
    }

    // =========================================================================
    // test_get_asset_keys_text_empty (Req 2.7)
    // =========================================================================

    #[test]
    fn test_get_asset_keys_text_empty() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        assert!(reader.get_asset_keys(Some(AssetType::Text), None).is_empty());
        assert!(reader.get_asset_keys(Some(AssetType::Data), None).is_empty());
        assert!(reader
            .get_asset_keys(Some(AssetType::Graphics), None)
            .is_empty());
    }

    // =========================================================================
    // test_close_clears_assets (Req 2.5)
    // =========================================================================

    #[test]
    fn test_close_clears_assets() {
        let data = make_gray_2x2();
        let mut reader = PNGDatasetReader::from_bytes(&data).unwrap();
        assert!(reader.has_asset("image:0"));

        reader.close().unwrap();
        assert!(!reader.has_asset("image:0"));
        assert!(reader.get_asset("image:0").is_err());
        assert!(reader.get_asset_keys(Some(AssetType::Image), None).is_empty());
    }

    // =========================================================================
    // Pixel decode + BSQ conversion tests
    // =========================================================================

    #[test]
    fn test_grayscale_8bit_pixels() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert_eq!(image.num_columns(), 2);
        assert_eq!(image.num_rows(), 2);
        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.pixel_value_type(), PixelType::UInt8);

        let (pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 2]);
        assert_eq!(pixels, vec![10, 20, 30, 40]);
    }

    #[test]
    fn test_rgb_8bit_bsq_conversion() {
        // 2x2 RGB: interleaved = [R0G0B0 R1G1B1 R2G2B2 R3G3B3]
        let interleaved: Vec<u8> = vec![
            1, 2, 3, // pixel (0,0)
            4, 5, 6, // pixel (0,1)
            7, 8, 9, // pixel (1,0)
            10, 11, 12, // pixel (1,1)
        ];
        let data = make_png(2, 2, png::ColorType::Rgb, png::BitDepth::Eight, &interleaved);
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert_eq!(image.num_bands(), 3);
        assert_eq!(image.pixel_value_type(), PixelType::UInt8);

        let (pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 2, 2]);
        // BSQ: band0=[1,4,7,10], band1=[2,5,8,11], band2=[3,6,9,12]
        assert_eq!(pixels, vec![1, 4, 7, 10, 2, 5, 8, 11, 3, 6, 9, 12]);
    }

    #[test]
    fn test_rgba_8bit_bsq_conversion() {
        // 2x1 RGBA
        let interleaved: Vec<u8> = vec![
            10, 20, 30, 40, // pixel (0,0)
            50, 60, 70, 80, // pixel (0,1)
        ];
        let data = make_png(2, 1, png::ColorType::Rgba, png::BitDepth::Eight, &interleaved);
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert_eq!(image.num_bands(), 4);
        let (pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [4, 1, 2]);
        // BSQ: R=[10,50], G=[20,60], B=[30,70], A=[40,80]
        assert_eq!(pixels, vec![10, 50, 20, 60, 30, 70, 40, 80]);
    }

    #[test]
    fn test_grayscale_16bit_endian_conversion() {
        // 2x1 grayscale 16-bit, big-endian from png crate
        // Values: 256 (0x0100), 512 (0x0200)
        let interleaved: Vec<u8> = vec![0x01, 0x00, 0x02, 0x00];
        let data = make_png(
            2,
            1,
            png::ColorType::Grayscale,
            png::BitDepth::Sixteen,
            &interleaved,
        );
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.pixel_value_type(), PixelType::UInt16);

        let (pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 1, 2]);
        // Should be native-endian u16 values 256 and 512
        let val0 = u16::from_ne_bytes([pixels[0], pixels[1]]);
        let val1 = u16::from_ne_bytes([pixels[2], pixels[3]]);
        assert_eq!(val0, 256);
        assert_eq!(val1, 512);
    }

    #[test]
    fn test_rgb_16bit_bsq_conversion() {
        // 1x2 RGB 16-bit
        // Pixel (0,0): R=100, G=200, B=300
        // Pixel (1,0): R=400, G=500, B=600
        let mut interleaved = Vec::new();
        for &v in &[100u16, 200, 300, 400, 500, 600] {
            interleaved.extend_from_slice(&v.to_be_bytes());
        }
        let data = make_png(1, 2, png::ColorType::Rgb, png::BitDepth::Sixteen, &interleaved);
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert_eq!(image.num_bands(), 3);
        assert_eq!(image.pixel_value_type(), PixelType::UInt16);

        let (pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 2, 1]);

        // BSQ: R=[100,400], G=[200,500], B=[300,600] in native endian
        let read_u16 = |offset: usize| -> u16 {
            u16::from_ne_bytes([pixels[offset], pixels[offset + 1]])
        };
        // Band 0 (R)
        assert_eq!(read_u16(0), 100);
        assert_eq!(read_u16(2), 400);
        // Band 1 (G)
        assert_eq!(read_u16(4), 200);
        assert_eq!(read_u16(6), 500);
        // Band 2 (B)
        assert_eq!(read_u16(8), 300);
        assert_eq!(read_u16(10), 600);
    }

    // =========================================================================
    // Metadata extraction test
    // =========================================================================

    #[test]
    fn test_metadata_contains_dataset_level() {
        let data = make_gray_2x2();
        let reader = PNGDatasetReader::from_bytes(&data).unwrap();
        let meta = reader.metadata();
        let dict = meta.as_dict(None);

        assert_eq!(dict.get("width").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(dict.get("height").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(dict.get("bit_depth").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(
            dict.get("color_type").and_then(|v| v.as_str()),
            Some("Grayscale")
        );
    }

    // =========================================================================
    // Indexed PNG test (Req 3.18)
    // =========================================================================

    #[test]
    fn test_indexed_png_returns_raw_indices() {
        // Create a 2x2 indexed PNG with palette
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, 2, 2);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![
                255, 0, 0, // index 0 = red
                0, 255, 0, // index 1 = green
                0, 0, 255, // index 2 = blue
            ]);
            let mut writer = encoder.write_header().unwrap();
            // Pixel indices: [0, 1, 2, 0]
            writer.write_image_data(&[0, 1, 2, 0]).unwrap();
        }

        let reader = PNGDatasetReader::from_bytes(&buf).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset.as_image().expect("expected Image variant");

        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.pixel_value_type(), PixelType::UInt8);

        let (pixels, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 2, 2]);
        // Raw palette indices, not expanded RGB
        assert_eq!(pixels, vec![0, 1, 2, 0]);

        // PLTE should be in metadata
        let meta = reader.metadata();
        let dict = meta.as_dict(None);
        assert!(dict.contains_key("PLTE"));
    }
}
