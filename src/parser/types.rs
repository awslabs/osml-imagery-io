//! Core type definitions for the data-driven binary parser.
//!
//! This module contains the fundamental types used to represent structure
//! definitions parsed from KSY YAML files.

use std::collections::HashMap;

use super::expression::Expression;

/// A complete structure definition parsed from a KSY file.
#[derive(Debug, Clone)]
pub struct StructureDefinition {
    /// Unique identifier for this structure
    pub id: String,
    /// Human-readable title
    pub title: Option<String>,
    /// Default byte order (big or little endian)
    pub endian: Endian,
    /// Ordered sequence of field definitions
    pub fields: Vec<FieldDefinition>,
    /// Named nested type definitions
    pub types: HashMap<String, StructureDefinition>,
    /// Enumeration definitions
    pub enums: HashMap<String, EnumDefinition>,
}

impl StructureDefinition {
    /// Create a new structure definition with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            endian: Endian::Big,
            fields: Vec::new(),
            types: HashMap::new(),
            enums: HashMap::new(),
        }
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the endianness.
    pub fn with_endian(mut self, endian: Endian) -> Self {
        self.endian = endian;
        self
    }

    /// Add a field definition.
    pub fn with_field(mut self, field: FieldDefinition) -> Self {
        self.fields.push(field);
        self
    }

    /// Add a nested type definition.
    pub fn with_type(mut self, name: impl Into<String>, def: StructureDefinition) -> Self {
        self.types.insert(name.into(), def);
        self
    }

    /// Add an enum definition.
    pub fn with_enum(mut self, name: impl Into<String>, def: EnumDefinition) -> Self {
        self.enums.insert(name.into(), def);
        self
    }
}

/// Definition of a single field in a structure.
#[derive(Debug, Clone)]
pub struct FieldDefinition {
    /// Field identifier (used in path access)
    pub id: String,
    /// Field type specification
    pub field_type: FieldType,
    /// Size in bytes (for strings/bytes) or expression
    pub size: SizeSpec,
    /// Character encoding for strings
    pub encoding: Option<Encoding>,
    /// Padding character for fixed-width fields
    pub pad: Option<u8>,
    /// Conditional expression (field present only if true)
    pub condition: Option<Expression>,
    /// Repetition specification
    pub repeat: Option<RepeatSpec>,
    /// Documentation string
    pub doc: Option<String>,
}

impl FieldDefinition {
    /// Create a new field definition with the given id and type.
    pub fn new(id: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            id: id.into(),
            field_type,
            size: SizeSpec::Fixed(0),
            encoding: None,
            pad: None,
            condition: None,
            repeat: None,
            doc: None,
        }
    }

    /// Set the size specification.
    pub fn with_size(mut self, size: SizeSpec) -> Self {
        self.size = size;
        self
    }

    /// Set the encoding.
    pub fn with_encoding(mut self, encoding: Encoding) -> Self {
        self.encoding = Some(encoding);
        self
    }

    /// Set the padding character.
    pub fn with_pad(mut self, pad: u8) -> Self {
        self.pad = Some(pad);
        self
    }

    /// Set the condition expression.
    pub fn with_condition(mut self, condition: Expression) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Set the repetition specification.
    pub fn with_repeat(mut self, repeat: RepeatSpec) -> Self {
        self.repeat = Some(repeat);
        self
    }

    /// Set the documentation string.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }
}

/// Supported field types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// Fixed-size string with encoding
    String,
    /// Raw byte array
    Bytes,
    /// Unsigned integer (1, 2, or 4 bytes)
    UnsignedInt(u8),
    /// Signed integer (1, 2, or 4 bytes)
    SignedInt(u8),
    /// Reference to a nested type
    TypeRef(String),
}

impl FieldType {
    /// Create an unsigned 8-bit integer type (u1).
    pub fn u1() -> Self {
        Self::UnsignedInt(1)
    }

    /// Create an unsigned 16-bit integer type (u2).
    pub fn u2() -> Self {
        Self::UnsignedInt(2)
    }

    /// Create an unsigned 32-bit integer type (u4).
    pub fn u4() -> Self {
        Self::UnsignedInt(4)
    }

    /// Create a signed 8-bit integer type (s1).
    pub fn s1() -> Self {
        Self::SignedInt(1)
    }

    /// Create a signed 16-bit integer type (s2).
    pub fn s2() -> Self {
        Self::SignedInt(2)
    }

    /// Create a signed 32-bit integer type (s4).
    pub fn s4() -> Self {
        Self::SignedInt(4)
    }
}

/// Character encodings for NITF fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// ASCII (default)
    Ascii,
    /// NITF Basic Character Set - Alphanumeric (ASCII 0x20-0x7E)
    BcsA,
    /// NITF Basic Character Set - Numeric (digits 0-9 and space)
    BcsN,
    /// NITF Extended Character Set - Alphanumeric
    EcsA,
}

impl Encoding {
    /// Get the default padding character for this encoding.
    pub fn default_pad(&self) -> u8 {
        match self {
            Encoding::Ascii | Encoding::BcsA | Encoding::EcsA => 0x20, // space
            Encoding::BcsN => 0x30,                                    // '0'
        }
    }

    /// Validate that a byte is valid for this encoding.
    pub fn is_valid_byte(&self, byte: u8) -> bool {
        match self {
            Encoding::Ascii => byte.is_ascii(),
            Encoding::BcsA => (0x20..=0x7E).contains(&byte),
            Encoding::BcsN => (0x30..=0x39).contains(&byte) || byte == 0x20,
            Encoding::EcsA => byte >= 0x20, // Extended allows broader range
        }
    }

    /// Validate that all bytes are valid for this encoding.
    pub fn validate(&self, data: &[u8]) -> bool {
        data.iter().all(|&b| self.is_valid_byte(b))
    }
}

/// Byte order (endianness).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Endian {
    /// Big-endian (most significant byte first) - default for NITF
    #[default]
    Big,
    /// Little-endian (least significant byte first)
    Little,
}

/// Size specification (fixed or expression-based).
#[derive(Debug, Clone)]
pub enum SizeSpec {
    /// Fixed size in bytes
    Fixed(usize),
    /// Size determined by expression
    Expression(Expression),
}

impl SizeSpec {
    /// Create a fixed size specification.
    pub fn fixed(size: usize) -> Self {
        Self::Fixed(size)
    }

    /// Create an expression-based size specification.
    pub fn expr(expr: Expression) -> Self {
        Self::Expression(expr)
    }

    /// Get the fixed size if this is a fixed specification.
    pub fn as_fixed(&self) -> Option<usize> {
        match self {
            SizeSpec::Fixed(size) => Some(*size),
            SizeSpec::Expression(_) => None,
        }
    }
}

/// Repetition specification.
#[derive(Debug, Clone)]
pub enum RepeatSpec {
    /// Repeat a fixed number of times
    Count(usize),
    /// Repeat based on expression result
    Expression(Expression),
    /// Repeat until condition is true
    Until(Expression),
    /// Repeat until end of stream
    Eos,
}

impl RepeatSpec {
    /// Create a fixed count repetition.
    pub fn count(n: usize) -> Self {
        Self::Count(n)
    }

    /// Create an expression-based repetition.
    pub fn expr(expr: Expression) -> Self {
        Self::Expression(expr)
    }

    /// Create an until-condition repetition.
    pub fn until(condition: Expression) -> Self {
        Self::Until(condition)
    }

    /// Create an end-of-stream repetition.
    pub fn eos() -> Self {
        Self::Eos
    }
}

/// Enumeration definition mapping integer values to names.
#[derive(Debug, Clone, Default)]
pub struct EnumDefinition {
    /// Mapping from integer value to name
    pub values: HashMap<i64, String>,
}

impl EnumDefinition {
    /// Create a new empty enum definition.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Add a value mapping.
    pub fn with_value(mut self, value: i64, name: impl Into<String>) -> Self {
        self.values.insert(value, name.into());
        self
    }

    /// Get the name for a value.
    pub fn get_name(&self, value: i64) -> Option<&str> {
        self.values.get(&value).map(|s| s.as_str())
    }

    /// Get the value for a name.
    pub fn get_value(&self, name: &str) -> Option<i64> {
        self.values
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(v, _)| *v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_definition_builder() {
        let def = StructureDefinition::new("test_struct")
            .with_title("Test Structure")
            .with_endian(Endian::Little)
            .with_field(FieldDefinition::new("field1", FieldType::String).with_size(SizeSpec::fixed(10)));

        assert_eq!(def.id, "test_struct");
        assert_eq!(def.title, Some("Test Structure".to_string()));
        assert_eq!(def.endian, Endian::Little);
        assert_eq!(def.fields.len(), 1);
        assert_eq!(def.fields[0].id, "field1");
    }

    #[test]
    fn field_definition_builder() {
        let field = FieldDefinition::new("test_field", FieldType::String)
            .with_size(SizeSpec::fixed(20))
            .with_encoding(Encoding::BcsA)
            .with_pad(0x20)
            .with_doc("A test field");

        assert_eq!(field.id, "test_field");
        assert_eq!(field.field_type, FieldType::String);
        assert!(matches!(field.size, SizeSpec::Fixed(20)));
        assert_eq!(field.encoding, Some(Encoding::BcsA));
        assert_eq!(field.pad, Some(0x20));
        assert_eq!(field.doc, Some("A test field".to_string()));
    }

    #[test]
    fn field_type_integer_constructors() {
        assert_eq!(FieldType::u1(), FieldType::UnsignedInt(1));
        assert_eq!(FieldType::u2(), FieldType::UnsignedInt(2));
        assert_eq!(FieldType::u4(), FieldType::UnsignedInt(4));
        assert_eq!(FieldType::s1(), FieldType::SignedInt(1));
        assert_eq!(FieldType::s2(), FieldType::SignedInt(2));
        assert_eq!(FieldType::s4(), FieldType::SignedInt(4));
    }

    #[test]
    fn encoding_default_pad() {
        assert_eq!(Encoding::Ascii.default_pad(), 0x20);
        assert_eq!(Encoding::BcsA.default_pad(), 0x20);
        assert_eq!(Encoding::BcsN.default_pad(), 0x30);
        assert_eq!(Encoding::EcsA.default_pad(), 0x20);
    }

    #[test]
    fn encoding_bcs_a_validation() {
        // Valid BCS-A: ASCII 0x20-0x7E
        assert!(Encoding::BcsA.is_valid_byte(0x20)); // space
        assert!(Encoding::BcsA.is_valid_byte(0x41)); // 'A'
        assert!(Encoding::BcsA.is_valid_byte(0x7E)); // '~'

        // Invalid BCS-A
        assert!(!Encoding::BcsA.is_valid_byte(0x1F)); // below range
        assert!(!Encoding::BcsA.is_valid_byte(0x7F)); // DEL
        assert!(!Encoding::BcsA.is_valid_byte(0x00)); // NUL
    }

    #[test]
    fn encoding_bcs_n_validation() {
        // Valid BCS-N: digits 0-9 and space
        assert!(Encoding::BcsN.is_valid_byte(0x30)); // '0'
        assert!(Encoding::BcsN.is_valid_byte(0x35)); // '5'
        assert!(Encoding::BcsN.is_valid_byte(0x39)); // '9'
        assert!(Encoding::BcsN.is_valid_byte(0x20)); // space

        // Invalid BCS-N
        assert!(!Encoding::BcsN.is_valid_byte(0x41)); // 'A'
        assert!(!Encoding::BcsN.is_valid_byte(0x2F)); // '/'
        assert!(!Encoding::BcsN.is_valid_byte(0x3A)); // ':'
    }

    #[test]
    fn encoding_validate_slice() {
        assert!(Encoding::BcsA.validate(b"Hello World"));
        assert!(!Encoding::BcsA.validate(b"Hello\x00World"));

        assert!(Encoding::BcsN.validate(b"12345"));
        assert!(Encoding::BcsN.validate(b"  123"));
        assert!(!Encoding::BcsN.validate(b"12A45"));
    }

    #[test]
    fn endian_default() {
        assert_eq!(Endian::default(), Endian::Big);
    }

    #[test]
    fn size_spec_fixed() {
        let size = SizeSpec::fixed(100);
        assert_eq!(size.as_fixed(), Some(100));
    }

    #[test]
    fn repeat_spec_constructors() {
        let count = RepeatSpec::count(5);
        assert!(matches!(count, RepeatSpec::Count(5)));

        let eos = RepeatSpec::eos();
        assert!(matches!(eos, RepeatSpec::Eos));
    }

    #[test]
    fn enum_definition_builder() {
        let enum_def = EnumDefinition::new()
            .with_value(0, "NONE")
            .with_value(1, "FIRST")
            .with_value(2, "SECOND");

        assert_eq!(enum_def.get_name(0), Some("NONE"));
        assert_eq!(enum_def.get_name(1), Some("FIRST"));
        assert_eq!(enum_def.get_name(2), Some("SECOND"));
        assert_eq!(enum_def.get_name(3), None);

        assert_eq!(enum_def.get_value("NONE"), Some(0));
        assert_eq!(enum_def.get_value("FIRST"), Some(1));
        assert_eq!(enum_def.get_value("UNKNOWN"), None);
    }
}
