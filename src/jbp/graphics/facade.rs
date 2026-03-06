//! Facade pattern for typed access to graphic subheader fields.
//!
//! The [`GraphicSubheaderFacade`] wraps a [`StructureAccessor`] to provide
//! convenient, typed access to graphic subheader fields. This pattern allows
//! the underlying structure definition to vary (e.g., NITF 2.0 vs 2.1)
//! while presenting a consistent API.
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::graphics::GraphicSubheaderFacade;
//! use osml_io::parser::StructureAccessor;
//!
//! let facade = GraphicSubheaderFacade::from_bytes(bytes, &registry, format)?;
//! let sdlvl = facade.sdlvl()?;
//! let salvl = facade.salvl()?;
//! let (row, col) = facade.sloc()?;
//! ```

use crate::error::CodecError;
use crate::jbp::types::NitfFormat;
use crate::parser::{StructureAccessor, StructureRegistry};

/// Facade providing typed access to graphic subheader fields via StructureAccessor.
///
/// This struct wraps a `StructureAccessor` and provides methods to access
/// graphic subheader fields with proper type conversion. The facade handles
/// the details of field naming and parsing, presenting a clean API for
/// accessing graphic metadata.
pub struct GraphicSubheaderFacade<'a> {
    /// The underlying structure accessor
    accessor: StructureAccessor<'a>,
}

impl<'a> GraphicSubheaderFacade<'a> {
    /// Create a facade from a StructureAccessor.
    ///
    /// # Arguments
    /// * `accessor` - The structure accessor for the graphic subheader
    ///
    /// # Returns
    /// A new `GraphicSubheaderFacade` wrapping the accessor.
    pub fn new(accessor: StructureAccessor<'a>) -> Self {
        Self { accessor }
    }

    /// Create from raw bytes using the appropriate structure definition.
    ///
    /// This constructor validates required field values:
    /// - SY must be "SY"
    /// - SFMT must be "C" (CGM format)
    /// - ENCRYP must be "0" (not encrypted)
    ///
    /// # Arguments
    /// * `data` - Raw bytes of the graphic subheader
    /// * `registry` - Structure registry for looking up definitions
    /// * `format` - NITF format variant (determines which definition to use)
    ///
    /// # Returns
    /// A new `GraphicSubheaderFacade` or an error if parsing or validation fails.
    pub fn from_bytes(
        data: &'a [u8],
        registry: &StructureRegistry,
        format: NitfFormat,
    ) -> Result<Self, CodecError> {
        let def_name = format.graphic_subheader_definition();
        let definition = registry
            .get(def_name)
            .ok_or_else(|| CodecError::InvalidFormat(format!("Structure definition not found: {}", def_name)))?;

        let accessor = StructureAccessor::new(definition, data)
            .map_err(|e| CodecError::Parse(format!("Failed to create accessor: {}", e)))?;

        let facade = Self { accessor };

        // Validate required field values
        facade.validate()?;

        Ok(facade)
    }

    /// Validate required field values.
    ///
    /// This method checks:
    /// - SY == "SY" (file part type marker)
    /// - SFMT == "C" (CGM format)
    /// - ENCRYP == "0" (not encrypted)
    fn validate(&self) -> Result<(), CodecError> {
        // Validate SY field
        let sy = self.sy()?;
        if sy != "SY" {
            return Err(CodecError::Decode(format!(
                "Invalid graphic segment marker: expected 'SY', got '{}'",
                sy
            )));
        }

        // Validate SFMT field
        let sfmt = self.sfmt()?;
        if sfmt != "C" {
            return Err(CodecError::Decode(format!(
                "Unsupported graphic format: expected 'C' (CGM), got '{}'",
                sfmt
            )));
        }

        // Validate ENCRYP field
        let encryp = self.encryp()?;
        if encryp != "0" {
            return Err(CodecError::Decode(
                "Encrypted graphics not supported".to_string()
            ));
        }

        Ok(())
    }

    /// Get the underlying accessor for direct field access.
    ///
    /// This is useful when you need to access fields not exposed by the facade,
    /// or when you need to perform custom operations on the accessor.
    pub fn accessor(&self) -> &StructureAccessor<'a> {
        &self.accessor
    }

    // ==================== Identification Field Accessors ====================

    /// Get the file part type (SY).
    ///
    /// This is a 2-character field that should always be "SY" for graphic segments.
    pub fn sy(&self) -> Result<String, CodecError> {
        self.get_str_field("SY")
    }

    /// Get the graphic identifier (SID).
    ///
    /// This is a 10-character identifier for the graphic segment.
    pub fn sid(&self) -> Result<String, CodecError> {
        self.get_str_field("SID")
    }

    /// Get the graphic name (SNAME).
    ///
    /// This is a 20-character name for the graphic segment.
    pub fn sname(&self) -> Result<String, CodecError> {
        self.get_str_field("SNAME")
    }

    /// Get the graphic type/format (SFMT).
    ///
    /// This is a 1-character field indicating the graphic format.
    /// For JBP, this must be "C" for CGM (Computer Graphics Metafile).
    pub fn sfmt(&self) -> Result<String, CodecError> {
        self.get_str_field("SFMT")
    }

    /// Get the encryption flag (ENCRYP).
    ///
    /// This is a 1-character field indicating encryption status.
    /// Must be "0" (not encrypted) for JBP compliance.
    pub fn encryp(&self) -> Result<String, CodecError> {
        self.get_str_field("ENCRYP")
    }

    // ==================== Display/Attachment Level Accessors ====================

    /// Get the graphic display level (SDLVL).
    ///
    /// This is a 3-digit value (001-999) determining the z-order for rendering.
    /// Higher values render on top of lower values.
    pub fn sdlvl(&self) -> Result<u32, CodecError> {
        self.get_u32_field("SDLVL")
    }

    /// Get the graphic attachment level (SALVL).
    ///
    /// This is a 3-digit value (000-998) indicating which segment this graphic
    /// attaches to:
    /// - 0 = unattached to any image segment
    /// - 1-998 = attached to the image segment with matching display level
    pub fn salvl(&self) -> Result<u32, CodecError> {
        self.get_u32_field("SALVL")
    }

    // ==================== Location Accessors ====================

    /// Get the graphic location (SLOC).
    ///
    /// Returns the row and column offset relative to the attached segment's origin.
    /// Format is RRRRRCCCCC (5-digit row, 5-digit column).
    ///
    /// # Returns
    /// A tuple of (row, column) as signed integers.
    pub fn sloc(&self) -> Result<(i32, i32), CodecError> {
        let value = self.get_str_field("SLOC")?;
        Self::parse_location(&value, "SLOC")
    }

    /// Get the first graphic bound location (SBND1).
    ///
    /// Returns the row and column of the upper-left corner of the bounding box.
    /// Format is RRRRRCCCCC (5-digit row, 5-digit column).
    ///
    /// # Returns
    /// A tuple of (row, column) as signed integers.
    pub fn sbnd1(&self) -> Result<(i32, i32), CodecError> {
        let value = self.get_str_field("SBND1")?;
        Self::parse_location(&value, "SBND1")
    }

    /// Get the second graphic bound location (SBND2).
    ///
    /// Returns the row and column of the lower-right corner of the bounding box.
    /// Format is RRRRRCCCCC (5-digit row, 5-digit column).
    ///
    /// # Returns
    /// A tuple of (row, column) as signed integers.
    pub fn sbnd2(&self) -> Result<(i32, i32), CodecError> {
        let value = self.get_str_field("SBND2")?;
        Self::parse_location(&value, "SBND2")
    }

    /// Get the graphic color indicator (SCOLOR).
    ///
    /// This is a 1-character field indicating color capability:
    /// - "C" = color
    /// - "M" = monochrome
    pub fn scolor(&self) -> Result<String, CodecError> {
        self.get_str_field("SCOLOR")
    }

    // ==================== TRE Accessors ====================

    /// Get the extended subheader data length (SXSHDL).
    ///
    /// This is a 5-digit value indicating the length of extended subheader data:
    /// - 00000 = no TREs
    /// - 00003-99999 = length of SXSOFL + SXSHD
    pub fn sxshdl(&self) -> Result<u32, CodecError> {
        self.get_u32_field("SXSHDL")
    }

    /// Get the extended subheader overflow indicator (SXSOFL).
    ///
    /// This is a 3-digit value indicating TRE overflow:
    /// - 000 = no overflow
    /// - 001-999 = DES sequence number containing overflow TREs
    ///
    /// This field is only present when SXSHDL > 0.
    ///
    /// # Returns
    /// The overflow indicator, or an error if SXSHDL is 0.
    pub fn sxsofl(&self) -> Result<u32, CodecError> {
        let sxshdl = self.sxshdl()?;
        if sxshdl == 0 {
            return Err(CodecError::Parse(
                "SXSOFL field not present when SXSHDL is 0".to_string()
            ));
        }
        self.get_u32_field("SXSOFL")
    }

    // ==================== Location Parsing Helper ====================

    /// Parse a location string in RRRRRCCCCC format.
    ///
    /// The format is a 10-character string where:
    /// - Characters 0-4 are the row value (can be negative with leading sign)
    /// - Characters 5-9 are the column value (can be negative with leading sign)
    ///
    /// # Arguments
    /// * `value` - The 10-character location string
    /// * `field_name` - Name of the field for error messages
    ///
    /// # Returns
    /// A tuple of (row, column) as signed integers.
    fn parse_location(value: &str, field_name: &str) -> Result<(i32, i32), CodecError> {
        if value.len() != 10 {
            return Err(CodecError::Parse(format!(
                "Invalid {} format: expected 10 characters, got {}",
                field_name,
                value.len()
            )));
        }

        let row_str = &value[0..5];
        let col_str = &value[5..10];

        let row = row_str.trim().parse::<i32>().map_err(|e| {
            CodecError::Parse(format!(
                "Failed to parse {} row '{}': {}",
                field_name, row_str, e
            ))
        })?;

        let col = col_str.trim().parse::<i32>().map_err(|e| {
            CodecError::Parse(format!(
                "Failed to parse {} column '{}': {}",
                field_name, col_str, e
            ))
        })?;

        Ok((row, col))
    }

    // ==================== Private Helper Methods ====================

    /// Get a string field from the accessor.
    fn get_str_field(&self, field: &str) -> Result<String, CodecError> {
        let value = self.accessor
            .get(field)
            .map_err(|e| CodecError::Parse(format!("Failed to read field '{}': {}", field, e)))?;
        let s = value
            .as_str()
            .map_err(|e| CodecError::Parse(format!("Failed to parse field '{}' as string: {}", field, e)))?;
        Ok(s.to_string())
    }

    /// Get a u32 field from the accessor (parsed from string).
    fn get_u32_field(&self, field: &str) -> Result<u32, CodecError> {
        let s = self.get_str_field(field)?;
        s.trim()
            .parse::<u32>()
            .map_err(|e| CodecError::Parse(format!("Failed to parse field '{}' as u32: {}", field, e)))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ExpressionEvaluator, FieldDefinition, FieldType, SizeSpec, StructureDefinition};

    /// Create a minimal graphic subheader structure definition for testing.
    fn create_test_graphic_subheader_definition() -> StructureDefinition {
        // Condition for SXSOFL: present when SXSHDL > 0
        let sxsofl_condition = ExpressionEvaluator::parse("SXSHDL.to_i > 0").unwrap();

        // Condition for SXSHD: present when SXSHDL > 3 (SXSHDL includes 3 bytes for SXSOFL)
        let sxshd_condition = ExpressionEvaluator::parse("SXSHDL.to_i > 3").unwrap();

        StructureDefinition::new("nitf_02.10_graphic_subheader")
            .with_field(
                FieldDefinition::new("SY", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("File Part Type"),
            )
            .with_field(
                FieldDefinition::new("SID", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("Graphic Identifier"),
            )
            .with_field(
                FieldDefinition::new("SNAME", FieldType::String)
                    .with_size(SizeSpec::Fixed(20))
                    .with_doc("Graphic Name"),
            )
            // Security fields (simplified for testing)
            .with_field(
                FieldDefinition::new("SSCLAS", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Security Classification"),
            )
            .with_field(
                FieldDefinition::new("SSCLSY", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Security Classification System"),
            )
            .with_field(
                FieldDefinition::new("SSCODE", FieldType::String)
                    .with_size(SizeSpec::Fixed(11))
                    .with_doc("Codewords"),
            )
            .with_field(
                FieldDefinition::new("SSCTLH", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Control and Handling"),
            )
            .with_field(
                FieldDefinition::new("SSREL", FieldType::String)
                    .with_size(SizeSpec::Fixed(20))
                    .with_doc("Releasing Instructions"),
            )
            .with_field(
                FieldDefinition::new("SSDCTP", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Declassification Type"),
            )
            .with_field(
                FieldDefinition::new("SSDCDT", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_doc("Declassification Date"),
            )
            .with_field(
                FieldDefinition::new("SSDCXM", FieldType::String)
                    .with_size(SizeSpec::Fixed(4))
                    .with_doc("Declassification Exemption"),
            )
            .with_field(
                FieldDefinition::new("SSDG", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Downgrade"),
            )
            .with_field(
                FieldDefinition::new("SSDGDT", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_doc("Downgrade Date"),
            )
            .with_field(
                FieldDefinition::new("SSCLTX", FieldType::String)
                    .with_size(SizeSpec::Fixed(43))
                    .with_doc("Classification Text"),
            )
            .with_field(
                FieldDefinition::new("SSCATP", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Classification Authority Type"),
            )
            .with_field(
                FieldDefinition::new("SSCAUT", FieldType::String)
                    .with_size(SizeSpec::Fixed(40))
                    .with_doc("Classification Authority"),
            )
            .with_field(
                FieldDefinition::new("SSCRSN", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Classification Reason"),
            )
            .with_field(
                FieldDefinition::new("SSSRDT", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_doc("Security Source Date"),
            )
            .with_field(
                FieldDefinition::new("SSCTLN", FieldType::String)
                    .with_size(SizeSpec::Fixed(15))
                    .with_doc("Security Control Number"),
            )
            .with_field(
                FieldDefinition::new("ENCRYP", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Encryption"),
            )
            .with_field(
                FieldDefinition::new("SFMT", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Graphic Type"),
            )
            .with_field(
                FieldDefinition::new("SSTRUCT", FieldType::String)
                    .with_size(SizeSpec::Fixed(13))
                    .with_doc("Reserved"),
            )
            .with_field(
                FieldDefinition::new("SDLVL", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_doc("Display Level"),
            )
            .with_field(
                FieldDefinition::new("SALVL", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_doc("Attachment Level"),
            )
            .with_field(
                FieldDefinition::new("SLOC", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("Location"),
            )
            .with_field(
                FieldDefinition::new("SBND1", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("First Bound"),
            )
            .with_field(
                FieldDefinition::new("SCOLOR", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Color"),
            )
            .with_field(
                FieldDefinition::new("SBND2", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("Second Bound"),
            )
            .with_field(
                FieldDefinition::new("SRES2", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Reserved"),
            )
            .with_field(
                FieldDefinition::new("SXSHDL", FieldType::String)
                    .with_size(SizeSpec::Fixed(5))
                    .with_doc("Extended Subheader Length"),
            )
            .with_field(
                FieldDefinition::new("SXSOFL", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_condition(sxsofl_condition)
                    .with_doc("Extended Subheader Overflow"),
            )
            .with_field(
                FieldDefinition::new("SXSHD", FieldType::Bytes)
                    .with_size(SizeSpec::Expression(
                        ExpressionEvaluator::parse("SXSHDL.to_i - 3").unwrap(),
                    ))
                    .with_condition(sxshd_condition)
                    .with_doc("Extended Subheader Data"),
            )
    }

    /// Create test graphic subheader bytes.
    fn create_test_subheader_bytes(
        sy: &str,
        sid: &str,
        sname: &str,
        encryp: &str,
        sfmt: &str,
        sdlvl: &str,
        salvl: &str,
        sloc: &str,
        sbnd1: &str,
        scolor: &str,
        sbnd2: &str,
        sxshdl: &str,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // SY (2)
        bytes.extend_from_slice(format!("{:2}", sy).as_bytes());
        // SID (10)
        bytes.extend_from_slice(format!("{:10}", sid).as_bytes());
        // SNAME (20)
        bytes.extend_from_slice(format!("{:20}", sname).as_bytes());
        // Security fields (167 bytes total)
        bytes.extend_from_slice(b"U");                    // SSCLAS (1)
        bytes.extend_from_slice(b"  ");                   // SSCLSY (2)
        bytes.extend_from_slice(b"           ");          // SSCODE (11)
        bytes.extend_from_slice(b"  ");                   // SSCTLH (2)
        bytes.extend_from_slice(b"                    "); // SSREL (20)
        bytes.extend_from_slice(b"  ");                   // SSDCTP (2)
        bytes.extend_from_slice(b"        ");             // SSDCDT (8)
        bytes.extend_from_slice(b"    ");                 // SSDCXM (4)
        bytes.extend_from_slice(b" ");                    // SSDG (1)
        bytes.extend_from_slice(b"        ");             // SSDGDT (8)
        bytes.extend_from_slice(b"                                           "); // SSCLTX (43)
        bytes.extend_from_slice(b" ");                    // SSCATP (1)
        bytes.extend_from_slice(b"                                        "); // SSCAUT (40)
        bytes.extend_from_slice(b" ");                    // SSCRSN (1)
        bytes.extend_from_slice(b"        ");             // SSSRDT (8)
        bytes.extend_from_slice(b"               ");      // SSCTLN (15)
        // ENCRYP (1)
        bytes.extend_from_slice(format!("{:1}", encryp).as_bytes());
        // SFMT (1)
        bytes.extend_from_slice(format!("{:1}", sfmt).as_bytes());
        // SSTRUCT (13)
        bytes.extend_from_slice(b"             ");
        // SDLVL (3)
        bytes.extend_from_slice(format!("{:03}", sdlvl).as_bytes());
        // SALVL (3)
        bytes.extend_from_slice(format!("{:03}", salvl).as_bytes());
        // SLOC (10)
        bytes.extend_from_slice(format!("{:10}", sloc).as_bytes());
        // SBND1 (10)
        bytes.extend_from_slice(format!("{:10}", sbnd1).as_bytes());
        // SCOLOR (1)
        bytes.extend_from_slice(format!("{:1}", scolor).as_bytes());
        // SBND2 (10)
        bytes.extend_from_slice(format!("{:10}", sbnd2).as_bytes());
        // SRES2 (2)
        bytes.extend_from_slice(b"  ");
        // SXSHDL (5)
        bytes.extend_from_slice(format!("{:05}", sxshdl).as_bytes());
        
        bytes
    }

    fn create_test_registry() -> StructureRegistry {
        let mut registry = StructureRegistry::new();
        registry.register("nitf_02.10_graphic_subheader", create_test_graphic_subheader_definition());
        registry
    }

    #[test]
    fn facade_basic_fields() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "SY",
            "GRAPHIC001",
            "Test Graphic Name",
            "0",
            "C",
            "001",
            "000",
            "0010000200",
            "0000000000",
            "C",
            "0050000500",
            "00000",
        );

        let facade = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();

        assert_eq!(facade.sy().unwrap(), "SY");
        assert_eq!(facade.sid().unwrap().trim(), "GRAPHIC001");
        assert_eq!(facade.sname().unwrap().trim(), "Test Graphic Name");
        assert_eq!(facade.sfmt().unwrap(), "C");
        assert_eq!(facade.encryp().unwrap(), "0");
    }

    #[test]
    fn facade_display_attachment_levels() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "SY",
            "GRAPHIC001",
            "Test Graphic",
            "0",
            "C",
            "005",
            "001",
            "0010000200",
            "0000000000",
            "C",
            "0050000500",
            "00000",
        );

        let facade = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();

        assert_eq!(facade.sdlvl().unwrap(), 5);
        assert_eq!(facade.salvl().unwrap(), 1);
    }

    #[test]
    fn facade_location_parsing() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "SY",
            "GRAPHIC001",
            "Test Graphic",
            "0",
            "C",
            "001",
            "000",
            "0010000200",  // row=100, col=200
            "0000000000",  // row=0, col=0
            "C",
            "0050000500",  // row=500, col=500
            "00000",
        );

        let facade = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();

        let (row, col) = facade.sloc().unwrap();
        assert_eq!(row, 100);
        assert_eq!(col, 200);

        let (row1, col1) = facade.sbnd1().unwrap();
        assert_eq!(row1, 0);
        assert_eq!(col1, 0);

        let (row2, col2) = facade.sbnd2().unwrap();
        assert_eq!(row2, 500);
        assert_eq!(col2, 500);
    }

    #[test]
    fn facade_scolor() {
        let registry = create_test_registry();
        
        // Test color
        let bytes_color = create_test_subheader_bytes(
            "SY", "GRAPHIC001", "Test", "0", "C", "001", "000",
            "0000000000", "0000000000", "C", "0000000000", "00000",
        );
        let facade = GraphicSubheaderFacade::from_bytes(&bytes_color, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.scolor().unwrap(), "C");

        // Test monochrome
        let bytes_mono = create_test_subheader_bytes(
            "SY", "GRAPHIC001", "Test", "0", "C", "001", "000",
            "0000000000", "0000000000", "M", "0000000000", "00000",
        );
        let facade = GraphicSubheaderFacade::from_bytes(&bytes_mono, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.scolor().unwrap(), "M");
    }

    #[test]
    fn facade_sxshdl_zero() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "SY", "GRAPHIC001", "Test", "0", "C", "001", "000",
            "0000000000", "0000000000", "C", "0000000000", "00000",
        );

        let facade = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.sxshdl().unwrap(), 0);
        
        // SXSOFL should error when SXSHDL is 0
        assert!(facade.sxsofl().is_err());
    }

    #[test]
    fn facade_invalid_sy_marker() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "XX",  // Invalid SY marker
            "GRAPHIC001", "Test", "0", "C", "001", "000",
            "0000000000", "0000000000", "C", "0000000000", "00000",
        );

        let result = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Invalid graphic segment marker"));
    }

    #[test]
    fn facade_invalid_sfmt() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "SY", "GRAPHIC001", "Test", "0",
            "X",  // Invalid SFMT (not "C")
            "001", "000",
            "0000000000", "0000000000", "C", "0000000000", "00000",
        );

        let result = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Unsupported graphic format"));
    }

    #[test]
    fn facade_encrypted_graphics_rejected() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "SY", "GRAPHIC001", "Test",
            "1",  // Encrypted
            "C", "001", "000",
            "0000000000", "0000000000", "C", "0000000000", "00000",
        );

        let result = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Encrypted graphics not supported"));
    }

    #[test]
    fn parse_location_valid() {
        // Positive values
        let (row, col) = GraphicSubheaderFacade::parse_location("0010000200", "TEST").unwrap();
        assert_eq!(row, 100);
        assert_eq!(col, 200);

        // Zero values
        let (row, col) = GraphicSubheaderFacade::parse_location("0000000000", "TEST").unwrap();
        assert_eq!(row, 0);
        assert_eq!(col, 0);

        // Max values
        let (row, col) = GraphicSubheaderFacade::parse_location("9999999999", "TEST").unwrap();
        assert_eq!(row, 99999);
        assert_eq!(col, 99999);
    }

    #[test]
    fn parse_location_negative_values() {
        // Negative row
        let (row, col) = GraphicSubheaderFacade::parse_location("-010000200", "TEST").unwrap();
        assert_eq!(row, -100);
        assert_eq!(col, 200);

        // Negative column
        let (row, col) = GraphicSubheaderFacade::parse_location("00100-0200", "TEST").unwrap();
        assert_eq!(row, 100);
        assert_eq!(col, -200);

        // Both negative
        let (row, col) = GraphicSubheaderFacade::parse_location("-0100-0200", "TEST").unwrap();
        assert_eq!(row, -100);
        assert_eq!(col, -200);
    }

    #[test]
    fn parse_location_invalid_length() {
        let result = GraphicSubheaderFacade::parse_location("12345", "TEST");
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("expected 10 characters"));
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::parser::{ExpressionEvaluator, FieldDefinition, FieldType, SizeSpec, StructureDefinition};
    use proptest::prelude::*;

    /// Create a minimal graphic subheader structure definition for property testing.
    fn create_test_graphic_subheader_definition() -> StructureDefinition {
        // Condition for SXSOFL: present when SXSHDL > 0
        let sxsofl_condition = ExpressionEvaluator::parse("SXSHDL.to_i > 0").unwrap();

        // Condition for SXSHD: present when SXSHDL > 3 (SXSHDL includes 3 bytes for SXSOFL)
        let sxshd_condition = ExpressionEvaluator::parse("SXSHDL.to_i > 3").unwrap();

        StructureDefinition::new("nitf_02.10_graphic_subheader")
            .with_field(
                FieldDefinition::new("SY", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("File Part Type"),
            )
            .with_field(
                FieldDefinition::new("SID", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("Graphic Identifier"),
            )
            .with_field(
                FieldDefinition::new("SNAME", FieldType::String)
                    .with_size(SizeSpec::Fixed(20))
                    .with_doc("Graphic Name"),
            )
            // Security fields (simplified for testing)
            .with_field(
                FieldDefinition::new("SSCLAS", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Security Classification"),
            )
            .with_field(
                FieldDefinition::new("SSCLSY", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Security Classification System"),
            )
            .with_field(
                FieldDefinition::new("SSCODE", FieldType::String)
                    .with_size(SizeSpec::Fixed(11))
                    .with_doc("Codewords"),
            )
            .with_field(
                FieldDefinition::new("SSCTLH", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Control and Handling"),
            )
            .with_field(
                FieldDefinition::new("SSREL", FieldType::String)
                    .with_size(SizeSpec::Fixed(20))
                    .with_doc("Releasing Instructions"),
            )
            .with_field(
                FieldDefinition::new("SSDCTP", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Declassification Type"),
            )
            .with_field(
                FieldDefinition::new("SSDCDT", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_doc("Declassification Date"),
            )
            .with_field(
                FieldDefinition::new("SSDCXM", FieldType::String)
                    .with_size(SizeSpec::Fixed(4))
                    .with_doc("Declassification Exemption"),
            )
            .with_field(
                FieldDefinition::new("SSDG", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Downgrade"),
            )
            .with_field(
                FieldDefinition::new("SSDGDT", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_doc("Downgrade Date"),
            )
            .with_field(
                FieldDefinition::new("SSCLTX", FieldType::String)
                    .with_size(SizeSpec::Fixed(43))
                    .with_doc("Classification Text"),
            )
            .with_field(
                FieldDefinition::new("SSCATP", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Classification Authority Type"),
            )
            .with_field(
                FieldDefinition::new("SSCAUT", FieldType::String)
                    .with_size(SizeSpec::Fixed(40))
                    .with_doc("Classification Authority"),
            )
            .with_field(
                FieldDefinition::new("SSCRSN", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Classification Reason"),
            )
            .with_field(
                FieldDefinition::new("SSSRDT", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_doc("Security Source Date"),
            )
            .with_field(
                FieldDefinition::new("SSCTLN", FieldType::String)
                    .with_size(SizeSpec::Fixed(15))
                    .with_doc("Security Control Number"),
            )
            .with_field(
                FieldDefinition::new("ENCRYP", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Encryption"),
            )
            .with_field(
                FieldDefinition::new("SFMT", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Graphic Type"),
            )
            .with_field(
                FieldDefinition::new("SSTRUCT", FieldType::String)
                    .with_size(SizeSpec::Fixed(13))
                    .with_doc("Reserved"),
            )
            .with_field(
                FieldDefinition::new("SDLVL", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_doc("Display Level"),
            )
            .with_field(
                FieldDefinition::new("SALVL", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_doc("Attachment Level"),
            )
            .with_field(
                FieldDefinition::new("SLOC", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("Location"),
            )
            .with_field(
                FieldDefinition::new("SBND1", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("First Bound"),
            )
            .with_field(
                FieldDefinition::new("SCOLOR", FieldType::String)
                    .with_size(SizeSpec::Fixed(1))
                    .with_doc("Color"),
            )
            .with_field(
                FieldDefinition::new("SBND2", FieldType::String)
                    .with_size(SizeSpec::Fixed(10))
                    .with_doc("Second Bound"),
            )
            .with_field(
                FieldDefinition::new("SRES2", FieldType::String)
                    .with_size(SizeSpec::Fixed(2))
                    .with_doc("Reserved"),
            )
            .with_field(
                FieldDefinition::new("SXSHDL", FieldType::String)
                    .with_size(SizeSpec::Fixed(5))
                    .with_doc("Extended Subheader Length"),
            )
            .with_field(
                FieldDefinition::new("SXSOFL", FieldType::String)
                    .with_size(SizeSpec::Fixed(3))
                    .with_condition(sxsofl_condition)
                    .with_doc("Extended Subheader Overflow"),
            )
            .with_field(
                FieldDefinition::new("SXSHD", FieldType::Bytes)
                    .with_size(SizeSpec::Expression(
                        ExpressionEvaluator::parse("SXSHDL.to_i - 3").unwrap(),
                    ))
                    .with_condition(sxshd_condition)
                    .with_doc("Extended Subheader Data"),
            )
    }

    /// Create test graphic subheader bytes with specified SALVL and bounds.
    fn create_test_subheader_bytes_with_bounds(
        salvl: u32,
        sbnd1_row: i32,
        sbnd1_col: i32,
        sbnd2_row: i32,
        sbnd2_col: i32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // SY (2)
        bytes.extend_from_slice(b"SY");
        // SID (10)
        bytes.extend_from_slice(b"GRAPHIC001");
        // SNAME (20)
        bytes.extend_from_slice(b"Test Graphic        ");
        // Security fields (167 bytes total)
        bytes.extend_from_slice(b"U");                    // SSCLAS (1)
        bytes.extend_from_slice(b"  ");                   // SSCLSY (2)
        bytes.extend_from_slice(b"           ");          // SSCODE (11)
        bytes.extend_from_slice(b"  ");                   // SSCTLH (2)
        bytes.extend_from_slice(b"                    "); // SSREL (20)
        bytes.extend_from_slice(b"  ");                   // SSDCTP (2)
        bytes.extend_from_slice(b"        ");             // SSDCDT (8)
        bytes.extend_from_slice(b"    ");                 // SSDCXM (4)
        bytes.extend_from_slice(b" ");                    // SSDG (1)
        bytes.extend_from_slice(b"        ");             // SSDGDT (8)
        bytes.extend_from_slice(b"                                           "); // SSCLTX (43)
        bytes.extend_from_slice(b" ");                    // SSCATP (1)
        bytes.extend_from_slice(b"                                        "); // SSCAUT (40)
        bytes.extend_from_slice(b" ");                    // SSCRSN (1)
        bytes.extend_from_slice(b"        ");             // SSSRDT (8)
        bytes.extend_from_slice(b"               ");      // SSCTLN (15)
        // ENCRYP (1)
        bytes.extend_from_slice(b"0");
        // SFMT (1)
        bytes.extend_from_slice(b"C");
        // SSTRUCT (13)
        bytes.extend_from_slice(b"             ");
        // SDLVL (3) - Display level 001
        bytes.extend_from_slice(b"001");
        // SALVL (3) - Attachment level (from parameter)
        bytes.extend_from_slice(format!("{:03}", salvl).as_bytes());
        // SLOC (10) - Location 0,0
        bytes.extend_from_slice(b"0000000000");
        // SBND1 (10) - First bound (from parameters)
        bytes.extend_from_slice(format!("{:05}{:05}", sbnd1_row, sbnd1_col).as_bytes());
        // SCOLOR (1) - Color
        bytes.extend_from_slice(b"C");
        // SBND2 (10) - Second bound (from parameters)
        bytes.extend_from_slice(format!("{:05}{:05}", sbnd2_row, sbnd2_col).as_bytes());
        // SRES2 (2) - Reserved
        bytes.extend_from_slice(b"  ");
        // SXSHDL (5) - No extended subheader
        bytes.extend_from_slice(b"00000");
        
        bytes
    }

    fn create_test_registry() -> StructureRegistry {
        let mut registry = StructureRegistry::new();
        registry.register("nitf_02.10_graphic_subheader", create_test_graphic_subheader_definition());
        registry
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 3: Invalid SALVL Reference Parsing
        ///
        /// For any graphic subheader with SALVL referencing a non-existent display level,
        /// parsing SHALL succeed without validation errors (cross-reference validation
        /// is caller's responsibility).
        ///
        /// **Feature: jbp-graphic-segments, Property 3: Invalid SALVL Reference Parsing**
        /// **Validates: Requirements 3.4**
        #[test]
        fn prop_invalid_salvl_reference_parsing(
            salvl in 0u32..=998,  // Full valid range for SALVL field
        ) {
            let registry = create_test_registry();
            
            // Create subheader with the generated SALVL value
            // Note: SALVL can reference any display level (0-998), even if no segment
            // with that display level exists. The parser should not validate this.
            let bytes = create_test_subheader_bytes_with_bounds(
                salvl,
                0, 0,    // SBND1: row=0, col=0
                100, 100 // SBND2: row=100, col=100
            );
            
            // Parsing should succeed regardless of SALVL value
            let result = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
            prop_assert!(result.is_ok(), "Parsing failed for SALVL={}: {:?}", salvl, result.err());
            
            let facade = result.unwrap();
            
            // Verify the SALVL value was correctly parsed
            let parsed_salvl = facade.salvl().unwrap();
            prop_assert_eq!(parsed_salvl, salvl, "SALVL mismatch: expected {}, got {}", salvl, parsed_salvl);
        }

        /// Property 4: Invalid Bounds Parsing
        ///
        /// For any graphic subheader where SBND1 row > SBND2 row OR SBND1 column > SBND2 column,
        /// parsing SHALL succeed and expose the values through metadata without error.
        ///
        /// **Feature: jbp-graphic-segments, Property 4: Invalid Bounds Parsing**
        /// **Validates: Requirements 4.4, 4.5**
        #[test]
        fn prop_invalid_bounds_parsing(
            sbnd1_row in 0i32..=9999,
            sbnd1_col in 0i32..=9999,
            sbnd2_row in 0i32..=9999,
            sbnd2_col in 0i32..=9999,
        ) {
            let registry = create_test_registry();
            
            // Create subheader with the generated bounds
            // This includes cases where SBND1 > SBND2 (inverted bounds)
            let bytes = create_test_subheader_bytes_with_bounds(
                0,  // SALVL = 0 (unattached)
                sbnd1_row, sbnd1_col,
                sbnd2_row, sbnd2_col
            );
            
            // Parsing should succeed regardless of whether bounds are valid
            let result = GraphicSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
            prop_assert!(
                result.is_ok(),
                "Parsing failed for SBND1=({},{}) SBND2=({},{}): {:?}",
                sbnd1_row, sbnd1_col, sbnd2_row, sbnd2_col, result.err()
            );
            
            let facade = result.unwrap();
            
            // Verify the bounds were correctly parsed
            let (parsed_row1, parsed_col1) = facade.sbnd1().unwrap();
            let (parsed_row2, parsed_col2) = facade.sbnd2().unwrap();
            
            prop_assert_eq!(parsed_row1, sbnd1_row, "SBND1 row mismatch");
            prop_assert_eq!(parsed_col1, sbnd1_col, "SBND1 col mismatch");
            prop_assert_eq!(parsed_row2, sbnd2_row, "SBND2 row mismatch");
            prop_assert_eq!(parsed_col2, sbnd2_col, "SBND2 col mismatch");
            
            // Explicitly test inverted bounds case (this is the key property)
            // The parser should NOT reject inverted bounds
            let is_inverted_row = sbnd1_row > sbnd2_row;
            let is_inverted_col = sbnd1_col > sbnd2_col;
            
            if is_inverted_row || is_inverted_col {
                // Even with inverted bounds, parsing succeeded (we're here)
                // and values are accessible - this is the expected behavior
                prop_assert!(true, "Inverted bounds were correctly accepted");
            }
        }
    }
}
