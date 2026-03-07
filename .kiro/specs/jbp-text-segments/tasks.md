# Implementation Plan: JBP Text Segments

## Overview

This implementation plan adds full JBP Text Segments support to the osml-imagery-io library. The implementation extends the existing `JBPTextAssetProvider` to implement the `TextAssetProvider` trait and adds comprehensive text subheader parsing based on JBP Table 5.17-1.

## Tasks

- [ ] 1. Create text subheader structure definition
  - [ ] 1.1 Create text subheader definition in `src/jbp/text/mod.rs`
    - Define all fields from JBP Table 5.17-1: TE, TEXTID, TXTALVL, TXTDT, TXTITL, security fields, ENCRYP, TXTFMT, TXSHDL, TXSOFL, TXSHD
    - Use existing `StructureDefinition` and `FieldDefinition` patterns
    - Add conditional logic for TXSOFL and TXSHD fields
    - _Requirements: 1.1_

  - [ ]* 1.2 Write unit tests for subheader definition
    - Test field sizes match JBP specification
    - Test conditional field presence
    - _Requirements: 1.1_

- [ ] 2. Implement TextSubheaderFacade
  - [ ] 2.1 Create `TextSubheaderFacade` struct in `src/jbp/text/facade.rs`
    - Implement `from_bytes()` constructor with format validation
    - Add field accessors: te(), textid(), txtalvl(), txtdt(), txtitl(), txtfmt(), encryp()
    - Add TRE accessors: txshdl(), txsofl()
    - _Requirements: 1.1, 3.1_

  - [ ]* 2.2 Write property test for subheader field round-trip
    - **Property 1: Subheader Field Round-Trip**
    - **Validates: Requirements 1.1, 2.5, 3.1**

  - [ ] 2.3 Add validation in facade for required field values
    - Validate TE == "TE"
    - Validate ENCRYP == "0"
    - Return appropriate CodecError for invalid values
    - Allow unknown TXTFMT values (no validation)
    - _Requirements: 1.2, 1.3, 1.4_

  - [ ]* 2.4 Write property test for invalid field validation
    - **Property 2: Invalid Field Validation**
    - **Validates: Requirements 1.2, 1.3**

- [ ] 3. Checkpoint - Ensure subheader parsing tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement text encoding/decoding functions
  - [ ] 4.1 Create text encoding module in `src/jbp/text/encoding.rs`
    - Implement `decode_and_normalize()` function for STA, U8S, UT1, MTF
    - Implement `normalize_line_endings()` function (CR/LF → platform-native)
    - Implement `encode_with_crlf()` function for writing
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 7.4, 7.5, 7.6_

  - [ ]* 4.2 Write property test for text decoding round-trip
    - **Property 6: Text Decoding Round-Trip**
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4**

  - [ ] 4.3 Implement error handling for invalid encoding sequences
    - Return CodecError for invalid ASCII bytes in STA
    - Return CodecError for invalid UTF-8 bytes in U8S
    - Return CodecError for invalid MTF bytes
    - _Requirements: 4.5_

  - [ ]* 4.4 Write property test for invalid encoding error
    - **Property 7: Invalid Encoding Error**
    - **Validates: Requirements 4.5**

  - [ ]* 4.5 Write property test for line delimiter normalization
    - **Property 13: Line Delimiter Normalization (Write)**
    - **Validates: Requirements 7.4, 7.5, 7.6**

- [ ] 5. Update JBPTextAssetProvider
  - [ ] 5.1 Extend `JBPTextAssetProvider` struct in `src/jbp/asset.rs`
    - Add `txtfmt` field to store the text format code
    - Update constructor to accept txtfmt parameter
    - _Requirements: 2.1, 2.2, 2.3, 2.4_

  - [ ] 5.2 Implement `TextAssetProvider` trait for `JBPTextAssetProvider`
    - Implement `text()` using decode_and_normalize()
    - Implement `encoding()` returning normalized encoding name
    - Implement `format()` returning TXTFMT code
    - _Requirements: 5.1, 5.2_

  - [ ] 5.3 Update `media_type()` to return encoding-aware MIME type
    - STA → "text/plain; charset=us-ascii"
    - U8S → "text/plain; charset=utf-8"
    - UT1 → "text/plain; charset=iso-8859-1"
    - MTF → "text/plain"
    - _Requirements: 5.4, 5.5, 5.6, 5.7_

  - [ ]* 5.4 Write property test for encoding name mapping
    - **Property 4: Encoding Name Mapping**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4**

  - [ ]* 5.5 Write property test for media type mapping
    - **Property 10: Media Type Mapping**
    - **Validates: Requirements 5.4, 5.5, 5.6, 5.7**

  - [ ]* 5.6 Write property test for asset type invariant
    - **Property 9: Asset Type Invariant**
    - **Validates: Requirements 5.3**

  - [ ]* 5.7 Write property test for raw asset preservation
    - **Property 8: Raw Asset Preservation**
    - **Validates: Requirements 4.6**

- [ ] 6. Checkpoint - Ensure asset provider tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Update JBPDatasetReader for text segments
  - [ ] 7.1 Update `create_text_subheader_definition()` to use full definition
    - Replace minimal definition with complete JBP Table 5.17-1 definition
    - Register definition in StructureRegistry
    - _Requirements: 1.1_

  - [ ] 7.2 Update `parse_segment()` for text segments
    - Use TextSubheaderFacade for validation
    - Extract title from TXTITL field
    - Extract TXTFMT and pass to JBPTextAssetProvider
    - _Requirements: 1.1, 1.2, 1.3, 1.4_

  - [ ] 7.3 Implement `extract_text_tres()` method
    - Parse TXSHD bytes as TRE envelopes when TXSHDL > 0
    - Handle TXSOFL overflow to DES segments
    - _Requirements: 6.1, 6.2_

  - [ ]* 7.4 Write property test for TRE parsing
    - **Property 11: TRE Parsing**
    - **Validates: Requirements 6.1, 6.3**

  - [ ]* 7.5 Write property test for TRE overflow resolution
    - **Property 12: TRE Overflow Resolution**
    - **Validates: Requirements 6.2**

- [ ] 8. Checkpoint - Ensure reader tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 9. Implement BufferedTextAssetProvider
  - [ ] 9.1 Create `BufferedTextAssetProvider` struct in `src/buffered/text.rs`
    - Implement constructor with key, text_content, encoding parameters
    - Implement builder methods: with_title(), with_description(), with_roles(), with_metadata()
    - _Requirements: 7.1, 7.2, 7.3_

  - [ ] 9.2 Implement `TextAssetProvider` trait for `BufferedTextAssetProvider`
    - Implement `text()` returning stored content
    - Implement `encoding()` returning stored encoding
    - Implement `format()` returning TXTFMT code based on encoding
    - _Requirements: 7.1_

  - [ ] 9.3 Implement `AssetProvider` trait for `BufferedTextAssetProvider`
    - Implement `raw_asset()` using encode_with_crlf()
    - Implement `media_type()` with charset parameter
    - Implement `asset_type()` returning AssetType::Text
    - _Requirements: 7.4, 7.5, 7.6_

  - [ ]* 9.4 Write unit tests for BufferedTextAssetProvider
    - Test construction with various encodings
    - Test line ending conversion
    - Test round-trip: create → raw_asset → decode
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

- [ ] 10. Checkpoint - Ensure Rust tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 11. Update Python bindings
  - [ ] 11.1 Update PyTextAssetProvider to use TextAssetProvider trait
    - Ensure inner type is `Arc<dyn TextAssetProvider>`
    - Verify all AssetProvider methods are exposed
    - Verify text, encoding, format properties are exposed
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6_

  - [ ] 11.2 Update JBPDatasetReader Python binding
    - Ensure get_asset() returns PyTextAssetProvider for text segments
    - _Requirements: 8.7_

  - [ ] 11.3 Add PyBufferedTextAssetProvider binding
    - Expose constructor and builder methods
    - _Requirements: 7.1, 7.2, 7.3_

  - [ ]* 11.4 Write Python property test for API completeness
    - **Property 14: Python API Completeness**
    - **Validates: Requirements 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7**

- [ ] 12. Checkpoint - Ensure Python tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 13. Additional property tests
  - [ ]* 13.1 Write property test for unknown format code handling
    - **Property 3: Unknown Format Code Handling**
    - **Validates: Requirements 1.4**

  - [ ]* 13.2 Write property test for invalid TXTALVL reference parsing
    - **Property 5: Invalid TXTALVL Reference Parsing**
    - **Validates: Requirements 3.4**

- [ ] 14. Update documentation
  - [ ] 14.1 Update `docs/API_DESIGN.md`
    - Add TextAssetProvider section documenting the interface
    - Add usage examples for accessing text content and metadata
    - Add example for BufferedTextAssetProvider
    - _Requirements: 9.1_

  - [ ] 14.2 Update `docs/JBP_ROADMAP.md`
    - Mark Phase 2 (Text Segments) as complete
    - _Requirements: 9.2_

  - [ ] 14.3 Update `docs/JBP_CLEVEL_ASSESSMENT.md`
    - Update Text Segments section to show ✅ implemented status
    - Update all text-related rows in the feature matrix
    - _Requirements: 9.3_

- [ ] 15. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation follows existing patterns in `src/jbp/graphics/` for consistency
