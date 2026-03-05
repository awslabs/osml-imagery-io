# Requirements Document

## Introduction

This document specifies the requirements for Phase 6: Image Masking in the osml-imagery-io library. Image masking enables NITF files to contain sparse or irregular imagery where some blocks may be empty (masked out) or contain pad pixels. This feature is essential for efficiently storing imagery that doesn't fill a complete rectangular grid, such as satellite imagery with irregular boundaries or imagery with cloud-covered regions.

The implementation builds on the existing JBP dataset reader/writer infrastructure (Phases 0-5) and adds support for the Image Data Mask table defined in JBP Table 5.13-9, along with masked IC (Image Compression) values.

## Glossary

- **Image_Data_Mask**: A binary structure preceding image data that contains block offsets and pad pixel information for masked images
- **Block_Mask**: A table of offsets indicating where each block's data begins, with 0xFFFFFFFF indicating an empty (masked) block
- **Pad_Pixel_Mask**: A table indicating which pixels within blocks are pad pixels (fill values)
- **Pad_Pixel**: A pixel value used to fill areas outside the actual image boundary within a block
- **IC_Field**: The Image Compression field in the NITF image subheader indicating compression type and masking
- **IMDATOFF**: Image Data Offset - the offset from the beginning of the Image Data Mask to the start of actual image data
- **BMRLNTH**: Block Mask Record Length - the length in bytes of each block mask record
- **TMRLNTH**: Pad Pixel Mask Record Length - the length in bytes of each pad pixel mask record
- **TPXCDLNTH**: Pad Pixel Code Length - the number of bits used to represent the pad pixel code
- **TPXCD**: Pad Pixel Code - the actual pad pixel value
- **BMRnBNDm**: Block Mask Record for block n, band m - contains the offset to block data
- **TMRnBNDm**: Pad Pixel Mask Record for block n, band m - contains pad pixel offset information
- **Empty_Block**: A block with offset 0xFFFFFFFF indicating no image data exists for that block
- **JBPImageAssetProvider**: The Rust struct that provides block-based access to NITF image segments
- **JBPDatasetWriter**: The Rust struct that writes NITF files with image segments
- **BufferedImageAssetProvider**: An in-memory image provider used for creating images programmatically

## Requirements

### Requirement 1: Image Data Mask Parsing

**User Story:** As a developer, I want to read NITF files with masked images, so that I can access sparse imagery where some blocks may be empty or contain pad pixels.

#### Acceptance Criteria

1. WHEN a NITF file with a masked IC value (NM, M1, M3, M4, M5, M7, M8, M9, MA, MB, MC, MD, ME) is opened, THE JBPDatasetReader SHALL parse the Image Data Mask table from the image segment
2. WHEN parsing the Image Data Mask, THE Parser SHALL extract IMDATOFF, BMRLNTH, TMRLNTH, TPXCDLNTH, and TPXCD fields according to JBP Table 5.13-9
3. WHEN BMRLNTH is greater than zero, THE Parser SHALL read BMRnBNDm offset values for all blocks and bands
4. WHEN TMRLNTH is greater than zero, THE Parser SHALL read TMRnBNDm pad pixel offset values for all blocks and bands
5. WHEN a block offset value equals 0xFFFFFFFF, THE Parser SHALL recognize this as an empty (masked) block
6. WHEN parsing completes, THE ImageDataMask struct SHALL provide methods to query block offsets and pad pixel information

### Requirement 2: Masked Block Access

**User Story:** As a developer, I want to query whether a block exists before attempting to read it, so that I can efficiently iterate over only the valid blocks in a sparse image.

#### Acceptance Criteria

1. WHEN has_block() is called for a block with offset 0xFFFFFFFF, THE JBPImageAssetProvider SHALL return false
2. WHEN has_block() is called for a block with a valid offset, THE JBPImageAssetProvider SHALL return true
3. WHEN get_block() is called for a masked (empty) block, THE JBPImageAssetProvider SHALL raise an appropriate error
4. WHEN get_block() is called for a valid block in a masked image, THE JBPImageAssetProvider SHALL decode and return the block data using the offset from the block mask
5. WHEN iterating over blocks, THE Consumer SHALL be able to use has_block() to skip masked blocks without errors

### Requirement 3: Pad Pixel Handling

**User Story:** As a developer, I want pad pixels to be correctly identified and handled, so that I can distinguish between actual image data and fill values.

#### Acceptance Criteria

1. WHEN a masked image has TPXCDLNTH greater than zero, THE JBPImageAssetProvider SHALL expose the pad pixel value via pad_pixel_value property
2. WHEN reading blocks from a masked image with pad pixel masks, THE Decoder SHALL correctly identify pad pixel regions
3. WHEN the pad pixel code is defined, THE ImageAssetProvider SHALL return the pad pixel value that matches the TPXCD field

### Requirement 4: Image Data Mask Writing

**User Story:** As a developer, I want to write NITF files with masked images, so that I can efficiently store sparse imagery without wasting space on empty blocks.

#### Acceptance Criteria

1. WHEN writing a masked image, THE JBPDatasetWriter SHALL generate a valid Image Data Mask table
2. WHEN blocks are provided via set_block(), THE Writer SHALL record their offsets in the block mask
3. WHEN a block is not provided (sparse data), THE Writer SHALL record 0xFFFFFFFF as the block offset
4. WHEN all blocks are provided, THE Writer SHALL calculate IMDATOFF as the size of the mask table
5. WHEN writing completes, THE Output file SHALL contain a valid Image Data Mask that can be parsed by the reader

### Requirement 5: Sparse Block Data Handling

**User Story:** As a developer, I want to provide only the blocks that contain actual image data when using a masked IC value, so that I can efficiently create sparse imagery.

#### Acceptance Criteria

1. WHEN a masked IC value is set AND a BufferedImageAssetProvider has missing blocks (not all blocks set), THE Writer SHALL accept the sparse data
2. WHEN generating the mask, THE Writer SHALL set block offsets based on which blocks were provided via set_block()
3. WHEN a block was not provided via set_block(), THE Writer SHALL mark it as empty (0xFFFFFFFF offset)
4. WHEN all blocks are provided with a masked IC value, THE Writer SHALL still generate a valid mask table with all blocks having valid offsets

### Requirement 6: Masked JPEG 2000 Support

**User Story:** As a developer, I want to read and write JPEG 2000 compressed images with masks (M8, MD), so that I can work with compressed sparse imagery.

#### Acceptance Criteria

1. WHEN a NITF file has IC=M8 (JPEG 2000 Part 1 with masks), THE Reader SHALL parse the mask table and decode J2K blocks using offsets from the mask
2. WHEN a NITF file has IC=MD (HTJ2K with masks), THE Reader SHALL parse the mask table and decode HTJ2K blocks using offsets from the mask
3. WHEN writing a sparse image with J2K compression, THE Writer SHALL use IC=M8 and generate appropriate mask tables
4. WHEN writing a sparse image with HTJ2K compression, THE Writer SHALL use IC=MD and generate appropriate mask tables
5. WHEN a masked J2K image has empty blocks, THE Reader SHALL return false from has_block() for those blocks

### Requirement 7: IC Field Selection and Validation

**User Story:** As a developer, I want to explicitly choose the IC value and have the writer validate that my block data is consistent with that choice, so that I have full control over the output format.

#### Acceptance Criteria

1. WHEN the user sets a masked IC value (NM, M1, M3, M4, M5, M7, M8, M9, MA, MB, MC, MD, ME) via metadata, THE Writer SHALL accept sparse block data and generate appropriate masks
2. WHEN the user sets a non-masked IC value (NC, C8, CD) via metadata, THE Writer SHALL require all blocks to be provided
3. IF a non-masked IC is set AND the user does not provide all expected blocks, THEN THE Writer SHALL raise a missing-block error during encoding
4. WHEN IC=NM is set, THE Writer SHALL write uncompressed image data with a block mask
5. WHEN IC=M8 is set, THE Writer SHALL write JPEG 2000 compressed data with a block mask
6. WHEN IC=MD is set, THE Writer SHALL write HTJ2K compressed data with a block mask

### Requirement 8: Roundtrip Consistency

**User Story:** As a developer, I want masked images to survive roundtrip encoding and decoding, so that I can trust the library preserves my data correctly.

#### Acceptance Criteria

1. FOR ALL valid masked images, writing then reading SHALL produce equivalent block data for all non-masked blocks
2. FOR ALL valid masked images, the set of masked blocks SHALL be preserved through roundtrip
3. FOR ALL valid masked images with pad pixels, the pad pixel value SHALL be preserved through roundtrip
4. WHEN a masked image is written and read back, THE has_block() results SHALL match the original sparse block pattern

### Requirement 9: Synthetic Masked Image Generation

**User Story:** As a developer, I want to generate synthetic masked images for testing, so that I can validate the masking implementation with known patterns.

#### Acceptance Criteria

1. WHEN --masked flag is provided to generate_synthetic_image.py, THE Script SHALL generate an image with some blocks masked out
2. WHEN --mask-pattern is provided, THE Script SHALL use the specified pattern (checkerboard, border, random) to determine which blocks are masked
3. WHEN generating a masked image, THE Script SHALL use BufferedImageAssetProvider with selective set_block() calls to create sparse data

### Requirement 10: Property Test Integration

**User Story:** As a developer, I want property-based tests to cover masked images, so that I can have confidence in the correctness of the masking implementation across many generated inputs.

#### Acceptance Criteria

1. WHEN generating test images, THE Hypothesis strategies SHALL include options for generating masked images
2. WHEN testing roundtrip properties, THE Tests SHALL verify that masked blocks remain masked after roundtrip
3. WHEN testing block access, THE Tests SHALL verify has_block() returns correct values for masked and non-masked blocks
4. WHEN testing with masked images, THE Tests SHALL verify that get_block() on valid blocks returns correct data
