//! Unit tests for StructureAccessor.

use super::*;
use crate::parser::types::{FieldDefinition, FieldType, SizeSpec, StructureDefinition};

fn create_simple_definition() -> StructureDefinition {
    StructureDefinition::new("test_struct")
        .with_field(
            FieldDefinition::new("magic", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
        .with_field(
            FieldDefinition::new("version", FieldType::UnsignedInt(2))
                .with_size(SizeSpec::Fixed(2)),
        )
        .with_field(
            FieldDefinition::new("name", FieldType::String)
                .with_size(SizeSpec::Fixed(10))
                .with_encoding(Encoding::BcsA),
        )
}

#[test]
fn accessor_new() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();
    assert_eq!(accessor.data().len(), 16);
}

#[test]
fn accessor_get_string_field() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let value = accessor.get("magic").unwrap();
    assert_eq!(value.as_str().unwrap(), "TEST");
}

#[test]
fn accessor_get_integer_field() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let value = accessor.get("version").unwrap();
    assert_eq!(value.as_u64().unwrap(), 1);
}

#[test]
fn accessor_get_padded_string() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let value = accessor.get("name").unwrap();
    assert_eq!(value.as_str().unwrap(), "HELLO");
}


#[test]
fn accessor_unknown_field_error() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let result = accessor.get("nonexistent");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AccessError::UnknownField { .. }));
}

#[test]
fn accessor_has_existing_field() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    assert!(accessor.has("magic"));
    assert!(accessor.has("version"));
    assert!(accessor.has("name"));
}

#[test]
fn accessor_has_nonexistent_field() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    assert!(!accessor.has("nonexistent"));
}

#[test]
fn accessor_field_info() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let info = accessor.field_info("magic").unwrap();
    assert_eq!(info.size, 4);
    assert_eq!(info.offset, 0);
    assert_eq!(info.field_type, FieldType::String);

    let info = accessor.field_info("version").unwrap();
    assert_eq!(info.size, 2);
    assert_eq!(info.offset, 4);

    let info = accessor.field_info("name").unwrap();
    assert_eq!(info.size, 10);
    assert_eq!(info.offset, 6);
}

#[test]
fn accessor_fields_iterator() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let fields: Vec<String> = accessor.fields().collect();
    assert_eq!(fields.len(), 3);
    assert!(fields.contains(&"magic".to_string()));
    assert!(fields.contains(&"version".to_string()));
    assert!(fields.contains(&"name".to_string()));
}

#[test]
fn accessor_raw_slice() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let slice = accessor.raw_slice("magic").unwrap();
    assert_eq!(slice, b"TEST");

    let slice = accessor.raw_slice("version").unwrap();
    assert_eq!(slice, &[0x00, 0x01]);
}

#[test]
fn accessor_field_byte_range() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let (offset, size) = accessor.field_byte_range("magic").unwrap();
    assert_eq!(offset, 0);
    assert_eq!(size, 4);

    let (offset, size) = accessor.field_byte_range("version").unwrap();
    assert_eq!(offset, 4);
    assert_eq!(size, 2);

    let (offset, size) = accessor.field_byte_range("name").unwrap();
    assert_eq!(offset, 6);
    assert_eq!(size, 10);
}


#[test]
fn accessor_raw_slice_repeated_field_element() {
    // Test that raw_slice works for individual elements of repeated fields
    let def = Arc::new(
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(4)),
        ),
    );
    let data = b"\x01\x02\x03\x04";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Individual elements should work
    let slice = accessor.raw_slice("items_0").unwrap();
    assert_eq!(slice, &[0x01]);

    let slice = accessor.raw_slice("items_3").unwrap();
    assert_eq!(slice, &[0x04]);
}

#[test]
fn accessor_raw_slice_non_contiguous_array() {
    // Test that raw_slice returns NonContiguous for array access without index
    let def = Arc::new(
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(4)),
        ),
    );
    let data = b"\x01\x02\x03\x04";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Accessing entire array should return NonContiguous
    let result = accessor.raw_slice("items");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AccessError::NonContiguous { .. }
    ));
}

#[test]
fn accessor_field_byte_range_non_contiguous_array() {
    // Test that field_byte_range returns NonContiguous for array access without index
    let def = Arc::new(
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(4)),
        ),
    );
    let data = b"\x01\x02\x03\x04";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Accessing entire array should return NonContiguous
    let result = accessor.field_byte_range("items");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AccessError::NonContiguous { .. }
    ));

    // Individual elements should work
    let (offset, size) = accessor.field_byte_range("items_0").unwrap();
    assert_eq!(offset, 0);
    assert_eq!(size, 1);

    let (offset, size) = accessor.field_byte_range("items_2").unwrap();
    assert_eq!(offset, 2);
    assert_eq!(size, 1);
}

#[test]
fn accessor_unexpected_eof() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST"; // Too short
    let accessor = StructureAccessor::new(def, data).unwrap();

    // First field should work
    assert!(accessor.get("magic").is_ok());

    // Second field should fail with EOF
    let result = accessor.get("version");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AccessError::UnexpectedEof { .. }
    ));
}

#[test]
fn accessor_little_endian() {
    use crate::parser::types::Endian;

    let def = StructureDefinition::new("test_struct")
        .with_endian(Endian::Little)
        .with_field(
            FieldDefinition::new("value", FieldType::UnsignedInt(2)).with_size(SizeSpec::Fixed(2)),
        );

    let data = &[0x01, 0x00]; // 1 in little-endian
    let accessor = StructureAccessor::new(Arc::new(def), data).unwrap();

    let value = accessor.get("value").unwrap();
    assert_eq!(value.as_u64().unwrap(), 1);
}

#[test]
fn accessor_big_endian() {
    use crate::parser::types::Endian;

    let def = StructureDefinition::new("test_struct")
        .with_endian(Endian::Big)
        .with_field(
            FieldDefinition::new("value", FieldType::UnsignedInt(2)).with_size(SizeSpec::Fixed(2)),
        );

    let data = &[0x00, 0x01]; // 1 in big-endian
    let accessor = StructureAccessor::new(Arc::new(def), data).unwrap();

    let value = accessor.get("value").unwrap();
    assert_eq!(value.as_u64().unwrap(), 1);
}

#[test]
fn accessor_offset_caching() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // First access calculates offset
    let _ = accessor.get("name").unwrap();

    // Check cache
    let cache = accessor.offset_cache.borrow();
    assert!(cache.contains_key("name"));
    assert_eq!(cache.get("name"), Some(&(6, 10)));
}


#[test]
fn accessor_variable_offset_field() {
    // Create a definition with expression-based size
    use crate::parser::expression::ExpressionEvaluator;

    let size_expr = ExpressionEvaluator::parse("len").unwrap();
    let def = StructureDefinition::new("test_struct")
        .with_field(
            FieldDefinition::new("len", FieldType::UnsignedInt(1)).with_size(SizeSpec::Fixed(1)),
        )
        .with_field(
            FieldDefinition::new("data", FieldType::String)
                .with_size(SizeSpec::Expression(size_expr)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        );

    // len=5, data="HELLO", trailer="DONE"
    let data = b"\x05HELLODONE";
    let accessor = StructureAccessor::new(Arc::new(def), data).unwrap();

    // Access len
    let len_val = accessor.get("len").unwrap();
    assert_eq!(len_val.as_u64().unwrap(), 5);

    // Access data (variable size based on len)
    let data_val = accessor.get("data").unwrap();
    assert_eq!(data_val.as_str().unwrap(), "HELLO");

    // Access trailer (offset depends on variable-size data field)
    let trailer_val = accessor.get("trailer").unwrap();
    assert_eq!(trailer_val.as_str().unwrap(), "DONE");

    // Verify offsets
    let (offset, size) = accessor.field_byte_range("data").unwrap();
    assert_eq!(offset, 1);
    assert_eq!(size, 5);

    let (offset, size) = accessor.field_byte_range("trailer").unwrap();
    assert_eq!(offset, 6);
    assert_eq!(size, 4);
}

#[test]
fn accessor_caching_multiple_accesses() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access same field multiple times
    for _ in 0..5 {
        let _ = accessor.get("name").unwrap();
    }

    // Cache should have the entry
    let cache = accessor.offset_cache.borrow();
    assert!(cache.contains_key("name"));

    // Verify cached value is correct
    let (offset, size) = *cache.get("name").unwrap();
    assert_eq!(offset, 6);
    assert_eq!(size, 10);
}

// ==================== Conditional Field Tests ====================

fn create_conditional_definition() -> StructureDefinition {
    use crate::parser::expression::ExpressionEvaluator;

    let condition = ExpressionEvaluator::parse("has_extra == 1").unwrap();

    StructureDefinition::new("test_struct")
        .with_field(
            FieldDefinition::new("has_extra", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1)),
        )
        .with_field(
            FieldDefinition::new("extra_data", FieldType::String)
                .with_size(SizeSpec::Fixed(10))
                .with_condition(condition),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_conditional_field_present() {
    let def = Arc::new(create_conditional_definition());
    // has_extra=1, extra_data="EXTRA     ", trailer="DONE"
    let data = b"\x01EXTRA     DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // has_extra should be 1
    let has_extra = accessor.get("has_extra").unwrap();
    assert_eq!(has_extra.as_u64().unwrap(), 1);

    // extra_data should be accessible
    assert!(accessor.has("extra_data"));
    let extra = accessor.get("extra_data").unwrap();
    assert_eq!(extra.as_str().unwrap(), "EXTRA");

    // trailer should be accessible
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");
}

#[test]
fn accessor_conditional_field_not_present() {
    let def = Arc::new(create_conditional_definition());
    // has_extra=0, trailer="DONE" (no extra_data)
    let data = b"\x00DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // has_extra should be 0
    let has_extra = accessor.get("has_extra").unwrap();
    assert_eq!(has_extra.as_u64().unwrap(), 0);

    // extra_data should NOT be accessible
    assert!(!accessor.has("extra_data"));
    let result = accessor.get("extra_data");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AccessError::ConditionalNotPresent { .. }
    ));

    // trailer should still be accessible (at offset 1, not 11)
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");
}


#[test]
fn accessor_conditional_field_affects_offset() {
    let def = Arc::new(create_conditional_definition());

    // When condition is true, trailer is at offset 11
    let data_with_extra = b"\x01EXTRA     DONE";
    let accessor1 = StructureAccessor::new(Arc::clone(&def), data_with_extra).unwrap();
    let (offset, _) = accessor1.field_byte_range("trailer").unwrap();
    assert_eq!(offset, 11);

    // When condition is false, trailer is at offset 1
    let data_without_extra = b"\x00DONE";
    let accessor2 = StructureAccessor::new(def, data_without_extra).unwrap();
    let (offset, _) = accessor2.field_byte_range("trailer").unwrap();
    assert_eq!(offset, 1);
}

#[test]
fn accessor_conditional_fields_iterator() {
    let def = Arc::new(create_conditional_definition());

    // With condition true
    let data_with_extra = b"\x01EXTRA     DONE";
    let accessor1 = StructureAccessor::new(Arc::clone(&def), data_with_extra).unwrap();
    let fields1: Vec<String> = accessor1.fields().collect();
    assert!(fields1.contains(&"has_extra".to_string()));
    assert!(fields1.contains(&"extra_data".to_string()));
    assert!(fields1.contains(&"trailer".to_string()));

    // With condition false
    let data_without_extra = b"\x00DONE";
    let accessor2 = StructureAccessor::new(def, data_without_extra).unwrap();
    let fields2: Vec<String> = accessor2.fields().collect();
    assert!(fields2.contains(&"has_extra".to_string()));
    assert!(!fields2.contains(&"extra_data".to_string())); // Should be excluded
    assert!(fields2.contains(&"trailer".to_string()));
}

// ==================== Repetition Tests ====================

fn create_repeat_expr_definition() -> StructureDefinition {
    use crate::parser::expression::ExpressionEvaluator;

    let repeat_expr = ExpressionEvaluator::parse("count").unwrap();

    StructureDefinition::new("test_struct")
        .with_field(
            FieldDefinition::new("count", FieldType::UnsignedInt(1)).with_size(SizeSpec::Fixed(1)),
        )
        .with_field(
            FieldDefinition::new("items", FieldType::String)
                .with_size(SizeSpec::Fixed(4))
                .with_repeat(RepeatSpec::Expression(repeat_expr)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_repeat_expr_basic() {
    let def = Arc::new(create_repeat_expr_definition());
    // count=3, items=["AAAA", "BBBB", "CCCC"], trailer="DONE"
    let data = b"\x03AAAABBBBCCCCDONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access count
    let count = accessor.get("count").unwrap();
    assert_eq!(count.as_u64().unwrap(), 3);

    // Access individual items using underscore-indexed naming
    let item0 = accessor.get("items_0").unwrap();
    assert_eq!(item0.as_str().unwrap(), "AAAA");

    let item1 = accessor.get("items_1").unwrap();
    assert_eq!(item1.as_str().unwrap(), "BBBB");

    let item2 = accessor.get("items_2").unwrap();
    assert_eq!(item2.as_str().unwrap(), "CCCC");

    // Access trailer
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");
}

#[test]
fn accessor_repeat_expr_as_array() {
    let def = Arc::new(create_repeat_expr_definition());
    let data = b"\x03AAAABBBBCCCCDONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access entire array
    let items = accessor.get("items").unwrap();
    assert!(items.is_array());

    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str().unwrap(), "AAAA");
        assert_eq!(arr[1].as_str().unwrap(), "BBBB");
        assert_eq!(arr[2].as_str().unwrap(), "CCCC");
    } else {
        panic!("Expected array");
    }
}

#[test]
fn accessor_repeat_expr_zero_count() {
    let def = Arc::new(create_repeat_expr_definition());
    // count=0, no items, trailer="DONE"
    let data = b"\x00DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access count
    let count = accessor.get("count").unwrap();
    assert_eq!(count.as_u64().unwrap(), 0);

    // Access entire array - should be empty
    let items = accessor.get("items").unwrap();
    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 0);
    } else {
        panic!("Expected array");
    }

    // items_0 should not exist
    assert!(!accessor.has("items_0"));

    // trailer should be at offset 1
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");
}


#[test]
fn accessor_repeat_expr_out_of_bounds() {
    let def = Arc::new(create_repeat_expr_definition());
    let data = b"\x03AAAABBBBCCCCDONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // items_3 should not exist (only 0, 1, 2)
    assert!(!accessor.has("items_3"));
    let result = accessor.get("items_3");
    assert!(result.is_err());
}

fn create_repeat_count_definition() -> StructureDefinition {
    StructureDefinition::new("test_struct")
        .with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(4)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_repeat_count() {
    let def = Arc::new(create_repeat_count_definition());
    // 4 bytes for items, 4 bytes for trailer
    let data = b"\x01\x02\x03\x04DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access individual items
    assert_eq!(accessor.get("items_0").unwrap().as_u64().unwrap(), 1);
    assert_eq!(accessor.get("items_1").unwrap().as_u64().unwrap(), 2);
    assert_eq!(accessor.get("items_2").unwrap().as_u64().unwrap(), 3);
    assert_eq!(accessor.get("items_3").unwrap().as_u64().unwrap(), 4);

    // Access trailer
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");
}

fn create_repeat_eos_definition() -> StructureDefinition {
    StructureDefinition::new("test_struct").with_field(
        FieldDefinition::new("items", FieldType::UnsignedInt(1))
            .with_size(SizeSpec::Fixed(1))
            .with_repeat(RepeatSpec::Eos),
    )
}

#[test]
fn accessor_repeat_eos() {
    let def = Arc::new(create_repeat_eos_definition());
    let data = b"\x01\x02\x03\x04\x05";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access entire array
    let items = accessor.get("items").unwrap();
    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0].as_u64().unwrap(), 1);
        assert_eq!(arr[4].as_u64().unwrap(), 5);
    } else {
        panic!("Expected array");
    }
}

#[test]
fn accessor_repeat_fields_iterator() {
    let def = Arc::new(create_repeat_expr_definition());
    let data = b"\x03AAAABBBBCCCCDONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let fields: Vec<String> = accessor.fields().collect();

    // Should contain count, items_0, items_1, items_2, trailer
    assert!(fields.contains(&"count".to_string()));
    assert!(fields.contains(&"items_0".to_string()));
    assert!(fields.contains(&"items_1".to_string()));
    assert!(fields.contains(&"items_2".to_string()));
    assert!(fields.contains(&"trailer".to_string()));

    // Should NOT contain items_3
    assert!(!fields.contains(&"items_3".to_string()));
}
