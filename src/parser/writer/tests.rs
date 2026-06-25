//! Unit tests for the structure writer.

use super::*;
use crate::parser::types::{
    Encoding, Endian, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
};

fn create_simple_definition() -> Arc<StructureDefinition> {
    Arc::new(
        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("field1", FieldType::String).with_size(SizeSpec::fixed(10)),
            )
            .with_field(
                FieldDefinition::new("field2", FieldType::String).with_size(SizeSpec::fixed(5)),
            )
            .with_field(FieldDefinition::new("field3", FieldType::u2())),
    )
}

fn create_definition_with_encoding() -> Arc<StructureDefinition> {
    Arc::new(
        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("bcs_a_field", FieldType::String)
                    .with_size(SizeSpec::fixed(10))
                    .with_encoding(Encoding::BcsA),
            )
            .with_field(
                FieldDefinition::new("bcs_n_field", FieldType::String)
                    .with_size(SizeSpec::fixed(5))
                    .with_encoding(Encoding::BcsN),
            ),
    )
}

fn create_definition_with_repeat() -> Arc<StructureDefinition> {
    Arc::new(
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::String)
                .with_size(SizeSpec::fixed(4))
                .with_repeat(RepeatSpec::count(3)),
        ),
    )
}

// ==================== Streaming mode tests ====================

#[test]
fn new_creates_empty_buffer() {
    let def = create_simple_definition();
    let writer = StructureWriter::new(def);
    assert_eq!(writer.buffer().len(), 0);
}

#[test]
fn set_writes_sequentially() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HELLO").unwrap();

    // Check that "HELLO" is at offset 0, padded with spaces
    assert_eq!(&writer.buffer()[0..5], b"HELLO");
    assert_eq!(&writer.buffer()[5..10], b"     ");
}

#[test]
fn sequential_writes_produce_correct_output() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HELLO").unwrap();
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();

    let buffer = writer.buffer();

    // field1 at offset 0-9
    assert_eq!(&buffer[0..5], b"HELLO");
    // field2 at offset 10-14
    assert_eq!(&buffer[10..15], b"WORLD");
    // field3 at offset 15-16 (big-endian u16)
    assert_eq!(&buffer[15..17], &[0, 42]);
}

#[test]
fn out_of_order_write_fails() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    // Try to write field2 before field1
    let result = writer.set("field2", "WORLD");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }));
}

#[test]
fn is_set_tracks_written_fields() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    assert!(!writer.is_set("field1"));
    writer.set("field1", "HELLO").unwrap();
    assert!(writer.is_set("field1"));
    assert!(!writer.is_set("field2"));
}

#[test]
fn finish_returns_buffer() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HELLO").unwrap();
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();

    let result = writer.finish().unwrap();
    assert_eq!(result.len(), 17);
}

#[test]
fn finish_fails_if_required_field_missing() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HELLO").unwrap();
    // field2 and field3 not written

    let result = writer.finish();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WriteError::MissingRequired { .. }
    ));
}

#[test]
fn value_too_large_error() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    // field1 is 10 bytes, try to write 16 bytes
    let result = writer.set("field1", "THIS IS TOO LONG");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WriteError::ValueTooLarge { .. }
    ));
}

#[test]
fn padding_applied_for_short_strings() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HI").unwrap();

    // "HI" followed by 8 spaces
    assert_eq!(&writer.buffer()[0..2], b"HI");
    assert_eq!(&writer.buffer()[2..10], b"        ");
}

// ==================== Encoding validation tests ====================

#[test]
fn bcs_a_validation_accepts_valid_chars() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new(def);

    // Valid BCS-A: ASCII 0x20-0x7E
    let result = writer.set("bcs_a_field", "Hello 123");
    assert!(result.is_ok());
}

#[test]
fn bcs_a_validation_rejects_invalid_chars() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new(def);

    // Invalid BCS-A: contains control character
    let result = writer.set("bcs_a_field", "Hello\x00");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WriteError::ValidationError { .. }
    ));
}

#[test]
fn bcs_n_validation_accepts_digits_and_space() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new(def);

    writer.set("bcs_a_field", "TEST").unwrap();

    // Valid BCS-N: digits, space, plus, minus, decimal point, slash
    let result = writer.set("bcs_n_field", "+2.34");
    assert!(result.is_ok());
}

#[test]
fn bcs_n_validation_rejects_letters_in_strict_mode() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new(def);
    writer.set_strict_encoding(true);

    writer.set("bcs_a_field", "TEST").unwrap();

    // Invalid BCS-N: contains letters (rejected in strict mode)
    let result = writer.set("bcs_n_field", "12A34");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WriteError::ValidationError { .. }
    ));
}

#[test]
fn bcs_n_validation_accepts_letters_in_permissive_mode() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new(def);

    writer.set("bcs_a_field", "TEST").unwrap();

    // BCS-N with letters: accepted in permissive mode (default)
    let result = writer.set("bcs_n_field", "1.0E5");
    assert!(result.is_ok());
}

#[test]
fn bcs_npi_accepts_plus_sign_in_permissive_mode() {
    // Simulates the RPC00B round-trip: reading HEIGHT_SCALE "+0697" and writing it back
    let def = Arc::new(
        StructureDefinition::new("rpc_snippet").with_field(
            FieldDefinition::new("HEIGHT_SCALE", FieldType::String)
                .with_size(SizeSpec::fixed(5))
                .with_encoding(Encoding::BcsNPI),
        ),
    );

    let mut writer = StructureWriter::new(def);
    let result = writer.set("HEIGHT_SCALE", "+0697");
    assert!(result.is_ok());
    assert_eq!(writer.buffer(), b"+0697");
}

#[test]
fn bcs_npi_rejects_plus_sign_in_strict_mode() {
    let def = Arc::new(
        StructureDefinition::new("rpc_snippet").with_field(
            FieldDefinition::new("HEIGHT_SCALE", FieldType::String)
                .with_size(SizeSpec::fixed(5))
                .with_encoding(Encoding::BcsNPI),
        ),
    );

    let mut writer = StructureWriter::new(def);
    writer.set_strict_encoding(true);
    let result = writer.set("HEIGHT_SCALE", "+0697");
    assert!(result.is_err());
}

#[test]
fn bcs_n_uses_zero_padding() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new(def);

    writer.set("bcs_a_field", "TEST").unwrap();
    writer.set("bcs_n_field", "12").unwrap();

    // BCS-N left-pads with '0' (0x30) — numeric right-justification
    let buffer = writer.buffer();
    assert_eq!(&buffer[10..13], b"000");
    assert_eq!(&buffer[13..15], b"12");
}

// ==================== Repeated field tests ====================

#[test]
fn repeated_fields_via_array() {
    let def = create_definition_with_repeat();
    let mut writer = StructureWriter::new(def);

    writer.set("items", vec!["AAA", "BBB", "CCC"]).unwrap();

    let result = writer.finish().unwrap();
    assert_eq!(result.len(), 12); // 3 * 4 bytes
    assert_eq!(&result[0..3], b"AAA");
    assert_eq!(&result[4..7], b"BBB");
    assert_eq!(&result[8..11], b"CCC");
}

#[test]
fn finish_fails_if_repeated_field_missing() {
    let def = create_definition_with_repeat();
    let writer = StructureWriter::new(def);

    let result = writer.finish();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WriteError::MissingRequired { .. }
    ));
}

// ==================== Integer encoding tests ====================

#[test]
fn unsigned_integer_big_endian() {
    let def = Arc::new(
        StructureDefinition::new("test").with_field(FieldDefinition::new("value", FieldType::u2())),
    );
    let mut writer = StructureWriter::new(def);

    writer.set("value", 0x1234u64).unwrap();

    assert_eq!(writer.buffer(), &[0x12, 0x34]);
}

#[test]
fn unsigned_integer_little_endian() {
    let def = Arc::new(
        StructureDefinition::new("test")
            .with_endian(Endian::Little)
            .with_field(FieldDefinition::new("value", FieldType::u2())),
    );
    let mut writer = StructureWriter::new(def);

    writer.set("value", 0x1234u64).unwrap();

    assert_eq!(writer.buffer(), &[0x34, 0x12]);
}

#[test]
fn signed_integer_encoding() {
    let def = Arc::new(
        StructureDefinition::new("test").with_field(FieldDefinition::new("value", FieldType::s2())),
    );
    let mut writer = StructureWriter::new(def);

    writer.set("value", -1i64).unwrap();

    // -1 as i16 big-endian is 0xFFFF
    assert_eq!(writer.buffer(), &[0xFF, 0xFF]);
}

// ==================== Growable buffer tests ====================

#[test]
fn streaming_mode_growable_buffer() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    // Buffer should grow as we write
    assert_eq!(writer.buffer().len(), 0);

    writer.set("field1", "HELLO").unwrap();
    assert_eq!(writer.buffer().len(), 10);

    writer.set("field2", "WORLD").unwrap();
    assert_eq!(writer.buffer().len(), 15);

    writer.set("field3", 42u64).unwrap();
    assert_eq!(writer.buffer().len(), 17);
}

#[test]
fn skipping_field_fails() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HELLO").unwrap();
    // Skip field2, try to write field3
    let result = writer.set("field3", 42u64);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }));
}

// ==================== WriteValue conversion tests ====================

#[test]
fn write_value_from_string() {
    let value: WriteValue = "test".into();
    assert!(matches!(value, WriteValue::String(s) if s == "test"));
}

#[test]
fn write_value_from_i64() {
    let value: WriteValue = 42i64.into();
    assert!(matches!(value, WriteValue::Integer(42)));
}

#[test]
fn write_value_from_u64() {
    let value: WriteValue = 42u64.into();
    assert!(matches!(value, WriteValue::Unsigned(42)));
}

#[test]
fn write_value_from_bytes() {
    let value: WriteValue = vec![1u8, 2, 3].into();
    assert!(matches!(value, WriteValue::Bytes(b) if b == vec![1, 2, 3]));
}

#[test]
fn write_value_from_array() {
    let value: WriteValue = vec!["a", "b", "c"].into();
    assert!(matches!(value, WriteValue::Array(_)));
}

// ==================== write_to tests ====================

#[test]
fn write_to_outputs_to_writer() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new(def);

    writer.set("field1", "HELLO").unwrap();
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();

    let mut output = Vec::new();
    let bytes_written = writer.write_to(&mut output).unwrap();

    assert_eq!(bytes_written, 17);
    assert_eq!(output.len(), 17);
}

// ==================== SizeSpec::Eos tests ====================

#[test]
fn eos_field_writes_verbatim_no_padding() {
    let def = Arc::new(
        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("tag", FieldType::String).with_size(SizeSpec::fixed(6)),
            )
            .with_field(
                FieldDefinition::new("comment", FieldType::String).with_size(SizeSpec::Eos),
            ),
    );
    let mut writer = StructureWriter::new(def);

    writer.set("tag", "HEADER").unwrap();
    writer.set("comment", "Hello, world!").unwrap();
    let data = writer.finish().unwrap();

    assert_eq!(&data[..6], b"HEADER");
    assert_eq!(&data[6..], b"Hello, world!");
    assert_eq!(data.len(), 19);
}

#[test]
fn eos_field_writes_empty_string() {
    let def =
        Arc::new(StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("comment", FieldType::String).with_size(SizeSpec::Eos),
        ));
    let mut writer = StructureWriter::new(def);

    writer.set("comment", "").unwrap();
    let data = writer.finish().unwrap();

    assert_eq!(data.len(), 0);
}
