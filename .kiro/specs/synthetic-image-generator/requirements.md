# Requirements Document

## Introduction

This document specifies the requirements for a synthetic image generator utility for the AWS OSML IO project. The utility generates test images using the existing Python bindings and DatasetWriter abstractions to create NITF files with configurable dimensions, tile sizes, band configurations, pixel types, and interleave modes. Each generated image contains a checkerboard test pattern with tile IDs for visual verification of pixel correctness.

## Glossary

- **Synthetic_Image_Generator**: The Python script utility that creates test NITF images with configurable parameters
- **DatasetWriter**: The existing Python binding that writes NITF/NSIF files via the IO.open() factory
- **Tile**: A rectangular block of pixels within the image, corresponding to NITF blocking parameters (NPPBH × NPPBV)
- **IMODE**: Image interleave mode specifying how multi-band pixel data is organized (B, P, R, S)
- **Pixel_Type**: The numeric data type for pixel values (uint8, uint16)
- **Band_Configuration**: The number and arrangement of spectral bands (grayscale=1, RGB=3, multispectral=5)
- **Checkerboard_Pattern**: A test pattern where adjacent tiles have alternating colors for visual verification
- **Tile_ID**: A numeric identifier rendered in the center of each tile for verification

## Requirements

### Requirement 1: Command-Line Interface

**User Story:** As a developer, I want to run the synthetic image generator from the command line with configurable parameters, so that I can create test images for various scenarios.

#### Acceptance Criteria

1. THE Synthetic_Image_Generator SHALL accept command-line arguments for output file path
2. THE Synthetic_Image_Generator SHALL accept command-line arguments for image dimensions (width and height in pixels)
3. THE Synthetic_Image_Generator SHALL accept command-line arguments for tile size (width and height in pixels)
4. THE Synthetic_Image_Generator SHALL accept command-line arguments for band configuration (1, 3, or 5 bands)
5. THE Synthetic_Image_Generator SHALL accept command-line arguments for pixel type (uint8 or uint16)
6. THE Synthetic_Image_Generator SHALL accept command-line arguments for IMODE (B, P, R, or S)
7. THE Synthetic_Image_Generator SHALL provide sensible default values for all optional parameters
8. THE Synthetic_Image_Generator SHALL display help text describing all available options

### Requirement 2: Image Dimension Support

**User Story:** As a developer, I want to generate images of various dimensions, so that I can test the IO library with different image sizes.

#### Acceptance Criteria

1. WHEN image dimensions are specified, THE Synthetic_Image_Generator SHALL create an image with the exact requested width and height
2. THE Synthetic_Image_Generator SHALL support image dimensions that are exact multiples of the tile size
3. THE Synthetic_Image_Generator SHALL support image dimensions that are not exact multiples of the tile size (partial edge tiles)
4. THE Synthetic_Image_Generator SHALL support minimum image dimensions of 1×1 pixels
5. THE Synthetic_Image_Generator SHALL NOT impose arbitrary upper limits on image dimensions

### Requirement 3: Tile Size Configuration

**User Story:** As a developer, I want to configure tile sizes, so that I can test the blocking/tiling functionality of the IO library.

#### Acceptance Criteria

1. WHEN tile size is specified, THE Synthetic_Image_Generator SHALL use the exact requested tile dimensions
2. THE Synthetic_Image_Generator SHALL support tile sizes from 16×16 to 2048×2048 pixels
3. THE Synthetic_Image_Generator SHALL support non-square tiles (different width and height)
4. WHEN tile size exceeds image dimensions, THE Synthetic_Image_Generator SHALL use a single tile for the entire image

### Requirement 4: Band Configuration Support

**User Story:** As a developer, I want to generate images with different numbers of bands, so that I can test grayscale, RGB, and multispectral imagery.

#### Acceptance Criteria

1. WHEN band count is 1, THE Synthetic_Image_Generator SHALL create a grayscale image
2. WHEN band count is 3, THE Synthetic_Image_Generator SHALL create an RGB image with bands representing Red, Green, and Blue
3. WHEN band count is 5, THE Synthetic_Image_Generator SHALL create a multispectral image
4. THE Synthetic_Image_Generator SHALL generate appropriate band metadata (IREPBAND) for each band configuration
5. FOR ALL band configurations, THE Synthetic_Image_Generator SHALL generate pixel data for all bands

### Requirement 5: Pixel Type Support

**User Story:** As a developer, I want to generate images with different pixel types and bit depths, so that I can test 8-bit, 11-bit, and 16-bit imagery handling.

#### Acceptance Criteria

1. WHEN pixel type is uint8, THE Synthetic_Image_Generator SHALL generate 8-bit unsigned integer pixel values (0-255)
2. WHEN pixel type is uint16, THE Synthetic_Image_Generator SHALL generate 16-bit unsigned integer pixel values (0-65535)
3. THE Synthetic_Image_Generator SHALL accept an optional ABPP (actual bits per pixel) parameter
4. WHEN ABPP is specified, THE Synthetic_Image_Generator SHALL constrain pixel values to the specified bit depth
5. THE Synthetic_Image_Generator SHALL support ABPP values such as 11 bits for uint16 storage
6. THE Synthetic_Image_Generator SHALL set appropriate NBPP and ABPP values in the image subheader
7. THE Synthetic_Image_Generator SHALL set appropriate PVTYPE value in the image subheader

### Requirement 6: IMODE Configuration

**User Story:** As a developer, I want to generate images with different interleave modes, so that I can test all IMODE configurations.

#### Acceptance Criteria

1. WHEN IMODE is B, THE Synthetic_Image_Generator SHALL organize pixel data as band-interleaved-by-block
2. WHEN IMODE is P, THE Synthetic_Image_Generator SHALL organize pixel data as band-interleaved-by-pixel
3. WHEN IMODE is R, THE Synthetic_Image_Generator SHALL organize pixel data as band-interleaved-by-row
4. WHEN IMODE is S, THE Synthetic_Image_Generator SHALL organize pixel data as band-sequential
5. THE Synthetic_Image_Generator SHALL set the IMODE field correctly in the image subheader

### Requirement 7: Checkerboard Test Pattern

**User Story:** As a developer, I want each tile to have a distinct checkerboard color pattern, so that I can visually verify that pixels are correctly positioned.

#### Acceptance Criteria

1. THE Synthetic_Image_Generator SHALL assign alternating colors to adjacent tiles in a checkerboard pattern
2. THE Synthetic_Image_Generator SHALL use visually distinct colors for the checkerboard pattern
3. FOR ALL tiles, THE Synthetic_Image_Generator SHALL fill the tile with a solid color (except for the tile ID text)
4. THE Synthetic_Image_Generator SHALL use colors that are distinguishable in both grayscale and color modes
5. WHEN pixel type is uint16, THE Synthetic_Image_Generator SHALL scale colors appropriately to use the full dynamic range

### Requirement 8: Tile ID Rendering

**User Story:** As a developer, I want each tile to display its tile ID in the center, so that I can verify tile ordering and positioning.

#### Acceptance Criteria

1. THE Synthetic_Image_Generator SHALL render a numeric tile ID in the center of each tile
2. THE Synthetic_Image_Generator SHALL use a contrasting color for the tile ID text relative to the tile background
3. THE Synthetic_Image_Generator SHALL number tiles sequentially starting from 0
4. THE Synthetic_Image_Generator SHALL use row-major ordering for tile numbering (left-to-right, top-to-bottom)
5. WHEN tile size is too small to render text, THE Synthetic_Image_Generator SHALL skip the tile ID rendering

### Requirement 9: NumPy-Based Pixel Generation

**User Story:** As a developer, I want all pixel data generated using NumPy, so that the utility uses standard Python scientific computing tools.

#### Acceptance Criteria

1. THE Synthetic_Image_Generator SHALL use NumPy arrays for all pixel data generation
2. THE Synthetic_Image_Generator SHALL use appropriate NumPy dtypes matching the requested pixel type
3. THE Synthetic_Image_Generator SHALL generate pixel arrays with shape (height, width, bands) for multi-band images
4. THE Synthetic_Image_Generator SHALL convert NumPy arrays to bytes for the DatasetWriter

### Requirement 10: Integration with Existing IO Abstractions

**User Story:** As a developer, I want the generator to use the existing DatasetWriter API, so that it serves as a functional test of the IO library.

#### Acceptance Criteria

1. THE Synthetic_Image_Generator SHALL use IO.open() to create the DatasetWriter
2. THE Synthetic_Image_Generator SHALL use AssetProvider.from_bytes() to create image assets
3. THE Synthetic_Image_Generator SHALL use DatasetWriter.add_asset() to add images to the file
4. THE Synthetic_Image_Generator SHALL use DatasetWriter.close() to finalize the file
5. IF the IO library has bugs or missing features, THE Synthetic_Image_Generator SHALL expose them through normal usage

### Requirement 11: Error Handling

**User Story:** As a developer, I want clear error messages when generation fails, so that I can diagnose issues with the IO library.

#### Acceptance Criteria

1. IF invalid parameters are provided, THE Synthetic_Image_Generator SHALL display a descriptive error message
2. IF the IO library raises an exception, THE Synthetic_Image_Generator SHALL propagate the error with context
3. IF file writing fails, THE Synthetic_Image_Generator SHALL report the failure reason
4. THE Synthetic_Image_Generator SHALL return a non-zero exit code on failure
