//! Text segment support for JBP/NITF files.
//!
//! This module provides typed access to text segment subheaders and metadata.
//! Text segments contain textual data with associated metadata for display
//! positioning and character encoding.
//!
//! # Key Components
//!
//! - [`TextSubheaderFacade`] - Typed access to text subheader fields
//! - [`create_text_subheader_definition`] - Creates the full text subheader structure definition
//!
//! # Text Format Codes
//!
//! The TXTFMT field indicates the character encoding:
//! - `STA` - Standard BCS (ASCII) text with CR/LF line delimiters
//! - `MTF` - Message Text Formatting per STANAG 5500/MIL-STD-6040
//! - `UT1` - Legacy ECS (Extended Character Set) text formatting
//! - `U8S` - UTF-8 text formatting (modern replacement for UT1)
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::text::TextSubheaderFacade;
//!
//! let facade = TextSubheaderFacade::from_bytes(subheader_bytes, &registry, format)?;
//! let textid = facade.textid()?;  // Text identifier
//! let txtalvl = facade.txtalvl()?;  // Attachment level (000-998)
//! let txtfmt = facade.txtfmt()?;  // Text format (STA, MTF, UT1, U8S)
//! ```

mod encoding;
mod facade;

pub use encoding::{decode_and_normalize, encode_with_crlf, normalize_line_endings};
pub use facade::TextSubheaderFacade;

use crate::parser::{ExpressionEvaluator, FieldDefinition, FieldType, SizeSpec, StructureDefinition};

/// Create a full text subheader structure definition per JBP Table 5.17-1.
///
/// This definition includes all fields from the JBP specification:
/// - TE, TEXTID: Identification fields
/// - TXTALVL: Text attachment level (000-998)
/// - TXTDT: Text date and time (CCYYMMDDhhmmss)
/// - TXTITL: Text title (80 characters)
/// - Security fields (TSCLAS through TSCTLN): 167 bytes total
/// - ENCRYP: Encryption (must be "0")
/// - TXTFMT: Text format (STA, MTF, UT1, U8S)
/// - TXSHDL, TXSOFL, TXSHD: Extended subheader data (TRE support)
///
/// # Conditional Fields
///
/// - TXSOFL: Present when TXSHDL > 0
/// - TXSHD: Present when TXSHDL > 3 (TXSHDL includes 3 bytes for TXSOFL)
pub fn create_text_subheader_definition() -> StructureDefinition {
    // Condition for TXSOFL: present when TXSHDL > 0
    let txsofl_condition = ExpressionEvaluator::parse("TXSHDL.to_i > 0").unwrap();

    // Condition for TXSHD: present when TXSHDL > 3 (TXSHDL includes 3 bytes for TXSOFL)
    let txshd_condition = ExpressionEvaluator::parse("TXSHDL.to_i > 3").unwrap();

    StructureDefinition::new("nitf_02.10_text_subheader")
        // File Part Type - always "TE"
        .with_field(
            FieldDefinition::new("TE", FieldType::String)
                .with_size(SizeSpec::Fixed(2))
                .with_doc("File Part Type"),
        )
        // Text Identifier
        .with_field(
            FieldDefinition::new("TEXTID", FieldType::String)
                .with_size(SizeSpec::Fixed(7))
                .with_doc("Text Identifier"),
        )
        // Text Attachment Level (000-998)
        .with_field(
            FieldDefinition::new("TXTALVL", FieldType::String)
                .with_size(SizeSpec::Fixed(3))
                .with_doc("Text Attachment Level"),
        )
        // Text Date and Time (CCYYMMDDhhmmss)
        .with_field(
            FieldDefinition::new("TXTDT", FieldType::String)
                .with_size(SizeSpec::Fixed(14))
                .with_doc("Text Date and Time"),
        )
        // Text Title
        .with_field(
            FieldDefinition::new("TXTITL", FieldType::String)
                .with_size(SizeSpec::Fixed(80))
                .with_doc("Text Title"),
        )
        // Security Fields (167 bytes total) - using "TS" prefix for text segments
        // Security Classification
        .with_field(
            FieldDefinition::new("TSCLAS", FieldType::String)
                .with_size(SizeSpec::Fixed(1))
                .with_doc("Text Security Classification"),
        )
        // Security Classification System
        .with_field(
            FieldDefinition::new("TSCLSY", FieldType::String)
                .with_size(SizeSpec::Fixed(2))
                .with_doc("Text Security Classification System"),
        )
        // Codewords
        .with_field(
            FieldDefinition::new("TSCODE", FieldType::String)
                .with_size(SizeSpec::Fixed(11))
                .with_doc("Text Codewords"),
        )
        // Control and Handling
        .with_field(
            FieldDefinition::new("TSCTLH", FieldType::String)
                .with_size(SizeSpec::Fixed(2))
                .with_doc("Text Control and Handling"),
        )
        // Releasing Instructions
        .with_field(
            FieldDefinition::new("TSREL", FieldType::String)
                .with_size(SizeSpec::Fixed(20))
                .with_doc("Text Releasing Instructions"),
        )
        // Declassification Type
        .with_field(
            FieldDefinition::new("TSDCTP", FieldType::String)
                .with_size(SizeSpec::Fixed(2))
                .with_doc("Text Declassification Type"),
        )
        // Declassification Date
        .with_field(
            FieldDefinition::new("TSDCDT", FieldType::String)
                .with_size(SizeSpec::Fixed(8))
                .with_doc("Text Declassification Date"),
        )
        // Declassification Exemption
        .with_field(
            FieldDefinition::new("TSDCXM", FieldType::String)
                .with_size(SizeSpec::Fixed(4))
                .with_doc("Text Declassification Exemption"),
        )
        // Downgrade
        .with_field(
            FieldDefinition::new("TSDG", FieldType::String)
                .with_size(SizeSpec::Fixed(1))
                .with_doc("Text Downgrade"),
        )
        // Downgrade Date
        .with_field(
            FieldDefinition::new("TSDGDT", FieldType::String)
                .with_size(SizeSpec::Fixed(8))
                .with_doc("Text Downgrade Date"),
        )
        // Classification Text
        .with_field(
            FieldDefinition::new("TSCLTX", FieldType::String)
                .with_size(SizeSpec::Fixed(43))
                .with_doc("Text Classification Text"),
        )
        // Classification Authority Type
        .with_field(
            FieldDefinition::new("TSCATP", FieldType::String)
                .with_size(SizeSpec::Fixed(1))
                .with_doc("Text Classification Authority Type"),
        )
        // Classification Authority
        .with_field(
            FieldDefinition::new("TSCAUT", FieldType::String)
                .with_size(SizeSpec::Fixed(40))
                .with_doc("Text Classification Authority"),
        )
        // Classification Reason
        .with_field(
            FieldDefinition::new("TSCRSN", FieldType::String)
                .with_size(SizeSpec::Fixed(1))
                .with_doc("Text Classification Reason"),
        )
        // Security Source Date
        .with_field(
            FieldDefinition::new("TSSRDT", FieldType::String)
                .with_size(SizeSpec::Fixed(8))
                .with_doc("Text Security Source Date"),
        )
        // Security Control Number
        .with_field(
            FieldDefinition::new("TSCTLN", FieldType::String)
                .with_size(SizeSpec::Fixed(15))
                .with_doc("Text Security Control Number"),
        )
        // Encryption - must be "0" (not encrypted)
        .with_field(
            FieldDefinition::new("ENCRYP", FieldType::String)
                .with_size(SizeSpec::Fixed(1))
                .with_doc("Encryption"),
        )
        // Text Format (STA, MTF, UT1, U8S)
        .with_field(
            FieldDefinition::new("TXTFMT", FieldType::String)
                .with_size(SizeSpec::Fixed(3))
                .with_doc("Text Format"),
        )
        // Text Extended Subheader Data Length
        // Value 00000 = no TREs, 00003-99999 = length of TXSOFL + TXSHD
        .with_field(
            FieldDefinition::new("TXSHDL", FieldType::String)
                .with_size(SizeSpec::Fixed(5))
                .with_doc("Text Extended Subheader Data Length"),
        )
        // Text Extended Subheader Overflow (conditional: present when TXSHDL > 0)
        // Value 000 = no overflow, 001-999 = DES sequence number for overflow
        .with_field(
            FieldDefinition::new("TXSOFL", FieldType::String)
                .with_size(SizeSpec::Fixed(3))
                .with_condition(txsofl_condition)
                .with_doc("Text Extended Subheader Overflow"),
        )
        // Text Extended Subheader Data (conditional: present when TXSHDL > 3)
        // Contains TRE data, length = TXSHDL - 3
        .with_field(
            FieldDefinition::new("TXSHD", FieldType::Bytes)
                .with_size(SizeSpec::Expression(
                    ExpressionEvaluator::parse("TXSHDL.to_i - 3").unwrap(),
                ))
                .with_condition(txshd_condition)
                .with_doc("Text Extended Subheader Data"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{StructureAccessor, StructureRegistry};

    /// Calculate the expected size of a text subheader without TRE data.
    /// TE(2) + TEXTID(7) + TXTALVL(3) + TXTDT(14) + TXTITL(80) + Security(167) + ENCRYP(1) + TXTFMT(3) + TXSHDL(5) = 282
    const TEXT_SUBHEADER_BASE_SIZE: usize = 282;

    /// Create test text subheader bytes with no TRE data.
    fn create_test_subheader_bytes(
        te: &str,
        textid: &str,
        txtalvl: &str,
        txtdt: &str,
        txtitl: &str,
        encryp: &str,
        txtfmt: &str,
        txshdl: &str,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();

        // TE (2)
        bytes.extend_from_slice(format!("{:2}", te).as_bytes());
        // TEXTID (7)
        bytes.extend_from_slice(format!("{:7}", textid).as_bytes());
        // TXTALVL (3)
        bytes.extend_from_slice(format!("{:03}", txtalvl).as_bytes());
        // TXTDT (14)
        bytes.extend_from_slice(format!("{:14}", txtdt).as_bytes());
        // TXTITL (80)
        bytes.extend_from_slice(format!("{:80}", txtitl).as_bytes());

        // Security fields (167 bytes total)
        bytes.extend_from_slice(b"U");                    // TSCLAS (1)
        bytes.extend_from_slice(b"  ");                   // TSCLSY (2)
        bytes.extend_from_slice(b"           ");          // TSCODE (11)
        bytes.extend_from_slice(b"  ");                   // TSCTLH (2)
        bytes.extend_from_slice(b"                    "); // TSREL (20)
        bytes.extend_from_slice(b"  ");                   // TSDCTP (2)
        bytes.extend_from_slice(b"        ");             // TSDCDT (8)
        bytes.extend_from_slice(b"    ");                 // TSDCXM (4)
        bytes.extend_from_slice(b" ");                    // TSDG (1)
        bytes.extend_from_slice(b"        ");             // TSDGDT (8)
        bytes.extend_from_slice(b"                                           "); // TSCLTX (43)
        bytes.extend_from_slice(b" ");                    // TSCATP (1)
        bytes.extend_from_slice(b"                                        "); // TSCAUT (40)
        bytes.extend_from_slice(b" ");                    // TSCRSN (1)
        bytes.extend_from_slice(b"        ");             // TSSRDT (8)
        bytes.extend_from_slice(b"               ");      // TSCTLN (15)

        // ENCRYP (1)
        bytes.extend_from_slice(format!("{:1}", encryp).as_bytes());
        // TXTFMT (3)
        bytes.extend_from_slice(format!("{:3}", txtfmt).as_bytes());
        // TXSHDL (5)
        bytes.extend_from_slice(format!("{:05}", txshdl).as_bytes());

        bytes
    }

    fn create_test_registry() -> StructureRegistry {
        let mut registry = StructureRegistry::new();
        registry.register("nitf_02.10_text_subheader", create_text_subheader_definition());
        registry
    }

    #[test]
    fn definition_has_correct_base_size() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test Text Title",
            "0",
            "STA",
            "00000",
        );

        assert_eq!(bytes.len(), TEXT_SUBHEADER_BASE_SIZE);

        let definition = registry.get("nitf_02.10_text_subheader").unwrap();
        let accessor = StructureAccessor::new(definition, &bytes).unwrap();

        // Verify we can access all fields
        assert_eq!(accessor.get("TE").unwrap().as_str().unwrap(), "TE");
        assert_eq!(accessor.get("TEXTID").unwrap().as_str().unwrap(), "TEXT001");
        assert_eq!(accessor.get("TXTALVL").unwrap().as_str().unwrap(), "000");
        assert_eq!(accessor.get("TXTDT").unwrap().as_str().unwrap(), "20240101120000");
        assert_eq!(accessor.get("TXTFMT").unwrap().as_str().unwrap(), "STA");
        assert_eq!(accessor.get("TXSHDL").unwrap().as_str().unwrap(), "00000");
    }

    #[test]
    fn definition_parses_all_txtfmt_values() {
        let registry = create_test_registry();

        for txtfmt in &["STA", "MTF", "UT1", "U8S"] {
            let bytes = create_test_subheader_bytes(
                "TE",
                "TEXT001",
                "000",
                "20240101120000",
                "Test",
                "0",
                txtfmt,
                "00000",
            );

            let definition = registry.get("nitf_02.10_text_subheader").unwrap();
            let accessor = StructureAccessor::new(definition, &bytes).unwrap();
            assert_eq!(accessor.get("TXTFMT").unwrap().as_str().unwrap(), *txtfmt);
        }
    }

    #[test]
    fn definition_parses_unknown_txtfmt() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "XYZ",  // Unknown format code
            "00000",
        );

        let definition = registry.get("nitf_02.10_text_subheader").unwrap();
        let accessor = StructureAccessor::new(definition, &bytes).unwrap();
        // Should still parse successfully
        assert_eq!(accessor.get("TXTFMT").unwrap().as_str().unwrap(), "XYZ");
    }

    #[test]
    fn definition_conditional_txsofl_present_when_txshdl_nonzero() {
        let registry = create_test_registry();
        let mut bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00003",  // TXSHDL > 0, so TXSOFL should be present
        );
        // Add TXSOFL (3 bytes)
        bytes.extend_from_slice(b"000");

        let definition = registry.get("nitf_02.10_text_subheader").unwrap();
        let accessor = StructureAccessor::new(definition, &bytes).unwrap();
        assert_eq!(accessor.get("TXSOFL").unwrap().as_str().unwrap(), "000");
    }

    #[test]
    fn definition_conditional_txshd_present_when_txshdl_gt_3() {
        let registry = create_test_registry();
        let mut bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00008",  // TXSHDL = 8, so TXSHD should be 5 bytes
        );
        // Add TXSOFL (3 bytes)
        bytes.extend_from_slice(b"000");
        // Add TXSHD (5 bytes = TXSHDL - 3)
        bytes.extend_from_slice(b"HELLO");

        let definition = registry.get("nitf_02.10_text_subheader").unwrap();
        let accessor = StructureAccessor::new(definition, &bytes).unwrap();
        assert_eq!(accessor.get("TXSOFL").unwrap().as_str().unwrap(), "000");
        assert_eq!(accessor.get("TXSHD").unwrap().as_bytes(), b"HELLO");
    }

    #[test]
    fn definition_txsofl_absent_when_txshdl_zero() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00000",  // TXSHDL = 0, so TXSOFL should be absent
        );

        let definition = registry.get("nitf_02.10_text_subheader").unwrap();
        let accessor = StructureAccessor::new(definition, &bytes).unwrap();
        // TXSOFL should not be present
        assert!(accessor.get("TXSOFL").is_err());
    }

    #[test]
    fn definition_parses_security_fields() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00000",
        );

        let definition = registry.get("nitf_02.10_text_subheader").unwrap();
        let accessor = StructureAccessor::new(definition, &bytes).unwrap();

        // Verify security fields are accessible
        assert_eq!(accessor.get("TSCLAS").unwrap().as_str().unwrap(), "U");
        assert_eq!(accessor.get("ENCRYP").unwrap().as_str().unwrap(), "0");
    }

    #[test]
    fn definition_parses_txtalvl_values() {
        let registry = create_test_registry();

        for txtalvl in &["000", "001", "500", "998"] {
            let bytes = create_test_subheader_bytes(
                "TE",
                "TEXT001",
                txtalvl,
                "20240101120000",
                "Test",
                "0",
                "STA",
                "00000",
            );

            let definition = registry.get("nitf_02.10_text_subheader").unwrap();
            let accessor = StructureAccessor::new(definition, &bytes).unwrap();
            assert_eq!(accessor.get("TXTALVL").unwrap().as_str().unwrap(), *txtalvl);
        }
    }
}
