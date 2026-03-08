# Implementation Plan: JPEG DCT Compression (IC=C3/M3/I1)

## Overview

This implementation plan adds JPEG DCT compression support to the osml-imagery-io library using FFI bindings to libjpeg-turbo. The implementation follows the existing patterns established by the JPEG 2000 codec and integrates with the JBP image reader/writer infrastructure.

## Tasks

- [x] 1. Set up JPEG module structure and FFI bindings
  - [x] 1.1 Create `src/jbp/jpeg/mod.rs` with module exports
    - Define module structure matching j2k pattern
    - Export public types: JpegCodec, JpegBlockDecoder, JpegBlockEncoder, JpegComrat
    - _Requirements: 6.1, 6.2_
  
  - [x] 1.2 Create `src/jbp/jpeg/sys.rs` with raw FFI declarations for libjpeg-turbo
    - Define turbojpeg API types: tjhandle, TJPF pixel formats, TJSAMP subsampling
    - Define libjpeg API types for 12-bit: jpeg_compress_struct, jpeg_decompress_struct
    - Declare extern functions: tjInitCompress, tjInitDecompress, tjCompress2, tjDecompress2
    - Declare 12-bit functions: jpeg_CreateCompress, jpeg_CreateDecompress
    - _Requirements: 6.1, 6.3, 6.4_
  
  - [x] 1.3 Create `src/jbp/jpeg/ffi.rs` with safe Rust wrappers
    - Implement TjHandle wrapper with Drop for automatic cleanup
    - Implement safe compress/decompress functions with error handling
    - Implement 12-bit compress/decompress wrappers
    - Add thread-local error message storage (following j2k pattern)
    - _Requirements: 6.1, 6.3, 6.4_
  
  - [x] 1.4 Update `Cargo.toml` with libjpeg-turbo feature flag
    - Add `libjpeg-turbo` feature to default features
    - Add conditional compilation for jpeg module
    - _Requirements: 6.1_

- [x] 2. Implement JPEG codec core
  - [x] 2.1 Create `src/jbp/jpeg/codec.rs` with JpegCodec struct
    - Define JpegCodecCapabilities struct
    - Implement JpegCodec::new() and JpegCodec::with_quality()
    - Implement capabilities() method
    - _Requirements: 6.3, 6.4_
  
  - [x] 2.2 Create `src/jbp/jpeg/comrat.rs` with COMRAT parsing
    - Define JpegComrat enum (Quality, Default)
    - Implement parse() for NITF COMRAT field format
    - Implement to_comrat_string() for writing
    - Implement quality() to get JPEG quality 1-100
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  
  - [x] 2.3 Write unit tests for COMRAT parsing
    - Test valid COMRAT values ("00.0" to "99.9")
    - Test default quality mapping
    - Test edge cases and invalid inputs
    - _Requirements: 5.1, 5.2, 5.3_

- [x] 3. Implement JPEG block decoder
  - [x] 3.1 Create `src/jbp/jpeg/decoder.rs` with JpegBlockDecoder
    - Implement new() with pixel_type, num_bands, dimensions, imode, color_space
    - Implement decode_block() for 8-bit grayscale
    - Implement decode_block() for 8-bit RGB/YCbCr
    - _Requirements: 1.1, 1.2, 1.4, 1.5_
  
  - [x] 3.2 Add 12-bit decoding support to JpegBlockDecoder
    - Return clear error message for 12-bit JPEG requests
    - Document limitation and suggest alternatives (J2K, uncompressed)
    - _Requirements: 1.3_
  
  - [x] 3.3 Add multiband decoding support
    - Implement IMODE=B (block interleaved) decoding
    - Implement IMODE=S (sequential) decoding
    - Parse band-prefixed JPEG streams
    - _Requirements: 1.6_
  
  - [x] 3.4 Write unit tests for JPEG decoding
    - Test 8-bit grayscale decoding
    - Test 8-bit RGB decoding
    - Test 12-bit grayscale decoding
    - Test multiband decoding
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [x] 4. Implement JPEG block encoder
  - [x] 4.1 Create `src/jbp/jpeg/encoder.rs` with JpegBlockEncoder
    - Implement new() with pixel_type, num_bands, dimensions, imode, color_space, quality
    - Implement encode_block() for 8-bit grayscale
    - Implement encode_block() for 8-bit RGB
    - _Requirements: 2.1, 2.2, 2.4_
  
  - [x] 4.2 Add YCbCr color space conversion
    - Implement RGB to YCbCr601 conversion before encoding
    - Use turbojpeg's built-in color space handling
    - _Requirements: 2.5_
  
  - [x] 4.3 Add 12-bit encoding support
    - Implement 12-bit grayscale encoding using libjpeg API
    - Handle UInt16 input with values in 0-4095 range
    - _Requirements: 2.3_
  
  - [x] 4.4 Add multiband encoding support
    - Implement IMODE=B (block interleaved) encoding
    - Implement IMODE=S (sequential) encoding
    - Generate band-prefixed JPEG streams with length headers
    - _Requirements: 2.6_
  
  - [x] 4.5 Write unit tests for JPEG encoding
    - Test 8-bit grayscale encoding
    - Test 8-bit RGB encoding
    - Test 12-bit grayscale encoding
    - Test multiband encoding
    - Test quality parameter effect
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [x] 5. Checkpoint - Verify codec functionality
  - Ensure all codec unit tests pass
  - Verify FFI bindings work correctly
  - Ask the user if questions arise

- [x] 6. Integrate JPEG codec with JBP reader
  - [x] 6.1 Update `src/jbp/image/decoder.rs` to dispatch to JPEG decoder
    - Add IC code matching for C3, M3, I1
    - Create JpegBlockDecoder when IC indicates JPEG
    - Pass through COMRAT and IMODE from image subheader
    - _Requirements: 1.1_
  
  - [x] 6.2 Update `src/jbp/image/facade.rs` for JPEG image assets
    - Handle JPEG-specific metadata (COMRAT)
    - Support resolution levels (JPEG has only level 0)
    - _Requirements: 1.1, 5.1_
  
  - [x] 6.3 Add masked JPEG (M3) support to reader
    - Parse mask table for M3 images
    - Implement has_block() for masked blocks
    - Skip masked blocks in get_block()
    - _Requirements: 3.1, 3.2, 3.3_
  
  - [x] 6.4 Add downsampled JPEG (I1) support to reader
    - Handle single-block I1 images
    - Validate 2048×2048 dimension constraint
    - _Requirements: 4.1, 4.2_

- [x] 7. Integrate JPEG codec with JBP writer
  - [x] 7.1 Update `src/jbp/image/encoder.rs` to dispatch to JPEG encoder
    - Add IC code handling for C3, M3, I1 metadata hints
    - Create JpegBlockEncoder when IC indicates JPEG
    - Pass through COMRAT and IMODE from metadata
    - _Requirements: 2.1_
  
  - [x] 7.2 Update `src/jbp/image/builder.rs` for JPEG encoding hints
    - Handle IC=C3/M3/I1 metadata hints
    - Set default COMRAT if not specified
    - Validate IMODE compatibility with JPEG
    - _Requirements: 2.1, 5.4_
  
  - [x] 7.3 Add masked JPEG (M3) support to writer
    - Generate mask table for sparse images
    - Mark omitted blocks as masked
    - _Requirements: 3.4, 3.5_
  
  - [x] 7.4 Add downsampled JPEG (I1) support to writer
    - Encode as single JPEG block
    - Validate 2048×2048 dimension constraint
    - Return error for oversized images
    - _Requirements: 4.3, 4.4_

- [x] 8. Checkpoint - Verify integration
  - Ensure reader/writer integration works
  - Test basic roundtrip with IC=C3
  - Ask the user if questions arise

- [x] 9. Add property-based tests
  - [x] 9.1 Update `tests/property/strategies.py` with JPEG strategies
    - Add JPEG IC codes (C3, M3, I1) to IC code strategies
    - Add JPEG pixel types (UInt8, UInt16 for 12-bit)
    - Add I1 dimension constraints (≤2048×2048)
    - Add JPEG-specific image generator for realistic compression testing
    - _Requirements: 7.1, 7.2_
  
  - [x] 9.2 Create `tests/property/test_jpeg_roundtrip.py` with Property 1
    - **Property 1: JPEG DCT Lossy Roundtrip Quality**
    - Test 8-bit mono, RGB, YCbCr, multiband configurations
    - Test 12-bit mono configuration
    - Verify PSNR >= 30 dB, SSIM >= 0.95
    - Verify shape and dtype preservation
    - **Validates: Requirements 1.1-1.6, 2.1-2.6, 7.1-7.4**
  
  - [x] 9.3 Add Property 2 test for masked JPEG roundtrip
    - **Property 2: Masked JPEG Roundtrip**
    - Test IC=M3 with various mask patterns
    - Verify mask pattern preservation
    - Verify valid block data matches original
    - **Validates: Requirements 3.1-3.5**
  
  - [x] 9.4 Add Property 3 test for downsampled JPEG (I1)
    - **Property 3: Downsampled JPEG (I1) Roundtrip**
    - Test IC=I1 with images ≤2048×2048
    - Verify quality bounds (PSNR >= 30 dB, SSIM >= 0.95)
    - Verify dimension preservation
    - **Validates: Requirements 4.1-4.3**
  
  - [ ]* 9.5 Add Property 4 test for COMRAT preservation
    - **Property 4: COMRAT Metadata Preservation**
    - Test various COMRAT values
    - Verify COMRAT is preserved in metadata after roundtrip
    - **Validates: Requirements 5.1-5.3**

- [x] 10. Checkpoint - Verify property tests
  - Run all property tests with `pytest -m property tests/property/test_jpeg_roundtrip.py`
  - Ensure all properties pass with 100 iterations
  - Ask the user if questions arise

- [x] 11. Update documentation
  - [x] 11.1 Update `docs/JBP_CLEVEL_ASSESSMENT.md`
    - Mark JPEG DCT (C3/M3) features as implemented
    - Mark Downsampled JPEG (I1) as implemented
    - Update status summary
    - _Requirements: 8.1_
  
  - [x] 11.2 Update `docs/API_DESIGN.md`
    - Add JPEG compression to supported IC codes table
    - Add COMRAT format documentation for JPEG
    - Add usage examples for JPEG compression
    - _Requirements: 8.2_
  
  - [x] 11.3 Update `docs/JBP_ROADMAP.md`
    - Mark Phase 3 as complete
    - Add implementation notes
    - _Requirements: 8.3_

- [x] 12. Final checkpoint
  - Run full test suite: `cargo test` and `pytest`
  - Verify all documentation is updated
  - Ensure all tests pass
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation follows the existing J2K codec pattern for consistency
