# Parser Improvement Suggestions

This document captures opportunities to improve the parser code.

## Current State

The parser is well-structured after refactoring (accessor, writer, expression split into submodules). The custom expression evaluator handles Kaitai-specific syntax that third-party libraries don't support.

## Improvement Opportunities

### 1. Offset Caching

Field offsets are recalculated on each access. For repeated access patterns, caching would improve performance.

```rust
struct StructureAccessor<'a> {
    // Add offset cache
    offset_cache: RefCell<HashMap<String, usize>>,
}
```

Implementation approach:
1. Add `offset_cache: RefCell<HashMap<String, usize>>` to `StructureAccessor`
2. Check cache before calculating offset in `calculate_field_offset`
3. Store computed offsets in cache after calculation
4. Invalidate cache if underlying data changes (unlikely in read-only accessor)

Priority: Implement if profiling shows offset calculation as a bottleneck.

### 2. Bitwise Expression Support

Some TREs use existence masks requiring bitwise operations:

```yaml
# BANDSB has 32-bit existence mask
- id: existence_mask
  type: u4
# Fields present based on mask bits
- id: field_a
  if: (existence_mask & 0x80000000) != 0
```

Current workaround: Capture conditional sections as raw bytes (see `docs/STRUCTURES_LIMITATIONS.md`).

To support properly:
1. Add `&`, `|`, `^`, `<<`, `>>` tokens to `expression/lexer.rs`
2. Add bitwise precedence level to `expression/parser.rs` (between comparison and additive)
3. Add `eval_bitwise_*` functions to `expression/ops.rs`
4. Update `expression/eval.rs` to handle new operators

Priority: Implement when working on BANDSB, ILLUMB, BCHIPA, or IOMAPA TREs.
