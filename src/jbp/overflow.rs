//! TRE overflow resolution for NITF segments.
//!
//! This module provides helper functions for resolving TRE overflow references.
//! When TREs exceed the available space in a segment header, they are stored in
//! TRE_OVERFLOW Data Extension Segments (DES). The overflow index fields in
//! segment subheaders (UDOFL, IXSOFL, etc.) point to these DES segments.
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::overflow;
//! use osml_imagery_io::parser::StructureAccessor;
//!
//! // Get overflow indices from an image subheader
//! let (udofl, ixsofl) = overflow::get_image_overflow_indices(&accessor)?;
//!
//! // Fetch overflow TREs if present
//! if udofl > 0 {
//!     let tres = overflow::fetch_overflow_tres(udofl, &des_locations, file_data)?;
//!     // Process overflow TREs...
//! }
//! ```

use super::error::JBPError;
use super::tre::TreEnvelope;
use super::types::SegmentLocation;
use crate::parser::StructureAccessor;

/// Source header type for TRE overflow.
///
/// When TREs exceed the available space in a header field, they are stored in
/// TRE_OVERFLOW DES segments. This enum identifies which header field overflowed,
/// and is used to set the DESOFLW field in the TRE_OVERFLOW DES subheader.
///
/// # DESOFLW Values
///
/// According to NITF 2.1 specification:
/// - "UDHD  " - File header user-defined header data overflow
/// - "UDHDX " - File header extended header data overflow (XHD)
/// - "UDID  " - Image subheader user-defined image data overflow
/// - "IXSHD " - Image extended subheader data overflow
/// - "SXSHD " - Graphic extended subheader data overflow
/// - "TXSHD " - Text extended subheader data overflow
///
/// # Requirements
///
/// _Requirements: 12.3, 12.4_
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowSource {
    /// File header user-defined header data (UDHD) overflow
    FileHeaderUdhd,
    /// File header extended header data (XHD) overflow
    FileHeaderXhd,
    /// Image subheader user-defined image data (UDID) overflow
    ImageUdid,
    /// Image extended subheader data (IXSHD) overflow
    ImageIxshd,
    /// Graphic extended subheader data (SXSHD) overflow
    GraphicSxshd,
    /// Text extended subheader data (TXSHD) overflow
    TextTxshd,
}

impl OverflowSource {
    /// Convert to the 6-character DESOFLW field value.
    ///
    /// Returns the DESOFLW value as specified in the NITF 2.1 standard.
    /// The value is left-justified and space-padded to 6 characters.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use osml_imagery_io::jbp::overflow::OverflowSource;
    ///
    /// assert_eq!(OverflowSource::FileHeaderUdhd.to_desoflw(), "UDHD  ");
    /// assert_eq!(OverflowSource::ImageIxshd.to_desoflw(), "IXSHD ");
    /// ```
    pub fn to_desoflw(&self) -> &'static str {
        match self {
            OverflowSource::FileHeaderUdhd => "UDHD  ",
            OverflowSource::FileHeaderXhd => "UDHDX ",
            OverflowSource::ImageUdid => "UDID  ",
            OverflowSource::ImageIxshd => "IXSHD ",
            OverflowSource::GraphicSxshd => "SXSHD ",
            OverflowSource::TextTxshd => "TXSHD ",
        }
    }

    /// Parse a DESOFLW field value into an OverflowSource.
    ///
    /// # Arguments
    ///
    /// * `desoflw` - The 6-character DESOFLW field value
    ///
    /// # Returns
    ///
    /// The corresponding OverflowSource variant, or an error if the value is invalid.
    pub fn from_desoflw(desoflw: &str) -> Result<Self, JBPError> {
        match desoflw.trim() {
            "UDHD" => Ok(OverflowSource::FileHeaderUdhd),
            "UDHDX" => Ok(OverflowSource::FileHeaderXhd),
            "XHD" => Ok(OverflowSource::FileHeaderXhd), // Alternative name
            "UDID" => Ok(OverflowSource::ImageUdid),
            "IXSHD" => Ok(OverflowSource::ImageIxshd),
            "SXSHD" => Ok(OverflowSource::GraphicSxshd),
            "TXSHD" => Ok(OverflowSource::TextTxshd),
            _ => Err(JBPError::InvalidFormat {
                message: format!("Invalid DESOFLW value: '{}'", desoflw),
            }),
        }
    }
}

/// Default security classification fields for DES subheader.
///
/// These are the default values used when creating a TRE_OVERFLOW DES
/// without explicit security field values.
struct DefaultSecurityFields;

impl DefaultSecurityFields {
    const DESCLAS: &'static str = "U"; // Unclassified
    const DESCLSY: &'static str = "  ";
    const DESCODE: &'static str = "           ";
    const DESCTLH: &'static str = "  ";
    const DESREL: &'static str = "                    ";
    const DESDCTP: &'static str = "  ";
    const DESDCDT: &'static str = "        ";
    const DESDCXM: &'static str = "    ";
    const DESDG: &'static str = " ";
    const DESDGDT: &'static str = "        ";
    const DESCLTX: &'static str = "                                           ";
    const DESCATP: &'static str = " ";
    const DESCAUT: &'static str = "                                        ";
    const DESCRSN: &'static str = " ";
    const DESSRDT: &'static str = "        ";
    const DESCTLN: &'static str = "               ";
}

/// Create a TRE_OVERFLOW DES for excess TREs.
///
/// Creates a complete TRE_OVERFLOW DES segment including the subheader and data.
/// The subheader contains the DESOFLW field indicating which header overflowed,
/// and the DESITEM field indicating which segment index overflowed.
///
/// # Arguments
///
/// * `source` - The overflow source indicating which header field overflowed
/// * `segment_index` - The 0-based segment index that overflowed (0 for file header)
/// * `tres` - The TRE envelopes to store in the overflow DES
/// * `security_fields` - Optional security field values (uses defaults if None)
///
/// # Returns
///
/// A tuple `(subheader_bytes, data_bytes)` where:
/// - `subheader_bytes` contains the complete DES subheader
/// - `data_bytes` contains the serialized TRE envelopes
///
/// # Example
///
/// ```ignore
/// use osml_imagery_io::jbp::overflow::{create_overflow_des, OverflowSource};
/// use osml_imagery_io::jbp::TreEnvelope;
///
/// let tres = vec![TreEnvelope::new("GEOLOB", vec![1, 2, 3]).unwrap()];
/// let (subheader, data) = create_overflow_des(
///     OverflowSource::ImageUdid,
///     0,  // First image segment
///     &tres,
///     None,
/// )?;
/// ```
///
/// # Requirements
///
/// _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_
pub fn create_overflow_des(
    source: OverflowSource,
    segment_index: u16,
    tres: &[TreEnvelope],
    security_fields: Option<&std::collections::HashMap<String, serde_json::Value>>,
) -> Result<(Vec<u8>, Vec<u8>), JBPError> {
    use super::tre::write_tre_envelopes;

    // Build the DES subheader
    let mut subheader = Vec::with_capacity(200);

    // DE - File part type (2 bytes)
    subheader.extend_from_slice(b"DE");

    // DESID - Unique DES type identifier (25 bytes, left-justified, space-padded)
    subheader.extend_from_slice(b"TRE_OVERFLOW             ");

    // DESVER - Version (2 bytes)
    subheader.extend_from_slice(b"01");

    // Security fields - use provided values or defaults
    let get_security_field = |name: &str, default: &str| -> String {
        if let Some(fields) = security_fields {
            if let Some(value) = fields.get(name) {
                if let Some(s) = value.as_str() {
                    return s.to_string();
                }
            }
        }
        default.to_string()
    };

    // DESCLAS - Security classification (1 byte)
    let desclas = get_security_field("DESCLAS", DefaultSecurityFields::DESCLAS);
    subheader.extend_from_slice(desclas.as_bytes());

    // DESCLSY - Classification system (2 bytes)
    let desclsy = get_security_field("DESCLSY", DefaultSecurityFields::DESCLSY);
    subheader.extend_from_slice(format!("{:<2}", desclsy).as_bytes());

    // DESCODE - Codewords (11 bytes)
    let descode = get_security_field("DESCODE", DefaultSecurityFields::DESCODE);
    subheader.extend_from_slice(format!("{:<11}", descode).as_bytes());

    // DESCTLH - Control and handling (2 bytes)
    let desctlh = get_security_field("DESCTLH", DefaultSecurityFields::DESCTLH);
    subheader.extend_from_slice(format!("{:<2}", desctlh).as_bytes());

    // DESREL - Releasing instructions (20 bytes)
    let desrel = get_security_field("DESREL", DefaultSecurityFields::DESREL);
    subheader.extend_from_slice(format!("{:<20}", desrel).as_bytes());

    // DESDCTP - Declassification type (2 bytes)
    let desdctp = get_security_field("DESDCTP", DefaultSecurityFields::DESDCTP);
    subheader.extend_from_slice(format!("{:<2}", desdctp).as_bytes());

    // DESDCDT - Declassification date (8 bytes)
    let desdcdt = get_security_field("DESDCDT", DefaultSecurityFields::DESDCDT);
    subheader.extend_from_slice(format!("{:<8}", desdcdt).as_bytes());

    // DESDCXM - Declassification exemption (4 bytes)
    let desdcxm = get_security_field("DESDCXM", DefaultSecurityFields::DESDCXM);
    subheader.extend_from_slice(format!("{:<4}", desdcxm).as_bytes());

    // DESDG - Downgrade (1 byte)
    let desdg = get_security_field("DESDG", DefaultSecurityFields::DESDG);
    subheader.extend_from_slice(desdg.as_bytes());

    // DESDGDT - Downgrade date (8 bytes)
    let desdgdt = get_security_field("DESDGDT", DefaultSecurityFields::DESDGDT);
    subheader.extend_from_slice(format!("{:<8}", desdgdt).as_bytes());

    // DESCLTX - Classification text (43 bytes)
    let descltx = get_security_field("DESCLTX", DefaultSecurityFields::DESCLTX);
    subheader.extend_from_slice(format!("{:<43}", descltx).as_bytes());

    // DESCATP - Classification authority type (1 byte)
    let descatp = get_security_field("DESCATP", DefaultSecurityFields::DESCATP);
    subheader.extend_from_slice(descatp.as_bytes());

    // DESCAUT - Classification authority (40 bytes)
    let descaut = get_security_field("DESCAUT", DefaultSecurityFields::DESCAUT);
    subheader.extend_from_slice(format!("{:<40}", descaut).as_bytes());

    // DESCRSN - Classification reason (1 byte)
    let descrsn = get_security_field("DESCRSN", DefaultSecurityFields::DESCRSN);
    subheader.extend_from_slice(descrsn.as_bytes());

    // DESSRDT - Security source date (8 bytes)
    let dessrdt = get_security_field("DESSRDT", DefaultSecurityFields::DESSRDT);
    subheader.extend_from_slice(format!("{:<8}", dessrdt).as_bytes());

    // DESCTLN - Security control number (15 bytes)
    let desctln = get_security_field("DESCTLN", DefaultSecurityFields::DESCTLN);
    subheader.extend_from_slice(format!("{:<15}", desctln).as_bytes());

    // DESOFLW - Overflowed header type (6 bytes) - only for TRE_OVERFLOW
    subheader.extend_from_slice(source.to_desoflw().as_bytes());

    // DESITEM - Data item overflowed (3 bytes, 1-based segment index)
    // For file header overflow, use 000
    let desitem = if matches!(
        source,
        OverflowSource::FileHeaderUdhd | OverflowSource::FileHeaderXhd
    ) {
        0
    } else {
        segment_index + 1 // Convert 0-based to 1-based
    };
    subheader.extend_from_slice(format!("{:03}", desitem).as_bytes());

    // DESSHL - DES-defined subheader fields length (4 bytes)
    // TRE_OVERFLOW has no DES-defined subheader fields
    subheader.extend_from_slice(b"0000");

    // Serialize the TRE envelopes as the DES data
    let data = write_tre_envelopes(tres);

    Ok((subheader, data))
}

/// Get overflow DES indices from an image subheader.
///
/// Extracts the UDOFL (User Defined Overflow) and IXSOFL (Image Extended Subheader
/// Overflow) fields from an image subheader accessor.
///
/// # Arguments
///
/// * `accessor` - A StructureAccessor for the parsed image subheader
///
/// # Returns
///
/// A tuple `(udofl, ixsofl)` where:
/// - `udofl` is the 1-based DES index for UDID overflow (0 if no overflow)
/// - `ixsofl` is the 1-based DES index for IXSHD overflow (0 if no overflow)
///
/// # Errors
///
/// Returns an error if the overflow fields cannot be read from the subheader.
///
/// # Requirements
///
/// _Requirements: 6.3, 6.4_
pub fn get_image_overflow_indices(accessor: &StructureAccessor) -> Result<(u16, u16), JBPError> {
    // UDOFL field - User Defined Overflow (3 digits)
    let udofl = get_overflow_field(accessor, "UDOFL")?;

    // IXSOFL field - Image Extended Subheader Overflow (3 digits)
    let ixsofl = get_overflow_field(accessor, "IXSOFL")?;

    Ok((udofl, ixsofl))
}

/// Get overflow DES index from a graphic subheader.
///
/// Extracts the SXSOFL (Graphic Extended Subheader Overflow) field from a
/// graphic subheader accessor.
///
/// # Arguments
///
/// * `accessor` - A StructureAccessor for the parsed graphic subheader
///
/// # Returns
///
/// The 1-based DES index for SXSHD overflow (0 if no overflow).
///
/// # Errors
///
/// Returns an error if the overflow field cannot be read from the subheader.
///
/// # Requirements
///
/// _Requirements: 6.5_
pub fn get_graphic_overflow_index(accessor: &StructureAccessor) -> Result<u16, JBPError> {
    get_overflow_field(accessor, "SXSOFL")
}

/// Get overflow DES index from a text subheader.
///
/// Extracts the TXSOFL (Text Extended Subheader Overflow) field from a
/// text subheader accessor.
///
/// # Arguments
///
/// * `accessor` - A StructureAccessor for the parsed text subheader
///
/// # Returns
///
/// The 1-based DES index for TXSHD overflow (0 if no overflow).
///
/// # Errors
///
/// Returns an error if the overflow field cannot be read from the subheader.
///
/// # Requirements
///
/// _Requirements: 6.6_
pub fn get_text_overflow_index(accessor: &StructureAccessor) -> Result<u16, JBPError> {
    get_overflow_field(accessor, "TXSOFL")
}

/// Get overflow DES indices from a file header.
///
/// Extracts the UDHOFL (User Defined Header Overflow) and XHDLOFL (Extended
/// Header Data Overflow) fields from a file header accessor.
///
/// # Arguments
///
/// * `accessor` - A StructureAccessor for the parsed file header
///
/// # Returns
///
/// A tuple `(udhofl, xhdlofl)` where:
/// - `udhofl` is the 1-based DES index for UDHD overflow (0 if no overflow)
/// - `xhdlofl` is the 1-based DES index for XHD overflow (0 if no overflow)
///
/// # Errors
///
/// Returns an error if the overflow fields cannot be read from the header.
///
/// # Requirements
///
/// _Requirements: 6.3_
pub fn get_file_header_overflow_indices(
    accessor: &StructureAccessor,
) -> Result<(u16, u16), JBPError> {
    // UDHOFL field - User Defined Header Overflow (3 digits)
    let udhofl = get_overflow_field(accessor, "UDHOFL")?;

    // XHDLOFL field - Extended Header Data Overflow (3 digits)
    let xhdlofl = get_overflow_field(accessor, "XHDLOFL")?;

    Ok((udhofl, xhdlofl))
}

/// Fetch TRE envelopes from a DES segment by 1-based index.
///
/// This function retrieves TRE envelopes from a TRE_OVERFLOW DES segment.
/// The DES data section is parsed as a sequence of TRE envelopes.
///
/// # Arguments
///
/// * `des_index` - 1-based DES segment index (from overflow field)
/// * `des_locations` - Slice of DES segment locations
/// * `file_data` - Complete file data buffer
///
/// # Returns
///
/// A vector of TRE envelopes parsed from the DES data section.
/// Returns an empty vector if `des_index` is 0 (no overflow).
///
/// # Errors
///
/// - `JBPError::InvalidOverflowIndex` if the index exceeds the DES count
/// - `JBPError::UnexpectedEof` if there isn't enough data
/// - TRE parsing errors if the DES data is malformed
///
/// # Requirements
///
/// _Requirements: 6.1, 6.2_
pub fn fetch_overflow_tres(
    des_index: u16,
    des_locations: &[SegmentLocation],
    file_data: &[u8],
) -> Result<Vec<TreEnvelope>, JBPError> {
    // Return empty vec if index is 0 (no overflow)
    if des_index == 0 {
        return Ok(Vec::new());
    }

    // Convert 1-based index to 0-based
    let zero_based_index = (des_index - 1) as usize;

    // Validate index is within bounds
    if zero_based_index >= des_locations.len() {
        return Err(JBPError::InvalidOverflowIndex {
            index: des_index,
            des_count: des_locations.len(),
        });
    }

    // Get the DES location
    let des_loc = &des_locations[zero_based_index];

    // Extract the DES data section
    let data_start = des_loc.data_offset as usize;
    let data_end = data_start + des_loc.data_length as usize;

    // Validate we have enough data
    if data_end > file_data.len() {
        return Err(JBPError::UnexpectedEof {
            expected: data_end,
            available: file_data.len(),
        });
    }

    let des_data = &file_data[data_start..data_end];

    // Parse TRE envelopes from the DES data
    TreEnvelope::parse_all(des_data)
}

/// Helper function to extract an overflow field value from an accessor.
///
/// Overflow fields are 3-digit numeric strings representing 1-based DES indices.
/// A value of "000" or 0 indicates no overflow.
fn get_overflow_field(accessor: &StructureAccessor, field_name: &str) -> Result<u16, JBPError> {
    // Try to get the field - if it doesn't exist, return 0 (no overflow)
    let value = match accessor.get(field_name) {
        Ok(v) => v,
        Err(_) => return Ok(0), // Field not present means no overflow
    };

    // Parse the string value as a number
    let str_value = value.as_str().map_err(|e| JBPError::ValidationError {
        message: format!("Failed to read overflow field '{}': {}", field_name, e),
    })?;

    // Parse as u16, treating empty or whitespace-only as 0
    let trimmed = str_value.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }

    trimmed
        .parse::<u16>()
        .map_err(|_| JBPError::ValidationError {
            message: format!(
                "Invalid overflow field '{}': '{}' is not a valid number",
                field_name, str_value
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn fetch_overflow_tres_returns_empty_for_zero_index() {
        let des_locations = vec![SegmentLocation::new(0, 100, 100, 50)];
        let file_data = vec![0u8; 200];

        let result = fetch_overflow_tres(0, &des_locations, &file_data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn fetch_overflow_tres_returns_error_for_invalid_index() {
        let des_locations = vec![SegmentLocation::new(0, 100, 100, 50)];
        let file_data = vec![0u8; 200];

        // Index 2 is out of bounds (only 1 DES segment)
        let result = fetch_overflow_tres(2, &des_locations, &file_data);
        assert!(result.is_err());

        match result {
            Err(JBPError::InvalidOverflowIndex { index, des_count }) => {
                assert_eq!(index, 2);
                assert_eq!(des_count, 1);
            }
            _ => panic!("Expected InvalidOverflowIndex error"),
        }
    }

    #[test]
    fn fetch_overflow_tres_returns_error_for_empty_des_list() {
        let des_locations: Vec<SegmentLocation> = vec![];
        let file_data = vec![0u8; 200];

        // Any non-zero index is invalid with empty DES list
        let result = fetch_overflow_tres(1, &des_locations, &file_data);
        assert!(result.is_err());

        match result {
            Err(JBPError::InvalidOverflowIndex { index, des_count }) => {
                assert_eq!(index, 1);
                assert_eq!(des_count, 0);
            }
            _ => panic!("Expected InvalidOverflowIndex error"),
        }
    }

    #[test]
    fn fetch_overflow_tres_parses_single_tre() {
        // Create a DES with a single TRE envelope
        // TRE: CETAG="GEOLOB", CEL="00003", CEDATA=[0x01, 0x02, 0x03]
        let tre_data = b"GEOLOB00003\x01\x02\x03";

        // DES location: subheader at 0-99, data at 100-113
        let des_locations = vec![SegmentLocation::new(0, 100, 100, tre_data.len() as u64)];

        // Build file data with TRE at offset 100
        let mut file_data = vec![0u8; 100];
        file_data.extend_from_slice(tre_data);

        let result = fetch_overflow_tres(1, &des_locations, &file_data).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, "GEOLOB");
        assert_eq!(result[0].data, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn fetch_overflow_tres_parses_multiple_tres() {
        // Create DES data with two TRE envelopes
        let mut tre_data = Vec::new();
        tre_data.extend_from_slice(b"GEOLOB00003\x01\x02\x03");
        tre_data.extend_from_slice(b"TEST  00002\xAA\xBB");

        let des_locations = vec![SegmentLocation::new(0, 50, 50, tre_data.len() as u64)];

        let mut file_data = vec![0u8; 50];
        file_data.extend_from_slice(&tre_data);

        let result = fetch_overflow_tres(1, &des_locations, &file_data).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tag, "GEOLOB");
        assert_eq!(result[0].data, vec![0x01, 0x02, 0x03]);
        assert_eq!(result[1].tag, "TEST  ");
        assert_eq!(result[1].data, vec![0xAA, 0xBB]);
    }

    #[test]
    fn fetch_overflow_tres_handles_second_des() {
        // Create two DES segments, fetch from the second one
        let tre_data_1 = b"FIRST 00001\x01";
        let tre_data_2 = b"SECOND00002\x02\x03";

        let des_locations = vec![
            SegmentLocation::new(0, 50, 50, tre_data_1.len() as u64),
            SegmentLocation::new(
                50 + tre_data_1.len() as u64,
                50,
                100 + tre_data_1.len() as u64,
                tre_data_2.len() as u64,
            ),
        ];

        let mut file_data = vec![0u8; 50];
        file_data.extend_from_slice(tre_data_1);
        file_data.extend(vec![0u8; 50]); // Second subheader
        file_data.extend_from_slice(tre_data_2);

        // Fetch from DES index 2 (1-based)
        let result = fetch_overflow_tres(2, &des_locations, &file_data).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, "SECOND");
        assert_eq!(result[0].data, vec![0x02, 0x03]);
    }

    #[test]
    fn fetch_overflow_tres_returns_error_for_insufficient_data() {
        // DES location claims more data than available
        let des_locations = vec![SegmentLocation::new(0, 50, 50, 1000)];
        let file_data = vec![0u8; 100]; // Only 100 bytes, but DES claims 1000

        let result = fetch_overflow_tres(1, &des_locations, &file_data);
        assert!(result.is_err());

        match result {
            Err(JBPError::UnexpectedEof {
                expected,
                available,
            }) => {
                assert_eq!(expected, 1050); // 50 + 1000
                assert_eq!(available, 100);
            }
            _ => panic!("Expected UnexpectedEof error"),
        }
    }

    #[test]
    fn fetch_overflow_tres_handles_empty_des_data() {
        // DES with zero-length data section
        let des_locations = vec![SegmentLocation::new(0, 50, 50, 0)];
        let file_data = vec![0u8; 50];

        let result = fetch_overflow_tres(1, &des_locations, &file_data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn overflow_source_to_desoflw() {
        assert_eq!(OverflowSource::FileHeaderUdhd.to_desoflw(), "UDHD  ");
        assert_eq!(OverflowSource::FileHeaderXhd.to_desoflw(), "UDHDX ");
        assert_eq!(OverflowSource::ImageUdid.to_desoflw(), "UDID  ");
        assert_eq!(OverflowSource::ImageIxshd.to_desoflw(), "IXSHD ");
        assert_eq!(OverflowSource::GraphicSxshd.to_desoflw(), "SXSHD ");
        assert_eq!(OverflowSource::TextTxshd.to_desoflw(), "TXSHD ");
    }

    #[test]
    fn overflow_source_to_desoflw_length() {
        // All DESOFLW values must be exactly 6 characters
        assert_eq!(OverflowSource::FileHeaderUdhd.to_desoflw().len(), 6);
        assert_eq!(OverflowSource::FileHeaderXhd.to_desoflw().len(), 6);
        assert_eq!(OverflowSource::ImageUdid.to_desoflw().len(), 6);
        assert_eq!(OverflowSource::ImageIxshd.to_desoflw().len(), 6);
        assert_eq!(OverflowSource::GraphicSxshd.to_desoflw().len(), 6);
        assert_eq!(OverflowSource::TextTxshd.to_desoflw().len(), 6);
    }

    #[test]
    fn overflow_source_from_desoflw_valid() {
        assert_eq!(
            OverflowSource::from_desoflw("UDHD  ").unwrap(),
            OverflowSource::FileHeaderUdhd
        );
        assert_eq!(
            OverflowSource::from_desoflw("UDHDX ").unwrap(),
            OverflowSource::FileHeaderXhd
        );
        assert_eq!(
            OverflowSource::from_desoflw("XHD").unwrap(),
            OverflowSource::FileHeaderXhd
        );
        assert_eq!(
            OverflowSource::from_desoflw("UDID  ").unwrap(),
            OverflowSource::ImageUdid
        );
        assert_eq!(
            OverflowSource::from_desoflw("IXSHD ").unwrap(),
            OverflowSource::ImageIxshd
        );
        assert_eq!(
            OverflowSource::from_desoflw("SXSHD ").unwrap(),
            OverflowSource::GraphicSxshd
        );
        assert_eq!(
            OverflowSource::from_desoflw("TXSHD ").unwrap(),
            OverflowSource::TextTxshd
        );
    }

    #[test]
    fn overflow_source_from_desoflw_trimmed() {
        // Should handle trimmed values
        assert_eq!(
            OverflowSource::from_desoflw("UDHD").unwrap(),
            OverflowSource::FileHeaderUdhd
        );
        assert_eq!(
            OverflowSource::from_desoflw("IXSHD").unwrap(),
            OverflowSource::ImageIxshd
        );
    }

    #[test]
    fn overflow_source_from_desoflw_invalid() {
        assert!(OverflowSource::from_desoflw("INVALID").is_err());
        assert!(OverflowSource::from_desoflw("").is_err());
        assert!(OverflowSource::from_desoflw("      ").is_err());
    }

    #[test]
    fn overflow_source_round_trip() {
        // All variants should round-trip through to_desoflw and from_desoflw
        let variants = [
            OverflowSource::FileHeaderUdhd,
            OverflowSource::FileHeaderXhd,
            OverflowSource::ImageUdid,
            OverflowSource::ImageIxshd,
            OverflowSource::GraphicSxshd,
            OverflowSource::TextTxshd,
        ];

        for variant in variants {
            let desoflw = variant.to_desoflw();
            let parsed = OverflowSource::from_desoflw(desoflw).unwrap();
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn create_overflow_des_basic() {
        let tres = vec![TreEnvelope::new("GEOLOB", vec![0x01, 0x02, 0x03]).unwrap()];
        let (subheader, data) = create_overflow_des(
            OverflowSource::ImageUdid,
            0, // First image segment
            &tres,
            None,
        )
        .unwrap();

        // Verify subheader starts with "DE"
        assert_eq!(&subheader[0..2], b"DE");

        // Verify DESID is "TRE_OVERFLOW" (25 bytes, space-padded)
        assert_eq!(&subheader[2..27], b"TRE_OVERFLOW             ");

        // Verify DESVER is "01"
        assert_eq!(&subheader[27..29], b"01");

        // Verify data contains the TRE envelope
        assert_eq!(data, b"GEOLOB00003\x01\x02\x03");
    }

    #[test]
    fn create_overflow_des_desoflw_field() {
        let tres = vec![TreEnvelope::new("TEST", vec![]).unwrap()];

        // Test each overflow source
        let test_cases = [
            (OverflowSource::FileHeaderUdhd, "UDHD  "),
            (OverflowSource::FileHeaderXhd, "UDHDX "),
            (OverflowSource::ImageUdid, "UDID  "),
            (OverflowSource::ImageIxshd, "IXSHD "),
            (OverflowSource::GraphicSxshd, "SXSHD "),
            (OverflowSource::TextTxshd, "TXSHD "),
        ];

        for (source, expected_desoflw) in test_cases {
            let (subheader, _) = create_overflow_des(source, 0, &tres, None).unwrap();

            // DESOFLW is at offset 196 (after all security fields)
            // DE(2) + DESID(25) + DESVER(2) + security fields(167) = 196
            let desoflw_offset = 196;
            let desoflw =
                std::str::from_utf8(&subheader[desoflw_offset..desoflw_offset + 6]).unwrap();
            assert_eq!(
                desoflw, expected_desoflw,
                "DESOFLW mismatch for {:?}",
                source
            );
        }
    }

    #[test]
    fn create_overflow_des_desitem_field() {
        let tres = vec![TreEnvelope::new("TEST", vec![]).unwrap()];

        // File header overflow should have DESITEM = 000
        let (subheader, _) =
            create_overflow_des(OverflowSource::FileHeaderUdhd, 5, &tres, None).unwrap();
        let desitem_offset = 202; // After DESOFLW (196 + 6)
        let desitem = std::str::from_utf8(&subheader[desitem_offset..desitem_offset + 3]).unwrap();
        assert_eq!(desitem, "000");

        // Image segment overflow should have DESITEM = segment_index + 1 (1-based)
        let (subheader, _) =
            create_overflow_des(OverflowSource::ImageUdid, 2, &tres, None).unwrap();
        let desitem = std::str::from_utf8(&subheader[desitem_offset..desitem_offset + 3]).unwrap();
        assert_eq!(desitem, "003"); // 2 + 1 = 3

        // First segment (index 0) should have DESITEM = 001
        let (subheader, _) =
            create_overflow_des(OverflowSource::ImageIxshd, 0, &tres, None).unwrap();
        let desitem = std::str::from_utf8(&subheader[desitem_offset..desitem_offset + 3]).unwrap();
        assert_eq!(desitem, "001");
    }

    #[test]
    fn create_overflow_des_desshl_field() {
        let tres = vec![TreEnvelope::new("TEST", vec![]).unwrap()];
        let (subheader, _) =
            create_overflow_des(OverflowSource::ImageUdid, 0, &tres, None).unwrap();

        // DESSHL is at offset 205 (after DESITEM: 202 + 3)
        let desshl_offset = 205;
        let desshl = std::str::from_utf8(&subheader[desshl_offset..desshl_offset + 4]).unwrap();
        assert_eq!(desshl, "0000"); // TRE_OVERFLOW has no DES-defined subheader
    }

    #[test]
    fn create_overflow_des_subheader_length() {
        let tres = vec![TreEnvelope::new("TEST", vec![]).unwrap()];
        let (subheader, _) =
            create_overflow_des(OverflowSource::ImageUdid, 0, &tres, None).unwrap();

        // Expected length: DE(2) + DESID(25) + DESVER(2) + security(167) + DESOFLW(6) + DESITEM(3) + DESSHL(4) = 209
        assert_eq!(subheader.len(), 209);
    }

    #[test]
    fn create_overflow_des_empty_tres() {
        let tres: Vec<TreEnvelope> = vec![];
        let (subheader, data) =
            create_overflow_des(OverflowSource::ImageUdid, 0, &tres, None).unwrap();

        // Subheader should still be valid
        assert_eq!(subheader.len(), 209);

        // Data should be empty
        assert!(data.is_empty());
    }

    #[test]
    fn create_overflow_des_multiple_tres() {
        let tres = vec![
            TreEnvelope::new("GEOLOB", vec![0x01, 0x02, 0x03]).unwrap(),
            TreEnvelope::new("SENSRB", vec![0x04, 0x05]).unwrap(),
        ];
        let (_, data) = create_overflow_des(OverflowSource::ImageUdid, 0, &tres, None).unwrap();

        // Data should contain both TRE envelopes concatenated
        let expected_len = 14 + 13; // GEOLOB(14) + SENSRB(13)
        assert_eq!(data.len(), expected_len);

        // Verify we can parse the TREs back
        let parsed = TreEnvelope::parse_all(&data).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].tag, "GEOLOB");
        assert_eq!(parsed[1].tag, "SENSRB");
    }

    #[test]
    fn create_overflow_des_default_security() {
        let tres = vec![TreEnvelope::new("TEST", vec![]).unwrap()];
        let (subheader, _) =
            create_overflow_des(OverflowSource::ImageUdid, 0, &tres, None).unwrap();

        // DESCLAS should be "U" (Unclassified) at offset 29
        assert_eq!(&subheader[29..30], b"U");
    }

    #[test]
    fn get_image_overflow_indices_reads_fields_from_accessor() {
        use crate::parser::{
            Encoding, ExpressionEvaluator, FieldDefinition, FieldType, SizeSpec,
            StructureDefinition,
        };

        // Build a minimal definition mimicking the image subheader overflow fields:
        // UDIDL (5 bytes BCS-N), UDOFL (3 bytes BCS-N, if UDIDL.to_i > 0),
        // IXSHDL (5 bytes BCS-N), IXSOFL (3 bytes BCS-N, if IXSHDL.to_i > 0)
        let udidl_condition = ExpressionEvaluator::parse("UDIDL.to_i > 0").unwrap();
        let ixshdl_condition = ExpressionEvaluator::parse("IXSHDL.to_i > 0").unwrap();

        let def = Arc::new(
            StructureDefinition::new("test_image_overflow")
                .with_field(
                    FieldDefinition::new("UDIDL", FieldType::String)
                        .with_size(SizeSpec::Fixed(5))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("UDOFL", FieldType::String)
                        .with_size(SizeSpec::Fixed(3))
                        .with_encoding(Encoding::BcsN)
                        .with_condition(udidl_condition),
                )
                .with_field(
                    FieldDefinition::new("IXSHDL", FieldType::String)
                        .with_size(SizeSpec::Fixed(5))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("IXSOFL", FieldType::String)
                        .with_size(SizeSpec::Fixed(3))
                        .with_encoding(Encoding::BcsN)
                        .with_condition(ixshdl_condition),
                ),
        );

        // Case 1: Both overflow fields present (UDIDL=00003, UDOFL=002, IXSHDL=00003, IXSOFL=001)
        let data = b"0000300200003001";
        let accessor = StructureAccessor::new(def.clone(), data.as_slice()).unwrap();
        let (udofl, ixsofl) = get_image_overflow_indices(&accessor).unwrap();
        assert_eq!(udofl, 2);
        assert_eq!(ixsofl, 1);

        // Case 2: No overflow (UDIDL=00000, IXSHDL=00000)
        let data_no_overflow = b"0000000000";
        let accessor = StructureAccessor::new(def.clone(), data_no_overflow.as_slice()).unwrap();
        let (udofl, ixsofl) = get_image_overflow_indices(&accessor).unwrap();
        assert_eq!(udofl, 0);
        assert_eq!(ixsofl, 0);
    }

    #[test]
    fn get_file_header_overflow_indices_reads_fields_from_accessor() {
        use crate::parser::{
            Encoding, ExpressionEvaluator, FieldDefinition, FieldType, SizeSpec,
            StructureDefinition,
        };

        let udhdl_condition = ExpressionEvaluator::parse("UDHDL.to_i > 0").unwrap();
        let xhdl_condition = ExpressionEvaluator::parse("XHDL.to_i > 0").unwrap();

        let def = Arc::new(
            StructureDefinition::new("test_file_header_overflow")
                .with_field(
                    FieldDefinition::new("UDHDL", FieldType::String)
                        .with_size(SizeSpec::Fixed(5))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("UDHOFL", FieldType::String)
                        .with_size(SizeSpec::Fixed(3))
                        .with_encoding(Encoding::BcsN)
                        .with_condition(udhdl_condition),
                )
                .with_field(
                    FieldDefinition::new("XHDL", FieldType::String)
                        .with_size(SizeSpec::Fixed(5))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("XHDLOFL", FieldType::String)
                        .with_size(SizeSpec::Fixed(3))
                        .with_encoding(Encoding::BcsN)
                        .with_condition(xhdl_condition),
                ),
        );

        // Both overflow fields present
        let data = b"0000300500003007";
        let accessor = StructureAccessor::new(def.clone(), data.as_slice()).unwrap();
        let (udhofl, xhdlofl) = get_file_header_overflow_indices(&accessor).unwrap();
        assert_eq!(udhofl, 5);
        assert_eq!(xhdlofl, 7);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy to generate valid CETAG strings (1-6 alphanumeric characters)
    fn valid_cetag_strategy() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::char::ranges(vec!['A'..='Z', '0'..='9'].into()), 1..=6)
            .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    /// Strategy to generate CEDATA bytes (0 to 100 bytes for practical testing)
    fn cedata_strategy() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(any::<u8>(), 0..=100)
    }

    /// Strategy to generate a valid TRE envelope
    fn tre_envelope_strategy() -> impl Strategy<Value = TreEnvelope> {
        (valid_cetag_strategy(), cedata_strategy())
            .prop_map(|(tag, data)| TreEnvelope::new(tag, data).unwrap())
    }

    /// Strategy to generate a list of TRE envelopes
    fn tre_envelopes_strategy() -> impl Strategy<Value = Vec<TreEnvelope>> {
        prop::collection::vec(tre_envelope_strategy(), 0..=5)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Feature: tre-des-support, Property 5: TRE Overflow Resolution via Index
        ///
        /// For any segment with a non-zero overflow field (UDOFL, IXSOFL, etc.),
        /// the overflow field value SHALL be a valid 1-based DES segment index,
        /// and parsing that DES segment's data as TRE envelopes SHALL succeed.
        ///
        /// **Validates: Requirements 6.1, 6.2, 6.3, 6.4, 6.5, 6.6**
        #[test]
        fn prop_5_tre_overflow_resolution_via_index(
            tres in tre_envelopes_strategy(),
            num_des_before in 0usize..3,
            num_des_after in 0usize..3,
        ) {
            // Serialize TRE envelopes to bytes
            let mut tre_bytes = Vec::new();
            for tre in &tres {
                tre_bytes.extend(tre.to_bytes());
            }

            // Build file data with DES segments
            // Each DES has a 50-byte subheader followed by data
            let subheader_size = 50u64;
            let mut file_data = Vec::new();
            let mut des_locations = Vec::new();
            let mut current_offset = 0u64;

            // Add DES segments before the target
            for _ in 0..num_des_before {
                let dummy_data = b"DUMMY 00000"; // Empty TRE
                des_locations.push(SegmentLocation::new(
                    current_offset,
                    subheader_size,
                    current_offset + subheader_size,
                    dummy_data.len() as u64,
                ));
                file_data.extend(vec![0u8; subheader_size as usize]);
                file_data.extend_from_slice(dummy_data);
                current_offset += subheader_size + dummy_data.len() as u64;
            }

            // Add the target DES with our TRE data
            let target_index = num_des_before + 1; // 1-based index
            des_locations.push(SegmentLocation::new(
                current_offset,
                subheader_size,
                current_offset + subheader_size,
                tre_bytes.len() as u64,
            ));
            file_data.extend(vec![0u8; subheader_size as usize]);
            file_data.extend(&tre_bytes);
            current_offset += subheader_size + tre_bytes.len() as u64;

            // Add DES segments after the target
            for _ in 0..num_des_after {
                let dummy_data = b"DUMMY 00000";
                des_locations.push(SegmentLocation::new(
                    current_offset,
                    subheader_size,
                    current_offset + subheader_size,
                    dummy_data.len() as u64,
                ));
                file_data.extend(vec![0u8; subheader_size as usize]);
                file_data.extend_from_slice(dummy_data);
                current_offset += subheader_size + dummy_data.len() as u64;
            }

            // Fetch overflow TREs using the target index
            let result = fetch_overflow_tres(
                target_index as u16,
                &des_locations,
                &file_data,
            );

            // Verify the fetch succeeded
            prop_assert!(result.is_ok(), "fetch_overflow_tres should succeed for valid index");

            let fetched_tres = result.unwrap();

            // Verify the correct number of TREs were fetched
            prop_assert_eq!(
                fetched_tres.len(),
                tres.len(),
                "Should fetch the same number of TREs"
            );

            // Verify each TRE matches the original
            for (i, (original, fetched)) in tres.iter().zip(fetched_tres.iter()).enumerate() {
                // Tags should match (accounting for space padding)
                let expected_tag = format!("{:<6}", original.tag);
                prop_assert_eq!(
                    &fetched.tag,
                    &expected_tag,
                    "TRE {} tag should match", i
                );

                // Data should match exactly
                prop_assert_eq!(
                    &fetched.data,
                    &original.data,
                    "TRE {} data should match", i
                );
            }
        }

        /// Feature: tre-des-support, Property 5 (Extended): Zero index returns empty
        ///
        /// When the overflow index is 0 (no overflow), fetch_overflow_tres SHALL
        /// return an empty vector without error.
        ///
        /// **Validates: Requirements 6.1, 6.2**
        #[test]
        fn prop_5_zero_index_returns_empty(
            num_des in 0usize..5,
        ) {
            // Create some DES locations
            let des_locations: Vec<SegmentLocation> = (0..num_des)
                .map(|i| SegmentLocation::new(
                    i as u64 * 100,
                    50,
                    i as u64 * 100 + 50,
                    50,
                ))
                .collect();

            let file_data = vec![0u8; num_des * 100];

            // Fetch with index 0 (no overflow)
            let result = fetch_overflow_tres(0, &des_locations, &file_data);

            prop_assert!(result.is_ok(), "Zero index should not error");
            prop_assert!(result.unwrap().is_empty(), "Zero index should return empty vec");
        }

        /// Feature: tre-des-support, Property 5 (Extended): Invalid index returns error
        ///
        /// When the overflow index exceeds the DES count, fetch_overflow_tres SHALL
        /// return an InvalidOverflowIndex error.
        ///
        /// **Validates: Requirements 6.1, 6.2**
        #[test]
        fn prop_5_invalid_index_returns_error(
            num_des in 0usize..5,
            extra_offset in 1u16..10,
        ) {
            // Create some DES locations
            let des_locations: Vec<SegmentLocation> = (0..num_des)
                .map(|i| SegmentLocation::new(
                    i as u64 * 100,
                    50,
                    i as u64 * 100 + 50,
                    50,
                ))
                .collect();

            let file_data = vec![0u8; num_des * 100];

            // Try to fetch with an invalid index (beyond DES count)
            let invalid_index = (num_des as u16) + extra_offset;
            let result = fetch_overflow_tres(invalid_index, &des_locations, &file_data);

            prop_assert!(result.is_err(), "Invalid index should error");

            match result {
                Err(JBPError::InvalidOverflowIndex { index, des_count }) => {
                    prop_assert_eq!(index, invalid_index, "Error should contain the invalid index");
                    prop_assert_eq!(des_count, num_des, "Error should contain the DES count");
                }
                _ => prop_assert!(false, "Should be InvalidOverflowIndex error"),
            }
        }
    }
}
