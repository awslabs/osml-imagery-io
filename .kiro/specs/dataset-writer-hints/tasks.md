# Implementation Plan: Dataset Writer Encoding Hints

## Overview

This implementation plan covers adding encoding hints support to the dataset writer system. The feature allows users to pass format-specific encoding options (imode, ic, nppbh, nppbv, comrat) through the MetadataProvider interface rather than format-specific parameters in abstract interfaces.

Field names use lowercase to match the .ksy parser output (e.g., "imode" not "IMODE"), ensuring consistency between reader and writer APIs.

The implementation is in Rust with PyO3 Python bindings, following the existing codebase patterns.

## Tasks

- [x] 1. Implement SimpleMetadataProvider in Rust
  - [x] 1.1 Create SimpleMetadataProvider struct with thread-safe HashMap storage
    - Add `src/simple_metadata.rs` with RwLock<HashMap<String, serde_json::Value>>
    - Implement `new()` constructor
    - Implement `set(key, value)` and `get(key)` methods
    - Implement `remove(key)` and `clear()` methods
    - _Requirements: 1.1, 1.2, 1.3_
  
  - [x] 1.2 Implement MetadataProvider trait for SimpleMetadataProvider
    - Implement `raw()` returning empty bytes (no raw representation)
    - Implement `as_dict(None)` returning all stored pairs
    - Implement `as_dict(Some(prefix))` with prefix filtering
    - _Requirements: 1.4, 1.5_
  
  - [x] 1.3 Implement from_provider constructor
    - Add `from_provider(source: &dyn MetadataProvider)` method
    - Copy all key-value pairs from source.as_dict(None)
    - _Requirements: 1.8_
  
  - [ ]* 1.4 Write property tests for SimpleMetadataProvider
    - **Property 1: Set/Get Round-Trip**
    - **Property 2: as_dict Completeness**
    - **Property 3: Prefix Filtering**
    - **Property 4: from_provider Copies All Pairs**
    - **Validates: Requirements 1.2, 1.3, 1.4, 1.5, 1.8**

- [x] 2. Add Python bindings for SimpleMetadataProvider
  - [x] 2.1 Create PySimpleMetadataProvider in src/bindings/simple_metadata.rs
    - Add #[pyclass] wrapper around Arc<SimpleMetadataProvider>
    - Implement #[new] constructor with optional source MetadataProvider
    - Implement set(), get(), remove(), clear() methods
    - Implement raw property and as_dict() method
    - _Requirements: 1.7_
  
  - [x] 2.2 Register SimpleMetadataProvider in module exports
    - Add to src/bindings/mod.rs exports
    - Register in Python module initialization
    - _Requirements: 1.7_
  
  - [x] 2.3 Write Python unit tests for SimpleMetadataProvider
    - Test construction (empty and from existing provider)
    - Test set/get/remove/clear operations
    - Test as_dict with and without prefix
    - _Requirements: 1.7_

- [x] 3. Checkpoint - Verify SimpleMetadataProvider works
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Update MemoryImageAssetProvider to accept custom metadata
  - [x] 4.1 Add metadata field to MemoryImageAssetProvider
    - Add `metadata: Arc<dyn MetadataProvider>` field
    - Update `new()` to use EmptyMetadataProvider by default
    - Add `with_metadata()` constructor that accepts custom metadata
    - _Requirements: 2.1, 2.2, 2.3_
  
  - [x] 4.2 Remove imode from MemoryImageConfig
    - Remove `imode` field from MemoryImageConfig struct
    - Remove `with_imode()` builder method
    - Update default() to not set imode
    - Keep `irep` field (derived from band count)
    - _Requirements: 2.4_
  
  - [x] 4.3 Update Python bindings for MemoryImageAssetProvider
    - Remove `imode` parameter from create() method
    - Add `metadata` parameter accepting MetadataProvider
    - Update docstrings
    - _Requirements: 2.1, 2.2_
  
  - [x] 4.4 Write property test for metadata round-trip
    - **Property 5: MemoryImageAssetProvider Metadata Round-Trip**
    - **Validates: Requirements 2.2**

- [x] 5. Implement encoding hint extraction in JBPDatasetWriter
  - [x] 5.1 Add EncodingHints struct and extraction method
    - Create EncodingHints struct with imode, ic, nppbh, nppbv, comrat fields
    - Add `extract_encoding_hints()` method to JBPDatasetWriter
    - Read hints from asset.metadata().as_dict(None)
    - Use defaults for missing fields
    - _Requirements: 3.1, 3.7_
  
  - [x] 5.2 Add encoding hint validation
    - Add `validate_encoding_hints()` method
    - Validate IMODE is in {B, P, R, S}
    - Validate NPPBH and NPPBV are in [1, 8192]
    - Auto-adjust block sizes larger than image dimensions
    - Return appropriate errors for invalid values
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  
  - [x] 5.3 Update image subheader creation to use encoding hints
    - Modify `create_image_subheader_with_tres()` to accept EncodingHints
    - Use hints.imode instead of props.imode
    - Use hints.nppbh and hints.nppbv for block sizes
    - Use hints.ic for compression code
    - _Requirements: 3.2, 3.3, 3.4, 3.5, 3.6_
  
  - [x] 5.4 Update IMODE conversion to use encoding hints
    - Modify `convert_bsq_to_imode()` to use hints.imode
    - Update `extract_image_properties()` to not include imode
    - _Requirements: 3.2_
  
  - [ ]* 5.5 Write property tests for encoding hints
    - **Property 6: Encoding Hints Applied to Output**
    - **Property 7: Invalid IMODE Values Cause Errors**
    - **Property 8: Invalid Block Size Values Cause Errors**
    - **Property 9: Block Sizes Auto-Adjusted to Image Dimensions**
    - **Validates: Requirements 3.2, 3.4, 3.5, 4.1, 4.3, 4.4, 4.5**

- [x] 6. Implement conflict resolution
  - [x] 6.1 Add conflict detection and resolution logic
    - Provider structural properties (num_bands, pixel_type, dimensions) override metadata
    - Log warnings for IREP/band count mismatches
    - _Requirements: 5.1, 5.3_
  
  - [ ]* 6.2 Write property test for conflict resolution
    - **Property 10: Provider Structural Properties Override Metadata**
    - **Validates: Requirements 5.1, 5.3**

- [x] 7. Checkpoint - Verify writer encoding hints work
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Update generate_synthetic_image.py script
  - [x] 8.1 Update script to use SimpleMetadataProvider for encoding hints
    - Import SimpleMetadataProvider
    - Create metadata provider and set IMODE hint
    - Pass metadata to MemoryImageAssetProvider.create()
    - Remove imode parameter from create() call
    - _Requirements: 6.1, 6.2_
  
  - [x] 8.2 Test script with various IMODE values
    - Verify script works with IMODE B, P, R, S
    - Verify output files have correct IMODE
    - _Requirements: 3.2_

- [x] 9. Fix field name casing to use lowercase (matching .ksy files)
  - [x] 9.1 Update writer to read lowercase field names from metadata
    - Change extract_encoding_hints() to read "imode" instead of "IMODE"
    - Change to read "ic" instead of "IC"
    - Change to read "nppbh" instead of "NPPBH"
    - Change to read "nppbv" instead of "NPPBV"
    - Change to read "comrat" instead of "COMRAT"
    - _Requirements: 6.1_
  
  - [x] 9.2 Update all Rust tests and examples to use lowercase field names
    - Update src/simple_metadata.rs tests
    - Update src/memory_image.rs tests and examples
    - Update src/bindings/simple_metadata.rs docstrings
    - Update src/bindings/memory_image.rs docstrings
    - _Requirements: 6.1_
  
  - [x] 9.3 Update Python tests and scripts to use lowercase field names
    - Update tests/test_simple_metadata.py
    - Update scripts/generate_synthetic_image.py
    - _Requirements: 6.1_

- [x] 10. Expand image subheader metadata in reader (generic .ksy-driven approach)
  - [x] 10.1 Extend create_image_subheader_definition() to include encoding fields
    - The metadata exposure MUST remain fully generic and driven by .ksy structure definitions
    - Add nrows, ncols, pvtype, irep, icat, abpp, pjust fields
    - Add icords field (conditional igeolo handling may be deferred)
    - Add ic field
    - Add nbands field and band info structure
    - Add isync and imode fields
    - Add nbpr, nbpc, nppbh, nppbv fields
    - _Note: This enables Property 11 (Metadata Field Names Consistent Between Reader and Writer)_
    - _Requirements: 7.1, 7.2, 7.3_
  
  - [x] 10.2 Verify JBPSegmentMetadataProvider uses generic .ksy-driven metadata
    - Ensure as_dict() dynamically returns all fields from .ksy definitions
    - NO hardcoded field lists in facades - the getmetadata API must be fully dynamic
    - Facades MAY access specific fields for writer logic, but metadata exposure stays generic
    - _Requirements: 6.1, 6.3, 7.2, 7.4_

- [x] 11. Document functional test scenarios
  - [x] 11.1 Create docs/TODO_FUNCTIONAL_TESTS.md
    - Document read-modify-write workflow test scenario
    - Document synthetic image with hints test scenario
    - Include expected behaviors and validation criteria
    - _Note: User will update this file outside the spec workflow_

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation uses Rust with PyO3 Python bindings as per the existing codebase
