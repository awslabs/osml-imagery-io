# Implementation Plan: API Design Alignment

## Overview

This implementation plan covers aligning the API design document with the actual implementation and vice versa. Changes are organized to minimize breaking changes and ensure tests pass at each checkpoint.

## Tasks

- [x] 1. Update IO.open to accept list of paths
  - [x] 1.1 Modify IO.open signature in src/bindings/io.rs
    - Change `uri: &str` parameter to `paths: Vec<String>`
    - Use `paths.first()` to get the first path for single-file implementations
    - Add validation to return ValueError if paths is empty
    - _Requirements: 1.2, 1.3_
  
  - [ ]* 1.2 Write property test for IO.open paths handling
    - **Property 1: IO.open paths list handling**
    - **Validates: Requirements 1.2, 1.3**
  
  - [x] 1.3 Write unit tests for IO.open edge cases
    - Test single-element list
    - Test multi-element list (verify first is used)
    - Test empty list raises ValueError
    - _Requirements: 1.2, 1.3_

- [x] 2. Update DatasetReader to use metadata property
  - [x] 2.1 Change get_metadata() to metadata property in src/bindings/reader.rs
    - Add `#[getter]` attribute to the method
    - Rename method from `get_metadata` to `metadata`
    - _Requirements: 2.2_
  
  - [ ]* 2.2 Write property test for metadata property accessor
    - **Property 2: DatasetReader metadata property accessor**
    - **Validates: Requirements 2.2**

- [x] 3. Update DatasetWriter API
  - [x] 3.1 Remove add_image_asset method from src/bindings/writer.rs
    - Delete the `add_image_asset` method entirely
    - Existing `add_asset` already accepts Arc<dyn AssetProvider>
    - _Requirements: 3.1_
  
  - [x] 3.2 Convert set_metadata to metadata property setter in src/bindings/writer.rs
    - Add `#[setter]` attribute for metadata property
    - Keep existing set_metadata logic in the setter
    - _Requirements: 3.4 (from API_DESIGN.md update)_
  
  - [x] 3.3 Write property test for add_asset type hierarchy
    - **Property 3: add_asset accepts all AssetProvider subtypes**
    - **Validates: Requirements 3.2**

- [x] 4. Checkpoint - Verify core API changes
  - Ensure all tests pass, ask the user if questions arise.
  - Run `cargo test` and `pytest tests/`

- [x] 5. Update TextAssetProvider to use text property
  - [x] 5.1 Change get_text() to text property in src/bindings/text.rs
    - Add `#[getter]` attribute to the method
    - Rename method from `get_text` to `text`
    - _Requirements: 6.4_
  
  - [ ]* 5.2 Write property test for text property accessor
    - **Property 4: TextAssetProvider text property accessor**
    - **Validates: Requirements 6.4**

- [x] 6. Update DataAssetProvider API
  - [x] 6.1 Change get_mime_type() to mime_type property in src/bindings/data.rs
    - Add `#[getter]` attribute to the method
    - Rename method from `get_mime_type` to `mime_type`
    - _Requirements: 7.2_
  
  - [x] 6.2 Update parse_as_xml to return ElementTree in src/bindings/data.rs
    - Import Python's xml.etree.ElementTree module
    - Parse XML string using ElementTree.fromstring()
    - Return the Element object instead of string
    - _Requirements: 7.3_
  
  - [ ]* 6.3 Write property test for mime_type property accessor
    - **Property 5: DataAssetProvider mime_type property accessor**
    - **Validates: Requirements 7.2**
  
  - [ ]* 6.4 Write property test for XML parsing
    - **Property 6: XML parsing returns traversable ElementTree**
    - **Validates: Requirements 7.3**

- [x] 7. Checkpoint - Verify all binding changes
  - Ensure all tests pass, ask the user if questions arise.
  - Run `cargo test` and `pytest tests/`

- [x] 8. Update API_DESIGN.md - Core API Structure
  - [x] 8.1 Update IO class diagram
    - Remove fsspec filesystem parameter
    - Change signature to `open(paths: List[str], mode: str, format: Optional[str])`
    - _Requirements: 1.1, 1.4, 1.5_
  
  - [x] 8.2 Update DatasetReader class diagram
    - Replace `get_metadata()` with `metadata` property
    - _Requirements: 2.1_
  
  - [x] 8.3 Update DatasetWriter class diagram
    - Replace `set_metadata()` with `metadata` property setter
    - Document that add_asset accepts any AssetProvider
    - _Requirements: 3.3, 3.4_

- [x] 9. Update API_DESIGN.md - Asset Provider Hierarchy
  - [x] 9.1 Update AssetProvider class diagram
    - Replace all `get_*()` methods with property accessors
    - Show key, title, description, media_type, roles, asset_type as properties
    - Add from_bytes() static method
    - _Requirements: 4.1, 4.2, 4.3_
  
  - [x] 9.2 Update ImageAssetProvider class diagram
    - Show bands parameter as optional in get_block()
    - _Requirements: 5.1_
  
  - [x] 9.3 Update TextAssetProvider class diagram
    - Replace get_text(), get_encoding(), get_format() with properties
    - _Requirements: 6.1, 6.2, 6.3_
  
  - [x] 9.4 Update DataAssetProvider class diagram
    - Replace get_mime_type() with mime_type property
    - _Requirements: 7.1_

- [x] 10. Update API_DESIGN.md - Remove deprecated concepts
  - [x] 10.1 Remove FilesDatasetReader and FilesDatasetWriter
    - Remove from Format-Specific Implementations diagram
    - Remove any references in text
    - _Requirements: 9.3, 9.4_
  
  - [x] 10.2 Remove all fsspec mentions
    - Remove filesystem parameter from JBPDatasetReader/Writer
    - Remove any fsspec imports or references
    - _Requirements: 9.5_

- [x] 11. Update API_DESIGN.md - Add new documentation
  - [x] 11.1 Update MemoryImageAssetProvider documentation
    - Change constructor to create() static method signature
    - Document set_full_image() and set_block() methods
    - Show ndarray shape (bands, rows, cols)
    - _Requirements: 8.1, 8.2, 8.3_
  
  - [x] 11.2 Add SimpleMetadataProvider section
    - Document class extending MetadataProvider
    - Document set(), get(), remove(), clear() methods
    - Include usage examples for encoding hints
    - _Requirements: 10.1, 10.2, 10.3, 10.4_
  
  - [x] 11.3 Add Parser Infrastructure section
    - Document PyStructureRegistry for managing definitions
    - Document PyStructureDefinition for KSY files
    - Document PyStructureAccessor for reading binary data
    - Document PyStructureWriter for encoding values
    - Include usage examples
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6_

- [x] 12. Update API_DESIGN.md - Usage examples
  - [x] 12.1 Update Basic Blocked Image Access example
    - Change streamline.open to IO.open with list
    - Change get_metadata() to metadata property
    - Use correct import paths
    - _Requirements: 1.6, 2.3, 12.1, 12.2_
  
  - [x] 12.2 Update Multi-Asset Access example
    - Use property accessors throughout
    - _Requirements: 12.1_
  
  - [x] 12.3 Update Creating Datasets from Memory example
    - Use MemoryImageAssetProvider.create() syntax
    - Use add_asset instead of add_image_asset
    - _Requirements: 12.1, 12.2_
  
  - [x] 12.4 Add SimpleMetadataProvider usage example
    - Show setting encoding hints
    - Show using with MemoryImageAssetProvider
    - _Requirements: 12.4_
  
  - [x] 12.5 Add PyStructure classes usage example
    - Show loading structure definitions
    - Show parsing binary data
    - Show writing binary data
    - _Requirements: 12.4_

- [x] 13. Final checkpoint - Verify all changes
  - Ensure all tests pass, ask the user if questions arise.
  - Run `cargo test` and `pytest tests/`
  - Verify API_DESIGN.md renders correctly

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation uses Rust (PyO3) for bindings and Python for tests
