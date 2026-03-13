# Implementation Plan: libtiff FFI Bindings and Basic TIFF Reading

## Overview

This plan implements TIFF format reading by adding a `src/tiff/` module mirroring the `src/jbp/` architecture. It layers from raw FFI declarations through safe wrappers, trait implementations, and IO factory integration. Each task builds incrementally so the module compiles and tests pass at every step. The implementation language is Rust with Python bindings via PyO3.

## Tasks

- [ ] 1. Build system and feature flag setup
  - [ ] 1.1 Add `libtiff` feature flag to `Cargo.toml`
    - Add `libtiff = []` to `[features]` section
    - Add `"libtiff"` to the `default` feature list
    - _Requirements: 7.5, 13.1_

  - [ ] 1.2 Add `configure_libtiff()` to `build.rs`
    - Add `#[cfg(feature = "libtiff")]` block in `main()` calling `configure_libtiff()`
    - Implement `configure_libtiff()` following the `configure_openjpeg()` pattern: try pkg-config (`libtiff-4`), then conda (`$CONDA_PREFIX/lib/libtiff.{dylib,so}`), then system paths
    - Emit `cargo:rustc-link-lib=tiff` for dynamic linking
    - Print warning with install instructions for macOS, Ubuntu, Fedora if not found
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.6_

  - [ ] 1.3 Create `src/tiff/mod.rs` with feature gate and empty submodules
    - Create `src/tiff/mod.rs` gated behind `#[cfg(feature = "libtiff")]`
    - Declare submodules: `sys`, `ffi`, `tags`, `image`, `reader`, `metadata`
    - Add `#[cfg(feature = "libtiff")] pub mod tiff;` to `src/lib.rs`
    - Create empty stub files for each submodule so the crate compiles
    - _Requirements: 13.1, 13.2, 13.3_

- [ ] 2. Checkpoint - Verify build compiles with libtiff feature
  - Ensure `cargo test` passes with the new feature flag and empty module stubs, ask the user if questions arise.

- [ ] 3. Raw FFI declarations and TIFF tag constants
  - [ ] 3.1 Implement `src/tiff/sys.rs` with extern "C" declarations
    - Declare `extern "C"` bindings for: `TIFFClientOpen`, `TIFFClose`, `TIFFGetField`, `TIFFSetField`, `TIFFReadEncodedTile`, `TIFFWriteEncodedTile`, `TIFFTileSize`, `TIFFNumberOfTiles`, `TIFFSetDirectory`, `TIFFCurrentDirectory`, `TIFFNumberOfDirectories`, `TIFFReadTile`, `TIFFWriteTile`, `TIFFIsTiled`, `TIFFStripSize`, `TIFFNumberOfStrips`, `TIFFReadEncodedStrip`
    - Declare `TIFFErrorHandler` type and `TIFFSetErrorHandler` / `TIFFSetWarningHandler`
    - Declare callback function pointer types: `TIFFReadWriteProc`, `TIFFSeekProc`, `TIFFCloseProc`, `TIFFSizeProc`, `TIFFMapFileProc`, `TIFFUnmapFileProc`
    - Use `*mut c_void` as the opaque TIFF handle type
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

  - [ ] 3.2 Implement `src/tiff/tags.rs` with TIFF tag and format constants
    - Define standard TIFF tag constants: `NEW_SUBFILE_TYPE`, `IMAGE_WIDTH`, `IMAGE_LENGTH`, `BITS_PER_SAMPLE`, `SAMPLES_PER_PIXEL`, `COMPRESSION`, `PHOTOMETRIC_INTERPRETATION`, `TILE_WIDTH`, `TILE_LENGTH`, `SAMPLE_FORMAT`, `PLANAR_CONFIGURATION`, `ROWS_PER_STRIP`, `STRIP_OFFSETS`, `STRIP_BYTE_COUNTS`, `TILE_OFFSETS`, `TILE_BYTE_COUNTS`
    - Define compression constants: `COMPRESSION_NONE` (1), `COMPRESSION_LZW` (5), `COMPRESSION_DEFLATE` (8), `COMPRESSION_PACKBITS` (32773), `COMPRESSION_ADOBE_DEFLATE` (32946)
    - Define sample format constants: `SAMPLE_FORMAT_UINT` (1), `SAMPLE_FORMAT_INT` (2), `SAMPLE_FORMAT_FLOAT` (3)
    - Define photometric constants: `PHOTOMETRIC_MINISBLACK` (1), `PHOTOMETRIC_RGB` (2), `PHOTOMETRIC_PALETTE` (3)
    - Define planar configuration constants: `PLANAR_CONFIG_CONTIG` (1), `PLANAR_CONFIG_SEPARATE` (2)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 4. Safe RAII wrapper (TiffHandle)
  - [ ] 4.1 Implement `MemoryReadStreamData` and memory callbacks in `src/tiff/ffi.rs`
    - Implement `MemoryReadStreamData` struct with data pointer, length, and position
    - Implement `tiff_read_proc`, `tiff_seek_proc` (SEEK_SET/SEEK_CUR/SEEK_END), `tiff_close_proc` (no-op), `tiff_size_proc`, `tiff_write_proc` (returns -1, read-only)
    - Follow the memory stream callback pattern from `src/jbp/j2k/ffi.rs`
    - _Requirements: 2.2, 2.7_

  - [ ] 4.2 Implement `TiffHandle` struct with `from_bytes()`, `Drop`, and error capture
    - Implement `TiffHandle` wrapping `*mut c_void` with `Drop` calling `TIFFClose`
    - Implement `from_bytes(data: &[u8]) -> Result<Self, CodecError>` using `TIFFClientOpen` with memory callbacks
    - Return `CodecError::InvalidFormat` with captured libtiff error when `TIFFClientOpen` returns null
    - Implement thread-local error/warning capture via `TIFFSetErrorHandler`/`TIFFSetWarningHandler`, following `src/jbp/j2k/ffi.rs`
    - Implement `unsafe impl Send for TiffHandle`
    - _Requirements: 2.1, 2.2, 2.5, 2.6, 2.8_

  - [ ] 4.3 Implement typed tag getters and IFD navigation on `TiffHandle`
    - Implement `get_field_u16`, `get_field_u32`, `get_field_f32`, `get_field_f64`, `get_field_string` returning `Result` types
    - Implement `set_directory(index) -> Result<()>`, `current_directory() -> u16`, `number_of_directories() -> u16`
    - _Requirements: 2.3, 2.9_

  - [ ] 4.4 Implement tile and strip I/O methods on `TiffHandle`
    - Implement `read_encoded_tile(tile_index) -> Result<Vec<u8>>` and `read_encoded_strip(strip_index) -> Result<Vec<u8>>`
    - Implement `tile_size()`, `strip_size()`, `number_of_tiles()`, `number_of_strips()`, `is_tiled()` helpers
    - _Requirements: 2.4_

  - [ ] 4.5 Write Rust unit tests for `TiffHandle`
    - Test `from_bytes` with valid TIFF data (create minimal synthetic TIFF in test)
    - Test `from_bytes` with invalid data returns `CodecError::InvalidFormat`
    - Test `from_bytes` with empty slice returns `CodecError::InvalidFormat`
    - Test IFD navigation: `set_directory`, `current_directory`, `number_of_directories`
    - Test tag getters return correct values for known tags
    - _Requirements: 2.1, 2.2, 2.3, 2.8, 2.9_

- [ ] 5. Checkpoint - Verify TiffHandle compiles and links against libtiff
  - Ensure `cargo test` passes with the FFI wrapper, ask the user if questions arise.

- [ ] 6. Metadata provider
  - [ ] 6.1 Implement `src/tiff/metadata.rs` with `TIFFMetadataProvider`
    - Implement `TIFFMetadataProvider` struct with `HashMap<String, serde_json::Value>` storage
    - Implement `MetadataProvider` trait: `as_dict(None)` returns all tags, `as_dict(Some("tiff"))` returns TIFF tags, `as_dict(Some(other))` returns empty HashMap
    - Implement `from_handle(handle, ifd_index)` constructor that reads standard TIFF tags via `TIFFGetField` and maps to named keys (`"ImageWidth"`, `"ImageLength"`, `"BitsPerSample"`, `"SamplesPerPixel"`, `"Compression"`, `"PhotometricInterpretation"`, `"PlanarConfiguration"`, `"SampleFormat"`, `"TileWidth"`, `"TileLength"`, `"RowsPerStrip"`)
    - Implement dataset-level constructor storing only `"ByteOrder"`, `"NumberOfDirectories"`, `"NumberOfImageSegments"`
    - Represent numeric values as JSON numbers and string values as JSON strings
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9_

  - [ ] 6.2 Write Rust unit tests for `TIFFMetadataProvider`
    - Test `as_dict(None)` returns all expected tags
    - Test `as_dict(Some("tiff"))` returns same result as `as_dict(None)`
    - Test `as_dict(Some("unknown"))` returns empty HashMap
    - Test dataset-level metadata contains exactly `ByteOrder`, `NumberOfDirectories`, `NumberOfImageSegments`
    - _Requirements: 6.5, 6.6, 6.7, 6.9_

- [ ] 7. Image asset provider
  - [ ] 7.1 Implement pixel format mapping in `src/tiff/image.rs`
    - Map `(SampleFormat, BitsPerSample)` to `PixelType` enum
    - Default absent `SampleFormat` to `SAMPLE_FORMAT_UINT` per TIFF 6.0 spec
    - Return `CodecError::Unsupported` for unsupported combinations
    - _Requirements: 4.8, 9.1, 9.2, 9.3, 9.4_

  - [ ] 7.2 Implement `TIFFImageAssetProvider` struct and `ImageAssetProvider` trait
    - Create struct with image dimensions, block layout, pixel type, planar config, `Arc<Mutex<TiffHandle>>`, `Arc<TIFFMetadataProvider>`
    - Implement trait methods: `key()`, `image_width()`, `image_height()`, `num_bands()`, `pixel_type()`, `num_pixels_per_block_horizontal()`, `num_pixels_per_block_vertical()`, `block_grid_size()`, `num_resolution_levels()` (returns 1), `has_block()`, `metadata()`
    - Tiled: block dims from TileWidth/TileLength, grid = `ceil(H/TileLength) x ceil(W/TileWidth)`
    - Stripped: block width = ImageWidth, block height = RowsPerStrip, grid = `(ceil(H/RowsPerStrip), 1)`
    - _Requirements: 4.1, 4.6, 4.7, 4.8, 4.9, 4.13, 11.1, 11.2_

  - [ ] 7.3 Implement `get_block()` for tiled TIFFs with chunky-to-BSQ conversion
    - Compute tile index from `(row, col)`, acquire mutex, call `TIFFSetDirectory` then `TIFFReadEncodedTile`
    - Deinterleave chunky (PlanarConfiguration=1) data from RGBRGB... to band-sequential RRR...GGG...BBB...
    - Handle planar (PlanarConfiguration=2) by reading per-band tiles and concatenating
    - Support band subsetting when `bands` is `Some(&[...])`
    - Return `CodecError::InvalidBlockCoordinates` for out-of-bounds, `CodecError::InvalidResolutionLevel` for level > 0
    - _Requirements: 4.2, 4.4, 4.5, 4.10, 4.11, 4.12, 9.5, 10.1, 10.3_

  - [ ] 7.4 Implement `get_block()` for stripped TIFFs
    - Map `(row, 0)` to strip index, call `TIFFReadEncodedStrip`
    - Apply same chunky-to-BSQ deinterleaving and band subsetting as tiled path
    - Handle last strip being shorter than RowsPerStrip
    - _Requirements: 4.3, 11.3_

  - [ ] 7.5 Write Rust unit tests for `TIFFImageAssetProvider`
    - Test pixel format mapping for all supported (SampleFormat, BitsPerSample) combinations
    - Test unsupported pixel format returns `CodecError::Unsupported`
    - Test block coordinate validation (in-bounds and out-of-bounds)
    - Test resolution level validation (level 0 ok, level > 0 error)
    - Test chunky-to-BSQ deinterleaving logic with known input/output
    - _Requirements: 4.6, 4.8, 4.11, 4.12, 9.1, 9.2, 9.3, 9.4, 9.5_

- [ ] 8. Checkpoint - Verify image provider compiles and basic tile/strip reading works
  - Ensure `cargo test` passes with the image asset provider, ask the user if questions arise.

- [ ] 9. Dataset reader
  - [ ] 9.1 Implement `TIFFDatasetReader` in `src/tiff/reader.rs`
    - Implement struct owning `Vec<u8>` data, `Arc<Mutex<TiffHandle>>`, image assets, asset keys, dataset metadata
    - Implement `from_bytes(data: &[u8])`: open via `TIFFClientOpen`, enumerate IFDs, classify by `NewSubfileType` (bit 0 = 0 -> full-res, bit 0 = 1 -> skip), create one `TIFFImageAssetProvider` per full-res IFD keyed as `image_segment_0`, `image_segment_1`, etc.
    - Special case: single-IFD file always becomes `image_segment_0` regardless of `NewSubfileType`
    - Implement `open(path)`: read file to `Vec<u8>`, delegate to `from_bytes()`
    - Detect byte order from magic bytes for dataset metadata
    - Return `CodecError::InvalidFormat` for invalid TIFF magic bytes
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.11, 5.12, 5.13_

  - [ ] 9.2 Implement `DatasetReader` trait for `TIFFDatasetReader`
    - Implement `get_asset(key)`: return provider for valid keys, `CodecError::AssetNotFound` for invalid
    - Implement `get_asset_keys(asset_type, name)`: image segment keys for `AssetType::Image`, empty for Text/Graphics/Data
    - Implement `metadata()` returning dataset-level `TIFFMetadataProvider`
    - Implement `close()` to release resources
    - _Requirements: 5.7, 5.8, 5.9, 5.10_

  - [ ] 9.3 Handle unsupported compression detection
    - During IFD enumeration, read `Compression` tag for each full-resolution IFD
    - Return `CodecError::Unsupported` with descriptive message if compression not in {1, 5, 8, 32773, 32946}
    - _Requirements: 10.2_

  - [ ]* 9.4 Write Rust unit tests for `TIFFDatasetReader`
    - Test `from_bytes` with valid single-IFD TIFF
    - Test `from_bytes` with invalid magic bytes returns `CodecError::InvalidFormat`
    - Test `from_bytes` with empty data returns `CodecError::InvalidFormat`
    - Test `get_asset` with valid and invalid keys
    - Test `get_asset_keys` for Image, Text, Graphics, Data asset types
    - Test dataset-level metadata values
    - _Requirements: 5.2, 5.7, 5.8, 5.9, 5.10, 5.11_

- [ ] 10. IO factory registration and Python bindings
  - [ ] 10.1 Register TIFF format in `src/bindings/io.rs`
    - Update `create_reader()` to detect `.tif`/`.tiff` extensions and create `TIFFDatasetReader`
    - Support explicit format strings `"tiff"` and `"tif"`
    - Return `CodecError::Unsupported` for TIFF write mode with message indicating Phase 2 scope
    - Gate TIFF branches behind `#[cfg(feature = "libtiff")]`
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 12.1, 12.2, 13.4_

  - [ ] 10.2 Write Rust unit tests for IO factory TIFF detection
    - Test `.tif` and `.tiff` extensions route to TIFF reader
    - Test explicit `"tiff"` and `"tif"` format strings route to TIFF reader
    - Test TIFF write mode returns `CodecError::Unsupported`
    - _Requirements: 8.1, 8.2, 8.3_

- [ ] 11. Checkpoint - Verify end-to-end TIFF reading through IO factory
  - Ensure `cargo test` passes with full TIFF reading pipeline, ask the user if questions arise.

- [ ] 12. Python tests
  - [ ] 12.1 Write Python unit tests in `tests/test_tiff_reader.py`
    - Test `IO.open(["image.tif"], "r")` returns a DatasetReader
    - Test reading image blocks as NumPy arrays through `get_block()`
    - Test accessing metadata through `as_dict()`
    - Test error conditions: invalid file, unsupported format string for write
    - Verify pixel values match expected data for a known synthetic TIFF
    - _Requirements: 12.1, 12.2, 12.3, 12.4_

- [ ] 13. Property-based tests
  - [ ] 13.1 Add TIFF hypothesis strategies to `tests/property/strategies.py`
    - Add `tiff_compression()`: draws from `["None", "LZW", "Deflate", "PackBits"]`
    - Add `tiff_planar_config()`: draws from `[1, 2]`
    - Add `tiff_layout()`: draws from `["tiled", "stripped"]`
    - Add `tiff_image_config()` composite strategy combining pixel type, dimensions, bands, compression, layout, planar config

  - [ ] 13.2 Write property test for pixel data roundtrip in `tests/property/test_tiff_roundtrip.py`
    - **Property 1: Pixel data roundtrip**
    - Write TIFF with known pixel data, read back through `get_block()`, verify byte-identical CHW output
    - Generate across pixel types, band counts (1-4), layouts, planar configs, compressions
    - **Validates: Requirements 4.2, 4.3, 4.4, 4.5, 4.8, 4.9, 9.1, 9.2, 9.3, 9.5, 10.1, 11.3**

  - [ ] 13.3 Write property test for band subsetting in `tests/property/test_tiff_roundtrip.py`
    - **Property 2: Band subsetting preserves correct data**
    - For multi-band TIFFs, verify `get_block()` with band subset matches corresponding bands from full read
    - **Validates: Requirements 4.10**

  - [ ] 13.4 Write property test for stripped TIFF block dimensions in `tests/property/test_tiff_roundtrip.py`
    - **Property 10: Stripped TIFF block dimensions**
    - Verify `num_pixels_per_block_horizontal() == ImageWidth`, `num_pixels_per_block_vertical() == RowsPerStrip`, `block_grid_size() == (ceil(ImageLength / RowsPerStrip), 1)`
    - **Validates: Requirements 11.1, 11.2**

  - [ ] 13.5 Write property test for block coordinate validation in `tests/property/test_tiff_api.py`
    - **Property 3: Block coordinate validation**
    - Verify `has_block()` returns true for all valid coordinates, `get_block()` returns `InvalidBlockCoordinates` for out-of-bounds
    - **Validates: Requirements 4.6, 4.11**

  - [ ] 13.6 Write property test for IFD enumeration in `tests/property/test_tiff_api.py`
    - **Property 4: IFD enumeration and asset key consistency**
    - For TIFFs with N full-res IFDs, verify `get_asset_keys` returns N keys, each `get_asset(key)` succeeds, each provider reports `num_resolution_levels() == 1`
    - **Validates: Requirements 4.7, 5.4, 5.5, 5.7, 5.9**

  - [ ] 13.7 Write property test for non-image asset access in `tests/property/test_tiff_api.py`
    - **Property 5: Non-image asset access**
    - Verify `get_asset()` with invalid keys returns `AssetNotFound`, `get_asset_keys` for Text/Graphics/Data returns empty
    - **Validates: Requirements 5.8, 5.10**

  - [ ] 13.8 Write property test for dataset-level metadata in `tests/property/test_tiff_api.py`
    - **Property 6: Dataset-level metadata contains only file-level information**
    - Verify dataset metadata has exactly `ByteOrder`, `NumberOfDirectories`, `NumberOfImageSegments`
    - **Validates: Requirements 5.6, 6.9**

  - [ ] 13.9 Write property test for per-IFD metadata completeness in `tests/property/test_tiff_api.py`
    - **Property 7: Per-IFD metadata completeness**
    - Verify per-segment metadata contains `ImageWidth`, `ImageLength`, `BitsPerSample`, `SamplesPerPixel` with correct values; `as_dict(None)` and `as_dict(Some("tiff"))` return identical results
    - **Validates: Requirements 4.13, 6.2, 6.3, 6.4, 6.5, 6.6, 6.8**

  - [ ]* 13.10 Write property test for unrecognized metadata section in `tests/property/test_tiff_api.py`
    - **Property 8: Unrecognized metadata section returns empty**
    - For any section name not equal to `"tiff"`, verify `as_dict(name)` returns empty dict
    - **Validates: Requirements 6.7**

  - [ ]* 13.11 Write property test for invalid format rejection in `tests/property/test_tiff_format.py`
    - **Property 9: Invalid format rejection**
    - For any byte sequence not starting with `II*\0` or `MM\0*`, verify `from_bytes()` returns `InvalidFormat`
    - **Validates: Requirements 5.11, 8.4**

  - [ ] 13.12 Update `tests/property/test_io_contracts.py` for TIFF format detection
    - Add TIFF extensions (`.tif`, `.tiff`) and format strings (`"tiff"`, `"tif"`) to IO contract tests
    - Verify IO factory routes TIFF files to the correct reader type
    - _Requirements: 8.1, 8.2_

- [ ] 14. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass (`cargo test` and `pytest`), ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at each layer boundary
- Property tests validate the 10 correctness properties from the design document
- Unit tests validate specific examples, edge cases, and error conditions
- The implementation language is Rust with Python bindings (PyO3), matching the existing codebase
