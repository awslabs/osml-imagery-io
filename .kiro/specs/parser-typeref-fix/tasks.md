# Implementation Plan: Parser TypeRef Fix

## Overview

This plan fixes the critical bug where fields after repeated nested type arrays (TypeRef) are inaccessible. The fix is localized to `src/parser/accessor/context.rs` with minimal changes to function signatures.

## Tasks

- [x] 1. Add TypeRef handling to get_simple_field_size
  - [x] 1.1 Create `get_nested_type_size()` helper function in `src/parser/accessor/context.rs`
    - Accept type_name, definition, ctx, evaluator, data, and offset parameters
    - Look up the nested type in definition.types
    - Return UnknownField error if type not found
    - Iterate through nested type's fields and sum their sizes
    - Handle conditional fields by evaluating conditions
    - Handle recursive TypeRef fields
    - _Requirements: 1.1, 1.2, 1.3, 1.4_
  
  - [x] 1.2 Update `get_simple_field_size()` signature
    - Add `definition: &StructureDefinition` parameter
    - Add `data: &[u8]` parameter
    - Add `base_offset: usize` parameter
    - _Requirements: 1.1_
  
  - [x] 1.3 Add TypeRef case to `get_simple_field_size()`
    - Match on `FieldType::TypeRef(type_name)`
    - Call `get_nested_type_size()` to calculate size
    - Return the calculated size
    - _Requirements: 1.1, 1.2, 1.3_

- [x] 2. Update get_simple_total_field_size
  - [x] 2.1 Update `get_simple_total_field_size()` signature
    - Add `definition: &StructureDefinition` parameter
    - Add `data: &[u8]` parameter
    - Add `base_offset: usize` parameter
    - _Requirements: 2.1_
  
  - [x] 2.2 Update calls to `get_simple_field_size()`
    - Pass through the new parameters
    - _Requirements: 2.1, 2.2_
  
  - [x] 2.3 Handle variable-length repeated TypeRef fields
    - For repeated TypeRef with variable-length elements, calculate each element's size
    - Sum individual element sizes for total
    - _Requirements: 2.3_

- [x] 3. Update build_context_from_definition
  - [x] 3.1 Update calls to `get_simple_field_size()` and `get_simple_total_field_size()`
    - Pass definition, data, and current_offset to size functions
    - _Requirements: 1.1, 2.1_
  
  - [x] 3.2 Ensure offset tracking remains accurate
    - Verify current_offset is updated correctly after each field
    - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [x] 4. Write unit tests for TypeRef size calculation
  - [x] 4.1 Create test for simple nested type size
    - Define a structure with a TypeRef field
    - Verify `get_simple_field_size()` returns correct size
    - _Requirements: 1.1_
  
  - [x] 4.2 Create test for nested type with conditional fields
    - Define a nested type like band_info_type with conditional fields
    - Verify size calculation handles conditionals correctly
    - _Requirements: 1.2_
  
  - [x] 4.3 Create test for repeated TypeRef total size
    - Define a structure with repeated TypeRef field
    - Verify `get_simple_total_field_size()` returns correct total
    - _Requirements: 2.1, 2.2_
  
  - [x] 4.4 Create test for unknown type error
    - Reference a non-existent type
    - Verify UnknownField error is returned
    - _Requirements: 1.4, 7.1_

- [x] 5. Write integration test for image subheader parsing
  - [x] 5.1 Create test that parses image subheader and accesses TRE fields
    - Use nitf_02.10_image_subheader definition
    - Create test data with band_info followed by TRE fields
    - Verify udidl, ixshdl fields are accessible
    - Verify udid, ixshd fields are accessible when length > 0
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_
  
  - [x] 5.2 Create test that verifies field iterator completeness
    - Parse image subheader
    - Call fields() and collect all paths
    - Verify TRE-related fields are included
    - _Requirements: 3.1, 3.2, 3.3_

- [x] 6. Write property-based test for TypeRef size consistency
  - [x] 6.1 Create property test comparing get_simple_field_size with get_type_size
    - **Property 1: TypeRef Size Accuracy**
    - Generate structures with TypeRef fields
    - Verify both functions return the same size
    - **Validates: Requirements 1.1, 1.2, 1.3**

- [x] 7. Verify backward compatibility
  - [x] 7.1 Run existing test suite
    - Execute `cargo test` and verify all tests pass
    - _Requirements: 6.1, 6.2_
  
  - [x] 7.2 Run existing property-based tests
    - Verify property tests in accessor/property_tests.rs pass
    - _Requirements: 6.2_

- [x] 8. Integration test with real NITF files
  - [x] 8.1 Test TRE extraction from JITC test files
    - Use files from data/integration/JITC/ that contain TREs
    - Verify TRE metadata is now accessible via MetadataProvider
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [x] 9. Update documentation
  - [x] 9.1 Update METADATA_PARSING_BUG.md to mark as resolved
    - Add resolution section describing the fix
    - Reference this spec
  
  - [x] 9.2 Add inline documentation to new/modified functions
    - Document get_nested_type_size() parameters and behavior
    - Document TypeRef handling in get_simple_field_size()

## Notes

- The fix is internal to the parser and does not change public APIs
- Existing code using StructureAccessor will automatically benefit
- The key insight is that `get_type_size()` in mod.rs already handles TypeRef correctly, but `get_simple_field_size()` in context.rs does not
- Variable-length nested types (like band_info_type with LUT data) require reading actual data to determine size

