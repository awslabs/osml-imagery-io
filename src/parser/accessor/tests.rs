//! Unit tests for StructureAccessor.

use super::*;
use crate::parser::types::{FieldDefinition, FieldType, SizeSpec, StructureDefinition};

fn create_simple_definition() -> StructureDefinition {
    StructureDefinition::new("test_struct")
        .with_field(FieldDefinition::new("magic", FieldType::String).with_size(SizeSpec::Fixed(4)))
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
    assert!(matches!(
        result.unwrap_err(),
        AccessError::UnknownField { .. }
    ));
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
fn accessor_field_info_offsets() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let info = accessor.field_info("magic").unwrap();
    assert_eq!(info.offset, 0);
    assert_eq!(info.size, 4);

    let info = accessor.field_info("version").unwrap();
    assert_eq!(info.offset, 4);
    assert_eq!(info.size, 2);

    let info = accessor.field_info("name").unwrap();
    assert_eq!(info.offset, 6);
    assert_eq!(info.size, 10);
}

#[test]
fn accessor_raw_slice_repeated_field_returns_non_contiguous() {
    // Test that raw_slice returns NonContiguous for repeated fields (no _N access)
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
fn accessor_field_info_non_contiguous_array() {
    // Test that field_info for array fields returns base offset info
    let def = Arc::new(
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(4)),
        ),
    );
    let data = b"\x01\x02\x03\x04";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // field_info for a repeated field returns base offset info
    let info = accessor.field_info("items").unwrap();
    assert_eq!(info.offset, 0);
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
fn accessor_caching_works() {
    let def = Arc::new(create_simple_definition());
    let data = b"TEST\x00\x01HELLO     ";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // First access triggers parse
    let _ = accessor.get("name").unwrap();

    // Verify field_info returns correct values (uses cached offsets)
    let info = accessor.field_info("name").unwrap();
    assert_eq!(info.offset, 6);
    assert_eq!(info.size, 10);
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

    // Verify offsets via field_info
    let info = accessor.field_info("data").unwrap();
    assert_eq!(info.offset, 1);
    assert_eq!(info.size, 5);

    let info = accessor.field_info("trailer").unwrap();
    assert_eq!(info.offset, 6);
    assert_eq!(info.size, 4);
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

    // Verify cached value is correct via field_info
    let info = accessor.field_info("name").unwrap();
    assert_eq!(info.offset, 6);
    assert_eq!(info.size, 10);
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
    let info = accessor1.field_info("trailer").unwrap();
    assert_eq!(info.offset, 11);

    // When condition is false, trailer is at offset 1
    let data_without_extra = b"\x00DONE";
    let accessor2 = StructureAccessor::new(def, data_without_extra).unwrap();
    let info = accessor2.field_info("trailer").unwrap();
    assert_eq!(info.offset, 1);
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

    // Access items as array
    let items = accessor.get("items").unwrap();
    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str().unwrap(), "AAAA");
        assert_eq!(arr[1].as_str().unwrap(), "BBBB");
        assert_eq!(arr[2].as_str().unwrap(), "CCCC");
    } else {
        panic!("Expected array");
    }

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

    // trailer should be at offset 1
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");
}

#[test]
fn accessor_repeat_expr_out_of_bounds() {
    let def = Arc::new(create_repeat_expr_definition());
    let data = b"\x03AAAABBBBCCCCDONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // items should be an array of 3 elements
    let items = accessor.get("items").unwrap();
    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 3);
    } else {
        panic!("Expected array");
    }
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

    // Access items as array
    let items = accessor.get("items").unwrap();
    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0].as_u64().unwrap(), 1);
        assert_eq!(arr[1].as_u64().unwrap(), 2);
        assert_eq!(arr[2].as_u64().unwrap(), 3);
        assert_eq!(arr[3].as_u64().unwrap(), 4);
    } else {
        panic!("Expected array");
    }

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

    // Repeated fields yield the base name once, not expanded indices
    assert!(fields.contains(&"count".to_string()));
    assert!(fields.contains(&"items".to_string()));
    assert!(fields.contains(&"trailer".to_string()));

    // Should NOT contain expanded indexed names
    assert!(!fields.contains(&"items_0".to_string()));
    assert!(!fields.contains(&"items_1".to_string()));
    assert!(!fields.contains(&"items_2".to_string()));
}

// ==================== TypeRef Size Calculation Tests ====================

/// Create a definition with a simple nested type (TypeRef).
fn create_simple_typeref_definition() -> StructureDefinition {
    // Define a nested type with fixed-size fields
    let nested_type = StructureDefinition::new("inner_type")
        .with_field(
            FieldDefinition::new("field_a", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
        .with_field(
            FieldDefinition::new("field_b", FieldType::UnsignedInt(2))
                .with_size(SizeSpec::Fixed(2)),
        );

    // Main structure with a TypeRef field followed by another field
    StructureDefinition::new("test_struct")
        .with_type("inner_type", nested_type)
        .with_field(FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::Fixed(4)))
        .with_field(
            FieldDefinition::new("nested", FieldType::TypeRef("inner_type".to_string()))
                .with_size(SizeSpec::Fixed(0)), // Size comes from type
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_typeref_simple_nested_type() {
    let def = Arc::new(create_simple_typeref_definition());
    // header="HEAD", nested.field_a="AAAA", nested.field_b=0x0001, trailer="DONE"
    let data = b"HEADAAAA\x00\x01DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access header
    let header = accessor.get("header").unwrap();
    assert_eq!(header.as_str().unwrap(), "HEAD");

    // Access nested field
    let nested = accessor.get("nested").unwrap();
    assert!(nested.is_struct());

    // Access trailer - this verifies TypeRef size was calculated correctly
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    // Verify offsets
    let (offset, size) = accessor.calculate_field_offset("header", None).unwrap();
    assert_eq!(offset, 0);
    assert_eq!(size, 4);

    let (offset, size) = accessor.calculate_field_offset("nested", None).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(size, 6); // 4 bytes for field_a + 2 bytes for field_b

    let (offset, size) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 10);
    assert_eq!(size, 4);
}

/// Create a definition with a nested type containing conditional fields.
/// Note: This tests the case where the conditional field IS present.
/// The current implementation's get_type_size doesn't dynamically evaluate
/// conditionals within nested types during offset calculation, so we test
/// the case where all fields are present.
fn create_conditional_typeref_definition() -> StructureDefinition {
    use crate::parser::expression::ExpressionEvaluator;

    // Nested type with a conditional field (like band_info_type)
    let condition = ExpressionEvaluator::parse("has_lut == 1").unwrap();
    let nested_type = StructureDefinition::new("band_type")
        .with_field(
            FieldDefinition::new("band_id", FieldType::String).with_size(SizeSpec::Fixed(2)),
        )
        .with_field(
            FieldDefinition::new("has_lut", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1)),
        )
        .with_field(
            FieldDefinition::new("lut_data", FieldType::Bytes)
                .with_size(SizeSpec::Fixed(4))
                .with_condition(condition),
        );

    StructureDefinition::new("test_struct")
        .with_type("band_type", nested_type)
        .with_field(FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::Fixed(4)))
        .with_field(
            FieldDefinition::new("band", FieldType::TypeRef("band_type".to_string()))
                .with_size(SizeSpec::Fixed(0)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_typeref_with_conditional_present() {
    let def = Arc::new(create_conditional_typeref_definition());
    // header="HEAD", band.band_id="AB", band.has_lut=1, band.lut_data=4 bytes, trailer="DONE"
    let data = b"HEADAB\x01\x00\x00\x00\x00DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access header
    let header = accessor.get("header").unwrap();
    assert_eq!(header.as_str().unwrap(), "HEAD");

    // Access trailer - verifies conditional field was included in size
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    // Verify band offset and size (2 + 1 + 4 = 7 bytes when conditional is present)
    let (offset, size) = accessor.calculate_field_offset("band", None).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(size, 7);

    // Verify trailer offset
    let (offset, _) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 11);
}

/// Test that nested types with conditionals work when the conditional is absent.
/// This uses a fixed-size nested type to avoid the complexity of dynamic
/// conditional evaluation during offset calculation.
fn create_fixed_nested_type_definition() -> StructureDefinition {
    // Nested type with all fixed-size fields (no conditionals)
    let nested_type = StructureDefinition::new("band_type")
        .with_field(
            FieldDefinition::new("band_id", FieldType::String).with_size(SizeSpec::Fixed(2)),
        )
        .with_field(
            FieldDefinition::new("band_value", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1)),
        );

    StructureDefinition::new("test_struct")
        .with_type("band_type", nested_type)
        .with_field(FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::Fixed(4)))
        .with_field(
            FieldDefinition::new("band", FieldType::TypeRef("band_type".to_string()))
                .with_size(SizeSpec::Fixed(0)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_typeref_fixed_nested_type() {
    let def = Arc::new(create_fixed_nested_type_definition());
    // header="HEAD", band.band_id="AB", band.band_value=5, trailer="DONE"
    let data = b"HEADAB\x05DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access header
    let header = accessor.get("header").unwrap();
    assert_eq!(header.as_str().unwrap(), "HEAD");

    // Access trailer - verifies TypeRef size was calculated correctly
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    // Verify band offset and size (2 + 1 = 3 bytes)
    let (offset, size) = accessor.calculate_field_offset("band", None).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(size, 3);

    // Verify trailer offset
    let (offset, _) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 7);
}

/// Create a definition with repeated TypeRef fields.
fn create_repeated_typeref_definition() -> StructureDefinition {
    use crate::parser::expression::ExpressionEvaluator;

    // Simple nested type
    let nested_type = StructureDefinition::new("item_type")
        .with_field(FieldDefinition::new("name", FieldType::String).with_size(SizeSpec::Fixed(4)))
        .with_field(
            FieldDefinition::new("value", FieldType::UnsignedInt(2)).with_size(SizeSpec::Fixed(2)),
        );

    let repeat_expr = ExpressionEvaluator::parse("count").unwrap();

    StructureDefinition::new("test_struct")
        .with_type("item_type", nested_type)
        .with_field(
            FieldDefinition::new("count", FieldType::UnsignedInt(1)).with_size(SizeSpec::Fixed(1)),
        )
        .with_field(
            FieldDefinition::new("items", FieldType::TypeRef("item_type".to_string()))
                .with_size(SizeSpec::Fixed(0))
                .with_repeat(RepeatSpec::Expression(repeat_expr)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_typeref_repeated() {
    let def = Arc::new(create_repeated_typeref_definition());
    // count=2, items=[{name="AAAA", value=1}, {name="BBBB", value=2}], trailer="DONE"
    let data = b"\x02AAAA\x00\x01BBBB\x00\x02DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access count
    let count = accessor.get("count").unwrap();
    assert_eq!(count.as_u64().unwrap(), 2);

    // Access items as array
    let items = accessor.get("items").unwrap();
    if let Value::Array(arr) = items {
        assert_eq!(arr.len(), 2);
        assert!(arr[0].is_struct());
        assert!(arr[1].is_struct());
    } else {
        panic!("Expected array");
    }

    // Access trailer - verifies repeated TypeRef total size was calculated correctly
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    // Verify trailer offset: 1 (count) + 2 * 6 (items) = 13
    let (offset, _) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 13);
}

#[test]
fn accessor_typeref_repeated_zero_count() {
    let def = Arc::new(create_repeated_typeref_definition());
    // count=0, no items, trailer="DONE"
    let data = b"\x00DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Access count
    let count = accessor.get("count").unwrap();
    assert_eq!(count.as_u64().unwrap(), 0);

    // Access trailer - should be at offset 1
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    let (offset, _) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 1);
}

/// Create a definition that references a non-existent type.
fn create_unknown_typeref_definition() -> StructureDefinition {
    StructureDefinition::new("test_struct")
        .with_field(FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::Fixed(4)))
        .with_field(
            FieldDefinition::new(
                "unknown",
                FieldType::TypeRef("nonexistent_type".to_string()),
            )
            .with_size(SizeSpec::Fixed(0)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_typeref_unknown_type_error() {
    let def = Arc::new(create_unknown_typeref_definition());
    let data = b"HEADXXXXDONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    // Accessing the unknown TypeRef field should return an error
    let result = accessor.get("unknown");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AccessError::UnknownField { .. }
    ));

    // Accessing trailer should also fail because offset calculation fails
    let result = accessor.get("trailer");
    assert!(result.is_err());
}

#[test]
fn accessor_typeref_fields_iterator() {
    let def = Arc::new(create_repeated_typeref_definition());
    // count=2, items=[{...}, {...}], trailer="DONE"
    let data = b"\x02AAAA\x00\x01BBBB\x00\x02DONE";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let fields: Vec<String> = accessor.fields().collect();

    // Repeated fields yield the base name once, not expanded indices
    assert!(fields.contains(&"count".to_string()));
    assert!(fields.contains(&"items".to_string()));
    assert!(fields.contains(&"trailer".to_string()));

    // Should NOT contain expanded indexed names
    assert!(!fields.contains(&"items_0".to_string()));
    assert!(!fields.contains(&"items_1".to_string()));
}

// ==================== Integration Tests for Image Subheader Parsing ====================
// These tests verify that fields after repeated TypeRef arrays (like band_info)
// are accessible, specifically testing TRE field access (udidl, ixshdl, udid, ixshd).

/// Helper function to create synthetic NITF image subheader test data.
/// This creates a minimal valid image subheader with band_info followed by TRE fields.
fn create_image_subheader_test_data(nbands: u8, udidl: u16, ixshdl: u16) -> Vec<u8> {
    let mut data = Vec::new();

    // IM (2) - Image segment marker
    data.extend_from_slice(b"IM");

    // IID1 (10) - Image identifier 1
    data.extend_from_slice(b"TestImage ");

    // IDATIM (14) - Image date and time
    data.extend_from_slice(b"20240101120000");

    // TGTID (17) - Target identifier
    data.extend_from_slice(b"                 ");

    // IID2 (80) - Image identifier 2
    data.extend_from_slice(&[b' '; 80]);

    // Security fields
    // ISCLAS (1)
    data.push(b'U');
    // ISCLSY (2)
    data.extend_from_slice(b"  ");
    // ISCODE (11)
    data.extend_from_slice(b"           ");
    // ISCTLH (2)
    data.extend_from_slice(b"  ");
    // ISREL (20)
    data.extend_from_slice(b"                    ");
    // ISDCTP (2)
    data.extend_from_slice(b"  ");
    // ISDCDT (8)
    data.extend_from_slice(b"        ");
    // ISDCXM (4)
    data.extend_from_slice(b"    ");
    // ISDG (1)
    data.push(b' ');
    // ISDGDT (8)
    data.extend_from_slice(b"        ");
    // ISCLTX (43)
    data.extend_from_slice(&[b' '; 43]);
    // ISCATP (1)
    data.push(b' ');
    // ISCAUT (40)
    data.extend_from_slice(&[b' '; 40]);
    // ISCRSN (1)
    data.push(b' ');
    // ISSRDT (8)
    data.extend_from_slice(b"        ");
    // ISCTLN (15)
    data.extend_from_slice(b"               ");

    // ENCRYP (1)
    data.push(b'0');

    // ISORCE (42)
    data.extend_from_slice(&[b' '; 42]);

    // NROWS (8)
    data.extend_from_slice(b"00000512");

    // NCOLS (8)
    data.extend_from_slice(b"00000512");

    // PVTYPE (3)
    data.extend_from_slice(b"INT");

    // IREP (8)
    data.extend_from_slice(b"MONO    ");

    // ICAT (8)
    data.extend_from_slice(b"VIS     ");

    // ABPP (2)
    data.extend_from_slice(b"08");

    // PJUST (1)
    data.push(b'R');

    // ICORDS (1) - Using blank to skip IGEOLO
    data.push(b' ');

    // NICOM (1) - No comments
    data.push(b'0');

    // IC (2) - No compression
    data.extend_from_slice(b"NC");

    // NBANDS (1)
    data.push(b'0' + nbands);

    // Band info for each band (when NBANDS > 0)
    for _ in 0..nbands {
        // IREPBAND (2)
        data.extend_from_slice(b"M ");
        // ISUBCAT (6)
        data.extend_from_slice(b"      ");
        // IFC (1)
        data.push(b'N');
        // IMFLT (3)
        data.extend_from_slice(b"   ");
        // NLUTS (1) - No LUTs
        data.push(b'0');
        // Note: NELUT and LUT_DATA are conditional on NLUTS > 0
    }

    // ISYNC (1)
    data.push(b'0');

    // IMODE (1)
    data.push(b'B');

    // NBPR (4)
    data.extend_from_slice(b"0001");

    // NBPC (4)
    data.extend_from_slice(b"0001");

    // NPPBH (4)
    data.extend_from_slice(b"0512");

    // NPPBV (4)
    data.extend_from_slice(b"0512");

    // NBPP (2)
    data.extend_from_slice(b"08");

    // IDLVL (3)
    data.extend_from_slice(b"001");

    // IALVL (3)
    data.extend_from_slice(b"000");

    // ILOC (10)
    data.extend_from_slice(b"0000000000");

    // IMAG (4)
    data.extend_from_slice(b"1.0 ");

    // UDIDL (5) - User defined image data length
    data.extend_from_slice(format!("{:05}", udidl).as_bytes());

    // UDOFL (3) and UDID (udidl - 3) - conditional on udidl > 0
    if udidl > 0 {
        // UDOFL (3)
        data.extend_from_slice(b"000");
        // UDID - TRE data
        let udid_len = udidl as usize - 3;
        // Create a simple TRE-like structure: CETAG (6) + CEL (5) + data
        if udid_len >= 11 {
            data.extend_from_slice(b"TESTTR"); // CETAG
            let tre_data_len = udid_len - 11;
            data.extend_from_slice(format!("{:05}", tre_data_len).as_bytes()); // CEL
            data.extend_from_slice(&vec![b'X'; tre_data_len]); // TRE data
        } else {
            data.extend_from_slice(&vec![b'X'; udid_len]);
        }
    }

    // IXSHDL (5) - Image extended subheader data length
    data.extend_from_slice(format!("{:05}", ixshdl).as_bytes());

    // IXSOFL (3) and IXSHD (ixshdl - 3) - conditional on ixshdl > 0
    if ixshdl > 0 {
        // IXSOFL (3)
        data.extend_from_slice(b"000");
        // IXSHD - TRE data
        let ixshd_len = ixshdl as usize - 3;
        // Create a simple TRE-like structure
        if ixshd_len >= 11 {
            data.extend_from_slice(b"RPC00B"); // CETAG
            let tre_data_len = ixshd_len - 11;
            data.extend_from_slice(format!("{:05}", tre_data_len).as_bytes()); // CEL
            data.extend_from_slice(&vec![b'Y'; tre_data_len]); // TRE data
        } else {
            data.extend_from_slice(&vec![b'Y'; ixshd_len]);
        }
    }

    data
}

#[test]
fn integration_image_subheader_tre_fields_accessible() {
    use crate::parser::StructureRegistry;

    // Load the image subheader definition from the registry
    let registry = StructureRegistry::new();
    let definition = registry.get("nitf_02.10_image_subheader");

    // Skip test if definition not available (e.g., in CI without data files)
    let definition = match definition {
        Some(def) => def,
        None => {
            eprintln!("Skipping test: nitf_02.10_image_subheader definition not found");
            return;
        }
    };

    // Create test data with 2 bands and TRE data in both UDID and IXSHD
    let test_data = create_image_subheader_test_data(2, 20, 25);

    let accessor = StructureAccessor::new(definition, &test_data).unwrap();

    // Verify basic fields are accessible
    let im = accessor.get("IM").unwrap();
    assert_eq!(im.as_str().unwrap(), "IM");

    let nbands = accessor.get("NBANDS").unwrap();
    assert_eq!(nbands.as_str().unwrap(), "2");

    // Verify band_info is accessible as array
    assert!(accessor.has("BAND_INFO"), "BAND_INFO should be accessible");
    let band_info = accessor.get("BAND_INFO").unwrap();
    if let Value::Array(arr) = band_info {
        assert_eq!(arr.len(), 2, "BAND_INFO should have 2 elements");
    } else {
        panic!("Expected BAND_INFO to be an array");
    }

    // CRITICAL: Verify TRE fields AFTER band_info are accessible
    // This is the main bug fix verification

    // UDIDL should be accessible
    assert!(
        accessor.has("UDIDL"),
        "UDIDL should be accessible after band_info"
    );
    let udidl = accessor.get("UDIDL").unwrap();
    assert_eq!(udidl.as_str().unwrap(), "00020");

    // UDOFL should be accessible (since udidl > 0)
    assert!(
        accessor.has("UDOFL"),
        "UDOFL should be accessible when UDIDL > 0"
    );
    let udofl = accessor.get("UDOFL").unwrap();
    assert_eq!(udofl.as_str().unwrap(), "000");

    // UDID should be accessible (since udidl > 0)
    assert!(
        accessor.has("UDID"),
        "UDID should be accessible when UDIDL > 0"
    );
    let udid = accessor.get("UDID").unwrap();
    // UDID is raw bytes, verify it has the expected length (udidl - 3 = 17)
    assert_eq!(udid.as_bytes().len(), 17);

    // IXSHDL should be accessible
    assert!(
        accessor.has("IXSHDL"),
        "IXSHDL should be accessible after UDID"
    );
    let ixshdl = accessor.get("IXSHDL").unwrap();
    assert_eq!(ixshdl.as_str().unwrap(), "00025");

    // IXSOFL should be accessible (since ixshdl > 0)
    assert!(
        accessor.has("IXSOFL"),
        "IXSOFL should be accessible when IXSHDL > 0"
    );
    let ixsofl = accessor.get("IXSOFL").unwrap();
    assert_eq!(ixsofl.as_str().unwrap(), "000");

    // IXSHD should be accessible (since ixshdl > 0)
    assert!(
        accessor.has("IXSHD"),
        "IXSHD should be accessible when IXSHDL > 0"
    );
    let ixshd = accessor.get("IXSHD").unwrap();
    // IXSHD is raw bytes, verify it has the expected length (ixshdl - 3 = 22)
    assert_eq!(ixshd.as_bytes().len(), 22);
}

#[test]
fn integration_image_subheader_no_tre_data() {
    use crate::parser::StructureRegistry;

    // Load the image subheader definition from the registry
    let registry = StructureRegistry::new();
    let definition = match registry.get("nitf_02.10_image_subheader") {
        Some(def) => def,
        None => {
            eprintln!("Skipping test: nitf_02.10_image_subheader definition not found");
            return;
        }
    };

    // Create test data with 1 band and NO TRE data
    let test_data = create_image_subheader_test_data(1, 0, 0);

    let accessor = StructureAccessor::new(definition, &test_data).unwrap();

    // Verify band_info is accessible as array with 1 element
    assert!(accessor.has("BAND_INFO"), "BAND_INFO should be accessible");
    let band_info = accessor.get("BAND_INFO").unwrap();
    if let Value::Array(arr) = band_info {
        assert_eq!(arr.len(), 1, "BAND_INFO should have 1 element");
    } else {
        panic!("Expected BAND_INFO to be an array");
    }

    // UDIDL should be accessible and be 0
    assert!(accessor.has("UDIDL"), "UDIDL should be accessible");
    let udidl = accessor.get("UDIDL").unwrap();
    assert_eq!(udidl.as_str().unwrap(), "00000");

    // UDOFL and UDID should NOT be accessible (since udidl = 0)
    assert!(
        !accessor.has("UDOFL"),
        "UDOFL should NOT be accessible when UDIDL = 0"
    );
    assert!(
        !accessor.has("UDID"),
        "UDID should NOT be accessible when UDIDL = 0"
    );

    // IXSHDL should be accessible and be 0
    assert!(accessor.has("IXSHDL"), "IXSHDL should be accessible");
    let ixshdl = accessor.get("IXSHDL").unwrap();
    assert_eq!(ixshdl.as_str().unwrap(), "00000");

    // IXSOFL and IXSHD should NOT be accessible (since ixshdl = 0)
    assert!(
        !accessor.has("IXSOFL"),
        "IXSOFL should NOT be accessible when IXSHDL = 0"
    );
    assert!(
        !accessor.has("IXSHD"),
        "IXSHD should NOT be accessible when IXSHDL = 0"
    );
}

#[test]
fn integration_image_subheader_field_iterator_completeness() {
    use crate::parser::StructureRegistry;

    // Load the image subheader definition from the registry
    let registry = StructureRegistry::new();
    let definition = match registry.get("nitf_02.10_image_subheader") {
        Some(def) => def,
        None => {
            eprintln!("Skipping test: nitf_02.10_image_subheader definition not found");
            return;
        }
    };

    // Create test data with 2 bands and TRE data
    let test_data = create_image_subheader_test_data(2, 20, 25);

    let accessor = StructureAccessor::new(definition, &test_data).unwrap();

    // Collect all field paths
    let fields: Vec<String> = accessor.fields().collect();

    // Verify TRE-related fields are included in the iterator
    assert!(
        fields.contains(&"UDIDL".to_string()),
        "fields() should include UDIDL"
    );
    assert!(
        fields.contains(&"UDOFL".to_string()),
        "fields() should include UDOFL (when UDIDL > 0)"
    );
    assert!(
        fields.contains(&"UDID".to_string()),
        "fields() should include UDID (when UDIDL > 0)"
    );
    assert!(
        fields.contains(&"IXSHDL".to_string()),
        "fields() should include IXSHDL"
    );
    assert!(
        fields.contains(&"IXSOFL".to_string()),
        "fields() should include IXSOFL (when IXSHDL > 0)"
    );
    assert!(
        fields.contains(&"IXSHD".to_string()),
        "fields() should include IXSHD (when IXSHDL > 0)"
    );

    // Verify band_info field is included (base name, not expanded)
    assert!(
        fields.contains(&"BAND_INFO".to_string()),
        "fields() should include BAND_INFO"
    );
    assert!(
        !fields.contains(&"BAND_INFO_0".to_string()),
        "fields() should NOT include BAND_INFO_0"
    );
    assert!(
        !fields.contains(&"BAND_INFO_1".to_string()),
        "fields() should NOT include BAND_INFO_1"
    );

    // Verify fields before band_info are included
    assert!(
        fields.contains(&"IM".to_string()),
        "fields() should include IM"
    );
    assert!(
        fields.contains(&"NBANDS".to_string()),
        "fields() should include NBANDS"
    );

    // Verify fields after band_info but before TRE fields are included
    assert!(
        fields.contains(&"ISYNC".to_string()),
        "fields() should include ISYNC"
    );
    assert!(
        fields.contains(&"IMODE".to_string()),
        "fields() should include IMODE"
    );
    assert!(
        fields.contains(&"IMAG".to_string()),
        "fields() should include IMAG"
    );
}

#[test]
fn integration_image_subheader_field_iterator_no_tre() {
    use crate::parser::StructureRegistry;

    // Load the image subheader definition from the registry
    let registry = StructureRegistry::new();
    let definition = match registry.get("nitf_02.10_image_subheader") {
        Some(def) => def,
        None => {
            eprintln!("Skipping test: nitf_02.10_image_subheader definition not found");
            return;
        }
    };

    // Create test data with 1 band and NO TRE data
    let test_data = create_image_subheader_test_data(1, 0, 0);

    let accessor = StructureAccessor::new(definition, &test_data).unwrap();

    // Collect all field paths
    let fields: Vec<String> = accessor.fields().collect();

    // Verify UDIDL and IXSHDL are included (they're always present)
    assert!(
        fields.contains(&"UDIDL".to_string()),
        "fields() should include UDIDL"
    );
    assert!(
        fields.contains(&"IXSHDL".to_string()),
        "fields() should include IXSHDL"
    );

    // Verify conditional TRE fields are NOT included when their conditions are false
    assert!(
        !fields.contains(&"UDOFL".to_string()),
        "fields() should NOT include UDOFL when UDIDL = 0"
    );
    assert!(
        !fields.contains(&"UDID".to_string()),
        "fields() should NOT include UDID when UDIDL = 0"
    );
    assert!(
        !fields.contains(&"IXSOFL".to_string()),
        "fields() should NOT include IXSOFL when IXSHDL = 0"
    );
    assert!(
        !fields.contains(&"IXSHD".to_string()),
        "fields() should NOT include IXSHD when IXSHDL = 0"
    );
}

/// Create a definition with a nested type containing expression-based sizes.
/// This tests the case where a field's size depends on a previous field's value
/// within the same nested type (like LUT_DATA depending on NELUT).
fn create_expression_size_nested_type_definition() -> StructureDefinition {
    use crate::parser::expression::ExpressionEvaluator;

    // Nested type with expression-based size (like band_info_type with LUT)
    let nluts_condition = ExpressionEvaluator::parse("nluts > 0").unwrap();
    let nelut_size_expr = ExpressionEvaluator::parse("nelut").unwrap();
    let nluts_repeat_expr = ExpressionEvaluator::parse("nluts").unwrap();

    let nested_type = StructureDefinition::new("band_type")
        .with_field(
            FieldDefinition::new("band_id", FieldType::String).with_size(SizeSpec::Fixed(2)),
        )
        .with_field(
            FieldDefinition::new("nluts", FieldType::UnsignedInt(1)).with_size(SizeSpec::Fixed(1)),
        )
        .with_field(
            FieldDefinition::new("nelut", FieldType::UnsignedInt(2))
                .with_size(SizeSpec::Fixed(2))
                .with_condition(nluts_condition.clone()),
        )
        .with_field(
            FieldDefinition::new("lut_data", FieldType::Bytes)
                .with_size(SizeSpec::Expression(nelut_size_expr))
                .with_repeat(RepeatSpec::Expression(nluts_repeat_expr))
                .with_condition(nluts_condition),
        );

    StructureDefinition::new("test_struct")
        .with_type("band_type", nested_type)
        .with_field(FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::Fixed(4)))
        .with_field(
            FieldDefinition::new("band", FieldType::TypeRef("band_type".to_string()))
                .with_size(SizeSpec::Fixed(0)),
        )
        .with_field(
            FieldDefinition::new("trailer", FieldType::String).with_size(SizeSpec::Fixed(4)),
        )
}

#[test]
fn accessor_typeref_with_expression_size_lut_present() {
    let def = Arc::new(create_expression_size_nested_type_definition());

    // Create test data:
    // header="HEAD" (4 bytes)
    // band.band_id="AB" (2 bytes)
    // band.nluts=2 (1 byte)
    // band.nelut=4 (2 bytes, big-endian)
    // band.lut_data=4 bytes × 2 LUTs = 8 bytes
    // trailer="DONE" (4 bytes)
    let mut data = Vec::new();
    data.extend_from_slice(b"HEAD"); // header
    data.extend_from_slice(b"AB"); // band_id
    data.push(2); // nluts = 2
    data.extend_from_slice(&4u16.to_be_bytes()); // nelut = 4
    data.extend_from_slice(b"LUT1"); // lut_data[0] = 4 bytes
    data.extend_from_slice(b"LUT2"); // lut_data[1] = 4 bytes
    data.extend_from_slice(b"DONE"); // trailer

    let accessor = StructureAccessor::new(def, &data).unwrap();

    // Access header
    let header = accessor.get("header").unwrap();
    assert_eq!(header.as_str().unwrap(), "HEAD");

    // Access trailer - this verifies the nested type size was calculated correctly
    // including the expression-based LUT_DATA size
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    // Verify band offset and size
    // band_id(2) + nluts(1) + nelut(2) + lut_data(4*2=8) = 13 bytes
    let (offset, size) = accessor.calculate_field_offset("band", None).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(size, 13);

    // Verify trailer offset
    let (offset, _) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 17); // 4 (header) + 13 (band) = 17
}

#[test]
fn accessor_typeref_with_expression_size_no_lut() {
    let def = Arc::new(create_expression_size_nested_type_definition());

    // Create test data with nluts=0 (no LUT data):
    // header="HEAD" (4 bytes)
    // band.band_id="AB" (2 bytes)
    // band.nluts=0 (1 byte)
    // (no nelut or lut_data since nluts=0)
    // trailer="DONE" (4 bytes)
    let mut data = Vec::new();
    data.extend_from_slice(b"HEAD"); // header
    data.extend_from_slice(b"AB"); // band_id
    data.push(0); // nluts = 0
    data.extend_from_slice(b"DONE"); // trailer

    let accessor = StructureAccessor::new(def, &data).unwrap();

    // Access header
    let header = accessor.get("header").unwrap();
    assert_eq!(header.as_str().unwrap(), "HEAD");

    // Access trailer - verifies conditional fields were skipped correctly
    let trailer = accessor.get("trailer").unwrap();
    assert_eq!(trailer.as_str().unwrap(), "DONE");

    // Verify band offset and size
    // band_id(2) + nluts(1) = 3 bytes (no nelut or lut_data)
    let (offset, size) = accessor.calculate_field_offset("band", None).unwrap();
    assert_eq!(offset, 4);
    assert_eq!(size, 3);

    // Verify trailer offset
    let (offset, _) = accessor.calculate_field_offset("trailer", None).unwrap();
    assert_eq!(offset, 7); // 4 (header) + 3 (band) = 7
}

#[test]
fn accessor_reads_bcs_npi_field_with_plus_sign() {
    // Simulates the RPC00B HEIGHT_SCALE case: BCS-NPI field containing a leading +
    let def = Arc::new(
        StructureDefinition::new("rpc_snippet")
            .with_field(
                FieldDefinition::new("LONG_SCALE", FieldType::String)
                    .with_size(SizeSpec::Fixed(9))
                    .with_encoding(Encoding::BcsN),
            )
            .with_field(
                FieldDefinition::new("HEIGHT_SCALE", FieldType::String)
                    .with_size(SizeSpec::Fixed(5))
                    .with_encoding(Encoding::BcsNPI),
            )
            .with_field(
                FieldDefinition::new("NEXT_FIELD", FieldType::String)
                    .with_size(SizeSpec::Fixed(12))
                    .with_encoding(Encoding::BcsA),
            ),
    );
    let data = b"+000.1012+0697+1.00000E+00";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let height_scale = accessor.get("HEIGHT_SCALE").unwrap();
    assert_eq!(height_scale.as_str().unwrap(), "+0697");

    let next = accessor.get("NEXT_FIELD").unwrap();
    assert_eq!(next.as_str().unwrap(), "+1.00000E+00");

    assert!(accessor.has("HEIGHT_SCALE"));
    let fields: Vec<String> = accessor.fields().collect();
    assert!(fields.contains(&"HEIGHT_SCALE".to_string()));
}

// ==================== SizeSpec::Eos Tests ====================

#[test]
fn accessor_size_eos_string_returns_remaining_bytes() {
    let def = Arc::new(
        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::Fixed(4)),
            )
            .with_field(
                FieldDefinition::new("comment", FieldType::String).with_size(SizeSpec::Eos),
            ),
    );
    let data = b"HDRRsome comment text here";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let header = accessor.get("header").unwrap();
    assert_eq!(header.as_str().unwrap(), "HDRR");

    let comment = accessor.get("comment").unwrap();
    assert_eq!(comment.as_str().unwrap(), "some comment text here");
}

#[test]
fn accessor_size_eos_empty_data() {
    let def =
        Arc::new(StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("comment", FieldType::String).with_size(SizeSpec::Eos),
        ));
    let data = b"";
    let accessor = StructureAccessor::new(def, data).unwrap();

    let comment = accessor.get("comment").unwrap();
    assert_eq!(comment.as_str().unwrap(), "");
}
