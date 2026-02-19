# Requirements Document

## Introduction

This document defines the requirements for fixing a critical bug in the structure parser that prevents fields after variable-length nested type arrays from being accessible. The bug affects TRE metadata extraction from NITF image subheaders, where fields like `udid` and `ixshd` (which contain TRE data) are inaccessible because they come after the `band_info` repeated array.

## Problem Statement

The KSY-based structure parser fails to correctly calculate offsets for fields that come after repeated arrays of nested types (`TypeRef`). Specifically:

1. In `get_simple_field_size()`, when a field has `FieldType::TypeRef`, the function returns `0` instead of calculating the actual size of the nested type
2. This causes `get_simple_total_field_size()` to return incorrect sizes for repeated `TypeRef` fields
3. As a result, offset calculations for subsequent fields are wrong, making them inaccessible
4. The `fields()` iterator stops yielding fields after the problematic array

## Impact

- TRE metadata (RPC00B, GEOLOB, USE00A, etc.) cannot be extracted from NITF files
- The `MetadataProvider.as_dict()` API returns incomplete metadata
- The `describe_dataset.py --metadata` script shows only partial subheader fields
- Any KSY definition with repeated nested types followed by other fields is affected

## Glossary

- **TypeRef**: A field type that references a nested type definition (e.g., `band_info_type`)
- **Nested Type**: A sub-structure defined in the `types` section of a KSY file
- **StructureAccessor**: The lazy map-like interface for reading parsed values from binary data
- **EvalContext**: The context used for evaluating expressions during parsing
- **Offset Calculation**: The process of determining where each field starts in the binary data

## Requirements

### Requirement 1: TypeRef Size Calculation in Context Building

**User Story:** As a developer, I want the parser to correctly calculate sizes for TypeRef fields during context building, so that subsequent field offsets are accurate.

#### Acceptance Criteria

1. WHEN `get_simple_field_size()` encounters a `FieldType::TypeRef`, IT SHALL calculate the size by summing the sizes of all fields in the referenced nested type definition
2. WHEN calculating nested type size, THE function SHALL handle nested types that contain conditional fields by including only fields whose conditions evaluate to true
3. WHEN calculating nested type size, THE function SHALL recursively handle nested types that contain other TypeRef fields
4. WHEN the referenced type is not found in the definition's types map, THE function SHALL return an error

### Requirement 2: TypeRef Total Size Calculation

**User Story:** As a developer, I want repeated TypeRef fields to have correct total sizes, so that fields after repeated arrays are accessible.

#### Acceptance Criteria

1. WHEN `get_simple_total_field_size()` calculates size for a repeated TypeRef field, IT SHALL multiply the element size by the repeat count
2. WHEN the repeat count is determined by an expression, THE function SHALL evaluate the expression using the current context
3. WHEN the nested type contains variable-length fields (conditional or expression-sized), THE function SHALL calculate the actual size by parsing each element

### Requirement 3: Field Iterator Completeness

**User Story:** As a developer, I want the field iterator to yield all accessible fields, so that I can enumerate the complete structure.

#### Acceptance Criteria

1. WHEN iterating over fields, THE `FieldIterator` SHALL yield all fields defined in the structure, including those after repeated TypeRef arrays
2. WHEN a field is conditional and its condition is false, THE iterator SHALL skip that field
3. WHEN a field is a repeated TypeRef, THE iterator SHALL yield indexed paths for each element (e.g., `band_info_0`, `band_info_1`)

### Requirement 4: Nested Type Field Access

**User Story:** As a developer, I want to access fields within nested type instances, so that I can read band information and other nested data.

#### Acceptance Criteria

1. WHEN accessing a field within a nested type using dot notation (e.g., `band_info_0.irepband`), THE accessor SHALL return the correct value
2. WHEN the nested type contains conditional fields, THE accessor SHALL correctly handle field presence
3. WHEN the nested type contains variable-length fields (like LUT data), THE accessor SHALL correctly calculate offsets within the nested structure

### Requirement 5: Image Subheader TRE Field Access

**User Story:** As a developer, I want to access TRE fields (udid, ixshd) from image subheaders, so that TRE metadata extraction works correctly.

#### Acceptance Criteria

1. WHEN parsing an image subheader with the `nitf_02.10_image_subheader` definition, THE accessor SHALL be able to access the `udidl` field
2. WHEN `udidl > 0`, THE accessor SHALL be able to access the `udofl` and `udid` fields
3. WHEN parsing an image subheader, THE accessor SHALL be able to access the `ixshdl` field
4. WHEN `ixshdl > 0`, THE accessor SHALL be able to access the `ixsofl` and `ixshd` fields
5. WHEN calling `fields()` on an image subheader accessor, THE result SHALL include `udidl`, `ixshdl`, and (when present) `udid`, `ixshd`

### Requirement 6: Backward Compatibility

**User Story:** As a developer, I want existing parsing functionality to continue working, so that the fix doesn't break other features.

#### Acceptance Criteria

1. ALL existing unit tests SHALL continue to pass after the fix
2. ALL existing property-based tests SHALL continue to pass after the fix
3. Structures without TypeRef fields SHALL parse identically before and after the fix
4. The fix SHALL not change the public API of StructureAccessor or related types

### Requirement 7: Error Handling

**User Story:** As a developer, I want clear error messages when TypeRef resolution fails, so that I can diagnose definition issues.

#### Acceptance Criteria

1. WHEN a TypeRef references a non-existent type, THE parser SHALL return an error with the type name
2. WHEN a nested type has circular references, THE parser SHALL detect and report the cycle
3. WHEN size calculation fails for a nested type field, THE error SHALL include the field path and nested type name

