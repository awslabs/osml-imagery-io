# Implementation Plan

- [x] 1. Write bug condition exploration test
  - **Property 1: Fault Condition** — COMRAT Ignored in extract_encoding_hints
  - **CRITICAL**: This test MUST FAIL on unfixed code — failure confirms the bug exists
  - **DO NOT attempt to fix the test or the code when it fails**
  - **NOTE**: This test encodes the expected behavior — it will validate the fix when it passes after implementation
  - **GOAL**: Surface counterexamples that demonstrate `extract_encoding_hints()` ignores user-supplied COMRAT
  - **Scoped PBT Approach**: Use `proptest` in Rust. Scope the property to concrete failing cases:
    - `IC=C8` with `COMRAT=N001.0` (no `J2K_LOSSLESS` set) → assert `j2k_hints.lossless == true`
    - `IC=C8` with `COMRAT=00.5` (no `J2K_COMPRESSION_RATIO` set) → assert compression_ratio derives from 0.5 bpp
    - `IC=C8` with `COMRAT=N001.0`, `J2K_LOSSLESS=false`, `J2K_COMPRESSION_RATIO=10.0` → assert `j2k_hints.lossless == true` (COMRAT wins)
  - Write as a `proptest!` block in `src/jbp/writer.rs` under a new `#[cfg(test)] mod bugfix_tests`
  - Build a `BufferedMetadataProvider` with the metadata dict, construct a `QueuedAsset`, call `extract_encoding_hints()`
  - Assert that `J2KEncodingHints.lossless` and `J2KEncodingHints.compression_ratio` match `J2KComrat::parse(comrat)` output
  - Run test on UNFIXED code
  - **EXPECTED OUTCOME**: Test FAILS (this is correct — it proves the bug exists: `lossless` defaults to `false` via `J2K_LOSSLESS`, compression_ratio defaults to `10.0`)
  - Document counterexamples found (e.g., "COMRAT=N001.0 produces lossless=false, compression_ratio=Some(10.0)")
  - Mark task complete when test is written, run, and failure is documented
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3_

- [x] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** — Non-COMRAT Encoding Parameters and Non-J2K IC Codes Unchanged
  - **IMPORTANT**: Follow observation-first methodology
  - **Observe on UNFIXED code**:
    - `IC=NC` / `IC=NM` → `j2k_hints` is `None` (no J2K encoding)
    - `IC=C3` / `IC=M3` → `j2k_hints` is `None`, `comrat` passed through as-is
    - `IC=C8` with `J2K_DECOMPOSITION_LEVELS=3` → `j2k_hints.decomposition_levels == 3`
    - `IC=C8` with `J2K_QUALITY_LAYERS=4` → `j2k_hints.quality_layers == 4`
    - `IC=CD` → `j2k_hints.htj2k == true`; `IC=C8` → `j2k_hints.htj2k == false`
  - Write `proptest!` block(s) in `src/jbp/writer.rs` under the same `bugfix_tests` module:
    - Property 2a: For any non-J2K IC code (`NC`, `NM`, `C3`, `M3`), `j2k_hints` is `None` and other `EncodingHints` fields are unaffected
    - Property 2b: For any J2K IC code, `decomposition_levels` equals the `J2K_DECOMPOSITION_LEVELS` metadata value (or default 5), `quality_layers` equals `J2K_QUALITY_LAYERS` (or default 1), and `htj2k` matches `IC=CD`/`IC=MD`
  - Verify tests PASS on UNFIXED code
  - **EXPECTED OUTCOME**: Tests PASS (this confirms baseline behavior to preserve)
  - Mark task complete when tests are written, run, and passing on unfixed code
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [x] 3. Fix COMRAT as single source of truth for J2K encoding parameters

  - [x] 3.1 Modify `extract_encoding_hints()` in `src/jbp/writer.rs`
    - Remove the `J2K_LOSSLESS` read block (~lines 668–681) that parses the boolean from metadata
    - Remove the `J2K_COMPRESSION_RATIO` read block (~lines 683–693) that parses the f64 from metadata
    - After extracting the `comrat` string, parse it via `J2KComrat::parse()` to derive `lossless` and `compression_ratio`:
      - `NumericallyLossless` → `lossless = true`, `compression_ratio = None`
      - `VisuallyLossless(bpp)` → `lossless = false`, `compression_ratio = Some(8.0 / bpp as f64)`
      - `TargetBpp(bpp)` → `lossless = false`, `compression_ratio = Some(8.0 / bpp as f64)`
      - `Unknown` or no COMRAT → `lossless = true`, `compression_ratio = None` (default to lossless)
    - Keep `J2K_DECOMPOSITION_LEVELS` and `J2K_QUALITY_LAYERS` reads unchanged
    - Keep HTJ2K detection (`IC=CD` / `IC=MD`) unchanged
    - _Bug_Condition: isBugCondition(input) where IC in {C8, CD, M8, MD} AND COMRAT is provided AND J2K_LOSSLESS/J2K_COMPRESSION_RATIO override COMRAT_
    - _Expected_Behavior: extract_encoding_hints() SHALL parse COMRAT via J2KComrat::parse() and derive lossless/compression_ratio from it_
    - _Preservation: J2K_DECOMPOSITION_LEVELS, J2K_QUALITY_LAYERS, htj2k, non-J2K IC codes unchanged_
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 3.2 Modify subheader generation in `src/jbp/writer.rs` (~line 1524)
    - Use user-supplied `hints.comrat` string directly in the image subheader instead of calling `generate_comrat(j2k_hints)`
    - Fall back to `generate_comrat(j2k_hints)` only when no COMRAT was provided by the user
    - Priority: user-supplied COMRAT → generated default
    - _Bug_Condition: subheader overwrites user COMRAT with generate_comrat() output_
    - _Expected_Behavior: subheader SHALL contain the user-supplied COMRAT string verbatim_
    - _Preservation: when no COMRAT provided, generate_comrat() still produces a valid default_
    - _Requirements: 2.1, 3.6_

  - [x] 3.3 Add fallback doc comment to `generate_comrat()` in `src/jbp/j2k/comrat.rs`
    - Add `/// Used as fallback when no COMRAT is provided by the user.` doc comment
    - No functional changes to `generate_comrat()` itself
    - _Requirements: 2.4_

  - [x] 3.4 Update documentation in `docs/user-guide/image-assets-writing.md`
    - Remove `J2K_LOSSLESS` row from the encoder parameters table
    - Remove `J2K_COMPRESSION_RATIO` row from the encoder parameters table
    - Remove `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from all Python code examples
    - Remove the paragraph about `J2K_LOSSLESS=true` ignoring compression ratio
    - Ensure only `COMRAT`, `J2K_DECOMPOSITION_LEVELS`, and `J2K_QUALITY_LAYERS` remain as J2K encoder parameters
    - _Requirements: 2.5_

  - [x] 3.5 Codify metadata derivation principle in `.kiro/specs/jpeg2000-compression/design.md`
    - Add a section stating that synthetic `J2K_` prefixed metadata fields should only exist when no standard NITF field carries the same information
    - State that `COMRAT` encodes lossless mode and compression ratio, so `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` violate this rule
    - State that `J2K_DECOMPOSITION_LEVELS` and `J2K_QUALITY_LAYERS` are acceptable because no standard NITF field carries that information
    - _Requirements: 2.4, 2.5_

  - [x] 3.6 Verify bug condition exploration test now passes
    - **Property 1: Expected Behavior** — COMRAT-Derived Encoding Parameters
    - **IMPORTANT**: Re-run the SAME test from task 1 — do NOT write a new test
    - The test from task 1 encodes the expected behavior (COMRAT parsed via `J2KComrat::parse()`)
    - When this test passes, it confirms `extract_encoding_hints()` now derives lossless/compression_ratio from COMRAT
    - Run bug condition exploration test from step 1
    - **EXPECTED OUTCOME**: Test PASSES (confirms bug is fixed)
    - _Requirements: 2.1, 2.2, 2.3_

  - [x] 3.7 Verify preservation tests still pass
    - **Property 2: Preservation** — Non-COMRAT Encoding Parameters and Non-J2K IC Codes Unchanged
    - **IMPORTANT**: Re-run the SAME tests from task 2 — do NOT write new tests
    - Run preservation property tests from step 2
    - **EXPECTED OUTCOME**: Tests PASS (confirms no regressions)
    - Confirm decomposition_levels, quality_layers, htj2k, and non-J2K IC codes are unaffected
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [x] 4. Checkpoint — Ensure all tests pass
  - Run `cargo test` to verify all Rust tests pass (including bugfix property tests and existing tests)
  - Run `pytest` to verify all Python tests pass
  - Ensure no regressions in existing property tests
  - Ask the user if questions arise
