# Implementation Plan: Image Segment Structure

## Overview

This implementation plan covers Phase 4 of the JBP project: Image Segment Structure. The plan implements parsing, validation, and writing of image subheaders, along with reading and writing uncompressed imagery. The implementation uses a facade pattern over StructureAccessor and a strategy pattern for block decoders to support future compression formats.

## Tasks

- [x] 1. Create core image types and enums
  - [x] 1.1 Create `src/jbp/image/mod.rs` with module structure
    - Create module file exposing submodules: types, facade, decoder, pixel, interleave, validation
    - Add module to `src/jbp/mod.rs`
    - _Requirements: 1.6, 1.7, 1.10, 2.5_
  
  - [x] 1.2 Implement `PixelValueType` enum in `src/jbp/image/types.rs`
    - Implement variants: UnsignedInt, SignedInt, Real, Complex, BiLevel
    - Implement `from_str()` parsing from PVTYPE field
    - Implement `to_str()` for writing
    - Implement `to_pixel_type(nbpp)` conversion to `PixelType`
    - _Requirements: 1.6, 11.1-11.10_
  
  - [x] 1.3 Implement `ImageRepresentation` enum
    - Implement variants: Mono, Rgb, RgbLut, Multi, NoDisplay, NVector, Polar, Vph, YCbCr601
    - Implement `from_str()` and `to_str()`
    - Implement `expected_band_count()` returning Option<usize>
    - _Requirements: 1.7, 15.1-15.4_
  
  - [x] 1.4 Implement `InterleaveMode` enum
    - Implement variants: B, P, R, S
    - Implement `from_char()` and `to_char()`
    - _Requirements: 2.5, 5.3-5.6_
  
  - [x] 1.5 Implement `PixelJustification` enum
    - Implement variants: Right, Left
    - Implement `from_char()` and `to_char()`
    - _Requirements: 1.10_
  
  - [x] 1.6 Write unit tests for type conversions
    - Test all enum parsing and serialization
    - Test edge cases and invalid inputs
    - _Requirements: 1.6, 1.7, 1.10, 2.5_

- [x] 2. Implement ImageSubheaderFacade
  - [x] 2.1 Create `ImageSubheaderFacade` struct in `src/jbp/image/facade.rs`
    - Wrap `StructureAccessor` with lifetime parameter
    - Implement `new()` and `from_bytes()` constructors
    - Implement `accessor()` method for direct access
    - _Requirements: 1.1-1.10_
  
  - [x] 2.2 Implement identification field accessors
    - Implement `iid1()`, `iid2()`, `idatim()`, `tgtid()`, `isorce()`
    - Parse string fields from accessor
    - _Requirements: 1.2, 1.3, 1.4_
  
  - [x] 2.3 Implement dimension and pixel field accessors
    - Implement `nrows()`, `ncols()` with u32 parsing
    - Implement `pvtype()`, `irep()`, `icat()`, `abpp()`, `nbpp()`, `pjust()`
    - _Requirements: 1.5, 1.6, 1.7, 1.8, 1.9, 1.10_
  
  - [x] 2.4 Implement blocking parameter accessors
    - Implement `nbpr()`, `nbpc()`, `nppbh()`, `nppbv()`, `imode()`
    - _Requirements: 2.1-2.5_
  
  - [x] 2.5 Implement band information accessors
    - Implement `band_count()` handling NBANDS/XBANDS logic
    - Implement `band_info(index)` returning `BandInfoFacade`
    - _Requirements: 3.1, 3.2_
  
  - [x] 2.6 Implement compression field accessors
    - Implement `ic()` and `comrat()`
    - Implement `is_uncompressed()` helper
    - _Requirements: 5.1, 5.2_
  
  - [x] 2.7 Implement computed helper methods
    - Implement `bytes_per_pixel()`, `block_size_bytes()`, `image_data_size()`
    - _Requirements: 8.1-8.8_
  
  - [x] 2.8 Write property test for facade field access
    - **Property 1: Image Subheader Round-Trip**
    - Generate random valid subheader configurations
    - Write using StructureWriter, read using facade, verify field equality
    - **Validates: Requirements 1.1-1.10, 2.1-2.5, 7.1-7.8, 17.1**

- [x] 3. Implement BandInfoFacade and LUT support
  - [x] 3.1 Create `BandInfoFacade` struct
    - Wrap accessor reference with band index
    - Handle both `band_info` and `band_info_extended` paths
    - _Requirements: 3.3-3.6_
  
  - [x] 3.2 Implement band field accessors
    - Implement `irepband()`, `isubcat()`, `ifc()`, `imflt()`
    - Implement `nluts()`, `nelut()`, `lut_data(lut_index)`
    - _Requirements: 3.3-3.9_
  
  - [x] 3.3 Implement `LookUpTable` struct
    - Create struct with `entries: Vec<u8>`
    - Implement `from_bytes()`, `apply()`, `len()`
    - _Requirements: 4.1-4.4_
  
  - [x] 3.4 Write property test for band info round-trip
    - **Property 2: Band Information Round-Trip**
    - Generate images with 1-9 bands and 10+ bands (XBANDS)
    - Verify all band fields preserved through write/read cycle
    - **Validates: Requirements 3.1-3.9, 9.1-9.9**
  
  - [x] 3.5 Write property test for LUT round-trip
    - **Property 3: LUT Data Round-Trip**
    - Generate valid LUT configurations (1-4 LUTs per band)
    - Verify byte-identical LUT data after write/read
    - **Validates: Requirements 4.1, 4.2, 4.5**

- [x] 4. Checkpoint - Verify facade implementation
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement pixel module for uncompressed I/O
  - [x] 5.1 Create `src/jbp/image/pixel.rs` module
    - Define module structure with encode/decode functions
    - _Requirements: 5.7-5.12, 11.1-11.10_
  
  - [x] 5.2 Implement `bytes_per_pixel()` function
    - Calculate bytes from PVTYPE and NBPP
    - Handle all PVTYPE/NBPP combinations
    - _Requirements: 11.1-11.10_
  
  - [x] 5.3 Implement `decode_pixel()` for integer types
    - Handle INT (unsigned) with NBPP 8, 16, 32
    - Handle SI (signed) with NBPP 8, 16, 32
    - Handle big-endian byte order
    - _Requirements: 5.8, 5.9, 11.1-11.6_
  
  - [x] 5.4 Implement `decode_pixel()` for float and complex types
    - Handle R (real) with NBPP 32, 64
    - Handle C (complex) with NBPP 64 (two 32-bit floats)
    - _Requirements: 5.10, 5.11, 11.7-11.9_
  
  - [x] 5.5 Implement `decode_pixel()` for bi-level type
    - Handle B (bi-level) with NBPP 1
    - Unpack bits from bytes
    - _Requirements: 5.12, 11.10_
  
  - [x] 5.6 Implement `encode_pixel()` functions
    - Mirror decode functions for all PVTYPE/NBPP combinations
    - Handle pixel justification (PJUST)
    - _Requirements: 10.6, 11.1-11.10_
  
  - [x] 5.7 Implement bulk `decode()` and `encode()` functions
    - Process arrays of pixels efficiently
    - Handle ABPP vs NBPP bit shifting
    - _Requirements: 5.7-5.12_
  
  - [x] 5.8 Write property test for pixel value round-trip
    - **Property 5: Pixel Value Type Round-Trip**
    - Generate random pixel values for each PVTYPE/NBPP combination
    - Verify encode then decode produces equivalent values
    - **Validates: Requirements 5.7-5.12, 11.1-11.10**

- [x] 6. Implement interleave module for uncompressed I/O
  - [x] 6.1 Create `src/jbp/image/interleave.rs` module
    - Define conversion function signatures
    - _Requirements: 12.1-12.5_
  
  - [x] 6.2 Implement `to_band_sequential()` from IMODE B
    - Convert band-interleaved-by-block to band-sequential
    - _Requirements: 5.3, 12.1_
  
  - [x] 6.3 Implement `to_band_sequential()` from IMODE P
    - Convert band-interleaved-by-pixel to band-sequential
    - _Requirements: 5.4, 12.2_
  
  - [x] 6.4 Implement `to_band_sequential()` from IMODE R
    - Convert band-interleaved-by-row to band-sequential
    - _Requirements: 5.5, 12.3_
  
  - [x] 6.5 Implement `to_band_sequential()` from IMODE S
    - Handle band-sequential (may be no-op or reordering)
    - _Requirements: 5.6, 12.4_
  
  - [x] 6.6 Implement `from_band_sequential()` to all modes
    - Convert band-sequential to target IMODE
    - _Requirements: 10.2-10.5_
  
  - [x] 6.7 Implement `convert()` function
    - Combine to_band_sequential and from_band_sequential
    - Optimize for same-mode case (no-op)
    - _Requirements: 12.1-12.5_
  
  - [x] 6.8 Write property test for interleave conversion
    - **Property 9: Interleave Conversion Preserves Pixel Values**
    - Generate random image data
    - Convert from source mode to target mode and back
    - Verify byte-identical output
    - **Validates: Requirements 12.1-12.5**

- [x] 7. Checkpoint - Verify pixel and interleave modules
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Implement BlockDecoder trait and UncompressedBlockDecoder
  - [x] 8.1 Create `src/jbp/image/decoder.rs` module
    - Define `BlockDecoder` trait with methods: decode_block, has_block, compression_type, num_resolution_levels
    - Define `create_block_decoder()` factory function
    - _Requirements: 6.1-6.5_
  
  - [x] 8.2 Implement `UncompressedBlockDecoder` struct
    - Store image parameters extracted from facade
    - Store image data as `Arc<[u8]>`
    - _Requirements: 5.1, 5.2_
  
  - [x] 8.3 Implement block offset calculation
    - Calculate byte offset for block based on IMODE
    - Handle all four interleave modes
    - _Requirements: 5.3-5.6_
  
  - [x] 8.4 Implement `decode_block()` for UncompressedBlockDecoder
    - Read raw block data at calculated offset
    - Convert to band-sequential format
    - Apply band selection if specified
    - Handle edge blocks (partial blocks at image boundaries)
    - Return (data, [rows, cols, bands])
    - _Requirements: 6.1, 6.2, 6.4, 6.5_
  
  - [x] 8.5 Implement `has_block()` for UncompressedBlockDecoder
    - Validate block coordinates against NBPR, NBPC
    - _Requirements: 6.3_
  
  - [x] 8.6 Implement `create_block_decoder()` factory
    - Check IC field from facade
    - Return UncompressedBlockDecoder for NC/NM
    - Return UnsupportedCompression error for other IC values
    - _Requirements: 5.1, 5.2_
  
  - [x] 8.7 Write property test for block access correctness
    - **Property 6: Block Access Returns Correct Data**
    - Generate random image data with known pixel values
    - Read blocks and verify pixel values match expected
    - **Validates: Requirements 6.1, 6.2, 6.5**
  
  - [x] 8.8 Write property test for invalid block coordinates
    - **Property 7: Invalid Block Coordinates Return Error**
    - Generate out-of-bounds block coordinates
    - Verify InvalidBlockCoordinates error returned
    - **Validates: Requirements 6.3, 6.4**
  
  - [x] 8.9 Write property test for pixel data round-trip
    - **Property 4: Pixel Data Round-Trip per IMODE**
    - Generate random pixel data for each IMODE
    - Write then read with same IMODE
    - Verify byte-identical output
    - **Validates: Requirements 5.1-5.6, 10.1-10.8, 17.2**

- [x] 9. Extend JBPImageAssetProvider with ImageAssetProvider trait
  - [x] 9.1 Add ImageAssetProvider trait implementation
    - Add `decoder: OnceCell<Box<dyn BlockDecoder>>` field
    - Implement lazy decoder initialization
    - _Requirements: 18.1_
  
  - [x] 9.2 Implement dimension methods
    - Implement `num_rows()`, `num_columns()`, `num_bands()`
    - Delegate to facade
    - _Requirements: 18.2_
  
  - [x] 9.3 Implement block size methods
    - Implement `num_pixels_per_block_horizontal()`, `num_pixels_per_block_vertical()`
    - Delegate to facade
    - _Requirements: 18.3_
  
  - [x] 9.4 Implement pixel type methods
    - Implement `num_bits_per_pixel()`, `actual_bits_per_pixel()`, `pixel_value_type()`
    - Delegate to facade with type conversion
    - _Requirements: 18.4, 18.5_
  
  - [x] 9.5 Implement block access methods
    - Implement `has_block()` delegating to decoder
    - Implement `get_block()` delegating to decoder
    - Implement `num_resolution_levels()` delegating to decoder
    - _Requirements: 18.6, 18.7_
  
  - [x] 9.6 Write property test for trait compliance
    - **Property 14: ImageAssetProvider Trait Compliance**
    - Create JBPImageAssetProvider instances
    - Verify trait methods return values consistent with subheader
    - **Validates: Requirements 18.1-18.7**

- [x] 10. Checkpoint - Verify ImageAssetProvider implementation
  - Ensure all tests pass, ask the user if questions arise.

- [x] 11. Implement ImageValidator
  - [x] 11.1 Create `src/jbp/image/validation.rs` module
    - Define `ImageValidator` struct with validation methods
    - _Requirements: 13.1-13.6, 14.1-14.5, 15.1-15.5, 16.1-16.4_
  
  - [x] 11.2 Implement dimension validation
    - Check NROWS > 0 and NCOLS > 0
    - Check dimensions against CLEVEL limits
    - Return errors/warnings as appropriate
    - _Requirements: 13.1-13.4_
  
  - [x] 11.3 Implement blocking validation
    - Check NBPR × NPPBH ≥ NCOLS
    - Check NBPC × NPPBV ≥ NROWS
    - _Requirements: 13.5, 13.6_
  
  - [x] 11.4 Implement pixel type validation
    - Check PVTYPE is valid
    - Check ABPP ≤ NBPP
    - Check PVTYPE/NBPP consistency (R requires 32/64, C requires 64, B requires 1)
    - _Requirements: 14.1-14.5_
  
  - [x] 11.5 Implement band configuration validation
    - Check band count matches IREP requirements
    - Check IREPBANDn validity for IREP
    - _Requirements: 15.1-15.5_
  
  - [x] 11.6 Implement LUT validation
    - Check NLUTSn ≤ 4
    - Check NELUTn > 0 when NLUTSn > 0
    - Check RGB/LUT has 3 LUTs
    - _Requirements: 16.1-16.4_
  
  - [x] 11.7 Write property tests for validation
    - **Property 10: Zero Dimension Validation** - verify error for NROWS=0 or NCOLS=0
    - **Property 11: Pixel Type Validation** - verify error for invalid PVTYPE/NBPP
    - **Property 12: Band Configuration Validation** - verify error for IREP/band mismatch
    - **Property 13: LUT Configuration Validation** - verify error for invalid LUT config
    - **Validates: Requirements 13.1-13.6, 14.1-14.5, 15.1-15.5, 16.1-16.4**

- [x] 12. Implement ImageSubheaderBuilder for writing
  - [x] 12.1 Create `ImageSubheaderBuilder` struct
    - Store fields as HashMap<String, Value>
    - Store bands as Vec<BandInfoBuilder>
    - _Requirements: 7.1-7.8_
  
  - [x] 12.2 Implement fluent setter methods
    - Implement setters for all subheader fields
    - Implement `block_size()` for NPPBH/NPPBV
    - Implement `add_band()` for band configuration
    - _Requirements: 7.1-7.8, 9.1-9.9_
  
  - [x] 12.3 Implement blocking parameter calculation
    - Calculate NBPR from NCOLS and NPPBH
    - Calculate NBPC from NROWS and NPPBV
    - Ensure blocking covers image dimensions
    - _Requirements: 8.1-8.8_
  
  - [x] 12.4 Implement `build()` method
    - Write all fields to StructureWriter
    - Handle NBANDS vs XBANDS based on band count
    - Write band info for each band
    - _Requirements: 7.1-7.8, 9.1-9.9_
  
  - [x] 12.5 Implement `BandInfoBuilder` struct
    - Store band fields
    - Implement fluent setters
    - Implement write to StructureWriter
    - _Requirements: 9.1-9.9_
  
  - [x] 12.6 Write property test for blocking calculation
    - **Property 8: Blocking Parameters Cover Image Dimensions**
    - Generate random dimensions and block sizes
    - Verify NBPR × NPPBH ≥ NCOLS and NBPC × NPPBV ≥ NROWS
    - **Validates: Requirements 8.1-8.8, 13.5, 13.6**

- [x] 13. Implement Python bindings for ImageAssetProvider
  - [x] 13.1 Update `src/bindings/image.rs` with ImageAssetProvider methods
    - Expose `has_block()`, `get_block()` methods
    - Expose dimension and pixel type methods
    - _Requirements: 19.1_
  
  - [x] 13.2 Implement NumPy array return for `get_block()`
    - Convert raw bytes to NumPy array with correct dtype
    - Return array with shape (rows, cols, bands)
    - _Requirements: 19.2, 19.3_
  
  - [x] 13.3 Implement band selection parameter
    - Accept optional list of band indices
    - Pass to underlying get_block() call
    - _Requirements: 19.4_
  
  - [x] 13.4 Expose metadata through Python bindings
    - Ensure metadata() method returns image subheader fields
    - _Requirements: 19.5_
  
  - [x] 13.5 Write Python integration tests
    - Test get_block() returns correct NumPy array
    - Test band selection
    - Test metadata access
    - _Requirements: 19.1-19.5_

- [x] 14. Integration testing with JITC test files
  - [x] 14.1 Create integration test for image segment parsing
    - Load NITF_IMG_POS_*.ntf files from JITC test data
    - Parse image subheaders and verify against reference text files
    - _Requirements: 1.1-1.10, 2.1-2.5, 3.1-3.9_
  
  - [x] 14.2 Create integration test for block reading
    - Read blocks from uncompressed test images
    - Verify pixel values are reasonable
    - _Requirements: 5.1-5.6, 6.1-6.5_
  
  - [x] 14.3 Create integration test for multi-band images
    - Test RGB and multispectral images
    - Verify band count and band info
    - _Requirements: 3.1-3.9, 15.1-15.5_

- [ ] 15. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties from the design document
- Checkpoints ensure incremental validation
- The implementation uses Rust with PyO3 for Python bindings
- Integration tests require JITC test data in `data/integration/JITC/`
