# Implementation Plan: JBP Graphic Segments

## Overview

This implementation plan adds full JBP Graphic Segments support to the osml-imagery-io library. The implementation extends the existing `JBPGraphicsAssetProvider` to implement the `GraphicsAssetProvider` trait and adds comprehensive graphic subheader parsing based on JBP Table 5.15-1.

## Tasks

- [ ] 1. Create graphic subheader structure definition
  - [ ] 1.1 Create full graphic subheader definition in `src/jbp/graphics/mod.rs`
    - Define all fields from JBP Table 5.15-1: SY, SID, SNAME, security fields, ENCRYP, SFMT, SSTRUCT, SDLVL, SALVL, SLOC, SBND1, SCOLOR, SBND2, SRES2, SXSHDL, SXSOFL, SXSHD
    - Use existing `StructureDefinition` and `FieldDefinition` patterns
    - Add conditional logic for SXSOFL and SXSHD fields
    - _Requirements: 1.1_

  - [ ]* 1.2 Write unit tests for subheader definition
    - Test field sizes match JBP specification
    - Test conditional field presence
    - _Requirements: 1.1_

- [ ] 2. Implement GraphicSubheaderFacade
  - [ ] 2.1 Create `GraphicSubheaderFacade` struct in `src/jbp/graphics/facade.rs`
    - Implement `from_bytes()` constructor with format validation
    - Add field accessors: sy(), sid(), sname(), sfmt(), encryp()
    - Add display/attachment accessors: sdlvl(), salvl()
    - Add location accessors: sloc(), sbnd1(), sbnd2(), scolor()
    - Add TRE accessors: sxshdl(), sxsofl()
    - Implement location parsing helper for RRRRRCCCCC format
    - _Requirements: 1.1, 2.1, 3.1, 4.1, 4.2, 4.3_

  - [ ]* 2.2 Write property test for subheader field round-trip
    - **Property 1: Subheader Field Round-Trip**
    - **Validates: Requirements 1.1, 2.1, 2.2, 2.3, 3.1, 4.1, 4.2, 4.3**

  - [ ] 2.3 Add validation in facade for required field values
    - Validate SY == "SY"
    - Validate SFMT == "C"
    - Validate ENCRYP == "0"
    - Return appropriate CodecError for invalid values
    - _Requirements: 1.2, 1.3, 1.4_

  - [ ]* 2.4 Write property test for invalid field validation
    - **Property 2: Invalid Field Validation**
    - **Validates: Requirements 1.2, 1.3, 1.4**

- [ ] 3. Checkpoint - Ensure subheader parsing tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Update JBPGraphicsAssetProvider
  - [ ] 4.1 Implement `GraphicsAssetProvider` trait for `JBPGraphicsAssetProvider`
    - Add empty trait implementation (trait has no additional methods)
    - Verify all `AssetProvider` methods are correctly implemented
    - _Requirements: 6.1, 6.2, 6.3_

  - [ ]* 4.2 Write property test for asset type invariant
    - **Property 8: Asset Type Invariant**
    - **Validates: Requirements 6.3**

  - [ ]* 4.3 Write property test for media type invariant
    - **Property 6: Media Type Invariant**
    - **Validates: Requirements 5.2**

- [ ] 5. Update JBPDatasetReader for graphic segments
  - [ ] 5.1 Update `create_graphic_subheader_definition()` to use full definition
    - Replace minimal definition with complete JBP Table 5.15-1 definition
    - Register definition in StructureRegistry
    - _Requirements: 1.1_

  - [ ] 5.2 Update `parse_segment()` for graphic segments
    - Use GraphicSubheaderFacade for validation
    - Extract title from SNAME field
    - Extract description from SID field
    - _Requirements: 1.1, 1.2, 1.3, 1.4_

  - [ ] 5.3 Implement `extract_graphic_tres()` method
    - Parse SXSHD bytes as TRE envelopes when SXSHDL > 0
    - Handle SXSOFL overflow to DES segments
    - _Requirements: 7.1, 7.2_

  - [ ]* 5.4 Write property test for TRE parsing
    - **Property 9: TRE Parsing**
    - **Validates: Requirements 7.1, 7.3**

  - [ ]* 5.5 Write property test for TRE overflow resolution
    - **Property 10: TRE Overflow Resolution**
    - **Validates: Requirements 7.2**

- [ ] 6. Checkpoint - Ensure reader tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Implement CGM data access
  - [ ] 7.1 Verify `raw_asset()` implementation in JBPGraphicsAssetProvider
    - Ensure bounds checking is correct
    - Ensure complete CGM data is returned
    - _Requirements: 5.1, 5.3_

  - [ ]* 7.2 Write property test for CGM data round-trip
    - **Property 5: CGM Data Round-Trip**
    - **Validates: Requirements 5.1**

  - [ ]* 7.3 Write property test for bounds validation error
    - **Property 7: Bounds Validation Error**
    - **Validates: Requirements 5.3**

- [ ] 8. Implement CLEVEL validation
  - [ ] 8.1 Add graphic segment size validation to CLevelValidator
    - Add `validate_graphic_aggregate_size()` method
    - Implement CLEVEL 03 limit (1 MB)
    - Implement CLEVEL 05+ limit (2 MB)
    - _Requirements: 8.1, 8.2_

  - [ ]* 8.2 Write property test for CLEVEL size validation
    - **Property 11: CLEVEL Size Validation**
    - **Validates: Requirements 8.1, 8.2**

- [ ] 9. Checkpoint - Ensure Rust tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 10. Update Python bindings
  - [ ] 10.1 Update PyGraphicsAssetProvider to use GraphicsAssetProvider trait
    - Ensure inner type is `Arc<dyn GraphicsAssetProvider>`
    - Verify all AssetProvider methods are exposed
    - _Requirements: 9.1, 9.2, 9.3_

  - [ ] 10.2 Update JBPDatasetReader Python binding
    - Ensure get_asset() returns PyGraphicsAssetProvider for graphic segments
    - _Requirements: 9.4_

  - [ ]* 10.3 Write Python property test for API completeness
    - **Property 12: Python API Completeness**
    - **Validates: Requirements 9.1, 9.2, 9.3, 9.4**

- [ ] 11. Checkpoint - Ensure Python tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 12. Additional property tests
  - [ ]* 12.1 Write property test for invalid SALVL reference parsing
    - **Property 3: Invalid SALVL Reference Parsing**
    - **Validates: Requirements 3.4**

  - [ ]* 12.2 Write property test for invalid bounds parsing
    - **Property 4: Invalid Bounds Parsing**
    - **Validates: Requirements 4.4, 4.5**

- [ ] 13. Update documentation
  - [ ] 13.1 Update `docs/API_DESIGN.md`
    - Add GraphicsAssetProvider section documenting the interface
    - Add usage examples for accessing graphic metadata
    - Add example for accessing raw CGM data
    - _Requirements: 10.1_

  - [ ] 13.2 Update `docs/JBP_ROADMAP.md`
    - Mark Phase 1 (Graphic Segments) as complete
    - _Requirements: 10.2_

  - [ ] 13.3 Update `docs/JBP_CLEVEL_ASSESSMENT.md`
    - Update Graphic Segments section to show ✅ implemented status
    - Update all graphic-related rows in the feature matrix
    - _Requirements: 10.3_

- [ ] 14. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation follows existing patterns in `src/jbp/image/` for consistency
