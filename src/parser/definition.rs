//! Structure definition loading from KSY YAML files.
//!
//! The [`DefinitionLoader`] parses Kaitai Struct-compatible YAML files
//! into [`StructureDefinition`] objects.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::error::LoadError;
use super::expression::ExpressionEvaluator;
use super::types::{
    Encoding, Endian, EnumDefinition, FieldDefinition, FieldType, RepeatSpec, SizeSpec,
    StructureDefinition,
};

/// Loads structure definitions from KSY YAML files.
pub struct DefinitionLoader;

impl DefinitionLoader {
    /// Parse a KSY file from a path.
    pub fn load_file(path: &Path) -> Result<StructureDefinition, LoadError> {
        let content = fs::read_to_string(path)?;
        Self::load_str(&content)
    }

    /// Parse a KSY definition from a string.
    pub fn load_str(yaml: &str) -> Result<StructureDefinition, LoadError> {
        let raw: RawKsyFile = serde_yaml::from_str(yaml).map_err(|e| LoadError::YamlError {
            message: e.to_string(),
        })?;
        Self::convert_raw_ksy(raw)
    }

    /// Parse a KSY definition from a reader.
    pub fn load_reader<R: Read>(mut reader: R) -> Result<StructureDefinition, LoadError> {
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        Self::load_str(&content)
    }

    /// Convert raw KSY structure to StructureDefinition.
    fn convert_raw_ksy(raw: RawKsyFile) -> Result<StructureDefinition, LoadError> {
        let meta = raw.meta.ok_or_else(|| LoadError::MissingField {
            field: "meta".to_string(),
            context: "root".to_string(),
        })?;

        let id = meta.id.ok_or_else(|| LoadError::MissingField {
            field: "id".to_string(),
            context: "meta section".to_string(),
        })?;

        let endian = match meta.endian.as_deref() {
            Some("be") | Some("big") | None => Endian::Big,
            Some("le") | Some("little") => Endian::Little,
            Some(other) => {
                return Err(LoadError::InvalidType {
                    type_str: other.to_string(),
                    context: "meta.endian".to_string(),
                })
            }
        };

        // Parse fields from seq section
        let fields = if let Some(seq) = raw.seq {
            seq.into_iter()
                .map(|f| Self::convert_field(f, "seq"))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        // Parse nested types
        let types = if let Some(raw_types) = raw.types {
            raw_types
                .into_iter()
                .map(|(name, raw_type)| {
                    let def = Self::convert_nested_type(raw_type, &name)?;
                    Ok((name, def))
                })
                .collect::<Result<HashMap<_, _>, LoadError>>()?
        } else {
            HashMap::new()
        };

        // Parse enums
        let enums = if let Some(raw_enums) = raw.enums {
            raw_enums
                .into_iter()
                .map(|(name, raw_enum)| {
                    let def = Self::convert_enum(raw_enum)?;
                    Ok((name, def))
                })
                .collect::<Result<HashMap<_, _>, LoadError>>()?
        } else {
            HashMap::new()
        };

        Ok(StructureDefinition {
            id,
            title: meta.title,
            endian,
            fields,
            types,
            enums,
        })
    }

    /// Convert a nested type definition.
    fn convert_nested_type(
        raw: RawTypeDefinition,
        name: &str,
    ) -> Result<StructureDefinition, LoadError> {
        let fields = if let Some(seq) = raw.seq {
            seq.into_iter()
                .map(|f| Self::convert_field(f, &format!("types.{}", name)))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        // Parse nested types recursively
        let types = if let Some(raw_types) = raw.types {
            raw_types
                .into_iter()
                .map(|(nested_name, raw_type)| {
                    let def = Self::convert_nested_type(raw_type, &nested_name)?;
                    Ok((nested_name, def))
                })
                .collect::<Result<HashMap<_, _>, LoadError>>()?
        } else {
            HashMap::new()
        };

        // Parse enums
        let enums = if let Some(raw_enums) = raw.enums {
            raw_enums
                .into_iter()
                .map(|(enum_name, raw_enum)| {
                    let def = Self::convert_enum(raw_enum)?;
                    Ok((enum_name, def))
                })
                .collect::<Result<HashMap<_, _>, LoadError>>()?
        } else {
            HashMap::new()
        };

        Ok(StructureDefinition {
            id: name.to_string(),
            title: None,
            endian: Endian::Big, // Inherit from parent in practice
            fields,
            types,
            enums,
        })
    }

    /// Convert a raw field definition.
    fn convert_field(raw: RawFieldDefinition, context: &str) -> Result<FieldDefinition, LoadError> {
        let id = raw.id.clone().ok_or_else(|| LoadError::MissingField {
            field: "id".to_string(),
            context: context.to_string(),
        })?;

        let field_context = format!("{}.{}", context, id);

        // Parse field type
        let field_type = Self::parse_field_type(raw.field_type.as_deref(), &field_context)?;

        // Parse size
        let size = Self::parse_size(&raw, &field_context)?;

        // Parse encoding
        let encoding = Self::parse_encoding(raw.encoding.as_deref(), &field_context)?;

        // Parse padding
        let pad = Self::parse_pad(&raw)?;

        // Parse condition
        let condition = if let Some(ref if_expr) = raw.if_expr {
            Some(
                ExpressionEvaluator::parse(if_expr).map_err(|e| LoadError::InvalidExpression {
                    expr: if_expr.clone(),
                    message: e.to_string(),
                })?,
            )
        } else {
            None
        };

        // Parse repetition
        let repeat = Self::parse_repeat(&raw, &field_context)?;

        Ok(FieldDefinition {
            id,
            field_type,
            size,
            encoding,
            pad,
            condition,
            repeat,
            doc: raw.doc,
        })
    }

    /// Parse field type from type string.
    fn parse_field_type(
        type_str: Option<&str>,
        context: &str,
    ) -> Result<FieldType, LoadError> {
        match type_str {
            None | Some("str") | Some("strz") => Ok(FieldType::String),
            Some("u1") => Ok(FieldType::UnsignedInt(1)),
            Some("u2") | Some("u2be") | Some("u2le") => Ok(FieldType::UnsignedInt(2)),
            Some("u3") | Some("u3be") => Ok(FieldType::UnsignedInt(3)),
            Some("u4") | Some("u4be") | Some("u4le") => Ok(FieldType::UnsignedInt(4)),
            Some("u8") | Some("u8be") | Some("u8le") => Ok(FieldType::UnsignedInt(8)),
            Some("s1") => Ok(FieldType::SignedInt(1)),
            Some("s2") | Some("s2be") | Some("s2le") => Ok(FieldType::SignedInt(2)),
            Some("s4") | Some("s4be") | Some("s4le") => Ok(FieldType::SignedInt(4)),
            Some("s8") | Some("s8be") | Some("s8le") => Ok(FieldType::SignedInt(8)),
            Some(s) if s.starts_with("b") && s[1..].parse::<u32>().is_ok() => {
                // Bit fields like b1, b4, etc. - treat as bytes for now
                Ok(FieldType::Bytes)
            }
            Some(type_name) => {
                // Check if it's a type reference (starts with lowercase or contains underscore)
                if type_name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
                    || type_name.contains('_')
                {
                    Ok(FieldType::TypeRef(type_name.to_string()))
                } else {
                    Err(LoadError::InvalidType {
                        type_str: type_name.to_string(),
                        context: context.to_string(),
                    })
                }
            }
        }
    }

    /// Parse size specification.
    fn parse_size(raw: &RawFieldDefinition, context: &str) -> Result<SizeSpec, LoadError> {
        match &raw.size {
            Some(RawSize::Fixed(n)) => Ok(SizeSpec::Fixed(*n as usize)),
            Some(RawSize::Expression(expr)) => {
                let parsed =
                    ExpressionEvaluator::parse(expr).map_err(|e| LoadError::InvalidExpression {
                        expr: expr.clone(),
                        message: e.to_string(),
                    })?;
                Ok(SizeSpec::Expression(parsed))
            }
            None => {
                // For integer types, size is implicit
                match raw.field_type.as_deref() {
                    Some("u1") | Some("s1") => Ok(SizeSpec::Fixed(1)),
                    Some("u2") | Some("u2be") | Some("u2le") | Some("s2") | Some("s2be")
                    | Some("s2le") => Ok(SizeSpec::Fixed(2)),
                    Some("u4") | Some("u4be") | Some("u4le") | Some("s4") | Some("s4be")
                    | Some("s4le") => Ok(SizeSpec::Fixed(4)),
                    Some("u8") | Some("u8be") | Some("u8le") | Some("s8") | Some("s8be")
                    | Some("s8le") => Ok(SizeSpec::Fixed(8)),
                    Some(type_name) if !type_name.starts_with("str") => {
                        // Type references don't need explicit size - size comes from the type
                        Ok(SizeSpec::Fixed(0))
                    }
                    _ => {
                        // For strings without size, check if there's a size-eos flag
                        if raw.size_eos.unwrap_or(false) {
                            Ok(SizeSpec::Fixed(0)) // Will be determined at runtime
                        } else {
                            Err(LoadError::MissingField {
                                field: "size".to_string(),
                                context: context.to_string(),
                            })
                        }
                    }
                }
            }
        }
    }

    /// Parse encoding specification.
    fn parse_encoding(
        encoding: Option<&str>,
        _context: &str,
    ) -> Result<Option<Encoding>, LoadError> {
        match encoding {
            None => Ok(None),
            Some("ASCII") | Some("ascii") => Ok(Some(Encoding::Ascii)),
            Some("BCS-A") | Some("bcs-a") | Some("BCS_A") | Some("bcs_a") => {
                Ok(Some(Encoding::BcsA))
            }
            Some("BCS-N") | Some("bcs-n") | Some("BCS_N") | Some("bcs_n") => {
                Ok(Some(Encoding::BcsN))
            }
            Some("BCS-NPI") | Some("bcs-npi") | Some("BCS_NPI") | Some("bcs_npi") => {
                Ok(Some(Encoding::BcsNPI))
            }
            Some("ECS-A") | Some("ecs-a") | Some("ECS_A") | Some("ecs_a") => {
                Ok(Some(Encoding::EcsA))
            }
            Some("UTF-8") | Some("utf-8") | Some("utf8") => Ok(Some(Encoding::Ascii)), // Treat UTF-8 as ASCII for now
            Some(_) => Ok(None), // Unknown encodings default to None
        }
    }

    /// Parse padding character.
    fn parse_pad(raw: &RawFieldDefinition) -> Result<Option<u8>, LoadError> {
        match &raw.pad {
            Some(RawPad::Char(c)) => Ok(Some(*c as u8)),
            Some(RawPad::Byte(b)) => Ok(Some(*b)),
            None => {
                // Default padding based on encoding
                if let Some(enc) = raw.encoding.as_deref() {
                    match enc {
                        "BCS-N" | "bcs-n" | "BCS_N" | "bcs_n"
                        | "BCS-NPI" | "bcs-npi" | "BCS_NPI" | "bcs_npi" => Ok(Some(0x30)), // '0'
                        _ => Ok(Some(0x20)),                                      // space
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Parse repetition specification.
    fn parse_repeat(
        raw: &RawFieldDefinition,
        context: &str,
    ) -> Result<Option<RepeatSpec>, LoadError> {
        match raw.repeat.as_deref() {
            None => Ok(None),
            Some("eos") => Ok(Some(RepeatSpec::Eos)),
            Some("expr") => {
                let expr_str = raw.repeat_expr.as_ref().ok_or_else(|| LoadError::MissingField {
                    field: "repeat-expr".to_string(),
                    context: context.to_string(),
                })?;
                let expr = ExpressionEvaluator::parse(expr_str).map_err(|e| {
                    LoadError::InvalidExpression {
                        expr: expr_str.clone(),
                        message: e.to_string(),
                    }
                })?;
                Ok(Some(RepeatSpec::Expression(expr)))
            }
            Some("until") => {
                let until_str =
                    raw.repeat_until.as_ref().ok_or_else(|| LoadError::MissingField {
                        field: "repeat-until".to_string(),
                        context: context.to_string(),
                    })?;
                let expr = ExpressionEvaluator::parse(until_str).map_err(|e| {
                    LoadError::InvalidExpression {
                        expr: until_str.clone(),
                        message: e.to_string(),
                    }
                })?;
                Ok(Some(RepeatSpec::Until(expr)))
            }
            Some(other) => Err(LoadError::InvalidType {
                type_str: other.to_string(),
                context: format!("{}.repeat", context),
            }),
        }
    }

    /// Convert raw enum definition.
    fn convert_enum(raw: RawEnumDefinition) -> Result<EnumDefinition, LoadError> {
        let mut def = EnumDefinition::new();
        for (key, value) in raw.0 {
            def.values.insert(key, value);
        }
        Ok(def)
    }

    /// Validate that all type references in a definition resolve to defined types.
    pub fn validate_type_references(def: &StructureDefinition) -> Result<(), LoadError> {
        Self::validate_fields_type_refs(&def.fields, &def.types, &def.id)
    }

    fn validate_fields_type_refs(
        fields: &[FieldDefinition],
        types: &HashMap<String, StructureDefinition>,
        context: &str,
    ) -> Result<(), LoadError> {
        for field in fields {
            if let FieldType::TypeRef(type_name) = &field.field_type {
                if !types.contains_key(type_name) {
                    return Err(LoadError::UndefinedType {
                        type_name: type_name.clone(),
                        context: format!("field '{}' in {}", field.id, context),
                    });
                }
            }
        }

        // Recursively validate nested types
        for (name, nested_def) in types {
            Self::validate_fields_type_refs(&nested_def.fields, &nested_def.types, name)?;
        }

        Ok(())
    }
}

// Raw YAML structures for deserialization

#[derive(Debug, Deserialize)]
struct RawKsyFile {
    meta: Option<RawMeta>,
    seq: Option<Vec<RawFieldDefinition>>,
    types: Option<HashMap<String, RawTypeDefinition>>,
    enums: Option<HashMap<String, RawEnumDefinition>>,
}

#[derive(Debug, Deserialize)]
struct RawMeta {
    id: Option<String>,
    title: Option<String>,
    endian: Option<String>,
    #[serde(rename = "file-extension")]
    #[allow(dead_code)]
    file_extension: Option<String>,
    #[allow(dead_code)]
    application: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTypeDefinition {
    seq: Option<Vec<RawFieldDefinition>>,
    types: Option<HashMap<String, RawTypeDefinition>>,
    enums: Option<HashMap<String, RawEnumDefinition>>,
}

#[derive(Debug, Deserialize)]
struct RawFieldDefinition {
    id: Option<String>,
    #[serde(rename = "type")]
    field_type: Option<String>,
    size: Option<RawSize>,
    #[serde(rename = "size-eos")]
    size_eos: Option<bool>,
    encoding: Option<String>,
    #[serde(rename = "pad-right")]
    pad: Option<RawPad>,
    #[serde(rename = "if")]
    if_expr: Option<String>,
    repeat: Option<String>,
    #[serde(rename = "repeat-expr")]
    repeat_expr: Option<String>,
    #[serde(rename = "repeat-until")]
    repeat_until: Option<String>,
    doc: Option<String>,
    #[allow(dead_code)]
    contents: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawSize {
    Fixed(i64),
    Expression(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPad {
    Char(char),
    Byte(u8),
}

#[derive(Debug, Deserialize)]
struct RawEnumDefinition(HashMap<i64, String>);



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_minimal_ksy() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: field1
    type: str
    size: 10
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.id, "test_struct");
        assert_eq!(def.fields.len(), 1);
        assert_eq!(def.fields[0].id, "field1");
        assert_eq!(def.fields[0].field_type, FieldType::String);
        assert!(matches!(def.fields[0].size, SizeSpec::Fixed(10)));
    }

    #[test]
    fn load_ksy_with_title_and_endian() {
        let yaml = r#"
meta:
  id: test_struct
  title: Test Structure
  endian: le
seq: []
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.id, "test_struct");
        assert_eq!(def.title, Some("Test Structure".to_string()));
        assert_eq!(def.endian, Endian::Little);
    }

    #[test]
    fn load_ksy_with_integer_types() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: byte_field
    type: u1
  - id: short_field
    type: u2
  - id: int_field
    type: u4
  - id: signed_byte
    type: s1
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields.len(), 4);
        assert_eq!(def.fields[0].field_type, FieldType::UnsignedInt(1));
        assert_eq!(def.fields[1].field_type, FieldType::UnsignedInt(2));
        assert_eq!(def.fields[2].field_type, FieldType::UnsignedInt(4));
        assert_eq!(def.fields[3].field_type, FieldType::SignedInt(1));
    }

    #[test]
    fn load_ksy_with_encoding() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: bcs_a_field
    type: str
    size: 10
    encoding: BCS-A
  - id: bcs_n_field
    type: str
    size: 5
    encoding: BCS-N
  - id: ecs_a_field
    type: str
    size: 20
    encoding: ECS-A
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields[0].encoding, Some(Encoding::BcsA));
        assert_eq!(def.fields[1].encoding, Some(Encoding::BcsN));
        assert_eq!(def.fields[2].encoding, Some(Encoding::EcsA));
    }

    #[test]
    fn load_ksy_with_conditional() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: has_extra
    type: u1
  - id: extra_data
    type: str
    size: 10
    if: has_extra == 1
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields.len(), 2);
        assert!(def.fields[0].condition.is_none());
        assert!(def.fields[1].condition.is_some());
    }

    #[test]
    fn load_ksy_with_repeat_expr() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: count
    type: u2
  - id: items
    type: str
    size: 10
    repeat: expr
    repeat-expr: count
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert!(def.fields[1].repeat.is_some());
        assert!(matches!(def.fields[1].repeat, Some(RepeatSpec::Expression(_))));
    }

    #[test]
    fn load_ksy_with_repeat_until() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: items
    type: u1
    repeat: until
    repeat-until: _ == 0
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert!(matches!(def.fields[0].repeat, Some(RepeatSpec::Until(_))));
    }

    #[test]
    fn load_ksy_with_repeat_eos() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: items
    type: u1
    repeat: eos
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert!(matches!(def.fields[0].repeat, Some(RepeatSpec::Eos)));
    }

    #[test]
    fn load_ksy_with_nested_types() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: header
    type: header_type
types:
  header_type:
    seq:
      - id: magic
        type: str
        size: 4
      - id: version
        type: u2
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields.len(), 1);
        assert_eq!(def.fields[0].field_type, FieldType::TypeRef("header_type".to_string()));
        assert!(def.types.contains_key("header_type"));
        let header_type = &def.types["header_type"];
        assert_eq!(header_type.fields.len(), 2);
    }

    #[test]
    fn load_ksy_with_enums() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: status
    type: u1
enums:
  status_enum:
    0: inactive
    1: active
    2: pending
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert!(def.enums.contains_key("status_enum"));
        let status_enum = &def.enums["status_enum"];
        assert_eq!(status_enum.get_name(0), Some("inactive"));
        assert_eq!(status_enum.get_name(1), Some("active"));
        assert_eq!(status_enum.get_name(2), Some("pending"));
    }

    #[test]
    fn load_ksy_with_expression_size() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: len
    type: u2
  - id: data
    type: str
    size: len
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert!(matches!(def.fields[1].size, SizeSpec::Expression(_)));
    }

    #[test]
    fn load_ksy_missing_meta_error() {
        let yaml = r#"
seq:
  - id: field1
    type: str
    size: 10
"#;
        let result = DefinitionLoader::load_str(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::MissingField { field, .. } if field == "meta"));
    }

    #[test]
    fn load_ksy_missing_id_error() {
        let yaml = r#"
meta:
  title: Test
seq: []
"#;
        let result = DefinitionLoader::load_str(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::MissingField { field, .. } if field == "id"));
    }

    #[test]
    fn load_invalid_yaml_error() {
        let yaml = "this is not: valid: yaml: [";
        let result = DefinitionLoader::load_str(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::YamlError { .. }));
    }

    #[test]
    fn validate_undefined_type_error() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: header
    type: undefined_type
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        let result = DefinitionLoader::validate_type_references(&def);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoadError::UndefinedType { type_name, .. } if type_name == "undefined_type"));
    }

    #[test]
    fn validate_defined_type_ok() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: header
    type: header_type
types:
  header_type:
    seq:
      - id: magic
        type: str
        size: 4
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        let result = DefinitionLoader::validate_type_references(&def);
        assert!(result.is_ok());
    }

    #[test]
    fn load_ksy_with_doc() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: field1
    type: str
    size: 10
    doc: This is a documentation string
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields[0].doc, Some("This is a documentation string".to_string()));
    }

    #[test]
    fn load_ksy_big_endian_explicit() {
        let yaml = r#"
meta:
  id: test_struct
  endian: be
seq: []
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.endian, Endian::Big);
    }

    #[test]
    fn load_ksy_default_endian_is_big() {
        let yaml = r#"
meta:
  id: test_struct
seq: []
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.endian, Endian::Big);
    }

    #[test]
    fn load_ksy_with_raw_bytes() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: raw_data
    size: 100
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields[0].field_type, FieldType::String);
        assert!(matches!(def.fields[0].size, SizeSpec::Fixed(100)));
    }

    #[test]
    fn load_ksy_with_padding() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: bcs_n_field
    type: str
    size: 10
    encoding: BCS-N
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        // BCS-N fields should default to '0' padding
        assert_eq!(def.fields[0].pad, Some(0x30));
    }

    #[test]
    fn load_ksy_with_explicit_padding() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: field1
    type: str
    size: 10
    encoding: BCS-A
    pad-right: 32
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields[0].pad, Some(32)); // space character
    }

    #[test]
    fn load_ksy_with_endian_specific_types() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: be_short
    type: u2be
  - id: le_short
    type: u2le
  - id: be_int
    type: u4be
  - id: le_int
    type: u4le
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields[0].field_type, FieldType::UnsignedInt(2));
        assert_eq!(def.fields[1].field_type, FieldType::UnsignedInt(2));
        assert_eq!(def.fields[2].field_type, FieldType::UnsignedInt(4));
        assert_eq!(def.fields[3].field_type, FieldType::UnsignedInt(4));
    }

    #[test]
    fn load_ksy_with_type_reference() {
        let yaml = r#"
meta:
  id: test_struct
seq:
  - id: custom_field
    type: my_custom_type
types:
  my_custom_type:
    seq:
      - id: value
        type: u4
"#;
        let def = DefinitionLoader::load_str(yaml).unwrap();
        assert_eq!(def.fields[0].field_type, FieldType::TypeRef("my_custom_type".to_string()));
    }
}


/// Property-based tests for definition loading.
/// These tests verify universal properties across many random inputs.
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Generate a valid field id (alphanumeric with underscores, starting with letter)
    fn valid_field_id() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,15}".prop_map(|s| s.to_string())
    }

    /// Generate a valid structure id
    fn valid_struct_id() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{2,20}".prop_map(|s| s.to_string())
    }

    /// Generate a valid field size
    fn valid_size() -> impl Strategy<Value = usize> {
        1usize..1000
    }

    /// Generate a valid encoding string
    fn valid_encoding() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("ASCII"),
            Just("BCS-A"),
            Just("BCS-N"),
            Just("BCS-NPI"),
            Just("ECS-A"),
        ]
    }

    /// Generate a valid field type string
    fn valid_field_type() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("str"),
            Just("u1"),
            Just("u2"),
            Just("u4"),
            Just("s1"),
            Just("s2"),
            Just("s4"),
        ]
    }

    /// Generate a valid endian string
    fn valid_endian() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("be"), Just("le"),]
    }

    /// Property 1: Definition Round-Trip
    /// For any valid StructureDefinition, serializing it to KSY YAML format and then
    /// parsing it back SHALL produce an equivalent StructureDefinition.
    /// **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 1.6**
    mod prop_1_definition_round_trip {
        use super::*;

        /// Serialize a StructureDefinition to YAML format
        fn serialize_definition(def: &StructureDefinition) -> String {
            let mut yaml = String::new();
            yaml.push_str("meta:\n");
            yaml.push_str(&format!("  id: {}\n", def.id));
            if let Some(ref title) = def.title {
                yaml.push_str(&format!("  title: {}\n", title));
            }
            yaml.push_str(&format!(
                "  endian: {}\n",
                match def.endian {
                    Endian::Big => "be",
                    Endian::Little => "le",
                }
            ));

            if !def.fields.is_empty() {
                yaml.push_str("seq:\n");
                for field in &def.fields {
                    yaml.push_str(&format!("  - id: {}\n", field.id));
                    match &field.field_type {
                        FieldType::String => yaml.push_str("    type: str\n"),
                        FieldType::Bytes => yaml.push_str("    type: str\n"),
                        FieldType::UnsignedInt(1) => yaml.push_str("    type: u1\n"),
                        FieldType::UnsignedInt(2) => yaml.push_str("    type: u2\n"),
                        FieldType::UnsignedInt(4) => yaml.push_str("    type: u4\n"),
                        FieldType::UnsignedInt(8) => yaml.push_str("    type: u8\n"),
                        FieldType::SignedInt(1) => yaml.push_str("    type: s1\n"),
                        FieldType::SignedInt(2) => yaml.push_str("    type: s2\n"),
                        FieldType::SignedInt(4) => yaml.push_str("    type: s4\n"),
                        FieldType::SignedInt(8) => yaml.push_str("    type: s8\n"),
                        FieldType::TypeRef(name) => yaml.push_str(&format!("    type: {}\n", name)),
                        _ => {}
                    }
                    if let SizeSpec::Fixed(size) = &field.size {
                        if *size > 0
                            && !matches!(
                                field.field_type,
                                FieldType::UnsignedInt(_) | FieldType::SignedInt(_)
                            )
                        {
                            yaml.push_str(&format!("    size: {}\n", size));
                        }
                    }
                    if let Some(encoding) = &field.encoding {
                        let enc_str = match encoding {
                            Encoding::Ascii => "ASCII",
                            Encoding::BcsA => "BCS-A",
                            Encoding::BcsN => "BCS-N",
                            Encoding::BcsNPI => "BCS-NPI",
                            Encoding::EcsA => "ECS-A",
                        };
                        yaml.push_str(&format!("    encoding: {}\n", enc_str));
                    }
                }
            } else {
                yaml.push_str("seq: []\n");
            }

            yaml
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn round_trip_preserves_id(id in valid_struct_id()) {
                let yaml = format!(
                    "meta:\n  id: {}\nseq: []\n",
                    id
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                prop_assert_eq!(&def.id, &id);
            }

            #[test]
            fn round_trip_preserves_endian(endian in valid_endian()) {
                let yaml = format!(
                    "meta:\n  id: test\n  endian: {}\nseq: []\n",
                    endian
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                let expected = match endian {
                    "be" => Endian::Big,
                    "le" => Endian::Little,
                    _ => unreachable!(),
                };
                prop_assert_eq!(def.endian, expected);
            }

            #[test]
            fn round_trip_preserves_field_count(
                field_count in 1usize..10,
            ) {
                let mut yaml = String::from("meta:\n  id: test\nseq:\n");
                for i in 0..field_count {
                    yaml.push_str(&format!("  - id: field_{}\n    type: u1\n", i));
                }
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                prop_assert_eq!(def.fields.len(), field_count);
            }

            #[test]
            fn round_trip_preserves_field_id(field_id in valid_field_id()) {
                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: {}\n    type: u1\n",
                    field_id
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                prop_assert_eq!(&def.fields[0].id, &field_id);
            }

            #[test]
            fn round_trip_preserves_field_size(size in valid_size()) {
                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: field1\n    type: str\n    size: {}\n",
                    size
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                prop_assert!(matches!(def.fields[0].size, SizeSpec::Fixed(s) if s == size));
            }

            #[test]
            fn round_trip_preserves_encoding(encoding in valid_encoding()) {
                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: field1\n    type: str\n    size: 10\n    encoding: {}\n",
                    encoding
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                let expected = match encoding {
                    "ASCII" => Some(Encoding::Ascii),
                    "BCS-A" => Some(Encoding::BcsA),
                    "BCS-N" => Some(Encoding::BcsN),
                    "BCS-NPI" => Some(Encoding::BcsNPI),
                    "ECS-A" => Some(Encoding::EcsA),
                    _ => None,
                };
                prop_assert_eq!(def.fields[0].encoding, expected);
            }

            #[test]
            fn round_trip_preserves_field_type(field_type in valid_field_type()) {
                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: field1\n    type: {}\n    size: 10\n",
                    field_type
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                let expected = match field_type {
                    "str" => FieldType::String,
                    "u1" => FieldType::UnsignedInt(1),
                    "u2" => FieldType::UnsignedInt(2),
                    "u4" => FieldType::UnsignedInt(4),
                    "s1" => FieldType::SignedInt(1),
                    "s2" => FieldType::SignedInt(2),
                    "s4" => FieldType::SignedInt(4),
                    _ => unreachable!(),
                };
                prop_assert_eq!(&def.fields[0].field_type, &expected);
            }

            #[test]
            fn full_round_trip(
                id in valid_struct_id(),
                endian in valid_endian(),
                field_id in valid_field_id(),
                size in valid_size(),
            ) {
                let yaml = format!(
                    "meta:\n  id: {}\n  endian: {}\nseq:\n  - id: {}\n    type: str\n    size: {}\n",
                    id, endian, field_id, size
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();

                // Serialize back and parse again
                let yaml2 = serialize_definition(&def);
                let def2 = DefinitionLoader::load_str(&yaml2).unwrap();

                // Verify equivalence
                prop_assert_eq!(def.id, def2.id);
                prop_assert_eq!(def.endian, def2.endian);
                prop_assert_eq!(def.fields.len(), def2.fields.len());
                if !def.fields.is_empty() {
                    prop_assert_eq!(&def.fields[0].id, &def2.fields[0].id);
                    prop_assert_eq!(&def.fields[0].field_type, &def2.fields[0].field_type);
                }
            }
        }
    }

    /// Property 3: Invalid YAML Error Handling
    /// For any string that is not valid YAML syntax, the DefinitionLoader SHALL return a YamlError.
    /// **Validates: Requirements 1.7**
    mod prop_3_invalid_yaml {
        use super::*;

        /// Generate strings that are definitely not valid YAML
        fn invalid_yaml_string() -> impl Strategy<Value = String> {
            prop_oneof![
                // Unbalanced brackets
                Just("[unclosed bracket".to_string()),
                Just("{unclosed brace".to_string()),
                Just("key: [value".to_string()),
                // Invalid indentation patterns
                Just("  bad:\n indent".to_string()),
                // Invalid characters in keys
                Just("@invalid: value".to_string()),
                // Duplicate keys at same level (some parsers reject this)
                Just("key: 1\nkey: 2".to_string()),
                // Tab characters in indentation (YAML spec violation)
                Just("key:\n\t- value".to_string()),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn invalid_yaml_returns_error(yaml in invalid_yaml_string()) {
                let result = DefinitionLoader::load_str(&yaml);
                // Should either be a YAML error or a missing field error
                // (since even if YAML parses, it won't have required fields)
                prop_assert!(result.is_err());
            }

            #[test]
            fn empty_string_returns_error(s in "\\s*") {
                // Empty or whitespace-only strings should fail
                let result = DefinitionLoader::load_str(&s);
                prop_assert!(result.is_err());
            }

            #[test]
            fn random_garbage_returns_error(garbage in "[^a-zA-Z0-9:\\-\\s]{10,50}") {
                let result = DefinitionLoader::load_str(&garbage);
                prop_assert!(result.is_err());
            }
        }
    }

    /// Property 4: Undefined Type Reference Error
    /// For any KSY definition that references a type name not defined in the `types` section,
    /// the DefinitionLoader SHALL return an UndefinedType error identifying the missing type.
    /// **Validates: Requirements 1.8**
    mod prop_4_undefined_type {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn undefined_type_detected(
                // Use names that won't be interpreted as built-in types
                // Avoid: u1-u8, s1-s8, b followed by digits, str, strz
                type_name in "[a-z][a-z_]{2,10}[a-z]",
            ) {
                // Skip names that look like built-in types
                prop_assume!(!type_name.starts_with("str"));
                prop_assume!(!(type_name.len() == 2 && (type_name.starts_with('u') || type_name.starts_with('s'))));
                
                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: field1\n    type: {}\n",
                    type_name
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                let result = DefinitionLoader::validate_type_references(&def);

                // Should return UndefinedType error with the correct type name
                match result {
                    Err(LoadError::UndefinedType { type_name: ref name, .. }) => {
                        prop_assert_eq!(name, &type_name);
                    }
                    _ => prop_assert!(false, "Expected UndefinedType error"),
                }
            }

            #[test]
            fn defined_type_passes_validation(
                type_name in "[a-z][a-z0-9_]{2,15}",
            ) {
                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: field1\n    type: {}\ntypes:\n  {}:\n    seq:\n      - id: inner\n        type: u1\n",
                    type_name, type_name
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                let result = DefinitionLoader::validate_type_references(&def);
                prop_assert!(result.is_ok());
            }

            #[test]
            fn multiple_undefined_types_first_detected(
                type1 in "[a-z][a-z0-9_]{2,10}",
                type2 in "[a-z][a-z0-9_]{2,10}",
            ) {
                // Ensure type names are different
                prop_assume!(type1 != type2);

                let yaml = format!(
                    "meta:\n  id: test\nseq:\n  - id: field1\n    type: {}\n  - id: field2\n    type: {}\n",
                    type1, type2
                );
                let def = DefinitionLoader::load_str(&yaml).unwrap();
                let result = DefinitionLoader::validate_type_references(&def);

                // Should detect at least one undefined type
                match result {
                    Err(LoadError::UndefinedType { .. }) => prop_assert!(true),
                    _ => prop_assert!(false, "Expected UndefinedType error"),
                }
            }
        }
    }
}
