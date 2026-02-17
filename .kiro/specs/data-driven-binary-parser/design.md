# Design Document: Data-Driven Binary Parser

## Overview

This design describes a data-driven binary parser infrastructure for NITF/NSIF format handling. The system uses declarative YAML-based structure definitions (Kaitai Struct-compatible subset) to parse and write binary data without hardcoding format details. The key innovation is runtime interpretation of structure definitions rather than compile-time code generation, enabling user-extensible format support.

The parser is optimized for NITF's ASCII-centric design where most fields are fixed-width character strings (BCS-A, BCS-N, ECS-A) with numeric values encoded as ASCII digits. Binary types are limited to specific use cases like mask tables.

### Key Design Decisions

1. **Runtime Interpretation**: Structure definitions are interpreted at runtime rather than compiled, enabling dynamic loading of user-provided TRE definitions without recompilation.

2. **Lazy Evaluation**: Fields are parsed on-demand when accessed, with offset caching for repeated access. This is critical for large NITF files where only specific fields may be needed.

3. **Zero-Copy Access**: Raw data access returns slices into the original buffer (or memory-mapped file), avoiding allocation for large binary segments like image data.

4. **Bidirectional Symmetry**: The same structure definition drives both reading and writing, ensuring round-trip consistency.

## Architecture

```mermaid
graph TB
    subgraph "Structure Definitions"
        KSY[".ksy YAML Files"]
        Loader["Definition Loader"]
        Def["StructureDefinition"]
    end
    
    subgraph "Registry"
        Registry["StructureRegistry"]
        Cache["Definition Cache"]
        Paths["Search Paths"]
    end
    
    subgraph "Reading"
        Accessor["StructureAccessor"]
        ExprEval["ExpressionEvaluator"]
        OffsetCache["Offset Cache"]
    end
    
    subgraph "Writing"
        Writer["StructureWriter"]
        FixedBuf["Fixed Buffer Mode"]
        StreamBuf["Streaming Mode"]
    end
    
    subgraph "Values"
        Value["Value"]
        Conv["Type Conversions"]
    end
    
    KSY --> Loader
    Loader --> Def
    Def --> Registry
    Registry --> Cache
    Paths --> Registry
    
    Registry --> Accessor
    Def --> Accessor
    Accessor --> ExprEval
    Accessor --> OffsetCache
    Accessor --> Value
    
    Registry --> Writer
    Def --> Writer
    Writer --> FixedBuf
    Writer --> StreamBuf
    
    Value --> Conv
```

### Component Interactions

1. **Loading Flow**: KSY files → Loader → StructureDefinition → Registry (cached)
2. **Reading Flow**: Binary data + Definition → Accessor → lazy parse → Value → type conversion
3. **Writing Flow**: Values + Definition → Writer → encode → binary output

## Components and Interfaces

### StructureDefinition

Represents a parsed structure definition from a KSY file.

```rust
/// A complete structure definition parsed from a KSY file
pub struct StructureDefinition {
    /// Unique identifier for this structure
    pub id: String,
    /// Human-readable title
    pub title: Option<String>,
    /// Default byte order (big or little endian)
    pub endian: Endian,
    /// Ordered sequence of field definitions
    pub fields: Vec<FieldDefinition>,
    /// Named nested type definitions
    pub types: HashMap<String, StructureDefinition>,
    /// Enumeration definitions
    pub enums: HashMap<String, EnumDefinition>,
}

/// Definition of a single field in a structure
pub struct FieldDefinition {
    /// Field identifier (used in path access)
    pub id: String,
    /// Field type specification
    pub field_type: FieldType,
    /// Size in bytes (for strings/bytes) or expression
    pub size: SizeSpec,
    /// Character encoding for strings
    pub encoding: Option<Encoding>,
    /// Padding character for fixed-width fields
    pub pad: Option<u8>,
    /// Conditional expression (field present only if true)
    pub condition: Option<Expression>,
    /// Repetition specification
    pub repeat: Option<RepeatSpec>,
    /// Documentation string
    pub doc: Option<String>,
}

/// Supported field types
pub enum FieldType {
    /// Fixed-size string with encoding
    String,
    /// Raw byte array
    Bytes,
    /// Unsigned integer (1, 2, or 4 bytes)
    UnsignedInt(u8),
    /// Reference to a nested type
    TypeRef(String),
}

/// Character encodings for NITF
pub enum Encoding {
    /// ASCII (default)
    Ascii,
    /// NITF Basic Character Set - Alphanumeric
    BcsA,
    /// NITF Basic Character Set - Numeric
    BcsN,
    /// NITF Extended Character Set - Alphanumeric
    EcsA,
}

/// Size specification (fixed or expression-based)
pub enum SizeSpec {
    Fixed(usize),
    Expression(Expression),
}

/// Repetition specification
pub enum RepeatSpec {
    /// Repeat a fixed number of times
    Count(usize),
    /// Repeat based on expression result
    Expression(Expression),
    /// Repeat until condition is true
    Until(Expression),
    /// Repeat until end of stream
    Eos,
}
```

### DefinitionLoader

Parses KSY YAML files into StructureDefinition objects.

```rust
/// Loads structure definitions from KSY YAML files
pub struct DefinitionLoader;

impl DefinitionLoader {
    /// Parse a KSY file from a path
    pub fn load_file(path: &Path) -> Result<StructureDefinition, LoadError>;
    
    /// Parse a KSY definition from a string
    pub fn load_str(yaml: &str) -> Result<StructureDefinition, LoadError>;
    
    /// Parse a KSY definition from a reader
    pub fn load_reader<R: Read>(reader: R) -> Result<StructureDefinition, LoadError>;
}

/// Errors during definition loading
pub enum LoadError {
    /// YAML syntax error
    YamlError { source: serde_yaml::Error },
    /// Missing required field in definition
    MissingField { field: String, context: String },
    /// Invalid field type specification
    InvalidType { type_str: String, context: String },
    /// Reference to undefined type
    UndefinedType { type_name: String, context: String },
    /// Invalid expression syntax
    InvalidExpression { expr: String, source: ExpressionError },
    /// I/O error reading file
    IoError { source: std::io::Error },
}
```

### StructureRegistry

Manages structure definitions with hierarchical search paths.

```rust
/// Registry for structure definitions with search path resolution
pub struct StructureRegistry {
    /// Cached definitions by name
    definitions: HashMap<String, Arc<StructureDefinition>>,
    /// Search paths in priority order (later overrides earlier)
    search_paths: Vec<PathBuf>,
}

impl StructureRegistry {
    /// Create registry with default search paths
    pub fn new() -> Result<Self, RegistryError>;
    
    /// Add a search path (higher priority than existing)
    pub fn add_search_path(&mut self, path: impl AsRef<Path>);
    
    /// Get a structure definition by name
    pub fn get(&self, name: &str) -> Option<Arc<StructureDefinition>>;
    
    /// List all available structure names
    pub fn list(&self) -> Vec<String>;
    
    /// Reload all definitions from disk
    pub fn reload(&mut self) -> Result<(), RegistryError>;
    
    /// Register a definition at runtime (highest priority)
    pub fn register(&mut self, name: &str, def: StructureDefinition);
}

/// Default search path order:
/// 1. Runtime-registered definitions (highest priority)
/// 2. OSML_IO_STRUCTURE_PATH environment variable paths
/// 3. Package data directory: $CARGO_MANIFEST_DIR/data/structures/
/// 4. Built-in definitions compiled into the library
```

### StructureAccessor

Lazy map-like interface for reading parsed values.

```rust
/// Lazy accessor for reading structure fields from binary data
pub struct StructureAccessor<'a> {
    /// The structure definition
    definition: Arc<StructureDefinition>,
    /// Source data buffer
    data: &'a [u8],
    /// Cached field offsets for repeated access
    offset_cache: RefCell<HashMap<String, (usize, usize)>>,
    /// Expression evaluator
    evaluator: ExpressionEvaluator,
}

impl<'a> StructureAccessor<'a> {
    /// Create accessor from definition and data buffer
    pub fn new(
        definition: Arc<StructureDefinition>,
        data: &'a [u8],
    ) -> Result<Self, AccessError>;
    
    /// Access a field by dot-notation path
    pub fn get(&self, path: &str) -> Result<Value<'a>, AccessError>;
    
    /// Check if a field exists and is accessible
    pub fn has(&self, path: &str) -> bool;
    
    /// Get field metadata (type, size, offset)
    pub fn field_info(&self, path: &str) -> Option<FieldInfo>;
    
    /// Iterate over all accessible field paths
    pub fn fields(&self) -> impl Iterator<Item = String>;
    
    /// Get raw byte slice for a field (zero-copy)
    pub fn raw_slice(&self, path: &str) -> Result<&'a [u8], AccessError>;
    
    /// Get byte offset and length for a field
    pub fn field_byte_range(&self, path: &str) -> Result<(usize, usize), AccessError>;
}

/// Implement Index trait for bracket notation access
impl<'a> std::ops::Index<&str> for StructureAccessor<'a> {
    type Output = Value<'a>;
    
    fn index(&self, path: &str) -> &Self::Output {
        // Note: This panics on error; use get() for Result
        self.get(path).expect("field access failed")
    }
}
```

### Value

Tagged union representing parsed field values with type conversions.

```rust
/// A parsed field value with type conversion methods
pub enum Value<'a> {
    /// String value (may reference source buffer)
    String(Cow<'a, str>),
    /// Raw bytes (references source buffer)
    Bytes(&'a [u8]),
    /// Unsigned integer
    Unsigned(u64),
    /// Nested structure accessor
    Struct(Box<StructureAccessor<'a>>),
    /// Array of values
    Array(Vec<Value<'a>>),
}

impl<'a> Value<'a> {
    /// Get as string, trimming padding
    pub fn as_str(&self) -> Result<&str, ConversionError>;
    
    /// Parse as signed integer (for BCS-N strings)
    pub fn as_i64(&self) -> Result<i64, ConversionError>;
    
    /// Parse as unsigned integer (for BCS-N strings)
    pub fn as_u64(&self) -> Result<u64, ConversionError>;
    
    /// Parse as floating-point (for numeric strings)
    pub fn as_f64(&self) -> Result<f64, ConversionError>;
    
    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8];
}
```

### StructureWriter

Interface for encoding values into binary format.

```rust
/// Writer for encoding values according to a structure definition
pub struct StructureWriter {
    /// The structure definition
    definition: Arc<StructureDefinition>,
    /// Output buffer
    buffer: Vec<u8>,
    /// Current write position (for streaming mode)
    position: usize,
    /// Fields that have been written
    written: HashSet<String>,
    /// Writing mode
    mode: WriterMode,
}

pub enum WriterMode {
    /// Fixed-size buffer, fields can be written in any order
    Fixed { size: usize },
    /// Streaming mode, fields must be written in order
    Streaming,
}

impl StructureWriter {
    /// Create writer for fixed-size structure
    pub fn new_fixed(definition: Arc<StructureDefinition>) -> Result<Self, WriteError>;
    
    /// Create streaming writer for variable-size structure
    pub fn new_streaming(definition: Arc<StructureDefinition>) -> Self;
    
    /// Write a value to a field
    pub fn set(&mut self, path: &str, value: impl Into<WriteValue>) -> Result<(), WriteError>;
    
    /// Check if a field has been written
    pub fn is_set(&self, path: &str) -> bool;
    
    /// Finalize and return encoded bytes
    pub fn finish(self) -> Result<Vec<u8>, WriteError>;
    
    /// Write to an output stream
    pub fn write_to<W: Write>(self, writer: W) -> Result<usize, WriteError>;
}

/// Value types accepted for writing
pub enum WriteValue {
    String(String),
    Bytes(Vec<u8>),
    Integer(i64),
    Unsigned(u64),
    Float(f64),
}
```

### ExpressionEvaluator

Evaluates expressions for computed values, conditionals, and repeat counts.

```rust
/// Evaluates expressions in the context of a structure
pub struct ExpressionEvaluator {
    /// Root accessor for field references
    root: Option<Weak<StructureAccessor<'static>>>,
}

impl ExpressionEvaluator {
    /// Evaluate an expression to a value
    pub fn evaluate(
        &self,
        expr: &Expression,
        context: &EvalContext,
    ) -> Result<EvalResult, ExpressionError>;
}

/// Parsed expression AST
pub enum Expression {
    /// Literal value
    Literal(Literal),
    /// Field reference (dot-notation path)
    FieldRef(String),
    /// Binary operation
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    /// Unary operation
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },
    /// Method call (.to_i, .to_s, .length)
    MethodCall {
        target: Box<Expression>,
        method: String,
    },
    /// Special variable (_index, _root, _parent, _io)
    SpecialVar(SpecialVariable),
}

pub enum BinaryOperator {
    Add, Sub, Mul, Div, Mod,           // Arithmetic
    Eq, Ne, Lt, Gt, Le, Ge,            // Comparison
    And, Or,                            // Logical
}

pub enum UnaryOperator {
    Not, Neg,
}

pub enum SpecialVariable {
    Index,   // Current repetition index
    Root,    // Root structure
    Parent,  // Parent structure
    Io,      // I/O stream info (pos, size, eof)
}
```

## Data Models

### Field Path Resolution

Field paths use dot notation with underscore-indexed naming for repeated fields:

```
field                    → Simple field access
parent.child             → Nested field access
repeated_0               → First element of repeated field
repeated_0.subfield      → Subfield of first repeated element
repeated_1.nested_2.val  → Deeply nested repeated access
```

### Offset Calculation

For lazy evaluation, field offsets are calculated on-demand:

1. **Fixed-offset fields**: Offset computed at definition load time
2. **Variable-offset fields**: Requires parsing preceding fields
3. **Conditional fields**: May shift subsequent field offsets
4. **Repeated fields**: Each element's offset depends on previous elements

```rust
/// Cached offset information for a field
pub struct FieldOffset {
    /// Absolute byte offset from structure start
    pub offset: usize,
    /// Field size in bytes
    pub size: usize,
    /// Whether this offset is fixed or was computed
    pub is_fixed: bool,
}
```

### NITF Character Set Validation

```rust
/// Validate BCS-A (Basic Character Set - Alphanumeric)
/// Valid: ASCII 0x20-0x7E (space through tilde)
fn validate_bcs_a(data: &[u8]) -> bool {
    data.iter().all(|&b| b >= 0x20 && b <= 0x7E)
}

/// Validate BCS-N (Basic Character Set - Numeric)
/// Valid: ASCII 0x30-0x39 (digits 0-9) and 0x20 (space)
fn validate_bcs_n(data: &[u8]) -> bool {
    data.iter().all(|&b| (b >= 0x30 && b <= 0x39) || b == 0x20)
}

/// Validate ECS-A (Extended Character Set - Alphanumeric)
/// Valid: Full Latin-1 range with some exclusions
fn validate_ecs_a(data: &[u8]) -> bool {
    // ECS-A allows broader character range
    data.iter().all(|&b| b >= 0x20)
}
```

### Search Path Resolution

```rust
/// Resolve structure definition file from search paths
fn resolve_definition(name: &str, paths: &[PathBuf]) -> Option<PathBuf> {
    // Convert name to file path: "NITF_02.10_FileHeader" → "nitf/nitf_02.10_file_header.ksy"
    let file_name = name_to_filename(name);
    
    // Search in reverse order (later paths have higher priority)
    for path in paths.iter().rev() {
        let full_path = path.join(&file_name);
        if full_path.exists() {
            return Some(full_path);
        }
    }
    None
}

fn name_to_filename(name: &str) -> PathBuf {
    // "NITF_02.10_FileHeader" → "nitf/nitf_02.10_file_header.ksy"
    // "TRE_GEOLOB" → "tre/geolob.ksy"
    // "DES_TRE_OVERFLOW" → "des/tre_overflow.ksy"
    // ... implementation
}
```

## Correctness Properties


*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Definition Round-Trip

*For any* valid StructureDefinition, serializing it to KSY YAML format and then parsing it back SHALL produce an equivalent StructureDefinition.

**Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 1.6**

### Property 2: Binary Data Round-Trip

*For any* valid binary data that conforms to a structure definition, parsing it with StructureAccessor and then writing it with StructureWriter SHALL produce identical bytes.

**Validates: Requirements 17.1, 17.2**

### Property 3: Invalid YAML Error Handling

*For any* string that is not valid YAML syntax, the DefinitionLoader SHALL return a YamlError.

**Validates: Requirements 1.7**

### Property 4: Undefined Type Reference Error

*For any* KSY definition that references a type name not defined in the `types` section, the DefinitionLoader SHALL return an UndefinedType error identifying the missing type.

**Validates: Requirements 1.8**

### Property 5: BCS-A Character Validation

*For any* byte sequence, the BCS-A validator SHALL return true if and only if all bytes are in the range 0x20-0x7E (ASCII printable characters).

**Validates: Requirements 2.3**

### Property 6: BCS-N Character Validation

*For any* byte sequence, the BCS-N validator SHALL return true if and only if all bytes are digits (0x30-0x39) or space (0x20).

**Validates: Requirements 2.4**

### Property 7: Conditional Field Presence

*For any* structure with a conditional field, the field SHALL be accessible via `get()` and `has()` if and only if its condition expression evaluates to true.

**Validates: Requirements 3.2, 3.3, 3.4, 3.5**

### Property 8: Expression-Based Repetition Count

*For any* repeated field with `repeat: expr`, the number of accessible indexed elements (`field_0`, `field_1`, ...) SHALL equal the evaluated repeat-expr value.

**Validates: Requirements 4.1, 4.6**

### Property 9: Until-Condition Repetition

*For any* repeated field with `repeat: until`, parsing SHALL stop when the until-condition evaluates to true, and the last element SHALL be the one that satisfied the condition.

**Validates: Requirements 4.2**

### Property 10: End-of-Stream Repetition

*For any* repeated field with `repeat: eos`, the total bytes consumed by all elements SHALL equal the remaining buffer size.

**Validates: Requirements 4.3**

### Property 11: Underscore-Indexed Naming

*For any* repeated field with N elements, paths `field_0` through `field_{N-1}` SHALL be accessible, and `field_N` SHALL return UnknownField error.

**Validates: Requirements 4.4, 4.5**

### Property 12: Unknown Field Path Error

*For any* path that does not correspond to a field in the structure definition, `get()` SHALL return an UnknownField error.

**Validates: Requirements 5.3**

### Property 13: Field Existence Correctness

*For any* field path, `has(path)` SHALL return true if and only if `get(path)` returns a successful Value.

**Validates: Requirements 5.4**

### Property 14: Field Enumeration Completeness

*For any* structure, the set of paths returned by `fields()` SHALL equal the set of all paths for which `has(path)` returns true.

**Validates: Requirements 5.5**

### Property 15: String Padding Trimming

*For any* string field with padding, `as_str()` SHALL return the string with trailing padding characters removed.

**Validates: Requirements 6.1**

### Property 16: Numeric String Parsing

*For any* BCS-N string representing a valid integer, `as_i64()` and `as_u64()` SHALL return the numeric value. *For any* numeric string representing a valid float, `as_f64()` SHALL return the floating-point value.

**Validates: Requirements 6.2, 6.3, 6.4**

### Property 17: Invalid Numeric Conversion Error

*For any* string that cannot be parsed as a number, `as_i64()`, `as_u64()`, and `as_f64()` SHALL return a ConversionError.

**Validates: Requirements 6.6**

### Property 18: Raw Slice Identity

*For any* field, the bytes returned by `raw_slice(path)` SHALL be identical to the bytes at the offset and length returned by `field_byte_range(path)`.

**Validates: Requirements 7.1, 7.2**

### Property 19: Fixed-Size Out-of-Order Writing

*For any* fixed-size structure, writing fields in any order SHALL produce the same output as writing them in definition order.

**Validates: Requirements 8.2**

### Property 20: Missing Required Field Error

*For any* structure with required fields, calling `finish()` without writing all required fields SHALL return a MissingRequired error.

**Validates: Requirements 8.4**

### Property 21: Value Too Large Error

*For any* field with a fixed size, writing a value larger than that size SHALL return a ValueTooLarge error.

**Validates: Requirements 8.5, 10.4, 10.5**

### Property 22: Padding Application

*For any* string field with padding, writing a string shorter than the field size SHALL result in the remaining bytes being filled with the padding character.

**Validates: Requirements 8.6**

### Property 23: Streaming Mode Order Enforcement

*For any* streaming writer, writing a field before all preceding fields have been written SHALL return an OutOfOrder error.

**Validates: Requirements 9.2, 9.3**

### Property 24: Write Character Set Validation

*For any* BCS-N field, writing a string containing non-numeric characters SHALL return a validation error. *For any* BCS-A field, writing a string containing characters outside 0x20-0x7E SHALL return a validation error.

**Validates: Requirements 10.2, 10.3**

### Property 25: Registry Search Path Priority

*For any* structure name with definitions in multiple search paths, `get()` SHALL return the definition from the highest-priority path.

**Validates: Requirements 11.2, 11.4, 11.5**

### Property 26: Registry List Completeness

*For any* registry, `list()` SHALL return all structure names that are resolvable via `get()`.

**Validates: Requirements 11.6**

### Property 27: Runtime Registration Priority

*For any* structure name, a runtime-registered definition SHALL take priority over file-based definitions.

**Validates: Requirements 11.8**

### Property 28: Expression Arithmetic Correctness

*For any* arithmetic expression using +, -, *, /, %, evaluation SHALL produce the mathematically correct result.

**Validates: Requirements 13.2**

### Property 29: Expression Comparison Correctness

*For any* comparison expression using ==, !=, <, >, <=, >=, evaluation SHALL produce the logically correct boolean result.

**Validates: Requirements 13.3**

### Property 30: Expression Logical Correctness

*For any* logical expression using and, or, not, evaluation SHALL follow standard boolean logic.

**Validates: Requirements 13.4**

### Property 31: Expression Type Coercion

*For any* value, `.to_i` SHALL return its integer representation, `.to_s` SHALL return its string representation, and `.length` SHALL return its length.

**Validates: Requirements 13.5**

### Property 32: Expression Syntax Error Handling

*For any* string that is not a valid expression, parsing SHALL return an ExpressionError.

**Validates: Requirements 13.8**

## Error Handling

### Error Types

```rust
/// Errors during structure definition loading
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("YAML parse error: {source}")]
    YamlError {
        #[from]
        source: serde_yaml::Error,
    },
    
    #[error("Missing required field '{field}' in {context}")]
    MissingField { field: String, context: String },
    
    #[error("Invalid type '{type_str}' in {context}")]
    InvalidType { type_str: String, context: String },
    
    #[error("Undefined type '{type_name}' referenced in {context}")]
    UndefinedType { type_name: String, context: String },
    
    #[error("Invalid expression '{expr}': {source}")]
    InvalidExpression {
        expr: String,
        #[source]
        source: ExpressionError,
    },
    
    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}

/// Errors during structure access (reading)
#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    #[error("Unknown field path: '{path}'")]
    UnknownField { path: String },
    
    #[error("Unexpected end of data at '{path}': expected {expected} bytes, got {available}")]
    UnexpectedEof {
        path: String,
        expected: usize,
        available: usize,
    },
    
    #[error("Conditional field '{path}' not present (condition: {condition})")]
    ConditionalNotPresent { path: String, condition: String },
    
    #[error("Encoding error at '{path}' ({encoding}): {message}")]
    EncodingError {
        path: String,
        encoding: String,
        message: String,
    },
    
    #[error("Expression evaluation failed: {source}")]
    ExpressionError {
        #[from]
        source: ExpressionError,
    },
    
    #[error("Field '{path}' is not contiguous in memory")]
    NonContiguous { path: String },
}

/// Errors during value conversion
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Cannot convert {from_type} to {to_type}")]
    TypeMismatch {
        from_type: &'static str,
        to_type: &'static str,
    },
    
    #[error("Failed to parse '{value}' as {target_type}: {message}")]
    ParseError {
        value: String,
        target_type: &'static str,
        message: String,
    },
}

/// Errors during structure writing
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Field '{path}' written out of order (expected after '{expected_after}')")]
    OutOfOrder { path: String, expected_after: String },
    
    #[error("Value too large for field '{path}': max {max_size} bytes, got {actual_size}")]
    ValueTooLarge {
        path: String,
        max_size: usize,
        actual_size: usize,
    },
    
    #[error("Required field '{path}' not written")]
    MissingRequired { path: String },
    
    #[error("Invalid value for field '{path}': {message}")]
    ValidationError { path: String, message: String },
    
    #[error("Conversion error: {source}")]
    ConversionError {
        #[from]
        source: ConversionError,
    },
}

/// Errors during expression evaluation
#[derive(Debug, thiserror::Error)]
pub enum ExpressionError {
    #[error("Syntax error in expression: {message}")]
    SyntaxError { message: String },
    
    #[error("Unknown field reference: '{field}'")]
    UnknownField { field: String },
    
    #[error("Type error: cannot apply {operator} to {operand_type}")]
    TypeError {
        operator: String,
        operand_type: String,
    },
    
    #[error("Division by zero")]
    DivisionByZero,
}
```

### Error Context

All errors include contextual information:
- Field path where the error occurred
- Expected vs actual values where applicable
- Expression text for expression-related errors
- Byte offsets for data-related errors

## Testing Strategy

### Unit Tests

Unit tests verify specific examples and edge cases:

1. **Definition Loading**
   - Parse minimal valid KSY file
   - Parse KSY with all field types
   - Parse KSY with nested types
   - Parse KSY with conditionals and repetitions
   - Verify error on invalid YAML
   - Verify error on undefined type reference

2. **Character Set Validation**
   - BCS-A boundary cases (0x1F invalid, 0x20 valid, 0x7E valid, 0x7F invalid)
   - BCS-N boundary cases (space, digits only)
   - ECS-A extended characters

3. **Value Conversions**
   - Numeric string parsing with leading zeros
   - Numeric string parsing with spaces (BCS-N padding)
   - Float parsing with various formats
   - Error cases for non-numeric strings

4. **Writer Validation**
   - Padding application for short strings
   - Error on oversized values
   - Error on invalid characters for BCS-A/BCS-N

### Property-Based Tests

Property-based tests verify universal properties using the `proptest` crate:

```rust
// Example property test structure
use proptest::prelude::*;

proptest! {
    /// Feature: data-driven-binary-parser, Property 2: Binary Data Round-Trip
    #[test]
    fn prop_binary_round_trip(data in valid_structure_data()) {
        let definition = create_test_definition();
        let accessor = StructureAccessor::new(definition.clone(), &data)?;
        let mut writer = StructureWriter::new_fixed(definition)?;
        
        // Copy all fields from accessor to writer
        for path in accessor.fields() {
            let value = accessor.get(&path)?;
            writer.set(&path, value)?;
        }
        
        let output = writer.finish()?;
        prop_assert_eq!(data, output);
    }
    
    /// Feature: data-driven-binary-parser, Property 5: BCS-A Character Validation
    #[test]
    fn prop_bcs_a_validation(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let expected = bytes.iter().all(|&b| b >= 0x20 && b <= 0x7E);
        let actual = validate_bcs_a(&bytes);
        prop_assert_eq!(expected, actual);
    }
    
    /// Feature: data-driven-binary-parser, Property 16: Numeric String Parsing
    #[test]
    fn prop_numeric_parsing(n in any::<i64>()) {
        let s = format!("{}", n);
        let value = Value::String(Cow::Borrowed(&s));
        let parsed = value.as_i64()?;
        prop_assert_eq!(n, parsed);
    }
}
```

### Test Configuration

- Property tests run with minimum 100 iterations
- Each property test is tagged with its design document property number
- Tests use synthetic binary data in `data/unit/`
- Integration tests with real NITF files use `data/integration/` (gitignored)

### Python Integration Tests

Python tests verify the PyO3 bindings work correctly:

```python
import pytest
from aws.osml.io import StructureRegistry, StructureAccessor, StructureWriter

def test_accessor_dict_access():
    """Verify dict-like access works in Python."""
    registry = StructureRegistry()
    # ... test implementation

def test_raw_view_zero_copy():
    """Verify raw_view returns memoryview sharing buffer."""
    # ... test implementation

def test_writer_round_trip():
    """Verify write then read produces same values."""
    # ... test implementation
```

### Benchmark Tests

Benchmark tests measure performance for large file access patterns:

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_lazy_access(c: &mut Criterion) {
    // Benchmark accessing specific fields without parsing entire structure
}

fn bench_full_parse(c: &mut Criterion) {
    // Benchmark parsing all fields
}

fn bench_write_fixed(c: &mut Criterion) {
    // Benchmark writing fixed-size structures
}

criterion_group!(benches, bench_lazy_access, bench_full_parse, bench_write_fixed);
criterion_main!(benches);
```
