//! Facade pattern for typed access to text subheader fields.
//!
//! The [`TextSubheaderFacade`] wraps a [`StructureAccessor`] to provide
//! convenient, typed access to text subheader fields. This pattern allows
//! the underlying structure definition to vary (e.g., NITF 2.0 vs 2.1)
//! while presenting a consistent API.
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::text::TextSubheaderFacade;
//! use osml_io::parser::StructureAccessor;
//!
//! let facade = TextSubheaderFacade::from_bytes(bytes, &registry, format)?;
//! let textid = facade.textid()?;
//! let txtalvl = facade.txtalvl()?;
//! let txtfmt = facade.txtfmt()?;
//! ```

use crate::error::CodecError;
use crate::jbp::types::NitfFormat;
use crate::parser::{StructureAccessor, StructureRegistry};

/// Facade providing typed access to text subheader fields via StructureAccessor.
///
/// This struct wraps a `StructureAccessor` and provides methods to access
/// text subheader fields with proper type conversion. The facade handles
/// the details of field naming and parsing, presenting a clean API for
/// accessing text metadata.
pub struct TextSubheaderFacade<'a> {
    /// The underlying structure accessor
    accessor: StructureAccessor<'a>,
}

impl<'a> TextSubheaderFacade<'a> {
    /// Create a facade from a StructureAccessor.
    ///
    /// # Arguments
    /// * `accessor` - The structure accessor for the text subheader
    ///
    /// # Returns
    /// A new `TextSubheaderFacade` wrapping the accessor.
    pub fn new(accessor: StructureAccessor<'a>) -> Self {
        Self { accessor }
    }

    /// Create from raw bytes using the appropriate structure definition.
    ///
    /// This constructor validates required field values:
    /// - TE must be "TE"
    /// - ENCRYP must be "0" (not encrypted)
    ///
    /// Note: Unknown TXTFMT values are allowed per JBP specification.
    ///
    /// # Arguments
    /// * `data` - Raw bytes of the text subheader
    /// * `registry` - Structure registry for looking up definitions
    /// * `format` - NITF format variant (determines which definition to use)
    ///
    /// # Returns
    /// A new `TextSubheaderFacade` or an error if parsing or validation fails.
    pub fn from_bytes(
        data: &'a [u8],
        registry: &StructureRegistry,
        format: NitfFormat,
    ) -> Result<Self, CodecError> {
        let def_name = format.text_subheader_definition();
        let definition = registry.get(def_name).ok_or_else(|| {
            CodecError::InvalidFormat(format!("Structure definition not found: {}", def_name))
        })?;

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
    /// - TE == "TE" (file part type marker)
    /// - ENCRYP == "0" (not encrypted)
    ///
    /// Note: Unknown TXTFMT values are allowed per JBP specification.
    fn validate(&self) -> Result<(), CodecError> {
        // Validate TE field
        let te = self.te()?;
        if te != "TE" {
            return Err(CodecError::Decode(format!(
                "Invalid text segment marker: expected 'TE', got '{}'",
                te
            )));
        }

        // Validate ENCRYP field
        let encryp = self.encryp()?;
        if encryp != "0" {
            return Err(CodecError::Decode(
                "Encrypted text not supported".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the underlying structure accessor.
    ///
    /// This is useful for accessing fields not exposed through the facade API.
    pub fn accessor(&self) -> &StructureAccessor<'a> {
        &self.accessor
    }

    // ========================================================================
    // Field Accessors
    // ========================================================================

    /// Get the file part type (TE field).
    ///
    /// This should always be "TE" for text segments.
    pub fn te(&self) -> Result<String, CodecError> {
        self.get_str_field("TE")
    }

    /// Get the text identifier (TEXTID field).
    ///
    /// A 7-character BCS-A identification code associated with the text item.
    pub fn textid(&self) -> Result<String, CodecError> {
        self.get_str_field("TEXTID")
    }

    /// Get the text attachment level (TXTALVL field).
    ///
    /// Returns the attachment level as a u32:
    /// - 000 = attached to overall file (unattached)
    /// - 001-998 = display level of the image or graphic this text attaches to
    pub fn txtalvl(&self) -> Result<u32, CodecError> {
        self.get_u32_field("TXTALVL")
    }

    /// Get the text date and time (TXTDT field).
    ///
    /// Returns the date/time string in CCYYMMDDhhmmss format.
    /// Unknown portions may contain "--" characters.
    pub fn txtdt(&self) -> Result<String, CodecError> {
        self.get_str_field("TXTDT")
    }

    /// Get the text title (TXTITL field).
    ///
    /// Returns the 80-character ECS-A title of the text item.
    pub fn txtitl(&self) -> Result<String, CodecError> {
        self.get_str_field("TXTITL")
    }

    /// Get the encryption field (ENCRYP field).
    ///
    /// This should always be "0" (not encrypted) for supported files.
    pub fn encryp(&self) -> Result<String, CodecError> {
        self.get_str_field("ENCRYP")
    }

    /// Get the text format (TXTFMT field).
    ///
    /// Returns the 3-character format code:
    /// - "STA" = Standard BCS (ASCII) text
    /// - "MTF" = Message Text Formatting (STANAG 5500/MIL-STD-6040)
    /// - "UT1" = Legacy ECS text formatting
    /// - "U8S" = UTF-8 text formatting
    ///
    /// Unknown format codes are allowed and returned as-is.
    pub fn txtfmt(&self) -> Result<String, CodecError> {
        self.get_str_field("TXTFMT")
    }

    /// Get the extended subheader data length (TXSHDL field).
    ///
    /// Returns the length as a u32:
    /// - 00000 = no TRE data
    /// - 00003-99999 = total length of TXSOFL + TXSHD fields
    pub fn txshdl(&self) -> Result<u32, CodecError> {
        self.get_u32_field("TXSHDL")
    }

    /// Get the extended subheader overflow (TXSOFL field).
    ///
    /// This field is only present when TXSHDL > 0.
    ///
    /// Returns the overflow indicator as a u32:
    /// - 000 = no overflow to DES
    /// - 001-999 = DES sequence number containing overflow TREs
    pub fn txsofl(&self) -> Result<u32, CodecError> {
        self.get_u32_field("TXSOFL")
    }

    /// Get the extended subheader data (TXSHD field) as raw bytes.
    ///
    /// This field is only present when TXSHDL > 3.
    /// Contains TRE envelope data.
    pub fn txshd(&self) -> Result<Vec<u8>, CodecError> {
        let value = self
            .accessor
            .get("TXSHD")
            .map_err(|e| CodecError::Parse(format!("Failed to get TXSHD: {}", e)))?;
        Ok(value.as_bytes().to_vec())
    }

    // ========================================================================
    // Security Field Accessors
    // ========================================================================

    /// Get the security classification (TSCLAS field).
    pub fn tsclas(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCLAS")
    }

    /// Get the security classification system (TSCLSY field).
    pub fn tsclsy(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCLSY")
    }

    /// Get the codewords (TSCODE field).
    pub fn tscode(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCODE")
    }

    /// Get the control and handling (TSCTLH field).
    pub fn tsctlh(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCTLH")
    }

    /// Get the releasing instructions (TSREL field).
    pub fn tsrel(&self) -> Result<String, CodecError> {
        self.get_str_field("TSREL")
    }

    /// Get the declassification type (TSDCTP field).
    pub fn tsdctp(&self) -> Result<String, CodecError> {
        self.get_str_field("TSDCTP")
    }

    /// Get the declassification date (TSDCDT field).
    pub fn tsdcdt(&self) -> Result<String, CodecError> {
        self.get_str_field("TSDCDT")
    }

    /// Get the declassification exemption (TSDCXM field).
    pub fn tsdcxm(&self) -> Result<String, CodecError> {
        self.get_str_field("TSDCXM")
    }

    /// Get the downgrade (TSDG field).
    pub fn tsdg(&self) -> Result<String, CodecError> {
        self.get_str_field("TSDG")
    }

    /// Get the downgrade date (TSDGDT field).
    pub fn tsdgdt(&self) -> Result<String, CodecError> {
        self.get_str_field("TSDGDT")
    }

    /// Get the classification text (TSCLTX field).
    pub fn tscltx(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCLTX")
    }

    /// Get the classification authority type (TSCATP field).
    pub fn tscatp(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCATP")
    }

    /// Get the classification authority (TSCAUT field).
    pub fn tscaut(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCAUT")
    }

    /// Get the classification reason (TSCRSN field).
    pub fn tscrsn(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCRSN")
    }

    /// Get the security source date (TSSRDT field).
    pub fn tssrdt(&self) -> Result<String, CodecError> {
        self.get_str_field("TSSRDT")
    }

    /// Get the security control number (TSCTLN field).
    pub fn tsctln(&self) -> Result<String, CodecError> {
        self.get_str_field("TSCTLN")
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get a string field value, trimming trailing spaces.
    fn get_str_field(&self, field: &str) -> Result<String, CodecError> {
        let value = self
            .accessor
            .get(field)
            .map_err(|e| CodecError::Parse(format!("Failed to get {}: {}", field, e)))?;
        let s = value.as_str().map_err(|e| {
            CodecError::Parse(format!("Failed to convert {} to string: {}", field, e))
        })?;
        Ok(s.trim_end().to_string())
    }

    /// Get a numeric field value as u32.
    fn get_u32_field(&self, field: &str) -> Result<u32, CodecError> {
        let value = self.get_str_field(field)?;
        value
            .parse::<u32>()
            .map_err(|e| CodecError::Parse(format!("Failed to parse {} as u32: {}", field, e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Calculate the expected size of a text subheader without TRE data.
    const TEXT_SUBHEADER_BASE_SIZE: usize = 282;

    /// Create test text subheader bytes.
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
        bytes.extend_from_slice(b"U"); // TSCLAS (1)
        bytes.extend_from_slice(b"  "); // TSCLSY (2)
        bytes.extend_from_slice(b"           "); // TSCODE (11)
        bytes.extend_from_slice(b"  "); // TSCTLH (2)
        bytes.extend_from_slice(b"                    "); // TSREL (20)
        bytes.extend_from_slice(b"  "); // TSDCTP (2)
        bytes.extend_from_slice(b"        "); // TSDCDT (8)
        bytes.extend_from_slice(b"    "); // TSDCXM (4)
        bytes.extend_from_slice(b" "); // TSDG (1)
        bytes.extend_from_slice(b"        "); // TSDGDT (8)
        bytes.extend_from_slice(b"                                           "); // TSCLTX (43)
        bytes.extend_from_slice(b" "); // TSCATP (1)
        bytes.extend_from_slice(b"                                        "); // TSCAUT (40)
        bytes.extend_from_slice(b" "); // TSCRSN (1)
        bytes.extend_from_slice(b"        "); // TSSRDT (8)
        bytes.extend_from_slice(b"               "); // TSCTLN (15)

        // ENCRYP (1)
        bytes.extend_from_slice(format!("{:1}", encryp).as_bytes());
        // TXTFMT (3)
        bytes.extend_from_slice(format!("{:3}", txtfmt).as_bytes());
        // TXSHDL (5)
        bytes.extend_from_slice(format!("{:05}", txshdl).as_bytes());

        bytes
    }

    /// Create a registry that loads definitions from KSY files.
    fn create_test_registry() -> StructureRegistry {
        StructureRegistry::new()
    }

    #[test]
    fn facade_basic_fields() {
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

        let facade =
            TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();

        assert_eq!(facade.te().unwrap(), "TE");
        assert_eq!(facade.textid().unwrap(), "TEXT001");
        assert_eq!(facade.txtalvl().unwrap(), 0);
        assert_eq!(facade.txtdt().unwrap(), "20240101120000");
        assert_eq!(facade.txtitl().unwrap(), "Test Text Title");
        assert_eq!(facade.encryp().unwrap(), "0");
        assert_eq!(facade.txtfmt().unwrap(), "STA");
        assert_eq!(facade.txshdl().unwrap(), 0);
    }

    #[test]
    fn facade_attachment_levels() {
        let registry = create_test_registry();

        for (txtalvl_str, expected) in &[("000", 0u32), ("001", 1), ("500", 500), ("998", 998)] {
            let bytes = create_test_subheader_bytes(
                "TE",
                "TEXT001",
                txtalvl_str,
                "20240101120000",
                "Test",
                "0",
                "STA",
                "00000",
            );

            let facade =
                TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
            assert_eq!(facade.txtalvl().unwrap(), *expected);
        }
    }

    #[test]
    fn facade_txtfmt_values() {
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

            let facade =
                TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
            assert_eq!(facade.txtfmt().unwrap(), *txtfmt);
        }
    }

    #[test]
    fn facade_unknown_txtfmt_allowed() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "XYZ", // Unknown format code
            "00000",
        );

        // Should succeed - unknown TXTFMT values are allowed
        let facade =
            TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.txtfmt().unwrap(), "XYZ");
    }

    #[test]
    fn facade_invalid_te_marker() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "XX", // Invalid TE marker
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00000",
        );

        let result = TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
        match result {
            Err(err) => assert!(err.to_string().contains("Invalid text segment marker")),
            Ok(_) => panic!("Expected error for invalid TE marker"),
        }
    }

    #[test]
    fn facade_encrypted_text_rejected() {
        let registry = create_test_registry();
        let bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "1", // Encrypted
            "STA",
            "00000",
        );

        let result = TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21);
        match result {
            Err(err) => assert!(err.to_string().contains("Encrypted text not supported")),
            Ok(_) => panic!("Expected error for encrypted text"),
        }
    }

    #[test]
    fn facade_txshdl_zero() {
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

        let facade =
            TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.txshdl().unwrap(), 0);
        // TXSOFL should not be accessible when TXSHDL is 0
        assert!(facade.txsofl().is_err());
    }

    #[test]
    fn facade_txsofl_present() {
        let registry = create_test_registry();
        let mut bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00003", // TXSHDL > 0
        );
        // Add TXSOFL (3 bytes)
        bytes.extend_from_slice(b"000");

        let facade =
            TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.txshdl().unwrap(), 3);
        assert_eq!(facade.txsofl().unwrap(), 0);
    }

    #[test]
    fn facade_txshd_present() {
        let registry = create_test_registry();
        let mut bytes = create_test_subheader_bytes(
            "TE",
            "TEXT001",
            "000",
            "20240101120000",
            "Test",
            "0",
            "STA",
            "00008", // TXSHDL = 8, so TXSHD = 5 bytes
        );
        // Add TXSOFL (3 bytes)
        bytes.extend_from_slice(b"000");
        // Add TXSHD (5 bytes)
        bytes.extend_from_slice(b"HELLO");

        let facade =
            TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.txshdl().unwrap(), 8);
        assert_eq!(facade.txsofl().unwrap(), 0);
        assert_eq!(facade.txshd().unwrap(), b"HELLO");
    }

    #[test]
    fn facade_security_fields() {
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

        let facade =
            TextSubheaderFacade::from_bytes(&bytes, &registry, NitfFormat::Nitf21).unwrap();
        assert_eq!(facade.tsclas().unwrap(), "U");
        assert_eq!(facade.tsclsy().unwrap(), ""); // Trimmed spaces
        assert_eq!(facade.tscode().unwrap(), "");
        assert_eq!(facade.tsctlh().unwrap(), "");
        assert_eq!(facade.tsrel().unwrap(), "");
    }
}
