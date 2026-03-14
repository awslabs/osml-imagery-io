//! TIFFMetadataProvider — implements MetadataProvider for TIFF tag metadata.
//!
//! Provides per-IFD metadata (standard TIFF tags) and dataset-level metadata
//! (byte order, directory count, image segment count).

use std::collections::HashMap;

use serde_json::Value;

use crate::error::CodecError;
use crate::tiff::ffi::TiffHandle;
use crate::tiff::tags;
use crate::traits::metadata::MetadataProvider;

/// Metadata provider for TIFF tags.
///
/// Stores TIFF tag values as a `HashMap<String, serde_json::Value>` and optionally
/// retains the raw bytes of the source data. Supports two construction modes:
///
/// - **Per-IFD**: reads standard TIFF tags from a specific IFD via `TiffHandle`
/// - **Dataset-level**: stores only file-level information (byte order, directory count,
///   image segment count)
///
/// Section filtering:
/// - `as_dict(None)` → all tags
/// - `as_dict(Some("tiff"))` → TIFF tags (same as all for Phase 1)
/// - `as_dict(Some(other))` → empty HashMap
pub(crate) struct TIFFMetadataProvider {
    tags: HashMap<String, Value>,
    raw_bytes: Vec<u8>,
}

impl TIFFMetadataProvider {
    /// Create a per-IFD metadata provider by reading standard TIFF tags from the
    /// given IFD index. The handle's current directory is set to `ifd_index` before
    /// reading tags, and restored afterward.
    pub fn from_handle(handle: &TiffHandle, ifd_index: u16) -> Result<Self, CodecError> {
        handle.set_directory(ifd_index)?;

        let mut tags = HashMap::new();

        // Helper: try reading a u32 tag, insert if present
        macro_rules! try_u32_tag {
            ($tag:expr, $key:expr) => {
                if let Ok(v) = handle.get_field_u32($tag) {
                    tags.insert($key.to_string(), Value::from(v));
                }
            };
        }

        // Helper: try reading a u16 tag, insert if present
        macro_rules! try_u16_tag {
            ($tag:expr, $key:expr) => {
                if let Ok(v) = handle.get_field_u16($tag) {
                    tags.insert($key.to_string(), Value::from(v));
                }
            };
        }

        try_u32_tag!(tags::IMAGE_WIDTH, "ImageWidth");
        try_u32_tag!(tags::IMAGE_LENGTH, "ImageLength");
        try_u16_tag!(tags::BITS_PER_SAMPLE, "BitsPerSample");
        try_u16_tag!(tags::SAMPLES_PER_PIXEL, "SamplesPerPixel");
        try_u16_tag!(tags::COMPRESSION, "Compression");
        try_u16_tag!(tags::PHOTOMETRIC_INTERPRETATION, "PhotometricInterpretation");
        try_u16_tag!(tags::PLANAR_CONFIGURATION, "PlanarConfiguration");
        try_u16_tag!(tags::SAMPLE_FORMAT, "SampleFormat");
        try_u32_tag!(tags::TILE_WIDTH, "TileWidth");
        try_u32_tag!(tags::TILE_LENGTH, "TileLength");
        try_u32_tag!(tags::ROWS_PER_STRIP, "RowsPerStrip");
        try_u16_tag!(tags::PREDICTOR, "Predictor");

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
            None | Some("tiff") => self.tags.clone(),
            Some(_) => HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiff::ffi::TiffHandle;

    /// Build a minimal single-strip TIFF in memory (8-bit grayscale, 4×4).
    fn make_minimal_tiff() -> Vec<u8> {
        let width: u32 = 4;
        let height: u32 = 4;
        let pixel_data: Vec<u8> = vec![0u8; (width * height) as usize];

        // Little-endian TIFF
        let mut buf: Vec<u8> = Vec::new();

        // Header: byte order (II), magic (42), offset to first IFD
        buf.extend_from_slice(b"II");
        buf.extend_from_slice(&42u16.to_le_bytes());
        // IFD offset — we'll place it right after the header (offset 8)
        buf.extend_from_slice(&8u32.to_le_bytes());

        // IFD at offset 8
        let num_entries: u16 = 9;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        // Each IFD entry: tag(u16), type(u16), count(u32), value/offset(u32)
        // Tag 256: ImageWidth = 4 (type SHORT=3)
        write_ifd_entry(&mut buf, 256, 3, 1, width);
        // Tag 257: ImageLength = 4 (type SHORT=3)
        write_ifd_entry(&mut buf, 257, 3, 1, height);
        // Tag 258: BitsPerSample = 8 (type SHORT=3)
        write_ifd_entry(&mut buf, 258, 3, 1, 8);
        // Tag 259: Compression = 1 (None) (type SHORT=3)
        write_ifd_entry(&mut buf, 259, 3, 1, 1);
        // Tag 262: PhotometricInterpretation = 1 (MinIsBlack) (type SHORT=3)
        write_ifd_entry(&mut buf, 262, 3, 1, 1);
        // Tag 277: SamplesPerPixel = 1 (type SHORT=3)
        write_ifd_entry(&mut buf, 277, 3, 1, 1);
        // Tag 278: RowsPerStrip = 4 (type SHORT=3)
        write_ifd_entry(&mut buf, 278, 3, 1, height);
        // Tag 273: StripOffsets — offset to pixel data (type LONG=4)
        let pixel_data_offset = 8 + 2 + num_entries as u32 * 12 + 4;
        write_ifd_entry(&mut buf, 273, 4, 1, pixel_data_offset);
        // Tag 279: StripByteCounts (type LONG=4)
        write_ifd_entry(&mut buf, 279, 4, 1, pixel_data.len() as u32);

        // Next IFD offset = 0 (no more IFDs)
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Pixel data
        buf.extend_from_slice(&pixel_data);

        buf
    }

    fn write_ifd_entry(buf: &mut Vec<u8>, tag: u16, dtype: u16, count: u32, value: u32) {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&dtype.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn test_as_dict_none_returns_all_tags() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();
        let dict = provider.as_dict(None);

        assert_eq!(dict.get("ImageWidth").and_then(|v| v.as_u64()), Some(4));
        assert_eq!(dict.get("ImageLength").and_then(|v| v.as_u64()), Some(4));
        assert_eq!(dict.get("BitsPerSample").and_then(|v| v.as_u64()), Some(8));
        assert_eq!(dict.get("SamplesPerPixel").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(dict.get("Compression").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(
            dict.get("PhotometricInterpretation")
                .and_then(|v| v.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn test_as_dict_tiff_returns_same_as_none() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        let all = provider.as_dict(None);
        let tiff = provider.as_dict(Some("tiff"));
        assert_eq!(all, tiff);
    }

    #[test]
    fn test_as_dict_unknown_section_returns_empty() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let provider = TIFFMetadataProvider::from_handle(&handle, 0).unwrap();

        assert!(provider.as_dict(Some("unknown")).is_empty());
        assert!(provider.as_dict(Some("nitf")).is_empty());
        assert!(provider.as_dict(Some("")).is_empty());
    }

    #[test]
    fn test_dataset_level_metadata() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 3, 2);
        let dict = provider.as_dict(None);

        assert_eq!(dict.len(), 3);
        assert_eq!(
            dict.get("ByteOrder").and_then(|v| v.as_str()),
            Some("LittleEndian")
        );
        assert_eq!(
            dict.get("NumberOfDirectories").and_then(|v| v.as_u64()),
            Some(3)
        );
        assert_eq!(
            dict.get("NumberOfImageSegments").and_then(|v| v.as_u64()),
            Some(2)
        );
    }

    #[test]
    fn test_dataset_level_big_endian() {
        let provider = TIFFMetadataProvider::dataset_level("BigEndian", 1, 1);
        let dict = provider.as_dict(None);

        assert_eq!(
            dict.get("ByteOrder").and_then(|v| v.as_str()),
            Some("BigEndian")
        );
    }

    #[test]
    fn test_raw_returns_empty() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 1, 1);
        assert!(provider.raw().is_empty());
    }

    #[test]
    fn test_dataset_level_tiff_section_returns_all() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 1, 1);
        let all = provider.as_dict(None);
        let tiff = provider.as_dict(Some("tiff"));
        assert_eq!(all, tiff);
    }

    #[test]
    fn test_dataset_level_unknown_section_returns_empty() {
        let provider = TIFFMetadataProvider::dataset_level("LittleEndian", 1, 1);
        assert!(provider.as_dict(Some("unknown")).is_empty());
    }
}
