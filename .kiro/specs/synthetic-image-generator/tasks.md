# Implementation Plan: Synthetic Image Generator

## Overview

This implementation plan creates a Python script utility for generating synthetic NITF test images. The script uses the existing IO library Python bindings to create tiled images with configurable dimensions, tile sizes, band configurations, pixel types, and interleave modes. Each tile displays a checkerboard pattern with a unique tile ID.

## Tasks

- [x] 1. Create script skeleton and CLI
  - [x] 1.1 Create `scripts/generate_synthetic_image.py` with argparse CLI
    - Add positional argument for output path
    - Add optional arguments for width, height, tile-width, tile-height
    - Add optional arguments for bands, pixel-type, abpp, imode
    - Set sensible defaults (512x512, 256x256 tiles, 1 band, uint8, IMODE B)
    - Add help text for all options
    - _Requirements: 1.1-1.8_

  - [x] 1.2 Implement ImageConfig dataclass
    - Create dataclass with all configuration fields
    - Implement validation in `__post_init__`
    - Add computed properties (numpy_dtype, max_pixel_value, num_tiles_x/y)
    - _Requirements: 1.7, 5.3-5.5_

- [-] 2. Implement checkerboard pattern generation
  - [x] 2.1 Implement CheckerboardPattern class
    - Define light and dark base colors
    - Implement `get_tile_color()` method
    - Handle grayscale vs multi-band color selection
    - Scale colors to configured bit depth
    - _Requirements: 7.1, 7.3, 7.5_

  - [ ]* 2.2 Write property test for checkerboard alternation
    - **Property 3: Checkerboard Pattern Alternation**
    - Generate random tile positions
    - Verify adjacent tiles have different colors
    - **Validates: Requirements 7.1**

- [-] 3. Implement tile ID rendering
  - [x] 3.1 Implement TileIDRenderer class
    - Define 5x7 bitmap font patterns for digits 0-9
    - Implement `render_id()` method
    - Calculate centered position for text
    - Handle tiles too small for text
    - _Requirements: 8.1-8.5_

  - [ ]* 3.2 Write property test for tile ID contrast
    - **Property 6: Tile ID Text Contrast**
    - Generate tiles with various background colors
    - Verify text color contrasts with background
    - **Validates: Requirements 8.2**

- [x] 4. Implement tile generation
  - [x] 4.1 Implement TileGenerator class
    - Implement `generate_tile()` method
    - Calculate actual tile dimensions for edge tiles
    - Fill tile with checkerboard background
    - Render tile ID in center
    - _Requirements: 2.1-2.4, 3.1-3.4_

  - [ ]* 4.2 Write property test for tile dimensions
    - **Property 1: Image Dimensions Match Configuration**
    - Generate random dimensions and tile sizes
    - Verify output dimensions match configuration
    - **Validates: Requirements 2.1, 2.3**

  - [ ]* 4.3 Write property test for pixel value range
    - **Property 2: Pixel Values Within Configured Range**
    - Generate tiles with various ABPP settings
    - Verify all pixel values are within range
    - **Validates: Requirements 5.1, 5.2, 5.4**

- [x] 5. Implement image writing
  - [x] 5.1 Implement ImageWriter class
    - Implement `_generate_full_image()` to assemble tiles
    - Implement `_to_bytes()` for band-sequential conversion
    - Implement `write_image()` using IO library
    - _Requirements: 10.1-10.4_

  - [x] 5.2 Add error handling and exit codes
    - Wrap IO operations in try/except
    - Add context to error messages
    - Return appropriate exit codes
    - _Requirements: 11.1-11.4_

- [x] 6. Checkpoint - Verify basic functionality
  - Ensure script runs with default parameters
  - Ensure generated file can be opened with IO.open()
  - Ask the user if questions arise

- [x] 7. Implement band configurations
  - [x] 7.1 Add band metadata support
    - Set IREP based on band count (MONO, RGB, MULTI)
    - Generate appropriate IREPBAND values
    - _Requirements: 4.1-4.4_

  - [ ]* 7.2 Write property test for all bands populated
    - **Property 8: All Bands Populated**
    - Generate multi-band images
    - Verify all bands contain non-zero data
    - **Validates: Requirements 4.5**

- [x] 8. Implement IMODE support
  - [x] 8.1 Add IMODE configuration to ImageWriter
    - Pass IMODE to IO library
    - Verify correct IMODE in output file
    - _Requirements: 6.1-6.5_

- [x] 9. Final checkpoint - Full integration test
  - Test all parameter combinations
  - Verify generated files with describe_dataset.py
  - Ensure all tests pass, ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- The script uses existing IO library bindings - no new Rust code
- Property tests use hypothesis for randomized input generation
- This utility serves as a functional checkpoint for the IO library
