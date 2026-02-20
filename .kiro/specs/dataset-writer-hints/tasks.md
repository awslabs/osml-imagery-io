# Implementation Plan: Dataset Writer Encoding Hints

## Overview

This implementation plan covers adding encoding hints support to the dataset writer system. The feature allows users to pass format-specific encoding options (IMODE, IC, NPPBH, NPPBV, COMRAT) through the MetadataProvider interface rather than format-specific parameters in abstract interfaces.

The implementation is in Rust with PyO3 Python bindings, following the existing codebase patterns.

## Tasks

- [ ] 1. Implement SimpleMetadataProvider in Rust
  - [ ] 1.1 Create SimpleMetadataProvider struct with thread-safe HashMap storage
    - Add `src/simple_metadata.rs` with RwLock<HashMap<String, serde_json::Value>>
    - Implement `new()` constructor
    - Implement `set(key, value)` and `get(key)` methods
    - Implement `remove(key)` and `clear()` methods
    - _Requirements: 1.1, 1.2, 1.3_
  
  - [ ] 1.2 Implement MetadataProvider trait for SimpleMetadataProvider
    - Implement `raw()` returning empty bytes (no raw representation)
    - Implement `as_dict(None)` returning all stored pairs
    - Implement `as_dict(Some(prefix))` with prefix filtering
    - _Requirements: 1.4, 1.5_
  
  - [ ] 1.3 Implement from_provider constructor
    - Add `from_provider(source: &dyn MetadataProvider)` method
    - Copy all key-value pairs from source.as_dict(None)
    - _Requirements: 1.8_
  
  - [ ]* 1.4 Write property tests for SimpleMetadataProvider
    - **Property 1: Set/Get Round-Trip**
    - **Property 2: as_dict Completeness**
    - **Property 3: Prefix Filtering**
    - **Property 4: from_provider Copies All Pairs**
    - **Validates: Requirements 1.2, 1.3, 1.4, 1.5, 1.8**

- [ ] 2. Add Python bindings for SimpleMetadataProvider
  - [ ] 2.1 Create PySimpleMetadataProvider in src/bindings/simple_metadata.rs
    - Add #[pyclass] wrapper around Arc<SimpleMetadataProvider>
    - Implement #[new] constructor with optional source MetadataProvider
    - Implement set(), get(), remove(), clear() methods
    - Implement raw property and as_dict() method
    - _Requirements: 1.7_
  
  - [ ] 2.2 Register SimpleMetadataProvider in module exports
    - Add to src/bindings/mod.rs exports
    - Register in Python module initialization
    - _Requirements: 1.7_
  
  - [ ]* 2.3 Write Python unit tests for SimpleMetadataProvider
    - Test construction (empty and from existing provider)
    - Test set/get/remove/clear operations
    - Test as_dict with and without prefix
    - _Requirements: 1.7_

- [ ] 3. Checkpoint - Verify SimpleMetadataProvider works
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Update MemoryImageAssetProvider to accept custom metadata
  - [ ] 4.1 Add metadata field to MemoryImageAssetProvider
    - Add `metadata: Arc<dyn MetadataProvider>` field
    - Update `new()` to use EmptyMetadataProvider by default
    - Add `with_metadata()` constructor that accepts custom metadata
    - _Requirements: 2.1, 2.2, 2.3_
  
  - [ ] 4.2 Remove imode from MemoryImageConfig
    - Remove `imode` field from MemoryImageConfig struct
    - Remove `with_imode()` builder method
    - Update default() to not set imode
    - Keep `irep` field (derived from band count)
    - _Requirements: 2.4_
  
  - [ ] 4.3 Update Python bindings for MemoryImageAssetProvider
    - Remove `imode` parameter from create() method
    - Add `metadata` parameter accepting MetadataProvider
    - Update docstrings
    - _Requirements: 2.1, 2.2_
  
  - [ ]* 4.4 Write property test for metadata round-trip
    - **Property 5: MemoryImageAssetProvider Metadata Round-Trip**
    - **Validates: Requirements 2.2**

- [ ] 5. Implement encoding hint extraction in JBPDatasetWriter
  - [ ] 5.1 Add EncodingHints struct and extraction method
    - Create EncodingHints struct with imode, ic, nppbh, nppbv, comrat fields
    - Add `extract_encoding_hints()` method to JBPDatasetWriter
    - Read hints from asset.metadata().as_dict(None)
    - Use defaults for missing fields
    - _Requirements: 3.1, 3.7_
  
  - [ ] 5.2 Add encoding hint validation
    - Add `validate_encoding_hints()` method
    - Validate IMODE is in {B, P, R, S}
    - Validate NPPBH and NPPBV are in [1, 8192]
    - Auto-adjust block sizes larger than image dimensions
    - Return appropriate errors for invalid values
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  
  - [ ] 5.3 Update image subheader creation to use encoding hints
    - Modify `create_image_subheader_with_tres()` to accept EncodingHints
    - Use hints.imode instead of props.imode
    - Use hints.nppbh and hints.nppbv for block sizes
    - Use hints.ic for compression code
    - _Requirements: 3.2, 3.3, 3.4, 3.5, 3.6_
  
  - [ ] 5.4 Update IMODE conversion to use encoding hints
    - Modify `convert_bsq_to_imode()` to use hints.imode
    - Update `extract_image_properties()` to not include imode
    - _Requirements: 3.2_
  
  - [ ]* 5.5 Write property tests for encoding hints
    - **Property 6: Encoding Hints Applied to Output**
    - **Property 7: Invalid IMODE Values Cause Errors**
    - **Property 8: Invalid Block Size Values Cause Errors**
    - **Property 9: Block Sizes Auto-Adjusted to Image Dimensions**
    - **Validates: Requirements 3.2, 3.4, 3.5, 4.1, 4.3, 4.4, 4.5**

- [ ] 6. Implement conflict resolution
  - [ ] 6.1 Add conflict detection and resolution logic
    - Provider structural properties (num_bands, pixel_type, dimensions) override metadata
    - Log warnings for IREP/band count mismatches
    - _Requirements: 5.1, 5.3_
  
  - [ ]* 6.2 Write property test for conflict resolution
    - **Property 10: Provider Structural Properties Override Metadata**
    - **Validates: Requirements 5.1, 5.3**

- [ ] 7. Checkpoint - Verify writer encoding hints work
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Update generate_synthetic_image.py script
  - [ ] 8.1 Update script to use SimpleMetadataProvider for encoding hints
    - Import SimpleMetadataProvider
    - Create metadata provider and set IMODE hint
    - Pass metadata to MemoryImageAssetProvider.create()
    - Remove imode parameter from create() call
    - _Requirements: 6.1, 6.2_
  
  - [ ]* 8.2 Test script with various IMODE values
    - Verify script works with IMODE B, P, R, S
    - Verify output files have correct IMODE
    - _Requirements: 3.2_

- [ ] 9. Add integration tests
  - [ ]* 9.1 Write read-modify-write integration test
    - Read existing NITF file
    - Copy metadata to SimpleMetadataProvider
    - Modify encoding hints
    - Write new file
    - Verify hints applied
    - **Property 11: Metadata Field Names Consistent Between Reader and Writer**
    - **Validates: Requirements 6.1, 6.3**
  
  - [ ]* 9.2 Write synthetic image with hints integration test
    - Create MemoryImageAssetProvider with encoding hints
    - Write to NITF
    - Read back and verify hints
    - _Requirements: 3.2, 3.4, 3.5_

- [ ] 10. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation uses Rust with PyO3 Python bindings as per the existing codebase
