# Implementation Plan: Property-Based Testing Framework

## Overview

This plan implements a comprehensive property-based testing framework for osml-imagery-io. The implementation creates reusable hypothesis strategies, quality metrics utilities, and property tests organized in a dedicated `tests/property/` directory. The framework validates image codec correctness through generated test cases covering roundtrips, block access, and metadata preservation.

## Tasks

- [ ] 1. Set up test infrastructure and shared utilities
  - [ ] 1.1 Create tests/property/ directory structure with __init__.py
    - Create tests/property/__init__.py
    - _Requirements: 9.1_
  
  - [ ] 1.2 Create conftest.py with shared fixtures and pytest configuration
    - Add temp_nitf_path fixture for temporary file handling
    - Add hypothesis_settings fixture with max_examples=100, deadline=None
    - Register 'property' pytest marker
    - _Requirements: 9.2, 9.4, 9.5_
  
  - [ ] 1.3 Create quality.py with PSNR and SSIM calculation functions
    - Implement calculate_psnr() function
    - Implement calculate_ssim() function (use scikit-image if available)
    - Define MIN_PSNR_DB = 30.0 and MIN_SSIM = 0.95 constants
    - _Requirements: 3.2, 3.3_

- [ ] 2. Implement image generation strategies
  - [ ] 2.1 Create strategies.py with core strategy functions
    - Implement pixel_types() strategy
    - Implement image_dimensions() strategy with min/max bounds
    - Implement band_counts() strategy
    - Implement block_sizes() strategy
    - _Requirements: 1.1, 1.2, 1.3, 1.4_
  
  - [ ] 2.2 Implement image_arrays() and random_image() composite strategies
    - Implement image_arrays() using hypothesis.extra.numpy.arrays
    - Implement random_image() composite strategy returning (array, pixel_type, bands, rows, cols)
    - _Requirements: 1.1_
  
  - [ ] 2.3 Implement edge_case_images() strategy
    - Add single-pixel image generator
    - Add single-band image generator
    - Add max-value image generator
    - Add min-value image generator
    - Add gradient image generator
    - Add random noise image generator
    - _Requirements: 1.5_
  
  - [ ] 2.4 Implement valid_block_coordinates() strategy
    - Calculate num_block_rows and num_block_cols from image/block dimensions
    - Return strategy for (row, col) tuples within valid range
    - _Requirements: 1.6_
  
  - [ ] 2.5 Implement metadata strategies
    - Implement nitf_field_names() strategy (uppercase alphanumeric, 1-10 chars)
    - Implement metadata_values() strategy
    - _Requirements: 5.2_
  
  - [ ]* 2.6 Write property tests for strategy validity
    - **Property 1: Image Strategy Configuration Consistency**
    - **Property 2: Block Strategy Coordinate Validity**
    - **Property 9: Metadata Strategy Validity**
    - **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.6, 5.2**

- [ ] 3. Checkpoint - Verify strategies work correctly
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement roundtrip property tests
  - [ ] 4.1 Create test_roundtrip.py with test class structure
    - Create TestLosslessRoundtrip class
    - Create TestLossyRoundtrip class
    - Create TestIdempotentEncoding class
    - _Requirements: 2.1, 3.1, 6.1, 6.2_
  
  - [ ]* 4.2 Write property test for lossless roundtrip
    - **Property 3: Lossless Roundtrip Preservation**
    - Test with IC=NC (uncompressed)
    - Test with COMRAT=N001.0 (JPEG 2000 lossless)
    - Verify exact equality of pixel values, shape, and dtype
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**
  
  - [ ]* 4.3 Write property test for lossy roundtrip with quality bounds
    - **Property 4: Lossy Roundtrip Quality Bounds**
    - Test with lossy JPEG 2000 compression
    - Verify PSNR >= 30 dB and SSIM >= 0.95
    - Verify shape and dtype preservation
    - **Validates: Requirements 3.1, 3.4, 3.5**
  
  - [ ]* 4.4 Write property tests for idempotent encoding
    - **Property 10: Idempotent Encoding (Byte-Level)**
    - **Property 11: Idempotent Encoding (Value-Level)**
    - **Validates: Requirements 6.1, 6.2**

- [ ] 5. Implement block access property tests
  - [ ] 5.1 Create test_block_access.py with test class structure
    - Create TestBlockAccessCompleteness class
    - Create TestBlockReassembly class
    - Create TestResolutionLevels class
    - _Requirements: 4.1, 4.3, 7.1_
  
  - [ ]* 5.2 Write property test for block access completeness
    - **Property 5: Block Access Completeness**
    - Generate random images and block coordinates
    - Verify get_block succeeds for all valid coordinates
    - Verify returned block shape is correct
    - **Validates: Requirements 4.1, 4.2**
  
  - [ ]* 5.3 Write property test for block reassembly roundtrip
    - **Property 6: Block Reassembly Roundtrip**
    - Read all blocks from an image
    - Reassemble into full array
    - Verify equality with original
    - **Validates: Requirements 4.3**
  
  - [ ]* 5.4 Write property test for invalid block coordinate error handling
    - **Property 7: Invalid Block Coordinate Error Handling**
    - Generate coordinates outside valid range
    - Verify appropriate error is raised
    - **Validates: Requirements 4.4**
  
  - [ ]* 5.5 Write property test for resolution level consistency
    - **Property 12: Resolution Level Consistency**
    - Verify dimension reduction by 2^N at level N
    - Verify block shapes at each level
    - **Validates: Requirements 7.1, 7.2, 7.3**

- [ ] 6. Implement metadata property tests
  - [ ] 6.1 Create test_metadata.py with test class structure
    - Create TestMetadataRoundtrip class
    - _Requirements: 5.1_
  
  - [ ]* 6.2 Write property test for metadata roundtrip preservation
    - **Property 8: Metadata Roundtrip Preservation**
    - Generate random metadata key-value pairs
    - Attach to image, encode, decode
    - Verify all metadata preserved
    - **Validates: Requirements 5.1, 5.3**

- [ ] 7. Checkpoint - Verify all property tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Create documentation
  - [ ] 8.1 Create docs/PROPERTY_BASED_TESTING.md
    - Write introduction explaining PBT rationale for image codecs
    - Document property categories (roundtrip, structural, API contract)
    - Document quality thresholds (PSNR >= 30 dB, SSIM >= 0.95)
    - Include references to prior art (PyTorch Vision, Hypothesis articles)
    - Explain how to add new properties and strategies
    - Document test organization (tests/property/ structure)
    - Include commands for running property tests
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6_
  
  - [ ] 8.2 Update README.md Testing section
    - Add property-based testing subsection under Testing
    - Document `pytest -m property` command for running property tests
    - Document `pytest -m "not property"` for running only unit tests
    - Add brief explanation of PBT purpose with link to docs/PROPERTY_BASED_TESTING.md
    - _Requirements: 8.1, 9.5_
  
  - [ ] 8.3 Update .kiro/steering/tech.md with property testing commands
    - Add property test commands to Common Commands section
    - Document hypothesis and proptest as key dependencies
    - Add test marker documentation
    - _Requirements: 9.4, 9.5_
  
  - [ ] 8.4 Update .kiro/steering/structure.md with test organization
    - Add tests/property/ directory to project structure
    - Document the purpose of each file (strategies.py, quality.py, conftest.py)
    - Explain relationship between property tests and unit tests
    - _Requirements: 9.1, 9.2, 9.3_

- [ ] 9. Final checkpoint - Verify complete framework
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each property test references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests (existing) validate specific examples and edge cases
- The framework builds on existing PBT patterns in the codebase (see tests/test_api_design_alignment_pbt.py)
