# NITF J2K COMRAT Fix — Bugfix Design

## Overview

The NITF JPEG 2000 writer uses redundant `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` metadata parameters that duplicate information already encoded in the standard NITF `COMRAT` field. The `extract_encoding_hints()` function in `src/jbp/writer.rs` reads these `J2K_` fields as the primary source of truth, constructs `J2KEncodingHints` from them, and then `generate_comrat()` regenerates the COMRAT subheader value — silently discarding the user-supplied `COMRAT` string.

The fix removes `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from the codebase and documentation, making `COMRAT` the single source of truth. The writer will parse the user-supplied `COMRAT` string via `J2KComrat::parse()` to derive the `lossless` flag and `compression_ratio` for `J2KEncodingHints`. The `generate_comrat()` function becomes unnecessary for the primary write path since the user-supplied COMRAT string is written directly into the subheader.

## Glossary

- **Bug_Condition (C)**: The condition that triggers the bug — when a user sets `IC=C8/CD/M8/MD` with a `COMRAT` value, and the writer ignores COMRAT in favor of `J2K_LOSSLESS` / `J2K_COMPRESSION_RATIO` to derive encoding parameters and regenerate the subheader COMRAT
- **Property (P)**: The desired behavior — the writer SHALL parse the user-supplied `COMRAT` via `J2KComrat::parse()` to derive `lossless` and `compression_ratio` for `J2KEncodingHints`, and write the user-supplied COMRAT directly into the image subheader
- **Preservation**: Existing behavior that must remain unchanged — `J2K_DECOMPOSITION_LEVELS`, `J2K_QUALITY_LAYERS`, HTJ2K mode selection, uncompressed IC codes, non-J2K compression, and default-when-no-COMRAT behavior
- **`extract_encoding_hints()`**: Function in `src/jbp/writer.rs` (~line 606) that reads metadata fields from the asset's `BufferedMetadataProvider` and constructs an `EncodingHints` struct including `J2KEncodingHints`
- **`J2KComrat`**: Enum in `src/jbp/j2k/comrat.rs` representing parsed COMRAT values: `NumericallyLossless`, `VisuallyLossless(f32)`, `TargetBpp(f32)`, `Unknown`
- **`generate_comrat()`**: Function in `src/jbp/j2k/comrat.rs` (~line 340) that converts `J2KEncodingHints` back to a 4-character COMRAT string — the reverse direction of the intended data flow

## Bug Details

### Fault Condition

The bug manifests when a user sets a JPEG 2000 compression code (`IC=C8`, `CD`, `M8`, or `MD`) with a `COMRAT` value. The `extract_encoding_hints()` function ignores the user-supplied `COMRAT` for encoding configuration, instead reading `J2K_LOSSLESS` (defaulting to `false`) and `J2K_COMPRESSION_RATIO` (defaulting to `10.0`) to populate `J2KEncodingHints`. The subheader generation code then calls `generate_comrat(j2k_hints)` to produce a COMRAT string from these hints, overwriting whatever the user provided.

**Formal Specification:**
```
FUNCTION isBugCondition(input)
  INPUT: input of type EncodingHintInputs (metadata dict from BufferedMetadataProvider)
  OUTPUT: boolean

  LET ic = input.get("IC")
  LET comrat = input.get("COMRAT")

  RETURN ic IN ["C8", "CD", "M8", "MD"]
         AND comrat IS NOT NULL
         AND (input.get("J2K_LOSSLESS") IS NOT NULL
              OR input.get("J2K_COMPRESSION_RATIO") IS NOT NULL
              OR comrat-derived-encoding != j2k-field-derived-encoding)
END FUNCTION
```

The core issue: even when `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` are not explicitly set by the user, they take effect via defaults (`false` and `10.0`), meaning any user who sets `COMRAT=N001.0` without also setting `J2K_LOSSLESS=true` gets lossy encoding at 10:1 ratio.

### Examples

- User sets `COMRAT=N001.0` (numerically lossless) without `J2K_LOSSLESS=true` → encoder uses lossy mode at 10:1 ratio, subheader COMRAT becomes `"00.8"` instead of `"N001.0"`
- User sets `COMRAT=00.5` (0.5 bpp lossy) without `J2K_COMPRESSION_RATIO` → encoder uses default 10:1 ratio (0.8 bpp), subheader COMRAT becomes `"00.8"` instead of `"00.5"`
- User sets `COMRAT=N001.0` with `J2K_LOSSLESS=false` and `J2K_COMPRESSION_RATIO=10.0` → contradictory values silently resolved in favor of `J2K_` fields, subheader COMRAT becomes `"00.8"`
- User sets `COMRAT=V020.0` (visually lossless) → `J2K_LOSSLESS` defaults to `false`, encoder treats this as lossy at 10:1, ignoring the visually lossless intent entirely

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- `J2K_DECOMPOSITION_LEVELS` metadata field must continue to control wavelet decomposition levels (default 5)
- `J2K_QUALITY_LAYERS` metadata field must continue to control quality layers (default 1)
- HTJ2K mode must continue to be determined by `IC=CD` or `IC=MD`
- `IC=NC` / `IC=NM` (uncompressed) must continue to skip J2K encoding hint extraction entirely
- When no `COMRAT` is provided for J2K images, the system must default to numerically lossless (`N001.0` equivalent)
- The COMRAT field in the image subheader must continue to be a properly formatted 4-character value
- Non-J2K compression (e.g., `IC=C3` for JPEG DCT) must continue to handle COMRAT as a quality factor without J2K-specific parsing
- Mouse/programmatic interaction with `BufferedMetadataProvider` for non-J2K fields must be unaffected

**Scope:**
All inputs that do NOT involve JPEG 2000 compression codes (`IC` not in `{C8, CD, M8, MD}`) should be completely unaffected by this fix. For J2K inputs, only the source of `lossless` and `compression_ratio` changes — from `J2K_LOSSLESS`/`J2K_COMPRESSION_RATIO` metadata fields to `COMRAT` parsing.

## Hypothesized Root Cause

Based on the bug description and code analysis, the root cause is a design-implementation mismatch:

1. **Inverted Source of Truth**: The design spec (`.kiro/specs/jpeg2000-compression/design.md`) states "The writer parses COMRAT to determine lossless mode and compression ratio, so separate `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` hints are not needed." However, the implementation does the opposite — it reads `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from metadata, constructs `J2KEncodingHints`, and then regenerates COMRAT from those hints via `generate_comrat()`.

2. **Default Value Trap**: `J2K_LOSSLESS` defaults to `false` and `J2K_COMPRESSION_RATIO` defaults to `10.0` when not present in metadata. This means even users who correctly set `COMRAT=N001.0` get lossy encoding unless they also redundantly set `J2K_LOSSLESS=true`.

3. **Subheader Preference for Generated COMRAT**: In the subheader generation code (~line 1524), the `if let Some(ref j2k_hints)` branch takes priority over the `if let Some(ref comrat_str)` branch. Since `j2k_hints` is always `Some` for J2K IC codes, the user-supplied COMRAT string is never used.

4. **Documentation Amplification**: The user guide documents both `COMRAT` and the `J2K_` fields as valid encoder parameters, creating confusion about which actually controls behavior and encouraging users to set contradictory values.

## Correctness Properties

Property 1: Fault Condition — COMRAT-Derived Encoding Parameters

_For any_ input where `IC` is a JPEG 2000 code (`C8`, `CD`, `M8`, `MD`) and `COMRAT` is provided, the fixed `extract_encoding_hints()` function SHALL parse the `COMRAT` string via `J2KComrat::parse()` and derive `J2KEncodingHints.lossless` and `J2KEncodingHints.compression_ratio` from the parsed result, ignoring any `J2K_LOSSLESS` or `J2K_COMPRESSION_RATIO` metadata fields.

**Validates: Requirements 2.1, 2.2, 2.3, 2.4**

Property 2: Preservation — Non-COMRAT Encoding Parameters Unchanged

_For any_ input where `IC` is a JPEG 2000 code, the fixed `extract_encoding_hints()` function SHALL produce the same `J2KEncodingHints.decomposition_levels`, `J2KEncodingHints.quality_layers`, and `J2KEncodingHints.htj2k` values as the original function, preserving all encoding parameters not derived from COMRAT.

**Validates: Requirements 3.1, 3.2, 3.4**

Property 3: Preservation — Non-J2K Compression Unaffected

_For any_ input where `IC` is NOT a JPEG 2000 code (e.g., `NC`, `NM`, `C3`, `M3`), the fixed `extract_encoding_hints()` function SHALL produce exactly the same `EncodingHints` as the original function.

**Validates: Requirements 3.5, 3.6, 3.7**

## Fix Implementation

### Changes Required

Assuming our root cause analysis is correct:

**File**: `src/jbp/writer.rs`

**Function**: `extract_encoding_hints()`

**Specific Changes**:
1. **Remove J2K_LOSSLESS read**: Delete the block (~lines 668-681) that reads `J2K_LOSSLESS` from the metadata dict and parses it as a boolean
2. **Remove J2K_COMPRESSION_RATIO read**: Delete the block (~lines 683-693) that reads `J2K_COMPRESSION_RATIO` from the metadata dict and parses it as f64
3. **Add COMRAT parsing**: After extracting the `comrat` string, parse it via `J2KComrat::parse()` to derive `lossless` and `compression_ratio`:
   - `NumericallyLossless` → `lossless = true`, `compression_ratio = None`
   - `VisuallyLossless(bpp)` → `lossless = false`, `compression_ratio = Some(8.0 / bpp as f64)` (approximate)
   - `TargetBpp(bpp)` → `lossless = false`, `compression_ratio = Some(8.0 / bpp as f64)`
   - `Unknown` or no COMRAT → `lossless = true`, `compression_ratio = None` (default to lossless)
4. **Keep J2K_DECOMPOSITION_LEVELS and J2K_QUALITY_LAYERS**: These reads remain unchanged
5. **Keep HTJ2K detection**: `htj2k` continues to be derived from `IC=CD` or `IC=MD`

**File**: `src/jbp/writer.rs`

**Location**: Subheader generation (~line 1524)

**Specific Changes**:
6. **Use user-supplied COMRAT directly**: Instead of `generate_comrat(j2k_hints)`, use the user-supplied `hints.comrat` string. If no COMRAT was provided, fall back to generating one from the `J2KEncodingHints` (which now defaults to lossless). The priority becomes: user-supplied COMRAT → generated default.

**File**: `src/jbp/j2k/comrat.rs`

**Function**: `generate_comrat()`

**Specific Changes**:
7. **Retain but demote**: `generate_comrat()` is still useful as a fallback for generating a default COMRAT when the user doesn't provide one. It does not need to be removed, but it should no longer be the primary path. Consider adding a `/// Used as fallback when no COMRAT is provided by the user.` doc comment.

**File**: `docs/user-guide/image-assets-writing.md`

**Specific Changes**:
8. **Remove J2K_LOSSLESS from encoder parameters table**: Delete the row for `J2K_LOSSLESS`
9. **Remove J2K_COMPRESSION_RATIO from encoder parameters table**: Delete the row for `J2K_COMPRESSION_RATIO`
10. **Update code examples**: Remove `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from all Python code examples, showing only `COMRAT` for compression control
11. **Update explanatory text**: Remove the paragraph about `J2K_LOSSLESS=true` ignoring compression ratio

**File**: `.kiro/specs/jpeg2000-compression/design.md`

**Specific Changes**:
12. **Codify the metadata derivation principle**: Add a section stating that synthetic `J2K_` prefixed metadata fields should only exist when no standard NITF field carries the same information. `COMRAT` encodes lossless mode and compression ratio, so `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` violate this rule. `J2K_DECOMPOSITION_LEVELS` and `J2K_QUALITY_LAYERS` are acceptable because no standard NITF field carries that information.

## Testing Strategy

### Validation Approach

The testing strategy follows a two-phase approach: first, surface counterexamples that demonstrate the bug on unfixed code, then verify the fix works correctly and preserves existing behavior.

### Exploratory Fault Condition Checking

**Goal**: Surface counterexamples that demonstrate the bug BEFORE implementing the fix. Confirm or refute the root cause analysis. If we refute, we will need to re-hypothesize.

**Test Plan**: Write Rust unit tests that call `extract_encoding_hints()` with various metadata configurations and assert the resulting `J2KEncodingHints` fields. Run these tests on the UNFIXED code to observe failures and confirm the root cause.

**Test Cases**:
1. **Lossless COMRAT Ignored Test**: Set `IC=C8`, `COMRAT=N001.0`, no `J2K_LOSSLESS` → assert `j2k_hints.lossless == true` (will fail on unfixed code because `J2K_LOSSLESS` defaults to `false`)
2. **Lossy BPP COMRAT Ignored Test**: Set `IC=C8`, `COMRAT=00.5` (0.5 bpp), no `J2K_COMPRESSION_RATIO` → assert `j2k_hints.compression_ratio` corresponds to 0.5 bpp (will fail on unfixed code because ratio defaults to `10.0`)
3. **Contradictory Values Test**: Set `IC=C8`, `COMRAT=N001.0`, `J2K_LOSSLESS=false`, `J2K_COMPRESSION_RATIO=10.0` → assert `j2k_hints.lossless == true` (will fail on unfixed code)
4. **Subheader COMRAT Overwrite Test**: Set `IC=C8`, `COMRAT=00.5`, verify the subheader contains `"00.5"` not `"00.8"` (will fail on unfixed code because `generate_comrat()` overwrites it)

**Expected Counterexamples**:
- `extract_encoding_hints()` returns `lossless=false` when `COMRAT=N001.0` is set without `J2K_LOSSLESS=true`
- Subheader COMRAT value differs from user-supplied COMRAT string
- Possible causes confirmed: inverted source of truth, default value trap, subheader preference for generated COMRAT

### Fix Checking

**Goal**: Verify that for all inputs where the bug condition holds, the fixed function produces the expected behavior.

**Pseudocode:**
```
FOR ALL input WHERE isBugCondition(input) DO
  result := extract_encoding_hints_fixed(input)
  ASSERT result.j2k_hints.lossless == J2KComrat::parse(input.comrat).is_lossless()
  ASSERT result.j2k_hints.compression_ratio == derive_ratio_from_comrat(input.comrat)
END FOR
```

### Preservation Checking

**Goal**: Verify that for all inputs where the bug condition does NOT hold, the fixed function produces the same result as the original function.

**Pseudocode:**
```
FOR ALL input WHERE NOT isBugCondition(input) DO
  ASSERT extract_encoding_hints_original(input) = extract_encoding_hints_fixed(input)
END FOR
```

**Testing Approach**: Property-based testing is recommended for preservation checking because:
- It generates many metadata configurations automatically across the input domain
- It catches edge cases in COMRAT parsing that manual unit tests might miss
- It provides strong guarantees that non-J2K behavior is unchanged

**Test Plan**: Observe behavior on UNFIXED code first for non-J2K IC codes and for J2K codes with `J2K_DECOMPOSITION_LEVELS` / `J2K_QUALITY_LAYERS`, then write property-based tests capturing that behavior.

**Test Cases**:
1. **Non-J2K IC Preservation**: Observe that `IC=NC`, `IC=NM`, `IC=C3` produce identical `EncodingHints` on unfixed code, then verify this continues after fix
2. **Decomposition Levels Preservation**: Observe that `J2K_DECOMPOSITION_LEVELS` values are passed through unchanged on unfixed code, then verify this continues after fix
3. **Quality Layers Preservation**: Observe that `J2K_QUALITY_LAYERS` values are passed through unchanged on unfixed code, then verify this continues after fix
4. **HTJ2K Mode Preservation**: Observe that `IC=CD` sets `htj2k=true` on unfixed code, then verify this continues after fix

### Unit Tests

- Test `extract_encoding_hints()` with `COMRAT=N001.0` → `lossless=true`, `compression_ratio=None`
- Test `extract_encoding_hints()` with `COMRAT=00.5` → `lossless=false`, `compression_ratio` derived from 0.5 bpp
- Test `extract_encoding_hints()` with `COMRAT=V020.0` → `lossless=false`, `compression_ratio` derived from visually lossless quality
- Test `extract_encoding_hints()` with no COMRAT → defaults to lossless
- Test `extract_encoding_hints()` with `COMRAT=----` (unknown) → defaults to lossless
- Test subheader generation writes user-supplied COMRAT string directly
- Test that `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` in metadata are ignored (no effect on output)

### Property-Based Tests

- Generate random valid COMRAT strings (`N` + 3 digits, `V` + 3 digits, 4-digit bpp) and verify `extract_encoding_hints()` produces `J2KEncodingHints` consistent with `J2KComrat::parse()` output
- Generate random non-J2K IC codes and arbitrary metadata dicts, verify `EncodingHints` output is identical between original and fixed functions
- Generate random `J2K_DECOMPOSITION_LEVELS` (1–32) and `J2K_QUALITY_LAYERS` (1–255) values alongside various COMRAT strings, verify these fields pass through unchanged

### Integration Tests

- Write a NITF file with `IC=C8`, `COMRAT=N001.0`, read it back, verify the image subheader contains `COMRAT=N001.0` and the image data is losslessly compressed
- Write a NITF file with `IC=C8`, `COMRAT=00.5`, read it back, verify the subheader contains `COMRAT=00.5`
- Write a NITF file with `IC=CD`, `COMRAT=N001.0`, verify HTJ2K lossless encoding and correct subheader
- Roundtrip test: read an existing J2K NITF, copy metadata, write new file, verify COMRAT is preserved exactly
