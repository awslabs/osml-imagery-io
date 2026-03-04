//! Asset providers for NITF segment types.
//!
//! This module provides asset provider implementations for each NITF segment type:
//! - [`JBPImageAssetProvider`] - Image segments
//! - [`JBPTextAssetProvider`] - Text segments
//! - [`JBPGraphicsAssetProvider`] - Graphic segments
//! - [`JBPDataAssetProvider`] - Data Extension Segments (DES)
//!
//! Each provider implements the [`AssetProvider`] trait, providing access to
//! segment data and metadata through a unified interface.
//!
//! # Asset Key Generation
//!
//! Asset keys follow a consistent naming pattern: `{type}_segment_{index}`
//!
//! Examples:
//! - `image_segment_0` - First image segment
//! - `text_segment_1` - Second text segment
//! - `graphic_segment_0` - First graphic segment
//! - `des_segment_0` - First DES segment
//! - `res_segment_0` - First reserved extension segment
//!
//! Use [`generate_asset_key`] to create keys and [`parse_asset_key`] to parse them.

use std::sync::{Arc, OnceLock};

use crate::error::CodecError;
use crate::jbp::image::decoder::{create_block_decoder, BlockDecoder};
use crate::jbp::image::facade::ImageSubheaderFacade;
use crate::jbp::metadata::JBPSegmentMetadataProvider;
use crate::jbp::types::{NitfFormat, SegmentLocation, SegmentType};
use crate::parser::StructureRegistry;
use crate::traits::{AssetProvider, ImageAssetProvider, MetadataProvider};
use crate::types::{AssetType, PixelType};

/// Generate an asset key from segment type and index.
///
/// Asset keys follow the pattern `{type}_segment_{index}` where:
/// - `type` is the segment type prefix (image, graphic, text, des, res)
/// - `index` is the zero-based segment index within that type
///
/// # Arguments
/// * `segment_type` - The type of NITF segment
/// * `index` - Zero-based index of the segment within its type
///
/// # Returns
/// A string key in the format `{type}_segment_{index}`
///
/// # Examples
///
/// ```ignore
/// use _io::jbp::asset::generate_asset_key;
/// use _io::jbp::types::SegmentType;
///
/// assert_eq!(generate_asset_key(SegmentType::Image, 0), "image_segment_0");
/// assert_eq!(generate_asset_key(SegmentType::Text, 2), "text_segment_2");
/// assert_eq!(generate_asset_key(SegmentType::DataExtension, 0), "des_segment_0");
/// ```
pub fn generate_asset_key(segment_type: SegmentType, index: usize) -> String {
    format!("{}_segment_{}", segment_type.key_prefix(), index)
}

/// Parse an asset key to extract segment type and index.
///
/// This function parses keys in the format `{type}_segment_{index}` and returns
/// the corresponding segment type and index. Returns `None` if the key format
/// is invalid.
///
/// # Arguments
/// * `key` - The asset key to parse
///
/// # Returns
/// `Some((SegmentType, usize))` if the key is valid, `None` otherwise
///
/// # Examples
///
/// ```ignore
/// use _io::jbp::asset::parse_asset_key;
/// use _io::jbp::types::SegmentType;
///
/// assert_eq!(parse_asset_key("image_segment_0"), Some((SegmentType::Image, 0)));
/// assert_eq!(parse_asset_key("text_segment_5"), Some((SegmentType::Text, 5)));
/// assert_eq!(parse_asset_key("des_segment_0"), Some((SegmentType::DataExtension, 0)));
/// assert_eq!(parse_asset_key("invalid_key"), None);
/// assert_eq!(parse_asset_key("image_segment_abc"), None);
/// ```
pub fn parse_asset_key(key: &str) -> Option<(SegmentType, usize)> {
    let parts: Vec<&str> = key.split('_').collect();
    
    // Expected format: {type}_segment_{index} -> 3 parts
    if parts.len() != 3 || parts[1] != "segment" {
        return None;
    }
    
    let segment_type = SegmentType::from_key_prefix(parts[0])?;
    let index = parts[2].parse().ok()?;
    
    Some((segment_type, index))
}

/// Asset provider for NITF image segments.
///
/// Provides access to image segment data and metadata through the [`AssetProvider`]
/// trait. Image segments contain raster imagery data with associated subheader
/// metadata describing dimensions, compression, and other image properties.
///
/// # Example
///
/// ```ignore
/// let asset = reader.get_asset("image_segment_0")?;
/// assert_eq!(asset.asset_type(), AssetType::Image);
/// assert_eq!(asset.media_type(), "application/vnd.nitf.image");
///
/// // Access raw image data
/// let data = asset.raw_asset()?;
///
/// // Access image metadata
/// let metadata = asset.metadata();
/// let fields = metadata.as_dict(None);
/// ```
pub struct JBPImageAssetProvider {
    /// Unique key identifying this asset
    key: String,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles for this asset
    roles: Vec<String>,
    /// Segment location in the file
    location: SegmentLocation,
    /// Reference to the file data
    data: Arc<[u8]>,
    /// Segment metadata provider
    metadata: Arc<JBPSegmentMetadataProvider>,
    /// Structure registry for parsing
    registry: Arc<StructureRegistry>,
    /// NITF format variant
    format: NitfFormat,
    /// Lazy-initialized block decoder
    decoder: OnceLock<Box<dyn BlockDecoder>>,
}

impl JBPImageAssetProvider {
    /// Create a new image asset provider.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this asset
    /// * `title` - Human-readable title
    /// * `description` - Detailed description
    /// * `roles` - Semantic roles
    /// * `location` - Segment location in the file
    /// * `data` - Reference to the file data
    /// * `metadata` - Segment metadata provider
    /// * `registry` - Structure registry for parsing
    /// * `format` - NITF format variant
    pub fn new(
        key: String,
        title: String,
        description: String,
        roles: Vec<String>,
        location: SegmentLocation,
        data: Arc<[u8]>,
        metadata: Arc<JBPSegmentMetadataProvider>,
        registry: Arc<StructureRegistry>,
        format: NitfFormat,
    ) -> Self {
        Self {
            key,
            title,
            description,
            roles,
            location,
            data,
            metadata,
            registry,
            format,
            decoder: OnceLock::new(),
        }
    }

    /// Get the subheader bytes for this image segment.
    fn subheader_bytes(&self) -> &[u8] {
        let start = self.location.subheader_offset as usize;
        let end = start + self.location.subheader_length as usize;
        &self.data[start..end]
    }

    /// Get the image data bytes for this image segment.
    fn image_data(&self) -> Arc<[u8]> {
        let start = self.location.data_offset as usize;
        let end = start + self.location.data_length as usize;
        Arc::from(&self.data[start..end])
    }

    /// Get or create the block decoder.
    fn decoder(&self) -> Result<&dyn BlockDecoder, CodecError> {
        // Try to get existing decoder first
        if let Some(decoder) = self.decoder.get() {
            return Ok(decoder.as_ref());
        }
        
        // Initialize the decoder
        let subheader_bytes = self.subheader_bytes();
        let facade = ImageSubheaderFacade::from_bytes(
            subheader_bytes,
            &self.registry,
            self.format,
        )?;
        let image_data = self.image_data();
        let new_decoder = create_block_decoder(&facade, image_data)?;
        
        // Try to set it (another thread might have set it already)
        let _ = self.decoder.set(new_decoder);
        
        // Return the decoder (either ours or the one another thread set)
        Ok(self.decoder.get().unwrap().as_ref())
    }

    /// Get the subheader facade for metadata access.
    fn subheader(&self) -> Result<ImageSubheaderFacade<'_>, CodecError> {
        let subheader_bytes = self.subheader_bytes();
        ImageSubheaderFacade::from_bytes(subheader_bytes, &self.registry, self.format)
    }
}

impl AssetProvider for JBPImageAssetProvider {
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
        "application/vnd.nitf.image"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Image
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        let start = self.location.data_offset as usize;
        let end = start + self.location.data_length as usize;

        if end > self.data.len() {
            return Err(CodecError::Decode(format!(
                "Image segment data extends beyond file: offset {} + length {} > file size {}",
                start,
                self.location.data_length,
                self.data.len()
            )));
        }

        Ok(self.data[start..end].to_vec())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ImageAssetProvider for JBPImageAssetProvider {
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        if resolution_level != 0 {
            // Check if decoder supports multiple resolution levels
            if let Ok(decoder) = self.decoder() {
                if resolution_level >= decoder.num_resolution_levels() {
                    return false;
                }
            } else {
                return false;
            }
        }
        self.decoder()
            .map(|d| d.has_block(block_row, block_col))
            .unwrap_or(false)
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        let decoder = self.decoder()?;
        if resolution_level >= decoder.num_resolution_levels() {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }
        decoder.decode_block(block_row, block_col, resolution_level, bands)
    }

    fn num_resolution_levels(&self) -> u32 {
        self.decoder()
            .map(|d| d.num_resolution_levels())
            .unwrap_or(1)
    }

    fn num_bands(&self) -> u32 {
        self.subheader()
            .and_then(|s| s.band_count())
            .map(|c| c as u32)
            .unwrap_or(1)
    }

    fn num_rows(&self) -> u32 {
        self.subheader().and_then(|s| s.nrows()).unwrap_or(0)
    }

    fn num_columns(&self) -> u32 {
        self.subheader().and_then(|s| s.ncols()).unwrap_or(0)
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.subheader().and_then(|s| s.nppbh()).unwrap_or(0)
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.subheader().and_then(|s| s.nppbv()).unwrap_or(0)
    }

    fn num_bits_per_pixel(&self) -> u32 {
        self.subheader()
            .and_then(|s| s.nbpp())
            .map(|n| n as u32)
            .unwrap_or(8)
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        self.subheader()
            .and_then(|s| s.abpp())
            .map(|n| n as u32)
            .unwrap_or(8)
    }

    fn pixel_value_type(&self) -> PixelType {
        self.subheader()
            .and_then(|s| {
                let pvtype = s.pvtype()?;
                let nbpp = s.nbpp()?;
                Ok(pvtype.to_pixel_type(nbpp))
            })
            .unwrap_or(PixelType::UInt8)
    }

    fn pad_pixel_value(&self) -> f64 {
        0.0
    }
}


/// Asset provider for NITF text segments.
///
/// Provides access to text segment data and metadata through the [`AssetProvider`]
/// trait. Text segments contain plain text content with associated subheader
/// metadata.
///
/// # Example
///
/// ```ignore
/// let asset = reader.get_asset("text_segment_0")?;
/// assert_eq!(asset.asset_type(), AssetType::Text);
/// assert_eq!(asset.media_type(), "text/plain");
///
/// // Access raw text data
/// let data = asset.raw_asset()?;
/// let text = String::from_utf8_lossy(&data);
/// ```
pub struct JBPTextAssetProvider {
    /// Unique key identifying this asset
    key: String,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles for this asset
    roles: Vec<String>,
    /// Segment location in the file
    location: SegmentLocation,
    /// Reference to the file data
    data: Arc<[u8]>,
    /// Segment metadata provider
    metadata: Arc<JBPSegmentMetadataProvider>,
}

impl JBPTextAssetProvider {
    /// Create a new text asset provider.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this asset
    /// * `title` - Human-readable title
    /// * `description` - Detailed description
    /// * `roles` - Semantic roles
    /// * `location` - Segment location in the file
    /// * `data` - Reference to the file data
    /// * `metadata` - Segment metadata provider
    pub fn new(
        key: String,
        title: String,
        description: String,
        roles: Vec<String>,
        location: SegmentLocation,
        data: Arc<[u8]>,
        metadata: Arc<JBPSegmentMetadataProvider>,
    ) -> Self {
        Self {
            key,
            title,
            description,
            roles,
            location,
            data,
            metadata,
        }
    }
}

impl AssetProvider for JBPTextAssetProvider {
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
        "text/plain"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Text
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        let start = self.location.data_offset as usize;
        let end = start + self.location.data_length as usize;

        if end > self.data.len() {
            return Err(CodecError::Decode(format!(
                "Text segment data extends beyond file: offset {} + length {} > file size {}",
                start,
                self.location.data_length,
                self.data.len()
            )));
        }

        Ok(self.data[start..end].to_vec())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Asset provider for NITF graphic segments.
///
/// Provides access to graphic segment data and metadata through the [`AssetProvider`]
/// trait. Graphic segments contain CGM (Computer Graphics Metafile) vector graphics
/// with associated subheader metadata.
///
/// # Example
///
/// ```ignore
/// let asset = reader.get_asset("graphic_segment_0")?;
/// assert_eq!(asset.asset_type(), AssetType::Graphics);
/// assert_eq!(asset.media_type(), "image/cgm");
///
/// // Access raw CGM data
/// let data = asset.raw_asset()?;
/// ```
pub struct JBPGraphicsAssetProvider {
    /// Unique key identifying this asset
    key: String,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles for this asset
    roles: Vec<String>,
    /// Segment location in the file
    location: SegmentLocation,
    /// Reference to the file data
    data: Arc<[u8]>,
    /// Segment metadata provider
    metadata: Arc<JBPSegmentMetadataProvider>,
}

impl JBPGraphicsAssetProvider {
    /// Create a new graphics asset provider.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this asset
    /// * `title` - Human-readable title
    /// * `description` - Detailed description
    /// * `roles` - Semantic roles
    /// * `location` - Segment location in the file
    /// * `data` - Reference to the file data
    /// * `metadata` - Segment metadata provider
    pub fn new(
        key: String,
        title: String,
        description: String,
        roles: Vec<String>,
        location: SegmentLocation,
        data: Arc<[u8]>,
        metadata: Arc<JBPSegmentMetadataProvider>,
    ) -> Self {
        Self {
            key,
            title,
            description,
            roles,
            location,
            data,
            metadata,
        }
    }
}

impl AssetProvider for JBPGraphicsAssetProvider {
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
        "image/cgm"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Graphics
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        let start = self.location.data_offset as usize;
        let end = start + self.location.data_length as usize;

        if end > self.data.len() {
            return Err(CodecError::Decode(format!(
                "Graphic segment data extends beyond file: offset {} + length {} > file size {}",
                start,
                self.location.data_length,
                self.data.len()
            )));
        }

        Ok(self.data[start..end].to_vec())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Asset provider for NITF Data Extension Segments (DES).
///
/// Provides access to DES data and metadata through the [`AssetProvider`]
/// trait. DES segments contain structured data such as XML, TRE overflow,
/// or other application-specific content.
///
/// # Example
///
/// ```ignore
/// let asset = reader.get_asset("des_segment_0")?;
/// assert_eq!(asset.asset_type(), AssetType::Data);
/// assert_eq!(asset.media_type(), "application/octet-stream");
///
/// // Access raw DES data
/// let data = asset.raw_asset()?;
/// ```
pub struct JBPDataAssetProvider {
    /// Unique key identifying this asset
    key: String,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles for this asset
    roles: Vec<String>,
    /// Segment location in the file
    location: SegmentLocation,
    /// Reference to the file data
    data: Arc<[u8]>,
    /// Segment metadata provider
    metadata: Arc<JBPSegmentMetadataProvider>,
}

impl JBPDataAssetProvider {
    /// Create a new data asset provider.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this asset
    /// * `title` - Human-readable title
    /// * `description` - Detailed description
    /// * `roles` - Semantic roles
    /// * `location` - Segment location in the file
    /// * `data` - Reference to the file data
    /// * `metadata` - Segment metadata provider
    pub fn new(
        key: String,
        title: String,
        description: String,
        roles: Vec<String>,
        location: SegmentLocation,
        data: Arc<[u8]>,
        metadata: Arc<JBPSegmentMetadataProvider>,
    ) -> Self {
        Self {
            key,
            title,
            description,
            roles,
            location,
            data,
            metadata,
        }
    }
}

impl AssetProvider for JBPDataAssetProvider {
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
        "application/octet-stream"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Data
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        let start = self.location.data_offset as usize;
        let end = start + self.location.data_length as usize;

        if end > self.data.len() {
            return Err(CodecError::Decode(format!(
                "DES segment data extends beyond file: offset {} + length {} > file size {}",
                start,
                self.location.data_length,
                self.data.len()
            )));
        }

        Ok(self.data[start..end].to_vec())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{FieldDefinition, FieldType, SizeSpec, StructureDefinition};

    // Asset key generation tests
    #[test]
    fn generate_asset_key_image() {
        assert_eq!(generate_asset_key(SegmentType::Image, 0), "image_segment_0");
        assert_eq!(generate_asset_key(SegmentType::Image, 5), "image_segment_5");
        assert_eq!(generate_asset_key(SegmentType::Image, 999), "image_segment_999");
    }

    #[test]
    fn generate_asset_key_graphic() {
        assert_eq!(generate_asset_key(SegmentType::Graphic, 0), "graphic_segment_0");
        assert_eq!(generate_asset_key(SegmentType::Graphic, 3), "graphic_segment_3");
    }

    #[test]
    fn generate_asset_key_text() {
        assert_eq!(generate_asset_key(SegmentType::Text, 0), "text_segment_0");
        assert_eq!(generate_asset_key(SegmentType::Text, 1), "text_segment_1");
    }

    #[test]
    fn generate_asset_key_des() {
        assert_eq!(generate_asset_key(SegmentType::DataExtension, 0), "des_segment_0");
        assert_eq!(generate_asset_key(SegmentType::DataExtension, 2), "des_segment_2");
    }

    #[test]
    fn generate_asset_key_res() {
        assert_eq!(generate_asset_key(SegmentType::ReservedExtension, 0), "res_segment_0");
        assert_eq!(generate_asset_key(SegmentType::ReservedExtension, 1), "res_segment_1");
    }

    #[test]
    fn parse_asset_key_image() {
        assert_eq!(parse_asset_key("image_segment_0"), Some((SegmentType::Image, 0)));
        assert_eq!(parse_asset_key("image_segment_5"), Some((SegmentType::Image, 5)));
        assert_eq!(parse_asset_key("image_segment_999"), Some((SegmentType::Image, 999)));
    }

    #[test]
    fn parse_asset_key_graphic() {
        assert_eq!(parse_asset_key("graphic_segment_0"), Some((SegmentType::Graphic, 0)));
        assert_eq!(parse_asset_key("graphic_segment_3"), Some((SegmentType::Graphic, 3)));
    }

    #[test]
    fn parse_asset_key_text() {
        assert_eq!(parse_asset_key("text_segment_0"), Some((SegmentType::Text, 0)));
        assert_eq!(parse_asset_key("text_segment_1"), Some((SegmentType::Text, 1)));
    }

    #[test]
    fn parse_asset_key_des() {
        assert_eq!(parse_asset_key("des_segment_0"), Some((SegmentType::DataExtension, 0)));
        assert_eq!(parse_asset_key("des_segment_2"), Some((SegmentType::DataExtension, 2)));
    }

    #[test]
    fn parse_asset_key_res() {
        assert_eq!(parse_asset_key("res_segment_0"), Some((SegmentType::ReservedExtension, 0)));
        assert_eq!(parse_asset_key("res_segment_1"), Some((SegmentType::ReservedExtension, 1)));
    }

    #[test]
    fn parse_asset_key_invalid_format() {
        // Wrong number of parts
        assert_eq!(parse_asset_key("image"), None);
        assert_eq!(parse_asset_key("image_segment"), None);
        assert_eq!(parse_asset_key("image_segment_0_extra"), None);
        
        // Wrong middle part
        assert_eq!(parse_asset_key("image_seg_0"), None);
        assert_eq!(parse_asset_key("image_data_0"), None);
        
        // Invalid type prefix
        assert_eq!(parse_asset_key("unknown_segment_0"), None);
        assert_eq!(parse_asset_key("img_segment_0"), None);
        
        // Invalid index
        assert_eq!(parse_asset_key("image_segment_abc"), None);
        assert_eq!(parse_asset_key("image_segment_-1"), None);
        assert_eq!(parse_asset_key("image_segment_"), None);
        
        // Empty string
        assert_eq!(parse_asset_key(""), None);
    }

    #[test]
    fn parse_asset_key_roundtrip() {
        // Test that generate -> parse produces the same values
        for segment_type in [
            SegmentType::Image,
            SegmentType::Graphic,
            SegmentType::Text,
            SegmentType::DataExtension,
            SegmentType::ReservedExtension,
        ] {
            for index in [0, 1, 5, 10, 100, 999] {
                let key = generate_asset_key(segment_type, index);
                let parsed = parse_asset_key(&key);
                assert_eq!(parsed, Some((segment_type, index)),
                    "Roundtrip failed for {:?} index {}", segment_type, index);
            }
        }
    }

    /// Create a simple test structure definition for segment subheaders.
    fn create_test_definition() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("TestSubheader")
                .with_field(
                    FieldDefinition::new("ID", FieldType::String)
                        .with_size(SizeSpec::Fixed(10))
                        .with_doc("Segment identifier"),
                )
                .with_field(
                    FieldDefinition::new("TITLE", FieldType::String)
                        .with_size(SizeSpec::Fixed(20))
                        .with_doc("Segment title"),
                ),
        )
    }

    /// Create test data: subheader (30 bytes) + segment data
    fn create_test_file_data(segment_data: &[u8]) -> Arc<[u8]> {
        let mut data = Vec::new();
        // Subheader: ID (10 bytes) + TITLE (20 bytes) = 30 bytes
        data.extend_from_slice(b"IMG_00001 Test Image Title    ");
        // Segment data
        data.extend_from_slice(segment_data);
        Arc::from(data)
    }

    fn create_test_metadata(definition: Arc<StructureDefinition>) -> Arc<JBPSegmentMetadataProvider> {
        let raw_bytes: Arc<[u8]> = Arc::from(b"IMG_00001 Test Image Title    ".as_slice());
        Arc::new(JBPSegmentMetadataProvider::from_definition(definition, raw_bytes))
    }

    /// Create a test structure registry.
    fn create_test_registry() -> Arc<StructureRegistry> {
        Arc::new(StructureRegistry::new())
    }

    /// Get the default test format.
    fn test_format() -> NitfFormat {
        NitfFormat::Nitf21
    }

    // JBPImageAssetProvider tests
    #[test]
    fn image_provider_key() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        assert_eq!(provider.key(), "image_segment_0");
    }

    #[test]
    fn image_provider_title() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        assert_eq!(provider.title(), "Test Image");
    }

    #[test]
    fn image_provider_description() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        assert_eq!(provider.description(), "A test image segment");
    }

    #[test]
    fn image_provider_media_type() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        assert_eq!(provider.media_type(), "application/vnd.nitf.image");
    }

    #[test]
    fn image_provider_roles() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string(), "thumbnail".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        assert_eq!(provider.roles(), &["data", "thumbnail"]);
    }

    #[test]
    fn image_provider_asset_type() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        assert_eq!(provider.asset_type(), AssetType::Image);
    }

    #[test]
    fn image_provider_raw_asset() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        let raw = provider.raw_asset().unwrap();
        assert_eq!(raw, segment_data);
    }

    #[test]
    fn image_provider_raw_asset_out_of_bounds() {
        let definition = create_test_definition();
        let segment_data = b"short";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        // Location claims more data than exists
        let location = SegmentLocation::new(0, 30, 30, 1000);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        let result = provider.raw_asset();
        assert!(result.is_err());
    }

    #[test]
    fn image_provider_metadata() {
        let definition = create_test_definition();
        let segment_data = b"image pixel data here";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);
        let registry = create_test_registry();

        let provider = JBPImageAssetProvider::new(
            "image_segment_0".to_string(),
            "Test Image".to_string(),
            "A test image segment".to_string(),
            vec!["data".to_string()],
            location,
            file_data,
            metadata,
            registry,
            test_format(),
        );

        let meta = provider.metadata();
        let dict = meta.as_dict(None);
        assert!(dict.contains_key("ID"));
        assert!(dict.contains_key("TITLE"));
    }

    // JBPTextAssetProvider tests
    #[test]
    fn text_provider_media_type() {
        let definition = create_test_definition();
        let segment_data = b"This is some text content";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPTextAssetProvider::new(
            "text_segment_0".to_string(),
            "Test Text".to_string(),
            "A test text segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        assert_eq!(provider.media_type(), "text/plain");
    }

    #[test]
    fn text_provider_asset_type() {
        let definition = create_test_definition();
        let segment_data = b"This is some text content";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPTextAssetProvider::new(
            "text_segment_0".to_string(),
            "Test Text".to_string(),
            "A test text segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        assert_eq!(provider.asset_type(), AssetType::Text);
    }

    #[test]
    fn text_provider_raw_asset() {
        let definition = create_test_definition();
        let segment_data = b"This is some text content";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPTextAssetProvider::new(
            "text_segment_0".to_string(),
            "Test Text".to_string(),
            "A test text segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        let raw = provider.raw_asset().unwrap();
        assert_eq!(raw, segment_data);
    }

    // JBPGraphicsAssetProvider tests
    #[test]
    fn graphics_provider_media_type() {
        let definition = create_test_definition();
        let segment_data = b"CGM graphics data";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPGraphicsAssetProvider::new(
            "graphic_segment_0".to_string(),
            "Test Graphic".to_string(),
            "A test graphic segment".to_string(),
            vec!["annotation".to_string()],
            location,
            file_data,
            metadata,
        );

        assert_eq!(provider.media_type(), "image/cgm");
    }

    #[test]
    fn graphics_provider_asset_type() {
        let definition = create_test_definition();
        let segment_data = b"CGM graphics data";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPGraphicsAssetProvider::new(
            "graphic_segment_0".to_string(),
            "Test Graphic".to_string(),
            "A test graphic segment".to_string(),
            vec!["annotation".to_string()],
            location,
            file_data,
            metadata,
        );

        assert_eq!(provider.asset_type(), AssetType::Graphics);
    }

    #[test]
    fn graphics_provider_raw_asset() {
        let definition = create_test_definition();
        let segment_data = b"CGM graphics data";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPGraphicsAssetProvider::new(
            "graphic_segment_0".to_string(),
            "Test Graphic".to_string(),
            "A test graphic segment".to_string(),
            vec!["annotation".to_string()],
            location,
            file_data,
            metadata,
        );

        let raw = provider.raw_asset().unwrap();
        assert_eq!(raw, segment_data);
    }

    // JBPDataAssetProvider tests
    #[test]
    fn data_provider_media_type() {
        let definition = create_test_definition();
        let segment_data = b"<xml>DES data</xml>";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPDataAssetProvider::new(
            "des_segment_0".to_string(),
            "Test DES".to_string(),
            "A test DES segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        assert_eq!(provider.media_type(), "application/octet-stream");
    }

    #[test]
    fn data_provider_asset_type() {
        let definition = create_test_definition();
        let segment_data = b"<xml>DES data</xml>";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPDataAssetProvider::new(
            "des_segment_0".to_string(),
            "Test DES".to_string(),
            "A test DES segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        assert_eq!(provider.asset_type(), AssetType::Data);
    }

    #[test]
    fn data_provider_raw_asset() {
        let definition = create_test_definition();
        let segment_data = b"<xml>DES data</xml>";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        let location = SegmentLocation::new(0, 30, 30, segment_data.len() as u64);

        let provider = JBPDataAssetProvider::new(
            "des_segment_0".to_string(),
            "Test DES".to_string(),
            "A test DES segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        let raw = provider.raw_asset().unwrap();
        assert_eq!(raw, segment_data);
    }

    #[test]
    fn data_provider_raw_asset_out_of_bounds() {
        let definition = create_test_definition();
        let segment_data = b"short";
        let file_data = create_test_file_data(segment_data);
        let metadata = create_test_metadata(definition);
        // Location claims more data than exists
        let location = SegmentLocation::new(0, 30, 30, 1000);

        let provider = JBPDataAssetProvider::new(
            "des_segment_0".to_string(),
            "Test DES".to_string(),
            "A test DES segment".to_string(),
            vec!["metadata".to_string()],
            location,
            file_data,
            metadata,
        );

        let result = provider.raw_asset();
        assert!(result.is_err());
    }
}


/// Property-based tests for asset key generation.
///
/// These tests verify the correctness properties for asset key generation
/// and parsing as specified in the design document.
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy to generate valid segment types
    fn segment_type_strategy() -> impl Strategy<Value = SegmentType> {
        prop_oneof![
            Just(SegmentType::Image),
            Just(SegmentType::Graphic),
            Just(SegmentType::Text),
            Just(SegmentType::DataExtension),
            Just(SegmentType::ReservedExtension),
        ]
    }

    /// Property 4: Asset Key Enumeration Completeness
    /// For any NITF file with segment counts (NUMI, NUMS, NUMT, NUMDES, NUMRES),
    /// the `get_asset_keys(None, None)` SHALL return exactly NUMI + NUMS + NUMT + NUMDES + NUMRES keys,
    /// and each key SHALL match the pattern `{type}_segment_{index}`.
    /// **Validates: Requirements 3.1, 3.6**
    ///
    /// Note: Since JBPDatasetReader is not yet implemented, we test the underlying
    /// key generation logic that will be used by get_asset_keys().
    mod prop_4_asset_key_enumeration_completeness {
        use super::*;

        /// Helper function to generate all asset keys for given segment counts.
        /// This simulates what get_asset_keys(None, None) will do.
        fn generate_all_asset_keys(
            numi: usize,
            nums: usize,
            numt: usize,
            numdes: usize,
            numres: usize,
        ) -> Vec<String> {
            let mut keys = Vec::with_capacity(numi + nums + numt + numdes + numres);

            for i in 0..numi {
                keys.push(generate_asset_key(SegmentType::Image, i));
            }
            for i in 0..nums {
                keys.push(generate_asset_key(SegmentType::Graphic, i));
            }
            for i in 0..numt {
                keys.push(generate_asset_key(SegmentType::Text, i));
            }
            for i in 0..numdes {
                keys.push(generate_asset_key(SegmentType::DataExtension, i));
            }
            for i in 0..numres {
                keys.push(generate_asset_key(SegmentType::ReservedExtension, i));
            }

            keys
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Total key count equals sum of all segment counts
            #[test]
            fn total_key_count_equals_segment_sum(
                numi in 0usize..10,
                nums in 0usize..10,
                numt in 0usize..10,
                numdes in 0usize..10,
                numres in 0usize..10,
            ) {
                let keys = generate_all_asset_keys(numi, nums, numt, numdes, numres);
                let expected_total = numi + nums + numt + numdes + numres;

                prop_assert_eq!(keys.len(), expected_total,
                    "Expected {} keys, got {}", expected_total, keys.len());
            }

            /// All generated keys match the pattern {type}_segment_{index}
            #[test]
            fn all_keys_match_pattern(
                numi in 0usize..5,
                nums in 0usize..5,
                numt in 0usize..5,
                numdes in 0usize..5,
                numres in 0usize..5,
            ) {
                let keys = generate_all_asset_keys(numi, nums, numt, numdes, numres);

                for key in &keys {
                    prop_assert!(key.contains("_segment_"),
                        "Key '{}' does not contain '_segment_'", key);

                    // Verify key can be parsed back
                    let parsed = parse_asset_key(key);
                    prop_assert!(parsed.is_some(),
                        "Key '{}' could not be parsed", key);
                }
            }

            /// All generated keys are unique
            #[test]
            fn all_keys_unique(
                numi in 0usize..5,
                nums in 0usize..5,
                numt in 0usize..5,
                numdes in 0usize..5,
                numres in 0usize..5,
            ) {
                let keys = generate_all_asset_keys(numi, nums, numt, numdes, numres);
                let unique_keys: std::collections::HashSet<_> = keys.iter().collect();

                prop_assert_eq!(keys.len(), unique_keys.len(),
                    "Found duplicate keys");
            }

            /// Image segment keys are correctly counted
            #[test]
            fn image_key_count_correct(numi in 0usize..20) {
                let keys = generate_all_asset_keys(numi, 0, 0, 0, 0);
                let image_keys: Vec<_> = keys.iter()
                    .filter(|k| k.starts_with("image_"))
                    .collect();

                prop_assert_eq!(image_keys.len(), numi,
                    "Expected {} image keys, got {}", numi, image_keys.len());
            }

            /// Graphic segment keys are correctly counted
            #[test]
            fn graphic_key_count_correct(nums in 0usize..20) {
                let keys = generate_all_asset_keys(0, nums, 0, 0, 0);
                let graphic_keys: Vec<_> = keys.iter()
                    .filter(|k| k.starts_with("graphic_"))
                    .collect();

                prop_assert_eq!(graphic_keys.len(), nums,
                    "Expected {} graphic keys, got {}", nums, graphic_keys.len());
            }

            /// Text segment keys are correctly counted
            #[test]
            fn text_key_count_correct(numt in 0usize..20) {
                let keys = generate_all_asset_keys(0, 0, numt, 0, 0);
                let text_keys: Vec<_> = keys.iter()
                    .filter(|k| k.starts_with("text_"))
                    .collect();

                prop_assert_eq!(text_keys.len(), numt,
                    "Expected {} text keys, got {}", numt, text_keys.len());
            }

            /// DES segment keys are correctly counted
            #[test]
            fn des_key_count_correct(numdes in 0usize..20) {
                let keys = generate_all_asset_keys(0, 0, 0, numdes, 0);
                let des_keys: Vec<_> = keys.iter()
                    .filter(|k| k.starts_with("des_"))
                    .collect();

                prop_assert_eq!(des_keys.len(), numdes,
                    "Expected {} DES keys, got {}", numdes, des_keys.len());
            }

            /// RES segment keys are correctly counted
            #[test]
            fn res_key_count_correct(numres in 0usize..20) {
                let keys = generate_all_asset_keys(0, 0, 0, 0, numres);
                let res_keys: Vec<_> = keys.iter()
                    .filter(|k| k.starts_with("res_"))
                    .collect();

                prop_assert_eq!(res_keys.len(), numres,
                    "Expected {} RES keys, got {}", numres, res_keys.len());
            }
        }
    }

    /// Property 5: Asset Key Type Filtering
    /// For any NITF file and any asset type filter, `get_asset_keys(Some(type), None)`
    /// SHALL return only keys whose segment type matches the filter, and the count
    /// SHALL equal the corresponding segment count field.
    /// **Validates: Requirements 3.2, 3.3, 3.4, 3.5**
    ///
    /// Note: Since JBPDatasetReader is not yet implemented, we test the underlying
    /// key generation and filtering logic.
    mod prop_5_asset_key_type_filtering {
        use super::*;

        /// Helper function to generate asset keys filtered by type.
        /// This simulates what get_asset_keys(Some(type), None) will do.
        fn generate_filtered_asset_keys(
            segment_type: SegmentType,
            numi: usize,
            nums: usize,
            numt: usize,
            numdes: usize,
            numres: usize,
        ) -> Vec<String> {
            let count = match segment_type {
                SegmentType::Image => numi,
                SegmentType::Graphic => nums,
                SegmentType::Text => numt,
                SegmentType::DataExtension => numdes,
                SegmentType::ReservedExtension => numres,
            };

            (0..count)
                .map(|i| generate_asset_key(segment_type, i))
                .collect()
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Filtered keys only contain the requested type
            #[test]
            fn filtered_keys_match_type(
                segment_type in segment_type_strategy(),
                count in 0usize..20,
            ) {
                let keys = generate_filtered_asset_keys(segment_type, count, count, count, count, count);
                let expected_prefix = segment_type.key_prefix();

                for key in &keys {
                    prop_assert!(key.starts_with(expected_prefix),
                        "Key '{}' does not start with '{}'", key, expected_prefix);

                    // Verify parsed type matches
                    if let Some((parsed_type, _)) = parse_asset_key(key) {
                        prop_assert_eq!(parsed_type, segment_type,
                            "Parsed type {:?} does not match expected {:?}", parsed_type, segment_type);
                    } else {
                        prop_assert!(false, "Failed to parse key '{}'", key);
                    }
                }
            }

            /// Filtered key count equals segment count for that type
            #[test]
            fn filtered_count_equals_segment_count(
                segment_type in segment_type_strategy(),
                numi in 0usize..10,
                nums in 0usize..10,
                numt in 0usize..10,
                numdes in 0usize..10,
                numres in 0usize..10,
            ) {
                let keys = generate_filtered_asset_keys(segment_type, numi, nums, numt, numdes, numres);

                let expected_count = match segment_type {
                    SegmentType::Image => numi,
                    SegmentType::Graphic => nums,
                    SegmentType::Text => numt,
                    SegmentType::DataExtension => numdes,
                    SegmentType::ReservedExtension => numres,
                };

                prop_assert_eq!(keys.len(), expected_count,
                    "Expected {} keys for {:?}, got {}", expected_count, segment_type, keys.len());
            }

            /// Image filter returns only image keys
            #[test]
            fn image_filter_returns_only_images(numi in 0usize..20) {
                let keys = generate_filtered_asset_keys(SegmentType::Image, numi, 5, 5, 5, 5);

                prop_assert_eq!(keys.len(), numi);
                for key in &keys {
                    prop_assert!(key.starts_with("image_segment_"));
                }
            }

            /// Graphic filter returns only graphic keys
            #[test]
            fn graphic_filter_returns_only_graphics(nums in 0usize..20) {
                let keys = generate_filtered_asset_keys(SegmentType::Graphic, 5, nums, 5, 5, 5);

                prop_assert_eq!(keys.len(), nums);
                for key in &keys {
                    prop_assert!(key.starts_with("graphic_segment_"));
                }
            }

            /// Text filter returns only text keys
            #[test]
            fn text_filter_returns_only_text(numt in 0usize..20) {
                let keys = generate_filtered_asset_keys(SegmentType::Text, 5, 5, numt, 5, 5);

                prop_assert_eq!(keys.len(), numt);
                for key in &keys {
                    prop_assert!(key.starts_with("text_segment_"));
                }
            }

            /// DES filter returns only DES keys
            #[test]
            fn des_filter_returns_only_des(numdes in 0usize..20) {
                let keys = generate_filtered_asset_keys(SegmentType::DataExtension, 5, 5, 5, numdes, 5);

                prop_assert_eq!(keys.len(), numdes);
                for key in &keys {
                    prop_assert!(key.starts_with("des_segment_"));
                }
            }

            /// RES filter returns only RES keys
            #[test]
            fn res_filter_returns_only_res(numres in 0usize..20) {
                let keys = generate_filtered_asset_keys(SegmentType::ReservedExtension, 5, 5, 5, 5, numres);

                prop_assert_eq!(keys.len(), numres);
                for key in &keys {
                    prop_assert!(key.starts_with("res_segment_"));
                }
            }

            /// Key indices are sequential starting from 0
            #[test]
            fn key_indices_sequential(
                segment_type in segment_type_strategy(),
                count in 1usize..20,
            ) {
                let keys = generate_filtered_asset_keys(segment_type, count, count, count, count, count);

                for (expected_index, key) in keys.iter().enumerate() {
                    if let Some((_, actual_index)) = parse_asset_key(key) {
                        prop_assert_eq!(actual_index, expected_index,
                            "Expected index {}, got {} for key '{}'", expected_index, actual_index, key);
                    } else {
                        prop_assert!(false, "Failed to parse key '{}'", key);
                    }
                }
            }
        }
    }

    /// Additional property tests for asset key round-trip consistency
    mod prop_asset_key_roundtrip {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Generate then parse produces original values
            #[test]
            fn generate_parse_roundtrip(
                segment_type in segment_type_strategy(),
                index in 0usize..1000,
            ) {
                let key = generate_asset_key(segment_type, index);
                let parsed = parse_asset_key(&key);

                prop_assert_eq!(parsed, Some((segment_type, index)),
                    "Roundtrip failed: generated '{}', parsed {:?}", key, parsed);
            }

            /// Generated keys always have correct format
            #[test]
            fn generated_key_format(
                segment_type in segment_type_strategy(),
                index in 0usize..1000,
            ) {
                let key = generate_asset_key(segment_type, index);
                let expected = format!("{}_segment_{}", segment_type.key_prefix(), index);

                prop_assert_eq!(key, expected);
            }

            /// Parse rejects malformed keys
            #[test]
            fn parse_rejects_malformed(
                prefix in "[a-z]{1,10}",
                middle in "[a-z]{1,10}",
                suffix in "[a-z0-9]{1,10}",
            ) {
                // Skip if we accidentally generate a valid key
                let key = format!("{}_{}_{}",  prefix, middle, suffix);
                if middle == "segment" {
                    if SegmentType::from_key_prefix(&prefix).is_some() {
                        if suffix.parse::<usize>().is_ok() {
                            // This is actually a valid key, skip
                            return Ok(());
                        }
                    }
                }

                let parsed = parse_asset_key(&key);
                // Either None or the middle part wasn't "segment"
                if middle != "segment" {
                    prop_assert_eq!(parsed, None,
                        "Expected None for malformed key '{}', got {:?}", key, parsed);
                }
            }
        }
    }

    /// Property 14: ImageAssetProvider Trait Compliance
    /// For any JBPImageAssetProvider instance, the trait methods SHALL return values
    /// consistent with the underlying ImageSubheader: num_rows() = NROWS,
    /// num_columns() = NCOLS, num_bands() = band count, etc.
    /// **Validates: Requirements 18.1-18.7**
    mod prop_14_image_asset_provider_trait_compliance {
        use super::*;
        use crate::jbp::metadata::JBPSegmentMetadataProvider;
        use crate::parser::{FieldDefinition, FieldType, SizeSpec, StructureDefinition};
        use crate::traits::ImageAssetProvider;

        /// Create a test structure definition (same as in tests module)
        fn create_test_definition() -> Arc<StructureDefinition> {
            Arc::new(
                StructureDefinition::new("TestSubheader")
                    .with_field(
                        FieldDefinition::new("ID", FieldType::String)
                            .with_size(SizeSpec::Fixed(10))
                            .with_doc("Segment identifier"),
                    )
                    .with_field(
                        FieldDefinition::new("TITLE", FieldType::String)
                            .with_size(SizeSpec::Fixed(20))
                            .with_doc("Segment title"),
                    ),
            )
        }

        /// Create a test structure registry
        fn create_test_registry() -> Arc<StructureRegistry> {
            Arc::new(StructureRegistry::new())
        }

        /// Get the default test format
        fn test_format() -> NitfFormat {
            NitfFormat::Nitf21
        }

        /// Helper to create a valid NITF image subheader with configurable parameters.
        /// Returns (subheader_bytes, image_data, expected_values)
        fn create_test_image_segment(
            nrows: u32,
            ncols: u32,
            nbands: u8,
            nbpp: u8,
            nppbh: u32,
            nppbv: u32,
        ) -> (Vec<u8>, Vec<u8>, TestExpectedValues) {
            let mut subheader = Vec::new();

            // IM (2) - Image segment marker
            subheader.extend_from_slice(b"IM");

            // IID1 (10) - Image identifier 1
            subheader.extend_from_slice(b"TestImg01 ");

            // IDATIM (14) - Image date and time
            subheader.extend_from_slice(b"20240101120000");

            // TGTID (17) - Target identifier
            subheader.extend_from_slice(&[b' '; 17]);

            // IID2 (80) - Image identifier 2
            subheader.extend_from_slice(&[b' '; 80]);

            // Security fields (167 bytes total)
            subheader.push(b'U');           // ISCLAS (1)
            subheader.extend_from_slice(&[b' '; 2]);  // ISCLSY (2)
            subheader.extend_from_slice(&[b' '; 11]); // ISCODE (11)
            subheader.extend_from_slice(&[b' '; 2]);  // ISCTLH (2)
            subheader.extend_from_slice(&[b' '; 20]); // ISREL (20)
            subheader.extend_from_slice(&[b' '; 2]);  // ISDCTP (2)
            subheader.extend_from_slice(&[b' '; 8]);  // ISDCDT (8)
            subheader.extend_from_slice(&[b' '; 4]);  // ISDCXM (4)
            subheader.push(b' ');           // ISDG (1)
            subheader.extend_from_slice(&[b' '; 8]);  // ISDGDT (8)
            subheader.extend_from_slice(&[b' '; 43]); // ISCLTX (43)
            subheader.push(b' ');           // ISCATP (1)
            subheader.extend_from_slice(&[b' '; 40]); // ISCAUT (40)
            subheader.push(b' ');           // ISCRSN (1)
            subheader.extend_from_slice(&[b' '; 8]);  // ISSRDT (8)
            subheader.extend_from_slice(&[b' '; 15]); // ISCTLN (15)

            // ENCRYP (1)
            subheader.push(b'0');

            // ISORCE (42)
            subheader.extend_from_slice(&[b' '; 42]);

            // NROWS (8)
            subheader.extend_from_slice(format!("{:08}", nrows).as_bytes());

            // NCOLS (8)
            subheader.extend_from_slice(format!("{:08}", ncols).as_bytes());

            // PVTYPE (3)
            subheader.extend_from_slice(b"INT");

            // IREP (8)
            let irep = if nbands == 1 { b"MONO    " } else { b"MULTI   " };
            subheader.extend_from_slice(irep);

            // ICAT (8)
            subheader.extend_from_slice(b"VIS     ");

            // ABPP (2)
            subheader.extend_from_slice(format!("{:02}", nbpp).as_bytes());

            // PJUST (1)
            subheader.push(b'R');

            // ICORDS (1) - Using blank to skip IGEOLO
            subheader.push(b' ');

            // NICOM (1) - No comments
            subheader.push(b'0');

            // IC (2) - No compression
            subheader.extend_from_slice(b"NC");

            // NBANDS (1)
            subheader.push(b'0' + nbands);

            // Band info for each band
            for _ in 0..nbands {
                subheader.extend_from_slice(b"M "); // IREPBAND (2)
                subheader.extend_from_slice(&[b' '; 6]); // ISUBCAT (6)
                subheader.push(b'N');        // IFC (1)
                subheader.extend_from_slice(&[b' '; 3]); // IMFLT (3)
                subheader.push(b'0');        // NLUTS (1) - No LUTs
            }

            // ISYNC (1)
            subheader.push(b'0');

            // IMODE (1)
            subheader.push(b'B');

            // Calculate blocking parameters
            let nbpr = (ncols + nppbh - 1) / nppbh;
            let nbpc = (nrows + nppbv - 1) / nppbv;

            // NBPR (4)
            subheader.extend_from_slice(format!("{:04}", nbpr).as_bytes());

            // NBPC (4)
            subheader.extend_from_slice(format!("{:04}", nbpc).as_bytes());

            // NPPBH (4)
            subheader.extend_from_slice(format!("{:04}", nppbh).as_bytes());

            // NPPBV (4)
            subheader.extend_from_slice(format!("{:04}", nppbv).as_bytes());

            // NBPP (2)
            subheader.extend_from_slice(format!("{:02}", nbpp).as_bytes());

            // IDLVL (3)
            subheader.extend_from_slice(b"001");

            // IALVL (3)
            subheader.extend_from_slice(b"000");

            // ILOC (10)
            subheader.extend_from_slice(b"0000000000");

            // IMAG (4)
            subheader.extend_from_slice(b"1.0 ");

            // UDIDL (5) - No user defined data
            subheader.extend_from_slice(b"00000");

            // IXSHDL (5) - No extended subheader data
            subheader.extend_from_slice(b"00000");

            // Calculate image data size
            let bytes_per_pixel = ((nbpp as usize) + 7) / 8;
            let block_size = (nppbh as usize) * (nppbv as usize) * (nbands as usize) * bytes_per_pixel;
            let total_blocks = (nbpr as usize) * (nbpc as usize);
            let image_data_size = block_size * total_blocks;

            // Create image data (zeros)
            let image_data = vec![0u8; image_data_size];

            let expected = TestExpectedValues {
                nrows,
                ncols,
                nbands: nbands as u32,
                nbpp: nbpp as u32,
                abpp: nbpp as u32,
                nppbh,
                nppbv,
                nbpr,
                nbpc,
            };

            (subheader, image_data, expected)
        }

        #[derive(Debug)]
        struct TestExpectedValues {
            nrows: u32,
            ncols: u32,
            nbands: u32,
            nbpp: u32,
            abpp: u32,
            nppbh: u32,
            nppbv: u32,
            nbpr: u32,
            nbpc: u32,
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(50))]

            /// ImageAssetProvider returns correct dimension values
            #[test]
            fn trait_returns_correct_dimensions(
                nrows in 1u32..256,
                ncols in 1u32..256,
            ) {
                let (subheader, image_data, expected) = create_test_image_segment(
                    nrows, ncols, 1, 8, 64, 64,
                );

                // Create file data: subheader + image data
                let subheader_len = subheader.len();
                let mut file_data = subheader;
                file_data.extend_from_slice(&image_data);
                let file_data: Arc<[u8]> = Arc::from(file_data);

                let definition = create_test_definition();
                let metadata = Arc::new(JBPSegmentMetadataProvider::from_definition(
                    definition,
                    Arc::from(&file_data[..subheader_len]),
                ));
                let registry = create_test_registry();
                let location = SegmentLocation::new(
                    0,
                    subheader_len as u64,
                    subheader_len as u64,
                    image_data.len() as u64,
                );

                let provider = JBPImageAssetProvider::new(
                    "image_segment_0".to_string(),
                    "Test Image".to_string(),
                    "Test".to_string(),
                    vec!["data".to_string()],
                    location,
                    file_data,
                    metadata,
                    registry,
                    test_format(),
                );

                prop_assert_eq!(provider.num_rows(), expected.nrows,
                    "num_rows() mismatch: expected {}, got {}", expected.nrows, provider.num_rows());
                prop_assert_eq!(provider.num_columns(), expected.ncols,
                    "num_columns() mismatch: expected {}, got {}", expected.ncols, provider.num_columns());
            }

            /// ImageAssetProvider returns correct band count
            #[test]
            fn trait_returns_correct_band_count(nbands in 1u8..9) {
                let (subheader, image_data, expected) = create_test_image_segment(
                    64, 64, nbands, 8, 64, 64,
                );

                let subheader_len = subheader.len();
                let mut file_data = subheader;
                file_data.extend_from_slice(&image_data);
                let file_data: Arc<[u8]> = Arc::from(file_data);

                let definition = create_test_definition();
                let metadata = Arc::new(JBPSegmentMetadataProvider::from_definition(
                    definition,
                    Arc::from(&file_data[..subheader_len]),
                ));
                let registry = create_test_registry();
                let location = SegmentLocation::new(
                    0,
                    subheader_len as u64,
                    subheader_len as u64,
                    image_data.len() as u64,
                );

                let provider = JBPImageAssetProvider::new(
                    "image_segment_0".to_string(),
                    "Test Image".to_string(),
                    "Test".to_string(),
                    vec!["data".to_string()],
                    location,
                    file_data,
                    metadata,
                    registry,
                    test_format(),
                );

                prop_assert_eq!(provider.num_bands(), expected.nbands,
                    "num_bands() mismatch: expected {}, got {}", expected.nbands, provider.num_bands());
            }

            /// ImageAssetProvider returns correct block size
            #[test]
            fn trait_returns_correct_block_size(
                nppbh in prop::sample::select(vec![32u32, 64, 128, 256]),
                nppbv in prop::sample::select(vec![32u32, 64, 128, 256]),
            ) {
                let (subheader, image_data, expected) = create_test_image_segment(
                    256, 256, 1, 8, nppbh, nppbv,
                );

                let subheader_len = subheader.len();
                let mut file_data = subheader;
                file_data.extend_from_slice(&image_data);
                let file_data: Arc<[u8]> = Arc::from(file_data);

                let definition = create_test_definition();
                let metadata = Arc::new(JBPSegmentMetadataProvider::from_definition(
                    definition,
                    Arc::from(&file_data[..subheader_len]),
                ));
                let registry = create_test_registry();
                let location = SegmentLocation::new(
                    0,
                    subheader_len as u64,
                    subheader_len as u64,
                    image_data.len() as u64,
                );

                let provider = JBPImageAssetProvider::new(
                    "image_segment_0".to_string(),
                    "Test Image".to_string(),
                    "Test".to_string(),
                    vec!["data".to_string()],
                    location,
                    file_data,
                    metadata,
                    registry,
                    test_format(),
                );

                prop_assert_eq!(provider.num_pixels_per_block_horizontal(), expected.nppbh,
                    "nppbh mismatch: expected {}, got {}", expected.nppbh, provider.num_pixels_per_block_horizontal());
                prop_assert_eq!(provider.num_pixels_per_block_vertical(), expected.nppbv,
                    "nppbv mismatch: expected {}, got {}", expected.nppbv, provider.num_pixels_per_block_vertical());
            }

            /// ImageAssetProvider returns correct bits per pixel
            #[test]
            fn trait_returns_correct_bits_per_pixel(
                nbpp in prop::sample::select(vec![8u8, 16, 32]),
            ) {
                let (subheader, image_data, expected) = create_test_image_segment(
                    64, 64, 1, nbpp, 64, 64,
                );

                let subheader_len = subheader.len();
                let mut file_data = subheader;
                file_data.extend_from_slice(&image_data);
                let file_data: Arc<[u8]> = Arc::from(file_data);

                let definition = create_test_definition();
                let metadata = Arc::new(JBPSegmentMetadataProvider::from_definition(
                    definition,
                    Arc::from(&file_data[..subheader_len]),
                ));
                let registry = create_test_registry();
                let location = SegmentLocation::new(
                    0,
                    subheader_len as u64,
                    subheader_len as u64,
                    image_data.len() as u64,
                );

                let provider = JBPImageAssetProvider::new(
                    "image_segment_0".to_string(),
                    "Test Image".to_string(),
                    "Test".to_string(),
                    vec!["data".to_string()],
                    location,
                    file_data,
                    metadata,
                    registry,
                    test_format(),
                );

                prop_assert_eq!(provider.num_bits_per_pixel(), expected.nbpp,
                    "nbpp mismatch: expected {}, got {}", expected.nbpp, provider.num_bits_per_pixel());
                prop_assert_eq!(provider.actual_bits_per_pixel(), expected.abpp,
                    "abpp mismatch: expected {}, got {}", expected.abpp, provider.actual_bits_per_pixel());
            }
        }
    }
}
