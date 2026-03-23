//! Property-based tests for StructureAccessor.

use super::*;
use crate::parser::expression::ExpressionEvaluator;
use proptest::prelude::*;

/// Property 7: Conditional Field Presence
/// For any structure with a conditional field, the field SHALL be accessible
/// via `get()` and `has()` if and only if its condition expression evaluates to true.
/// **Validates: Requirements 3.2, 3.3, 3.4, 3.5**
mod prop_7_conditional_field_presence {
    use super::*;

    /// Create a structure definition with a conditional field based on a flag
    fn create_conditional_def(threshold: u8) -> StructureDefinition {
        let condition = ExpressionEvaluator::parse(&format!("flag >= {}", threshold)).unwrap();

        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("flag", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1)),
            )
            .with_field(
                FieldDefinition::new("conditional_data", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_condition(condition),
            )
            .with_field(
                FieldDefinition::new("always_present", FieldType::String)
                    .with_size(SizeSpec::Fixed(4)),
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// When condition is true, field is accessible via get() and has()
        #[test]
        fn conditional_field_accessible_when_true(
            flag in 128u8..=255u8,  // Always >= 128
        ) {
            let def = Arc::new(create_conditional_def(128));
            // flag (1 byte) + conditional_data (8 bytes) + always_present (4 bytes)
            let mut data = vec![flag];
            data.extend_from_slice(b"CONDDATA");
            data.extend_from_slice(b"DONE");

            let accessor = StructureAccessor::new(def, &data).unwrap();

            // has() should return true
            prop_assert!(accessor.has("conditional_data"),
                "has() should return true when condition is met (flag={})", flag);

            // get() should succeed
            let result = accessor.get("conditional_data");
            prop_assert!(result.is_ok(),
                "get() should succeed when condition is met (flag={})", flag);

            // Value should be correct
            let value = result.unwrap();
            prop_assert_eq!(value.as_str().unwrap(), "CONDDATA");
        }
    }
}


/// Property 8: Expression-Based Repetition Count
/// For any repeated field with `repeat: expr`, the number of elements in the
/// returned `Value::Array` SHALL equal the evaluated repeat-expr value.
/// **Validates: Requirements 4.1, 4.6**
mod prop_8_expression_based_repetition {
    use super::*;

    fn create_repeat_expr_def() -> StructureDefinition {
        let repeat_expr = ExpressionEvaluator::parse("count").unwrap();

        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("count", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1)),
            )
            .with_field(
                FieldDefinition::new("items", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1))
                    .with_repeat(RepeatSpec::Expression(repeat_expr)),
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Number of accessible elements equals repeat-expr value
        #[test]
        fn element_count_equals_expr(count in 0u8..20u8) {
            let def = Arc::new(create_repeat_expr_def());

            // Create data: count byte + count item bytes
            let mut data = vec![count];
            for i in 0..count {
                data.push(i);
            }

            let accessor = StructureAccessor::new(def, &data).unwrap();

            // Access items as array and check length
            let items = accessor.get("items").unwrap();
            if let Value::Array(arr) = items {
                prop_assert_eq!(arr.len(), count as usize,
                    "Expected {} elements, found {}", count, arr.len());
            } else {
                prop_assert!(false, "Expected array value");
            }
        }

        /// Array access returns correct number of elements
        #[test]
        fn array_has_correct_length(count in 0u8..20u8) {
            let def = Arc::new(create_repeat_expr_def());

            let mut data = vec![count];
            for i in 0..count {
                data.push(i);
            }

            let accessor = StructureAccessor::new(def, &data).unwrap();

            let items = accessor.get("items").unwrap();
            if let Value::Array(arr) = items {
                prop_assert_eq!(arr.len(), count as usize,
                    "Array length should be {}", count);
            } else {
                prop_assert!(false, "Expected array value");
            }
        }
    }
}

/// Property 9: Until-Condition Repetition
/// For any repeated field with `repeat: until`, parsing SHALL stop when the
/// until-condition evaluates to true, and the last element SHALL be the one
/// that satisfied the condition.
/// **Validates: Requirements 4.2**
mod prop_9_until_condition_repetition {
    use super::*;

    fn create_repeat_until_def() -> StructureDefinition {
        // Repeat until we see a zero byte
        let until_expr = ExpressionEvaluator::parse("_ == 0").unwrap();

        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Until(until_expr)),
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Parsing stops at the terminator element
        #[test]
        fn stops_at_terminator(
            prefix_len in 1usize..10usize,
            prefix_values in proptest::collection::vec(1u8..255u8, 1..10),
        ) {
            let def = Arc::new(create_repeat_until_def());

            // Create data: non-zero values followed by zero terminator
            let actual_len = prefix_len.min(prefix_values.len());
            let mut data: Vec<u8> = prefix_values.into_iter().take(actual_len).collect();
            data.push(0); // Terminator
            data.push(99); // Extra data that should not be parsed

            let accessor = StructureAccessor::new(def, &data).unwrap();

            let items = accessor.get("items").unwrap();
            if let Value::Array(arr) = items {
                // Should include all prefix values plus the terminator
                prop_assert_eq!(arr.len(), actual_len + 1,
                    "Expected {} elements (including terminator)", actual_len + 1);

                // Last element should be the terminator (0)
                let last = arr.last().unwrap();
                prop_assert_eq!(last.as_u64().unwrap(), 0,
                    "Last element should be the terminator (0)");
            } else {
                prop_assert!(false, "Expected array value");
            }
        }

        /// All elements before terminator have correct values
        #[test]
        fn prefix_values_correct(
            values in proptest::collection::vec(1u8..255u8, 1..5),
        ) {
            let def = Arc::new(create_repeat_until_def());

            let mut data: Vec<u8> = values.clone();
            data.push(0); // Terminator

            let accessor = StructureAccessor::new(def, &data).unwrap();

            let items = accessor.get("items").unwrap();
            if let Value::Array(arr) = items {
                // Check all values before terminator
                for (i, expected) in values.iter().enumerate() {
                    let actual = arr[i].as_u64().unwrap();
                    prop_assert_eq!(actual, *expected as u64,
                        "Element {} should be {}", i, expected);
                }
            } else {
                prop_assert!(false, "Expected array value");
            }
        }
    }
}


/// Property 10: End-of-Stream Repetition
/// For any repeated field with `repeat: eos`, the total bytes consumed by all
/// elements SHALL equal the remaining buffer size.
/// **Validates: Requirements 4.3**
mod prop_10_eos_repetition {
    use super::*;

    fn create_repeat_eos_def() -> StructureDefinition {
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Eos),
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// All bytes are consumed as elements
        #[test]
        fn consumes_all_bytes(data in proptest::collection::vec(any::<u8>(), 0..50)) {
            let def = Arc::new(create_repeat_eos_def());
            let accessor = StructureAccessor::new(def, &data).unwrap();

            let items = accessor.get("items").unwrap();
            if let Value::Array(arr) = items {
                // Number of elements should equal number of bytes
                prop_assert_eq!(arr.len(), data.len(),
                    "Expected {} elements for {} bytes", data.len(), data.len());
            } else {
                prop_assert!(false, "Expected array value");
            }
        }

        /// Element values match input bytes
        #[test]
        fn values_match_bytes(data in proptest::collection::vec(any::<u8>(), 1..20)) {
            let def = Arc::new(create_repeat_eos_def());
            let accessor = StructureAccessor::new(def, &data).unwrap();

            let items = accessor.get("items").unwrap();
            if let Value::Array(arr) = items {
                for (i, expected) in data.iter().enumerate() {
                    let actual = arr[i].as_u64().unwrap();
                    prop_assert_eq!(actual, *expected as u64,
                        "Element {} should be {}", i, expected);
                }
            } else {
                prop_assert!(false, "Expected array value");
            }
        }
    }
}

/// Feature: metadata-restructure, Property 5: get() Returns Array for Repeated Fields
///
/// Property 11: Repeated Field Array Access
/// For any repeated field with N elements, `get(field_id)` SHALL return a
/// `Value::Array` with N elements, and `get("field_N")` (with underscore index)
/// SHALL return `UnknownField` error since `_N` access is removed.
/// Scalar fields continue to return `Value::String` or `Value::Unsigned` as before.
/// **Validates: Requirements 2.3, 5.1, 5.3**
mod prop_11_repeated_field_array_access {
    use super::*;

    fn create_repeat_count_def(count: usize) -> StructureDefinition {
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(count)),
        )
    }

    fn create_mixed_def(repeat_count: usize, scalar_size: usize) -> StructureDefinition {
        StructureDefinition::new("test_struct")
            .with_field(
                FieldDefinition::new("scalar_str", FieldType::String)
                    .with_size(SizeSpec::Fixed(scalar_size)),
            )
            .with_field(
                FieldDefinition::new("scalar_uint", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1)),
            )
            .with_field(
                FieldDefinition::new("items", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1))
                    .with_repeat(RepeatSpec::Count(repeat_count)),
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// get(field_id) returns Value::Array with correct length
        #[test]
        fn get_returns_array_with_correct_length(count in 0usize..20usize) {
            let def = Arc::new(create_repeat_count_def(count));
            let data: Vec<u8> = (0..count as u8).collect();
            let accessor = StructureAccessor::new(def, &data).unwrap();

            let result = accessor.get("items").unwrap();
            if let Value::Array(arr) = result {
                prop_assert_eq!(arr.len(), count,
                    "Expected array of length {}, got {}", count, arr.len());
            } else {
                prop_assert!(false, "Expected Value::Array, got {:?}", result);
            }
        }

        /// _N indexed paths return UnknownField error
        #[test]
        fn indexed_paths_return_unknown_field(count in 1usize..10usize) {
            let def = Arc::new(create_repeat_count_def(count));
            let data: Vec<u8> = (0..count as u8).collect();
            let accessor = StructureAccessor::new(def, &data).unwrap();

            // _N paths should no longer work
            for i in 0..count {
                let path = format!("items_{}", i);
                prop_assert!(!accessor.has(&path),
                    "Path '{}' should NOT be accessible (indexed access removed)", path);

                let result = accessor.get(&path);
                prop_assert!(result.is_err(),
                    "get('{}') should fail (indexed access removed)", path);

                match result.unwrap_err() {
                    AccessError::UnknownField { .. } => {},
                    other => prop_assert!(false,
                        "Expected UnknownField error, got {:?}", other),
                }
            }
        }

        /// Repeated fields yield base name once (not expanded indices)
        #[test]
        fn base_name_yielded_once(count in 1usize..10usize) {
            let def = Arc::new(create_repeat_count_def(count));
            let data: Vec<u8> = (0..count as u8).collect();
            let accessor = StructureAccessor::new(def, &data).unwrap();

            let fields: Vec<String> = accessor.fields().collect();

            // Should contain the base field name exactly once
            prop_assert!(fields.contains(&"items".to_string()),
                "fields() should contain base name 'items'");
            let base_count = fields.iter().filter(|f| f.as_str() == "items").count();
            prop_assert_eq!(base_count, 1,
                "Base name 'items' should appear exactly once");

            // Should NOT contain any expanded indexed names
            for i in 0..count {
                let indexed_path = format!("items_{}", i);
                prop_assert!(!fields.contains(&indexed_path),
                    "fields() should NOT contain expanded name '{}'", indexed_path);
            }
        }

        /// Scalar fields return Value::String or Value::Unsigned, not Value::Array
        #[test]
        fn scalar_fields_return_non_array(
            scalar_size in 1usize..10usize,
            repeat_count in 0usize..10usize,
        ) {
            let def = Arc::new(create_mixed_def(repeat_count, scalar_size));

            // Build data: scalar_str (scalar_size bytes) + scalar_uint (1 byte) + items (repeat_count bytes)
            let mut data = vec![b'A'; scalar_size];
            data.push(42u8);
            for i in 0..repeat_count as u8 {
                data.push(i);
            }

            let accessor = StructureAccessor::new(def, &data).unwrap();

            // Scalar string field should return Value::String
            let str_val = accessor.get("scalar_str").unwrap();
            match &str_val {
                Value::String(_) => {},
                other => prop_assert!(false,
                    "Expected Value::String for scalar_str, got {:?}", other),
            }

            // Scalar unsigned field should return Value::Unsigned
            let uint_val = accessor.get("scalar_uint").unwrap();
            match &uint_val {
                Value::Unsigned(n) => {
                    prop_assert_eq!(*n, 42u64,
                        "Expected scalar_uint value 42, got {}", n);
                },
                other => prop_assert!(false,
                    "Expected Value::Unsigned for scalar_uint, got {:?}", other),
            }

            // Repeated field should return Value::Array
            let arr_val = accessor.get("items").unwrap();
            match &arr_val {
                Value::Array(arr) => {
                    prop_assert_eq!(arr.len(), repeat_count,
                        "Expected array of length {}, got {}", repeat_count, arr.len());
                },
                other => prop_assert!(false,
                    "Expected Value::Array for items, got {:?}", other),
            }
        }
    }
}


/// Property 1: TypeRef Size Accuracy
/// For any field with `FieldType::TypeRef(type_name)` where `type_name` exists in the
/// definition's types map, `get_simple_field_size()` SHALL return the same size as
/// `get_type_size()` for the same field.
/// **Validates: Requirements 1.1, 1.2, 1.3**
mod prop_1_typeref_size_accuracy {
    use super::*;
    use crate::parser::accessor::context::get_simple_field_size;
    use crate::parser::expression::EvalContext;

    /// Create a structure definition with a simple nested type
    fn create_simple_nested_def(inner_field_size: usize) -> StructureDefinition {
        // Create a nested type with a single fixed-size field
        let nested_type = StructureDefinition::new("inner_type").with_field(
            FieldDefinition::new("inner_field", FieldType::String)
                .with_size(SizeSpec::Fixed(inner_field_size)),
        );

        StructureDefinition::new("test_struct")
            .with_type("inner_type", nested_type)
            .with_field(
                FieldDefinition::new("nested", FieldType::TypeRef("inner_type".to_string()))
                    .with_size(SizeSpec::Fixed(0)), // Size comes from type
            )
    }

    /// Create a structure definition with a nested type containing multiple fields
    fn create_multi_field_nested_def(field1_size: usize, field2_size: usize) -> StructureDefinition {
        let nested_type = StructureDefinition::new("multi_field_type")
            .with_field(
                FieldDefinition::new("field1", FieldType::String)
                    .with_size(SizeSpec::Fixed(field1_size)),
            )
            .with_field(
                FieldDefinition::new("field2", FieldType::String)
                    .with_size(SizeSpec::Fixed(field2_size)),
            );

        StructureDefinition::new("test_struct")
            .with_type("multi_field_type", nested_type)
            .with_field(
                FieldDefinition::new("nested", FieldType::TypeRef("multi_field_type".to_string()))
                    .with_size(SizeSpec::Fixed(0)),
            )
    }

    /// Create a structure definition with a nested type containing conditional fields
    fn create_conditional_nested_def(threshold: u8) -> StructureDefinition {
        let condition = ExpressionEvaluator::parse(&format!("flag >= {}", threshold)).unwrap();

        let nested_type = StructureDefinition::new("conditional_type")
            .with_field(
                FieldDefinition::new("flag", FieldType::UnsignedInt(1))
                    .with_size(SizeSpec::Fixed(1)),
            )
            .with_field(
                FieldDefinition::new("conditional_data", FieldType::String)
                    .with_size(SizeSpec::Fixed(8))
                    .with_condition(condition),
            )
            .with_field(
                FieldDefinition::new("always_present", FieldType::String)
                    .with_size(SizeSpec::Fixed(4)),
            );

        StructureDefinition::new("test_struct")
            .with_type("conditional_type", nested_type)
            .with_field(
                FieldDefinition::new("nested", FieldType::TypeRef("conditional_type".to_string()))
                    .with_size(SizeSpec::Fixed(0)),
            )
    }

    /// Create a structure definition with recursively nested types
    fn create_recursive_nested_def(inner_size: usize) -> StructureDefinition {
        // Inner type
        let inner_type = StructureDefinition::new("inner_type").with_field(
            FieldDefinition::new("inner_field", FieldType::String)
                .with_size(SizeSpec::Fixed(inner_size)),
        );

        // Outer type that contains inner type
        let outer_type = StructureDefinition::new("outer_type")
            .with_field(
                FieldDefinition::new("outer_field", FieldType::String)
                    .with_size(SizeSpec::Fixed(4)),
            )
            .with_field(
                FieldDefinition::new("inner", FieldType::TypeRef("inner_type".to_string()))
                    .with_size(SizeSpec::Fixed(0)),
            );

        StructureDefinition::new("test_struct")
            .with_type("inner_type", inner_type)
            .with_type("outer_type", outer_type)
            .with_field(
                FieldDefinition::new("nested", FieldType::TypeRef("outer_type".to_string()))
                    .with_size(SizeSpec::Fixed(0)),
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Simple nested type: get_simple_field_size matches get_type_size
        #[test]
        fn simple_nested_type_size_matches(inner_size in 1usize..50usize) {
            let def = Arc::new(create_simple_nested_def(inner_size));
            let data: Vec<u8> = vec![b'X'; inner_size + 10]; // Extra padding

            let accessor = StructureAccessor::new(def.clone(), &data).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();

            // Find the nested field
            let field = def.fields.iter().find(|f| f.id == "nested").unwrap();

            // Get size using get_simple_field_size (context.rs)
            let simple_size = get_simple_field_size(field, &ctx, &evaluator, &def, &data, 0).unwrap();

            // Get size using get_type_size (mod.rs) via accessor
            let accessor_size = accessor.get_type_size("inner_type", 0).unwrap();

            prop_assert_eq!(simple_size, accessor_size,
                "get_simple_field_size ({}) should match get_type_size ({}) for inner_size={}",
                simple_size, accessor_size, inner_size);

            // Also verify the size is correct
            prop_assert_eq!(simple_size, inner_size,
                "Size should be {} but got {}", inner_size, simple_size);
        }

        /// Multi-field nested type: sizes are summed correctly
        #[test]
        fn multi_field_nested_type_size_matches(
            field1_size in 1usize..25usize,
            field2_size in 1usize..25usize,
        ) {
            let def = Arc::new(create_multi_field_nested_def(field1_size, field2_size));
            let total_size = field1_size + field2_size;
            let data: Vec<u8> = vec![b'X'; total_size + 10];

            let accessor = StructureAccessor::new(def.clone(), &data).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();

            let field = def.fields.iter().find(|f| f.id == "nested").unwrap();

            let simple_size = get_simple_field_size(field, &ctx, &evaluator, &def, &data, 0).unwrap();
            let accessor_size = accessor.get_type_size("multi_field_type", 0).unwrap();

            prop_assert_eq!(simple_size, accessor_size,
                "get_simple_field_size ({}) should match get_type_size ({}) for field1={}, field2={}",
                simple_size, accessor_size, field1_size, field2_size);

            prop_assert_eq!(simple_size, total_size,
                "Size should be {} but got {}", total_size, simple_size);
        }

        /// Conditional nested type with condition TRUE: includes conditional field
        #[test]
        fn conditional_nested_type_condition_true(flag in 128u8..=255u8) {
            let def = Arc::new(create_conditional_nested_def(128));
            // flag (1) + conditional_data (8) + always_present (4) = 13
            let mut data = vec![flag];
            data.extend_from_slice(b"CONDDATA"); // 8 bytes
            data.extend_from_slice(b"DONE");     // 4 bytes
            data.extend_from_slice(&[0u8; 10]);  // padding

            let accessor = StructureAccessor::new(def.clone(), &data).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();

            let field = def.fields.iter().find(|f| f.id == "nested").unwrap();

            let simple_size = get_simple_field_size(field, &ctx, &evaluator, &def, &data, 0).unwrap();
            let accessor_size = accessor.get_type_size("conditional_type", 0).unwrap();

            prop_assert_eq!(simple_size, accessor_size,
                "get_simple_field_size ({}) should match get_type_size ({}) when condition is true (flag={})",
                simple_size, accessor_size, flag);

            // Expected: flag (1) + conditional_data (8) + always_present (4) = 13
            prop_assert_eq!(simple_size, 13,
                "Size should be 13 when condition is true, got {}", simple_size);
        }

        /// Conditional nested type with condition FALSE: excludes conditional field
        #[test]
        fn conditional_nested_type_condition_false(flag in 0u8..128u8) {
            let def = Arc::new(create_conditional_nested_def(128));
            // flag (1) + always_present (4) = 5 (conditional_data skipped)
            let mut data = vec![flag];
            data.extend_from_slice(b"DONE");     // 4 bytes
            data.extend_from_slice(&[0u8; 10]);  // padding

            let accessor = StructureAccessor::new(def.clone(), &data).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();

            let field = def.fields.iter().find(|f| f.id == "nested").unwrap();

            let simple_size = get_simple_field_size(field, &ctx, &evaluator, &def, &data, 0).unwrap();
            let accessor_size = accessor.get_type_size("conditional_type", 0).unwrap();

            prop_assert_eq!(simple_size, accessor_size,
                "get_simple_field_size ({}) should match get_type_size ({}) when condition is false (flag={})",
                simple_size, accessor_size, flag);

            // Expected: flag (1) + always_present (4) = 5
            prop_assert_eq!(simple_size, 5,
                "Size should be 5 when condition is false, got {}", simple_size);
        }

        /// Recursive nested types: sizes are calculated correctly through nesting
        #[test]
        fn recursive_nested_type_size_matches(inner_size in 1usize..30usize) {
            let def = Arc::new(create_recursive_nested_def(inner_size));
            // outer_field (4) + inner_type (inner_size) = 4 + inner_size
            let total_size = 4 + inner_size;
            let data: Vec<u8> = vec![b'X'; total_size + 10];

            let accessor = StructureAccessor::new(def.clone(), &data).unwrap();
            let evaluator = ExpressionEvaluator::new();
            let ctx = EvalContext::new();

            let field = def.fields.iter().find(|f| f.id == "nested").unwrap();

            let simple_size = get_simple_field_size(field, &ctx, &evaluator, &def, &data, 0).unwrap();
            let accessor_size = accessor.get_type_size("outer_type", 0).unwrap();

            prop_assert_eq!(simple_size, accessor_size,
                "get_simple_field_size ({}) should match get_type_size ({}) for recursive nested type with inner_size={}",
                simple_size, accessor_size, inner_size);

            prop_assert_eq!(simple_size, total_size,
                "Size should be {} but got {}", total_size, simple_size);
        }
    }
}
