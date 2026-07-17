//! TIFFDatasetReader — implements DatasetReader for TIFF files.
//!
//! Opens a TIFF from a byte slice, enumerates IFDs, classifies each by
//! `NewSubfileType`, and creates one `TIFFImageAssetProvider` per IFD.
//! Full-resolution IFDs get keys like `image:0` with role `"data"`;
//! overview IFDs get keys like `image:0:overview:1` with role `"overview"`.
//! Transparency-mask IFDs (NewSubfileType bit 2) get a `:mask` suffix bound to
//! their associated image/overview by enumeration order (`image:0:mask`,
//! `image:0:overview:1:mask`) with roles `["mask"]` / `["overview","mask"]`.
//! Dataset-level metadata contains byte order, directory count, and image
//! segment count.

use std::sync::{Arc, Mutex};

use serde_json;

use crate::error::CodecError;
use crate::owned_buffer::OwnedBuffer;
use crate::tiff::ffi::TiffHandle;
use crate::tiff::image::TIFFImageAssetProvider;
use crate::tiff::metadata::TIFFMetadataProvider;
use crate::tiff::tags;
use crate::traits::metadata::MetadataProvider;
use crate::traits::reader::DatasetReader;
use crate::traits::{AssetMetadata, AssetProvider};
use crate::types::AssetType;

/// Supported compression codes. IFDs using other compressions are rejected
/// with `CodecError::Unsupported` during enumeration.
const SUPPORTED_COMPRESSIONS: &[u16] = &[
    tags::COMPRESSION_NONE,
    tags::COMPRESSION_CCITT_G3,
    tags::COMPRESSION_CCITT_G4,
    tags::COMPRESSION_LZW,
    tags::COMPRESSION_JPEG,
    tags::COMPRESSION_DEFLATE,
    tags::COMPRESSION_PACKBITS,
    tags::COMPRESSION_ADOBE_DEFLATE,
];

/// TIFF dataset reader implementing the `DatasetReader` trait.
///
/// Owns the raw byte buffer (keeping it alive for the libtiff handle's
/// lifetime), a shared `TiffHandle`, one `TIFFImageAssetProvider` per
/// full-resolution IFD, and dataset-level metadata.
pub struct TIFFDatasetReader {
    /// Dropped first — holds a raw pointer into `_source_data`'s bytes.
    #[allow(dead_code)]
    handle: Arc<Mutex<TiffHandle>>,
    /// One image asset provider per IFD (full-resolution and overview).
    image_assets: Vec<Arc<TIFFImageAssetProvider>>,
    /// Ordered keys for the image assets (e.g. "image:0", "image:0:overview:1").
    asset_keys: Vec<String>,
    /// Dataset-level metadata (byte order, directory count, segment count).
    dataset_metadata: Arc<TIFFMetadataProvider>,
    /// Dropped last — keeps backing bytes alive for the TiffHandle's raw pointer.
    _source_data: OwnedBuffer,
}

impl std::fmt::Debug for TIFFDatasetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TIFFDatasetReader")
            .field("asset_keys", &self.asset_keys)
            .finish()
    }
}

impl TIFFDatasetReader {
    /// Open a TIFF from an `OwnedBuffer`.
    ///
    /// Validates the TIFF magic bytes, opens via `TIFFClientOpen` with memory
    /// callbacks, enumerates IFDs, checks compression support, classifies by
    /// `NewSubfileType`, and creates one `TIFFImageAssetProvider` per
    /// full-resolution IFD.
    pub fn from_buffer(buffer: OwnedBuffer) -> Result<Self, CodecError> {
        let data = buffer.as_bytes();

        // Validate magic bytes before handing to libtiff
        if data.len() < 4 {
            return Err(CodecError::InvalidFormat(
                "Data too short to be a valid TIFF file".to_string(),
            ));
        }
        let magic = &data[0..4];
        let byte_order = match magic {
            [0x49, 0x49, 0x2A, 0x00] | [0x49, 0x49, 0x2B, 0x00] => "LittleEndian",
            [0x4D, 0x4D, 0x00, 0x2A] | [0x4D, 0x4D, 0x00, 0x2B] => "BigEndian",
            _ => {
                return Err(CodecError::InvalidFormat(
                    "Invalid TIFF magic bytes: expected II*\\0 (little-endian) or MM\\0* (big-endian)"
                        .to_string(),
                ));
            }
        };

        let handle = TiffHandle::from_bytes(data)?;
        let handle = Arc::new(Mutex::new(handle));

        let num_directories = {
            let guard = handle.lock().map_err(|e| {
                CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
            })?;
            guard.number_of_directories()
        };

        let single_ifd = num_directories == 1;

        // Enumerate IFDs and build image asset providers for all IFDs
        let mut image_assets: Vec<Arc<TIFFImageAssetProvider>> = Vec::new();
        let mut asset_keys: Vec<String> = Vec::new();
        let mut segment_index: usize = 0;
        let mut current_parent_index: usize = 0;
        let mut overview_index: u32 = 1;
        // Counter for mask-of-overview IFDs. Per COG Recommendation 3 the mask
        // overviews follow the image overviews in the same largest→smallest
        // order, so mask overview M associates with image overview M.
        let mut mask_overview_index: u32 = 1;
        // The most recent full-resolution image key. A following mask IFD binds
        // to it (order-based association). `None` until the first full-res IFD
        // is seen, which triggers the standalone-mask fallback.
        let mut last_full_res_key: Option<String> = None;

        for ifd_index in 0..num_directories {
            let guard = handle.lock().map_err(|e| {
                CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
            })?;
            guard.set_directory(ifd_index)?;

            // Check compression support
            let compression = guard
                .get_field_u16(tags::COMPRESSION)
                .unwrap_or(tags::COMPRESSION_NONE);
            if !SUPPORTED_COMPRESSIONS.contains(&compression) {
                return Err(CodecError::Unsupported(format!(
                    "Unsupported TIFF compression type: {} (code {}). Supported: None (1), CCITT Group 3 (3), CCITT Group 4 (4), LZW (5), JPEG (7), Deflate (8), PackBits (32773), Adobe Deflate (32946)",
                    compression_name(compression),
                    compression,
                )));
            }

            // JPEG compression requires 8-bit samples
            if compression == tags::COMPRESSION_JPEG {
                let bps = guard.get_field_u16(tags::BITS_PER_SAMPLE).unwrap_or(8);
                if bps != 8 {
                    return Err(CodecError::Unsupported(
                        "JPEG compression requires 8-bit samples".to_string(),
                    ));
                }
            }

            // Classify by NewSubfileType (tag 254). Per TIFF 6.0 p.36 the bits
            // are independent flags: bit 0 marks a reduced-resolution (overview)
            // subfile, bit 2 marks a transparency mask (PhotometricInterpretation
            // must be 4). We inspect them independently.
            let new_subfile_type = guard.get_field_u32(tags::NEW_SUBFILE_TYPE).unwrap_or(0);
            let is_reduced_res = (new_subfile_type & 1) != 0;
            let is_mask = (new_subfile_type & 4) != 0;

            // Drop the guard before creating the provider (it acquires its own lock)
            drop(guard);

            // Determine key and roles based on IFD classification. Masks are
            // bound to their associated image/overview by enumeration order,
            // relying on the COG IFD ordering guarantee (OGC COG Recommendation
            // 3: image → its mask → image overviews → mask overviews). When no
            // parent has been seen (non-COG ordering), a mask is exposed as a
            // standalone `image:N:mask`.
            let (key, roles) = if single_ifd {
                // Single-IFD override: always the primary image, regardless of
                // any reduced-resolution or mask bits.
                let key = format!("image:{}", segment_index);
                last_full_res_key = Some(key.clone());
                current_parent_index = segment_index;
                segment_index += 1;
                overview_index = 1;
                mask_overview_index = 1;
                (key, vec!["data".to_string()])
            } else if is_mask && is_reduced_res {
                // Mask of an overview (bits 0+2). Associates with the image
                // overview of the same ordinal.
                if last_full_res_key.is_some() {
                    let key = format!(
                        "image:{}:overview:{}:mask",
                        current_parent_index, mask_overview_index
                    );
                    mask_overview_index += 1;
                    (key, vec!["overview".to_string(), "mask".to_string()])
                } else {
                    // Fallback: no parent seen — standalone mask.
                    let key = format!("image:{}:mask", segment_index);
                    segment_index += 1;
                    (key, vec!["overview".to_string(), "mask".to_string()])
                }
            } else if is_mask {
                // Mask of the full-resolution image (bit 2 only).
                if let Some(parent) = &last_full_res_key {
                    let key = format!("{}:mask", parent);
                    (key, vec!["mask".to_string()])
                } else {
                    // Fallback: no parent seen — standalone mask.
                    let key = format!("image:{}:mask", segment_index);
                    segment_index += 1;
                    (key, vec!["mask".to_string()])
                }
            } else if is_reduced_res {
                // Overview IFD (bit 0 only).
                let key = format!("image:{}:overview:{}", current_parent_index, overview_index);
                overview_index += 1;
                (key, vec!["overview".to_string()])
            } else {
                // Full-resolution IFD (NewSubfileType 0).
                let key = format!("image:{}", segment_index);
                last_full_res_key = Some(key.clone());
                current_parent_index = segment_index;
                segment_index += 1;
                overview_index = 1;
                mask_overview_index = 1;
                (key, vec!["data".to_string()])
            };

            // Build metadata and create the provider
            {
                let guard = handle.lock().map_err(|e| {
                    CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
                })?;
                let mut metadata = TIFFMetadataProvider::from_handle(&guard, ifd_index)?;

                // JPEG YCbCr fixup: libtiff's default JPEGCOLORMODE_RGB converts
                // YCbCr→RGB on decode, so the pixels we return are RGB. Update
                // the metadata to match the actual pixel data.
                if compression == tags::COMPRESSION_JPEG {
                    let photometric = guard
                        .get_field_u16(tags::PHOTOMETRIC_INTERPRETATION)
                        .unwrap_or(tags::PHOTOMETRIC_RGB);
                    if photometric == tags::PHOTOMETRIC_YCBCR {
                        metadata.set_tag(
                            tags::PHOTOMETRIC_INTERPRETATION,
                            serde_json::Value::from(tags::PHOTOMETRIC_RGB as i64),
                        );
                    }
                }

                let metadata = Arc::new(metadata);
                drop(guard);

                let provider = TIFFImageAssetProvider::new(
                    key.clone(),
                    ifd_index,
                    Arc::clone(&handle),
                    metadata,
                    roles,
                )?;

                asset_keys.push(key);
                image_assets.push(Arc::new(provider));
            }
        }

        let num_image_segments = image_assets.len() as u32;
        let dataset_metadata = Arc::new(TIFFMetadataProvider::dataset_level(
            byte_order,
            num_directories,
            num_image_segments,
        ));

        Ok(Self {
            handle,
            image_assets,
            asset_keys,
            dataset_metadata,
            _source_data: buffer,
        })
    }
}

impl DatasetReader for TIFFDatasetReader {
    fn get_asset(&self, key: &str) -> Result<AssetProvider, CodecError> {
        for (i, k) in self.asset_keys.iter().enumerate() {
            if k == key {
                return Ok(AssetProvider::Image(self.image_assets[i].clone()));
            }
        }
        Err(CodecError::AssetNotFound(key.to_string()))
    }

    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<&[String]>,
    ) -> Vec<String> {
        self.asset_keys
            .iter()
            .enumerate()
            .filter(|(_, _)| match asset_type {
                None | Some(AssetType::Image) => true,
                Some(AssetType::Text) | Some(AssetType::Graphics) | Some(AssetType::Data) => false,
            })
            .filter(|(i, _)| match roles {
                None => true,
                Some(requested) => {
                    let asset_roles = self.image_assets[*i].roles();
                    requested.iter().any(|r| asset_roles.contains(r))
                }
            })
            .map(|(_, k)| k.clone())
            .collect()
    }

    fn has_asset(&self, key: &str) -> bool {
        self.asset_keys.iter().any(|k| k == key)
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.dataset_metadata.clone()
    }

    fn close(&mut self) -> Result<(), CodecError> {
        self.image_assets.clear();
        self.asset_keys.clear();
        Ok(())
    }
}

/// Map a compression code to a human-readable name for error messages.
fn compression_name(code: u16) -> &'static str {
    match code {
        1 => "None",
        2 => "CCITT RLE",
        3 => "CCITT Group 3",
        4 => "CCITT Group 4",
        5 => "LZW",
        6 => "Old JPEG",
        7 => "JPEG",
        8 => "Deflate",
        32773 => "PackBits",
        32946 => "Adobe Deflate",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_buffer::OwnedBuffer;

    /// Helper: write a single IFD entry (12 bytes) in little-endian format.
    fn write_ifd_entry(buf: &mut Vec<u8>, tag: u16, dtype: u16, count: u32, value: u32) {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&dtype.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Build a minimal single-strip TIFF (8-bit grayscale, 4×4).
    fn make_single_ifd_tiff() -> Vec<u8> {
        let width: u32 = 4;
        let height: u32 = 4;
        let pixel_data = vec![0u8; (width * height) as usize];

        let mut buf = Vec::new();
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes());

        let num_entries: u16 = 9;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        let pixel_data_offset = 8 + 2 + num_entries as u32 * 12 + 4;

        write_ifd_entry(&mut buf, 256, 3, 1, width);
        write_ifd_entry(&mut buf, 257, 3, 1, height);
        write_ifd_entry(&mut buf, 258, 3, 1, 8);
        write_ifd_entry(&mut buf, 259, 3, 1, 1);
        write_ifd_entry(&mut buf, 262, 3, 1, 1);
        write_ifd_entry(&mut buf, 273, 4, 1, pixel_data_offset);
        write_ifd_entry(&mut buf, 277, 3, 1, 1);
        write_ifd_entry(&mut buf, 278, 3, 1, height);
        write_ifd_entry(&mut buf, 279, 4, 1, pixel_data.len() as u32);

        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&pixel_data);
        buf
    }

    #[test]
    fn test_from_buffer_valid_single_ifd() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        assert_eq!(reader.asset_keys.len(), 1);
        assert_eq!(reader.asset_keys[0], "image:0");
        assert_eq!(reader.image_assets.len(), 1);
    }

    #[test]
    fn test_from_buffer_invalid_magic() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let result = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data));
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("Invalid TIFF magic bytes"));
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_buffer_empty_data() {
        let result = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(vec![]));
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("too short"));
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_buffer_truncated_magic() {
        let result = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(vec![0x49, 0x49]));
        match result {
            Err(CodecError::InvalidFormat(_)) => {}
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_get_asset_valid_key() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0");
        assert!(asset.is_ok());
        assert_eq!(asset.unwrap().key(), "image:0");
    }

    #[test]
    fn test_get_asset_invalid_key() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let result = reader.get_asset("image:99");
        match result {
            Err(CodecError::AssetNotFound(key)) => assert_eq!(key, "image:99"),
            _ => panic!("Expected AssetNotFound"),
        }
    }

    #[test]
    fn test_get_asset_keys_image() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image:0"]);
    }

    #[test]
    fn test_get_asset_keys_text_empty() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(reader
            .get_asset_keys(Some(AssetType::Text), None)
            .is_empty());
    }

    #[test]
    fn test_get_asset_keys_graphics_empty() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(reader
            .get_asset_keys(Some(AssetType::Graphics), None)
            .is_empty());
    }

    #[test]
    fn test_get_asset_keys_data_empty() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(reader
            .get_asset_keys(Some(AssetType::Data), None)
            .is_empty());
    }

    #[test]
    fn test_get_asset_keys_none_returns_all() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let keys = reader.get_asset_keys(None, None);
        assert_eq!(keys, vec!["image:0"]);
    }

    #[test]
    fn test_has_asset() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(reader.has_asset("image:0"));
        assert!(!reader.has_asset("image:1"));
        assert!(!reader.has_asset("bogus_key"));
    }

    #[test]
    fn test_dataset_metadata() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let meta = reader.metadata();
        let dict = meta.entries(None);

        assert_eq!(
            dict.get("ByteOrder").and_then(|v| v.as_str()),
            Some("LittleEndian")
        );
        assert_eq!(
            dict.get("NumberOfDirectories").and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            dict.get("NumberOfImageSegments").and_then(|v| v.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn test_close_clears_assets() {
        let data = make_single_ifd_tiff();
        let mut reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(!reader.asset_keys.is_empty());

        reader.close().unwrap();
        assert!(reader.asset_keys.is_empty());
        assert!(reader.image_assets.is_empty());
    }

    #[test]
    fn test_image_asset_provider_accessible() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("image:0").unwrap();

        // Use typed accessor to get the ImageAssetProvider
        let image = asset.as_image().expect("Asset should be an Image variant");

        assert_eq!(image.num_columns(), 4);
        assert_eq!(image.num_rows(), 4);
        assert_eq!(image.num_bands(), 1);
        assert_eq!(image.num_resolution_levels(), 1);
    }

    #[test]
    fn test_big_endian_magic_detected() {
        // Build a big-endian TIFF header (MM\0*)
        let mut data = make_single_ifd_tiff();
        // Swap to big-endian magic — this won't be a valid BE TIFF but we're
        // testing that the magic detection path works. libtiff will reject it
        // if the rest of the data is LE, so we just test the error path.
        data[0] = 0x4D;
        data[1] = 0x4D;
        data[2] = 0x00;
        data[3] = 0x2A;

        // libtiff will likely fail to parse the LE data with BE header,
        // but the magic check itself should pass (not InvalidFormat for magic).
        let result = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data));
        // The error should be from libtiff, not from our magic check
        if let Err(CodecError::InvalidFormat(msg)) = result {
            assert!(!msg.contains("Invalid TIFF magic bytes"));
        }
    }

    #[test]
    fn test_compression_name_helper() {
        assert_eq!(compression_name(1), "None");
        assert_eq!(compression_name(5), "LZW");
        assert_eq!(compression_name(7), "JPEG");
        assert_eq!(compression_name(8), "Deflate");
        assert_eq!(compression_name(32773), "PackBits");
        assert_eq!(compression_name(32946), "Adobe Deflate");
        assert_eq!(compression_name(9999), "Unknown");
    }

    #[test]
    fn test_compression_name_jpeg() {
        assert_eq!(compression_name(7), "JPEG");
    }

    /// Build a multi-IFD TIFF where each IFD is 4×4 grayscale uncompressed.
    /// `new_subfile_types` specifies the NewSubfileType value for each IFD.
    fn make_multi_ifd_tiff(new_subfile_types: &[u32]) -> Vec<u8> {
        let width: u32 = 4;
        let height: u32 = 4;
        let pixel_data = vec![0u8; (width * height) as usize];
        let num_ifds = new_subfile_types.len();

        // We need to know the total size to compute offsets.
        // Each IFD has 10 entries (9 standard + NewSubfileType).
        let num_entries: u16 = 10;
        let ifd_byte_size = 2 + num_entries as u32 * 12 + 4; // entry count + entries + next-IFD ptr

        // Layout: header (8) | IFD_0 | pixel_0 | IFD_1 | pixel_1 | ...
        let mut buf = Vec::new();

        // TIFF Header
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        // First IFD offset = 8
        buf.extend_from_slice(&8u32.to_le_bytes());

        for (i, &subfile_type) in new_subfile_types.iter().enumerate() {
            let ifd_start = buf.len() as u32;
            let pixel_data_offset = ifd_start + ifd_byte_size;
            let next_ifd_offset = if i + 1 < num_ifds {
                pixel_data_offset + pixel_data.len() as u32
            } else {
                0
            };

            buf.extend_from_slice(&num_entries.to_le_bytes());

            // Tag 254: NewSubfileType (LONG)
            write_ifd_entry(&mut buf, 254, 4, 1, subfile_type);
            // Tag 256: ImageWidth
            write_ifd_entry(&mut buf, 256, 3, 1, width);
            // Tag 257: ImageLength
            write_ifd_entry(&mut buf, 257, 3, 1, height);
            // Tag 258: BitsPerSample
            write_ifd_entry(&mut buf, 258, 3, 1, 8);
            // Tag 259: Compression = None
            write_ifd_entry(&mut buf, 259, 3, 1, 1);
            // Tag 262: PhotometricInterpretation = MinIsBlack
            write_ifd_entry(&mut buf, 262, 3, 1, 1);
            // Tag 273: StripOffsets
            write_ifd_entry(&mut buf, 273, 4, 1, pixel_data_offset);
            // Tag 277: SamplesPerPixel
            write_ifd_entry(&mut buf, 277, 3, 1, 1);
            // Tag 278: RowsPerStrip
            write_ifd_entry(&mut buf, 278, 3, 1, height);
            // Tag 279: StripByteCounts
            write_ifd_entry(&mut buf, 279, 4, 1, pixel_data.len() as u32);

            // Next IFD offset
            buf.extend_from_slice(&next_ifd_offset.to_le_bytes());

            // Pixel data for this IFD
            buf.extend_from_slice(&pixel_data);
        }

        buf
    }

    /// Req 2.3: Single-IFD with NewSubfileType=1 should still be keyed as
    /// `image:0` with role `"data"` (single-IFD override).
    #[test]
    fn test_single_ifd_overview_bit_treated_as_primary() {
        let data = make_multi_ifd_tiff(&[1]); // single IFD, NewSubfileType=1
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        assert_eq!(reader.asset_keys.len(), 1);
        assert_eq!(reader.asset_keys[0], "image:0");

        let asset = reader.get_asset("image:0").unwrap();
        assert_eq!(asset.roles(), &["data".to_string()]);
    }

    /// Req 6.1, 6.2: 3-IFD COG (1 full-res + 2 overviews) produces the
    /// correct keys and roles.
    #[test]
    fn test_three_ifd_cog_keys_and_roles() {
        // IFD 0: full-res (NewSubfileType=0)
        // IFD 1: overview (NewSubfileType=1)
        // IFD 2: overview (NewSubfileType=1)
        let data = make_multi_ifd_tiff(&[0, 1, 1]);
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        assert_eq!(
            reader.asset_keys,
            vec!["image:0", "image:0:overview:1", "image:0:overview:2"]
        );

        // Verify roles
        let full_res = reader.get_asset("image:0").unwrap();
        assert_eq!(full_res.roles(), &["data".to_string()]);

        let ov1 = reader.get_asset("image:0:overview:1").unwrap();
        assert_eq!(ov1.roles(), &["overview".to_string()]);

        let ov2 = reader.get_asset("image:0:overview:2").unwrap();
        assert_eq!(ov2.roles(), &["overview".to_string()]);
    }

    /// Req 1.7: Old-format key `image_segment_0` returns `AssetNotFound`.
    #[test]
    fn test_old_format_key_returns_asset_not_found() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        let result = reader.get_asset("image_segment_0");
        match result {
            Err(CodecError::AssetNotFound(key)) => {
                assert_eq!(key, "image_segment_0");
            }
            Ok(_) => panic!("Expected AssetNotFound for old-format key, got Ok"),
            Err(e) => panic!("Expected AssetNotFound for old-format key, got: {:?}", e),
        }
    }

    /// BigTIFF LE magic bytes (II + version 43) pass validation.
    #[test]
    fn test_bigtiff_le_magic_passes_validation() {
        let mut data = make_single_ifd_tiff();
        // Change version byte from 42 to 43 (BigTIFF)
        data[2] = 0x2B;
        data[3] = 0x00;

        let result = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data));
        // The magic check should pass — the error (if any) comes from libtiff
        // failing to parse classic IFD data as BigTIFF, not from our magic check.
        if let Err(CodecError::InvalidFormat(msg)) = &result {
            assert!(
                !msg.contains("Invalid TIFF magic bytes"),
                "BigTIFF LE magic should not be rejected: {}",
                msg
            );
        }
    }

    /// BigTIFF BE magic bytes (MM + version 43) pass validation.
    #[test]
    fn test_bigtiff_be_magic_passes_validation() {
        let data = vec![0x4D, 0x4D, 0x00, 0x2B, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let result = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data));
        // Should not fail with "Invalid TIFF magic bytes"
        if let Err(CodecError::InvalidFormat(msg)) = &result {
            assert!(
                !msg.contains("Invalid TIFF magic bytes"),
                "BigTIFF BE magic should not be rejected: {}",
                msg
            );
        }
    }

    /// Req 2.5, 2.6: Role-based filtering in `get_asset_keys()`.
    #[test]
    fn test_role_based_filtering() {
        let data = make_multi_ifd_tiff(&[0, 1, 1]);
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        // Filter by "overview" role → only overview keys
        let overview_keys =
            reader.get_asset_keys(Some(AssetType::Image), Some(&["overview".to_string()]));
        assert_eq!(
            overview_keys,
            vec!["image:0:overview:1", "image:0:overview:2"]
        );

        // Filter by "data" role → only full-res key
        let data_keys = reader.get_asset_keys(Some(AssetType::Image), Some(&["data".to_string()]));
        assert_eq!(data_keys, vec!["image:0"]);

        // No role filter → all keys
        let all_keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(
            all_keys,
            vec!["image:0", "image:0:overview:1", "image:0:overview:2"]
        );
    }

    /// Phase 3: full COG mask layout per OGC COG Recommendation 3
    /// (image → its mask → image overviews → mask overviews). NewSubfileType
    /// values: 0 (image), 4 (mask), 1×5 (image overviews), 5×5 (mask overviews).
    #[test]
    fn test_mask_bearing_cog_keys_and_roles() {
        let data = make_multi_ifd_tiff(&[0, 4, 1, 1, 1, 1, 1, 5, 5, 5, 5, 5]);
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        assert_eq!(
            reader.asset_keys,
            vec![
                "image:0",
                "image:0:mask",
                "image:0:overview:1",
                "image:0:overview:2",
                "image:0:overview:3",
                "image:0:overview:4",
                "image:0:overview:5",
                "image:0:overview:1:mask",
                "image:0:overview:2:mask",
                "image:0:overview:3:mask",
                "image:0:overview:4:mask",
                "image:0:overview:5:mask",
            ]
        );

        // Full-res image → data
        assert_eq!(
            reader.get_asset("image:0").unwrap().roles(),
            &["data".to_string()]
        );
        // Full-res mask → mask only
        assert_eq!(
            reader.get_asset("image:0:mask").unwrap().roles(),
            &["mask".to_string()]
        );
        // Image overview → overview only
        assert_eq!(
            reader.get_asset("image:0:overview:1").unwrap().roles(),
            &["overview".to_string()]
        );
        // Mask overview → dual roles
        assert_eq!(
            reader.get_asset("image:0:overview:1:mask").unwrap().roles(),
            &["overview".to_string(), "mask".to_string()]
        );
    }

    /// Phase 3: role-based filtering with masks present. A query for `mask`
    /// returns all masks (full-res + overview masks); a query for `overview`
    /// returns image overviews *and* mask overviews (dual role).
    #[test]
    fn test_role_filtering_with_masks() {
        let data = make_multi_ifd_tiff(&[0, 4, 1, 1, 5, 5]);
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        let mask_keys = reader.get_asset_keys(Some(AssetType::Image), Some(&["mask".to_string()]));
        assert_eq!(
            mask_keys,
            vec![
                "image:0:mask",
                "image:0:overview:1:mask",
                "image:0:overview:2:mask",
            ]
        );

        let overview_keys =
            reader.get_asset_keys(Some(AssetType::Image), Some(&["overview".to_string()]));
        assert_eq!(
            overview_keys,
            vec![
                "image:0:overview:1",
                "image:0:overview:2",
                "image:0:overview:1:mask",
                "image:0:overview:2:mask",
            ]
        );

        // `data` role still selects only the primary image (regression guard
        // for `_resolve_asset`, which prefers roles=["data"]).
        let data_keys = reader.get_asset_keys(Some(AssetType::Image), Some(&["data".to_string()]));
        assert_eq!(data_keys, vec!["image:0"]);
    }

    /// Phase 3: a mask IFD (NewSubfileType=4) appearing before any full-res
    /// image — non-COG ordering — falls back to a standalone `image:N:mask`
    /// rather than mis-associating.
    #[test]
    fn test_out_of_order_mask_falls_back_to_standalone() {
        // Mask first, then the full-res image.
        let data = make_multi_ifd_tiff(&[4, 0]);
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        assert_eq!(reader.asset_keys, vec!["image:0:mask", "image:1"]);

        // The standalone mask still carries the mask role.
        assert_eq!(
            reader.get_asset("image:0:mask").unwrap().roles(),
            &["mask".to_string()]
        );
        // The following full-res image is keyed independently.
        assert_eq!(
            reader.get_asset("image:1").unwrap().roles(),
            &["data".to_string()]
        );
    }

    /// Phase 3: a mask-of-overview IFD (NewSubfileType=5) appearing before any
    /// full-res image also falls back to a standalone `image:N:mask`.
    #[test]
    fn test_out_of_order_overview_mask_falls_back_to_standalone() {
        let data = make_multi_ifd_tiff(&[5, 0]);
        let reader = TIFFDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        assert_eq!(reader.asset_keys, vec!["image:0:mask", "image:1"]);
        assert_eq!(
            reader.get_asset("image:0:mask").unwrap().roles(),
            &["overview".to_string(), "mask".to_string()]
        );
    }
}
