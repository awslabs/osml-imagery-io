# Data-Driven Binary Parser

This document describes the data-driven binary parser infrastructure in `src/parser/`. The parser uses declarative YAML-based structure definitions inspired by [Kaitai Struct](https://kaitai.io/) to parse and write binary data without hardcoding format details.

## Relationship to Kaitai Struct

Our definition format is inspired by Kaitai Struct's `.ksy` YAML format but is not fully compatible. We implement a subset of Kaitai features tailored for NITF parsing, with some extensions (like NITF-specific character encodings) and some omissions (like `instances` and `params`).

Key differences from Kaitai Struct:

| Aspect | Kaitai Struct | Our Implementation |
|--------|---------------|-------------------|
| Execution model | Compiles to target language code | Runtime interpretation |
| Expression syntax | Full Kaitai expression language | Subset (see below) |
| `instances` | Supported | Not implemented |
| `params` | Supported | Not implemented |
| Array indexing | `arr[0]` syntax | `arr_0` naming convention |
| Bitwise operators | Supported | Not yet implemented |
| NITF encodings | Not built-in | BCS-A, BCS-N, ECS-A support |
| Writing support | Limited | Full bidirectional read/write |

Our definition files use the `.ksy` extension for familiarity but should be considered a Kaitai-inspired format rather than true Kaitai Struct files. They may not work with the official Kaitai Struct compiler.

## Architecture Overview

```
src/parser/
├── mod.rs              # Public API exports
├── definition.rs       # YAML definition loading (uses serde_yaml)
├── types.rs            # Core types: StructureDefinition, FieldDefinition, etc.
├── value.rs            # Value type with type conversions
├── error.rs            # Error types (uses thiserror)
├── encoding.rs         # NITF character set validation (BCS-A, BCS-N, ECS-A)
├── registry.rs         # Structure definition registry with search paths
├── accessor/           # Reading binary data
│   ├── mod.rs          # StructureAccessor public API
│   ├── offset.rs       # Field offset calculation
│   ├── read.rs         # Value reading from bytes
│   ├── iterator.rs     # Field iteration
│   └── context.rs      # Expression evaluation context building
├── expression/         # Expression parsing and evaluation
│   ├── mod.rs          # Expression AST types
│   ├── lexer.rs        # Hand-written tokenizer
│   ├── parser.rs       # Recursive descent parser
│   ├── eval.rs         # Expression evaluator
│   └── ops.rs          # Operator implementations
└── writer/             # Writing binary data
    ├── mod.rs          # StructureWriter public API
    ├── encode.rs       # Value encoding
    ├── integer.rs      # Integer encoding
    ├── fixed.rs        # Fixed-size structure writing
    ├── streaming.rs    # Variable-size streaming writing
    └── validation.rs   # Write validation
```

## Key Components

### 1. Structure Definitions

Structure definitions are loaded from YAML files using `serde_yaml`. The `DefinitionLoader` deserializes into intermediate `Raw*` structs, then converts to the final `StructureDefinition` type.

```rust
// Loading a definition
let def = DefinitionLoader::load_file(Path::new("nitf_file_header.ksy"))?;
let def = DefinitionLoader::load_str(yaml_string)?;
```

#### Supported KSY Features

| Feature | Status | Notes |
|---------|--------|-------|
| `meta.id`, `meta.title`, `meta.endian` | ✓ | Required id, optional title |
| `seq` fields | ✓ | Sequential field definitions |
| `types` (nested) | ✓ | Recursive type definitions |
| `enums` | ✓ | Integer-to-name mappings |
| Field types: `u1-u8`, `s1-s8`, `str` | ✓ | Integer and string types |
| Endian-specific types: `u2be`, `u4le` | ✓ | Parsed but endian from meta |
| `size` (fixed and expression) | ✓ | Fixed integer or expression |
| `encoding` | ✓ | ASCII, BCS-A, BCS-N, ECS-A |
| `pad-right` | ✓ | Padding character |
| `if` (conditional) | ✓ | Expression-based conditions |
| `repeat: expr` | ✓ | Expression-based count |
| `repeat: until` | ✓ | Condition-based termination |
| `repeat: eos` | ✓ | Read until end of stream |
| `doc` | ✓ | Documentation strings |
| `instances` | ✗ | Not implemented |
| `params` | ✗ | Not implemented |
| Bit fields (`b1`, `b4`) | Partial | Parsed as bytes |

### 2. StructureAccessor (Reading)

The `StructureAccessor` provides lazy, map-like access to binary data:

```rust
let accessor = StructureAccessor::new(Arc::new(def), &data)?;

// Access fields by path
let version = accessor.get("fver")?.as_str()?;
let num_images = accessor.get("numi")?.as_i64()?;

// Repeated fields use underscore-indexed naming
let first_len = accessor.get("image_info_0.li")?.as_str()?;

// Check field existence
if accessor.has("optional_field") { ... }

// Zero-copy raw slice access
let raw_bytes: &[u8] = accessor.raw_slice("data_field")?;
let (offset, len) = accessor.field_byte_range("data_field")?;

// Iterate all fields
for field_path in accessor.fields() { ... }
```

#### Offset Calculation

Field offsets are calculated on-demand by walking the field sequence:
1. Fixed-size fields: offset computed from preceding field sizes
2. Variable-size fields: preceding fields parsed to determine sizes
3. Conditional fields: condition evaluated to determine presence
4. Repeated fields: repeat count evaluated, element sizes computed

### 3. StructureWriter (Writing)

The `StructureWriter` supports two modes:

```rust
// Fixed-size mode: pre-allocated buffer, random access
let mut writer = StructureWriter::new_fixed(Arc::new(def))?;
writer.set("field_a", "value")?;
writer.set("field_b", 42i64)?;
let bytes = writer.finish()?;

// Streaming mode: sequential writes, growable buffer
let mut writer = StructureWriter::new_streaming(Arc::new(def));
writer.set("field_a", "value")?;  // Must be in order
writer.set("field_b", 42i64)?;
let bytes = writer.finish()?;
```

### 4. Expression Evaluator

The expression system parses and evaluates Kaitai Struct-style expressions:

```rust
let expr = ExpressionEvaluator::parse("numi.to_i * 16 + 388")?;
let evaluator = ExpressionEvaluator::new();
let result = evaluator.evaluate(&expr, &context)?;
```

#### Supported Expression Syntax

| Category | Syntax | Example |
|----------|--------|---------|
| Literals | integers, floats, strings, booleans | `42`, `3.14`, `"text"`, `true` |
| Field references | dot-notation paths | `header.version`, `items_0.value` |
| Arithmetic | `+`, `-`, `*`, `/`, `%` | `width * height` |
| Comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` | `version >= 2` |
| Logical | `and`, `or`, `not` | `a > 0 and b < 10` |
| Methods | `.to_i`, `.to_s`, `.length` | `numi.to_i`, `data.length` |
| Special vars | `_index` | Current repeat index |
| Parentheses | `(expr)` | `(a + b) * c` |

#### Not Supported

- Array indexing: `arr[0]` (use `arr_0` naming instead)
- `_root`, `_parent`, `_io` (parsed but not evaluated)
- Ternary operator: `a ? b : c`
- Bitwise operators: `&`, `|`, `^`, `<<`, `>>` (planned, see `docs/PARSER_SUGGESTIONS.md`)

#### Why a Custom Expression Evaluator?

We use a hand-written expression evaluator rather than a third-party library for several reasons:

1. **Kaitai-specific syntax**: The `.to_i`, `.to_s`, and `.length` method call syntax is specific to Kaitai Struct. Generic expression libraries like `meval` don't support method calls, and adapting them would require significant preprocessing.

2. **License constraints**: This project requires Apache-2.0, MIT, BSD, ISC, Zlib, or public domain licenses. Some expression evaluation crates (e.g., `evalexpr`) use AGPL licensing which is incompatible.

3. **NITF-specific needs**: NITF fields are often ASCII strings representing numbers (e.g., `"003"` for the count 3). The `.to_i` method handles this conversion naturally. Generic evaluators would need custom type coercion.

4. **Minimal dependencies**: The custom evaluator adds no external dependencies and is ~1500 lines of well-tested code organized into lexer, parser, evaluator, and operator modules.

The expression evaluator is sufficient for NITF structure definitions. Future enhancements (bitwise operators for existence masks) can be added incrementally.

### 5. Structure Registry

The registry manages loading and caching of structure definitions:

```rust
let mut registry = StructureRegistry::new();
registry.add_search_path("data/structures/tre");

// Get definition (loads and caches)
let def = registry.get("TRE_GEOLOB")?;

// Runtime registration
registry.register("CUSTOM", custom_def);

// List available definitions
for name in registry.list() { ... }
```

#### Naming Convention

| Prefix | Example | File Path |
|--------|---------|-----------|
| `TRE_` | `TRE_GEOLOB` | `tre/tre_geolob.ksy` |
| `DES_` | `DES_TRE_OVERFLOW` | `des/des_tre_overflow.ksy` |
| `NITF_` | `NITF_02.10_FileHeader` | `nitf/nitf_02.10_file_header.ksy` |
| `NSIF_` | `NSIF_01.00_FileHeader` | `nsif/nsif_01.00_file_header.ksy` |

### 6. NITF Character Set Encoding

The `encoding` module validates NITF-specific character sets:

```rust
// BCS-A: Basic Character Set - Alphanumeric (0x20-0x7E)
encoding::validate_bcs_a(data) -> bool
encoding::validate_bcs_a_detailed(data) -> ValidationResult

// BCS-N: Basic Character Set - Numeric (0x20, 0x2B-0x2D, 0x30-0x39)
encoding::validate_bcs_n(data) -> bool

// ECS-A: Extended Character Set - Alphanumeric (0x20-0x7E, 0xA0-0xFF)
encoding::validate_ecs_a(data) -> bool
```

## Value Type

The `Value` enum represents parsed field values with conversion methods:

```rust
pub enum Value<'a> {
    String(Cow<'a, str>),      // String fields
    Bytes(&'a [u8]),           // Raw byte fields
    Unsigned(u64),             // Unsigned integers
    Struct(StructValue<'a>),   // Nested structures
    Array(Vec<Value<'a>>),     // Repeated fields
}

// Conversions (handle NITF's ASCII-numeric fields)
value.as_str()?      // Trims padding
value.as_i64()?      // Parses numeric strings
value.as_u64()?      // Parses unsigned strings
value.as_f64()?      // Parses float strings
value.as_bytes()     // Raw bytes
```

## Error Types

```rust
LoadError       // Definition loading errors (YAML, missing fields, invalid types)
AccessError     // Reading errors (unknown field, EOF, encoding, expression)
WriteError      // Writing errors (out of order, too large, missing required)
ConversionError // Value conversion errors (type mismatch, parse failure)
ExpressionError // Expression errors (syntax, unknown field, type, division by zero)
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde_yaml` | YAML deserialization for KSY files |
| `serde` | Derive macros for deserialization |
| `thiserror` | Error type derivation |

## Repeated Field Naming Convention

Repeated fields use underscore-indexed naming: `{field_id}_{index}` (zero-based).

```yaml
seq:
  - id: num_segments
    type: u2
  - id: segment_info
    type: segment_entry
    repeat: expr
    repeat-expr: num_segments
```

If `num_segments` is 3:
- `segment_info_0` - First entry
- `segment_info_0.offset` - Nested field access
- `segment_info_1` - Second entry
- `segment_info_2` - Third entry

## Example: NITF File Header

```yaml
meta:
  id: nitf_file_header
  title: NITF 2.1 File Header
  endian: be

seq:
  - id: fhdr
    type: str
    size: 4
    encoding: BCS-A
    doc: File profile name (NITF or NSIF)
    
  - id: fver
    type: str
    size: 5
    encoding: BCS-A
    doc: File version (02.10)
    
  - id: numi
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of image segments
    
  - id: image_info
    type: image_segment_info
    repeat: expr
    repeat-expr: numi.to_i

types:
  image_segment_info:
    seq:
      - id: lish
        type: str
        size: 6
        encoding: BCS-N
      - id: li
        type: str
        size: 10
        encoding: BCS-N
```

```rust
let accessor = StructureAccessor::new(def, &data)?;
let version = accessor.get("fver")?.as_str()?;  // "02.10"
let num_images = accessor.get("numi")?.as_i64()?;  // 2

for i in 0..num_images {
    let path = format!("image_info_{}.li", i);
    let image_len = accessor.get(&path)?.as_i64()?;
}
```

## Comparison with Original Design Document

The implementation matches the original design with these differences:

| Feature | Original Design | Implementation |
|---------|-----------------|----------------|
| `instances` | Planned | Not implemented |
| `params` | Not mentioned | Not implemented |
| `_root`, `_parent`, `_io` | Planned | Parsed but not evaluated |
| Array indexing `arr[0]` | Planned | Use `arr_0` naming |
| Bitwise operators | Not mentioned | Not supported |
| Ternary operator | Not mentioned | Not supported |
| Python bindings | Planned | Not yet implemented in parser |
| Offset caching | Planned | Not implemented (computed on-demand) |

## See Also

- `internal/STRUCTURES_LIMITATIONS.md` - Known limitations in TRE/DES definitions
- `internal/PARSER_SUGGESTIONS.md` - Planned improvements (offset caching, bitwise operators)
