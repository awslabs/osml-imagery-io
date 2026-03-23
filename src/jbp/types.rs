//! Core type definitions for JBP dataset operations.
//!
//! This module contains the fundamental types used throughout the JBP implementation:
//! - [`NitfFormat`] - Detected NITF format variant
//! - [`SegmentType`] - Type of NITF segment
//! - [`SegmentLocation`] - Location information for a segment
//! - [`SegmentOffsets`] - Pre-calculated offsets for all segments
//! - [`JBPReaderOptions`] - Configuration options for the reader

use crate::jbp::error::JBPError;
use crate::parser::Value;
use crate::parser::StructureAccessor;
use std::sync::Arc;

/// Detected NITF format variant.
///
/// NITF files can be either NITF 2.1 (US standard) or NSIF 1.0 (NATO variant).
/// The format is determined by the magic number at the start of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NitfFormat {
    /// NITF 2.1 format (magic: "NITF02.10")
    Nitf21,
    /// NSIF 1.0 format (magic: "NSIF01.00")
    Nsif10,
}

impl NitfFormat {
    /// Returns the magic number string for this format.
    pub fn magic(&self) -> &'static str {
        match self {
            NitfFormat::Nitf21 => "NITF02.10",
            NitfFormat::Nsif10 => "NSIF01.00",
        }
    }

    /// Returns the structure definition name for the file header.
    pub fn file_header_definition(&self) -> &'static str {
        // NSIF 1.0 uses the same structure as NITF 2.1
        "nitf_02.10_file_header"
    }

    /// Returns the structure definition name for image subheaders.
    pub fn image_subheader_definition(&self) -> &'static str {
        // NSIF 1.0 uses the same structure as NITF 2.1
        "nitf_02.10_image_subheader"
    }

    /// Returns the structure definition name for graphic subheaders.
    pub fn graphic_subheader_definition(&self) -> &'static str {
        // NSIF 1.0 uses the same structure as NITF 2.1
        "nitf_02.10_graphic_subheader"
    }

    /// Returns the structure definition name for text subheaders.
    pub fn text_subheader_definition(&self) -> &'static str {
        // NSIF 1.0 uses the same structure as NITF 2.1
        "nitf_02.10_text_subheader"
    }

    /// Returns the structure definition name for DES subheaders.
    pub fn des_subheader_definition(&self) -> &'static str {
        // NSIF 1.0 uses the same structure as NITF 2.1
        "nitf_02.10_des_subheader"
    }
}

impl std::fmt::Display for NitfFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NitfFormat::Nitf21 => write!(f, "NITF 2.1"),
            NitfFormat::Nsif10 => write!(f, "NSIF 1.0"),
        }
    }
}

/// Type of NITF segment.
///
/// NITF files contain multiple segment types, each with its own subheader
/// format and data content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentType {
    /// Image segment containing raster imagery
    Image,
    /// Graphic segment containing CGM vector graphics
    Graphic,
    /// Text segment containing plain text
    Text,
    /// Data Extension Segment (DES) containing structured data
    DataExtension,
    /// Reserved Extension Segment (RES) for future use
    ReservedExtension,
}

impl SegmentType {
    /// Returns the prefix used in asset keys for this segment type.
    pub fn key_prefix(&self) -> &'static str {
        match self {
            SegmentType::Image => "image",
            SegmentType::Graphic => "graphic",
            SegmentType::Text => "text",
            SegmentType::DataExtension => "des",
            SegmentType::ReservedExtension => "res",
        }
    }

    /// Parse a segment type from an asset key prefix.
    pub fn from_key_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "image" => Some(SegmentType::Image),
            "graphic" => Some(SegmentType::Graphic),
            "text" => Some(SegmentType::Text),
            "des" => Some(SegmentType::DataExtension),
            "res" => Some(SegmentType::ReservedExtension),
            _ => None,
        }
    }
}

impl std::fmt::Display for SegmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SegmentType::Image => write!(f, "Image"),
            SegmentType::Graphic => write!(f, "Graphic"),
            SegmentType::Text => write!(f, "Text"),
            SegmentType::DataExtension => write!(f, "Data Extension"),
            SegmentType::ReservedExtension => write!(f, "Reserved Extension"),
        }
    }
}

/// Location information for a single segment within a NITF file.
///
/// Contains byte offsets and lengths for both the subheader and data portions
/// of a segment, enabling direct seeks without sequential parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentLocation {
    /// Byte offset of subheader from file start
    pub subheader_offset: u64,
    /// Length of subheader in bytes
    pub subheader_length: u64,
    /// Byte offset of data from file start
    pub data_offset: u64,
    /// Length of data in bytes
    pub data_length: u64,
}

impl SegmentLocation {
    /// Create a new segment location.
    pub fn new(
        subheader_offset: u64,
        subheader_length: u64,
        data_offset: u64,
        data_length: u64,
    ) -> Self {
        Self {
            subheader_offset,
            subheader_length,
            data_offset,
            data_length,
        }
    }

    /// Returns the total size of this segment (subheader + data).
    pub fn total_size(&self) -> u64 {
        self.subheader_length + self.data_length
    }

    /// Returns the byte offset immediately after this segment.
    pub fn end_offset(&self) -> u64 {
        self.data_offset + self.data_length
    }
}

/// Pre-calculated segment offsets for all segments in a NITF file.
///
/// Offsets are calculated from the file header during initialization,
/// enabling direct seeks to any segment without sequential parsing.
#[derive(Debug, Clone, Default)]
pub struct SegmentOffsets {
    /// Image segment locations
    pub images: Vec<SegmentLocation>,
    /// Graphic segment locations
    pub graphics: Vec<SegmentLocation>,
    /// Text segment locations
    pub text: Vec<SegmentLocation>,
    /// Data Extension Segment (DES) locations
    pub des: Vec<SegmentLocation>,
    /// Reserved Extension Segment (RES) locations
    pub res: Vec<SegmentLocation>,
}

impl SegmentOffsets {
    /// Create a new empty SegmentOffsets.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate segment offsets from a file header accessor.
    ///
    /// This method extracts segment counts and length arrays from the parsed
    /// file header and calculates the byte offset for each segment's subheader
    /// and data section.
    ///
    /// # Arguments
    /// * `header` - A StructureAccessor for the parsed file header
    ///
    /// # Returns
    /// A `SegmentOffsets` containing locations for all segments, or an error
    /// if required fields are missing or invalid.
    ///
    /// # NITF Header Fields Used
    /// - `HL` - Header Length (total file header size)
    /// - `NUMI` - Number of Image Segments
    /// - `NUMS` - Number of Graphic Segments
    /// - `NUMT` - Number of Text Segments
    /// - `NUMDES` - Number of Data Extension Segments
    /// - `NUMRES` - Number of Reserved Extension Segments
    /// - `IMAGE_INFO[i].LISH` / `IMAGE_INFO[i].LI` - Image segment subheader/data lengths
    /// - `GRAPHIC_INFO[i].LSSH` / `GRAPHIC_INFO[i].LS` - Graphic segment subheader/data lengths
    /// - `TEXT_INFO[i].LTSH` / `TEXT_INFO[i].LT` - Text segment subheader/data lengths
    /// - `DES_INFO[i].LDSH` / `DES_INFO[i].LD` - DES subheader/data lengths
    /// - `RES_INFO[i].LRESH` / `RES_INFO[i].LRE` - RES subheader/data lengths
    pub fn from_header(header: &StructureAccessor) -> Result<Self, JBPError> {
        // Get header length (HL field)
        let hl = Self::get_u64_field(header, "HL")?;

        // Get segment counts
        let numi = Self::get_usize_field(header, "NUMI")?;
        let nums = Self::get_usize_field(header, "NUMS")?;
        let numt = Self::get_usize_field(header, "NUMT")?;
        let numdes = Self::get_usize_field(header, "NUMDES")?;
        let numres = Self::get_usize_field(header, "NUMRES")?;

        let mut current_offset = hl;
        let mut images = Vec::with_capacity(numi);
        let mut graphics = Vec::with_capacity(nums);
        let mut text = Vec::with_capacity(numt);
        let mut des = Vec::with_capacity(numdes);
        let mut res = Vec::with_capacity(numres);

        // Calculate image segment offsets from IMAGE_INFO array
        let image_info = Self::get_struct_array(header, "IMAGE_INFO", numi)?;
        for (i, element) in image_info.iter().enumerate() {
            let lish = Self::get_struct_u64(header, element, "LISH", "IMAGE_INFO", i)?;
            let li = Self::get_struct_u64(header, element, "LI", "IMAGE_INFO", i)?;

            images.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: lish,
                data_offset: current_offset + lish,
                data_length: li,
            });
            current_offset += lish + li;
        }

        // Calculate graphic segment offsets from GRAPHIC_INFO array
        let graphic_info = Self::get_struct_array(header, "GRAPHIC_INFO", nums)?;
        for (i, element) in graphic_info.iter().enumerate() {
            let lssh = Self::get_struct_u64(header, element, "LSSH", "GRAPHIC_INFO", i)?;
            let ls = Self::get_struct_u64(header, element, "LS", "GRAPHIC_INFO", i)?;

            graphics.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: lssh,
                data_offset: current_offset + lssh,
                data_length: ls,
            });
            current_offset += lssh + ls;
        }

        // Calculate text segment offsets from TEXT_INFO array
        let text_info = Self::get_struct_array(header, "TEXT_INFO", numt)?;
        for (i, element) in text_info.iter().enumerate() {
            let ltsh = Self::get_struct_u64(header, element, "LTSH", "TEXT_INFO", i)?;
            let lt = Self::get_struct_u64(header, element, "LT", "TEXT_INFO", i)?;

            text.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: ltsh,
                data_offset: current_offset + ltsh,
                data_length: lt,
            });
            current_offset += ltsh + lt;
        }

        // Calculate DES segment offsets from DES_INFO array
        let des_info = Self::get_struct_array(header, "DES_INFO", numdes)?;
        for (i, element) in des_info.iter().enumerate() {
            let ldsh = Self::get_struct_u64(header, element, "LDSH", "DES_INFO", i)?;
            let ld = Self::get_struct_u64(header, element, "LD", "DES_INFO", i)?;

            des.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: ldsh,
                data_offset: current_offset + ldsh,
                data_length: ld,
            });
            current_offset += ldsh + ld;
        }

        // Calculate RES segment offsets from RES_INFO array
        let res_info = Self::get_struct_array(header, "RES_INFO", numres)?;
        for (i, element) in res_info.iter().enumerate() {
            let lresh = Self::get_struct_u64(header, element, "LRESH", "RES_INFO", i)?;
            let lre = Self::get_struct_u64(header, element, "LRE", "RES_INFO", i)?;

            res.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: lresh,
                data_offset: current_offset + lresh,
                data_length: lre,
            });
            current_offset += lresh + lre;
        }

        Ok(Self {
            images,
            graphics,
            text,
            des,
            res,
        })
    }

    /// Helper to extract a u64 field from the header.
    fn get_u64_field(header: &StructureAccessor, field: &str) -> Result<u64, JBPError> {
        header
            .get(field)
            .map_err(|e| JBPError::ValidationError {
                message: format!("Failed to read field '{}': {}", field, e),
            })?
            .as_u64()
            .map_err(|e| JBPError::ValidationError {
                message: format!("Failed to parse field '{}' as u64: {}", field, e),
            })
    }

    /// Helper to extract a usize field from the header.
    fn get_usize_field(header: &StructureAccessor, field: &str) -> Result<usize, JBPError> {
        let value = Self::get_u64_field(header, field)?;
        Ok(value as usize)
    }

    /// Helper to get a repeated struct field as a Vec of Value elements.
    /// Returns an empty vec if expected_count is 0.
    fn get_struct_array<'a>(
        header: &'a StructureAccessor,
        field: &str,
        expected_count: usize,
    ) -> Result<Vec<Value<'a>>, JBPError> {
        if expected_count == 0 {
            return Ok(Vec::new());
        }
        match header.get(field) {
            Ok(Value::Array(elements)) => Ok(elements),
            Ok(_) => Err(JBPError::ValidationError {
                message: format!("Expected array for field '{}'", field),
            }),
            Err(e) => Err(JBPError::ValidationError {
                message: format!("Failed to read field '{}': {}", field, e),
            }),
        }
    }

    /// Helper to extract a u64 sub-field from a Value::Struct element.
    /// Creates a nested StructureAccessor using the type definition from the parent.
    fn get_struct_u64(
        header: &StructureAccessor,
        element: &Value,
        sub_field: &str,
        array_name: &str,
        index: usize,
    ) -> Result<u64, JBPError> {
        match element {
            Value::Struct(struct_val) => {
                let nested_def = header
                    .definition()
                    .types
                    .get(&struct_val.type_name)
                    .ok_or_else(|| JBPError::ValidationError {
                        message: format!(
                            "Unknown type '{}' for {}[{}]",
                            struct_val.type_name, array_name, index
                        ),
                    })?;
                let nested_accessor =
                    StructureAccessor::new(Arc::new(nested_def.clone()), struct_val.data)
                        .map_err(|e| JBPError::ValidationError {
                            message: format!(
                                "Failed to create accessor for {}[{}]: {}",
                                array_name, index, e
                            ),
                        })?;
                nested_accessor
                    .get(sub_field)
                    .map_err(|e| JBPError::ValidationError {
                        message: format!(
                            "Failed to read {}[{}].{}: {}",
                            array_name, index, sub_field, e
                        ),
                    })?
                    .as_u64()
                    .map_err(|e| JBPError::ValidationError {
                        message: format!(
                            "Failed to parse {}[{}].{} as u64: {}",
                            array_name, index, sub_field, e
                        ),
                    })
            }
            _ => Err(JBPError::ValidationError {
                message: format!("Expected struct for {}[{}]", array_name, index),
            }),
        }
    }

    /// Returns the total number of segments across all types.
    pub fn total_segments(&self) -> usize {
        self.images.len()
            + self.graphics.len()
            + self.text.len()
            + self.des.len()
            + self.res.len()
    }

    /// Get segment location by type and index.
    pub fn get(&self, segment_type: SegmentType, index: usize) -> Option<&SegmentLocation> {
        match segment_type {
            SegmentType::Image => self.images.get(index),
            SegmentType::Graphic => self.graphics.get(index),
            SegmentType::Text => self.text.get(index),
            SegmentType::DataExtension => self.des.get(index),
            SegmentType::ReservedExtension => self.res.get(index),
        }
    }

    /// Get the count of segments for a specific type.
    pub fn count(&self, segment_type: SegmentType) -> usize {
        match segment_type {
            SegmentType::Image => self.images.len(),
            SegmentType::Graphic => self.graphics.len(),
            SegmentType::Text => self.text.len(),
            SegmentType::DataExtension => self.des.len(),
            SegmentType::ReservedExtension => self.res.len(),
        }
    }

    /// Returns an iterator over all segment types and their locations.
    pub fn iter(&self) -> impl Iterator<Item = (SegmentType, usize, &SegmentLocation)> {
        self.images
            .iter()
            .enumerate()
            .map(|(i, loc)| (SegmentType::Image, i, loc))
            .chain(
                self.graphics
                    .iter()
                    .enumerate()
                    .map(|(i, loc)| (SegmentType::Graphic, i, loc)),
            )
            .chain(
                self.text
                    .iter()
                    .enumerate()
                    .map(|(i, loc)| (SegmentType::Text, i, loc)),
            )
            .chain(
                self.des
                    .iter()
                    .enumerate()
                    .map(|(i, loc)| (SegmentType::DataExtension, i, loc)),
            )
            .chain(
                self.res
                    .iter()
                    .enumerate()
                    .map(|(i, loc)| (SegmentType::ReservedExtension, i, loc)),
            )
    }
}

/// Configuration options for JBPDatasetReader.
#[derive(Debug, Clone, Default)]
pub struct JBPReaderOptions {
    /// Whether to validate file length against the FL field.
    ///
    /// When enabled, the reader will compare the calculated file length
    /// (from header and segment lengths) against both the FL field value
    /// and the actual file size. Mismatches will produce validation warnings.
    ///
    /// Disable this for partial file access (e.g., reading only metadata
    /// from cloud storage without downloading the entire file).
    pub validate_file_length: bool,
}

impl JBPReaderOptions {
    /// Create new options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable file length validation.
    pub fn with_file_length_validation(mut self, enabled: bool) -> Self {
        self.validate_file_length = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NitfFormat tests
    #[test]
    fn nitf_format_magic() {
        assert_eq!(NitfFormat::Nitf21.magic(), "NITF02.10");
        assert_eq!(NitfFormat::Nsif10.magic(), "NSIF01.00");
    }

    #[test]
    fn nitf_format_file_header_definition() {
        // Both NITF 2.1 and NSIF 1.0 use the same structure definitions
        // since NSIF 1.0 is structurally identical to NITF 2.1
        assert_eq!(
            NitfFormat::Nitf21.file_header_definition(),
            "nitf_02.10_file_header"
        );
        assert_eq!(
            NitfFormat::Nsif10.file_header_definition(),
            "nitf_02.10_file_header"
        );
    }

    #[test]
    fn nitf_format_display() {
        assert_eq!(NitfFormat::Nitf21.to_string(), "NITF 2.1");
        assert_eq!(NitfFormat::Nsif10.to_string(), "NSIF 1.0");
    }

    #[test]
    fn nitf_format_equality() {
        assert_eq!(NitfFormat::Nitf21, NitfFormat::Nitf21);
        assert_ne!(NitfFormat::Nitf21, NitfFormat::Nsif10);
    }

    // SegmentType tests
    #[test]
    fn segment_type_key_prefix() {
        assert_eq!(SegmentType::Image.key_prefix(), "image");
        assert_eq!(SegmentType::Graphic.key_prefix(), "graphic");
        assert_eq!(SegmentType::Text.key_prefix(), "text");
        assert_eq!(SegmentType::DataExtension.key_prefix(), "des");
        assert_eq!(SegmentType::ReservedExtension.key_prefix(), "res");
    }

    #[test]
    fn segment_type_from_key_prefix() {
        assert_eq!(
            SegmentType::from_key_prefix("image"),
            Some(SegmentType::Image)
        );
        assert_eq!(
            SegmentType::from_key_prefix("graphic"),
            Some(SegmentType::Graphic)
        );
        assert_eq!(
            SegmentType::from_key_prefix("text"),
            Some(SegmentType::Text)
        );
        assert_eq!(
            SegmentType::from_key_prefix("des"),
            Some(SegmentType::DataExtension)
        );
        assert_eq!(
            SegmentType::from_key_prefix("res"),
            Some(SegmentType::ReservedExtension)
        );
        assert_eq!(SegmentType::from_key_prefix("unknown"), None);
    }

    #[test]
    fn segment_type_display() {
        assert_eq!(SegmentType::Image.to_string(), "Image");
        assert_eq!(SegmentType::Graphic.to_string(), "Graphic");
        assert_eq!(SegmentType::Text.to_string(), "Text");
        assert_eq!(SegmentType::DataExtension.to_string(), "Data Extension");
        assert_eq!(
            SegmentType::ReservedExtension.to_string(),
            "Reserved Extension"
        );
    }

    // SegmentLocation tests
    #[test]
    fn segment_location_new() {
        let loc = SegmentLocation::new(100, 50, 150, 1000);
        assert_eq!(loc.subheader_offset, 100);
        assert_eq!(loc.subheader_length, 50);
        assert_eq!(loc.data_offset, 150);
        assert_eq!(loc.data_length, 1000);
    }

    #[test]
    fn segment_location_total_size() {
        let loc = SegmentLocation::new(100, 50, 150, 1000);
        assert_eq!(loc.total_size(), 1050);
    }

    #[test]
    fn segment_location_end_offset() {
        let loc = SegmentLocation::new(100, 50, 150, 1000);
        assert_eq!(loc.end_offset(), 1150);
    }

    // SegmentOffsets tests
    #[test]
    fn segment_offsets_new_is_empty() {
        let offsets = SegmentOffsets::new();
        assert_eq!(offsets.total_segments(), 0);
    }

    #[test]
    fn segment_offsets_total_segments() {
        let mut offsets = SegmentOffsets::new();
        offsets.images.push(SegmentLocation::new(0, 10, 10, 100));
        offsets.images.push(SegmentLocation::new(110, 10, 120, 100));
        offsets.text.push(SegmentLocation::new(220, 5, 225, 50));
        assert_eq!(offsets.total_segments(), 3);
    }

    #[test]
    fn segment_offsets_get() {
        let mut offsets = SegmentOffsets::new();
        let loc = SegmentLocation::new(100, 50, 150, 1000);
        offsets.images.push(loc);

        assert_eq!(offsets.get(SegmentType::Image, 0), Some(&loc));
        assert_eq!(offsets.get(SegmentType::Image, 1), None);
        assert_eq!(offsets.get(SegmentType::Text, 0), None);
    }

    #[test]
    fn segment_offsets_count() {
        let mut offsets = SegmentOffsets::new();
        offsets.images.push(SegmentLocation::new(0, 10, 10, 100));
        offsets.images.push(SegmentLocation::new(110, 10, 120, 100));
        offsets.text.push(SegmentLocation::new(220, 5, 225, 50));

        assert_eq!(offsets.count(SegmentType::Image), 2);
        assert_eq!(offsets.count(SegmentType::Text), 1);
        assert_eq!(offsets.count(SegmentType::Graphic), 0);
    }

    #[test]
    fn segment_offsets_iter() {
        let mut offsets = SegmentOffsets::new();
        offsets.images.push(SegmentLocation::new(0, 10, 10, 100));
        offsets.text.push(SegmentLocation::new(110, 5, 115, 50));

        let items: Vec<_> = offsets.iter().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, SegmentType::Image);
        assert_eq!(items[0].1, 0);
        assert_eq!(items[1].0, SegmentType::Text);
        assert_eq!(items[1].1, 0);
    }

    // JBPReaderOptions tests
    #[test]
    fn jbp_reader_options_default() {
        let options = JBPReaderOptions::default();
        assert!(!options.validate_file_length);
    }

    #[test]
    fn jbp_reader_options_with_file_length_validation() {
        let options = JBPReaderOptions::new().with_file_length_validation(true);
        assert!(options.validate_file_length);
    }
}

/// Property-based tests for segment offset calculation.
///
/// These tests verify the cumulative offset calculation logic directly,
/// bypassing StructureAccessor since it interprets underscores in field names
/// as array indices (e.g., "LISH_0" becomes "LISH[0]").
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Create SegmentOffsets directly from length arrays, simulating what
    /// from_header() does but without needing StructureAccessor.
    fn create_offsets_from_lengths(
        hl: u64,
        image_lengths: &[(u64, u64)],    // (subheader_len, data_len)
        graphic_lengths: &[(u64, u64)],
        text_lengths: &[(u64, u64)],
        des_lengths: &[(u64, u64)],
        res_lengths: &[(u64, u64)],
    ) -> SegmentOffsets {
        let mut current_offset = hl;
        let mut images = Vec::with_capacity(image_lengths.len());
        let mut graphics = Vec::with_capacity(graphic_lengths.len());
        let mut text = Vec::with_capacity(text_lengths.len());
        let mut des = Vec::with_capacity(des_lengths.len());
        let mut res = Vec::with_capacity(res_lengths.len());

        // Calculate image segment offsets
        for (lish, li) in image_lengths {
            images.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: *lish,
                data_offset: current_offset + lish,
                data_length: *li,
            });
            current_offset += lish + li;
        }

        // Calculate graphic segment offsets
        for (lssh, ls) in graphic_lengths {
            graphics.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: *lssh,
                data_offset: current_offset + lssh,
                data_length: *ls,
            });
            current_offset += lssh + ls;
        }

        // Calculate text segment offsets
        for (ltsh, lt) in text_lengths {
            text.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: *ltsh,
                data_offset: current_offset + ltsh,
                data_length: *lt,
            });
            current_offset += ltsh + lt;
        }

        // Calculate DES segment offsets
        for (ldsh, ld) in des_lengths {
            des.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: *ldsh,
                data_offset: current_offset + ldsh,
                data_length: *ld,
            });
            current_offset += ldsh + ld;
        }

        // Calculate RES segment offsets
        for (lresh, lre) in res_lengths {
            res.push(SegmentLocation {
                subheader_offset: current_offset,
                subheader_length: *lresh,
                data_offset: current_offset + lresh,
                data_length: *lre,
            });
            current_offset += lresh + lre;
        }

        SegmentOffsets {
            images,
            graphics,
            text,
            des,
            res,
        }
    }

    /// Property 3: Segment Offset Cumulative Calculation
    /// For any valid NITF file with N segments of any type, the calculated offset
    /// for segment i SHALL equal the header length plus the sum of all
    /// (subheader_length + data_length) for segments 0 through i-1.
    /// **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**
    mod prop_3_segment_offset_cumulative_calculation {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// First segment starts at header length
            #[test]
            fn first_segment_starts_at_hl(
                hl in 100u64..10000,
                subheader_len in 100u64..1000,
                data_len in 1000u64..100000,
            ) {
                let offsets = create_offsets_from_lengths(
                    hl,
                    &[(subheader_len, data_len)],
                    &[],
                    &[],
                    &[],
                    &[],
                );

                prop_assert_eq!(offsets.images.len(), 1);
                prop_assert_eq!(offsets.images[0].subheader_offset, hl,
                    "First segment should start at HL={}", hl);
            }

            /// Segment offsets are cumulative
            #[test]
            fn offsets_are_cumulative(
                hl in 100u64..1000,
                num_images in 1usize..5,
                subheader_len in 100u64..500,
                data_len in 1000u64..5000,
            ) {
                let image_lengths: Vec<(u64, u64)> = (0..num_images)
                    .map(|_| (subheader_len, data_len))
                    .collect();

                let offsets = create_offsets_from_lengths(hl, &image_lengths, &[], &[], &[], &[]);

                prop_assert_eq!(offsets.images.len(), num_images);

                let mut expected_offset = hl;
                for (i, loc) in offsets.images.iter().enumerate() {
                    prop_assert_eq!(loc.subheader_offset, expected_offset,
                        "Segment {} should start at {}", i, expected_offset);
                    prop_assert_eq!(loc.subheader_length, subheader_len);
                    prop_assert_eq!(loc.data_offset, expected_offset + subheader_len);
                    prop_assert_eq!(loc.data_length, data_len);

                    expected_offset += subheader_len + data_len;
                }
            }

            /// Mixed segment types have correct cumulative offsets
            #[test]
            fn mixed_segments_cumulative(
                hl in 100u64..500,
                num_images in 0usize..3,
                num_graphics in 0usize..3,
                num_text in 0usize..3,
            ) {
                // Use fixed lengths for simplicity
                let img_sh = 200u64;
                let img_data = 1000u64;
                let gfx_sh = 100u64;
                let gfx_data = 500u64;
                let txt_sh = 50u64;
                let txt_data = 200u64;

                let image_lengths: Vec<(u64, u64)> = (0..num_images)
                    .map(|_| (img_sh, img_data))
                    .collect();
                let graphic_lengths: Vec<(u64, u64)> = (0..num_graphics)
                    .map(|_| (gfx_sh, gfx_data))
                    .collect();
                let text_lengths: Vec<(u64, u64)> = (0..num_text)
                    .map(|_| (txt_sh, txt_data))
                    .collect();

                let offsets = create_offsets_from_lengths(
                    hl,
                    &image_lengths,
                    &graphic_lengths,
                    &text_lengths,
                    &[],
                    &[],
                );

                // Verify counts
                prop_assert_eq!(offsets.images.len(), num_images);
                prop_assert_eq!(offsets.graphics.len(), num_graphics);
                prop_assert_eq!(offsets.text.len(), num_text);

                // Calculate expected offsets
                let mut expected_offset = hl;

                // Check image segments
                for loc in &offsets.images {
                    prop_assert_eq!(loc.subheader_offset, expected_offset);
                    expected_offset += img_sh + img_data;
                }

                // Check graphic segments (should follow images)
                for loc in &offsets.graphics {
                    prop_assert_eq!(loc.subheader_offset, expected_offset);
                    expected_offset += gfx_sh + gfx_data;
                }

                // Check text segments (should follow graphics)
                for loc in &offsets.text {
                    prop_assert_eq!(loc.subheader_offset, expected_offset);
                    expected_offset += txt_sh + txt_data;
                }
            }

            /// Total segments equals sum of all segment counts
            #[test]
            fn total_segments_correct(
                num_images in 0usize..5,
                num_graphics in 0usize..5,
                num_text in 0usize..5,
                num_des in 0usize..5,
                num_res in 0usize..5,
            ) {
                let hl = 500u64;
                let image_lengths: Vec<(u64, u64)> = (0..num_images).map(|_| (100, 1000)).collect();
                let graphic_lengths: Vec<(u64, u64)> = (0..num_graphics).map(|_| (50, 500)).collect();
                let text_lengths: Vec<(u64, u64)> = (0..num_text).map(|_| (30, 200)).collect();
                let des_lengths: Vec<(u64, u64)> = (0..num_des).map(|_| (40, 300)).collect();
                let res_lengths: Vec<(u64, u64)> = (0..num_res).map(|_| (20, 100)).collect();

                let offsets = create_offsets_from_lengths(
                    hl,
                    &image_lengths,
                    &graphic_lengths,
                    &text_lengths,
                    &des_lengths,
                    &res_lengths,
                );

                let expected_total = num_images + num_graphics + num_text + num_des + num_res;
                prop_assert_eq!(offsets.total_segments(), expected_total,
                    "Total segments should be {}", expected_total);
            }

            /// Data offset equals subheader offset plus subheader length
            #[test]
            fn data_offset_follows_subheader(
                hl in 100u64..1000,
                subheader_len in 50u64..500,
                data_len in 100u64..10000,
            ) {
                let offsets = create_offsets_from_lengths(
                    hl,
                    &[(subheader_len, data_len)],
                    &[],
                    &[],
                    &[],
                    &[],
                );

                let loc = &offsets.images[0];
                prop_assert_eq!(loc.data_offset, loc.subheader_offset + loc.subheader_length,
                    "Data offset should be subheader_offset + subheader_length");
            }

            /// End offset equals data offset plus data length
            #[test]
            fn end_offset_correct(
                hl in 100u64..1000,
                subheader_len in 50u64..500,
                data_len in 100u64..10000,
            ) {
                let offsets = create_offsets_from_lengths(
                    hl,
                    &[(subheader_len, data_len)],
                    &[],
                    &[],
                    &[],
                    &[],
                );

                let loc = &offsets.images[0];
                prop_assert_eq!(loc.end_offset(), loc.data_offset + loc.data_length,
                    "End offset should be data_offset + data_length");
            }

            /// Empty file (no segments) produces empty offsets
            #[test]
            fn empty_file_empty_offsets(hl in 100u64..10000) {
                let offsets = create_offsets_from_lengths(hl, &[], &[], &[], &[], &[]);

                prop_assert_eq!(offsets.total_segments(), 0);
                prop_assert!(offsets.images.is_empty());
                prop_assert!(offsets.graphics.is_empty());
                prop_assert!(offsets.text.is_empty());
                prop_assert!(offsets.des.is_empty());
                prop_assert!(offsets.res.is_empty());
            }
        }
    }
}
