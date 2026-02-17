# Implementation Plan: JBP Dataset Integration

## Overview

This implementation plan breaks down the JBP Dataset Integration into incremental coding tasks. Each task builds on previous work, with property-based tests integrated close to implementation to catch errors early. The implementation uses Rust with PyO3 for Python bindings, building on the Phase 1 data-driven binary parser infrastructure.

## Tasks

- [ ] 1. Set up module structure and core types
  - [ ] 1.1 Create `src/jbp/` module directory with `mod.rs`
    - Add module declarations for reader, writer, asset, metadata, datetime, error submodules
    - _Requirements: 1.1, 7.1_
  - [ ] 1.2 Implement JBP error types in `src/jbp/error.rs`
    - Define JBPError enum with InvalidFormat, AssetNotFound, DuplicateKey, ValidationError variants
    - Implement From<JBPError> for CodecError
    - Define ValidationWarning and ValidationCode types
    - _Requirements: 18.1, 18.2_
  - [ ] 1.3 Implement core type definitions in `src/jbp/types.rs`
    - Define NitfFormat enum (Nitf21, Nsif10)
    - Define SegmentType enum (Image, Graphic, Text, DataExtension, ReservedExtension)
    - Define SegmentLocation struct (subheader_offset, subheader_length, data_offset, data_length)
    - Define SegmentOffsets struct with vectors for each segment type
    - Define JBPReaderOptions struct
    - _Requirements: 1.3, 1.4, 2.1_

- [ ] 2. Implement format detection and segment offset calculation
  - [ ] 2.1 Implement extension-based format detection in `src/jbp/format.rs`
    - Implement is_nitf_extension() for .ntf, .nitf, .nsif (case-insensitive)
    - Implement validate_nitf_magic() for magic number validation during parsing
    - _Requirements: 1.3, 1.4, 1.5, 12.1, 12.2, 12.3_
  - [ ] 2.2 Write property tests for format detection
    - **Property 1: Extension-Based Format Selection**
    - **Property 2: Magic Number Validation During Parse**
    - **Validates: Requirements 1.3, 1.4, 1.5, 12.1, 12.2, 12.3**
  - [ ] 2.3 Implement SegmentOffsets::from_header() in `src/jbp/types.rs`
    - Extract segment counts (NUMI, NUMS, NUMT, NUMDES, NUMRES) from header
    - Calculate cumulative offsets for each segment type
    - Store SegmentLocation for each segment
    - _Requirements: 1.6, 1.7, 2.1, 2.2, 2.3, 2.4, 2.5_
  - [ ] 2.4 Write property test for segment offset calculation
    - **Property 3: Segment Offset Cumulative Calculation**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**

- [ ] 3. Checkpoint - Format detection and offset calculation complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement DateTime parser utility
  - [ ] 4.1 Implement datetime parsing in `src/jbp/datetime.rs`
    - Define NitfDateTime struct with optional components
    - Implement parse_nitf_datetime() for CCYYMMDDhhmmss format
    - Handle "--" for unknown components
    - Implement to_naive_datetime() and to_iso8601() methods
    - Define DateTimeParseError enum
    - _Requirements: 16.1, 16.2, 16.3, 16.4_
  - [ ] 4.2 Write property tests for datetime parsing
    - **Property 17: DateTime Parsing Round-Trip**
    - **Property 18: DateTime Partial Date Handling**
    - **Property 19: DateTime Invalid Input Error**
    - **Validates: Requirements 16.1, 16.2, 16.3, 16.4**

- [ ] 5. Implement metadata providers
  - [ ] 5.1 Implement JBPFileMetadataProvider in `src/jbp/metadata.rs`
    - Create from StructureAccessor and raw bytes
    - Implement MetadataProvider trait
    - Implement as_dict() with prefix-based filtering
    - Implement raw() returning header bytes
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [ ] 5.2 Implement JBPSegmentMetadataProvider in `src/jbp/metadata.rs`
    - Create from StructureAccessor and raw subheader bytes
    - Implement MetadataProvider trait with same prefix filtering
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_
  - [ ] 5.3 Write property tests for metadata providers
    - **Property 8: Metadata Prefix Filtering**
    - **Property 9: Raw Metadata Identity**
    - **Validates: Requirements 5.2, 5.3, 5.4, 6.5**

- [ ] 6. Checkpoint - Metadata providers complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Implement asset providers
  - [ ] 7.1 Implement JBPImageAssetProvider in `src/jbp/asset.rs`
    - Store key, title, description, roles, location, subheader accessor, data reference
    - Implement AssetProvider trait
    - Return "application/vnd.nitf.image" for media_type
    - Implement raw_asset() returning segment data bytes
    - _Requirements: 4.1, 6.1_
  - [ ] 7.2 Implement JBPTextAssetProvider in `src/jbp/asset.rs`
    - Implement AssetProvider trait for text segments
    - Return "text/plain" for media_type
    - _Requirements: 4.3, 6.2_
  - [ ] 7.3 Implement JBPGraphicsAssetProvider in `src/jbp/asset.rs`
    - Implement AssetProvider trait for graphic segments
    - Return "image/cgm" for media_type
    - _Requirements: 4.2, 6.3_
  - [ ] 7.4 Implement JBPDataAssetProvider in `src/jbp/asset.rs`
    - Implement AssetProvider trait for DES segments
    - Return "application/octet-stream" for media_type
    - _Requirements: 4.4, 6.4_

- [ ] 8. Implement asset key generation
  - [ ] 8.1 Implement asset key utilities in `src/jbp/asset.rs`
    - Implement generate_asset_key(segment_type, index) → "{type}_segment_{index}"
    - Implement parse_asset_key(key) → Option<(SegmentType, usize)>
    - _Requirements: 3.6_
  - [ ] 8.2 Write property tests for asset key generation
    - **Property 4: Asset Key Enumeration Completeness**
    - **Property 5: Asset Key Type Filtering**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6**

- [ ] 9. Checkpoint - Asset providers complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 10. Implement JBPDatasetReader
  - [ ] 10.1 Implement reader initialization in `src/jbp/reader.rs`
    - Implement JBPDatasetReader::open(path) using memory-mapped file
    - Implement JBPDatasetReader::from_bytes(data)
    - Implement JBPDatasetReader::with_options(path, options)
    - Validate magic number during header parsing
    - Calculate segment offsets from header
    - Create file metadata provider
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_
  - [ ] 10.2 Implement DatasetReader trait for JBPDatasetReader
    - Implement get_asset_keys() with type and role filtering
    - Implement has_asset() checking key validity
    - Implement get_asset() with lazy subheader parsing and caching
    - Implement metadata() returning file metadata provider
    - Implement close() releasing resources
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.7, 4.1, 4.2, 4.3, 4.4, 4.7, 5.1_
  - [ ] 10.3 Write property tests for reader
    - **Property 6: Asset Key Existence Consistency**
    - **Property 7: Segment Subheader Parsing**
    - **Validates: Requirements 3.7, 4.1, 4.2, 4.3, 4.4, 4.7**
  - [ ] 10.4 Implement validation in reader
    - Validate CLEVEL and add warning for invalid values
    - Validate segment counts match length arrays
    - Implement optional file length validation
    - Implement warnings() method to retrieve collected warnings
    - _Requirements: 13.1, 13.2, 13.3, 14.1, 14.2, 14.3, 14.4, 14.5, 15.1, 15.2, 15.3, 15.4, 15.5, 15.6, 18.1, 18.2, 18.3, 18.4_
  - [ ] 10.5 Write property tests for validation
    - **Property 14: Segment Count Consistency**
    - **Property 15: File Length Validation (When Enabled)**
    - **Property 16: File Length Validation Skip (When Disabled)**
    - **Property 21: CLEVEL Validation**
    - **Property 22: Warning Collection**
    - **Validates: Requirements 13.1, 13.2, 13.3, 14.1, 14.5, 15.2, 15.5, 15.6, 18.1, 18.4**

- [ ] 11. Checkpoint - JBPDatasetReader complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 12. Implement JBPDatasetWriter
  - [ ] 12.1 Implement writer initialization in `src/jbp/writer.rs`
    - Implement JBPDatasetWriter::new(path, format)
    - Implement JBPDatasetWriter::with_registry(path, format, registry)
    - Initialize empty asset queue and metadata storage
    - _Requirements: 7.1, 7.2, 7.3_
  - [ ] 12.2 Implement DatasetWriter trait for JBPDatasetWriter
    - Implement add_asset() queuing assets by type
    - Implement set_metadata() storing file metadata
    - Implement close() with two-pass writing
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 10.1, 10.2, 10.3_
  - [ ] 12.3 Write property tests for writer
    - **Property 10: Asset Addition Type Mapping**
    - **Property 11: Duplicate Key Rejection**
    - **Property 12: Asset Order Preservation**
    - **Validates: Requirements 8.1, 8.5, 8.6**
  - [ ] 12.4 Implement two-pass writing in close()
    - Calculate all segment lengths
    - Write file header with correct counts and length arrays
    - Write each segment subheader and data in order
    - Update FL field with total file size
    - _Requirements: 9.1, 9.2, 9.3, 9.4_
  - [ ] 12.5 Write property test for file header consistency
    - **Property 13: File Header Length Consistency**
    - **Validates: Requirements 9.1, 9.2, 9.4**

- [ ] 13. Checkpoint - JBPDatasetWriter complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 14. Implement IO factory
  - [ ] 14.1 Implement IO struct in `src/jbp/io.rs`
    - Implement IO::open(path) with extension-based format selection
    - Implement IO::open_as(path, format) for explicit format
    - Implement IO::create(path, format) for writing
    - _Requirements: 19.3, 19.4_
  - [ ] 14.2 Write property test for format auto-detection
    - **Property 23: Python Format Auto-Detection**
    - **Validates: Requirements 19.3**

- [ ] 15. Implement round-trip consistency
  - [ ] 15.1 Create synthetic NITF file generator for testing
    - Generate valid NITF headers with configurable segment counts
    - Generate minimal segment subheaders and data
    - _Requirements: 17.1, 17.2_
  - [ ] 15.2 Write property test for round-trip
    - **Property 20: Dataset Round-Trip Consistency**
    - **Validates: Requirements 17.1, 17.2, 17.3**

- [ ] 16. Checkpoint - Rust implementation complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 17. Create structure definition files
  - [ ] 17.1 Create `data/structures/nitf/nitf_02.10_file_header.ksy`
    - Define all fields from NITF 2.1 Table 5.1-1
    - Include FHDR, FVER, CLEVEL, STYPE, OSTAID, FDT, FTITLE
    - Include security fields (FSCLAS, FSCLSY, FSCODE, etc.)
    - Include segment count fields (NUMI, NUMS, NUMT, NUMDES, NUMRES)
    - Include segment length arrays with repetitions
    - _Requirements: 11.1_
  - [ ] 17.2 Create `data/structures/nitf/nitf_02.10_image_subheader.ksy`
    - Define all fields from NITF 2.1 Table 5.13-1
    - Include IM, IID1, IDATIM, TGTID, IID2
    - Include security fields
    - Include image dimensions (NROWS, NCOLS)
    - Include compression and band info
    - _Requirements: 11.2_
  - [ ] 17.3 Create `data/structures/nitf/nitf_02.10_graphic_subheader.ksy`
    - Define all fields from NITF 2.1 Table 5.15-1
    - Include SY, SID, SNAME, SFMT, SSTRUCT
    - Include display level fields (SDLVL, SALVL)
    - _Requirements: 11.3_
  - [ ] 17.4 Create `data/structures/nitf/nitf_02.10_text_subheader.ksy`
    - Define all fields from NITF 2.1 Table 5.17-1
    - Include TE, TEXTID, TXTALVL, TXTFMT
    - _Requirements: 11.4_
  - [ ] 17.5 Create `data/structures/nitf/nitf_02.10_des_subheader.ksy`
    - Define all fields from NITF 2.1 Table 5.18-1
    - Include DE, DESID, DESVER, DESOFLW, DESITEM
    - _Requirements: 11.5_
  - [ ] 17.6 Create `data/structures/nsif/nsif_01.00_file_header.ksy`
    - Define NSIF 1.0 file header fields (similar to NITF 2.1)
    - _Requirements: 11.6_

- [ ] 18. Create synthetic test data
  - [ ] 18.1 Create `data/unit/sample_nitf21.ntf`
    - Minimal valid NITF 2.1 file with one image segment
    - Valid magic number, header, and segment structure
    - _Requirements: 1.1, 1.3_
  - [ ] 18.2 Create `data/unit/sample_nsif10.nsif`
    - Minimal valid NSIF 1.0 file with one image segment
    - _Requirements: 1.2, 1.4_
  - [ ] 18.3 Create `data/unit/multi_segment.ntf`
    - NITF file with multiple segments of different types
    - Test segment offset calculation
    - _Requirements: 2.2, 2.3, 2.4, 2.5_

- [ ] 19. Checkpoint - Structure definitions and test data complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 20. Implement Python bindings
  - [ ] 20.1 Create PyO3 bindings in `src/bindings/jbp.rs`
    - Expose PyIO class with open(), open_as(), create() methods
    - Wrap DatasetReader and DatasetWriter as Python objects
    - Implement context manager protocol (__enter__, __exit__)
    - _Requirements: 19.1, 19.2, 19.5_
  - [ ] 20.2 Implement path handling in Python bindings
    - Accept str and pathlib.Path for file paths
    - Support fsspec filesystem objects
    - _Requirements: 19.6, 19.7_
  - [ ] 20.3 Export bindings in `python/aws/osml/io/__init__.py`
    - Add IO class export
    - Ensure DatasetReader/DatasetWriter are accessible but JBP implementations hidden
    - _Requirements: 19.1, 19.2_

- [ ] 21. Write Python integration tests
  - [ ] 21.1 Create `tests/test_jbp_reader.py`
    - Test IO.open() with NITF files
    - Test get_asset_keys() with type filtering
    - Test get_asset() returns correct asset providers
    - Test metadata().as_dict() with prefix filtering
    - Test context manager protocol
    - _Requirements: 19.1, 19.3, 19.5_
  - [ ] 21.2 Create `tests/test_jbp_writer.py`
    - Test IO.create() with format specification
    - Test add_asset() and set_metadata()
    - Test close() produces valid NITF file
    - Test round-trip read/write
    - _Requirements: 19.4, 17.1, 17.2_

- [ ] 22. Final checkpoint - All tests pass
  - Run `cargo test` for Rust tests
  - Run `maturin develop` to build Python extension
  - Run `pytest` for Python tests
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- All tasks including property tests are required for comprehensive coverage
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with minimum 100 iterations
- Rust unit tests are inline with source using `#[cfg(test)]`
- Python tests are in `tests/` directory using pytest
- Unit test data goes in `data/unit/` (checked into git)
- Integration tests with JITC files require `data/integration/JITC/` (gitignored)
