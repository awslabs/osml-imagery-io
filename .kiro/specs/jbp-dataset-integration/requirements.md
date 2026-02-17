# Requirements Document

## Introduction

This document defines the requirements for Phase 2 of the JBP (Joint BIIF Profile) implementation project: JBP Dataset Integration. This phase integrates the generic data-driven binary parser (Phase 1) with the DatasetReader/DatasetWriter interfaces from the API design. The implementation creates JBP-specific readers and writers that use structure definitions to parse NITF/NSIF file headers and navigate to segments, enabling asset-based access to NITF imagery files.

The JBP Dataset Integration provides the bridge between low-level binary parsing and high-level dataset access, mapping NITF segments to discoverable assets with meaningful keys and metadata.

## Glossary

- **JBP**: Joint BIIF Profile - A specification defining conformance requirements for NITF/NSIF files
- **NITF**: National Imagery Transmission Format - A binary file format for imagery and associated metadata
- **NSIF**: NATO Secondary Imagery Format - NATO variant of NITF
- **Segment**: A discrete unit within a NITF file (image, graphic, text, DES, or reserved extension)
- **Subheader**: Metadata block preceding each segment's data
- **File_Header**: The initial portion of a NITF file containing file-level metadata and segment counts
- **CLEVEL**: Complexity Level - A NITF field indicating file size and feature constraints (03-09)
- **Asset**: A discoverable resource within a dataset, identified by a unique key
- **Asset_Key**: A string identifier for an asset (e.g., "image_segment_0", "text_segment_1")
- **Structure_Accessor**: The lazy map-like interface from Phase 1 for reading parsed values
- **Structure_Writer**: The interface from Phase 1 for encoding values into binary format
- **FL**: File Length field in NITF header
- **HL**: Header Length field in NITF header
- **NUMI**: Number of Image Segments field
- **NUMS**: Number of Graphic Segments field
- **NUMT**: Number of Text Segments field
- **NUMDES**: Number of Data Extension Segments field
- **LISH**: Length of Image Subheader
- **LI**: Length of Image Data
- **CCS**: Common Coordinate System - The coordinate system used for NITF geolocation
- **DateTime_Parser**: A utility function that converts NITF date/time strings to standard datetime objects

## Requirements

### Requirement 1: JBPDatasetReader Initialization

**User Story:** As a developer, I want to open NITF/NSIF files through the DatasetReader interface, so that I can access imagery and metadata using the unified API.

#### Acceptance Criteria

1. WHEN a valid NITF 2.1 file path is provided, THE JBPDatasetReader SHALL parse the file header using the `NITF_02.10_FileHeader` structure definition
2. WHEN a valid NSIF 1.0 file path is provided, THE JBPDatasetReader SHALL parse the file header using the `NSIF_01.00_FileHeader` structure definition
3. WHEN the file begins with "NITF02.10", THE JBPDatasetReader SHALL identify it as NITF 2.1 format
4. WHEN the file begins with "NSIF01.00", THE JBPDatasetReader SHALL identify it as NSIF 1.0 format
5. IF the file does not begin with a valid magic number, THEN THE JBPDatasetReader SHALL return an InvalidFormat error
6. WHEN the file header is parsed, THE JBPDatasetReader SHALL extract segment counts (NUMI, NUMS, NUMT, NUMDES, NUMRES)
7. WHEN the file header is parsed, THE JBPDatasetReader SHALL calculate segment offsets from header length arrays

### Requirement 2: Segment Offset Calculation

**User Story:** As a developer, I want the reader to calculate segment offsets from header metadata, so that I can navigate directly to any segment without parsing the entire file.

#### Acceptance Criteria

1. WHEN the file header is parsed, THE JBPDatasetReader SHALL calculate the total header length (HL) from fixed and variable portions
2. WHEN image segment info is present, THE JBPDatasetReader SHALL calculate each image segment offset from cumulative LISH and LI values
3. WHEN graphic segment info is present, THE JBPDatasetReader SHALL calculate each graphic segment offset from cumulative LSSH and LS values
4. WHEN text segment info is present, THE JBPDatasetReader SHALL calculate each text segment offset from cumulative LTSH and LT values
5. WHEN DES segment info is present, THE JBPDatasetReader SHALL calculate each DES segment offset from cumulative LDSH and LD values
6. THE JBPDatasetReader SHALL store calculated offsets for lazy segment access without re-parsing the header

### Requirement 3: Asset Discovery

**User Story:** As a developer, I want to discover available assets by type and role, so that I can find imagery and metadata without knowing the file structure.

#### Acceptance Criteria

1. WHEN `get_asset_keys()` is called with no filters, THE JBPDatasetReader SHALL return keys for all segments
2. WHEN `get_asset_keys()` is called with `asset_type=Image`, THE JBPDatasetReader SHALL return only image segment keys
3. WHEN `get_asset_keys()` is called with `asset_type=Text`, THE JBPDatasetReader SHALL return only text segment keys
4. WHEN `get_asset_keys()` is called with `asset_type=Graphics`, THE JBPDatasetReader SHALL return only graphic segment keys
5. WHEN `get_asset_keys()` is called with `asset_type=Data`, THE JBPDatasetReader SHALL return only DES segment keys
6. THE JBPDatasetReader SHALL generate asset keys using the pattern `{type}_segment_{index}` (e.g., "image_segment_0")
7. WHEN `has_asset(key)` is called, THE JBPDatasetReader SHALL return true if the key corresponds to a valid segment

### Requirement 4: Lazy Segment Access

**User Story:** As a developer, I want segment subheaders to be parsed on-demand, so that I can efficiently access specific segments without parsing all segments.

#### Acceptance Criteria

1. WHEN `get_asset(key)` is called for an image segment, THE JBPDatasetReader SHALL parse the image subheader using `NITF_02.10_ImageSubheader`
2. WHEN `get_asset(key)` is called for a graphic segment, THE JBPDatasetReader SHALL parse the graphic subheader using `NITF_02.10_GraphicSubheader`
3. WHEN `get_asset(key)` is called for a text segment, THE JBPDatasetReader SHALL parse the text subheader using `NITF_02.10_TextSubheader`
4. WHEN `get_asset(key)` is called for a DES segment, THE JBPDatasetReader SHALL parse the DES subheader using `NITF_02.10_DESSubheader`
5. THE JBPDatasetReader SHALL NOT parse segment subheaders until the segment is accessed
6. WHEN a segment is accessed multiple times, THE JBPDatasetReader SHALL cache the parsed subheader
7. IF an asset key does not exist, THEN `get_asset(key)` SHALL return an AssetNotFound error

### Requirement 5: File-Level Metadata

**User Story:** As a developer, I want to access file-level metadata through the MetadataProvider interface, so that I can retrieve security classification, originator, and other file attributes.

#### Acceptance Criteria

1. WHEN `metadata()` is called, THE JBPDatasetReader SHALL return a MetadataProvider for file header fields
2. WHEN `as_dict(None)` is called on the file metadata, THE MetadataProvider SHALL return all file header fields as key-value pairs
3. WHEN `as_dict(Some(prefix))` is called, THE MetadataProvider SHALL return only fields whose names start with the given prefix (e.g., "FS" returns FSCLAS, FSCLSY, FSCODE, etc.)
4. WHEN `raw()` is called on the file metadata, THE MetadataProvider SHALL return the raw file header bytes

### Requirement 6: Segment-Level Metadata

**User Story:** As a developer, I want to access segment-level metadata through each asset's MetadataProvider, so that I can retrieve segment-specific attributes.

#### Acceptance Criteria

1. WHEN `metadata()` is called on an image asset, THE AssetProvider SHALL return a MetadataProvider for image subheader fields
2. WHEN `metadata()` is called on a text asset, THE AssetProvider SHALL return a MetadataProvider for text subheader fields
3. WHEN `metadata()` is called on a graphic asset, THE AssetProvider SHALL return a MetadataProvider for graphic subheader fields
4. WHEN `metadata()` is called on a DES asset, THE AssetProvider SHALL return a MetadataProvider for DES subheader fields
5. WHEN `as_dict(None)` is called on segment metadata, THE MetadataProvider SHALL return all subheader fields as key-value pairs

### Requirement 7: JBPDatasetWriter Initialization

**User Story:** As a developer, I want to create new NITF files through the DatasetWriter interface, so that I can generate compliant imagery files.

#### Acceptance Criteria

1. WHEN a JBPDatasetWriter is created with a file path, THE writer SHALL prepare to write a NITF 2.1 file
2. THE JBPDatasetWriter SHALL support configuration for NITF 2.1 or NSIF 1.0 output format
3. THE JBPDatasetWriter SHALL initialize with default file header values that can be overridden via metadata
4. THE JBPDatasetWriter SHALL track added assets for segment count and offset calculation

### Requirement 8: Asset Addition

**User Story:** As a developer, I want to add assets to the dataset, so that I can include imagery, text, and other content in the output file.

#### Acceptance Criteria

1. WHEN `add_asset()` is called with an ImageAssetProvider, THE JBPDatasetWriter SHALL queue an image segment for writing
2. WHEN `add_asset()` is called with a TextAssetProvider, THE JBPDatasetWriter SHALL queue a text segment for writing
3. WHEN `add_asset()` is called with a GraphicsAssetProvider, THE JBPDatasetWriter SHALL queue a graphic segment for writing
4. WHEN `add_asset()` is called with a DataAssetProvider, THE JBPDatasetWriter SHALL queue a DES segment for writing
5. IF `add_asset()` is called with a duplicate key, THEN THE JBPDatasetWriter SHALL return a DuplicateKey error
6. THE JBPDatasetWriter SHALL preserve the order of added assets for segment ordering

### Requirement 9: Two-Pass Writing

**User Story:** As a developer, I want the writer to handle NITF's length-first format, so that segment lengths are correctly written in the file header.

#### Acceptance Criteria

1. WHEN `close()` is called, THE JBPDatasetWriter SHALL calculate all segment subheader and data lengths
2. THE JBPDatasetWriter SHALL write the file header with correct segment counts and length arrays
3. THE JBPDatasetWriter SHALL write each segment subheader followed by segment data
4. THE JBPDatasetWriter SHALL update the FL (File Length) field with the total file size
5. THE JBPDatasetWriter SHALL use StructureWriter to encode all headers according to structure definitions

### Requirement 10: Metadata Setting

**User Story:** As a developer, I want to set file-level metadata, so that I can specify security classification, originator, and other file attributes.

#### Acceptance Criteria

1. WHEN `set_metadata()` is called, THE JBPDatasetWriter SHALL store the metadata for file header generation
2. THE JBPDatasetWriter SHALL map metadata dictionary keys to file header field names
3. IF required metadata fields are missing, THEN THE JBPDatasetWriter SHALL use default values
4. THE JBPDatasetWriter SHALL validate metadata values against NITF field constraints

### Requirement 11: Structure Definition Files

**User Story:** As a developer, I want complete structure definitions for NITF headers, so that the parser can handle all file and segment header fields.

#### Acceptance Criteria

1. THE `NITF_02.10_FileHeader.ksy` definition SHALL include all fields from NITF 2.1 Table 5.1-1
2. THE `NITF_02.10_ImageSubheader.ksy` definition SHALL include all fields from NITF 2.1 Table 5.13-1
3. THE `NITF_02.10_GraphicSubheader.ksy` definition SHALL include all fields from NITF 2.1 Table 5.15-1
4. THE `NITF_02.10_TextSubheader.ksy` definition SHALL include all fields from NITF 2.1 Table 5.17-1
5. THE `NITF_02.10_DESSubheader.ksy` definition SHALL include all fields from NITF 2.1 Table 5.18-1
6. THE `NSIF_01.00_FileHeader.ksy` definition SHALL include NSIF 1.0 file header fields

### Requirement 12: Magic Number Validation

**User Story:** As a developer, I want the reader to validate file format markers, so that invalid files are rejected early.

#### Acceptance Criteria

1. WHEN opening a file, THE JBPDatasetReader SHALL verify the FHDR field is "NITF" or "NSIF"
2. WHEN opening a file, THE JBPDatasetReader SHALL verify the FVER field is "02.10" for NITF or "01.00" for NSIF
3. IF the magic number is invalid, THEN THE JBPDatasetReader SHALL return an InvalidFormat error with the actual bytes found
4. THE validation SHALL occur before any other parsing to fail fast on invalid files

### Requirement 13: Complexity Level Validation

**User Story:** As a developer, I want the reader to validate complexity level, so that files exceeding supported constraints are identified.

#### Acceptance Criteria

1. WHEN the file header is parsed, THE JBPDatasetReader SHALL extract and validate the CLEVEL field
2. THE JBPDatasetReader SHALL accept CLEVEL values 03, 05, 06, 07, and 09 for still imagery
3. IF CLEVEL is outside the valid range, THEN THE JBPDatasetReader SHALL return a warning (not error) with the invalid value
4. THE JBPDatasetReader SHALL store the CLEVEL for constraint checking during segment access

### Requirement 14: Segment Count Validation

**User Story:** As a developer, I want the reader to validate segment counts against length arrays, so that malformed files are detected.

#### Acceptance Criteria

1. WHEN the file header is parsed, THE JBPDatasetReader SHALL verify NUMI matches the count of image segment info entries
2. WHEN the file header is parsed, THE JBPDatasetReader SHALL verify NUMS matches the count of graphic segment info entries
3. WHEN the file header is parsed, THE JBPDatasetReader SHALL verify NUMT matches the count of text segment info entries
4. WHEN the file header is parsed, THE JBPDatasetReader SHALL verify NUMDES matches the count of DES segment info entries
5. IF segment counts are inconsistent, THEN THE JBPDatasetReader SHALL return a ValidationError

### Requirement 15: File Length Validation (Optional)

**User Story:** As a developer, I want optional file length validation, so that I can detect truncated files when needed without requiring full file downloads.

#### Acceptance Criteria

1. THE JBPDatasetReader SHALL support an optional validation mode that can be enabled or disabled
2. WHEN validation mode is enabled and the full file is available, THE JBPDatasetReader SHALL calculate expected file length from header and segment lengths
3. WHEN validation mode is enabled, THE JBPDatasetReader SHALL compare calculated length against the FL field value
4. WHEN validation mode is enabled, THE JBPDatasetReader SHALL compare calculated length against actual file size
5. IF file length is inconsistent and validation is enabled, THEN THE JBPDatasetReader SHALL return a ValidationError with expected and actual values
6. WHEN validation mode is disabled, THE JBPDatasetReader SHALL skip file length validation to support partial file access

### Requirement 16: DateTime Parsing Utility

**User Story:** As a developer, I want a utility to convert NITF date/time strings to standard datetime types, so that I can work with temporal metadata in a type-safe manner.

#### Acceptance Criteria

1. THE DateTime_Parser utility SHALL accept NITF FDT format strings (CCYYMMDDhhmmss)
2. WHEN a valid datetime string is provided, THE DateTime_Parser SHALL return a standard datetime object
3. WHEN a datetime string contains "--" for unknown components, THE DateTime_Parser SHALL handle partial dates appropriately
4. IF a datetime string has invalid format or out-of-range values, THEN THE DateTime_Parser SHALL return a ParseError
5. THE DateTime_Parser SHALL be available as a standalone utility function

### Requirement 17: Round-Trip Consistency

**User Story:** As a developer, I want to verify that writing then reading produces equivalent data, so that I can trust the writer for data preservation.

#### Acceptance Criteria

1. FOR ALL valid file header field values, writing with JBPDatasetWriter then reading with JBPDatasetReader SHALL produce equivalent values
2. FOR ALL valid segment subheader field values, writing then reading SHALL produce equivalent values
3. THE round-trip SHALL preserve all metadata fields without loss or corruption

### Requirement 18: Graceful Error Handling

**User Story:** As a developer, I want the reader to handle non-compliant files gracefully, so that partially valid files can still be processed.

#### Acceptance Criteria

1. WHEN a validation error occurs, THE JBPDatasetReader SHALL continue parsing if possible and collect all errors
2. THE JBPDatasetReader SHALL distinguish between fatal errors (cannot continue) and warnings (can continue)
3. WHEN `get_asset()` is called on a segment with validation warnings, THE JBPDatasetReader SHALL return the asset with warnings attached
4. THE JBPDatasetReader SHALL provide a method to retrieve all validation warnings and errors

### Requirement 19: Python Bindings

**User Story:** As a Python developer, I want to use the Dataset abstractions from Python without knowing the underlying format, so that I can work with imagery files in a format-agnostic way.

#### Acceptance Criteria

1. THE Python bindings SHALL expose DatasetReader and DatasetWriter abstract interfaces
2. THE Python bindings SHALL NOT expose JBPDatasetReader or JBPDatasetWriter directly to users
3. THE IO.open() function SHALL automatically select JBPDatasetReader for NITF/NSIF files based on format detection
4. WHEN creating a DatasetWriter, THE user SHALL specify the output format (e.g., "nitf", "nsif") to determine the underlying writer
5. THE Python bindings SHALL support context manager protocol (`with` statement) for automatic resource cleanup
6. THE Python bindings SHALL accept file paths as strings or Path objects


