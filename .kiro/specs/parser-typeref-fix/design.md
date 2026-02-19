# Design Document: Parser TypeRef Fix

## Overview

This design addresses a critical bug in the structure parser where fields after repeated nested type arrays are inaccessible. The root cause is that `get_simple_field_size()` returns `0` for `TypeRef` fields instead of calculating the actual nested type size.

The fix requires modifying the context building logic to properly calculate sizes for nested types, which will enable correct offset calculations for all subsequent fields.

### Key Design Decisions

1. **Extend get_simple_field_size()**: Add TypeRef handling that calculates nested type size by summing field sizes
2. **Pass Definition Reference**: The function needs access to the structure definition's `types` map to resolve TypeRef
3. **Handle Variable-Length Nested Types**: For nested types with conditional or expression-sized fields, calculate actual sizes
4. **Maintain Backward Compatibility**: Ensure all existing tests pass without API changes

## Architecture

The fix is localized to `src/parser/accessor/context.rs` with minimal changes to the function signatures:

```
┌─────────────────────────────────────────────────────────────────┐
│                    StructureAccessor                             │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ build_eval_context_up_to()                                  ││
│  │   └── build_context_from_definition()                       ││
│  │         ├── get_simple_field_size()      ◄── FIX HERE       ││
│  │         │     └── get_nested_type_size() ◄── NEW FUNCTION   ││
│  │         └── get_simple_total_field_size()                   ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ calculate_field_offset()                                    ││
│  │   └── get_total_field_size()                                ││
│  │         └── get_type_size()              ◄── ALREADY WORKS  ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

Note: `get_type_size()` in `mod.rs` already handles TypeRef correctly, but it's not used during context building. The fix aligns `get_simple_field_size()` with this behavior.

## Components and Interfaces

### Modified Function: get_simple_field_size

The function signature changes to accept the structure definition for type resolution:

```rust
/// Get field size using an existing context (avoids recursion).
/// 
/// # Arguments
/// * `field` - The field definition
/// * `ctx` - The evaluation context with previously parsed field values
/// * `evaluator` - Expression evaluator for size expressions
/// * `definition` - The parent structure definition (for TypeRef resolution)
/// * `data` - The raw data buffer (for variable-length nested types)
/// * `base_offset` - The offset where this field starts in the data
pub fn get_simple_field_size(
    field: &FieldDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
    definition: &StructureDefinition,
    data: &[u8],
    base_offset: usize,
) -> Result<usize, AccessError>
```

### New Function: get_nested_type_size

A helper function to calculate the size of a nested type:

```rust
/// Calculate the size of a nested type instance.
/// 
/// # Arguments
/// * `type_name` - Name of the nested type to resolve
/// * `definition` - The parent structure definition containing the types map
/// * `ctx` - The evaluation context
/// * `evaluator` - Expression evaluator
/// * `data` - The raw data buffer
/// * `offset` - The offset where this nested type instance starts
/// 
/// # Returns
/// The total size in bytes of the nested type instance
fn get_nested_type_size(
    type_name: &str,
    definition: &StructureDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
    data: &[u8],
    offset: usize,
) -> Result<usize, AccessError>
```

### Modified Function: get_simple_total_field_size

Updated to pass through the new parameters:

```rust
pub fn get_simple_total_field_size(
    field: &FieldDefinition,
    ctx: &EvalContext,
    evaluator: &ExpressionEvaluator,
    definition: &StructureDefinition,
    data: &[u8],
    base_offset: usize,
) -> Result<usize, AccessError>
```

### Modified Function: build_context_from_definition

Updated to pass the definition and data to size calculation functions:

```rust
pub fn build_context_from_definition<'a, F>(
    definition: &StructureDefinition,
    data: &'a [u8],
    evaluator: &ExpressionEvaluator,
    stop_at: &str,
    read_field: F,
) -> Result<EvalContext, AccessError>
```

## Implementation Details

### TypeRef Size Calculation

When `get_simple_field_size()` encounters a `TypeRef`:

1. Look up the type name in `definition.types`
2. If not found, return `AccessError::UnknownField`
3. Call `get_nested_type_size()` to calculate the size
4. Return the calculated size

### Nested Type Size Calculation

`get_nested_type_size()` iterates through the nested type's fields:

1. For each field, check if it's conditional
2. If conditional, evaluate the condition using the context
3. If condition is false, skip the field (size = 0)
4. Otherwise, calculate the field's size recursively
5. Sum all field sizes to get the total

### Handling Variable-Length Nested Types

For nested types like `band_info_type` that contain conditional fields (e.g., `nelut`, `lut_data`):

1. Parse the nested type's fields sequentially
2. For each field, read its value to update the context
3. Use the updated context to evaluate conditions and size expressions
4. This requires reading actual data, not just using the context

### Repeated TypeRef Fields

For repeated TypeRef fields like `band_info`:

1. Get the repeat count from the expression (e.g., `nbands.to_i`)
2. For each repetition, calculate the element size
3. If elements have variable sizes, calculate each one individually
4. Sum all element sizes for the total field size

## Data Flow

```
Image Subheader Parsing:
                                                                    
1. Parse fixed fields (im, iid1, ..., nbands)                      
   └── Offsets calculated correctly                                
                                                                    
2. Parse band_info (repeated TypeRef)                              
   ├── OLD: get_simple_field_size returns 0                        
   │   └── Total size = 0 × repeat_count = 0                       
   │       └── Subsequent field offsets WRONG                      
   │                                                                
   └── NEW: get_simple_field_size calls get_nested_type_size       
       └── Calculates actual band_info_type size                   
           └── Total size = element_size × repeat_count            
               └── Subsequent field offsets CORRECT                
                                                                    
3. Parse post-band fields (isync, imode, ..., udidl, udid, ...)   
   └── NOW ACCESSIBLE with correct offsets                         
```

## Correctness Properties

### Property 1: TypeRef Size Accuracy

*For any* field with `FieldType::TypeRef(type_name)` where `type_name` exists in the definition's types map, `get_simple_field_size()` SHALL return the same size as `get_type_size()` for the same field.

**Validates: Requirements 1.1, 1.2, 1.3**

### Property 2: Repeated TypeRef Total Size

*For any* repeated TypeRef field with a fixed repeat count, `get_simple_total_field_size()` SHALL return `element_size × repeat_count`.

**Validates: Requirements 2.1, 2.2**

### Property 3: Field Iterator Completeness

*For any* structure definition, `fields()` SHALL yield all non-conditional fields and all conditional fields whose conditions evaluate to true, regardless of whether they come before or after TypeRef arrays.

**Validates: Requirements 3.1, 3.2, 3.3**

### Property 4: Image Subheader Field Access

*For any* valid NITF image subheader, the accessor SHALL be able to access `udidl` and `ixshdl` fields, and when their values are > 0, the corresponding `udid` and `ixshd` fields.

**Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5**

### Property 5: Backward Compatibility

*For any* structure definition without TypeRef fields, parsing behavior SHALL be identical before and after the fix.

**Validates: Requirements 6.1, 6.2, 6.3, 6.4**

## Error Handling

| Error | Condition | Context |
|-------|-----------|---------|
| `UnknownField` | TypeRef references non-existent type | Type name |
| `ExpressionError` | Size expression evaluation fails | Field path, expression |
| `UnexpectedEof` | Nested type extends beyond data | Field path, expected, available |

## Testing Strategy

### Unit Tests

1. **TypeRef size calculation**: Test `get_simple_field_size()` with TypeRef fields
2. **Nested type with conditionals**: Test size calculation for types like `band_info_type`
3. **Repeated TypeRef**: Test total size calculation for repeated nested types
4. **Field iterator**: Test that all fields are yielded after TypeRef arrays

### Integration Tests

1. **Image subheader parsing**: Parse real NITF image subheaders and verify TRE field access
2. **JITC test files**: Use JITC files with TREs to verify end-to-end TRE extraction

### Property-Based Tests

1. **Size consistency**: Verify `get_simple_field_size()` matches `get_type_size()` for TypeRef
2. **Iterator completeness**: Verify all defined fields are yielded by the iterator

## Migration Notes

The fix is internal to the parser and does not change any public APIs. Existing code using `StructureAccessor` will automatically benefit from the fix without modifications.

