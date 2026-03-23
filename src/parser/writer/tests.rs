//! Unit tests for the structure writer.

use super::*;
use crate::parser::types::{Encoding, Endian, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition};

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
        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("items", FieldType::String)
                    .with_size(SizeSpec::fixed(4))
                    .with_repeat(RepeatSpec::count(3)),
            ),
    )
}

// ==================== Fixed-size mode tests ====================

#[test]
fn new_fixed_creates_preallocated_buffer() {
    let def = create_simple_definition();
    let writer = StructureWriter::new_fixed(def).unwrap();

    // 10 + 5 + 2 = 17 bytes
    assert_eq!(writer.buffer().len(), 17);
}

#[test]
fn set_writes_at_correct_offset() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("field1", "HELLO").unwrap();

    // Check that "HELLO" is at offset 0, padded with spaces
    assert_eq!(&writer.buffer()[0..5], b"HELLO");
    assert_eq!(&writer.buffer()[5..10], b"     ");
}

#[test]
fn set_out_of_order_works_in_fixed_mode() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    // Write fields out of order
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();
    writer.set("field1", "HELLO").unwrap();

    let buffer = writer.buffer();

    // field1 at offset 0-9
    assert_eq!(&buffer[0..5], b"HELLO");
    // field2 at offset 10-14
    assert_eq!(&buffer[10..15], b"WORLD");
    // field3 at offset 15-16 (big-endian u16)
    assert_eq!(&buffer[15..17], &[0, 42]);
}

#[test]
fn is_set_tracks_written_fields() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    assert!(!writer.is_set("field1"));
    writer.set("field1", "TEST").unwrap();
    assert!(writer.is_set("field1"));
    assert!(!writer.is_set("field2"));
}

#[test]
fn finish_returns_buffer() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("field1", "HELLO").unwrap();
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();

    let result = writer.finish().unwrap();
    assert_eq!(result.len(), 17);
}

#[test]
fn finish_fails_if_required_field_missing() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("field1", "HELLO").unwrap();
    // field2 and field3 not written

    let result = writer.finish();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::MissingRequired { .. }));
}

#[test]
fn value_too_large_error() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    // field1 is 10 bytes, try to write 15 bytes
    let result = writer.set("field1", "THIS IS TOO LONG");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::ValueTooLarge { .. }));
}

#[test]
fn padding_applied_for_short_strings() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("field1", "HI").unwrap();

    // "HI" followed by 8 spaces
    assert_eq!(&writer.buffer()[0..2], b"HI");
    assert_eq!(&writer.buffer()[2..10], b"        ");
}

// ==================== Encoding validation tests ====================

#[test]
fn bcs_a_validation_accepts_valid_chars() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    // Valid BCS-A: ASCII 0x20-0x7E
    let result = writer.set("bcs_a_field", "Hello 123");
    assert!(result.is_ok());
}

#[test]
fn bcs_a_validation_rejects_invalid_chars() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    // Invalid BCS-A: contains control character
    let result = writer.set("bcs_a_field", "Hello\x00");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::ValidationError { .. }));
}

#[test]
fn bcs_n_validation_accepts_digits_and_space() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("bcs_a_field", "TEST").unwrap(); // Need to write first field

    // Valid BCS-N: digits, space, plus, minus, decimal point, slash
    let result = writer.set("bcs_n_field", "+2.34");
    assert!(result.is_ok());
}

#[test]
fn bcs_n_validation_rejects_letters() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("bcs_a_field", "TEST").unwrap();

    // Invalid BCS-N: contains letters
    let result = writer.set("bcs_n_field", "12A34");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::ValidationError { .. }));
}

#[test]
fn bcs_n_uses_zero_padding() {
    let def = create_definition_with_encoding();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("bcs_a_field", "TEST").unwrap();
    writer.set("bcs_n_field", "12").unwrap();

    // BCS-N should pad with '0' (0x30)
    let buffer = writer.buffer();
    assert_eq!(&buffer[10..12], b"12");
    assert_eq!(&buffer[12..15], b"000");
}

// ==================== Repeated field tests ====================

#[test]
fn repeated_fields_with_index() {
    let def = create_definition_with_repeat();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("items_0", "AAA").unwrap();
    writer.set("items_1", "BBB").unwrap();
    writer.set("items_2", "CCC").unwrap();

    let buffer = writer.buffer();
    assert_eq!(&buffer[0..3], b"AAA");
    assert_eq!(&buffer[4..7], b"BBB");
    assert_eq!(&buffer[8..11], b"CCC");
}

#[test]
fn repeated_fields_out_of_order() {
    let def = create_definition_with_repeat();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    // Write in reverse order
    writer.set("items_2", "CCC").unwrap();
    writer.set("items_0", "AAA").unwrap();
    writer.set("items_1", "BBB").unwrap();

    let buffer = writer.buffer();
    assert_eq!(&buffer[0..3], b"AAA");
    assert_eq!(&buffer[4..7], b"BBB");
    assert_eq!(&buffer[8..11], b"CCC");
}

#[test]
fn finish_fails_if_repeated_element_missing() {
    let def = create_definition_with_repeat();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("items_0", "AAA").unwrap();
    writer.set("items_2", "CCC").unwrap();
    // items_1 not written

    let result = writer.finish();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::MissingRequired { path } if path == "items_1"));
}

// ==================== Integer encoding tests ====================

#[test]
fn unsigned_integer_big_endian() {
    let def = Arc::new(
        StructureDefinition::new("test")
            .with_field(FieldDefinition::new("value", FieldType::u2())),
    );
    let mut writer = StructureWriter::new_fixed(def).unwrap();

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
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("value", 0x1234u64).unwrap();

    assert_eq!(writer.buffer(), &[0x34, 0x12]);
}

#[test]
fn signed_integer_encoding() {
    let def = Arc::new(
        StructureDefinition::new("test")
            .with_field(FieldDefinition::new("value", FieldType::s2())),
    );
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("value", -1i64).unwrap();

    // -1 as i16 big-endian is 0xFFFF
    assert_eq!(writer.buffer(), &[0xFF, 0xFF]);
}

// ==================== Streaming mode tests ====================

#[test]
fn streaming_mode_sequential_writes() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_streaming(def);

    writer.set("field1", "HELLO").unwrap();
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();

    let result = writer.finish().unwrap();
    assert_eq!(result.len(), 17);
    assert_eq!(&result[0..5], b"HELLO");
    assert_eq!(&result[10..15], b"WORLD");
}

#[test]
fn streaming_mode_out_of_order_fails() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_streaming(def);

    // Try to write field2 before field1
    let result = writer.set("field2", "WORLD");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }));
}

#[test]
fn streaming_mode_repeated_fields_sequential() {
    let def = create_definition_with_repeat();
    let mut writer = StructureWriter::new_streaming(def);

    // Write repeated fields in order
    writer.set("items_0", "AAA").unwrap();
    writer.set("items_1", "BBB").unwrap();
    writer.set("items_2", "CCC").unwrap();

    let result = writer.finish().unwrap();
    assert_eq!(result.len(), 12); // 3 * 4 bytes
    assert_eq!(&result[0..3], b"AAA");
    assert_eq!(&result[4..7], b"BBB");
    assert_eq!(&result[8..11], b"CCC");
}

#[test]
fn streaming_mode_repeated_fields_out_of_order_fails() {
    let def = create_definition_with_repeat();
    let mut writer = StructureWriter::new_streaming(def);

    // Try to write items_1 before items_0
    let result = writer.set("items_1", "BBB");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }));
}

#[test]
fn streaming_mode_skipping_field_fails() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_streaming(def);

    writer.set("field1", "HELLO").unwrap();
    // Skip field2, try to write field3
    let result = writer.set("field3", 42u64);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }));
}

#[test]
fn streaming_mode_growable_buffer() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_streaming(def);

    // Buffer should grow as we write
    assert_eq!(writer.buffer().len(), 0);

    writer.set("field1", "HELLO").unwrap();
    assert_eq!(writer.buffer().len(), 10);

    writer.set("field2", "WORLD").unwrap();
    assert_eq!(writer.buffer().len(), 15);

    writer.set("field3", 42u64).unwrap();
    assert_eq!(writer.buffer().len(), 17);
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

// ==================== write_to tests ====================

#[test]
fn write_to_outputs_to_writer() {
    let def = create_simple_definition();
    let mut writer = StructureWriter::new_fixed(def).unwrap();

    writer.set("field1", "HELLO").unwrap();
    writer.set("field2", "WORLD").unwrap();
    writer.set("field3", 42u64).unwrap();

    let mut output = Vec::new();
    let bytes_written = writer.write_to(&mut output).unwrap();

    assert_eq!(bytes_written, 17);
    assert_eq!(output.len(), 17);
}
