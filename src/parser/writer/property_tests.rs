//! Property-based tests for the structure writer.

use super::*;
use crate::parser::types::{
    Encoding, Endian, FieldDefinition, FieldType, RepeatSpec, SizeSpec, StructureDefinition,
};
use proptest::prelude::*;

/// Property 23: Streaming Mode Order Enforcement
/// For any streaming writer, writing a field before all preceding fields have
/// been written SHALL return an OutOfOrder error.
/// **Validates: Requirements 9.2, 9.3**
mod prop_23_streaming_mode_order {
    use super::*;

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

    fn valid_bcs_a_string(max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, 1..=max_len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn in_order_writes_succeed(
            val_a in valid_bcs_a_string(4),
            val_b in valid_bcs_a_string(4),
            val_c in valid_bcs_a_string(4),
            val_d in valid_bcs_a_string(4)
        ) {
            let def = create_multi_field_def();
            let mut writer = StructureWriter::new(def);

            prop_assert!(writer.set("field_a", val_a).is_ok());
            prop_assert!(writer.set("field_b", val_b).is_ok());
            prop_assert!(writer.set("field_c", val_c).is_ok());
            prop_assert!(writer.set("field_d", val_d).is_ok());
        }

        #[test]
        fn skipping_field_fails(
            skip_index in 0usize..3,
            val in valid_bcs_a_string(4)
        ) {
            let def = create_multi_field_def();
            let mut writer = StructureWriter::new(def);

            let fields = ["field_a", "field_b", "field_c", "field_d"];

            for i in 0..skip_index {
                writer.set(fields[i], val.clone()).unwrap();
            }

            let skip_to = skip_index + 1;
            if skip_to < fields.len() {
                let result = writer.set(fields[skip_to], val.clone());
                prop_assert!(result.is_err(), "Skipping field should fail");
                let is_out_of_order = matches!(result.unwrap_err(), WriteError::OutOfOrder { .. });
                prop_assert!(is_out_of_order, "Error should be OutOfOrder");
            }
        }

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

            let mut writer = StructureWriter::new(def);

            let result = writer.set(&format!("items_{}", wrong_first_index), vals[0].clone());
            prop_assert!(result.is_err(), "Writing non-zero index first should fail");
            let is_out_of_order = matches!(result.unwrap_err(), WriteError::OutOfOrder { .. });
            prop_assert!(is_out_of_order, "Error should be OutOfOrder");
        }
    }
}

/// Property 20: Missing Required Field Error
/// **Validates: Requirements 8.4**
mod prop_20_missing_required_field {
    use super::*;

    fn field_name(index: usize) -> &'static str {
        ["alpha", "beta", "gamma", "delta", "epsilon"][index]
    }

    fn valid_bcs_a_string(max_len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, 1..=max_len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Finish succeeds when all required fields are written
        #[test]
        fn finish_succeeds_with_all_fields(
            num_fields in 1usize..5,
            val in valid_bcs_a_string(4)
        ) {
            let mut def = StructureDefinition::new("test_struct");
            for i in 0..num_fields {
                def = def.with_field(
                    FieldDefinition::new(field_name(i), FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                );
            }
            let def = Arc::new(def);
            let mut writer = StructureWriter::new(def);

            for i in 0..num_fields {
                writer.set(field_name(i), val.clone()).unwrap();
            }

            let result = writer.finish();
            prop_assert!(result.is_ok(), "finish() should succeed with all fields written");
        }

        /// Finish fails when not all fields are written (partial writes)
        #[test]
        fn finish_fails_with_partial_writes(
            num_fields in 2usize..5,
            write_count in 0usize..4
        ) {
            let write_count = write_count % num_fields; // ensure we write fewer than total
            let mut def = StructureDefinition::new("test_struct");
            for i in 0..num_fields {
                def = def.with_field(
                    FieldDefinition::new(field_name(i), FieldType::String)
                        .with_size(SizeSpec::fixed(4)),
                );
            }
            let def = Arc::new(def);
            let mut writer = StructureWriter::new(def);

            for i in 0..write_count {
                writer.set(field_name(i), "TEST").unwrap();
            }

            let result = writer.finish();
            prop_assert!(result.is_err(), "finish() should fail with missing fields");
            let is_missing = matches!(result.unwrap_err(), WriteError::MissingRequired { .. });
            prop_assert!(is_missing, "Error should be MissingRequired");
        }
    }
}

/// Property 21: Value Too Large Error
/// **Validates: Requirements 8.5, 10.4, 10.5**
mod prop_21_value_too_large {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

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

            let mut writer = StructureWriter::new(def);
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

        #[test]
        fn string_exact_size_succeeds(field_size in 1usize..50) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new(def);
            let value = "X".repeat(field_size);
            let result = writer.set("field", value);
            prop_assert!(result.is_ok(), "Writing exact-size value should succeed");
        }

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

            let mut writer = StructureWriter::new(def);
            let value = vec![0u8; field_size + extra_bytes];
            let result = writer.set("field", value);

            prop_assert!(result.is_err(), "Writing oversized bytes should fail");
            let is_too_large = matches!(result.unwrap_err(), WriteError::ValueTooLarge { .. });
            prop_assert!(is_too_large, "Error should be ValueTooLarge");
        }
    }
}

/// Property 22: Padding Application
/// **Validates: Requirements 8.6**
mod prop_22_padding_application {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

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

            let mut writer = StructureWriter::new(def);
            let value = "X".repeat(value_size);
            writer.set("field", value.clone()).unwrap();

            let buffer = writer.buffer();
            prop_assert_eq!(&buffer[..value_size], value.as_bytes());
            for i in value_size..field_size {
                prop_assert_eq!(buffer[i], 0x20, "Padding should be space (0x20)");
            }
        }

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

            let mut writer = StructureWriter::new(def);
            let value = "1".repeat(value_size);
            writer.set("field", value.clone()).unwrap();

            let buffer = writer.buffer();
            prop_assert_eq!(&buffer[..value_size], value.as_bytes());
            for i in value_size..field_size {
                prop_assert_eq!(buffer[i], 0x30, "BCS-N padding should be '0' (0x30)");
            }
        }

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

            let mut writer = StructureWriter::new(def);
            let value = "X".repeat(value_size);
            writer.set("field", value.clone()).unwrap();

            let buffer = writer.buffer();
            for i in value_size..field_size {
                prop_assert_eq!(buffer[i], pad_char);
            }
        }

        #[test]
        fn empty_string_fully_padded(field_size in 1usize..20) {
            let def = Arc::new(
                StructureDefinition::new("test")
                    .with_field(
                        FieldDefinition::new("field", FieldType::String)
                            .with_size(SizeSpec::fixed(field_size)),
                    ),
            );

            let mut writer = StructureWriter::new(def);
            writer.set("field", "").unwrap();

            let buffer = writer.buffer();
            for i in 0..field_size {
                prop_assert_eq!(buffer[i], 0x20, "Empty string should be fully padded");
            }
        }
    }
}

/// Property 24: Write Character Set Validation
/// **Validates: Requirements 10.2, 10.3**
mod prop_24_write_character_set_validation {
    use super::*;

    fn valid_bcs_a_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, len..=len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    fn valid_bcs_n_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                0x30u8..=0x39u8,
                Just(0x20u8),
                Just(0x2Bu8),
                Just(0x2Du8),
                Just(0x2Eu8),
                Just(0x2Fu8),
            ],
            len..=len,
        )
        .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    fn invalid_bcs_a_byte() -> impl Strategy<Value = u8> {
        prop_oneof![0x00u8..0x20u8, 0x7Fu8..=0xFFu8,]
    }

    fn invalid_bcs_n_byte() -> impl Strategy<Value = u8> {
        prop_oneof![
            Just(0x21u8),
            Just(0x22u8),
            Just(0x23u8),
            Just(0x24u8),
            Just(0x25u8),
            Just(0x26u8),
            Just(0x27u8),
            Just(0x28u8),
            Just(0x29u8),
            Just(0x2Au8),
            Just(0x2Cu8),
            0x3Au8..=0x7Eu8,
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

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

            let mut writer = StructureWriter::new(def);
            let result = writer.set("field", val);
            prop_assert!(result.is_ok(), "Valid BCS-A should be accepted");
        }

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

            let mut writer = StructureWriter::new(def);
            let mut bytes = prefix.into_bytes();
            bytes.push(invalid_byte);
            bytes.extend(suffix.bytes());

            if let Ok(invalid_str) = String::from_utf8(bytes) {
                let result = writer.set("field", invalid_str);
                prop_assert!(result.is_err(), "Invalid BCS-A should be rejected");
                let is_validation_err = matches!(result.unwrap_err(), WriteError::ValidationError { .. });
                prop_assert!(is_validation_err, "Error should be ValidationError");
            }
        }

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

            let mut writer = StructureWriter::new(def);
            let result = writer.set("field", val);
            prop_assert!(result.is_ok(), "Valid BCS-N should be accepted");
        }

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

            let mut writer = StructureWriter::new(def);
            let mut bytes = prefix.into_bytes();
            bytes.push(invalid_byte);
            bytes.extend(suffix.bytes());
            let invalid_str = String::from_utf8(bytes).unwrap();

            let result = writer.set("field", invalid_str);
            prop_assert!(result.is_err(), "Invalid BCS-N should be rejected");
            let is_validation_err = matches!(result.unwrap_err(), WriteError::ValidationError { .. });
            prop_assert!(is_validation_err, "Error should be ValidationError");
        }

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

            let mut writer = StructureWriter::new(def);
            let invalid_str = format!("12{}34", letter);
            let result = writer.set("field", invalid_str);
            prop_assert!(result.is_err(), "Letters in BCS-N should be rejected");
        }
    }
}

/// Property 2: Binary Data Round-Trip
/// **Validates: Requirements 16.1, 16.2**
mod prop_2_binary_data_round_trip {
    use super::*;
    use crate::parser::accessor::StructureAccessor;
    use crate::parser::value::Value;

    fn create_simple_round_trip_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("round_trip_test")
                .with_field(
                    FieldDefinition::new("magic", FieldType::String).with_size(SizeSpec::fixed(4)),
                )
                .with_field(FieldDefinition::new("version", FieldType::UnsignedInt(2)))
                .with_field(
                    FieldDefinition::new("name", FieldType::String).with_size(SizeSpec::fixed(10)),
                )
                .with_field(FieldDefinition::new("flags", FieldType::UnsignedInt(1))),
        )
    }

    fn create_repeated_round_trip_def() -> Arc<StructureDefinition> {
        Arc::new(
            StructureDefinition::new("repeated_round_trip_test")
                .with_field(
                    FieldDefinition::new("header", FieldType::String).with_size(SizeSpec::fixed(4)),
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

    fn valid_bcs_a_string(len: usize) -> impl Strategy<Value = String> {
        proptest::collection::vec(0x20u8..=0x7Eu8, len..=len)
            .prop_map(|bytes| String::from_utf8(bytes).unwrap())
    }

    /// Copy fields from accessor to writer in definition order (streaming).
    fn copy_fields_to_writer(
        accessor: &StructureAccessor,
        writer: &mut StructureWriter,
    ) -> Result<(), WriteError> {
        for path in accessor.fields() {
            let value = accessor
                .get(&path)
                .map_err(|e| WriteError::ValidationError {
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
                    // Write each element sequentially with indexed path
                    for (i, elem) in elements.iter().enumerate() {
                        let indexed_path = format!("{}_{}", path, i);
                        match elem {
                            Value::String(cow) => writer.set(&indexed_path, cow.to_string())?,
                            Value::Bytes(bytes) => writer.set(&indexed_path, bytes.to_vec())?,
                            Value::Unsigned(n) => writer.set(&indexed_path, *n)?,
                            _ => {}
                        }
                    }
                }
                Value::Struct(_) => {}
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn simple_structure_round_trip(
            magic in valid_bcs_a_string(4),
            version in 0u16..=65535u16,
            name in valid_bcs_a_string(10),
            flags in any::<u8>()
        ) {
            let def = create_simple_round_trip_def();

            let mut original_data = Vec::new();
            original_data.extend_from_slice(magic.as_bytes());
            original_data.extend_from_slice(&version.to_be_bytes());
            original_data.extend_from_slice(name.as_bytes());
            original_data.push(flags);

            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();
            let mut writer = StructureWriter::new(def);
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            prop_assert_eq!(original_data, written_data,
                "Round-trip should produce identical bytes");
        }

        #[test]
        fn repeated_fields_round_trip(
            header in valid_bcs_a_string(4),
            items in proptest::collection::vec(any::<u8>(), 4..=4),
            trailer in valid_bcs_a_string(4)
        ) {
            let def = create_repeated_round_trip_def();

            let mut original_data = Vec::new();
            original_data.extend_from_slice(header.as_bytes());
            original_data.extend_from_slice(&items);
            original_data.extend_from_slice(trailer.as_bytes());

            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();
            let mut writer = StructureWriter::new(def);
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            prop_assert_eq!(original_data, written_data,
                "Round-trip with repeated fields should produce identical bytes");
        }

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

            let mut original_data = Vec::new();
            original_data.push(u1_val);
            original_data.extend_from_slice(&u2_val.to_be_bytes());
            original_data.extend_from_slice(&u4_val.to_be_bytes());

            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();
            let mut writer = StructureWriter::new(def);
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            prop_assert_eq!(original_data, written_data,
                "Round-trip with integer fields should produce identical bytes");
        }

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

            let mut original_data = Vec::new();
            original_data.extend_from_slice(&u2_val.to_le_bytes());
            original_data.extend_from_slice(&u4_val.to_le_bytes());

            let accessor = StructureAccessor::new(def.clone(), &original_data).unwrap();
            let mut writer = StructureWriter::new(def);
            copy_fields_to_writer(&accessor, &mut writer).unwrap();
            let written_data = writer.finish().unwrap();

            prop_assert_eq!(original_data, written_data,
                "Round-trip with little-endian integers should produce identical bytes");
        }
    }
}
