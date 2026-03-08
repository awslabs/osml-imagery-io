# Requirements Document

## Introduction

This document specifies the requirements for implementing JPEG DCT (Discrete Cosine Transform) compression support in the osml-imagery-io library. JPEG DCT is a lossy compression format defined in the Joint BIIF Profile (JBP) specification for NITF imagery. This feature enables reading and writing NITF files with IC (Image Compression) codes C3, M3, and I1, completing Phase 3 of the JBP implementation roadmap.

JPEG DCT compression is required for full CLEVEL conformance across all complexity levels (03-07) and is commonly used for thumbnail imagery and legacy NITF files.

## Glossary

- **JPEG_DCT_Codec**: The component responsible for encoding and decoding JPEG DCT compressed image data using a third-party Rust library.
- **IC_Code**: The Image Compression field in the NITF image subheader indicating the compression type (C3, M3, I1).
- **COMRAT**: The Compression Rate field specifying quality settings for lossy compression.
- **IMODE**: The Image Mode field indicating band interleaving (B=block, P=pixel, R=row, S=sequential).
- **Block**: A rectangular tile of image data that is independently compressed.
- **Mask_Table**: A data structure indicating which blocks contain valid data in masked images.
- **JBP_Image_Writer**: The existing component responsible for writing NITF image segments.
- **JBP_Image_Reader**: The existing component responsible for reading NITF image segments.
- **JPEG_Block_Encoder**: The component that compresses a single block of image data to JPEG format.
- **JPEG_Block_Decoder**: The component that decompresses a single JPEG block to raw pixel data.

## Requirements

### Requirement 1: JPEG DCT Decoding

**User Story:** As a developer, I want to read NITF files with JPEG DCT compression (IC=C3), so that I can access legacy imagery and thumbnails stored in this format.

#### Acceptance Criteria

1. WHEN a NITF file with IC=C3 is opened, THE JBP_Image_Reader SHALL decode JPEG DCT compressed blocks and return raw pixel data.
2. WHEN decoding 8-bit monochrome JPEG blocks, THE JPEG_Block_Decoder SHALL produce UInt8 pixel arrays with correct dimensions.
3. WHEN decoding 12-bit monochrome JPEG blocks, THE JPEG_Block_Decoder SHALL return a clear error message explaining that 12-bit JPEG is not supported and suggesting alternatives.
4. WHEN decoding RGB 24-bit JPEG blocks (IMODE=P), THE JPEG_Block_Decoder SHALL produce 3-band UInt8 pixel arrays in BSQ format (bands, rows, cols).
5. WHEN decoding YCbCr601 24-bit JPEG blocks (IMODE=P), THE JPEG_Block_Decoder SHALL convert to RGB color space and produce 3-band UInt8 pixel arrays.
6. WHEN decoding multiband JPEG (IMODE=B or S), THE JPEG_Block_Decoder SHALL decode each band independently and combine into a multi-band array.

### Requirement 2: JPEG DCT Encoding

**User Story:** As a developer, I want to write NITF files with JPEG DCT compression (IC=C3), so that I can create compressed imagery compatible with legacy systems.

#### Acceptance Criteria

1. WHEN writing an image with IC=C3 metadata hint, THE JBP_Image_Writer SHALL encode blocks using JPEG DCT compression.
2. WHEN encoding 8-bit monochrome images, THE JPEG_Block_Encoder SHALL produce valid JPEG bitstreams.
3. WHEN encoding 12-bit monochrome images, THE JPEG_Block_Encoder SHALL return a clear error message explaining that 12-bit JPEG is not supported and suggesting alternatives.
4. WHEN encoding 3-band RGB images (IMODE=P), THE JPEG_Block_Encoder SHALL produce pixel-interleaved JPEG bitstreams.
5. WHEN encoding 3-band images with YCbCr601 color space hint, THE JPEG_Block_Encoder SHALL convert from RGB to YCbCr601 before compression.
6. WHEN encoding multiband images (IMODE=B or S), THE JPEG_Block_Encoder SHALL encode each band as a separate JPEG bitstream.

### Requirement 3: Masked JPEG DCT Images

**User Story:** As a developer, I want to read and write masked JPEG DCT images (IC=M3), so that I can handle sparse imagery with missing blocks.

#### Acceptance Criteria

1. WHEN a NITF file with IC=M3 is opened, THE JBP_Image_Reader SHALL parse the mask table and identify valid blocks.
2. WHEN has_block() is called for a masked block, THE JBP_Image_Reader SHALL return false.
3. WHEN has_block() is called for a valid block, THE JBP_Image_Reader SHALL return true and get_block() SHALL decode the JPEG data.
4. WHEN writing an image with IC=M3 metadata hint, THE JBP_Image_Writer SHALL generate a valid mask table based on provided blocks.
5. WHEN blocks are omitted during writing with IC=M3, THE JBP_Image_Writer SHALL mark those blocks as masked in the mask table.

### Requirement 4: Downsampled JPEG (IC=I1)

**User Story:** As a developer, I want to read and write downsampled JPEG images (IC=I1), so that I can handle thumbnail imagery in NITF files.

#### Acceptance Criteria

1. WHEN a NITF file with IC=I1 is opened, THE JBP_Image_Reader SHALL decode the single-block JPEG image.
2. WHEN reading IC=I1 images, THE JBP_Image_Reader SHALL enforce the 2048×2048 maximum dimension constraint.
3. WHEN writing an image with IC=I1 metadata hint, THE JBP_Image_Writer SHALL encode as a single JPEG block.
4. IF an image exceeds 2048×2048 pixels with IC=I1, THEN THE JBP_Image_Writer SHALL return an error indicating the dimension constraint violation.

### Requirement 5: COMRAT Handling

**User Story:** As a developer, I want to control JPEG compression quality through the COMRAT field, so that I can balance file size and image quality.

#### Acceptance Criteria

1. WHEN reading a JPEG DCT image, THE JBP_Image_Reader SHALL parse the COMRAT field and expose it via metadata.
2. WHEN writing with a COMRAT metadata hint, THE JPEG_Block_Encoder SHALL use the specified quality setting.
3. WHEN COMRAT specifies a quality factor (e.g., "00.0" to "99.9"), THE JPEG_Block_Encoder SHALL map it to the JPEG quality parameter.
4. IF no COMRAT is specified during writing, THEN THE JPEG_Block_Encoder SHALL use a default quality of 75.

### Requirement 6: Third-Party Library Integration

**User Story:** As a maintainer, I want JPEG encoding/decoding to use a permissively-licensed third-party library, so that the project remains Apache 2.0 compatible.

#### Acceptance Criteria

1. THE JPEG_DCT_Codec SHALL use a Rust library licensed under MIT, Apache 2.0, or BSD.
2. THE JPEG_DCT_Codec SHALL NOT use any GPL or LGPL licensed dependencies.
3. THE JPEG_DCT_Codec SHALL support 8-bit baseline JPEG encoding and decoding.
4. THE JPEG_DCT_Codec SHALL return a clear error message when 12-bit JPEG is requested, explaining the limitation and suggesting alternatives (JPEG 2000 or uncompressed).

**Note**: 12-bit JPEG support is not implemented due to architectural constraints in libjpeg-turbo. The TurboJPEG API only supports 8-bit samples, and 12-bit support would require linking against a separately compiled libjpeg12 library with different symbol names.

### Requirement 7: Lossy Roundtrip Quality

**User Story:** As a developer, I want JPEG DCT compression to maintain acceptable image quality, so that compressed imagery remains usable for analysis.

#### Acceptance Criteria

1. WHEN encoding then decoding an image with default quality settings, THE JPEG_DCT_Codec SHALL produce output with PSNR >= 30 dB.
2. WHEN encoding then decoding an image with default quality settings, THE JPEG_DCT_Codec SHALL produce output with SSIM >= 0.95.
3. WHEN encoding then decoding an image, THE JPEG_DCT_Codec SHALL preserve the original image dimensions exactly.
4. WHEN encoding then decoding an image, THE JPEG_DCT_Codec SHALL preserve the original pixel type (8-bit or 12-bit).

### Requirement 8: Documentation Updates

**User Story:** As a developer, I want updated documentation reflecting JPEG DCT support, so that I can understand how to use the new compression features.

#### Acceptance Criteria

1. WHEN Phase 3 is complete, THE JBP_CLEVEL_ASSESSMENT.md SHALL be updated to mark JPEG DCT features as implemented.
2. WHEN Phase 3 is complete, THE API_DESIGN.md SHALL include JPEGImageAssetProvider documentation.
3. WHEN Phase 3 is complete, THE JBP_ROADMAP.md SHALL mark Phase 3 as complete.
