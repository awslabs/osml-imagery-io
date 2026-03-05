# Implementation Plan: Phase 6 - Image Masking

## Overview

This implementation plan breaks down the image masking feature into discrete coding tasks. The implementation follows the existing patterns in the osml-imagery-io codebase, using Rust for core functionality with Python bindings via PyO3.

The implementation is organized into:
1. Core mask data structures and parsing
2. Reader integration (JBPImageAssetProvider updates)
3. Writer integration (JBPDatasetWriter updates, BufferedImageAssetProvider updates)
4. Property-based testing infrastructure
5. Synthetic image generator updates

## Tasks

- [x] 1. Implement ImageDataMask struct and parsing
  - [x] 1.1 Create `src/jbp/image/mask.rs` with ImageDataMask struct
    - Define struct fields: image_data_offset, block_mask_record_length, pad_pixel_mask_record_length, pad_pixel_code_length, pad_pixel_code, block_offsets, pad_pixel_offsets
    - Define EMPTY_BLOCK_OFFSET constant (0xFFFFFFFF)
    - _Requirements: 1.1, 1.2, 1.6_
  
  - [x] 1.2 Implement ImageDataMask::parse() method
    - Parse IMDATOFF (4 bytes, u32 BE)
    - Parse BMRLNTH (2 bytes, u16 BE)
    - Parse TMRLNTH (2 bytes, u16 BE)
    - Parse TPXCDLNTH (2 bytes, u16 BE)
    - Parse TPXCD if TPXCDLNTH > 0
    - Parse block offsets if BMRLNTH > 0
    - Parse pad pixel offsets if TMRLNTH > 0
    - Handle IMODE=S indexing (blocks × bands)
    - _Requirements: 1.2, 1.3, 1.4, 1.5_
  
  - [x] 1.3 Implement ImageDataMask::to_bytes() method
    - Serialize all fields to binary format
    - Calculate correct IMDATOFF based on mask table size
    - _Requirements: 4.1, 4.4_
  
  - [x] 1.4 Implement ImageDataMask query methods
    - has_block(block_row, block_col, num_blocks_per_row, band, imode) -> bool
    - get_block_offset(block_row, block_col, num_blocks_per_row, band, imode) -> Option<u64>
    - pad_pixel_value() -> Option<u32>
    - _Requirements: 2.1, 2.2, 3.1, 3.3_
  
  - [x] 1.5 Implement ImageDataMask::from_provided_blocks() constructor
    - Accept HashSet of (row, col) tuples for provided blocks
    - Set valid placeholder offsets for provided blocks (actual offsets set during encoding)
    - Set 0xFFFFFFFF for missing blocks
    - _Requirements: 4.2, 4.3, 5.2, 5.3_
  
  - [ ]* 1.6 Write property test for mask roundtrip
    - **Property 1: Mask Roundtrip Consistency**
    - Generate random ImageDataMask, serialize, parse, compare
    - **Validates: Requirements 1.2, 4.5, 8.1**

- [x] 2. Implement IC field classification helpers
  - [x] 2.1 Add IC classification functions to `src/jbp/image/mod.rs`
    - is_masked_ic(ic: &str) -> bool
    - unmask_ic(ic: &str) -> &str
    - mask_ic(ic: &str) -> &str
    - _Requirements: 7.1, 7.4, 7.5, 7.6_
  
  - [ ]* 2.2 Write unit tests for IC classification
    - Test all masked IC values (NM, M1, M3, M4, M5, M7, M8, M9, MA, MB, MC, MD, ME)
    - Test all non-masked IC values
    - Test mask/unmask roundtrip
    - _Requirements: 7.1_

- [x] 3. Checkpoint - Ensure mask parsing tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Update JBPImageAssetProvider for masked image reading
  - [x] 4.1 Add mask field to JBPImageAssetProvider
    - Add `mask: Option<ImageDataMask>` field
    - Parse mask during construction when IC is masked
    - _Requirements: 1.1_
  
  - [x] 4.2 Update has_block() implementation
    - For non-masked images: check bounds only (existing behavior)
    - For masked images: check mask.has_block()
    - _Requirements: 2.1, 2.2_
  
  - [x] 4.3 Update get_block() implementation
    - Validate block exists via has_block()
    - Return BlockNotFound error for masked blocks
    - For valid blocks: get offset from mask, decode at offset
    - _Requirements: 2.3, 2.4_
  
  - [x] 4.4 Update pad_pixel_value() implementation
    - Return mask.pad_pixel_value() if mask present
    - _Requirements: 3.1, 3.3_
  
  - [ ]* 4.5 Write property test for has_block correctness
    - **Property 2: has_block Correctness for Masked Blocks**
    - Generate masked images, verify has_block matches mask
    - **Validates: Requirements 2.1, 2.2, 6.5, 8.2, 8.4**

- [x] 5. Update BlockDecoder trait for offset-based decoding
  - [x] 5.1 Add decode_block_at_offset() to BlockDecoder trait
    - New method signature for decoding at specific offset
    - _Requirements: 2.4_
  
  - [x] 5.2 Implement decode_block_at_offset() in UncompressedBlockDecoder
    - Seek to offset, read block data, decode
    - _Requirements: 2.4_
  
  - [x] 5.3 Implement decode_block_at_offset() in J2KBlockDecoder
    - Seek to offset, extract J2K codestream, decode
    - _Requirements: 6.1, 6.2_

- [x] 6. Checkpoint - Ensure reader tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Update BufferedImageAssetProvider for sparse block tracking
  - [x] 7.1 Add provided_blocks field to BufferedImageAssetProvider
    - Add `provided_blocks: HashSet<(u32, u32)>` field
    - _Requirements: 5.1_
  
  - [x] 7.2 Update set_block() to track provided blocks
    - Insert (block_row, block_col) into provided_blocks
    - _Requirements: 5.1_
  
  - [x] 7.3 Update has_block() to check provided_blocks
    - Return true only if block is in provided_blocks
    - _Requirements: 5.1_

- [x] 8. Update JBPDatasetWriter for masked image writing
  - [x] 8.1 Add collect_provided_blocks() helper method
    - Iterate over block grid, check has_block() for each
    - Return HashSet of provided block coordinates
    - _Requirements: 5.1_
  
  - [x] 8.2 Add validation for non-masked IC with missing blocks
    - Check if IC is non-masked and blocks are missing
    - Raise MissingBlocks error with expected/provided counts
    - _Requirements: 7.2, 7.3_
  
  - [x] 8.3 Implement mask generation in write_image_segment()
    - Generate ImageDataMask from provided blocks when IC is masked
    - Write mask table before image data
    - Update block offsets during encoding
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.4_
  
  - [ ]* 8.4 Write property test for mask generation
    - **Property 4: Mask Generation from Sparse Data**
    - Generate sparse block sets, verify mask offsets
    - **Validates: Requirements 4.2, 4.3, 5.2, 5.3**
  
  - [ ]* 8.5 Write property test for non-masked IC validation
    - **Property 5: Non-Masked IC Validation**
    - Verify MissingBlocks error for non-masked IC with sparse data
    - **Validates: Requirements 7.2, 7.3**

- [x] 9. Checkpoint - Ensure writer tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Add error types for masking
  - [x] 10.1 Add BlockNotFound error variant to CodecError
    - Include row and col fields
    - _Requirements: 2.3_
  
  - [x] 10.2 Add MissingBlocks error variant to CodecError
    - Include expected, provided, and ic fields
    - _Requirements: 7.2, 7.3_
  
  - [x] 10.3 Add InvalidMaskTable error variant to CodecError
    - Include reason field
    - _Requirements: 1.1_

- [~] 11. Implement masked image roundtrip property tests
  - [x] 11.1 Add mask_patterns strategy to tests/property/strategies.py
    - Generate various mask patterns (all_present, all_masked, checkerboard, border_only, random, single_block)
    - _Requirements: 10.1_
  
  - [x] 11.2 Add masked_image strategy to tests/property/strategies.py
    - Combine image generation with mask pattern generation
    - Include IC value selection (NM, M8)
    - _Requirements: 10.1_
  
  - [x] 11.3 Create tests/property/test_masking.py
    - **Property 3: Valid Block Decoding in Masked Images**
    - **Property 6: Pad Pixel Value Preservation**
    - **Property 7: Masked Block Pattern Preservation**
    - **Validates: Requirements 2.4, 3.1, 3.3, 8.1, 8.2, 8.3**
    - Supports both NM (uncompressed masked) and M8 (JPEG 2000 masked)
  
  - [x] 11.4 Update tests/property/test_roundtrip.py for masked images
    - Add test cases using masked_image strategy
    - Verify lossless roundtrip for valid blocks
    - _Requirements: 8.1_
  
  - [x] 11.5 Enable M8 support in property tests
    - Fixed M8 writer to track per-block offsets (each block encoded as separate J2K codestream)
    - See `docs/BUG_MASKED_J2K_INCOMPLETE.md` for implementation details
    - Removed `assume(False)` skip for M8 in test_masking.py
    - _Requirements: 6.1, 6.2_

- [x] 12. Update synthetic image generator
  - [x] 12.1 Add --masked flag to generate_synthetic_image.py
    - Enable masked output mode
    - Select appropriate IC (NM for uncompressed, M8 for J2K)
    - _Requirements: 9.1_
  
  - [x] 12.2 Add --mask-pattern argument
    - Support patterns: checkerboard, border, random, single
    - _Requirements: 9.2_
  
  - [x] 12.3 Add --mask-ratio argument for random pattern
    - Specify fraction of blocks to mask (0.0-1.0)
    - _Requirements: 9.2_
  
  - [x] 12.4 Update ImageWriter to use selective set_block()
    - Only call set_block() for non-masked blocks based on pattern
    - _Requirements: 9.3_

- [x] 13. Update API documentation
  - [x] 13.1 Add masked image writing example to docs/API_DESIGN.md
    - Show how to create sparse images with masked IC
    - Show how to iterate over valid blocks when reading
    - _Requirements: 2.5_

- [x] 14. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation builds on existing Phase 5 J2K support for M8/MD variants
