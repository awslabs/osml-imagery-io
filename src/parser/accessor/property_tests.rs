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
/// For any repeated field with `repeat: expr`, the number of accessible indexed
/// elements (`field_0`, `field_1`, ...) SHALL equal the evaluated repeat-expr value.
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

            // Count accessible elements
            let mut accessible_count = 0;
            for i in 0..256 {
                let path = format!("items_{}", i);
                if accessor.has(&path) {
                    accessible_count += 1;
                } else {
                    break;
                }
            }

            prop_assert_eq!(accessible_count, count as usize,
                "Expected {} accessible elements, found {}", count, accessible_count);
        }

        /// Elements 0 to count-1 are accessible, count and beyond are not
        #[test]
        fn correct_elements_accessible(count in 1u8..20u8) {
            let def = Arc::new(create_repeat_expr_def());

            let mut data = vec![count];
            for i in 0..count {
                data.push(i);
            }

            let accessor = StructureAccessor::new(def, &data).unwrap();

            // All elements 0 to count-1 should be accessible
            for i in 0..count {
                let path = format!("items_{}", i);
                prop_assert!(accessor.has(&path),
                    "Element {} should be accessible for count={}", i, count);
            }

            // Element at index count should NOT be accessible
            let path = format!("items_{}", count);
            prop_assert!(!accessor.has(&path),
                "Element {} should NOT be accessible for count={}", count, count);
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

/// Property 11: Underscore-Indexed Naming
/// For any repeated field with N elements, paths `field_0` through `field_{N-1}`
/// SHALL be accessible, and `field_N` SHALL return UnknownField error.
/// **Validates: Requirements 4.4, 4.5**
mod prop_11_underscore_indexed_naming {
    use super::*;

    fn create_repeat_count_def(count: usize) -> StructureDefinition {
        StructureDefinition::new("test_struct").with_field(
            FieldDefinition::new("items", FieldType::UnsignedInt(1))
                .with_size(SizeSpec::Fixed(1))
                .with_repeat(RepeatSpec::Count(count)),
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Paths field_0 through field_{N-1} are accessible
        #[test]
        fn valid_indices_accessible(count in 1usize..20usize) {
            let def = Arc::new(create_repeat_count_def(count));
            let data: Vec<u8> = (0..count as u8).collect();
            let accessor = StructureAccessor::new(def, &data).unwrap();

            for i in 0..count {
                let path = format!("items_{}", i);
                prop_assert!(accessor.has(&path),
                    "Path '{}' should be accessible for count={}", path, count);

                let result = accessor.get(&path);
                prop_assert!(result.is_ok(),
                    "get('{}') should succeed for count={}", path, count);
            }
        }

        /// Path field_N returns UnknownField error
        #[test]
        fn invalid_index_returns_error(count in 1usize..20usize) {
            let def = Arc::new(create_repeat_count_def(count));
            let data: Vec<u8> = (0..count as u8).collect();
            let accessor = StructureAccessor::new(def, &data).unwrap();

            let path = format!("items_{}", count);
            prop_assert!(!accessor.has(&path),
                "Path '{}' should NOT be accessible for count={}", path, count);

            let result = accessor.get(&path);
            prop_assert!(result.is_err(),
                "get('{}') should fail for count={}", path, count);

            match result.unwrap_err() {
                AccessError::UnknownField { .. } => {},
                other => prop_assert!(false,
                    "Expected UnknownField error, got {:?}", other),
            }
        }

        /// Underscore naming format is correct
        #[test]
        fn naming_format_correct(count in 1usize..10usize) {
            let def = Arc::new(create_repeat_count_def(count));
            let data: Vec<u8> = (0..count as u8).collect();
            let accessor = StructureAccessor::new(def, &data).unwrap();

            let fields: Vec<String> = accessor.fields().collect();

            for i in 0..count {
                let expected_path = format!("items_{}", i);
                prop_assert!(fields.contains(&expected_path),
                    "fields() should contain '{}'", expected_path);
            }
        }
    }
}
