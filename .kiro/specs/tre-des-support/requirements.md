# Requirements Document

## Introduction

This document defines the requirements for Phase 3 of the JBP (Joint BIIF Profile) implementation project: Tagged Record Extensions (TREs) and Data Extension Segments (DES) support. This phase establishes the extensible metadata framework for TREs and DES using the data-driven parser infrastructure from Phase 1. TRE definitions are loaded at runtime via the Structure Registry, enabling new TRE types to be introduced by adding `.ksy` definition files without modifying the parser code.

## Glossary

- **TRE**: Tagged Record Extension - a mechanism for extending NITF files with additional metadata fields
- **TRE_Envelope**: The wrapper structure containing CETAG (6-char tag), CEL (length), and CEDATA (extension data)
- **DES**: Data Extension Segment - a NITF segment type for storing extended data and overflow TREs
- **DES_Subheader**: The header portion of a DES containing identification, security, and overflow information
- **TRE_OVERFLOW**: A special DES type used to store TREs that exceed the space available in their original header location
- **RES**: Reserved Extension Segment - a segment type reserved for future extensions
- **Structure_Registry**: The component from Phase 1 that manages loading and lookup of structure definitions
- **Structure_Accessor**: The lazy map-like interface from Phase 1 for reading parsed values
- **Structure_Writer**: The interface from Phase 1 for encoding values into binary format
- **CETAG**: The 6-character alphanumeric tag identifying a TRE type
- **CEL**: The length field indicating the size of CEDATA in bytes
- **CEDATA**: The extension data portion of a TRE containing the actual metadata fields
- **DESID**: The 25-character unique identifier for a DES type
- **DESVER**: The 2-digit version number of a DES definition
- **DESOFLW**: The 6-character field indicating which header type overflowed (for TRE_OVERFLOW DES)
- **DESITEM**: The 3-digit field indicating which segment index overflowed (for TRE_OVERFLOW DES)
- **KSY_File**: A YAML file following Kaitai Struct format conventions for defining binary structures

## Requirements

### Requirement 1: TRE Envelope Parsing

**User Story:** As a developer, I want to parse TRE envelopes from NITF headers, so that I can extract extension metadata from any location in the file.

#### Acceptance Criteria

1. WHEN parsing a TRE envelope, THE TRE_Parser SHALL extract the CETAG field as a 6-character alphanumeric string
2. WHEN parsing a TRE envelope, THE TRE_Parser SHALL extract the CEL field as a 5-digit numeric string representing the CEDATA length
3. WHEN parsing a TRE envelope, THE TRE_Parser SHALL extract CEDATA as raw bytes of length specified by CEL
4. WHEN a TRE envelope has invalid CETAG format (non-alphanumeric characters), THE TRE_Parser SHALL return a validation error
5. WHEN a TRE envelope has CEL value exceeding available bytes, THE TRE_Parser SHALL return an UnexpectedEof error

### Requirement 2: Configuration-Driven TRE Field Extraction

**User Story:** As a developer, I want TRE fields to be parsed using runtime-loaded definitions, so that I can support new TRE types without code changes.

#### Acceptance Criteria

1. WHEN parsing CEDATA for a known TRE type, THE TRE_Parser SHALL lookup the definition from Structure_Registry using the pattern `TRE_{CETAG}`
2. WHEN a TRE definition is found, THE TRE_Parser SHALL use Structure_Accessor to parse CEDATA into a map of field values
3. WHEN a TRE definition is not found, THE TRE_Parser SHALL preserve the raw CEDATA bytes as an unknown TRE
4. WHEN accessing TRE fields, THE TRE_Parser SHALL support nested field paths using dot notation (e.g., "field.subfield")
5. WHEN a TRE contains repeated fields, THE TRE_Parser SHALL use underscore-indexed naming (e.g., "field_0", "field_1")

### Requirement 3: TRE Location Handling

**User Story:** As a developer, I want to parse TREs from all valid NITF locations, so that I can access extension metadata regardless of where it is stored.

#### Acceptance Criteria

1. WHEN parsing file header TREs, THE TRE_Parser SHALL extract TREs from the UDHD (User Defined Header Data) field
2. WHEN parsing file header TREs, THE TRE_Parser SHALL extract TREs from the XHD (Extended Header Data) field
3. WHEN parsing image subheader TREs, THE TRE_Parser SHALL extract TREs from the UDID (User Defined Image Data) field
4. WHEN parsing image subheader TREs, THE TRE_Parser SHALL extract TREs from the IXSHD (Image Extended Subheader Data) field
5. WHEN parsing graphic subheader TREs, THE TRE_Parser SHALL extract TREs from the SXSHD (Graphic Extended Subheader Data) field
6. WHEN parsing text subheader TREs, THE TRE_Parser SHALL extract TREs from the TXSHD (Text Extended Subheader Data) field
7. WHEN parsing DES TREs, THE TRE_Parser SHALL extract TREs from the DESDATA field following the DES-defined subheader

### Requirement 4: Unknown TRE Preservation

**User Story:** As a developer, I want unknown TREs to be preserved exactly, so that I can round-trip files without data loss.

#### Acceptance Criteria

1. WHEN a TRE type has no definition in Structure_Registry, THE TRE_Parser SHALL store the complete TRE envelope (CETAG + CEL + CEDATA) as raw bytes
2. WHEN writing an unknown TRE, THE TRE_Writer SHALL output the preserved raw bytes exactly as read
3. FOR ALL unknown TREs, reading then writing SHALL produce byte-identical output

### Requirement 5: DES Subheader Parsing

**User Story:** As a developer, I want to parse DES subheaders, so that I can access data extension segment metadata and content.

#### Acceptance Criteria

1. WHEN parsing a DES subheader, THE DES_Parser SHALL extract the DE field (always "DE")
2. WHEN parsing a DES subheader, THE DES_Parser SHALL extract the DESID field as a 25-character identifier
3. WHEN parsing a DES subheader, THE DES_Parser SHALL extract the DESVER field as a 2-digit version number
4. WHEN parsing a DES subheader, THE DES_Parser SHALL extract all security classification fields (DESCLAS through DESCTLN)
5. WHEN DESID equals "TRE_OVERFLOW", THE DES_Parser SHALL extract the DESOFLW field indicating the overflowed header type
6. WHEN DESID equals "TRE_OVERFLOW", THE DES_Parser SHALL extract the DESITEM field indicating the overflowed segment index
7. WHEN parsing a DES subheader, THE DES_Parser SHALL extract DESSHL as the length of DES-defined subheader fields
8. WHEN DESSHL is greater than zero, THE DES_Parser SHALL extract DESSHF as the DES-defined subheader fields

### Requirement 6: TRE_OVERFLOW DES Handling

**User Story:** As a developer, I want to resolve TRE overflow references, so that I can access all TREs for a segment regardless of where they are stored.

#### Acceptance Criteria

1. WHEN a TRE_OVERFLOW DES is encountered, THE TRE_Parser SHALL parse the DESDATA as a sequence of TRE envelopes
2. WHEN resolving TREs for a segment, THE TRE_Parser SHALL combine TREs from the segment header with TREs from any associated TRE_OVERFLOW DES
3. WHEN DESOFLW is "UDHD" or "XHD", THE TRE_Parser SHALL associate the overflow TREs with the file header
4. WHEN DESOFLW is "UDSHD" or "IXSHD", THE TRE_Parser SHALL associate the overflow TREs with the image segment indicated by DESITEM
5. WHEN DESOFLW is "SXSHD", THE TRE_Parser SHALL associate the overflow TREs with the graphic segment indicated by DESITEM
6. WHEN DESOFLW is "TXSHD", THE TRE_Parser SHALL associate the overflow TREs with the text segment indicated by DESITEM

### Requirement 7: RES Structure Parsing

**User Story:** As a developer, I want to parse Reserved Extension Segments, so that I can handle future NITF extensions.

#### Acceptance Criteria

1. WHEN parsing a RES, THE RES_Parser SHALL extract the subheader using the NITF_02.10_RESSubheader definition
2. WHEN parsing a RES, THE RES_Parser SHALL preserve the RES data as raw bytes
3. WHEN a RES has an unknown type, THE RES_Parser SHALL store the complete segment for round-trip preservation

### Requirement 8: Configuration-Driven TRE Serialization

**User Story:** As a developer, I want to serialize TRE field values to binary, so that I can write TREs to NITF files.

#### Acceptance Criteria

1. WHEN writing a TRE, THE TRE_Writer SHALL accept a map of field names to values
2. WHEN writing a TRE, THE TRE_Writer SHALL lookup the definition from Structure_Registry using the pattern `TRE_{CETAG}`
3. WHEN a TRE definition is found, THE TRE_Writer SHALL use Structure_Writer to serialize the field values to CEDATA
4. WHEN writing a TRE, THE TRE_Writer SHALL validate field values against the definition constraints
5. IF a required field is missing, THEN THE TRE_Writer SHALL return a MissingRequired error
6. IF a field value exceeds its defined size, THEN THE TRE_Writer SHALL return a ValueTooLarge error

### Requirement 9: TRE Envelope Generation

**User Story:** As a developer, I want TRE envelopes to be generated correctly, so that the output files are valid NITF.

#### Acceptance Criteria

1. WHEN generating a TRE envelope, THE TRE_Writer SHALL write CETAG as a 6-character left-justified, space-padded string
2. WHEN generating a TRE envelope, THE TRE_Writer SHALL calculate CEL from the serialized CEDATA length
3. WHEN generating a TRE envelope, THE TRE_Writer SHALL write CEL as a 5-digit zero-padded numeric string
4. WHEN generating a TRE envelope, THE TRE_Writer SHALL write CEDATA immediately following CEL
5. WHEN CETAG contains fewer than 6 characters, THE TRE_Writer SHALL right-pad with spaces

### Requirement 10: TRE Placement in Headers

**User Story:** As a developer, I want TREs to be placed in the correct header locations, so that the output files follow NITF conventions.

#### Acceptance Criteria

1. WHEN adding TREs to a file header, THE TRE_Writer SHALL place them in UDHD if total length fits within UDHDL limit
2. WHEN UDHD is full, THE TRE_Writer SHALL place additional file header TREs in XHD
3. WHEN adding TREs to an image subheader, THE TRE_Writer SHALL place them in UDID if total length fits within UDIDL limit
4. WHEN UDID is full, THE TRE_Writer SHALL place additional image TREs in IXSHD
5. WHEN adding TREs to a graphic subheader, THE TRE_Writer SHALL place them in SXSHD
6. WHEN adding TREs to a text subheader, THE TRE_Writer SHALL place them in TXSHD

### Requirement 11: DES Subheader Generation

**User Story:** As a developer, I want to generate DES subheaders, so that I can write data extension segments to NITF files.

#### Acceptance Criteria

1. WHEN generating a DES subheader, THE DES_Writer SHALL write DE as "DE"
2. WHEN generating a DES subheader, THE DES_Writer SHALL write DESID as a 25-character left-justified, space-padded string
3. WHEN generating a DES subheader, THE DES_Writer SHALL write DESVER as a 2-digit zero-padded version number
4. WHEN generating a DES subheader, THE DES_Writer SHALL write all security classification fields with appropriate defaults or provided values
5. WHEN generating a TRE_OVERFLOW DES, THE DES_Writer SHALL write DESOFLW indicating the source header type
6. WHEN generating a TRE_OVERFLOW DES, THE DES_Writer SHALL write DESITEM indicating the source segment index
7. WHEN generating a DES subheader, THE DES_Writer SHALL calculate and write DESSHL based on DES-defined subheader content

### Requirement 12: TRE_OVERFLOW DES Generation

**User Story:** As a developer, I want overflow TREs to be automatically placed in TRE_OVERFLOW DES segments, so that large TRE sets are handled correctly.

#### Acceptance Criteria

1. WHEN TREs exceed the available space in a header field, THE TRE_Writer SHALL create a TRE_OVERFLOW DES for the excess
2. WHEN creating a TRE_OVERFLOW DES, THE TRE_Writer SHALL set DESID to "TRE_OVERFLOW" (space-padded to 25 characters)
3. WHEN creating a TRE_OVERFLOW DES, THE TRE_Writer SHALL set DESOFLW to indicate the source header type
4. WHEN creating a TRE_OVERFLOW DES, THE TRE_Writer SHALL set DESITEM to indicate the source segment index (000 for file header)
5. WHEN creating a TRE_OVERFLOW DES, THE TRE_Writer SHALL write the overflow TRE envelopes as DESDATA

### Requirement 13: TRE Definition File Format

**User Story:** As a developer, I want TRE definitions to use the standard KSY format, so that I can leverage the existing parser infrastructure.

#### Acceptance Criteria

1. THE TRE definition files SHALL be stored in `data/structures/tre/` directory
2. THE TRE definition files SHALL follow the Kaitai Struct YAML format
3. THE TRE definition files SHALL use the naming pattern `tre_{tag_lowercase}.ksy` (e.g., `tre_geolob.ksy`)
4. THE TRE definition files SHALL include a `meta` section with id, title, and endianness
5. THE TRE definition files SHALL include a `seq` section defining all fields in order
6. THE TRE definition files SHALL support conditional fields using `if` expressions
7. THE TRE definition files SHALL support repeated fields using `repeat` and `repeat-expr`

### Requirement 14: Structure Registry TRE Integration

**User Story:** As a developer, I want TRE definitions to be discoverable through the Structure Registry, so that I can use the standard lookup mechanism.

#### Acceptance Criteria

1. THE Structure_Registry SHALL search for TRE definitions using the pattern `TRE_{CETAG}`
2. THE Structure_Registry SHALL include `data/structures/tre/` in the default search paths
3. WHEN the `OSML_IO_STRUCTURE_PATH` environment variable includes a `tre/` subdirectory, THE Structure_Registry SHALL search that path for TRE definitions
4. THE Structure_Registry SHALL support runtime registration of custom TRE definitions
5. THE Structure_Registry SHALL cache loaded TRE definitions for performance

### Requirement 15: DES Definition File Format

**User Story:** As a developer, I want DES definitions to use the standard KSY format, so that I can parse DES-specific subheader fields.

#### Acceptance Criteria

1. THE DES definition files SHALL be stored in `data/structures/des/` directory
2. THE DES definition files SHALL follow the Kaitai Struct YAML format
3. THE DES definition files SHALL use the naming pattern `des_{desid_lowercase}.ksy` (e.g., `des_xml_data_content.ksy`)
4. THE DES definition files SHALL define the DES-specific subheader fields (DESSHF content)
5. THE Structure_Registry SHALL search for DES definitions using the pattern `DES_{DESID}`

### Requirement 16: TRE Validation

**User Story:** As a developer, I want TREs to be validated against their definitions, so that I can detect malformed data.

#### Acceptance Criteria

1. WHEN parsing a TRE, THE TRE_Parser SHALL validate that CETAG contains only alphanumeric characters and spaces
2. WHEN parsing a TRE, THE TRE_Parser SHALL validate that CEL matches the actual CEDATA length
3. WHEN parsing a known TRE, THE TRE_Parser SHALL validate field values against definition constraints
4. WHEN writing a TRE, THE TRE_Writer SHALL validate that all required fields are present
5. WHEN writing a TRE, THE TRE_Writer SHALL validate that field values fit within defined sizes
6. IF validation fails, THEN THE parser/writer SHALL return a descriptive error with field path and constraint violated

### Requirement 17: Round-Trip Consistency

**User Story:** As a developer, I want TRE parsing and writing to be symmetric, so that I can trust the system for data preservation.

#### Acceptance Criteria

1. FOR ALL valid TRE binary data, parsing then writing SHALL produce byte-identical output
2. FOR ALL valid TRE field value maps, writing then parsing SHALL produce equivalent field values
3. FOR ALL unknown TREs, the raw bytes SHALL be preserved exactly through read-write cycles

### Requirement 18: Python API Integration for TRE/DES

**User Story:** As a Python developer, I want to access TRE and DES data through the existing dataset and asset metadata interfaces, so that I can use a consistent API across all NITF metadata.

#### Acceptance Criteria

1. WHEN accessing file-level TREs, THE MetadataProvider SHALL include TRE fields in the dictionary interface with prefix "{CETAG}." (e.g., "GEOLOB.ARV")
2. WHEN accessing segment-level TREs, THE AssetProvider metadata SHALL include TRE fields in the dictionary interface with prefix "{CETAG}."
3. WHEN calling `as_dict(None)` on metadata, THE result SHALL include all TRE fields alongside header fields
4. WHEN calling `as_dict("GEOLOB")` on metadata, THE result SHALL return only fields from the GEOLOB TRE
5. WHEN accessing DES segments, THE DatasetReader SHALL expose them as assets with type Data and metadata containing DES subheader fields
6. WHEN writing TREs, THE AssetProvider metadata interface SHALL accept TRE field values with the "{CETAG}." prefix

### Requirement 19: Built-in TRE Definitions

**User Story:** As a developer, I want common JBP TREs to have built-in definitions, so that I can parse standard metadata without additional configuration.

#### Acceptance Criteria

1. THE library SHALL include built-in definitions for geospatial TREs: GEOLOB, GEOPSB, PRJPSB, MAPLOB
2. THE library SHALL include built-in definitions for sensor TREs: SENSRB, SENSRA, CSEXRA
3. THE library SHALL include built-in definitions for image chip TREs: ICHIPB, BCHIPA
4. THE library SHALL include built-in definitions for RPC TREs: RPC00A, RPC00B
5. THE library SHALL include built-in definitions for band TREs: BANDSB, BANDSA
6. THE library SHALL include built-in definitions for JPEG 2000 TREs: J2KLRA
7. THE library SHALL include built-in definitions for security TREs: SECURA
8. THE library SHALL include built-in definitions for history TREs: HISTOA
9. THE library SHALL include built-in definitions for comment TREs: COMNTA
10. THE library SHALL include built-in definitions for engineering TREs: ENGRDA

### Requirement 20: Built-in DES Definitions

**User Story:** As a developer, I want common DES types to have built-in definitions, so that I can parse standard data extensions without additional configuration.

#### Acceptance Criteria

1. THE library SHALL include built-in definition for TRE_OVERFLOW DES
2. THE library SHALL include built-in definition for XML_DATA_CONTENT DES
3. THE library SHALL include built-in definition for CSATTA DES (Coordinate System Attitude)
4. THE library SHALL include built-in definition for CSSHPA/CSSHPB DES (Coordinate System Shape)

### Requirement 21: Error Handling

**User Story:** As a developer, I want descriptive error messages for TRE/DES operations, so that I can diagnose issues effectively.

#### Acceptance Criteria

1. WHEN a TRE parse error occurs, THE error SHALL include the CETAG and byte offset
2. WHEN a TRE field error occurs, THE error SHALL include the field path and expected vs actual values
3. WHEN a DES parse error occurs, THE error SHALL include the DESID and segment index
4. WHEN a definition lookup fails, THE error SHALL include the searched name and paths checked
5. WHEN an overflow resolution fails, THE error SHALL include the source header type and segment index
