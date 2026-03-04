# Implementation Plan: JPEG 2000 Compression

## Overview

This implementation plan covers Phase 5 of the JBP implementation: JPEG 2000 compression support. The implementation adds `Jpeg2000BlockDecoder` and `Jpeg2000BlockEncoder` to handle IC=C8 (JPEG 2000 Part 1) and IC=CD (HTJ2K) compressed imagery.

The implementation builds on the existing `BlockDecoder` and `BlockEncoder` traits, adding OpenJPEG FFI bindings as the default codec backend.

## Tasks

- [ ] 1. Create custom OpenJPEG FFI bindings
  - [ ] 1.1 Create src/jbp/j2k/mod.rs module structure
    - Create `src/jbp/j2k/` directory
    - Add mod.rs with submodule declarations
    - Export public types from the module
    - _Requirements: 0.1_
  
  - [ ] 1.2 Create src/jbp/j2k/sys.rs with raw OpenJPEG FFI declarations
    - Declare opaque types: opj_codec_t, opj_stream_t
    - Declare struct representations: opj_image_t, opj_image_comp_t, opj_image_cmptparm_t
    - Declare parameter structs: opj_cparameters_t, opj_dparameters_t
    - Declare codec lifecycle functions: opj_create_decompress, opj_create_compress, opj_destroy_codec
    - Declare setup functions: opj_setup_decoder, opj_setup_encoder, opj_set_default_decoder_parameters, opj_set_default_encoder_parameters
    - Declare stream functions: opj_stream_create, opj_stream_destroy, opj_stream_set_read_function, opj_stream_set_write_function, opj_stream_set_skip_function, opj_stream_set_seek_function, opj_stream_set_user_data, opj_stream_set_user_data_length
    - Declare decoding functions: opj_read_header, opj_decode, opj_end_decompress, opj_set_decode_area, opj_set_decoded_resolution_factor
    - Declare tile decoding functions: opj_read_tile_header, opj_decode_tile_data, opj_get_decoded_tile
    - Declare encoding functions: opj_start_compress, opj_encode, opj_end_compress
    - Declare tile encoding function: opj_write_tile
    - Declare image functions: opj_image_create, opj_image_tile_create, opj_image_destroy, opj_image_data_alloc, opj_image_data_free
    - Declare info functions: opj_get_cstr_info, opj_get_cstr_index, opj_destroy_cstr_info, opj_destroy_cstr_index
    - Declare message handler functions: opj_set_info_handler, opj_set_warning_handler, opj_set_error_handler
    - Declare threading function: opj_codec_set_threads
    - Declare constants: OPJ_CODEC_J2K, OPJ_CODEC_JP2, OPJ_TRUE, OPJ_FALSE, OPJ_CLRSPC_* color spaces, OPJ_PROG_ORDER progression orders
    - _Requirements: 20.1, 20.2_
  
  - [ ] 1.3 Update Cargo.toml with OpenJPEG linking configuration
    - Add `openjpeg` feature flag (default enabled)
    - Add build.rs or pkg-config dependency for finding libopenjp2
    - Configure conditional compilation for openjpeg feature
    - _Requirements: 20.1, 20.2_
  
  - [ ] 1.4 Create src/jbp/j2k/ffi.rs with safe OpenJPEG wrapper
    - Wrap raw sys types in safe Rust abstractions
    - Implement Drop for opj_image_t, opj_codec_t, and opj_stream_t handles
    - Implement MemoryReadStream adapter for reading from &[u8] byte slices:
      - Store byte slice reference and current position in user_data struct
      - Implement read callback: copy bytes from slice to buffer, advance position
      - Implement skip callback: advance position by offset (clamped to bounds)
      - Implement seek callback: set position to absolute offset (clamped to bounds)
      - Set stream length via opj_stream_set_user_data_length
    - Implement MemoryWriteStream adapter for writing to Vec<u8>:
      - Store Vec<u8> and current position in user_data struct (boxed for FFI safety)
      - Implement write callback: extend/overwrite vec, advance position
      - Implement skip/seek callbacks for random access writes
      - Provide finalize method to extract the written bytes
    - Add error handling for FFI calls with proper error messages
    - Implement message handler callbacks to capture OpenJPEG warnings/errors
    - _Requirements: 20.1, 20.3_

- [ ] 2. Implement J2KCodec trait and OpenJpegCodec
  - [ ] 2.1 Create src/jbp/j2k/codec.rs with J2KCodec trait
    - Define J2KCodecCapabilities struct
    - Define J2KDecodeParams and J2KEncodeParams structs
    - Define J2KDecodeResult struct
    - Define J2KCodec trait with decode, start_encode, get_resolution_levels, get_dimensions
    - Define J2KEncodeState trait for incremental encoding
    - _Requirements: 0.1, 0.2, 0.3, 0.4, 0.5_
  
  - [ ] 2.2 Implement OpenJpegCodec in src/jbp/j2k/openjpeg.rs
    - Implement J2KCodec::capabilities() returning max_bit_depth=38, htj2k=false
    - Implement J2KCodec::decode() using opj_decode()
    - Implement J2KCodec::start_encode() returning OpenJpegEncodeState
    - Implement J2KCodec::get_resolution_levels() by reading COD marker
    - Implement J2KCodec::get_dimensions() by reading SIZ marker
    - _Requirements: 20.1, 20.3, 20.4, 20.5_
  
  - [ ] 2.3 Write unit tests for OpenJpegCodec
    - Test capabilities() returns correct values
    - Test decode() with synthetic J2K codestream
    - Test get_resolution_levels() and get_dimensions()
    - Test error handling for invalid codestreams
    - _Requirements: 20.1, 20.3_

- [ ] 3. Checkpoint - Verify OpenJPEG integration
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement COMRAT parsing and generation
  - [ ] 4.1 Create src/jbp/j2k/comrat.rs with J2KComrat enum
    - Implement J2KComrat::parse() for "Nnnn.n", "Vnnn.n", "nn.n" formats
    - Implement J2KComrat::to_string() for generating COMRAT
    - Implement generate_comrat() from J2KEncodingHints
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 10.1, 10.2, 10.3, 10.4_
  
  - [ ]* 4.2 Write property test for COMRAT round-trip
    - **Property 9: COMRAT Parse-Generate Round-Trip**
    - **Validates: Requirements 5.1, 10.4**
  
  - [ ] 4.3 Write unit tests for COMRAT edge cases
    - Test parsing invalid COMRAT values
    - Test boundary values for bpp rates
    - _Requirements: 5.5, 16.3_

- [ ] 5. Implement Jpeg2000BlockDecoder
  - [ ] 5.1 Create src/jbp/j2k/decoder.rs with Jpeg2000BlockDecoder struct
    - Store codestream, dimensions, nbands, nbpp, pvtype, ic, comrat, codec
    - Implement constructor with BPJ2K01.20 validation (IMODE=B, NBPP 1-38, ABPP=NBPP)
    - Implement select_bands() helper for band selection
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 6.1, 6.2, 6.3, 6.4, 6.5_
  
  - [ ] 5.2 Implement BlockDecoder trait for Jpeg2000BlockDecoder
    - Implement decode_block() delegating to J2KCodec::decode()
    - Implement has_block() returning true only for (0,0)
    - Implement compression_type() returning IC value
    - Implement num_resolution_levels() using cached value from codec
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 4.4, 4.5_
  
  - [ ]* 5.3 Write property test for lossless round-trip
    - **Property 1: Lossless Round-Trip Consistency**
    - **Validates: Requirements 15.1, 15.3, 15.4, 13.1-13.4, 14.1-14.4**
  
  - [ ]* 5.4 Write property test for resolution level scaling
    - **Property 3: Resolution Level Dimension Scaling**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.5**
  
  - [ ]* 5.5 Write property test for band count preservation
    - **Property 5: Band Count Preservation**
    - **Validates: Requirements 2.4, 2.5**
  
  - [ ]* 5.6 Write unit tests for BPJ2K profile validation
    - Test IMODE validation error
    - Test NBPP range validation error
    - Test ABPP != NBPP validation error
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 16.4_

- [ ] 6. Checkpoint - Verify decoder implementation
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Implement Jpeg2000BlockEncoder
  - [ ] 7.1 Create src/jbp/j2k/encoder.rs with J2KEncodingHints struct
    - Define compression_ratio, lossless, decomposition_levels, quality_layers, htj2k fields
    - Implement Default trait with sensible defaults
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5_
  
  - [ ] 7.2 Implement Jpeg2000BlockEncoder struct
    - Store codec, encode_state, block_grid, block_dims, ic, encoded_blocks
    - Implement constructor validating HTJ2K codec support
    - Calculate block grid from image dimensions and tile size
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_
  
  - [ ] 7.3 Implement BlockEncoder trait for Jpeg2000BlockEncoder
    - Implement encode_block() validating coordinates and delegating to J2KEncodeState::encode_tile()
    - Implement finalize() verifying all blocks encoded and calling J2KEncodeState::finalize()
    - Implement compression_type() returning "C8" or "CD"
    - Implement block_grid_size() and block_dimensions()
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 8.1, 8.2, 8.3, 8.4_
  
  - [ ]* 7.4 Write property test for lossy round-trip quality
    - **Property 2: Lossy Round-Trip Quality Tolerance**
    - **Validates: Requirements 15.2**

- [ ] 8. Extend factory functions for J2K support
  - [ ] 8.1 Update create_block_decoder() in src/jbp/image/decoder.rs
    - Add match arms for "C8" and "CD" IC codes
    - Create Jpeg2000BlockDecoder with default OpenJpegCodec
    - _Requirements: 17.1, 17.2_
  
  - [ ] 8.2 Update create_block_encoder() in src/jbp/image/encoder.rs
    - Add match arms for "C8" and "CD" IC codes
    - Create Jpeg2000BlockEncoder with J2KEncodingHints
    - _Requirements: 18.1, 18.2_
  
  - [ ] 8.3 Add get_j2k_codec() function for codec selection
    - Read OSML_IO_J2K_CODEC environment variable
    - Return OpenJpegCodec as default
    - _Requirements: 0.6, 0.7, 21.2, 21.3_

- [ ] 9. Checkpoint - Verify encoder and factory integration
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 10. Integrate with JBPDatasetReader
  - [ ] 10.1 Update JBPImageAssetProvider to use Jpeg2000BlockDecoder
    - Pass J2K codec to create_block_decoder() for IC=C8/CD
    - Expose num_resolution_levels() for J2K images
    - Support band selection for J2K images
    - _Requirements: 17.1, 17.2, 17.3, 17.4_
  
  - [ ]* 10.2 Write property test for decoded dimensions
    - **Property 4: Decoded Dimensions Match Subheader**
    - **Validates: Requirements 2.2**

- [ ] 11. Integrate with JBPDatasetWriter
  - [ ] 11.1 Update JBPDatasetWriter to use Jpeg2000BlockEncoder
    - Accept IC hint "C8" or "CD" for J2K compression
    - Accept J2KEncodingHints via encoding hints
    - Validate J2K constraints before encoding
    - _Requirements: 18.1, 18.2, 18.3, 18.4_
  
  - [ ] 11.2 Implement IC and COMRAT field generation
    - Set IC to "C8" for Part 1, "CD" for HTJ2K
    - Set IMODE to "B" for J2K images
    - Generate COMRAT from encoding hints
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 10.1, 10.2, 10.3, 10.4_
  
  - [ ] 11.3 Implement codestream embedding
    - Embed J2K codestream in image data area
    - Calculate correct image data length
    - Ensure correct offset after subheader
    - _Requirements: 12.1, 12.2, 12.3, 12.4_
  
  - [ ]* 11.4 Write property test for codestream embedding
    - **Property 10: Codestream Embedding Integrity**
    - **Validates: Requirements 12.1, 12.2, 12.3, 12.4**

- [ ] 12. Checkpoint - Verify reader/writer integration
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 13. Update Python bindings
  - [ ] 13.1 Update PyJBPImageAssetProvider for J2K support
    - Add resolution_level parameter to get_block()
    - Expose num_resolution_levels() method
    - Return correct NumPy dtypes for all J2K bit depths
    - _Requirements: 19.1, 19.2, 19.3_
  
  - [ ] 13.2 Update PyJBPDatasetWriter for J2K support
    - Accept ic="C8" or ic="CD" parameter
    - Accept compression configuration parameters (lossless, compression_ratio, etc.)
    - _Requirements: 19.4_
  
  - [ ] 13.3 Write Python integration tests
    - Test reading J2K image at different resolution levels
    - Test writing lossless J2K image and reading back
    - Test writing lossy J2K image with compression ratio
    - _Requirements: 19.1, 19.2, 19.3, 19.4_

- [ ] 14. Add error handling and validation
  - [ ] 14.1 Add J2K-specific error variants to CodecError
    - Add InvalidResolutionLevel error with requested and available levels
    - Ensure decode errors include codec error message and byte offset
    - Ensure encode errors include encoding parameters and failure reason
    - _Requirements: 16.1, 16.2, 16.5_
  
  - [ ]* 14.2 Write unit tests for error messages
    - Test decode error includes byte offset
    - Test encode error includes parameters
    - Test profile validation error includes constraint and JBP requirement ID
    - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5_

- [ ] 15. Final checkpoint - Complete integration testing
  - Ensure all tests pass, ask the user if questions arise.
  - Run integration tests with real J2K-compressed NITF files from data/integration/

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The OpenJPEG library does not support HTJ2K (Part 15) - HTJ2K support requires a different codec backend
