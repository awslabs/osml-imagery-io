# Requirements Document

## Introduction

This document defines the requirements for Phase 5 of the JBP (Joint BIIF Profile) implementation project: JPEG 2000 Compression. This phase implements reading and writing of JPEG 2000 compressed imagery, which is the most common compression format in modern NITF files. The implementation extends the `BlockDecoder` trait with a `Jpeg2000BlockDecoder` for reading and the `BlockEncoder` trait with a `Jpeg2000BlockEncoder` for writing.

JPEG 2000 (J2K) provides superior compression efficiency compared to legacy formats and supports features like progressive decoding, multiple resolution levels, and lossless/lossy modes. The JBP specification mandates compliance with the BPJ2K01.20 profile for NITF imagery.

## Glossary

- **JPEG_2000**: An image compression standard (ISO/IEC 15444) using wavelet-based compression, supporting both lossy and lossless modes
- **HTJ2K**: High-Throughput JPEG 2000 (Part 15), a faster variant of JPEG 2000 with simplified entropy coding
- **Codestream**: The compressed JPEG 2000 data embedded within a NITF image segment
- **IC**: Image Compression field in the NITF image subheader indicating the compression algorithm
- **C8**: IC value for JPEG 2000 Part 1 compression (non-masked)
- **CD**: IC value for JPEG 2000 Part 15 (HTJ2K) compression (non-masked)
- **COMRAT**: Compression Rate field specifying the target compression ratio or quality
- **NBPP**: Number of Bits Per Pixel - storage size for each pixel value (1-38 for J2K)
- **ABPP**: Actual Bits Per Pixel - for J2K, ABPP must equal NBPP
- **IMODE**: Image Mode - must be "B" (band interleaved by block) for JPEG 2000
- **BPJ2K01.20**: The BIIF Profile for JPEG 2000 specifying NITF-specific J2K constraints
- **Resolution_Level**: A decimated version of the image from the wavelet decomposition pyramid
- **Decomposition_Level**: The number of wavelet transform iterations, determining available resolution levels
- **BlockDecoder**: The trait from Phase 4 defining the interface for decoding image blocks
- **Jpeg2000BlockDecoder**: The BlockDecoder implementation for JPEG 2000 compressed imagery
- **Quality_Layer**: A subset of the codestream providing a specific quality level
- **Tile**: A rectangular region of the image that can be independently encoded/decoded
- **Compression_Ratio**: The ratio of original size to compressed size (e.g., "N001.0" means numerically lossless)
- **J2K_Codec**: An abstraction trait for JPEG 2000 encoding/decoding operations, allowing different backend implementations
- **OpenJPEG**: An open-source JPEG 2000 codec library (libopenjp2) used as the default backend
- **NVIDIA_nvJPEG2000**: NVIDIA's GPU-accelerated JPEG 2000 codec library for high-performance decoding

## Requirements

### Requirement 0: JPEG 2000 Codec Abstraction

**User Story:** As a library maintainer, I want a JPEG 2000 codec abstraction internally, so that the codec implementation can be changed without affecting the public API.

#### Acceptance Criteria

1. THE system SHALL define an internal J2K_Codec trait abstracting encode/decode operations
2. THE J2K_Codec trait SHALL support both JPEG 2000 Part 1 and Part 15 (HTJ2K) operations
3. THE J2K_Codec decode operations SHALL accept a byte slice (&[u8]) pointing to the codestream data
4. THE J2K_Codec decode operations SHALL work with memory-mapped files or direct pointers to NITF image segment data
5. THE J2K_Codec encode operations SHALL return owned byte vectors (Vec<u8>) containing the encoded codestream
6. THE system SHALL provide an OpenJPEG-based implementation of J2K_Codec as the default
7. THE codec selection SHALL be configurable via environment variable (OSML_IO_J2K_CODEC)
8. THE public API (JBPDatasetReader, JBPDatasetWriter) SHALL NOT expose codec details to users
9. WHEN a codec does not support a requested operation, THE system SHALL return a descriptive error

### Requirement 1: JPEG 2000 Codestream Extraction

**User Story:** As a developer, I want to extract JPEG 2000 codestreams from NITF image segments, so that I can decode compressed imagery.

#### Acceptance Criteria

1. WHEN IC equals "C8", THE Jpeg2000BlockDecoder SHALL extract the JPEG 2000 Part 1 codestream from the image data
2. WHEN IC equals "CD", THE Jpeg2000BlockDecoder SHALL extract the JPEG 2000 Part 15 (HTJ2K) codestream from the image data
3. WHEN extracting a codestream, THE Jpeg2000BlockDecoder SHALL validate the J2K magic bytes (0xFF4F for SOC marker)
4. WHEN the codestream is invalid, THE Jpeg2000BlockDecoder SHALL return a descriptive error including the byte offset
5. THE Jpeg2000BlockDecoder SHALL support codestreams with multiple tiles

### Requirement 2: JPEG 2000 Decoding

**User Story:** As a developer, I want to decode JPEG 2000 compressed image data, so that I can access pixel values for processing.

#### Acceptance Criteria

1. WHEN decoding a J2K codestream, THE Jpeg2000BlockDecoder SHALL delegate to the configured J2K_Codec implementation
2. WHEN decoding a J2K codestream, THE Jpeg2000BlockDecoder SHALL produce pixel data matching the NROWS and NCOLS dimensions
3. WHEN decoding a J2K codestream, THE Jpeg2000BlockDecoder SHALL handle bit depths from 1 to 38 bits per pixel
4. WHEN decoding a J2K codestream, THE Jpeg2000BlockDecoder SHALL preserve the number of bands specified in NBANDS/XBANDS
5. WHEN decoding a multi-band image, THE Jpeg2000BlockDecoder SHALL return data in band-sequential format
6. WHEN a decoding error occurs, THE Jpeg2000BlockDecoder SHALL return an error with the underlying codec error message

### Requirement 3: Resolution Level Support

**User Story:** As a developer, I want to access different resolution levels of JPEG 2000 imagery, so that I can efficiently display thumbnails or process at reduced resolution.

#### Acceptance Criteria

1. THE Jpeg2000BlockDecoder SHALL report the number of available resolution levels via num_resolution_levels()
2. WHEN resolution_level is 0, THE Jpeg2000BlockDecoder SHALL return full resolution data
3. WHEN resolution_level is N > 0, THE Jpeg2000BlockDecoder SHALL return data at 1/(2^N) of full resolution
4. WHEN resolution_level exceeds available levels, THE Jpeg2000BlockDecoder SHALL return an InvalidResolutionLevel error
5. FOR ALL resolution levels, THE Jpeg2000BlockDecoder SHALL return correctly scaled dimensions in the shape tuple

### Requirement 4: Block-Based Access for JPEG 2000

**User Story:** As a developer, I want to access JPEG 2000 imagery through the BlockDecoder interface, so that I can use the same API as uncompressed imagery.

#### Acceptance Criteria

1. THE Jpeg2000BlockDecoder SHALL implement the BlockDecoder trait
2. WHEN get_block() is called, THE Jpeg2000BlockDecoder SHALL decode and return the requested block region
3. WHEN has_block() is called, THE Jpeg2000BlockDecoder SHALL return true for valid block coordinates
4. WHEN block coordinates are out of bounds, THE Jpeg2000BlockDecoder SHALL return an InvalidBlockCoordinates error
5. WHEN bands parameter is specified, THE Jpeg2000BlockDecoder SHALL return only the requested bands

### Requirement 5: COMRAT Parsing

**User Story:** As a developer, I want to parse the COMRAT field, so that I can understand the compression parameters used.

#### Acceptance Criteria

1. WHEN IC is "C8" or "CD", THE COMRAT_Parser SHALL parse the 4-character compression rate field
2. WHEN COMRAT starts with "N", THE COMRAT_Parser SHALL interpret it as numerically lossless (e.g., "N001.0")
3. WHEN COMRAT starts with "V", THE COMRAT_Parser SHALL interpret it as visually lossless (e.g., "V001.0")
4. WHEN COMRAT is numeric, THE COMRAT_Parser SHALL interpret it as a target bits-per-pixel rate (e.g., "00.5")
5. WHEN COMRAT format is invalid, THE COMRAT_Parser SHALL return a validation warning

### Requirement 6: BPJ2K01.20 Profile Compliance

**User Story:** As a developer, I want JPEG 2000 decoding to comply with the BPJ2K01.20 profile, so that I can correctly handle NITF-specific J2K constraints.

#### Acceptance Criteria

1. THE Jpeg2000BlockDecoder SHALL validate that IMODE equals "B" for J2K compressed images
2. THE Jpeg2000BlockDecoder SHALL validate that NBPP is between 1 and 38 for J2K images
3. THE Jpeg2000BlockDecoder SHALL validate that ABPP equals NBPP for J2K images
4. WHEN profile constraints are violated, THE Jpeg2000BlockDecoder SHALL return a validation error
5. THE Jpeg2000BlockDecoder SHALL support both signed and unsigned pixel values per PVTYPE

### Requirement 7: JPEG 2000 Encoding

**User Story:** As a developer, I want to encode image data as JPEG 2000 tile-by-tile, so that I can write compressed NITF files without loading entire images into memory.

#### Acceptance Criteria

1. THE Jpeg2000BlockEncoder SHALL implement the existing BlockEncoder trait from the block-encoder-refactor
2. THE Jpeg2000BlockEncoder SHALL encode tiles incrementally using the J2K_Codec
3. THE Jpeg2000BlockEncoder SHALL support configurable tile sizes via block_dimensions()
4. THE Jpeg2000BlockEncoder SHALL accept pixel data via encode_block() and produce codestreams via finalize()
5. THE Jpeg2000BlockEncoder SHALL produce codestreams compliant with BPJ2K01.20 profile

### Requirement 8: HTJ2K Encoding

**User Story:** As a developer, I want to encode image data as HTJ2K, so that I can write NITF files with faster-decoding compression.

#### Acceptance Criteria

1. THE Jpeg2000Encoder SHALL encode pixel data into a valid JPEG 2000 Part 15 (HTJ2K) codestream
2. THE Jpeg2000Encoder SHALL support HTJ2K-specific encoding parameters
3. WHEN encoding as HTJ2K, THE Jpeg2000Encoder SHALL set IC to "CD"
4. THE Jpeg2000Encoder SHALL produce HTJ2K codestreams compatible with standard J2K decoders

### Requirement 9: IC Field Generation

**User Story:** As a developer, I want the IC field to be correctly generated, so that readers can identify the compression type.

#### Acceptance Criteria

1. WHEN encoding with JPEG 2000 Part 1, THE Image_Writer SHALL set IC to "C8"
2. WHEN encoding with HTJ2K, THE Image_Writer SHALL set IC to "CD"
3. WHEN encoding with JPEG 2000, THE Image_Writer SHALL set IMODE to "B"
4. THE Image_Writer SHALL validate IC/IMODE consistency before writing

### Requirement 10: COMRAT Generation

**User Story:** As a developer, I want the COMRAT field to be correctly generated, so that readers can understand the compression parameters.

#### Acceptance Criteria

1. WHEN encoding numerically lossless, THE Image_Writer SHALL set COMRAT to "N001.0"
2. WHEN encoding visually lossless, THE Image_Writer SHALL set COMRAT to "Vnnn.n" with the quality factor
3. WHEN encoding with target bitrate, THE Image_Writer SHALL set COMRAT to "nn.n" with the bits-per-pixel rate
4. THE Image_Writer SHALL format COMRAT as a 4-character field with proper padding

### Requirement 11: Compression Configuration

**User Story:** As a developer, I want to configure compression parameters, so that I can balance quality and file size.

#### Acceptance Criteria

1. THE Jpeg2000Encoder SHALL accept a target compression ratio parameter
2. THE Jpeg2000Encoder SHALL accept a lossless mode flag
3. THE Jpeg2000Encoder SHALL accept a number of decomposition levels parameter
4. THE Jpeg2000Encoder SHALL accept a number of quality layers parameter
5. WHEN no parameters are specified, THE Jpeg2000Encoder SHALL use sensible defaults (lossy, ratio 10:1, 5 decomposition levels)

### Requirement 12: Codestream Embedding

**User Story:** As a developer, I want the J2K codestream to be correctly embedded in the NITF image segment, so that the file is valid.

#### Acceptance Criteria

1. THE Image_Writer SHALL embed the J2K codestream directly in the image data area
2. THE Image_Writer SHALL calculate the correct image data length from the codestream size
3. THE Image_Writer SHALL ensure the codestream starts at the correct offset after the subheader
4. FOR ALL J2K images, THE Image_Writer SHALL write a single codestream (no tiling at NITF level)

### Requirement 13: Multi-Band JPEG 2000

**User Story:** As a developer, I want to encode and decode multi-band JPEG 2000 imagery, so that I can work with multispectral data.

#### Acceptance Criteria

1. THE Jpeg2000BlockDecoder SHALL decode multi-component J2K codestreams
2. THE Jpeg2000Encoder SHALL encode multi-band images as multi-component J2K codestreams
3. WHEN encoding multi-band images, THE Jpeg2000Encoder SHALL preserve band count in the codestream
4. FOR ALL multi-band images, band ordering SHALL be preserved through encode/decode cycle

### Requirement 14: Pixel Type Handling

**User Story:** As a developer, I want all supported pixel types to work with JPEG 2000, so that I can compress any valid NITF imagery.

#### Acceptance Criteria

1. WHEN PVTYPE is "INT", THE Jpeg2000BlockDecoder SHALL decode unsigned integer pixels
2. WHEN PVTYPE is "SI", THE Jpeg2000BlockDecoder SHALL decode signed integer pixels
3. THE Jpeg2000Encoder SHALL encode pixels according to the specified PVTYPE
4. FOR ALL supported PVTYPE values, pixel values SHALL be preserved within compression tolerance

### Requirement 15: Round-Trip Validation

**User Story:** As a developer, I want to validate that encoding and decoding produce consistent results, so that I can trust the compression implementation.

#### Acceptance Criteria

1. FOR ALL lossless-encoded images, decoding SHALL produce byte-identical pixel data
2. FOR ALL lossy-encoded images, decoding SHALL produce pixel data within the specified quality tolerance
3. FOR ALL encoded images, the decoded dimensions SHALL match the original NROWS and NCOLS
4. FOR ALL encoded images, the decoded band count SHALL match the original NBANDS/XBANDS

### Requirement 16: Error Handling

**User Story:** As a developer, I want descriptive error messages for JPEG 2000 operations, so that I can diagnose issues effectively.

#### Acceptance Criteria

1. WHEN a J2K decode error occurs, THE error SHALL include the library error message and byte offset
2. WHEN a J2K encode error occurs, THE error SHALL include the encoding parameters and failure reason
3. WHEN COMRAT parsing fails, THE error SHALL include the invalid COMRAT value
4. WHEN profile validation fails, THE error SHALL include the violated constraint and JBP requirement ID
5. WHEN resolution level is invalid, THE error SHALL include the requested level and available levels

### Requirement 17: Integration with JBPDatasetReader

**User Story:** As a developer, I want JPEG 2000 images to be automatically decoded when reading NITF files, so that I can use the standard DatasetReader interface.

#### Acceptance Criteria

1. WHEN JBPDatasetReader encounters IC="C8", THE reader SHALL use Jpeg2000BlockDecoder
2. WHEN JBPDatasetReader encounters IC="CD", THE reader SHALL use Jpeg2000BlockDecoder
3. THE JBPImageAssetProvider SHALL expose resolution levels for J2K images
4. THE JBPImageAssetProvider SHALL support band selection for J2K images

### Requirement 18: Integration with JBPDatasetWriter

**User Story:** As a developer, I want to write JPEG 2000 compressed NITF files, so that I can create compact imagery files.

#### Acceptance Criteria

1. WHEN IC hint is "C8", THE JBPDatasetWriter SHALL encode image data as JPEG 2000 Part 1
2. WHEN IC hint is "CD", THE JBPDatasetWriter SHALL encode image data as HTJ2K
3. THE JBPDatasetWriter SHALL accept compression parameters via encoding hints
4. THE JBPDatasetWriter SHALL validate J2K constraints before encoding

### Requirement 19: Python API Integration

**User Story:** As a Python developer, I want to access JPEG 2000 compressed imagery through the Python bindings, so that I can use compressed NITF files in Python applications.

#### Acceptance Criteria

1. THE Python ImageAssetProvider binding SHALL support resolution_level parameter in get_block()
2. THE Python bindings SHALL expose num_resolution_levels() for J2K images
3. THE Python bindings SHALL return correct NumPy dtypes for all J2K bit depths
4. THE Python bindings SHALL support compression configuration when writing J2K images

### Requirement 20: OpenJPEG Default Implementation

**User Story:** As a developer, I want OpenJPEG to be the default JPEG 2000 codec, so that I have a working implementation out of the box.

#### Acceptance Criteria

1. THE system SHALL provide an OpenJPEG-based J2K_Codec implementation using the openjp2 library
2. THE OpenJPEG implementation SHALL be the default codec when no other is configured
3. THE OpenJPEG implementation SHALL support JPEG 2000 Part 1 encoding and decoding
4. THE OpenJPEG implementation SHALL support bit depths from 1 to 38 bits per pixel
5. THE OpenJPEG implementation SHALL support multi-component (multi-band) images

### Requirement 21: Codec Backend Extensibility

**User Story:** As a library maintainer, I want to be able to add new JPEG 2000 codec backends, so that users can benefit from GPU-accelerated libraries like NVIDIA nvJPEG2000 without changing their code.

#### Acceptance Criteria

1. THE J2K_Codec trait SHALL be designed for easy implementation by new backends
2. THE system SHALL select the codec based on the OSML_IO_J2K_CODEC environment variable
3. WHEN OSML_IO_J2K_CODEC is not set, THE system SHALL use OpenJPEG as the default
4. WHEN a backend does not support a requested feature (e.g., HTJ2K), THE system SHALL return a descriptive error
5. THE codec selection SHALL be transparent to users of the public API

</content>
</invoke>