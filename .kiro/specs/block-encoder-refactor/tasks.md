# Implementation Plan: Block Encoder Refactor

## Overview

This implementation plan covers refactoring the JBP image writing code to use a symmetric block-based I/O architecture. The plan introduces a `BlockEncoder` trait that mirrors `BlockDecoder`, enabling consistent patterns for reading and writing image data with support for different tile sizes.

## Tasks

- [x] 1. Create BlockEncoder trait and module structure
  - [x] 1.1 Create `src/jbp/image/encoder.rs` module
    - Define `BlockEncoder` trait with encode_block(), finalize(), compression_type(), block_grid_size(), block_dimensions()
    - Add module to `src/jbp/image/mod.rs`
    - _Requirements: 1.1-1.6_
  
  - [ ]* 1.2 Write unit tests for BlockEncoder trait contract
    - Test that trait is object-safe
    - Test Send + Sync bounds compile
    - _Requirements: 1.6_

- [x] 2. Implement UncompressedBlockEncoder
  - [x] 2.1 Create `UncompressedBlockEncoder` struct
    - Store image dimensions, bands, bit depth, IMODE, block dimensions
    - Pre-allocate output buffer
    - Track which blocks have been encoded
    - _Requirements: 2.1, 2.3_
  
  - [x] 2.2 Implement encode_block() method
    - Validate block coordinates against grid size
    - Validate data size matches shape
    - Convert BSQ input to target IMODE
    - Write to correct position in output buffer
    - Mark block as encoded
    - _Requirements: 2.2, 2.4, 2.5_

  - [x] 2.3 Implement IMODE conversion functions
    - Implement bsq_to_imode_b() for IMODE B
    - Implement bsq_to_imode_p() for IMODE P
    - Implement bsq_to_imode_r() for IMODE R
    - Implement bsq_to_imode_s() for IMODE S
    - Reuse existing conversion logic from writer.rs where possible
    - _Requirements: 2.2, 2.4, 6.2, 6.3_
  
  - [x] 2.4 Implement finalize() method
    - Check all blocks have been encoded
    - Return IncompleteData error if blocks missing
    - Return encoded data buffer
    - _Requirements: 8.4_
  
  - [x] 2.5 Implement compression_type() and grid methods
    - Return "NC" for compression_type()
    - Return calculated grid size
    - Return block dimensions
    - _Requirements: 1.4, 1.5, 2.6_
  
  - [x] 2.6 Write property test for IMODE conversion
    - **Property 3: IMODE Conversion Preserves Pixels**
    - Generate random image data
    - Encode with each IMODE, decode, verify pixel equality
    - **Validates: Requirements 2.2, 2.4, 6.4, 7.3**

- [x] 3. Checkpoint - Verify UncompressedBlockEncoder
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Implement create_block_encoder factory
  - [x] 4.1 Create factory function in encoder.rs
    - Accept IC code, dimensions, bands, bit depth, IMODE, block dimensions
    - Return UncompressedBlockEncoder for IC="NC"
    - Return Unsupported error for other IC codes
    - _Requirements: 3.1-3.4_
  
  - [ ]* 4.2 Write unit tests for factory
    - Test NC returns UncompressedBlockEncoder
    - Test unsupported IC returns error
    - _Requirements: 3.3, 3.4_

- [x] 5. Implement TileAssembler
  - [x] 5.1 Create `TileAssembler` struct
    - Store reference to source ImageAssetProvider
    - Store output tile dimensions
    - Calculate source tile dimensions from provider
    - _Requirements: 5.1, 5.2_
  
  - [x] 5.2 Implement output_grid_size() method
    - Calculate grid based on image dimensions and output tile size
    - _Requirements: 5.1_
  
  - [x] 5.3 Implement get_output_tile() method
    - Calculate pixel region for output tile
    - Determine which source tiles overlap
    - Read source tiles and copy relevant pixels
    - Return assembled tile in BSQ format
    - _Requirements: 5.2, 5.3, 5.4_
  
  - [x] 5.4 Implement copy_tile_region() helper
    - Copy overlapping region from source tile to output buffer
    - Handle coordinate translation between source and output
    - _Requirements: 5.3_
  
  - [x] 5.5 Write property test for tile assembly
    - **Property 2: Tile Size Conversion Preserves Pixels**
    - Generate random images with various tile sizes
    - Assemble with different output tile sizes
    - Verify all pixels match original
    - **Validates: Requirements 5.1, 5.3, 5.4, 5.5, 7.2**

- [x] 6. Checkpoint - Verify TileAssembler
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Refactor JBPDatasetWriter to use BlockEncoder
  - [x] 7.1 Add BlockEncoder integration to writer
    - Create BlockEncoder based on IC hint
    - Use TileAssembler to read source tiles
    - Pass tiles to BlockEncoder
    - Call finalize() to get encoded data
    - _Requirements: 4.1-4.4_
  
  - [x] 7.2 Remove inline IMODE conversion code
    - Remove bsq_to_imode_* functions from writer.rs (now in encoder)
    - Update convert_bsq_to_imode to use BlockEncoder
    - _Requirements: 4.1_
  
  - [x] 7.3 Ensure public API unchanged
    - Verify add_asset() signature unchanged
    - Verify close() behavior unchanged
    - _Requirements: 4.5_
  
  - [x] 7.4 Write integration test for writer refactor
    - Write NITF with BufferedImageAssetProvider
    - Read back with JBPDatasetReader
    - Verify pixel data matches
    - _Requirements: 4.1-4.5_

- [x] 8. Checkpoint - Verify JBPDatasetWriter refactor
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Implement round-trip tests
  - [x] 9.1 Write property test for round-trip consistency
    - **Property 1: Round-Trip Consistency**
    - Generate random image data
    - Write with JBPDatasetWriter
    - Read with JBPDatasetReader
    - Verify pixel-perfect match
    - **Validates: Requirements 7.1, 7.4**
  
  - [x] 9.2 Write property test for tile size round-trip
    - Generate images with various source tile sizes
    - Write with different output tile sizes
    - Read back and verify pixel equality
    - **Validates: Requirements 5.5, 7.2**
  
  - [x] 9.3 Write property test for edge blocks
    - **Property 4: Edge Block Handling**
    - Generate images with non-divisible dimensions
    - Encode and decode
    - Verify edge block pixels correct
    - **Validates: Requirements 2.5**
  
  - [x] 9.4 Write property test for block grid calculation
    - **Property 5: Block Grid Calculation**
    - Generate random dimensions and block sizes
    - Verify grid size matches ceil formula
    - **Validates: Requirements 1.5**

- [x] 10. Error handling tests
  - [x] 10.1 Write tests for invalid block coordinates
    - Test encode_block with out-of-bounds coordinates
    - Verify error includes coordinates and grid size
    - _Requirements: 8.1_
  
  - [x] 10.2 Write tests for invalid data size
    - Test encode_block with wrong data size
    - Verify error includes expected and actual sizes
    - _Requirements: 8.2_
  
  - [x] 10.3 Write tests for incomplete encoding
    - Test finalize() before all blocks encoded
    - Verify error indicates missing blocks
    - _Requirements: 8.4_

- [x] 11. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties from the design document
- Checkpoints ensure incremental validation
- The implementation uses Rust with proptest for property-based testing
- Existing IMODE conversion code in writer.rs can be reused/moved to encoder module
