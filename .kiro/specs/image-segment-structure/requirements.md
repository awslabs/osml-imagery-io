# Requirements Document

## Introduction

This document defines the requirements for Phase 4 of the JBP (Joint BIIF Profile) implementation project: Image Segment Structure. This phase implements parsing, validation, and writing of image subheaders, along with reading and writing uncompressed imagery with single and multi-band support. The implementation leverages the data-driven parser infrastructure from Phase 1 and integrates with the JBPDatasetReader/Writer from Phase 2.

## Glossary

- **Image_Subheader**: The header portion of a NITF image segment containing identification, security, dimensions, compression, and band information
- **Image_Segment**: A complete NITF image segment consisting of a subheader followed by image data
- **PVTYPE**: Pixel Value Type - indicates the data type of pixel values (INT, SI, R, C, B)
- **IREP**: Image Representation - describes how the image should be displayed (MONO, RGB, RGB/LUT, MULTI, etc.)
- **ICAT**: Image Category - describes the type of imagery (VIS, SL, TI, FL, RD, etc.)
- **NBPP**: Number of Bits Per Pixel - the storage size for each pixel value
- **ABPP**: Actual Bits Per Pixel - the number of significant bits in each pixel value
- **PJUST**: Pixel Justification - indicates whether pixel values are right or left justified
- **IMODE**: Image Mode - the interleave pattern for multi-band imagery (B, P, R, S)
- **NBPR**: Number of Blocks Per Row - horizontal blocking factor
- **NBPC**: Number of Blocks Per Column - vertical blocking factor
- **NPPBH**: Number of Pixels Per Block Horizontal - block width in pixels
- **NPPBV**: Number of Pixels Per Block Vertical - block height in pixels
- **Band_Info**: Per-band metadata including representation, subcategory, filter, and LUT information
- **IREPBANDn**: Band Representation - indicates the role of each band (R, G, B, M, LU, etc.)
- **ISUBCATn**: Band Subcategory - additional band classification
- **LUT**: Look-Up Table - maps pixel values to display values
- **NLUTSn**: Number of LUTs for a band
- **NELUTn**: Number of Entries in each LUT
- **LUTDnm**: LUT Data - the actual lookup table values
- **IC**: Image Compression - indicates the compression algorithm (NC, NM, C1, C3, C4, C5, C8, etc.)
- **COMRAT**: Compression Rate - compression ratio or quality parameter
- **CLEVEL**: Complexity Level - determines maximum image dimensions and file size
- **Structure_Accessor**: The lazy map-like interface from Phase 1 for reading parsed values
- **Structure_Writer**: The interface from Phase 1 for encoding values into binary format
- **JBPDatasetReader**: The dataset reader from Phase 2 for accessing NITF files
- **JBPDatasetWriter**: The dataset writer from Phase 2 for creating NITF files
- **ImageAssetProvider**: The trait interface for blocked/tiled image access

## Requirements

### Requirement 1: Image Subheader Parsing

**User Story:** As a developer, I want to parse image subheaders from NITF files, so that I can access image metadata and prepare for image data reading.

#### Acceptance Criteria

1. WHEN parsing an image subheader, THE Image_Parser SHALL extract the IM field and validate it equals "IM"
2. WHEN parsing an image subheader, THE Image_Parser SHALL extract image identifiers IID1 (10 chars) and IID2 (80 chars)
3. WHEN parsing an image subheader, THE Image_Parser SHALL extract IDATIM as a 14-character date/time string
4. WHEN parsing an image subheader, THE Image_Parser SHALL extract TGTID as a 17-character target identifier
5. WHEN parsing an image subheader, THE Image_Parser SHALL extract NROWS and NCOLS as image dimensions
6. WHEN parsing an image subheader, THE Image_Parser SHALL extract PVTYPE as a 3-character pixel value type
7. WHEN parsing an image subheader, THE Image_Parser SHALL extract IREP as an 8-character image representation
8. WHEN parsing an image subheader, THE Image_Parser SHALL extract ICAT as an 8-character image category
9. WHEN parsing an image subheader, THE Image_Parser SHALL extract ABPP and NBPP as bits per pixel values
10. WHEN parsing an image subheader, THE Image_Parser SHALL extract PJUST as pixel justification (R or L)

### Requirement 2: Blocking Parameter Parsing

**User Story:** As a developer, I want to parse image blocking parameters, so that I can understand the image's block structure for efficient access.

#### Acceptance Criteria

1. WHEN parsing blocking parameters, THE Image_Parser SHALL extract NBPR as the number of blocks per row
2. WHEN parsing blocking parameters, THE Image_Parser SHALL extract NBPC as the number of blocks per column
3. WHEN parsing blocking parameters, THE Image_Parser SHALL extract NPPBH as pixels per block horizontal
4. WHEN parsing blocking parameters, THE Image_Parser SHALL extract NPPBV as pixels per block vertical
5. WHEN parsing blocking parameters, THE Image_Parser SHALL extract IMODE as the interleave mode (B, P, R, or S)

### Requirement 3: Band Information Parsing

**User Story:** As a developer, I want to parse band metadata for each image band, so that I can understand the spectral composition of the image.

#### Acceptance Criteria

1. WHEN parsing band information, THE Image_Parser SHALL extract NBANDS as the number of bands (1-9)
2. WHEN NBANDS equals 0, THE Image_Parser SHALL extract XBANDS as the extended band count (10-99999)
3. FOR EACH band, THE Image_Parser SHALL extract IREPBANDn as the 2-character band representation
4. FOR EACH band, THE Image_Parser SHALL extract ISUBCATn as the 6-character band subcategory
5. FOR EACH band, THE Image_Parser SHALL extract IFCn as the 1-character filter condition
6. FOR EACH band, THE Image_Parser SHALL extract IMFLTn as the 3-character filter code
7. FOR EACH band, THE Image_Parser SHALL extract NLUTSn as the number of LUTs (0-4)
8. WHEN NLUTSn is greater than 0, THE Image_Parser SHALL extract NELUTn as the number of LUT entries
9. WHEN NLUTSn is greater than 0, THE Image_Parser SHALL extract LUTDnm data for each LUT

### Requirement 4: Look-Up Table Support

**User Story:** As a developer, I want to parse and apply look-up tables, so that I can correctly display indexed color images.

#### Acceptance Criteria

1. WHEN parsing LUT data, THE Image_Parser SHALL extract NELUTn entries for each of NLUTSn LUTs
2. WHEN IREP is "RGB/LUT", THE Image_Parser SHALL expect exactly 3 LUTs (R, G, B)
3. WHEN applying a LUT, THE LUT_Processor SHALL map each pixel value to its corresponding LUT entry
4. WHEN a pixel value exceeds the LUT entry count, THE LUT_Processor SHALL return an error
5. FOR ALL LUT data, parsing then writing SHALL produce byte-identical output

### Requirement 5: Uncompressed Image Reading

**User Story:** As a developer, I want to read uncompressed image data, so that I can access pixel values for processing.

#### Acceptance Criteria

1. WHEN IC equals "NC" (no compression), THE Image_Reader SHALL read raw pixel data
2. WHEN IC equals "NM" (no compression with mask), THE Image_Reader SHALL read raw pixel data with mask handling
3. WHEN IMODE is "B" (band interleaved by block), THE Image_Reader SHALL read all bands for each block sequentially
4. WHEN IMODE is "P" (band interleaved by pixel), THE Image_Reader SHALL read bands interleaved within each pixel
5. WHEN IMODE is "R" (band interleaved by row), THE Image_Reader SHALL read bands interleaved by row within each block
6. WHEN IMODE is "S" (band sequential), THE Image_Reader SHALL read each band as a separate set of blocks
7. WHEN reading pixel data, THE Image_Reader SHALL decode values according to PVTYPE (INT, SI, R, C, B)
8. WHEN PVTYPE is "INT", THE Image_Reader SHALL interpret pixels as unsigned integers
9. WHEN PVTYPE is "SI", THE Image_Reader SHALL interpret pixels as signed integers
10. WHEN PVTYPE is "R", THE Image_Reader SHALL interpret pixels as IEEE floating-point values
11. WHEN PVTYPE is "C", THE Image_Reader SHALL interpret pixels as complex numbers (real, imaginary pairs)
12. WHEN PVTYPE is "B", THE Image_Reader SHALL interpret pixels as bi-level (1-bit) values

### Requirement 6: Block-Based Image Access

**User Story:** As a developer, I want to access image data by block, so that I can efficiently process large images without loading the entire image into memory.

#### Acceptance Criteria

1. WHEN accessing a block, THE ImageAssetProvider SHALL return pixel data for the specified block coordinates
2. WHEN accessing a block, THE ImageAssetProvider SHALL support optional band selection
3. WHEN block coordinates are out of bounds, THE ImageAssetProvider SHALL return an InvalidBlockCoordinates error
4. WHEN accessing edge blocks, THE ImageAssetProvider SHALL handle partial blocks correctly
5. FOR ALL valid block coordinates, THE ImageAssetProvider SHALL return data with shape [rows, cols, bands]

### Requirement 7: Image Subheader Generation

**User Story:** As a developer, I want to generate image subheaders, so that I can write NITF image segments.

#### Acceptance Criteria

1. WHEN generating an image subheader, THE Image_Writer SHALL write IM as "IM"
2. WHEN generating an image subheader, THE Image_Writer SHALL write IID1 as a 10-character space-padded string
3. WHEN generating an image subheader, THE Image_Writer SHALL write IID2 as an 80-character space-padded string
4. WHEN generating an image subheader, THE Image_Writer SHALL write IDATIM as a 14-character date/time string
5. WHEN generating an image subheader, THE Image_Writer SHALL write NROWS and NCOLS as 8-digit zero-padded strings
6. WHEN generating an image subheader, THE Image_Writer SHALL write PVTYPE as a 3-character string
7. WHEN generating an image subheader, THE Image_Writer SHALL write IREP as an 8-character space-padded string
8. WHEN generating an image subheader, THE Image_Writer SHALL write all security classification fields

### Requirement 8: Blocking Parameter Generation

**User Story:** As a developer, I want blocking parameters to be calculated automatically, so that I can write properly blocked images.

#### Acceptance Criteria

1. WHEN generating blocking parameters, THE Image_Writer SHALL calculate NBPR from NCOLS and NPPBH
2. WHEN generating blocking parameters, THE Image_Writer SHALL calculate NBPC from NROWS and NPPBV
3. WHEN generating blocking parameters, THE Image_Writer SHALL write NBPR as a 4-digit zero-padded string
4. WHEN generating blocking parameters, THE Image_Writer SHALL write NBPC as a 4-digit zero-padded string
5. WHEN generating blocking parameters, THE Image_Writer SHALL write NPPBH as a 4-digit zero-padded string
6. WHEN generating blocking parameters, THE Image_Writer SHALL write NPPBV as a 4-digit zero-padded string
7. WHEN generating blocking parameters, THE Image_Writer SHALL ensure NBPR × NPPBH ≥ NCOLS
8. WHEN generating blocking parameters, THE Image_Writer SHALL ensure NBPC × NPPBV ≥ NROWS

### Requirement 9: Band Information Generation

**User Story:** As a developer, I want to generate band metadata, so that I can write multi-band images correctly.

#### Acceptance Criteria

1. WHEN band count is 1-9, THE Image_Writer SHALL write NBANDS as a 1-digit string
2. WHEN band count is 10-99999, THE Image_Writer SHALL write NBANDS as "0" and XBANDS as a 5-digit string
3. FOR EACH band, THE Image_Writer SHALL write IREPBANDn as a 2-character string
4. FOR EACH band, THE Image_Writer SHALL write ISUBCATn as a 6-character space-padded string
5. FOR EACH band, THE Image_Writer SHALL write IFCn as a 1-character string (default "N")
6. FOR EACH band, THE Image_Writer SHALL write IMFLTn as a 3-character space-padded string
7. FOR EACH band, THE Image_Writer SHALL write NLUTSn as a 1-digit string
8. WHEN NLUTSn is greater than 0, THE Image_Writer SHALL write NELUTn as a 5-digit string
9. WHEN NLUTSn is greater than 0, THE Image_Writer SHALL write LUTDnm data for each LUT

### Requirement 10: Uncompressed Image Writing

**User Story:** As a developer, I want to write uncompressed image data, so that I can create NITF files with imagery.

#### Acceptance Criteria

1. WHEN writing uncompressed data, THE Image_Writer SHALL set IC to "NC"
2. WHEN writing with IMODE "B", THE Image_Writer SHALL write all bands for each block sequentially
3. WHEN writing with IMODE "P", THE Image_Writer SHALL write bands interleaved within each pixel
4. WHEN writing with IMODE "R", THE Image_Writer SHALL write bands interleaved by row within each block
5. WHEN writing with IMODE "S", THE Image_Writer SHALL write each band as a separate set of blocks
6. WHEN writing pixel data, THE Image_Writer SHALL encode values according to PVTYPE
7. WHEN blocks require padding, THE Image_Writer SHALL pad to block boundaries with appropriate fill values
8. WHEN writing image data, THE Image_Writer SHALL calculate and return the total image data length

### Requirement 11: Pixel Value Type Handling

**User Story:** As a developer, I want all pixel value types to be supported, so that I can work with any NITF imagery.

#### Acceptance Criteria

1. WHEN PVTYPE is "INT" and NBPP is 8, THE Pixel_Codec SHALL handle unsigned 8-bit integers
2. WHEN PVTYPE is "INT" and NBPP is 16, THE Pixel_Codec SHALL handle unsigned 16-bit integers
3. WHEN PVTYPE is "INT" and NBPP is 32, THE Pixel_Codec SHALL handle unsigned 32-bit integers
4. WHEN PVTYPE is "SI" and NBPP is 8, THE Pixel_Codec SHALL handle signed 8-bit integers
5. WHEN PVTYPE is "SI" and NBPP is 16, THE Pixel_Codec SHALL handle signed 16-bit integers
6. WHEN PVTYPE is "SI" and NBPP is 32, THE Pixel_Codec SHALL handle signed 32-bit integers
7. WHEN PVTYPE is "R" and NBPP is 32, THE Pixel_Codec SHALL handle 32-bit IEEE floating-point
8. WHEN PVTYPE is "R" and NBPP is 64, THE Pixel_Codec SHALL handle 64-bit IEEE floating-point
9. WHEN PVTYPE is "C" and NBPP is 64, THE Pixel_Codec SHALL handle complex numbers (two 32-bit floats)
10. WHEN PVTYPE is "B", THE Pixel_Codec SHALL handle bi-level (1-bit) values packed into bytes

### Requirement 12: Interleave Mode Conversion

**User Story:** As a developer, I want to convert between interleave modes, so that I can provide data in the format required by consumers.

#### Acceptance Criteria

1. THE Interleave_Converter SHALL convert from IMODE "B" to band-sequential arrays
2. THE Interleave_Converter SHALL convert from IMODE "P" to band-sequential arrays
3. THE Interleave_Converter SHALL convert from IMODE "R" to band-sequential arrays
4. THE Interleave_Converter SHALL convert from IMODE "S" to band-sequential arrays
5. FOR ALL interleave conversions, the output pixel values SHALL match the input pixel values

### Requirement 13: Image Dimension Validation

**User Story:** As a developer, I want image dimensions to be validated, so that I can detect invalid or corrupt image segments.

#### Acceptance Criteria

1. WHEN NROWS equals 0, THE Image_Validator SHALL return an error
2. WHEN NCOLS equals 0, THE Image_Validator SHALL return an error
3. WHEN NROWS exceeds CLEVEL limits, THE Image_Validator SHALL return a warning
4. WHEN NCOLS exceeds CLEVEL limits, THE Image_Validator SHALL return a warning
5. WHEN NBPR × NPPBH is less than NCOLS, THE Image_Validator SHALL return an error
6. WHEN NBPC × NPPBV is less than NROWS, THE Image_Validator SHALL return an error

### Requirement 14: Pixel Type Validation

**User Story:** As a developer, I want pixel type parameters to be validated, so that I can detect inconsistent metadata.

#### Acceptance Criteria

1. WHEN PVTYPE is not one of INT, SI, R, C, or B, THE Image_Validator SHALL return an error
2. WHEN ABPP is greater than NBPP, THE Image_Validator SHALL return an error
3. WHEN PVTYPE is "R" and NBPP is not 32 or 64, THE Image_Validator SHALL return an error
4. WHEN PVTYPE is "C" and NBPP is not 64, THE Image_Validator SHALL return an error
5. WHEN PVTYPE is "B" and NBPP is not 1, THE Image_Validator SHALL return an error

### Requirement 15: Band Configuration Validation

**User Story:** As a developer, I want band configurations to be validated, so that I can detect invalid multi-band images.

#### Acceptance Criteria

1. WHEN IREP is "RGB" and band count is not 3, THE Image_Validator SHALL return an error
2. WHEN IREP is "RGB/LUT" and band count is not 1, THE Image_Validator SHALL return an error
3. WHEN IREP is "MONO" and band count is not 1, THE Image_Validator SHALL return an error
4. WHEN IREPBANDn is invalid for the given IREP, THE Image_Validator SHALL return an error
5. WHEN IMODE is "S" and band count is 1, THE Image_Validator SHALL return a warning (inefficient)

### Requirement 16: LUT Validation

**User Story:** As a developer, I want LUT configurations to be validated, so that I can detect invalid lookup tables.

#### Acceptance Criteria

1. WHEN NLUTSn is greater than 4, THE Image_Validator SHALL return an error
2. WHEN NELUTn is 0 but NLUTSn is greater than 0, THE Image_Validator SHALL return an error
3. WHEN IREP is "RGB/LUT" and NLUTSn is not 3, THE Image_Validator SHALL return an error
4. WHEN NELUTn is less than 2^ABPP, THE Image_Validator SHALL return a warning (incomplete LUT)

### Requirement 17: Round-Trip Consistency

**User Story:** As a developer, I want image segment parsing and writing to be symmetric, so that I can trust the system for data preservation.

#### Acceptance Criteria

1. FOR ALL valid image subheader binary data, parsing then writing SHALL produce byte-identical output
2. FOR ALL valid image pixel data, reading then writing with the same IMODE SHALL produce byte-identical output
3. FOR ALL valid image configurations, writing then reading SHALL produce equivalent metadata values

### Requirement 18: ImageAssetProvider Implementation

**User Story:** As a developer, I want image segments to implement the ImageAssetProvider trait, so that I can use the standard blocked image access interface.

#### Acceptance Criteria

1. THE JBPImageAssetProvider SHALL implement the ImageAssetProvider trait
2. THE JBPImageAssetProvider SHALL return correct values for num_rows(), num_columns(), and num_bands()
3. THE JBPImageAssetProvider SHALL return correct values for num_pixels_per_block_horizontal() and num_pixels_per_block_vertical()
4. THE JBPImageAssetProvider SHALL return correct values for num_bits_per_pixel() and actual_bits_per_pixel()
5. THE JBPImageAssetProvider SHALL return the correct pixel_value_type() based on PVTYPE
6. THE JBPImageAssetProvider SHALL implement get_block() for uncompressed images
7. THE JBPImageAssetProvider SHALL implement has_block() to check block existence

### Requirement 19: Python API Integration

**User Story:** As a Python developer, I want to access image data through the Python bindings, so that I can use NITF imagery in Python applications.

#### Acceptance Criteria

1. THE Python ImageAssetProvider binding SHALL expose all ImageAssetProvider trait methods
2. THE Python get_block() method SHALL return a NumPy array with the correct dtype
3. THE Python get_block() method SHALL return data with shape (rows, cols, bands)
4. THE Python bindings SHALL support optional band selection in get_block()
5. THE Python bindings SHALL expose image metadata through the metadata() method

### Requirement 20: Error Handling

**User Story:** As a developer, I want descriptive error messages for image operations, so that I can diagnose issues effectively.

#### Acceptance Criteria

1. WHEN an image parse error occurs, THE error SHALL include the field name and byte offset
2. WHEN a block access error occurs, THE error SHALL include the block coordinates and image dimensions
3. WHEN a pixel decode error occurs, THE error SHALL include the PVTYPE and NBPP values
4. WHEN a validation error occurs, THE error SHALL include the JBP requirement ID if applicable
5. WHEN an interleave conversion error occurs, THE error SHALL include the source and target IMODE values
