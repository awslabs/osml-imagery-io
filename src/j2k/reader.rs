//! J2KDatasetReader — implements DatasetReader for standalone JPEG 2000 files.
//!
//! Opens a `.j2k` or `.jp2` file from a byte slice, validates the signature,
//! parses the SIZ marker to extract metadata (dimensions, bands, bit depth,
//! tile grid), and exposes a single image asset keyed as `"image:0"`.
//! Pixel decoding is deferred to `get_block()` time on the ImageAssetProvider.
//!
//! The entire input buffer is stored once as `Arc<[u8]>`. For JP2 files the
//! codestream byte range within that buffer is tracked so the image provider
//! can slice into it at decode time without an extra copy.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use serde_json::json;

use crate::error::CodecError;
use crate::j2k::codec::J2KCodec;
use crate::j2k::image::J2KImageAssetProvider;
use crate::j2k::markers::parse_main_header;
use crate::j2k::metadata::J2KMetadataProvider;
use crate::traits::asset::AssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::traits::reader::DatasetReader;
use crate::types::{AssetType, PixelType};

#[cfg(feature = "openjpeg")]
use crate::j2k::openjpeg::get_j2k_codec;

/// JP2 file signature box (first 12 bytes of a JP2 file).
/// Box length (4) + box type "jP  " (4) + signature (4).
const JP2_SIGNATURE: [u8; 12] = [
    0x00, 0x00, 0x00, 0x0C, // Box length = 12
    0x6A, 0x50, 0x20, 0x20, // Box type = "jP  "
    0x0D, 0x0A, 0x87, 0x0A, // JP2 signature
];

/// J2K SOC (Start of Codestream) marker.
const J2K_SOC: [u8; 2] = [0xFF, 0x4F];

/// JP2 contiguous codestream box type ("jp2c").
const JP2C_BOX_TYPE: [u8; 4] = [0x6A, 0x70, 0x32, 0x63];

/// JPEG 2000 dataset reader implementing the `DatasetReader` trait.
///
/// Owns a single image asset provider and dataset-level metadata.
/// Metadata is extracted eagerly during `from_bytes`; pixel decoding
/// is deferred to `get_block()` calls on the image asset provider.
pub struct J2KDatasetReader {
    image_asset: Option<Arc<J2KImageAssetProvider>>,
    metadata: Arc<J2KMetadataProvider>,
}

impl std::fmt::Debug for J2KDatasetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("J2KDatasetReader")
            .field("has_image", &self.image_asset.is_some())
            .finish()
    }
}

impl J2KDatasetReader {
    /// Construct from a raw byte slice.
    ///
    /// Validates the SOC marker (0xFF4F) for raw J2K codestreams or the JP2
    /// file signature for JP2 containers. Parses the SIZ marker to extract
    /// metadata (dimensions, bands, bit depth, tile grid). Does NOT decode
    /// pixel data — that is deferred to `get_block()` calls on the
    /// `ImageAssetProvider`.
    ///
    /// The input is copied once into an `Arc<[u8]>` that is shared with the
    /// image asset provider. For JP2 files the codestream byte range within
    /// that buffer is tracked so no additional copy is needed.
    #[cfg(feature = "openjpeg")]
    pub fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        let codec = get_j2k_codec();
        Self::from_bytes_with_codec(data, codec)
    }

    /// Construct from a raw byte slice using a specific codec.
    pub(crate) fn from_bytes_with_codec(
        data: &[u8],
        codec: Arc<dyn J2KCodec>,
    ) -> Result<Self, CodecError> {
        // Validate minimum length
        if data.len() < 2 {
            return Err(CodecError::InvalidFormat(
                "Not a valid JPEG 2000 file: too short".to_string(),
            ));
        }

        // Detect format and locate the codestream byte range.
        // We work on the raw slice first (before creating the Arc) so that
        // signature checks and SIZ parsing don't require an allocation.
        let (cs_range, compression_type) = if data.len() >= 12 && data[..12] == JP2_SIGNATURE {
            let range = Self::find_jp2_codestream_range(data)?;
            (range, "jp2")
        } else if data[..2] == J2K_SOC {
            (0..data.len(), "j2k")
        } else {
            return Err(CodecError::InvalidFormat(
                "Not a valid JPEG 2000 file: invalid signature".to_string(),
            ));
        };

        let codestream = &data[cs_range.clone()];

        // Parse SIZ marker for metadata
        let siz = Self::parse_siz_marker(codestream)?;

        // Get tile info and resolution levels from codec
        let (tile_width, tile_height, num_tiles_x, num_tiles_y) =
            codec.get_tile_info(codestream)?;
        let num_resolution_levels = codec.get_resolution_levels(codestream)?;

        // Determine pixel type from SIZ marker info
        let pixel_type = Self::pixel_type_from_siz(siz.bits_per_component, siz.is_signed);

        // Build metadata entries
        let mut entries = HashMap::new();
        entries.insert("width".to_string(), json!(siz.width));
        entries.insert("height".to_string(), json!(siz.height));
        entries.insert("num_components".to_string(), json!(siz.num_components));
        entries.insert("bits_per_component".to_string(), json!(siz.bits_per_component));
        entries.insert("is_signed".to_string(), json!(siz.is_signed));
        entries.insert("tile_width".to_string(), json!(tile_width));
        entries.insert("tile_height".to_string(), json!(tile_height));
        entries.insert("num_tiles_x".to_string(), json!(num_tiles_x));
        entries.insert("num_tiles_y".to_string(), json!(num_tiles_y));
        entries.insert("compression_type".to_string(), json!(compression_type));

        let metadata = Arc::new(J2KMetadataProvider::new(entries));

        // Single allocation: wrap the entire input buffer in an Arc.
        // The image provider will slice into it using cs_range.
        let buffer: Arc<[u8]> = Arc::from(data);

        // Parse main header for tile-part extraction
        let codestream_bytes = &buffer[cs_range.clone()];
        let header_info = parse_main_header(codestream_bytes)?;
        let tile_part_table = std::sync::OnceLock::new();
        if let Some(tlm_table) = header_info.tlm_offset_table {
            let _ = tile_part_table.set(tlm_table);
        }

        let image_asset = J2KImageAssetProvider::new(
            "image:0".to_string(),
            siz.width,
            siz.height,
            siz.num_components,
            pixel_type,
            siz.bits_per_component,
            tile_width,
            tile_height,
            num_tiles_x,
            num_tiles_y,
            buffer,
            cs_range,
            vec!["data".to_string()],
            metadata.clone(),
            num_resolution_levels,
            codec,
            header_info.decode_header,
            header_info.first_sot_offset,
            tile_part_table,
        );

        Ok(Self {
            image_asset: Some(Arc::new(image_asset)),
            metadata,
        })
    }

    /// Locate the raw J2K codestream byte range within a JP2 container.
    ///
    /// Scans JP2 boxes to find the contiguous codestream box (`jp2c`) and
    /// returns the byte range of its contents. No data is copied.
    fn find_jp2_codestream_range(data: &[u8]) -> Result<Range<usize>, CodecError> {
        let mut pos = 0;

        while pos + 8 <= data.len() {
            let box_len = u32::from_be_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
            ]) as u64;
            let box_type = &data[pos + 4..pos + 8];

            let (header_size, actual_len) = if box_len == 1 {
                // Extended length box
                if pos + 16 > data.len() {
                    return Err(CodecError::InvalidFormat(
                        "Not a valid JP2 file: truncated extended box header".to_string(),
                    ));
                }
                let ext_len = u64::from_be_bytes([
                    data[pos + 8],
                    data[pos + 9],
                    data[pos + 10],
                    data[pos + 11],
                    data[pos + 12],
                    data[pos + 13],
                    data[pos + 14],
                    data[pos + 15],
                ]);
                (16usize, ext_len)
            } else if box_len == 0 {
                // Box extends to end of file
                (8usize, (data.len() - pos) as u64)
            } else {
                (8usize, box_len)
            };

            if box_type == JP2C_BOX_TYPE {
                let content_start = pos + header_size;
                let content_end = (pos as u64 + actual_len) as usize;
                // Clamp to file length (jp2c is often the last box)
                let content_end = content_end.min(data.len());
                return Ok(content_start..content_end);
            }

            if actual_len < header_size as u64 {
                break;
            }
            pos += actual_len as usize;
        }

        Err(CodecError::InvalidFormat(
            "Not a valid JP2 file: no codestream box (jp2c) found".to_string(),
        ))
    }

    /// Parse the SIZ marker from a raw J2K codestream to extract metadata.
    fn parse_siz_marker(codestream: &[u8]) -> Result<SizMarkerInfo, CodecError> {
        let mut pos = 2; // Skip SOC marker
        while pos + 2 <= codestream.len() {
            if codestream[pos] == 0xFF && codestream[pos + 1] == 0x51 {
                // SIZ marker found — need marker(2) + Lsiz(2) + Rsiz(2) +
                // Xsiz(4) + Ysiz(4) + XOsiz(4) + YOsiz(4) + XTsiz(4) +
                // YTsiz(4) + XTOsiz(4) + YTOsiz(4) + Csiz(2) + Ssiz(1) = 41
                if pos + 41 > codestream.len() {
                    return Err(CodecError::InvalidFormat(
                        "Not a valid JPEG 2000 file: SIZ marker truncated".to_string(),
                    ));
                }

                let xsiz = u32::from_be_bytes([
                    codestream[pos + 6],
                    codestream[pos + 7],
                    codestream[pos + 8],
                    codestream[pos + 9],
                ]);
                let ysiz = u32::from_be_bytes([
                    codestream[pos + 10],
                    codestream[pos + 11],
                    codestream[pos + 12],
                    codestream[pos + 13],
                ]);
                let xosiz = u32::from_be_bytes([
                    codestream[pos + 14],
                    codestream[pos + 15],
                    codestream[pos + 16],
                    codestream[pos + 17],
                ]);
                let yosiz = u32::from_be_bytes([
                    codestream[pos + 18],
                    codestream[pos + 19],
                    codestream[pos + 20],
                    codestream[pos + 21],
                ]);

                let csiz = u16::from_be_bytes([codestream[pos + 38], codestream[pos + 39]]);

                let ssiz = codestream[pos + 40];
                let is_signed = (ssiz & 0x80) != 0;
                let bits_per_component = (ssiz & 0x7F) + 1;

                return Ok(SizMarkerInfo {
                    width: xsiz - xosiz,
                    height: ysiz - yosiz,
                    num_components: csiz as u32,
                    bits_per_component,
                    is_signed,
                });
            }

            // Skip to next marker
            if codestream[pos] == 0xFF {
                if pos + 4 > codestream.len() {
                    break;
                }
                let marker_len =
                    u16::from_be_bytes([codestream[pos + 2], codestream[pos + 3]]) as usize;
                pos += 2 + marker_len;
            } else {
                pos += 1;
            }
        }

        Err(CodecError::InvalidFormat(
            "Not a valid JPEG 2000 file: SIZ marker not found".to_string(),
        ))
    }

    /// Map SIZ marker bit depth and signedness to a PixelType.
    fn pixel_type_from_siz(bits_per_component: u8, is_signed: bool) -> PixelType {
        match (bits_per_component, is_signed) {
            (1..=8, false) => PixelType::UInt8,
            (1..=8, true) => PixelType::Int8,
            (9..=16, false) => PixelType::UInt16,
            (9..=16, true) => PixelType::Int16,
            (17..=32, false) => PixelType::UInt32,
            (17..=32, true) => PixelType::Int32,
            _ => PixelType::UInt8, // fallback
        }
    }
}

/// Parsed SIZ marker information.
struct SizMarkerInfo {
    width: u32,
    height: u32,
    num_components: u32,
    bits_per_component: u8,
    is_signed: bool,
}

// =============================================================================
// DatasetReader Implementation
// =============================================================================

impl DatasetReader for J2KDatasetReader {
    fn get_asset(&self, key: &str) -> Result<Arc<dyn AssetProvider>, CodecError> {
        match &self.image_asset {
            Some(asset) if asset.key() == key => Ok(asset.clone()),
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

    // =========================================================================
    // Signature validation tests
    //
    // These use from_bytes_with_codec with a stub codec so they compile and
    // run regardless of feature flags. The validation logic rejects the input
    // before any codec method is called.
    // =========================================================================

    use crate::j2k::codec::{
        J2KCodecCapabilities, J2KDecodeParams, J2KDecodeResult, J2KEncodeParams, J2KEncodeState,
    };

    /// Stub codec for tests that exercise pre-codec validation paths.
    /// All methods panic because they should never be reached.
    struct StubCodec;

    impl J2KCodec for StubCodec {
        fn capabilities(&self) -> J2KCodecCapabilities {
            unimplemented!("stub: not expected to be called")
        }
        fn decode(&self, _: &[u8], _: &J2KDecodeParams) -> Result<J2KDecodeResult, CodecError> {
            unimplemented!("stub: not expected to be called")
        }
        fn start_encode(&self, _: &J2KEncodeParams) -> Result<Box<dyn J2KEncodeState>, CodecError> {
            unimplemented!("stub: not expected to be called")
        }
        fn get_resolution_levels(&self, _: &[u8]) -> Result<u32, CodecError> {
            unimplemented!("stub: not expected to be called")
        }
        fn get_dimensions(&self, _: &[u8]) -> Result<(u32, u32, u32), CodecError> {
            unimplemented!("stub: not expected to be called")
        }
        fn get_tile_info(&self, _: &[u8]) -> Result<(u32, u32, u32, u32), CodecError> {
            unimplemented!("stub: not expected to be called")
        }
        fn decode_tile(&self, _: &[u8], _: u32, _: &J2KDecodeParams) -> Result<J2KDecodeResult, CodecError> {
            unimplemented!("stub: not expected to be called")
        }
    }

    fn stub_codec() -> Arc<dyn J2KCodec> {
        Arc::new(StubCodec)
    }

    #[test]
    fn test_from_bytes_empty_data() {
        let result = J2KDatasetReader::from_bytes_with_codec(&[], stub_codec());
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("too short"), "got: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_single_byte() {
        let result = J2KDatasetReader::from_bytes_with_codec(&[0xFF], stub_codec());
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
        let result = J2KDatasetReader::from_bytes_with_codec(&data, stub_codec());
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("invalid signature"), "got: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    // =========================================================================
    // JP2 box parsing tests
    // =========================================================================

    #[test]
    fn test_find_jp2_codestream_range_no_jp2c() {
        // Valid JP2 signature but no jp2c box
        let mut data = JP2_SIGNATURE.to_vec();
        // Add a dummy box (ftyp, 20 bytes total)
        data.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x14, // length = 20
            0x66, 0x74, 0x79, 0x70, // type = "ftyp"
            0x6A, 0x70, 0x32, 0x20, // brand = "jp2 "
            0x00, 0x00, 0x00, 0x00, // minor version
            0x6A, 0x70, 0x32, 0x20, // compat = "jp2 "
        ]);
        let result = J2KDatasetReader::find_jp2_codestream_range(&data);
        assert!(result.is_err());
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("jp2c"), "got: {}", msg);
            }
            _ => panic!("Expected InvalidFormat"),
        }
    }

    #[test]
    fn test_find_jp2_codestream_range_valid() {
        let mut data = JP2_SIGNATURE.to_vec();
        // Add a jp2c box with a small codestream
        let codestream = [0xFF, 0x4F, 0xFF, 0xD9]; // SOC + EOC
        let box_len = (8 + codestream.len()) as u32;
        data.extend_from_slice(&box_len.to_be_bytes());
        data.extend_from_slice(&JP2C_BOX_TYPE);
        data.extend_from_slice(&codestream);

        let range = J2KDatasetReader::find_jp2_codestream_range(&data).unwrap();
        assert_eq!(&data[range.clone()], &codestream);
        // Verify it's a zero-copy range into the original buffer
        assert_eq!(range.start, 12 + 8); // JP2 sig (12) + box header (8)
        assert_eq!(range.end, range.start + codestream.len());
    }

    // =========================================================================
    // SIZ marker parsing tests
    // =========================================================================

    #[test]
    fn test_parse_siz_marker_truncated() {
        // SOC + partial SIZ marker
        let data = [0xFF, 0x4F, 0xFF, 0x51, 0x00, 0x10];
        let result = J2KDatasetReader::parse_siz_marker(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_siz_marker_not_found() {
        // SOC + COD marker (not SIZ)
        let mut data = vec![0xFF, 0x4F]; // SOC
        data.extend_from_slice(&[0xFF, 0x52]); // COD marker
        data.extend_from_slice(&[0x00, 0x04]); // length = 4
        data.extend_from_slice(&[0x00, 0x00]); // dummy content
        let result = J2KDatasetReader::parse_siz_marker(&data);
        assert!(result.is_err());
    }

    // =========================================================================
    // pixel_type_from_siz tests
    // =========================================================================

    #[test]
    fn test_pixel_type_mapping() {
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(8, false),
            PixelType::UInt8
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(8, true),
            PixelType::Int8
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(16, false),
            PixelType::UInt16
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(16, true),
            PixelType::Int16
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(32, false),
            PixelType::UInt32
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(32, true),
            PixelType::Int32
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(12, false),
            PixelType::UInt16
        );
        assert_eq!(
            J2KDatasetReader::pixel_type_from_siz(1, false),
            PixelType::UInt8
        );
    }

    // =========================================================================
    // DatasetReader trait tests (using real codec)
    // =========================================================================

    /// Helper: encode a small test image as a raw J2K codestream.
    ///
    /// Uses 64×64 minimum tile size to satisfy OpenJPEG's code-block and
    /// decomposition level constraints. Decomposition levels are set to 0
    /// (single resolution) to keep the codestream minimal.
    #[cfg(feature = "openjpeg")]
    fn make_j2k_codestream(
        width: u32,
        height: u32,
        num_components: u32,
        bits: u8,
        is_signed: bool,
        pixel_data: &[u8],
    ) -> Vec<u8> {
        use crate::j2k::codec::J2KEncodeParams;
        use crate::j2k::openjpeg::get_j2k_codec;

        let codec = get_j2k_codec();
        let mut params = J2KEncodeParams {
            width,
            height,
            num_components,
            bits_per_component: bits,
            is_signed,
            lossless: true,
            compression_ratio: None,
            num_decomposition_levels: 5,
            num_quality_layers: 1,
            htj2k: false,
            tile_width: width,
            tile_height: height,
        };
        params.clamp_decomposition_levels();
        let mut state = codec.start_encode(&params).unwrap();
        state.encode_tile(0, pixel_data).unwrap();
        state.finalize().unwrap()
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_roundtrip_grayscale_8bit() {
        // 64x64 grayscale
        let npix = 64 * 64;
        let pixels: Vec<u8> = (0..npix).map(|i| (i % 256) as u8).collect();
        let cs = make_j2k_codestream(64, 64, 1, 8, false, &pixels);

        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();
        assert!(reader.has_asset("image:0"));
        assert!(!reader.has_asset("nonexistent"));

        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image:0"]);
        assert!(reader.get_asset_keys(Some(AssetType::Text), None).is_empty());

        // Check metadata
        let meta = reader.metadata();
        let dict = meta.as_dict(None);
        assert_eq!(dict.get("width").and_then(|v| v.as_u64()), Some(64));
        assert_eq!(dict.get("height").and_then(|v| v.as_u64()), Some(64));
        assert_eq!(dict.get("num_components").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(dict.get("bits_per_component").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(dict.get("is_signed").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(dict.get("compression_type").and_then(|v| v.as_str()), Some("j2k"));

        // Decode and verify pixels
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();
        assert_eq!(image.num_columns(), 64);
        assert_eq!(image.num_rows(), 64);
        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.pixel_value_type(), PixelType::UInt8);

        let (data, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(data, pixels);
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_roundtrip_rgb_8bit() {
        // 64x64 RGB in BSQ: 3 bands × 4096 pixels
        let npix = 64 * 64usize;
        let mut pixels = Vec::with_capacity(npix * 3);
        for band in 0u8..3 {
            for i in 0..npix {
                pixels.push(band.wrapping_mul(80).wrapping_add((i % 256) as u8));
            }
        }
        let cs = make_j2k_codestream(64, 64, 3, 8, false, &pixels);

        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();

        assert_eq!(image.num_bands(), 3);
        let (data, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [3, 64, 64]);
        assert_eq!(data, pixels);
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_get_asset_invalid_key() {
        let pixels: Vec<u8> = vec![0; 64 * 64];
        let cs = make_j2k_codestream(64, 64, 1, 8, false, &pixels);
        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();

        match reader.get_asset("nonexistent") {
            Err(CodecError::AssetNotFound(key)) => assert_eq!(key, "nonexistent"),
            Ok(_) => panic!("Expected AssetNotFound, got Ok"),
            Err(e) => panic!("Expected AssetNotFound, got: {}", e),
        }
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_close_clears_assets() {
        let pixels: Vec<u8> = vec![0; 64 * 64];
        let cs = make_j2k_codestream(64, 64, 1, 8, false, &pixels);
        let mut reader = J2KDatasetReader::from_bytes(&cs).unwrap();

        assert!(reader.has_asset("image:0"));
        reader.close().unwrap();
        assert!(!reader.has_asset("image:0"));
        assert!(reader.get_asset("image:0").is_err());
        assert!(reader.get_asset_keys(Some(AssetType::Image), None).is_empty());
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_invalid_block_coordinates() {
        let pixels: Vec<u8> = vec![0; 64 * 64];
        let cs = make_j2k_codestream(64, 64, 1, 8, false, &pixels);
        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();

        let err = image.get_block(1, 0, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(1, 0, 0)));

        let err = image.get_block(0, 1, 0, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidBlockCoordinates(0, 1, 0)));
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_invalid_resolution_level() {
        let pixels: Vec<u8> = vec![0; 64 * 64];
        let cs = make_j2k_codestream(64, 64, 1, 8, false, &pixels);
        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();

        let err = image.get_block(0, 0, 99, None).unwrap_err();
        assert!(matches!(err, CodecError::InvalidResolutionLevel(99)));
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_band_subset() {
        // 64x64 RGB
        let npix = 64 * 64usize;
        let mut pixels = Vec::with_capacity(npix * 3);
        for band in 0u8..3 {
            for i in 0..npix {
                pixels.push(band.wrapping_mul(80).wrapping_add((i % 256) as u8));
            }
        }
        let cs = make_j2k_codestream(64, 64, 3, 8, false, &pixels);
        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();

        // Request only band 0
        let (data, shape) = image.get_block(0, 0, 0, Some(&[0])).unwrap();
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(data, &pixels[..npix]);

        // Request bands in reverse order
        let (data, shape) = image.get_block(0, 0, 0, Some(&[2, 0])).unwrap();
        assert_eq!(shape, [2, 64, 64]);
        assert_eq!(&data[..npix], &pixels[npix * 2..npix * 3]);
        assert_eq!(&data[npix..], &pixels[..npix]);
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_has_block() {
        let pixels: Vec<u8> = vec![0; 64 * 64];
        let cs = make_j2k_codestream(64, 64, 1, 8, false, &pixels);
        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();

        assert!(image.has_block(0, 0, 0));
        assert!(!image.has_block(1, 0, 0));
        assert!(!image.has_block(0, 1, 0));
        assert!(!image.has_block(0, 0, 99));
    }

    #[cfg(feature = "openjpeg")]
    #[test]
    fn test_16bit_unsigned() {
        // 64x64 single-band UInt16
        let npix = 64u32 * 64;
        let mut pixels = Vec::new();
        for i in 0..npix as u16 {
            pixels.extend_from_slice(&i.wrapping_mul(100).to_ne_bytes());
        }
        let cs = make_j2k_codestream(64, 64, 1, 16, false, &pixels);
        let reader = J2KDatasetReader::from_bytes(&cs).unwrap();

        let meta = reader.metadata();
        let dict = meta.as_dict(None);
        assert_eq!(dict.get("bits_per_component").and_then(|v| v.as_u64()), Some(16));
        assert_eq!(dict.get("is_signed").and_then(|v| v.as_bool()), Some(false));

        let asset = reader.get_asset("image:0").unwrap();
        let image = asset
            .as_any()
            .downcast_ref::<J2KImageAssetProvider>()
            .unwrap();
        assert_eq!(image.pixel_value_type(), PixelType::UInt16);
        assert_eq!(image.num_bits_per_pixel(), 16);

        let (data, shape) = image.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 64, 64]);
        assert_eq!(data, pixels);
    }
}
