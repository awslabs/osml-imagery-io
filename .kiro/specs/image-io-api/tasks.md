# Implementation Plan: Image IO API

## Overview

This plan implements the foundational image IO API types and traits for the aws-osml-io library. The implementation is in Rust with Python bindings via PyO3. Tasks are ordered to build incrementally from core types to complex traits, with testing integrated throughout.

## Tasks

- [x] 1. Set up project structure and core types
  - [x] 1.1 Create module structure with traits/ and bindings/ directories
    - Create `src/traits/mod.rs` with submodule declarations
    - Create `src/bindings/mod.rs` with submodule declarations
    - Update `src/lib.rs` to include new modules
    - _Requirements: 11.1_

  - [x] 1.2 Implement AssetType enumeration
    - Create `src/types.rs` with AssetType enum (Image, Text, Graphics, Data)
    - Add PyO3 `#[pyclass]` derive for Python exposure
    - Implement Clone, Copy, PartialEq, Eq, Hash derives
    - _Requirements: 10.1, 10.3_

  - [x] 1.3 Implement PixelType enumeration
    - Add PixelType enum to `src/types.rs` (UInt8, UInt16, UInt32, Int8, Int16, Int32, Float32, Float64)
    - Implement `to_numpy_dtype()` method returning dtype string
    - Implement `bytes_per_pixel()` method
    - Add PyO3 `#[pyclass]` derive
    - _Requirements: 5.8_

  - [x] 1.4 Write unit tests for enumerations
    - Test AssetType equality comparisons
    - Test PixelType::to_numpy_dtype() returns correct strings
    - Test PixelType::bytes_per_pixel() returns correct values
    - _Requirements: 10.3_

- [x] 2. Implement error types and exception mapping
  - [x] 2.1 Extend CodecError enumeration
    - Add AssetNotFound(String) variant
    - Add InvalidBlockCoordinates(u32, u32, u32) variant
    - Add InvalidResolutionLevel(u32) variant
    - Add Parse(String) variant
    - Add DuplicateKey(String) variant
    - _Requirements: 1.7, 2.5, 5.13, 5.14, 8.4, 8.5_

  - [x] 2.2 Implement Python exception mapping
    - Update `From<CodecError> for PyErr` implementation
    - Map AssetNotFound to PyKeyError
    - Map DuplicateKey to PyValueError
    - Map InvalidBlockCoordinates to PyIndexError
    - Map InvalidResolutionLevel to PyValueError
    - Map Parse to PyValueError
    - _Requirements: 11.4_

  - [ ]* 2.3 Write property test for exception mapping
    - **Property 13: Python Exception Mapping**
    - Generate each CodecError variant, convert to PyErr, verify correct Python type
    - **Validates: Requirements 11.4**

- [x] 3. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Implement MetadataProvider trait and binding
  - [x] 4.1 Define MetadataProvider trait
    - Create `src/traits/metadata.rs`
    - Define `fn raw(&self) -> &[u8]` method
    - Define `fn as_dict(&self, name: Option<&str>) -> HashMap<String, serde_json::Value>` method
    - Add Send + Sync bounds
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [x] 4.2 Implement PyMetadataProvider binding
    - Create `src/bindings/metadata.rs`
    - Wrap `Arc<dyn MetadataProvider>` in PyMetadataProvider struct
    - Implement `raw` property returning Python BytesIO
    - Implement `as_dict` method with optional name parameter
    - _Requirements: 6.1, 6.2, 11.1_

  - [ ]* 4.3 Write property test for metadata filtering
    - **Property 11: Metadata Filtering**
    - Create mock MetadataProvider with multiple sections
    - Verify as_dict(name) returns only named section
    - Verify as_dict() returns all sections
    - **Validates: Requirements 6.3, 6.4**

- [x] 5. Implement AssetProvider trait and binding
  - [x] 5.1 Define AssetProvider trait
    - Create `src/traits/asset.rs`
    - Define key(), title(), description(), media_type() methods returning &str
    - Define roles() method returning &[String]
    - Define asset_type() method returning AssetType
    - Define raw_asset() method returning Result<Vec<u8>, CodecError>
    - Define metadata() method returning Arc<dyn MetadataProvider>
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8_

  - [x] 5.2 Implement PyAssetProvider binding
    - Create `src/bindings/asset.rs`
    - Wrap Arc<dyn AssetProvider> in PyAssetProvider struct
    - Implement all getter methods with PyO3 #[getter] attributes
    - Implement get_raw_asset() returning Python BytesIO
    - Implement get_metadata() returning PyMetadataProvider
    - _Requirements: 4.1-4.8, 11.1_

- [x] 6. Implement ImageAssetProvider trait and binding
  - [x] 6.1 Define ImageAssetProvider trait
    - Create `src/traits/image.rs`
    - Extend AssetProvider trait
    - Define has_block(block_row, block_col, resolution_level) -> bool
    - Define get_block(block_row, block_col, resolution_level, bands) -> Result<(Vec<u8>, [u32; 3]), CodecError>
    - Define property methods: num_resolution_levels, num_bands, num_rows, num_columns
    - Define block dimension methods: num_pixels_per_block_horizontal, num_pixels_per_block_vertical
    - Define bit depth methods: num_bits_per_pixel, actual_bits_per_pixel
    - Define pixel_value_type() -> PixelType and pad_pixel_value() -> f64
    - Provide default implementations for image_shape(), block_shape(), block_grid_size()
    - _Requirements: 5.1-5.12_

  - [x] 6.2 Implement PyImageAssetProvider binding
    - Add PyImageAssetProvider struct wrapping Arc<dyn ImageAssetProvider>
    - Implement has_block() method
    - Implement get_block() method returning numpy ndarray via PyO3
    - Implement all property getters
    - Implement image_shape, block_shape, block_grid_size as tuple properties
    - _Requirements: 5.1-5.12, 11.1, 11.2_

  - [ ]* 6.3 Write property test for image shape consistency
    - **Property 6: Image Shape Consistency**
    - Generate random image dimensions and block sizes
    - Verify image_shape equals (num_rows, num_columns, num_bands)
    - Verify block_shape equals (block_height, block_width, num_bands)
    - Verify block_grid_size is correctly computed
    - **Validates: Requirements 5.10, 5.11, 5.12**

  - [ ]* 6.4 Write property test for image property invariants
    - **Property 7: Image Property Invariants**
    - Generate random ImageAssetProvider configurations
    - Verify num_resolution_levels >= 1
    - Verify num_bands >= 1
    - Verify all dimension properties > 0
    - **Validates: Requirements 5.3, 5.4, 5.5, 5.6, 5.7**

  - [ ]* 6.5 Write property test for block coordinate validation
    - **Property 8: Block Coordinate Validation**
    - Generate out-of-bounds block coordinates
    - Verify get_block raises IndexError
    - **Validates: Requirements 5.13**

  - [ ]* 6.6 Write property test for resolution level validation
    - **Property 9: Resolution Level Validation**
    - Generate invalid resolution levels (>= num_resolution_levels)
    - Verify get_block raises ValueError
    - **Validates: Requirements 5.14**

- [x] 7. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Implement specialized asset provider traits
  - [x] 8.1 Define TextAssetProvider trait
    - Create `src/traits/text.rs`
    - Extend AssetProvider trait
    - Define text() -> Result<String, CodecError>
    - Define encoding() -> &str
    - Define format() -> &str
    - _Requirements: 7.1, 7.2, 7.3_

  - [x] 8.2 Define DataAssetProvider trait
    - Create `src/traits/data.rs`
    - Extend AssetProvider trait
    - Define mime_type() -> &str
    - Define parse_as_xml() -> Result<String, CodecError>
    - Define parse_as_json() -> Result<serde_json::Value, CodecError>
    - _Requirements: 8.1, 8.2, 8.3_

  - [x] 8.3 Define GraphicsAssetProvider trait
    - Create `src/traits/graphics.rs`
    - Extend AssetProvider trait
    - Document that raw_asset() provides graphics data access
    - _Requirements: 9.1, 9.2_

  - [x] 8.4 Implement Python bindings for specialized providers
    - Add PyTextAssetProvider with get_text(), get_encoding(), get_format()
    - Add PyDataAssetProvider with get_mime_type(), parse_as_xml(), parse_as_json()
    - Add PyGraphicsAssetProvider extending PyAssetProvider
    - _Requirements: 7.1-7.3, 8.1-8.3, 9.1-9.2, 11.1_

  - [ ]* 8.5 Write property test for parse error handling
    - **Property 12: Parse Error Handling**
    - Create DataAssetProvider with non-XML content
    - Verify parse_as_xml() raises ParseError
    - Create DataAssetProvider with non-JSON content
    - Verify parse_as_json() raises ParseError
    - **Validates: Requirements 8.4, 8.5**

- [x] 9. Implement DatasetReader trait and binding
  - [x] 9.1 Define DatasetReader trait
    - Create `src/traits/reader.rs`
    - Define get_asset(key: &str) -> Result<Arc<dyn AssetProvider>, CodecError>
    - Define get_asset_keys(asset_type, roles) -> Vec<String>
    - Define has_asset(key: &str) -> bool
    - Define metadata() -> Arc<dyn MetadataProvider>
    - Define close(&mut self) -> Result<(), CodecError>
    - Add Send + Sync bounds
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.6_

  - [x] 9.2 Implement PyDatasetReader binding
    - Create `src/bindings/reader.rs`
    - Wrap Option<Box<dyn DatasetReader>> for ownership management
    - Implement get_asset() returning appropriate Python wrapper based on asset type
    - Implement get_asset_keys() with optional filters
    - Implement has_asset()
    - Implement get_metadata() returning PyMetadataProvider
    - Implement close()
    - Implement __enter__ and __exit__ for context manager
    - _Requirements: 1.1-1.7, 1.5, 11.1, 11.3_

  - [ ]* 9.3 Write property test for asset key consistency
    - **Property 1: Asset Key Consistency**
    - Create mock DatasetReader with random assets
    - For all keys from get_asset_keys(), verify has_asset() returns true
    - For all keys from get_asset_keys(), verify get_asset() succeeds
    - **Validates: Requirements 1.2, 1.3**

  - [ ]* 9.4 Write property test for asset type filtering
    - **Property 2: Asset Type Filtering**
    - Create mock DatasetReader with mixed asset types
    - For each AssetType, verify get_asset_keys(asset_type=T) returns only matching assets
    - **Validates: Requirements 1.2, 10.2**

  - [ ]* 9.5 Write property test for non-existent key error
    - **Property 3: Non-Existent Key Error**
    - Create mock DatasetReader
    - Generate random keys not in dataset
    - Verify get_asset() raises KeyError
    - **Validates: Requirements 1.7**

- [x] 10. Implement DatasetWriter trait and binding
  - [x] 10.1 Define DatasetWriter trait
    - Create `src/traits/writer.rs`
    - Define add_asset(key, provider, title, description, roles) -> Result<(), CodecError>
    - Define set_metadata(metadata) -> Result<(), CodecError>
    - Define close(&mut self) -> Result<(), CodecError>
    - Add Send + Sync bounds
    - _Requirements: 2.1, 2.2, 2.4_

  - [x] 10.2 Implement PyDatasetWriter binding
    - Create `src/bindings/writer.rs`
    - Wrap Option<Box<dyn DatasetWriter>> for ownership management
    - Implement add_asset() accepting Python AssetProvider
    - Implement set_metadata() accepting PyMetadataProvider
    - Implement close()
    - Implement __enter__ and __exit__ for context manager
    - _Requirements: 2.1-2.6, 2.3, 11.1, 11.3_

  - [ ]* 10.3 Write property test for duplicate key error
    - **Property 4: Duplicate Key Error**
    - Create mock DatasetWriter
    - Add asset with random key
    - Attempt to add another asset with same key
    - Verify ValueError is raised
    - **Validates: Requirements 2.5**

- [x] 11. Implement IO Factory
  - [x] 11.1 Implement IO factory class
    - Create `src/bindings/io.rs`
    - Implement IO struct with #[pyclass]
    - Implement open(uri: &str, mode: &str) static method
    - Parse URI scheme (file://, s3://, plain path)
    - Return PyDatasetReader for mode "r"
    - Return PyDatasetWriter for mode "w"
    - Raise UnsupportedFormatError for unknown formats
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

  - [ ]* 11.2 Write property test for mode dispatch
    - **Property 5: IO Factory Mode Dispatch**
    - For mode "r", verify returned object is DatasetReader
    - For mode "w", verify returned object is DatasetWriter
    - **Validates: Requirements 3.2, 3.3**

- [x] 12. Wire up Python module and exports
  - [x] 12.1 Update lib.rs with all exports
    - Register all #[pyclass] types with the module
    - Export IO, AssetType, PixelType
    - Export PyDatasetReader, PyDatasetWriter
    - Export PyAssetProvider, PyImageAssetProvider
    - Export PyTextAssetProvider, PyDataAssetProvider, PyGraphicsAssetProvider
    - Export PyMetadataProvider
    - _Requirements: 11.1_

  - [x] 12.2 Update Python __init__.py with public API
    - Import and re-export all public classes
    - Add `open` function as alias for IO.open
    - Update __all__ list
    - _Requirements: 11.1_

- [x] 13. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional property-based tests that can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests should use `proptest` crate in Rust and `hypothesis` in Python
- Checkpoints ensure incremental validation throughout implementation
