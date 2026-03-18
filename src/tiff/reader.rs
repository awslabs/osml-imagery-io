//! TIFFDatasetReader — implements DatasetReader for TIFF files.
//!
//! Opens a TIFF from a byte slice, enumerates IFDs, classifies each by
//! `NewSubfileType`, and creates one `TIFFImageAssetProvider` per
//! full-resolution IFD. Dataset-level metadata contains byte order, directory
//! count, and image segment count.

use std::sync::{Arc, Mutex};

use crate::error::CodecError;
use crate::tiff::ffi::TiffHandle;
use crate::tiff::image::TIFFImageAssetProvider;
use crate::tiff::metadata::TIFFMetadataProvider;
use crate::tiff::tags;
use crate::traits::metadata::MetadataProvider;
use crate::traits::reader::DatasetReader;
use crate::traits::AssetProvider;
use crate::types::AssetType;

/// Supported compression codes. IFDs using other compressions are rejected
/// with `CodecError::Unsupported` during enumeration.
const SUPPORTED_COMPRESSIONS: &[u16] = &[
    tags::COMPRESSION_NONE,
    tags::COMPRESSION_LZW,
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
    /// Shared libtiff handle protected by a mutex.
    #[allow(dead_code)]
    handle: Arc<Mutex<TiffHandle>>,
    /// One image asset provider per full-resolution IFD.
    image_assets: Vec<Arc<TIFFImageAssetProvider>>,
    /// Ordered keys for the image assets (e.g. "image_segment_0").
    asset_keys: Vec<String>,
    /// Dataset-level metadata (byte order, directory count, segment count).
    dataset_metadata: Arc<TIFFMetadataProvider>,
    /// Owns the byte buffer so it outlives the TiffHandle.
    _data: Vec<u8>,
}

impl std::fmt::Debug for TIFFDatasetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TIFFDatasetReader")
            .field("asset_keys", &self.asset_keys)
            .finish()
    }
}

impl TIFFDatasetReader {
    /// Open a TIFF from an in-memory byte slice.
    ///
    /// Validates the TIFF magic bytes, opens via `TIFFClientOpen` with memory
    /// callbacks, enumerates IFDs, checks compression support, classifies by
    /// `NewSubfileType`, and creates one `TIFFImageAssetProvider` per
    /// full-resolution IFD.
    pub fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        // Validate magic bytes before handing to libtiff
        if data.len() < 4 {
            return Err(CodecError::InvalidFormat(
                "Data too short to be a valid TIFF file".to_string(),
            ));
        }
        let magic = &data[0..4];
        let byte_order = match magic {
            [0x49, 0x49, 0x2A, 0x00] => "LittleEndian",
            [0x4D, 0x4D, 0x00, 0x2A] => "BigEndian",
            _ => {
                return Err(CodecError::InvalidFormat(
                    "Invalid TIFF magic bytes: expected II*\\0 (little-endian) or MM\\0* (big-endian)"
                        .to_string(),
                ));
            }
        };

        // We must own the data so it outlives the TiffHandle
        let owned_data = data.to_vec();

        let handle = TiffHandle::from_bytes(&owned_data)?;
        let handle = Arc::new(Mutex::new(handle));

        let num_directories = {
            let guard = handle.lock().map_err(|e| {
                CodecError::Decode(format!("Failed to acquire TIFF handle lock: {}", e))
            })?;
            guard.number_of_directories()
        };

        let single_ifd = num_directories == 1;

        // Enumerate IFDs and build image asset providers for full-resolution IFDs
        let mut image_assets: Vec<Arc<TIFFImageAssetProvider>> = Vec::new();
        let mut asset_keys: Vec<String> = Vec::new();
        let mut segment_index: usize = 0;

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
                    "Unsupported TIFF compression type: {} (code {}). Supported: None (1), LZW (5), Deflate (8), PackBits (32773), Adobe Deflate (32946)",
                    compression_name(compression),
                    compression,
                )));
            }

            // Classify by NewSubfileType
            let new_subfile_type = guard.get_field_u32(tags::NEW_SUBFILE_TYPE).unwrap_or(0);
            let is_full_res = (new_subfile_type & 1) == 0;

            // Drop the guard before creating the provider (it acquires its own lock)
            drop(guard);

            // Single-IFD files always become image_segment_0 regardless of NewSubfileType
            if is_full_res || single_ifd {
                let key = format!("image_segment_{}", segment_index);

                let guard = handle.lock().map_err(|e| {
                    CodecError::Decode(format!(
                        "Failed to acquire TIFF handle lock: {}",
                        e
                    ))
                })?;
                let metadata =
                    Arc::new(TIFFMetadataProvider::from_handle(&guard, ifd_index)?);
                drop(guard);

                let provider = TIFFImageAssetProvider::new(
                    key.clone(),
                    ifd_index,
                    Arc::clone(&handle),
                    metadata,
                )?;

                asset_keys.push(key);
                image_assets.push(Arc::new(provider));
                segment_index += 1;
            }
        }

        let num_image_segments = image_assets.len() as u16;
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
            _data: owned_data,
        })
    }
}

impl DatasetReader for TIFFDatasetReader {
    fn get_asset(&self, key: &str) -> Result<Arc<dyn AssetProvider>, CodecError> {
        for (i, k) in self.asset_keys.iter().enumerate() {
            if k == key {
                return Ok(self.image_assets[i].clone());
            }
        }
        Err(CodecError::AssetNotFound(key.to_string()))
    }

    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        _roles: Option<&[String]>,
    ) -> Vec<String> {
        match asset_type {
            None | Some(AssetType::Image) => self.asset_keys.clone(),
            // TIFF has no text, graphics, or data segments
            Some(AssetType::Text) | Some(AssetType::Graphics) | Some(AssetType::Data) => {
                Vec::new()
            }
        }
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
    use crate::traits::image::ImageAssetProvider;

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
    fn test_from_bytes_valid_single_ifd() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();

        assert_eq!(reader.asset_keys.len(), 1);
        assert_eq!(reader.asset_keys[0], "image_segment_0");
        assert_eq!(reader.image_assets.len(), 1);
    }

    #[test]
    fn test_from_bytes_invalid_magic() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let result = TIFFDatasetReader::from_bytes(&data);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("Invalid TIFF magic bytes"));
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_empty_data() {
        let result = TIFFDatasetReader::from_bytes(&[]);
        match result {
            Err(CodecError::InvalidFormat(msg)) => {
                assert!(msg.contains("too short"));
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_truncated_magic() {
        let result = TIFFDatasetReader::from_bytes(&[0x49, 0x49]);
        match result {
            Err(CodecError::InvalidFormat(_)) => {}
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_get_asset_valid_key() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image_segment_0");
        assert!(asset.is_ok());
        assert_eq!(asset.unwrap().key(), "image_segment_0");
    }

    #[test]
    fn test_get_asset_invalid_key() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        let result = reader.get_asset("image_segment_99");
        match result {
            Err(CodecError::AssetNotFound(key)) => assert_eq!(key, "image_segment_99"),
            _ => panic!("Expected AssetNotFound"),
        }
    }

    #[test]
    fn test_get_asset_keys_image() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image_segment_0"]);
    }

    #[test]
    fn test_get_asset_keys_text_empty() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        assert!(reader.get_asset_keys(Some(AssetType::Text), None).is_empty());
    }

    #[test]
    fn test_get_asset_keys_graphics_empty() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        assert!(reader
            .get_asset_keys(Some(AssetType::Graphics), None)
            .is_empty());
    }

    #[test]
    fn test_get_asset_keys_data_empty() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        assert!(reader.get_asset_keys(Some(AssetType::Data), None).is_empty());
    }

    #[test]
    fn test_get_asset_keys_none_returns_all() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        let keys = reader.get_asset_keys(None, None);
        assert_eq!(keys, vec!["image_segment_0"]);
    }

    #[test]
    fn test_has_asset() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        assert!(reader.has_asset("image_segment_0"));
        assert!(!reader.has_asset("image_segment_1"));
        assert!(!reader.has_asset("bogus_key"));
    }

    #[test]
    fn test_dataset_metadata() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        let meta = reader.metadata();
        let dict = meta.as_dict(None);

        assert_eq!(dict.get("ByteOrder").and_then(|v| v.as_str()), Some("LittleEndian"));
        assert_eq!(dict.get("NumberOfDirectories").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(dict.get("NumberOfImageSegments").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn test_close_clears_assets() {
        let data = make_single_ifd_tiff();
        let mut reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        assert!(!reader.asset_keys.is_empty());

        reader.close().unwrap();
        assert!(reader.asset_keys.is_empty());
        assert!(reader.image_assets.is_empty());
    }

    #[test]
    fn test_image_asset_provider_accessible() {
        let data = make_single_ifd_tiff();
        let reader = TIFFDatasetReader::from_bytes(&data).unwrap();
        let asset = reader.get_asset("image_segment_0").unwrap();

        // Downcast to ImageAssetProvider to verify it's the right type
        let image = asset
            .as_any()
            .downcast_ref::<TIFFImageAssetProvider>()
            .expect("Asset should be a TIFFImageAssetProvider");

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
        let result = TIFFDatasetReader::from_bytes(&data);
        // The error should be from libtiff, not from our magic check
        if let Err(e) = result {
            match e {
                CodecError::InvalidFormat(msg) => {
                    assert!(!msg.contains("Invalid TIFF magic bytes"));
                }
                _ => {} // Other errors are fine (libtiff parse failure)
            }
        }
    }

    #[test]
    fn test_compression_name_helper() {
        assert_eq!(compression_name(1), "None");
        assert_eq!(compression_name(5), "LZW");
        assert_eq!(compression_name(8), "Deflate");
        assert_eq!(compression_name(32773), "PackBits");
        assert_eq!(compression_name(32946), "Adobe Deflate");
        assert_eq!(compression_name(9999), "Unknown");
    }
}
