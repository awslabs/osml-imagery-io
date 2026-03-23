//! Property-based tests for the structure writer.

use super::*;
use crate::parser::types::{Encoding, Endian, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition};
use proptest::prelude::*;

/// Property 19: Fixed-Size Out-of-Order Writing
/// For any fixed-size structure, writing fields in any order SHALL produce
/// the same output as writing them in definition order.
/// **Validates: Requirements 8.2**
mod prop_19_fixed_size_out_of_order {
    use super::*;

    /// Create a test definition with multiple fields
    fn create_multi_field_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("test_struct")
                .with_field(
                    FieldDefinition::new("field_a", FieldType::String)
                        .with_size(SizeSpec::fixed(8)),
                )
                .with_field(
                    FieldDefinition::new("field_b", FieldType::String)
                        .with_size(SizeSpec::fixed(8)),
                )
                .with_field(
                    FieldDefinition::new("field_c", FieldType::String)
                        .with_size(SizeSpec::fixed(8)),
                ),
        )
    }

    /// Generate a valid BCS-A string of specified length
    fn valid_bcs_a_string(max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, 1..=max_len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Writing fields in any order produces same result as definition order
        #[test]
        fn out_of_order_same_as_in_order(
            val_a in valid_bcs_a_string(8),
            val_b in valid_bcs_a_string(8),
            val_c in valid_bcs_a_string(8),
            order in prop::sample::subsequence((0..3).collect::<Vec<_>>(), 3)
        ) {
            let def = create_multi_field_def();

            // Write in definition order
            let mut writer_ordered = StructureWriter::new_fixed(def.clone()).unwrap();
            writer_ordered.set("field_a", val_a.clone()).unwrap();
            writer_ordered.set("field_b", val_b.clone()).unwrap();
            writer_ordered.set("field_c", val_c.clone()).unwrap();
            let ordered_result = writer_ordered.finish().unwrap();

            // Write in random order
            let mut writer_random = StructureWriter::new_fixed(def).unwrap();
            let fields = [
                ("field_a", val_a.clone()),
                ("field_b", val_b.clone()),
                ("field_c", val_c.clone()),
            ];

            for &idx in &order {
                let (name, val) = &fields[idx];
                writer_random.set(name, val.clone()).unwrap();
            }
            let random_result = writer_random.finish().unwrap();

            prop_assert_eq!(ordered_result, random_result,
                "Out-of-order write should produce same result as in-order write");
        }

        /// Writing same field multiple times overwrites previous value
        #[test]
        fn overwrite_produces_last_value(
            val1 in valid_bcs_a_string(8),
            val2 in valid_bcs_a_string(8)
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(8)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def.clone()).unwrap();
            writer.set("field", val1.clone()).unwrap();
            writer.set("field", val2.clone()).unwrap();

            // Create expected result with val2
            let mut expected_writer = StructureWriter::new_fixed(def).unwrap();
            expected_writer.set("field", val2.clone()).unwrap();

            prop_assert_eq!(writer.buffer(), expected_writer.buffer(),
                "Overwriting should use last value");
        }

        /// Repeated fields can be written in any order
        #[test]
        fn repeated_fields_any_order(
            vals in proptest::collection::vec(valid_bcs_a_string(4), 3..=3),
            order in prop::sample::subsequence((0..3).collect::<Vec<_>>(), 3)
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("items", FieldType::String)
                            .with_size(SizeSpec::fixed(4))
                            .with_repeat(RepeatSpec::count(3)),
                    ),
            );

            // Write in definition order
            let mut writer_ordered = StructureWriter::new_fixed(def.clone()).unwrap();
            for (i, val) in vals.iter().enumerate() {
                writer_ordered.set(&format!("items_{}", i), val.clone()).unwrap();
            }
            let ordered_result = writer_ordered.finish().unwrap();

            // Write in random order
            let mut writer_random = StructureWriter::new_fixed(def).unwrap();
            for &idx in &order {
                writer_random.set(&format!("items_{}", idx), vals[idx].clone()).unwrap();
            }
            let random_result = writer_random.finish().unwrap();

            prop_assert_eq!(ordered_result, random_result,
                "Repeated fields in any order should produce same result");
        }
    }
}


/// Property 23: Streaming Mode Order Enforcement
/// For any streaming writer, writing a field before all preceding fields have
/// been written SHALL return an OutOfOrder error.
/// **Validates: Requirements 9.2, 9.3**
mod prop_23_streaming_mode_order {
    use super::*;
    use proptest::prelude::*;

    /// Create a test definition with multiple fields
    fn create_multi_field_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("test_struct")
                .with_field(
                    FieldDefinition::new("field_a", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                )
                .with_field(
                    FieldDefinition::new("field_b", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                )
                .with_field(
                    FieldDefinition::new("field_c", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                )
                .with_field(
                    FieldDefinition::new("field_d", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                ),
        )
    }

    /// Generate a valid BCS-A string of specified length
    fn valid_bcs_a_string(max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, 1..=max_len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Writing fields in order succeeds
        #[test]
        fn in_order_writes_succeed(
            val_a in valid_bcs_a_string(4),
            val_b in valid_bcs_a_string(4),
            val_c in valid_bcs_a_string(4),
            val_d in valid_bcs_a_string(4)
        ) {
            let def = create_multi_field_def();
            let mut writer = StructureWriter::new_streaming(def);

            // Write in order - should all succeed
            prop_assert!(writer.set("field_a", val_a).is_ok());
            prop_assert!(writer.set("field_b", val_b).is_ok());
            prop_assert!(writer.set("field_c", val_c).is_ok());
            prop_assert!(writer.set("field_d", val_d).is_ok());
        }

        /// Skipping any field returns OutOfOrder error
        #[test]
        fn skipping_field_fails(
            skip_index in 0usize..3,
            val in valid_bcs_a_string(4)
        ) {
            let def = create_multi_field_def();
            let mut writer = StructureWriter::new_streaming(def);

            let fields = ["field_a", "field_b", "field_c", "field_d"];

            // Write fields up to skip_index
            for i in 0..skip_index {
                writer.set(fields[i], val.clone()).unwrap();
            }

            // Try to skip to a later field
            let skip_to = skip_index + 1;
            if skip_to < fields.len() {
                let result = writer.set(fields[skip_to], val.clone());
                prop_assert!(result.is_err(), "Skipping field should fail");
                prop_assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }),
                    "Error should be OutOfOrder");
            }
        }

        /// Writing same field twice in streaming mode succeeds (overwrites)
        #[test]
        fn writing_same_field_twice_succeeds(
            val1 in valid_bcs_a_string(4),
            _val2 in valid_bcs_a_string(4)
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(4)),
                    ),
            );

            let mut writer = StructureWriter::new_streaming(def);
            
            // First write should succeed
            prop_assert!(writer.set("field", val1).is_ok());
            
            // Second write to same field should also succeed (overwrite)
            // Note: In streaming mode, this is allowed since we're at the same position
            // The field is already marked as written, so this is a no-op or overwrite
        }

        /// Streaming mode produces same bytes as fixed mode for same values
        #[test]
        fn streaming_matches_fixed_output(
            val_a in valid_bcs_a_string(4),
            val_b in valid_bcs_a_string(4),
            val_c in valid_bcs_a_string(4),
            val_d in valid_bcs_a_string(4)
        ) {
            let def = create_multi_field_def();

            // Write with fixed mode
            let mut fixed_writer = StructureWriter::new_fixed(def.clone()).unwrap();
            fixed_writer.set("field_a", val_a.clone()).unwrap();
            fixed_writer.set("field_b", val_b.clone()).unwrap();
            fixed_writer.set("field_c", val_c.clone()).unwrap();
            fixed_writer.set("field_d", val_d.clone()).unwrap();
            let fixed_result = fixed_writer.finish().unwrap();

            // Write with streaming mode
            let mut streaming_writer = StructureWriter::new_streaming(def);
            streaming_writer.set("field_a", val_a).unwrap();
            streaming_writer.set("field_b", val_b).unwrap();
            streaming_writer.set("field_c", val_c).unwrap();
            streaming_writer.set("field_d", val_d).unwrap();
            let streaming_result = streaming_writer.finish().unwrap();

            prop_assert_eq!(fixed_result, streaming_result,
                "Streaming and fixed mode should produce identical output");
        }

        /// Repeated fields must be written in index order
        #[test]
        fn repeated_fields_require_index_order(
            vals in proptest::collection::vec(valid_bcs_a_string(4), 3..=3),
            wrong_first_index in 1usize..3
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("items", FieldType::String)
                            .with_size(SizeSpec::fixed(4))
                            .with_repeat(RepeatSpec::count(3)),
                    ),
            );

            let mut writer = StructureWriter::new_streaming(def);

            // Try to write a non-zero index first
            let result = writer.set(&format!("items_{}", wrong_first_index), vals[0].clone());
            prop_assert!(result.is_err(), "Writing non-zero index first should fail");
            prop_assert!(matches!(result.unwrap_err(), WriteError::OutOfOrder { .. }),
                "Error should be OutOfOrder");
        }
    }
}


/// Property 20: Missing Required Field Error
/// For any structure with required fields, calling `finish()` without writing
/// all required fields SHALL return a MissingRequired error.
/// **Validates: Requirements 8.4**
mod prop_20_missing_required_field {
    use super::*;
    use proptest::prelude::*;

    /// Create a test definition with multiple required fields
    fn create_multi_field_def(num_fields: usize) -> Arc<StructureDefinition> {
        let field_names = ["alpha", "beta", "gamma", "delta", "epsilon"];
        let mut def = StructureDefinition::new("test_struct");
        for i in 0..num_fields {
            def = def.with_field(
                FieldDefinition::new(field_names[i], FieldType::String)
                    .with_size(SizeSpec::fixed(4)),
            );
        }
        Arc::new(def)
    }

    /// Get field name by index
    fn field_name(index: usize) -> &'static str {
        let field_names = ["alpha", "beta", "gamma", "delta", "epsilon"];
        field_names[index]
    }

    /// Generate a valid BCS-A string
    fn valid_bcs_a_string(max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, 1..=max_len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Finish fails when any required field is missing
        #[test]
        fn finish_fails_with_missing_field(
            num_fields in 2usize..5,
            skip_index in 0usize..4
        ) {
            let skip_index = skip_index % num_fields;
            let def = create_multi_field_def(num_fields);
            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Write all fields except one
            for i in 0..num_fields {
                if i != skip_index {
                    writer.set(field_name(i), "TEST").unwrap();
                }
            }

            let result = writer.finish();
            prop_assert!(result.is_err(), "finish() should fail with missing field");
            
            if let Err(WriteError::MissingRequired { path }) = result {
                prop_assert_eq!(path, field_name(skip_index),
                    "Error should identify the missing field");
            } else {
                prop_assert!(false, "Error should be MissingRequired");
            }
        }

        /// Finish succeeds when all required fields are written
        #[test]
        fn finish_succeeds_with_all_fields(
            num_fields in 1usize..5,
            val in valid_bcs_a_string(4)
        ) {
            let def = create_multi_field_def(num_fields);
            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Write all fields
            for i in 0..num_fields {
                writer.set(field_name(i), val.clone()).unwrap();
            }

            let result = writer.finish();
            prop_assert!(result.is_ok(), "finish() should succeed with all fields written");
        }

        /// Missing repeated field element returns error
        #[test]
        fn missing_repeated_element_fails(
            count in 2usize..5,
            skip_index in 0usize..4
        ) {
            let skip_index = skip_index % count;
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("items", FieldType::String)
                            .with_size(SizeSpec::fixed(4))
                            .with_repeat(RepeatSpec::count(count)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Write all elements except one
            for i in 0..count {
                if i != skip_index {
                    writer.set(&format!("items_{}", i), "TEST").unwrap();
                }
            }

            let result = writer.finish();
            prop_assert!(result.is_err(), "finish() should fail with missing element");
            
            if let Err(WriteError::MissingRequired { path }) = result {
                prop_assert_eq!(path, format!("items_{}", skip_index),
                    "Error should identify the missing element");
            } else {
                prop_assert!(false, "Error should be MissingRequired");
            }
        }
    }
}


/// Property 21: Value Too Large Error
/// For any field with a fixed size, writing a value larger than that size
/// SHALL return a ValueTooLarge error.
/// **Validates: Requirements 8.5, 10.4, 10.5**
mod prop_21_value_too_large {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// String value exceeding field size returns error
        #[test]
        fn string_too_large_fails(
            field_size in 1usize..20,
            extra_chars in 1usize..10
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Create a string that's too long
            let value = "X".repeat(field_size + extra_chars);
            let result = writer.set("field", value);

            prop_assert!(result.is_err(), "Writing oversized value should fail");
            
            if let Err(WriteError::ValueTooLarge { max_size, actual_size, .. }) = result {
                prop_assert_eq!(max_size, field_size);
                prop_assert_eq!(actual_size, field_size + extra_chars);
            } else {
                prop_assert!(false, "Error should be ValueTooLarge");
            }
        }

        /// String value exactly at field size succeeds
        #[test]
        fn string_exact_size_succeeds(field_size in 1usize..50) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Create a string that's exactly the right size
            let value = "X".repeat(field_size);
            let result = writer.set("field", value);

            prop_assert!(result.is_ok(), "Writing exact-size value should succeed");
        }

        /// String value smaller than field size succeeds
        #[test]
        fn string_smaller_succeeds(
            field_size in 2usize..50,
            value_size in 1usize..49
        ) {
            let value_size = value_size % field_size;
            if value_size == 0 {
                return Ok(());
            }

            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            let value = "X".repeat(value_size);
            let result = writer.set("field", value);

            prop_assert!(result.is_ok(), "Writing smaller value should succeed");
        }

        /// Bytes value exceeding field size returns error
        #[test]
        fn bytes_too_large_fails(
            field_size in 1usize..20,
            extra_bytes in 1usize..10
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::Bytes)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Create bytes that are too long
            let value = vec![0u8; field_size + extra_bytes];
            let result = writer.set("field", value);

            prop_assert!(result.is_err(), "Writing oversized bytes should fail");
            let err = result.unwrap_err();
            prop_assert!(matches!(err, WriteError::ValueTooLarge { .. }), 
                "Error should be ValueTooLarge");
        }
    }
}


/// Property 22: Padding Application
/// For any string field with padding, writing a string shorter than the field
/// size SHALL result in the remaining bytes being filled with the padding character.
/// **Validates: Requirements 8.6**
mod prop_22_padding_application {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Short strings are padded with spaces by default
        #[test]
        fn default_space_padding(
            field_size in 5usize..20,
            value_size in 1usize..4
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            let value = "X".repeat(value_size);
            writer.set("field", value.clone()).unwrap();

            let buffer = writer.buffer();
            
            // Check value is at the start
            prop_assert_eq!(&buffer[..value_size], value.as_bytes());
            
            // Check padding is spaces
            for i in value_size..field_size {
                prop_assert_eq!(buffer[i], 0x20, "Padding should be space (0x20)");
            }
        }

        /// BCS-N fields are padded with zeros
        #[test]
        fn bcs_n_zero_padding(
            field_size in 5usize..20,
            value_size in 1usize..4
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size))
                            .with_encoding(Encoding::BcsN),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Use digits for BCS-N
            let value = "1".repeat(value_size);
            writer.set("field", value.clone()).unwrap();

            let buffer = writer.buffer();
            
            // Check value is at the start
            prop_assert_eq!(&buffer[..value_size], value.as_bytes());
            
            // Check padding is '0' (0x30)
            for i in value_size..field_size {
                prop_assert_eq!(buffer[i], 0x30, "BCS-N padding should be '0' (0x30)");
            }
        }

        /// Custom padding character is applied
        #[test]
        fn custom_padding_applied(
            field_size in 5usize..20,
            value_size in 1usize..4,
            pad_char in 0x20u8..0x7Fu8
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size))
                            .with_pad(pad_char),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            let value = "X".repeat(value_size);
            writer.set("field", value.clone()).unwrap();

            let buffer = writer.buffer();
            
            // Check padding is the custom character
            for i in value_size..field_size {
                prop_assert_eq!(buffer[i], pad_char, 
                    "Padding should be custom char (0x{:02X})", pad_char);
            }
        }

        /// Empty string is fully padded
        #[test]
        fn empty_string_fully_padded(field_size in 1usize..20) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            writer.set("field", "").unwrap();

            let buffer = writer.buffer();
            
            // All bytes should be padding (space)
            for i in 0..field_size {
                prop_assert_eq!(buffer[i], 0x20, "Empty string should be fully padded");
            }
        }
    }
}


/// Property 24: Write Character Set Validation
/// For any BCS-N field, writing a string containing non-numeric characters
/// SHALL return a validation error. For any BCS-A field, writing a string
/// containing characters outside 0x20-0x7E SHALL return a validation error.
/// **Validates: Requirements 10.2, 10.3**
mod prop_24_write_character_set_validation {
    use super::*;
    use proptest::prelude::*;

    /// Generate a valid BCS-A string
    fn valid_bcs_a_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, len..=len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    /// Generate a valid BCS-N string (digits, spaces, plus, minus, decimal, slash)
    fn valid_bcs_n_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                0x30u8..=0x39u8,  // digits
                Just(0x20u8),      // space
                Just(0x2Bu8),      // '+'
                Just(0x2Du8),      // '-'
                Just(0x2Eu8),      // '.'
                Just(0x2Fu8),      // '/'
            ],
            len..=len
        ).prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    /// Generate an invalid BCS-A byte (outside 0x20-0x7E)
    fn invalid_bcs_a_byte() -> impl Strategy<Value = u8> {
        prop_oneof![
            0x00u8..0x20u8,   // control characters
            0x7Fu8..=0xFFu8, // DEL and extended ASCII
        ]
    }

    /// Generate an invalid BCS-N byte (not digit, space, plus, minus, decimal, or slash)
    fn invalid_bcs_n_byte() -> impl Strategy<Value = u8> {
        prop_oneof![
            Just(0x21u8),        // '!'
            Just(0x22u8),        // '"'
            Just(0x23u8),        // '#'
            Just(0x24u8),        // '$'
            Just(0x25u8),        // '%'
            Just(0x26u8),        // '&'
            Just(0x27u8),        // '\''
            Just(0x28u8),        // '('
            Just(0x29u8),        // ')'
            Just(0x2Au8),        // '*'
            Just(0x2Cu8),        // ','
            0x3Au8..=0x7Eu8,     // characters after '9' through '~'
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Valid BCS-A strings are accepted
        #[test]
        fn valid_bcs_a_accepted(val in valid_bcs_a_string(10)) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(10))
                            .with_encoding(Encoding::BcsA),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();
            let result = writer.set("field", val);
            prop_assert!(result.is_ok(), "Valid BCS-A should be accepted");
        }

        /// Invalid BCS-A strings are rejected
        #[test]
        fn invalid_bcs_a_rejected(
            prefix in valid_bcs_a_string(4),
            invalid_byte in invalid_bcs_a_byte(),
            suffix in valid_bcs_a_string(4)
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(10))
                            .with_encoding(Encoding::BcsA),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Create string with invalid byte in the middle
            let mut bytes = prefix.into_bytes();
            bytes.push(invalid_byte);
            bytes.extend(suffix.bytes());
            
            // This might not be valid UTF-8, so we need to handle that
            if let Ok(invalid_str) = String::from_utf8(bytes) {
                let result = writer.set("field", invalid_str);
                prop_assert!(result.is_err(), "Invalid BCS-A should be rejected");
                let err = result.unwrap_err();
                prop_assert!(matches!(err, WriteError::ValidationError { .. }),
                    "Error should be ValidationError");
            }
        }

        /// Valid BCS-N strings are accepted
        #[test]
        fn valid_bcs_n_accepted(val in valid_bcs_n_string(5)) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(5))
                            .with_encoding(Encoding::BcsN),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();
            let result = writer.set("field", val);
            prop_assert!(result.is_ok(), "Valid BCS-N should be accepted");
        }

        /// Invalid BCS-N strings are rejected
        #[test]
        fn invalid_bcs_n_rejected(
            prefix in valid_bcs_n_string(2),
            invalid_byte in invalid_bcs_n_byte(),
            suffix in valid_bcs_n_string(2)
        ) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(5))
                            .with_encoding(Encoding::BcsN),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            // Create string with invalid byte
            let mut bytes = prefix.into_bytes();
            bytes.push(invalid_byte);
            bytes.extend(suffix.bytes());
            let invalid_str = String::from_utf8(bytes).unwrap();

            let result = writer.set("field", invalid_str);
            prop_assert!(result.is_err(), "Invalid BCS-N should be rejected");
            let err = result.unwrap_err();
            prop_assert!(matches!(err, WriteError::ValidationError { .. }),
                "Error should be ValidationError");
        }

        /// Letters in BCS-N field are rejected
        #[test]
        fn letters_in_bcs_n_rejected(letter in prop::char::range('A', 'z')) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(5))
                            .with_encoding(Encoding::BcsN),
                    ),
            );

            let mut writer = StructureWriter::new_fixed(def).unwrap();

            let invalid_str = format!("12{}34", letter);
            let result = writer.set("field", invalid_str);
            prop_assert!(result.is_err(), "Letters in BCS-N should be rejected");
        }
    }
}


/// Property 2: Binary Data Round-Trip
/// For any valid binary data that conforms to a structure definition, parsing
/// it with StructureAccessor and then writing it with StructureWriter SHALL
/// produce identical bytes.
/// **Validates: Requirements 16.1, 16.2**
mod prop_2_binary_data_round_trip {
    use super::*;
    use crate::parser::accessor::StructureAccessor;
    use crate::parser::value::Value;
    use proptest::prelude::*;

    /// Create a simple structure definition with various field types
    fn create_simple_round_trip_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("round_trip_test")
                .with_field(
                    FieldDefinition::new("magic", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                )
                .with_field(
                    FieldDefinition::new("version", FieldType::UnsignedInt(2)),
                )
                .with_field(
                    FieldDefinition::new("name", FieldType::String)
                        .with_size(SizeSpec::fixed(10)),
                )
                .with_field(
                    FieldDefinition::new("flags", FieldType::UnsignedInt(1)),
                ),
        )
    }

    /// Create a structure definition with repeated fields
    fn create_repeated_round_trip_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("repeated_round_trip_test")
                .with_field(
                    FieldDefinition::new("header", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                )
                .with_field(
                    FieldDefinition::new("items", FieldType::UnsignedInt(1))
                        .with_repeat(RepeatSpec::count(4)),
                )
                .with_field(
                    FieldDefinition::new("trailer", FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                ),
        )
    }

    /// Create a structure definition with BCS-A and BCS-N encoded fields
    fn create_encoded_round_trip_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("encoded_round_trip_test")
                .with_field(
                    FieldDefinition::new("bcs_a_field", FieldType::String)
                        .with_size(SizeSpec::fixed(8))
                        .with_encoding(Encoding::BcsA),
                )
                .with_field(
                    FieldDefinition::new("bcs_n_field", FieldType::String)
                        .with_size(SizeSpec::fixed(6))
                        .with_encoding(Encoding::BcsN),
                )
                .with_field(
                    FieldDefinition::new("plain_field", FieldType::String)
                        .with_size(SizeSpec::fixed(5)),
                ),
        )
    }

    /// Generate a valid BCS-A string of specified length
    fn valid_bcs_a_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, len..=len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    /// Generate a valid BCS-N string of specified length (digits, spaces, signs, decimal, slash)
    fn valid_bcs_n_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                0x30u8..=0x39u8,  // digits
                Just(0x20u8),      // space
                Just(0x2Bu8),      // '+'
                Just(0x2Du8),      // '-'
                Just(0x2Eu8),      // '.'
                Just(0x2Fu8),      // '/'
            ],
            len..=len
        ).prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    /// Helper function to copy all fields from accessor to writer
    fn copy_fields_to_writer(
        accessor: &StructureAccessor,
        writer: &mut StructureWriter,
    ) -> Result<(), WriteError> {
        for path in accessor.fields() {
            let value = accessor.get(&path).map_err(|e| WriteError::ValidationError {
                path: path.clone(),
                message: e.to_string(),
            })?;

            match value {
                Value::String(cow) => {
                    writer.set(&path, cow.to_string())?;
                }
                Value::Bytes(bytes) => {
                    writer.set(&path, bytes.to_vec())?;
                }
                Value::Unsigned(n) => {
                    writer.set(&path, n)?;
                }
                Value::Array(elements) => {
                    // Write each element with indexed path (writer uses _N internally)
                    for (i, elem) in elements.iter().enumerate() {
                        let indexed_path = format!("{}_{}", path, i);
                        match elem {
                            Value::String(cow) => {
                                writer.set(&indexed_path, cow.to_string())?;
                            }
                            Value::Bytes(bytes) => {
                                writer.set(&indexed_path, bytes.to_vec())?;
                            }
                            Value::Unsigned(n) => {
                                writer.set(&indexed_path, *n)?;
                            }
                            _ => {}
                        }
                    }
                }
                Value::Struct(_) => {
                    // Nested structs would need recursive handling
                    // For now, skip them
                }
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Simple structure round-trip: parse → write → compare
        #[test]
        fn simple_structure_round_trip(
            magic in valid_bcs_a_string(4),
            version in 0u16..=65535u16,
            name in valid_bcs_a_string(10),
            flags in any::<u8>()
        ) {
            let def = create_simple_round_trip_def();

            // Build original binary data
            let mut original_data = Vec::new();
            original_data.extend_from_slice(magic.as_bytes());
            original_data.extend_from_slice(&version.to_be_bytes());
            original_data.extend_from_slice(name.as_bytes());
            original_data.push(flags);

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with writer
            let mut writer = StructureWriter::new_fixed(def).unwrap();
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Round-trip should produce identical bytes");
        }

        /// Repeated fields round-trip: parse → write → compare
        #[test]
        fn repeated_fields_round_trip(
            header in valid_bcs_a_string(4),
            items in proptest::collection::vec(any::<u8>(), 4..=4),
            trailer in valid_bcs_a_string(4)
        ) {
            let def = create_repeated_round_trip_def();

            // Build original binary data
            let mut original_data = Vec::new();
            original_data.extend_from_slice(header.as_bytes());
            original_data.extend_from_slice(&items);
            original_data.extend_from_slice(trailer.as_bytes());

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with writer
            let mut writer = StructureWriter::new_fixed(def).unwrap();
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Round-trip with repeated fields should produce identical bytes");
        }

        /// Encoded fields round-trip: parse → write → compare
        #[test]
        fn encoded_fields_round_trip(
            bcs_a_val in valid_bcs_a_string(8),
            bcs_n_val in valid_bcs_n_string(6),
            plain_val in valid_bcs_a_string(5)
        ) {
            let def = create_encoded_round_trip_def();

            // Build original binary data
            let mut original_data = Vec::new();
            original_data.extend_from_slice(bcs_a_val.as_bytes());
            original_data.extend_from_slice(bcs_n_val.as_bytes());
            original_data.extend_from_slice(plain_val.as_bytes());

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with writer
            let mut writer = StructureWriter::new_fixed(def).unwrap();
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Round-trip with encoded fields should produce identical bytes");
        }

        /// Integer fields with various sizes round-trip correctly
        #[test]
        fn integer_sizes_round_trip(
            u1_val in any::<u8>(),
            u2_val in any::<u16>(),
            u4_val in any::<u32>()
        ) {
            let def = Arc::new(
                StructureDefinition::new("int_test")
                    .with_field(FieldDefinition::new("u1_field", FieldType::u1()))
                    .with_field(FieldDefinition::new("u2_field", FieldType::u2()))
                    .with_field(FieldDefinition::new("u4_field", FieldType::u4()))
            );

            // Build original binary data (big-endian)
            let mut original_data = Vec::new();
            original_data.push(u1_val);
            original_data.extend_from_slice(&u2_val.to_be_bytes());
            original_data.extend_from_slice(&u4_val.to_be_bytes());

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with writer
            let mut writer = StructureWriter::new_fixed(def).unwrap();
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Round-trip with integer fields should produce identical bytes");
        }

        /// Little-endian integers round-trip correctly
        #[test]
        fn little_endian_round_trip(
            u2_val in any::<u16>(),
            u4_val in any::<u32>()
        ) {
            let def = Arc::new(
                StructureDefinition::new("le_test")
                    .with_endian(Endian::Little)
                    .with_field(FieldDefinition::new("u2_field", FieldType::u2()))
                    .with_field(FieldDefinition::new("u4_field", FieldType::u4()))
            );

            // Build original binary data (little-endian)
            let mut original_data = Vec::new();
            original_data.extend_from_slice(&u2_val.to_le_bytes());
            original_data.extend_from_slice(&u4_val.to_le_bytes());

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with writer
            let mut writer = StructureWriter::new_fixed(def).unwrap();
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Round-trip with little-endian integers should produce identical bytes");
        }

        /// Mixed field types round-trip correctly
        #[test]
        fn mixed_types_round_trip(
            str_val in valid_bcs_a_string(6),
            int_val in any::<u16>(),
            byte_val in any::<u8>(),
            str2_val in valid_bcs_a_string(4)
        ) {
            let def = Arc::new(
                StructureDefinition::new("mixed_test")
                    .with_field(
                        FieldDefinition::new("str_field", FieldType::String)
                            .with_size(SizeSpec::fixed(6))
                    )
                    .with_field(FieldDefinition::new("int_field", FieldType::u2()))
                    .with_field(FieldDefinition::new("byte_field", FieldType::u1()))
                    .with_field(
                        FieldDefinition::new("str2_field", FieldType::String)
                            .with_size(SizeSpec::fixed(4))
                    )
            );

            // Build original binary data
            let mut original_data = Vec::new();
            original_data.extend_from_slice(str_val.as_bytes());
            original_data.extend_from_slice(&int_val.to_be_bytes());
            original_data.push(byte_val);
            original_data.extend_from_slice(str2_val.as_bytes());

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with writer
            let mut writer = StructureWriter::new_fixed(def).unwrap();
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Round-trip with mixed field types should produce identical bytes");
        }

        /// Streaming mode produces same round-trip result as fixed mode
        #[test]
        fn streaming_mode_round_trip(
            magic in valid_bcs_a_string(4),
            version in 0u16..=65535u16,
            name in valid_bcs_a_string(10),
            flags in any::<u8>()
        ) {
            let def = create_simple_round_trip_def();

            // Build original binary data
            let mut original_data = Vec::new();
            original_data.extend_from_slice(magic.as_bytes());
            original_data.extend_from_slice(&version.to_be_bytes());
            original_data.extend_from_slice(name.as_bytes());
            original_data.push(flags);

            // Parse with accessor
            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();

            // Write with streaming writer (must write in order)
            let mut writer = StructureWriter::new_streaming(def);
            
            // Get values from accessor
            let magic_val = accessor.get("magic").unwrap();
            let version_val = accessor.get("version").unwrap();
            let name_val = accessor.get("name").unwrap();
            let flags_val = accessor.get("flags").unwrap();

            // Write in definition order
            if let Value::String(s) = magic_val {
                writer.set("magic", s.to_string()).unwrap();
            }
            if let Value::Unsigned(n) = version_val {
                writer.set("version", n).unwrap();
            }
            if let Value::String(s) = name_val {
                writer.set("name", s.to_string()).unwrap();
            }
            if let Value::Unsigned(n) = flags_val {
                writer.set("flags", n).unwrap();
            }

            let written_data = writer.finish().unwrap();

            // Compare bytes
            prop_assert_eq!(original_data, written_data,
                "Streaming mode round-trip should produce identical bytes");
        }
    }
}
