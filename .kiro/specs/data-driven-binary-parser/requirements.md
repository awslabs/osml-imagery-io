# Requirements Document

## Introduction

This document defines the requirements for Phase 1 of the JBP (Joint BIIF Profile) implementation project: a data-driven binary parser infrastructure. This foundational component enables declarative structure definitions for parsing and writing NITF headers, TREs, and segments without hardcoding format details. The parser uses YAML-based definition files (Kaitai Struct-compatible subset) to describe binary layouts, supporting lazy reading and sequential writing.

## Glossary

- **Structure_Definition**: A YAML-based declarative specification of a binary format layout, including field types, sizes, conditions, and repetitions
- **Structure_Accessor**: A lazy map-like interface for reading parsed values from binary data on-demand
- **Structure_Writer**: An interface for encoding values into binary format according to a structure definition
- **Structure_Registry**: A component that manages loading, caching, and lookup of structure definitions from multiple search paths
- **Expression_Evaluator**: A component that evaluates expressions for computed values, conditionals, and repeat counts
- **KSY_File**: A YAML file following Kaitai Struct format conventions for defining binary structures
- **BCS-A**: NITF Basic Character Set - Alphanumeric (ASCII subset)
- **BCS-N**: NITF Basic Character Set - Numeric (digits and space)
- **ECS-A**: NITF Extended Character Set - Alphanumeric
- **Value**: A tagged union type representing parsed field values with type conversion methods
- **Zero_Copy**: Memory access pattern where data is read directly from the source buffer without allocation

## Requirements

### Requirement 1: Structure Definition Loading

**User Story:** As a developer, I want to load binary format definitions from YAML files, so that I can parse NITF structures without hardcoding format details.

#### Acceptance Criteria

1. WHEN a valid KSY file path is provided, THE Structure_Definition_Loader SHALL parse the YAML and return a Structure_Definition object
2. WHEN a KSY file contains a `meta` section, THE Structure_Definition_Loader SHALL extract the id, title, and endianness settings
3. WHEN a KSY file contains a `seq` section, THE Structure_Definition_Loader SHALL parse all field definitions in order
4. WHEN a KSY file contains a `types` section, THE Structure_Definition_Loader SHALL parse nested type definitions and make them available for reference
5. WHEN a KSY file contains an `enums` section, THE Structure_Definition_Loader SHALL parse enumeration mappings
6. IF a KSY file has invalid YAML syntax, THEN THE Structure_Definition_Loader SHALL return a descriptive parse error
7. IF a KSY file references an undefined type, THEN THE Structure_Definition_Loader SHALL return an error identifying the missing type

### Requirement 2: Field Type Support

**User Story:** As a developer, I want the parser to support NITF field types, so that I can parse headers and TREs which are primarily ASCII-encoded.

#### Acceptance Criteria

1. THE Structure_Definition_Loader SHALL support fixed-size string types with configurable size and encoding
2. THE Structure_Definition_Loader SHALL support raw byte arrays with configurable size for binary data segments
3. WHEN a string field specifies BCS-A encoding, THE Structure_Definition_Loader SHALL validate alphanumeric character constraints (ASCII 0x20-0x7E)
4. WHEN a string field specifies BCS-N encoding, THE Structure_Definition_Loader SHALL validate numeric character constraints (digits 0-9 and space)
5. WHEN a string field specifies ECS-A encoding, THE Structure_Definition_Loader SHALL support extended character set
6. WHEN a field specifies a `pad` attribute, THE Structure_Definition_Loader SHALL record the padding character for fixed-width fields (typically 0x20 for BCS-A, 0x30 for BCS-N)
7. THE Structure_Definition_Loader SHALL support unsigned integer types (u1, u2, u4) for the limited binary fields in NITF (e.g., mask tables, block offsets)

### Requirement 3: Conditional Fields

**User Story:** As a developer, I want to define fields that are conditionally present, so that I can handle optional NITF structures.

#### Acceptance Criteria

1. WHEN a field definition includes an `if` expression, THE Structure_Definition_Loader SHALL record the condition
2. WHEN parsing binary data with a conditional field, THE Structure_Accessor SHALL evaluate the condition to determine field presence
3. WHEN a conditional field's condition evaluates to false, THE Structure_Accessor SHALL skip the field and not include it in accessible paths
4. WHEN a conditional field's condition evaluates to true, THE Structure_Accessor SHALL parse and include the field normally
5. WHEN checking field existence with `has()`, THE Structure_Accessor SHALL return false for conditional fields whose condition is not met

### Requirement 4: Repetitions

**User Story:** As a developer, I want to define repeating fields with various termination conditions, so that I can parse arrays and variable-length sequences.

#### Acceptance Criteria

1. WHEN a field specifies `repeat: expr` with `repeat-expr`, THE Structure_Accessor SHALL parse the field the number of times specified by the expression
2. WHEN a field specifies `repeat: until` with `repeat-until`, THE Structure_Accessor SHALL parse until the condition becomes true
3. WHEN a field specifies `repeat: eos`, THE Structure_Accessor SHALL parse until end of stream/buffer
4. WHEN accessing repeated fields, THE Structure_Accessor SHALL use underscore-indexed naming: `field_0`, `field_1`, etc.
5. WHEN accessing nested fields within repeated elements, THE Structure_Accessor SHALL support paths like `field_0.subfield`
6. WHEN a repeat expression references another field, THE Structure_Accessor SHALL evaluate that field first to determine the count

### Requirement 5: Structure Accessor Reading

**User Story:** As a developer, I want a lazy map-like interface to read parsed values, so that I can efficiently access specific fields without parsing the entire structure.

#### Acceptance Criteria

1. WHEN accessing a field by path using bracket notation, THE Structure_Accessor SHALL return the parsed Value
2. WHEN accessing a nested field using dot notation (e.g., "parent.child"), THE Structure_Accessor SHALL navigate the structure hierarchy
3. WHEN a field path does not exist, THE Structure_Accessor SHALL return an UnknownField error
4. WHEN calling `has(path)`, THE Structure_Accessor SHALL return true if the field exists and is accessible
5. WHEN calling `fields()`, THE Structure_Accessor SHALL return an iterator over all accessible field paths
6. WHEN calling `field_info(path)`, THE Structure_Accessor SHALL return metadata including type, size, and offset
7. THE Structure_Accessor SHALL cache computed field offsets for repeated access efficiency

### Requirement 6: Value Type Conversions

**User Story:** As a developer, I want parsed values to provide type conversion methods, so that I can interpret ASCII-encoded numeric fields as integers or floats.

#### Acceptance Criteria

1. WHEN calling `as_str()` on a Value, THE Value SHALL return the string representation (trimming padding if applicable)
2. WHEN calling `as_i64()` on a Value containing a BCS-N string, THE Value SHALL parse and return the signed integer value
3. WHEN calling `as_u64()` on a Value containing a BCS-N string, THE Value SHALL parse and return the unsigned integer value
4. WHEN calling `as_f64()` on a Value containing a numeric string, THE Value SHALL parse and return the floating-point value
5. WHEN calling `as_bytes()` on a Value, THE Value SHALL return the raw byte representation
6. IF a string Value cannot be parsed as the requested numeric type, THEN THE conversion method SHALL return a ConversionError

### Requirement 7: Zero-Copy Raw Data Access

**User Story:** As a developer, I want zero-copy access to raw binary data, so that I can pass large fields directly to third-party decoders without memory allocation.

#### Acceptance Criteria

1. WHEN calling `raw_slice(path)`, THE Structure_Accessor SHALL return a byte slice referencing the original buffer directly
2. WHEN calling `field_byte_range(path)`, THE Structure_Accessor SHALL return the (offset, length) tuple for the field
3. THE raw_slice method SHALL NOT allocate new memory for the returned data
4. WHEN the accessor is backed by a memory-mapped file, THE raw_slice SHALL reference the mapped memory directly
5. IF a field is not backed by contiguous memory, THEN THE raw_slice method SHALL return an error indicating a copy is required

### Requirement 8: Structure Writer Fixed-Size Mode

**User Story:** As a developer, I want to write fixed-size structures with fields in any order, so that I can efficiently encode known-size NITF components.

#### Acceptance Criteria

1. WHEN creating a Structure_Writer with a fixed-size definition, THE Structure_Writer SHALL pre-allocate a buffer of the correct size
2. WHEN writing a field value, THE Structure_Writer SHALL encode it at the correct offset regardless of write order
3. WHEN calling `finish()`, THE Structure_Writer SHALL return the encoded bytes
4. IF a required field is not written before `finish()`, THEN THE Structure_Writer SHALL return a MissingRequired error
5. WHEN a value exceeds the field's defined size, THE Structure_Writer SHALL return a ValueTooLarge error
6. WHEN a string value is shorter than the field size, THE Structure_Writer SHALL apply the defined padding character

### Requirement 9: Structure Writer Streaming Mode

**User Story:** As a developer, I want to write variable-size structures sequentially, so that I can encode structures with repeating elements.

#### Acceptance Criteria

1. WHEN creating a streaming Structure_Writer, THE Structure_Writer SHALL accept a growable output buffer
2. WHEN writing fields in streaming mode, THE Structure_Writer SHALL require fields to be written in definition order
3. IF a field is written out of order in streaming mode, THEN THE Structure_Writer SHALL return an OutOfOrder error
4. WHEN writing repeated fields, THE Structure_Writer SHALL accept underscore-indexed paths (e.g., "field_0", "field_1")
5. WHEN calling `write_to(writer)`, THE Structure_Writer SHALL stream encoded bytes to the provided writer

### Requirement 10: Write Validation

**User Story:** As a developer, I want the writer to validate values against field constraints, so that I can catch encoding errors early.

#### Acceptance Criteria

1. WHEN writing a value with incorrect type, THE Structure_Writer SHALL return a ConversionError
2. WHEN writing a string to a BCS-N field with non-numeric characters, THE Structure_Writer SHALL return a validation error
3. WHEN writing a string to a BCS-A field with invalid characters, THE Structure_Writer SHALL return a validation error
4. WHEN writing a value that exceeds the field's maximum size, THE Structure_Writer SHALL return a ValueTooLarge error
5. THE Structure_Writer SHALL enforce size constraints for all fixed-width fields

### Requirement 11: Structure Registry

**User Story:** As a developer, I want a registry that manages structure definitions with hierarchical search paths, so that I can use built-in definitions and override them with custom ones.

#### Acceptance Criteria

1. WHEN creating a Structure_Registry, THE Structure_Registry SHALL initialize with default search paths
2. THE Structure_Registry SHALL search paths in order: built-in → package data → user override → current directory
3. WHEN the `OSML_IO_STRUCTURE_PATH` environment variable is set, THE Structure_Registry SHALL include those paths in the search order
4. WHEN calling `get(name)`, THE Structure_Registry SHALL return the structure definition or None if not found
5. WHEN multiple definitions exist for the same name, THE Structure_Registry SHALL use the one from the highest-priority path
6. WHEN calling `list()`, THE Structure_Registry SHALL return all available structure names
7. WHEN calling `reload()`, THE Structure_Registry SHALL refresh definitions from disk
8. WHEN calling `register(name, definition)`, THE Structure_Registry SHALL add a runtime definition that takes highest priority
9. THE Structure_Registry SHALL cache loaded definitions for performance

### Requirement 12: Registry Naming Convention

**User Story:** As a developer, I want consistent naming for structure definitions, so that I can easily find and reference NITF components.

#### Acceptance Criteria

1. THE Structure_Registry SHALL use the naming pattern `NITF_02.10_FileHeader` for NITF 2.1 file headers
2. THE Structure_Registry SHALL use the naming pattern `NITF_02.10_ImageSubheader` for NITF 2.1 image subheaders
3. THE Structure_Registry SHALL use the naming pattern `NSIF_01.00_FileHeader` for NSIF 1.0 file headers
4. THE Structure_Registry SHALL use the naming pattern `TRE_GEOLOB` for TRE definitions
5. THE Structure_Registry SHALL use the naming pattern `DES_TRE_OVERFLOW` for DES definitions

### Requirement 13: Expression Evaluator

**User Story:** As a developer, I want an expression evaluator for computed values and conditions, so that I can define dynamic field relationships.

#### Acceptance Criteria

1. THE Expression_Evaluator SHALL support field references using dot-notation paths
2. THE Expression_Evaluator SHALL support arithmetic operators: +, -, *, /, %
3. THE Expression_Evaluator SHALL support comparison operators: ==, !=, <, >, <=, >=
4. THE Expression_Evaluator SHALL support logical operators: and, or, not
5. THE Expression_Evaluator SHALL support type coercion methods: .to_i, .to_s, .length
6. THE Expression_Evaluator SHALL support special variables: _index, _root, _parent, _io
7. WHEN an expression references an unparsed field, THE Expression_Evaluator SHALL trigger parsing of that field first
8. IF an expression contains a syntax error, THEN THE Expression_Evaluator SHALL return a descriptive error

### Requirement 14: Python Bindings

**User Story:** As a Python developer, I want to use the parser from Python, so that I can integrate it with the osml-imagery-toolkit ecosystem.

#### Acceptance Criteria

1. THE Python bindings SHALL expose StructureRegistry class with get(), list(), reload(), and register() methods
2. THE Python bindings SHALL expose StructureAccessor class with dict-like access via __getitem__
3. THE Python bindings SHALL expose StructureWriter class with dict-like write access
4. THE Python bindings SHALL expose Value class with as_str(), as_int(), as_float(), as_bytes() methods
5. WHEN calling raw_view() on a Python accessor, THE bindings SHALL return a memoryview sharing the underlying buffer
6. THE Python bindings SHALL support memory-mapped file objects as input to StructureAccessor

### Requirement 15: Error Handling

**User Story:** As a developer, I want descriptive error messages with context, so that I can diagnose parsing and writing issues.

#### Acceptance Criteria

1. WHEN a parse error occurs, THE error SHALL include the field path where the error occurred
2. WHEN an unexpected EOF occurs, THE error SHALL include expected size and available bytes
3. WHEN a type mismatch occurs, THE error SHALL include expected and actual types
4. WHEN a write error occurs, THE error SHALL include the field path and constraint violated
5. WHEN an expression error occurs, THE error SHALL include the expression text and failure reason

### Requirement 16: Round-Trip Consistency

**User Story:** As a developer, I want to verify that parsing and writing are symmetric, so that I can trust the parser for data preservation.

#### Acceptance Criteria

1. FOR ALL valid binary data matching a structure definition, parsing then writing SHALL produce identical bytes
2. FOR ALL valid field values, writing then parsing SHALL produce equivalent values
3. THE parser SHALL preserve unknown/unrecognized bytes in raw form for round-trip fidelity
