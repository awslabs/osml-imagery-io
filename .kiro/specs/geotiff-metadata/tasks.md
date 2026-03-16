# Implementation Plan: GeoTIFF Metadata

## Overview

Implement GeoTIFF metadata parsing and writing for osml-imagery-io. The work extends five existing modules (`ffi.rs`, `tags.rs`, `metadata.rs`, `writer.rs`, `mod.rs`) and adds one new module (`geotiff.rs`). The approach is bottom-up: constants first, then FFI extensions, then the core parser/builder, then integration into the reader and writer, and finally property-based and unit tests. Rust and Python are both used as defined in the tech stack.

## Tasks

- [ ] 1. Add GeoTIFF tag constants to `src/tiff/tags.rs`
    - Add `GEO_KEY_DIRECTORY_TAG` (34735), `GEO_DOUBLE_PARAMS_TAG` (34736), `GEO_ASCII_PARAMS_TAG` (34737), `MODEL_TIEPOINT_TAG` (33922), `MODEL_PIXEL_SCALE_TAG` (33550), `MODEL_TRANSFORMATION_TAG` (34264)
    - Add GeoKey ID constants: `GT_MODEL_TYPE_GEO_KEY` (1024), `GT_RASTER_TYPE_GEO_KEY` (1025), `GEOGRAPHIC_TYPE_GEO_KEY` (2048), `PROJECTED_CS_TYPE_GEO_KEY` (3072)
    - Add value constants: `MODEL_TYPE_PROJECTED` (1), `MODEL_TYPE_GEOGRAPHIC` (2), `RASTER_PIXEL_IS_AREA` (1), `RASTER_PIXEL_IS_POINT` (2)
    - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [ ] 2. Extend TiffHandle FFI with array tag access
  - [ ] 2.1 Implement `get_field_u16_array(tag, count)` and `get_field_f64_array(tag, count)` on `TiffHandle` in `src/tiff/ffi.rs`
    - Use `TIFFGetField` with count+pointer semantics for variable-length SHORT and DOUBLE arrays
    - Copy data from libtiff-owned pointer into a `Vec` before returning
    - Return `CodecError::Decode` if the tag is not present
    - _Requirements: 1.1, 1.2, 1.6_

  - [ ] 2.2 Implement `set_field_u16_array(tag, data)`, `set_field_f64_array(tag, data)`, and `set_field_string(tag, value)` on `TiffHandle` in `src/tiff/ffi.rs`
    - Use `TIFFSetField` with count+pointer semantics for array writes
    - Use `TIFFSetField` with CString for ASCII string writes
    - _Requirements: 1.3, 1.4, 1.5_

  - [ ] 2.3 Write unit tests for FFI array tag methods in `src/tiff/ffi.rs`
    - Test write and read back of u16 array, f64 array, and string tags
    - Test reading a missing array tag returns `CodecError::Decode`
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [ ] 3. Checkpoint - Verify FFI extensions compile and pass tests
  - Ensure `cargo test` passes for the new FFI methods, ask the user if questions arise.

- [ ] 4. Implement GeoTIFF parser module (`src/tiff/geotiff.rs`)
  - [ ] 4.1 Register the `geotiff` module in `src/tiff/mod.rs`
    - Add `mod geotiff;` declaration so the new module is compiled
    - _Requirements: 3.1 (prerequisite)_

  - [ ] 4.2 Implement `parse_geokeys(directory, double_params, ascii_params)` function
    - Validate directory length ≥ 4 (header), return `CodecError::Decode` if too short
    - Parse NumberOfKeys from header, iterate key entries (4 SHORTs each)
    - Resolve inline SHORT values (TIFFTagLocation=0), double params (TIFFTagLocation=34736), and ASCII params (TIFFTagLocation=34737, strip trailing `|`)
    - Return `CodecError::Decode` if a key references a missing params tag or out-of-bounds offset
    - Map known GeoKeys: 1024→`"GeoModelType"` (Projected/Geographic), 1025→`"GeoRasterType"` (PixelIsArea/PixelIsPoint), 2048→`"GeoGeographicCRS"` (number), 3072→`"GeoProjectedCRS"` (number)
    - Map unknown GeoKeys to `"GeoKey_{KeyID}"` with raw value (number or string)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_

  - [ ] 4.3 Implement `parse_transformation_tags(pixel_scale, tiepoints, transformation)` function
    - Produce `"GeoPixelScale"` as `Value::Array` of 3 numbers from ModelPixelScaleTag
    - Produce `"GeoTiepoints"` as `Value::Array` of 6-element arrays from ModelTiepointTag; return `CodecError::Decode` if length is not a multiple of 6
    - Produce `"GeoTransformation"` as `Value::Array` of 16 numbers from ModelTransformationTag
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 9.1, 9.2, 9.3_

  - [ ] 4.4 Implement `build_geokey_directory(metadata)` function
    - Parse `"Geo"`-prefixed metadata fields back into GeoKey entries
    - Reverse-map `"GeoModelType"`, `"GeoRasterType"`, `"GeoProjectedCRS"`, `"GeoGeographicCRS"` to their GeoKey IDs and values
    - Assemble the u16 directory array with header (version=1, revision=1, minor=1, count=N)
    - Build optional double_params and ascii_params arrays for non-inline values
    - Return `CodecError::Encode` for invalid EPSG codes (not convertible to u16)
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.8, 7.10_

  - [ ] 4.5 Implement `extract_transformation_tags(metadata)` function
    - Extract `"GeoPixelScale"` → 3-element f64 vec; return `CodecError::Encode` if not 3 numbers
    - Extract `"GeoTiepoints"` → flattened f64 vec from nested arrays; return `CodecError::Encode` if not arrays of 6
    - Extract `"GeoTransformation"` → 16-element f64 vec; return `CodecError::Encode` if not 16 numbers
    - _Requirements: 7.5, 7.6, 7.7, 7.11_

  - [ ] 4.6 Write Rust unit tests for `parse_geokeys` in `src/tiff/geotiff.rs`
    - Test minimal directory with inline SHORT values only
    - Test directory with double and ASCII parameter references
    - Test GTModelTypeGeoKey mapping for values 1 and 2
    - Test GTRasterTypeGeoKey mapping for values 1 and 2
    - Test ProjectedCSTypeGeoKey and GeographicTypeGeoKey produce numeric values
    - Test unmapped GeoKey IDs produce `"GeoKey_{KeyID}"` format
    - Test malformed directory (length < 4) returns error
    - Test missing double params tag returns error
    - Test missing ASCII params tag returns error
    - _Requirements: 3.1–3.9, 4.1–4.7_

  - [ ] 4.7 Write Rust unit tests for `parse_transformation_tags` in `src/tiff/geotiff.rs`
    - Test ModelPixelScaleTag produces 3-element array
    - Test ModelTiepointTag with non-multiple-of-6 length returns error
    - Test ModelTransformationTag produces 16-element array
    - _Requirements: 5.1–5.4, 9.1–9.3_

- [ ] 5. Checkpoint - Verify geotiff parser module compiles and passes tests
  - Ensure `cargo test` passes for the geotiff module, ask the user if questions arise.

- [ ] 6. Integrate GeoTIFF metadata into the reader path
  - [ ] 6.1 Extend `TIFFMetadataProvider::from_handle()` in `src/tiff/metadata.rs`
    - After reading standard TIFF tags, attempt to read tag 34735 via `get_field_u16_array`
    - If present, read optional tags 34736 and 34737, call `geotiff::parse_geokeys`, merge results into `tags`
    - Read optional transformation tags (33550, 33922, 34264) via `get_field_f64_array`, call `geotiff::parse_transformation_tags` if any are present, merge results
    - If tag 34735 is absent, produce no Geo-prefixed fields (plain TIFF behavior)
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [ ] 6.2 Update `as_dict` section filtering in `src/tiff/metadata.rs`
    - `as_dict(None)` returns all fields (unchanged behavior)
    - Any `Some(prefix)` filters by `starts_with(prefix)`, e.g. `as_dict(Some("Geo"))` returns only Geo-prefixed fields
    - _Requirements: 6.1, 6.2_

  - [ ] 6.3 Write Rust unit tests for GeoTIFF metadata integration in `src/tiff/metadata.rs`
    - Test that a plain TIFF (no tag 34735) produces no Geo-prefixed keys
    - Test `as_dict(Some("Geo"))` returns only GeoTIFF fields
    - Test `as_dict(None)` returns both TIFF and GeoTIFF fields
    - _Requirements: 6.1, 6.2, 6.3_

- [ ] 7. Integrate GeoTIFF metadata into the writer path
  - [ ] 7.1 Extend `TIFFDatasetWriter` in `src/tiff/writer.rs` to write GeoTIFF tags
    - After writing standard TIFF tags, check metadata for Geo-prefixed encoding hints
    - Call `geotiff::build_geokey_directory` to assemble tag 34735, optional 34736, optional 34737
    - Call `geotiff::extract_transformation_tags` to get values for tags 33550, 33922, 34264
    - Write all assembled tags via the new FFI set methods
    - If no Geo-prefixed hints are present, write no GeoTIFF tags (plain TIFF)
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9_

  - [ ] 7.2 Write Rust unit tests for GeoTIFF writer integration in `src/tiff/writer.rs`
    - Test that encoding hints with GeoModelType, GeoProjectedCRS, GeoPixelScale produce a valid GeoTIFF
    - Test that no Geo hints produces a plain TIFF with no GeoTIFF tags
    - Test invalid EPSG code returns `CodecError::Encode`
    - Test invalid GeoPixelScale format returns `CodecError::Encode`
    - _Requirements: 7.1–7.11_

- [ ] 8. Checkpoint - Verify reader and writer integration compiles and passes tests
  - Ensure `cargo test` passes for all modified modules, ask the user if questions arise.

- [ ] 9. Add Python-level tests for GeoTIFF metadata
  - [ ] 9.1 Add GeoTIFF hypothesis strategies to `tests/property/strategies.py`
    - `geotiff_model_type()` — draws from `["Projected", "Geographic"]`
    - `geotiff_raster_type()` — draws from `["PixelIsArea", "PixelIsPoint"]`
    - `epsg_codes()` — draws valid u16 EPSG codes (1–32767)
    - `pixel_scale()` — draws 3-element arrays of positive floats
    - `tiepoint_tuples()` — draws lists of 1–4 tiepoint 6-element float arrays
    - `transformation_matrix()` — draws 16-element float arrays
    - `geotiff_metadata()` — composite strategy combining the above
    - _Requirements: 8.1, 8.2, 8.3, 8.4 (test infrastructure)_

  - [ ] 9.2 Write Python property test: GeoTIFF metadata write-read round-trip in `tests/property/test_geotiff_roundtrip.py`
    - **Property 1: GeoTIFF metadata write-read round-trip**
    - Use `geotiff_metadata()` strategy to generate random valid GeoTIFF encoding hints
    - Write a GeoTIFF via `TIFFDatasetWriter` with the hints, read back via `TIFFDatasetReader`
    - Verify all Geo-prefixed metadata fields have identical `serde_json::Value` representations
    - Mark with `@pytest.mark.property`
    - **Validates: Requirements 1.1–1.5, 3.1–3.5, 4.1–4.7, 5.1–5.3, 6.1–6.4, 7.1–7.8, 8.1–8.4, 9.1–9.6**

  - [ ] 9.3 Write Python property test: Idempotent GeoTIFF encoding in `tests/property/test_geotiff_roundtrip.py`
    - **Property 2: Idempotent GeoTIFF encoding**
    - Write GeoTIFF, read metadata, write again with read metadata as hints, read again
    - Verify second read produces identical metadata to first read
    - Mark with `@pytest.mark.property`
    - **Validates: Requirements 8.5**

  - [ ]* 9.4 Write Python unit tests for GeoTIFF metadata in `tests/test_tiff_geotiff.py`
    - Test writing GeoTIFF metadata via writer, reading back, verifying all fields match
    - Test plain TIFF has no Geo-prefixed metadata fields
    - Test `as_dict("Geo")` returns only GeoTIFF fields
    - Test `as_dict(None)` returns both TIFF and GeoTIFF fields
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 10. Checkpoint - Verify all Python tests pass
  - Ensure `pytest` passes for all new test files, ask the user if questions arise.

- [ ] 11. Add integration tests for real-world GeoTIFF files
  - [ ] 11.1 Write Python integration tests in `tests/test_tiff_geotiff_integration.py`
    - Read GeoTIFF files from `data/integration/` and verify expected metadata fields
    - Verify GeoModelType, GeoProjectedCRS or GeoGeographicCRS, GeoPixelScale, GeoTiepoints are present with plausible values
    - Mark with `@pytest.mark.integration`
    - Skip gracefully when `data/integration/` is not available
    - _Requirements: 11.1, 11.2, 11.3_

- [ ] 12. Final checkpoint - Ensure all tests pass
  - Ensure `cargo test` and `pytest` both pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples and edge cases
- The existing `"tiff"` alias bug in `as_dict` is tracked separately and not addressed here
- GeoTransform computation is out of scope — only raw metadata parsing and writing
