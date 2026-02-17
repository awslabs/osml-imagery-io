//! Test data generator for creating synthetic NITF/NSIF files.
//!
//! This module provides utilities to generate synthetic test files using
//! JBPDatasetWriter for round-trip testing and validation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::types::NitfFormat;
use crate::jbp::writer::JBPDatasetWriter;
use crate::traits::{AssetProvider, DatasetWriter, MetadataProvider};
use crate::types::AssetType;

/// A simple test asset provider for generating synthetic test files.
pub struct TestAssetProvider {
    key: String,
    title: String,
    description: String,
    roles: Vec<String>,
    asset_type: AssetType,
    data: Vec<u8>,
}

impl TestAssetProvider {
    /// Create a new test asset provider.
    pub fn new(key: &str, asset_type: AssetType, data: Vec<u8>) -> Self {
        Self {
            key: key.to_string(),
            title: format!("Test {}", key),
            description: format!("Test asset {}", key),
            roles: vec!["data".to_string()],
            asset_type,
            data,
        }
    }

    /// Create a test image asset with synthetic pixel data.
    pub fn image(key: &str, width: usize, height: usize) -> Self {
        // Create simple grayscale gradient data
        let mut data = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let value = ((x + y) % 256) as u8;
                data.push(value);
            }
        }
        Self::new(key, AssetType::Image, data)
    }

    /// Create a test text asset.
    pub fn text(key: &str, content: &str) -> Self {
        Self::new(key, AssetType::Text, content.as_bytes().to_vec())
    }

    /// Create a test graphics asset with synthetic CGM-like data.
    pub fn graphics(key: &str, size: usize) -> Self {
        // Create placeholder CGM data
        let data = vec![0u8; size];
        Self::new(key, AssetType::Graphics, data)
    }

    /// Create a test data extension segment.
    pub fn data_extension(key: &str, data: Vec<u8>) -> Self {
        Self::new(key, AssetType::Data, data)
    }
}

impl AssetProvider for TestAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn media_type(&self) -> &str {
        match self.asset_type {
            AssetType::Image => "application/vnd.nitf.image",
            AssetType::Text => "text/plain",
            AssetType::Graphics => "image/cgm",
            AssetType::Data => "application/octet-stream",
        }
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(self.data.clone())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        Arc::new(EmptyMetadataProvider)
    }
}

/// Empty metadata provider for test assets.
struct EmptyMetadataProvider;

impl MetadataProvider for EmptyMetadataProvider {
    fn raw(&self) -> &[u8] {
        &[]
    }

    fn as_dict(&self, _name: Option<&str>) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

/// Generate a minimal valid NITF 2.1 file with one image segment.
///
/// Creates a file at `data/unit/sample_nitf21.ntf` with:
/// - Valid NITF 2.1 magic number and header
/// - One 8x8 grayscale image segment
pub fn generate_sample_nitf21(path: impl AsRef<Path>) -> Result<(), CodecError> {
    let mut writer = JBPDatasetWriter::new(path, NitfFormat::Nitf21)?;

    // Add a single 8x8 image segment
    let image = Arc::new(TestAssetProvider::image("image_segment_0", 8, 8));
    writer.add_asset("image_segment_0", image, "Sample Image", "A minimal test image", &[])?;

    writer.close()?;
    Ok(())
}

/// Generate a minimal valid NSIF 1.0 file with one image segment.
///
/// Creates a file at `data/unit/sample_nsif10.nsif` with:
/// - Valid NSIF 1.0 magic number and header
/// - One 8x8 grayscale image segment
pub fn generate_sample_nsif10(path: impl AsRef<Path>) -> Result<(), CodecError> {
    let mut writer = JBPDatasetWriter::new(path, NitfFormat::Nsif10)?;

    // Add a single 8x8 image segment
    let image = Arc::new(TestAssetProvider::image("image_segment_0", 8, 8));
    writer.add_asset("image_segment_0", image, "Sample Image", "A minimal test image", &[])?;

    writer.close()?;
    Ok(())
}

/// Generate a NITF file with multiple segments of different types.
///
/// Creates a file at `data/unit/multi_segment.ntf` with:
/// - 2 image segments
/// - 1 text segment
/// - 1 graphic segment
/// - 1 DES segment
pub fn generate_multi_segment_nitf(path: impl AsRef<Path>) -> Result<(), CodecError> {
    let mut writer = JBPDatasetWriter::new(path, NitfFormat::Nitf21)?;

    // Add 2 image segments
    let image1 = Arc::new(TestAssetProvider::image("image_segment_0", 16, 16));
    writer.add_asset("image_segment_0", image1, "First Image", "First test image", &[])?;

    let image2 = Arc::new(TestAssetProvider::image("image_segment_1", 8, 8));
    writer.add_asset("image_segment_1", image2, "Second Image", "Second test image", &[])?;

    // Add 1 text segment
    let text = Arc::new(TestAssetProvider::text(
        "text_segment_0",
        "This is sample text content for testing.",
    ));
    writer.add_asset("text_segment_0", text, "Sample Text", "Test text segment", &[])?;

    // Add 1 graphic segment
    let graphic = Arc::new(TestAssetProvider::graphics("graphic_segment_0", 100));
    writer.add_asset("graphic_segment_0", graphic, "Sample Graphic", "Test graphic segment", &[])?;

    // Add 1 DES segment
    let des_data = b"Sample DES data content".to_vec();
    let des = Arc::new(TestAssetProvider::data_extension("des_segment_0", des_data));
    writer.add_asset("des_segment_0", des, "Sample DES", "Test DES segment", &[])?;

    writer.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jbp::reader::JBPDatasetReader;
    use crate::traits::DatasetReader;
    use tempfile::tempdir;

    #[test]
    fn test_generate_sample_nitf21() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sample_nitf21.ntf");

        let result = generate_sample_nitf21(&path);
        assert!(result.is_ok(), "Failed to generate NITF 2.1: {:?}", result.err());

        // Verify the file can be read back
        let reader = JBPDatasetReader::open(&path);
        assert!(reader.is_ok(), "Failed to read generated NITF 2.1: {:?}", reader.err());

        let reader = reader.unwrap();
        assert_eq!(reader.format(), NitfFormat::Nitf21);

        let keys = reader.get_asset_keys(None, None);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "image_segment_0");
    }

    #[test]
    fn test_generate_sample_nsif10() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sample_nsif10.nsif");

        let result = generate_sample_nsif10(&path);
        assert!(result.is_ok(), "Failed to generate NSIF 1.0: {:?}", result.err());

        // Verify the file can be read back
        let reader = JBPDatasetReader::open(&path);
        assert!(reader.is_ok(), "Failed to read generated NSIF 1.0: {:?}", reader.err());

        let reader = reader.unwrap();
        assert_eq!(reader.format(), NitfFormat::Nsif10);

        let keys = reader.get_asset_keys(None, None);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "image_segment_0");
    }

    #[test]
    fn test_generate_multi_segment_nitf() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("multi_segment.ntf");

        let result = generate_multi_segment_nitf(&path);
        assert!(result.is_ok(), "Failed to generate multi-segment NITF: {:?}", result.err());

        // Verify the file can be read back
        let reader = JBPDatasetReader::open(&path);
        assert!(reader.is_ok(), "Failed to read generated multi-segment NITF: {:?}", reader.err());

        let reader = reader.unwrap();
        assert_eq!(reader.format(), NitfFormat::Nitf21);

        // Check segment counts
        let offsets = reader.segment_offsets();
        assert_eq!(offsets.images.len(), 2, "Expected 2 image segments");
        assert_eq!(offsets.text.len(), 1, "Expected 1 text segment");
        assert_eq!(offsets.graphics.len(), 1, "Expected 1 graphic segment");
        assert_eq!(offsets.des.len(), 1, "Expected 1 DES segment");

        // Verify all keys are present
        let keys = reader.get_asset_keys(None, None);
        assert_eq!(keys.len(), 5);
        assert!(keys.contains(&"image_segment_0".to_string()));
        assert!(keys.contains(&"image_segment_1".to_string()));
        assert!(keys.contains(&"text_segment_0".to_string()));
        assert!(keys.contains(&"graphic_segment_0".to_string()));
        assert!(keys.contains(&"des_segment_0".to_string()));
    }

    #[test]
    fn test_round_trip_image_data() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("round_trip.ntf");

        // Create specific image data
        let original_data: Vec<u8> = (0..64).collect();
        let image = Arc::new(TestAssetProvider::new(
            "image_segment_0",
            AssetType::Image,
            original_data.clone(),
        ));

        // Write
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        writer.add_asset("image_segment_0", image, "Test", "", &[]).unwrap();
        writer.close().unwrap();

        // Read back
        let reader = JBPDatasetReader::open(&path).unwrap();
        let asset = reader.get_asset("image_segment_0").unwrap();
        let read_data = asset.raw_asset().unwrap();

        // Verify data matches
        assert_eq!(read_data, original_data, "Image data mismatch after round-trip");
    }

    #[test]
    fn test_round_trip_text_data() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("round_trip_text.ntf");

        let original_text = "Hello, NITF World! This is a test.";
        let text = Arc::new(TestAssetProvider::text("text_segment_0", original_text));

        // Write
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        writer.add_asset("text_segment_0", text, "Test", "", &[]).unwrap();
        writer.close().unwrap();

        // Read back
        let reader = JBPDatasetReader::open(&path).unwrap();
        let asset = reader.get_asset("text_segment_0").unwrap();
        let read_data = asset.raw_asset().unwrap();

        // Verify data matches
        assert_eq!(
            String::from_utf8_lossy(&read_data),
            original_text,
            "Text data mismatch after round-trip"
        );
    }
}
