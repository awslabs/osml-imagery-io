//! TIFFMetadataProvider — implements MetadataProvider for TIFF tag metadata.
//!
//! Provides per-IFD metadata (numeric tag ID keys) and dataset-level metadata
//! (byte order, directory count, image segment count).

use std::collections::HashMap;

use serde_json::Value;

use crate::error::CodecError;
use crate::tiff::ffi::TiffHandle;
use crate::traits::metadata::MetadataProvider;

/// Metadata provider for TIFF tags.
///
/// Stores TIFF tag values as a `HashMap<String, serde_json::Value>` and optionally
/// retains the raw bytes of the source data. Supports two construction modes:
///
/// - **Per-IFD**: enumerates all IFD tags and stores values under numeric string
///   keys (e.g. `"256"` for ImageWidth, `"34735"` for GeoKeyDirectoryTag)
/// - **Dataset-level**: stores only file-level information (byte order, directory count,
///   image segment count) under descriptive string keys
///
/// Section filtering:
/// - `as_dict(None)` → all entries
/// - `as_dict(Some(prefix))` → entries whose key starts with `prefix`
pub(crate) struct TIFFMetadataProvider {
    tags: HashMap<String, Value>,
    raw_bytes: Vec<u8>,
}

impl TIFFMetadataProvider {
    /// Create a per-IFD metadata provider by enumerating all tags in the given IFD.
    ///
    /// Uses `enumerate_ifd_tags` to discover every tag present, then `read_tag_value`
    /// to read each one. Tags are stored under their numeric string key (e.g. `"256"`).
    /// Unreadable tags are silently skipped.
    pub fn from_handle(handle: &TiffHandle, ifd_index: u16) -> Result<Self, CodecError> {
        handle.set_directory(ifd_index)?;

        let mut tags = HashMap::new();
        let entries = handle.enumerate_ifd_tags()?;

        for entry in &entries {
            match handle.read_tag_value(entry) {
                Ok(value) => {
                    tags.insert(entry.tag.to_string(), value);
                }
                Err(_) => {
                    // Skip unreadable tags (Req 4.4)
                    continue;
                }
            }
        }

        Ok(Self {
            tags,
            raw_bytes: Vec::new(),
        })
    }

    /// Create a dataset-level metadata provider with file-level information only.
    ///
    /// - `byte_order`: `"LittleEndian"` or `"BigEndian"` (detected from TIFF magic bytes)
    /// - `num_directories`: total number of IFDs in the file
    /// - `num_image_segments`: count of full-resolution IFDs
    pub fn dataset_level(
        byte_order: &str,
        num_directories: u16,
        num_image_segments: u16,
    ) -> Self {
        let mut tags = HashMap::new();
        tags.insert("ByteOrder".to_string(), Value::from(byte_order));
        tags.insert(
            "NumberOfDirectories".to_string(),
            Value::from(num_directories),
        );
        tags.insert(
            "NumberOfImageSegments".to_string(),
            Value::from(num_image_segments),
        );
        Self {
            tags,
            raw_bytes: Vec::new(),
        }
    }
}

impl MetadataProvider for TIFFMetadataProvider {
    fn raw(&self) -> &[u8] {
        &self.raw_bytes
    }

    fn as_dict(&self, name: Option<&str>) -> HashMap<String, Value> {
        match name {
            None => self.tags.clone(),
            Some(prefix) => self
                .tags
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiff::ffi::TiffHandle;
    use crate::tiff::tags;

    /// Build a minimal single-strip TIFF in memory (8-bit grayscale, 4x4).
    fn make_minimal_tiff() -> Vec<u8> {
        let width: u32 = 4;
        let height: u32 = 4;
        let pixel_data: Vec<u8> = vec![0u8; (width * height) as usize];

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes());

        let num_entries: u16 = 9;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        write_ifd_entry(&mut buf, 256, 3, 1, width);
        write_ifd_entry(&mut buf, 257, 3, 1, height);
        write_ifd_entry(&mut buf, 258, 3, 1, 8);
        write_ifd_entry(&mut buf, 259, 3, 1, 1);
        write_ifd_entry(&mut buf, 262, 3, 1, 1);
        write_ifd_entry(&mut buf, 277, 3, 1, 1);
        write_ifd_entry(&mut buf, 278, 3, 1, height);
        let pixel_data_offset = 8 + 2 + num_entries as u32 * 12 + 4;
        write_ifd_entry(&mut buf, 273, 4, 1, pixel_data_offset);
        write_ifd_entry(&mut buf, 279, 4, 1, pixel_data.len() as u32);

        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&pixel_data);
        buf
    }

    /// Build a TIFF with standard tags plus private-use tags (> 32768).
    /// libtiff requires minimum fields to open, so we include standard tags too.
    fn make_private_tags_tiff() -> Vec<u8> {
        let pixel_data: Vec<u8> = vec![0u8; 1];
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes());

        // 9 standard + 2 private = 11 entries (must be sorted by tag number)
        let num_entries: u16 = 11;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        write_ifd_entry(&mut buf, 256, 3, 1, 1);  // ImageWidth
        write_ifd_entry(&mut buf, 257, 3, 1, 1);  // ImageLength
        write_ifd_entry(&mut buf, 258, 3, 1, 8);  // BitsPerSample
        write_ifd_entry(&mut buf, 259, 3, 1, 1);  // Compression = None
        write_ifd_entry(&mut buf, 262, 3, 1, 1);  // PhotometricInterpretation
        write_ifd_entry(&mut buf, 277, 3, 1, 1);  // SamplesPerPixel
        write_ifd_entry(&mut buf, 278, 3, 1, 1);  // RowsPerStrip
        let pixel_data_offset = 8 + 2 + num_entries as u32 * 12 + 4;
        write_ifd_entry(&mut buf, 273, 4, 1, pixel_data_offset); // StripOffsets
        write_ifd_entry(&mut buf, 279, 4, 1, pixel_data.len() as u32); // StripByteCounts
        // Private-use tags (must come after standard tags for sorted order)
        write_ifd_entry(&mut buf, 33000, 3, 1, 42);
        write_ifd_entry(&mut buf, 65000, 3, 1, 99);

        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&pixel_data);
        buf
    }

    fn write_ifd_entry(buf: &mut Vec<u8>, tag: u16, dtype: u16, count: u32, value: u32) {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&dtype.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Build a minimal tiled TIFF via the write path with optional GeoTIFF tags.
    fn make_geotiff(
        geokey_dir: Option<&[u16]>,
        double_params: Option<&[f64]>,
        ascii_params: Option<&str>,
        pixel_scale: Option<&[f64]>,
        tiepoints: Option<&[f64]>,
        transformation: Option<&[f64]>,
    ) -> Vec<u8> {
        let handle = TiffHandle::from_write().unwrap();
        handle.set_field_u32(tags::IMAGE_WIDTH, 1).unwrap();
        handle.set_field_u32(tags::IMAGE_LENGTH, 1).unwrap();
        handle.set_field_u16(tags::BITS_PER_SAMPLE, 8).unwrap();
        handle.set_field_u16(tags::SAMPLES_PER_PIXEL, 1).unwrap();
        handle.set_field_u16(tags::SAMPLE_FORMAT, tags::SAMPLE_FORMAT_UINT).unwrap();
        handle.set_field_u16(tags::PHOTOMETRIC_INTERPRETATION, tags::PHOTOMETRIC_MINISBLACK).unwrap();
        handle.set_field_u32(tags::TILE_WIDTH, 16).unwrap();
        handle.set_field_u32(tags::TILE_LENGTH, 16).unwrap();
        handle.set_field_u16(tags::COMPRESSION, tags::COMPRESSION_NONE).unwrap();
        handle.set_field_u16(tags::PLANAR_CONFIGURATION, tags::PLANAR_CONFIG_CONTIG).unwrap();

        if let Some(dir) = geokey_dir {
            handle.set_field_u16_array(tags::GEO_KEY_DIRECTORY_TAG, dir).unwrap();
        }
        if let Some(dp) = double_params {
            handle.set_field_f64_array(tags::GEO_DOUBLE_PARAMS_TAG, dp).unwrap();
        }
        if let Some(ap) = ascii_params {
            handle.set_field_string(tags::GEO_ASCII_PARAMS_TAG, ap).unwrap();
        }
        if let Some(ps) = pixel_scale {
            handle.set_field_f64_array(tags::MODEL_PIXEL_SCALE_TAG, ps).unwrap();
        }
        if let Some(tp) = tiepoints {
            handle.set_field_f64_array(tags::MODEL_TIEPOINT_TAG, tp).unwrap();
        }
        if let Some(tf) = transformation {
            handle.set_field_f64_array(tags::MODEL_TRANSFORMATION_TAG, tf).unwrap();
        }

        let tile_data = vec![0u8; 16 * 16];
        handle.write_encoded_tile(0, &tile_data).unwrap();
        handle.write_directory().unwrap();
        handle.into_bytes().unwrap()
    }

    // =========================================================================
    // Numeric key tests (Req 1.1, 1.2, 8.1)
    // =========================================================================

    #[test]
    fn test_as_dict_none_returns_all_tags_with_numeric_keys() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let dict = provider.as_dict(None);

        assert_eq!(dict.get("256").and_then(|v| v.as_u64()), Some(4));
        assert_eq!(dict.get("257").and_then(|v| v.as_u64()), Some(4));
        assert_eq!(dict.get("258").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(dict.get("277").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(dict.get("259").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(dict.get("262").and_then(|v| v.as_u64()), Some(1));

        // Human-readable names must NOT be present
        assert!(!dict.contains_key("ImageWidth"));
        assert!(!dict.contains_key("ImageLength"));
        assert!(!dict.contains_key("BitsPerSample"));
        assert!(!dict.contains_key("Compression"));
    }

    #[test]
    fn test_all_ifd_keys_are_numeric() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let dict = provider.as_dict(None);

        for key in dict.keys() {
            assert!(
                key.parse::<u32>().is_ok(),
                "IFD key '{}' is not a valid numeric string",
                key
            );
        }
    }

    // =========================================================================
    // Pure prefix filter tests (Req 2.1-2.4, 8.2)
    // =========================================================================

    #[test]
    fn test_as_dict_prefix_filters_by_key_start() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        let filtered = provider.as_dict(Some("25"));
        assert!(filtered.contains_key("256"));
        assert!(filtered.contains_key("257"));
        assert!(filtered.contains_key("258"));
        assert!(filtered.contains_key("259"));
        assert!(!filtered.contains_key("262"));
        assert!(!filtered.contains_key("277"));
    }

    #[test]
    fn test_as_dict_prefix_27_matches_strip_and_sample_tags() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        let filtered = provider.as_dict(Some("27"));
        assert!(filtered.contains_key("273"));
        assert!(filtered.contains_key("277"));
        assert!(filtered.contains_key("278"));
        assert!(filtered.contains_key("279"));
        assert!(!filtered.contains_key("256"));
    }

    #[test]
    fn test_as_dict_no_special_case_for_tiff() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        // "tiff" is just a prefix -- no numeric keys start with "tiff", so empty
        let tiff = provider.as_dict(Some("tiff"));
        assert!(tiff.is_empty());
    }

    #[test]
    fn test_as_dict_unknown_prefix_returns_empty() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        assert!(provider.as_dict(Some("unknown")).is_empty());
        assert!(provider.as_dict(Some("nitf")).is_empty());
        assert!(provider.as_dict(Some("99999")).is_empty());
    }

    #[test]
    fn test_as_dict_empty_prefix_returns_all() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        assert_eq!(provider.as_dict(Some("")), provider.as_dict(None));
    }

    // =========================================================================
    // Dataset-level metadata tests (Req 1.3)
    // =========================================================================

    #[test]
    fn test_dataset_level_metadata() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 3, 2);
        let dict = provider.as_dict(None);

        assert_eq!(dict.len(), 3);
        assert_eq!(dict.get("ByteOrder").and_then(|v| v.as_str()), Some("LittleEndian"));
        assert_eq!(dict.get("NumberOfDirectories").and_then(|v| v.as_u64()), Some(3));
        assert_eq!(dict.get("NumberOfImageSegments").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn test_dataset_level_big_endian() {
        let provider = TIFFMetadataProvider::dataset_level("BigEndian", 1, 1);
        let dict = provider.as_dict(None);
        assert_eq!(dict.get("ByteOrder").and_then(|v| v.as_str()), Some("BigEndian"));
    }

    #[test]
    fn test_dataset_level_keys_are_descriptive_strings() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 1, 1);
        let dict = provider.as_dict(None);
        assert!(dict.contains_key("ByteOrder"));
        assert!(dict.contains_key("NumberOfDirectories"));
        assert!(dict.contains_key("NumberOfImageSegments"));
    }

    #[test]
    fn test_raw_returns_empty() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 1, 1);
        assert!(provider.raw().is_empty());
    }

    // =========================================================================
    // Empty IFD test (Req 4.1)
    // =========================================================================

    #[test]
    fn test_empty_ifd_returns_empty_tag_dictionary() {
        // Construct directly -- libtiff cannot open a TIFF with 0 IFD entries,
        // so we test the provider's behavior with an empty tags map.
        let provider = TIFFMetadataProvider {
            tags: HashMap::new(),
            raw_bytes: Vec::new(),
        };
        let dict = provider.as_dict(None);
        assert!(dict.is_empty(), "Empty IFD should produce empty Tag_Dictionary");
    }

    // =========================================================================
    // Private-use tags test (Req 4.3)
    // =========================================================================

    #[test]
    fn test_private_use_tags_are_included() {
        let data = make_private_tags_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let dict = provider.as_dict(None);

        assert!(dict.contains_key("33000"), "Private-use tag 33000 should be in Tag_Dictionary");
        assert_eq!(dict.get("33000").and_then(|v| v.as_u64()), Some(42));
        assert!(dict.contains_key("65000"), "Private-use tag 65000 should be in Tag_Dictionary");
        assert_eq!(dict.get("65000").and_then(|v| v.as_u64()), Some(99));
    }

    // =========================================================================
    // GeoTIFF metadata integration tests
    // =========================================================================

    #[test]
    fn test_plain_tiff_no_geo_keys() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let all = provider.as_dict(None);

        assert!(
            !all.keys().any(|k| k.starts_with("Geo")),
            "Plain TIFF should have no Geo-prefixed keys"
        );
    }

    #[test]
    fn test_geotiff_tags_stored_under_numeric_keys() {
        let dir: Vec<u16> = vec![1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618];
        let ps = [0.5, 0.5, 0.0];
        let tp = [0.0, 0.0, 0.0, 300000.0, 4500000.0, 0.0];

        let data = make_geotiff(Some(&dir), None, None, Some(&ps), Some(&tp), None);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let all = provider.as_dict(None);

        assert!(all.contains_key("34735"), "GeoKeyDirectoryTag should be under '34735'");
        assert!(all.contains_key("33550"), "ModelPixelScaleTag should be under '33550'");
        assert!(all.contains_key("33922"), "ModelTiepointTag should be under '33922'");

        // No Geo-prefixed decoded entries
        assert!(
            !all.keys().any(|k| k.starts_with("Geo")),
            "No Geo-prefixed keys should exist, found: {:?}",
            all.keys().filter(|k| k.starts_with("Geo")).collect::<Vec<_>>()
        );

        assert!(all.contains_key("256")); // ImageWidth
    }

    #[test]
    fn test_geotiff_no_decoded_geokeys_in_dict() {
        let dir: Vec<u16> = vec![1, 1, 1, 1, 1024, 0, 1, 2]; // Geographic
        let data = make_geotiff(Some(&dir), None, None, None, None, None);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let all = provider.as_dict(None);

        assert!(all.contains_key("256"));
        assert!(all.contains_key("34735"));
        assert!(!all.contains_key("GeoModelType"));
        assert!(!all.contains_key("GeoRasterType"));
        assert!(!all.contains_key("GeoProjectedCRS"));
    }

    #[test]
    fn test_geotiff_transformation_tag_under_numeric_key() {
        let tf: Vec<f64> = (0..16).map(|i| i as f64).collect();
        let data = make_geotiff(None, None, None, None, None, Some(&tf));
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let all = provider.as_dict(None);

        assert!(all.contains_key("34264"), "ModelTransformationTag should be under '34264'");
        assert!(!all.contains_key("GeoTransformation"));
    }

    #[test]
    fn test_prefix_filter_with_geotiff_numeric_keys() {
        let dir: Vec<u16> = vec![1, 1, 1, 1, 1024, 0, 1, 1];
        let ps = [1.0, 1.0, 0.0];
        let data = make_geotiff(Some(&dir), None, None, Some(&ps), None, None);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        let filtered = provider.as_dict(Some("3"));
        for key in filtered.keys() {
            assert!(key.starts_with("3"), "Key '{}' should start with '3'", key);
        }

        let filtered_34 = provider.as_dict(Some("34"));
        for key in filtered_34.keys() {
            assert!(key.starts_with("34"), "Key '{}' should start with '34'", key);
        }
        if provider.as_dict(None).contains_key("34735") {
            assert!(filtered_34.contains_key("34735"));
        }
    }
}
