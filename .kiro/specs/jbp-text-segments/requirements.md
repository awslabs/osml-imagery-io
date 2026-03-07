# Requirements Document

## Introduction

This document specifies the requirements for implementing JBP Text Segments support in the osml-imagery-io library. Text segments in NITF files contain textual data with associated metadata for display positioning and character encoding. The implementation will enable reading text segment metadata and content, with proper handling of different text format codes (STA, MTF, UT1, U8S) and line delimiter normalization.

## Glossary

- **Text_Segment**: A NITF segment containing textual data with a subheader describing format and display properties
- **TXTFMT**: Text Format code indicating the character encoding and formatting rules
- **STA**: Standard BCS (ASCII) text format with CR/LF line delimiters
- **MTF**: Message Text Formatting per STANAG 5500/MIL-STD-6040
- **UT1**: Legacy ECS (Extended Character Set) text formatting
- **U8S**: UTF-8 text formatting (modern replacement for UT1)
- **TXTALVL**: Text Attachment Level - indicates which segment this text attaches to (0 = unattached)
- **TXTDT**: Text Date and Time in CCYYMMDDhhmmss format
- **TXTITL**: Text Title - 80-character human-readable title
- **TEXTID**: Text Identifier - 7-character unique identifier
- **TextAssetProvider**: Trait interface for accessing text segment data and metadata
- **JBPTextAssetProvider**: Concrete implementation of TextAssetProvider for NITF files
- **BufferedTextAssetProvider**: In-memory implementation for creating text segments programmatically
- **MetadataProvider**: Interface for accessing segment metadata as key-value pairs
- **CR/LF**: Carriage Return (0x0D) followed by Line Feed (0x0A) - required line delimiter for JBP text

## Requirements

### Requirement 1: Text Subheader Parsing

**User Story:** As a developer, I want to parse text segment subheaders, so that I can access all text metadata fields defined in JBP Table 5.17-1.

#### Acceptance Criteria

1. WHEN a NITF file containing text segments is opened, THE JBPDatasetReader SHALL parse the text subheader fields using the data-driven parser infrastructure (StructureDefinition pattern): TE, TEXTID, TXTALVL, TXTDT, TXTITL, security fields (TSCLAS through TSDEVT), ENCRYP, TXTFMT, TXSHDL, TXSOFL, TXSHD
2. WHEN the TE field is not "TE", THE JBPDatasetReader SHALL return a parse error indicating invalid text segment marker
3. WHEN the ENCRYP field is not "0", THE JBPDatasetReader SHALL return an error indicating encrypted text is not supported
4. WHEN the TXTFMT field contains an unrecognized format code, THE JBPDatasetReader SHALL still parse the segment successfully and expose the raw format code

### Requirement 2: Text Format Code Handling

**User Story:** As a developer, I want to access text format information, so that I can properly interpret the text content encoding.

#### Acceptance Criteria

1. WHEN encoding() is called on a JBPTextAssetProvider with TXTFMT="STA", THE provider SHALL return "ASCII"
2. WHEN encoding() is called on a JBPTextAssetProvider with TXTFMT="U8S", THE provider SHALL return "UTF-8"
3. WHEN encoding() is called on a JBPTextAssetProvider with TXTFMT="UT1", THE provider SHALL return "ECS" (Extended Character Set)
4. WHEN encoding() is called on a JBPTextAssetProvider with TXTFMT="MTF", THE provider SHALL return "MTF"
5. THE MetadataProvider for a text segment SHALL expose the raw TXTFMT code through as_dict()["TXTFMT"]

### Requirement 3: Attachment Level Handling

**User Story:** As a developer, I want to access attachment level information, so that I can determine which segment a text is attached to.

#### Acceptance Criteria

1. THE MetadataProvider for a text segment SHALL expose the TXTALVL field as a string value
2. WHEN TXTALVL is "000", THE text segment SHALL be interpreted as unattached to any other segment
3. WHEN TXTALVL is greater than "000", THE text segment SHALL be interpreted as attached to the segment with matching display level
4. WHEN a text segment's TXTALVL references a non-existent display level, THE JBPDatasetReader SHALL still parse the segment successfully (validation is caller's responsibility)

### Requirement 4: Text Content Access

**User Story:** As a developer, I want to access text content with proper encoding handling, so that I can read the text data correctly.

#### Acceptance Criteria

1. WHEN text() is called on a JBPTextAssetProvider, THE provider SHALL return the decoded text content as a String
2. WHEN text() is called on a text segment with TXTFMT="STA", THE provider SHALL decode the content as ASCII and normalize CR/LF to platform-native line endings
3. WHEN text() is called on a text segment with TXTFMT="U8S", THE provider SHALL decode the content as UTF-8 and normalize CR/LF to platform-native line endings
4. WHEN text() is called on a text segment with TXTFMT="UT1", THE provider SHALL decode the content using ECS mapping and normalize CR/LF to platform-native line endings
5. WHEN the text content contains invalid encoding sequences, THE text() method SHALL return a CodecError
6. WHEN raw_asset() is called on a JBPTextAssetProvider, THE provider SHALL return the raw text bytes without any normalization

### Requirement 5: TextAssetProvider Trait Implementation

**User Story:** As a developer, I want JBPTextAssetProvider to implement the TextAssetProvider trait, so that I can use polymorphic access patterns.

#### Acceptance Criteria

1. THE JBPTextAssetProvider SHALL implement the TextAssetProvider trait
2. THE JBPTextAssetProvider SHALL implement all AssetProvider trait methods: key(), title(), description(), media_type(), roles(), asset_type(), raw_asset(), metadata(), as_any()
3. WHEN asset_type() is called, THE JBPTextAssetProvider SHALL return AssetType::Text
4. WHEN media_type() is called on a text segment with TXTFMT="STA", THE JBPTextAssetProvider SHALL return "text/plain; charset=us-ascii"
5. WHEN media_type() is called on a text segment with TXTFMT="U8S", THE JBPTextAssetProvider SHALL return "text/plain; charset=utf-8"
6. WHEN media_type() is called on a text segment with TXTFMT="UT1", THE JBPTextAssetProvider SHALL return "text/plain; charset=iso-8859-1"
7. WHEN media_type() is called on a text segment with TXTFMT="MTF", THE JBPTextAssetProvider SHALL return "text/plain"

### Requirement 6: Extended Subheader Data (TRE) Support

**User Story:** As a developer, I want to access TREs attached to text segments, so that I can read extended metadata.

#### Acceptance Criteria

1. WHEN TXSHDL is greater than 0, THE JBPDatasetReader SHALL parse the extended subheader data as TRE envelopes
2. WHEN TXSOFL indicates overflow, THE JBPDatasetReader SHALL resolve overflow TREs from the appropriate DES segment
3. THE MetadataProvider for a text segment SHALL expose parsed TREs through the standard TRE access interface

### Requirement 7: BufferedTextAssetProvider

**User Story:** As a developer, I want to create text segments in memory, so that I can write NITF files with text content.

#### Acceptance Criteria

1. THE BufferedTextAssetProvider SHALL implement the TextAssetProvider trait
2. THE BufferedTextAssetProvider SHALL accept text content as a String during construction
3. THE BufferedTextAssetProvider SHALL accept an encoding parameter ("ASCII", "UTF-8", "ECS", "MTF") during construction
4. WHEN raw_asset() is called on a BufferedTextAssetProvider with encoding="ASCII", THE provider SHALL return text bytes with CR/LF line delimiters
5. WHEN raw_asset() is called on a BufferedTextAssetProvider with encoding="UTF-8", THE provider SHALL return UTF-8 encoded bytes with CR/LF line delimiters
6. THE BufferedTextAssetProvider SHALL convert platform-native line endings to CR/LF when generating raw bytes

### Requirement 8: Python Bindings

**User Story:** As a Python developer, I want to access text segments through Python bindings, so that I can use the library from Python code.

#### Acceptance Criteria

1. THE PyTextAssetProvider SHALL expose all AssetProvider properties: key, title, description, media_type, roles, asset_type
2. THE PyTextAssetProvider SHALL expose get_raw_asset() returning a BytesIO object containing raw text bytes
3. THE PyTextAssetProvider SHALL expose get_metadata() returning a PyMetadataProvider for accessing text metadata
4. THE PyTextAssetProvider SHALL expose text property returning the decoded text content as a string
5. THE PyTextAssetProvider SHALL expose encoding property returning the normalized encoding name
6. THE PyTextAssetProvider SHALL expose format property returning the text format identifier
7. WHEN a text segment is accessed via DatasetReader.get_asset(), THE returned provider SHALL be usable as a TextAssetProvider in Python

### Requirement 9: Documentation Updates

**User Story:** As a developer, I want updated documentation, so that I can understand how to use the TextAssetProvider API.

#### Acceptance Criteria

1. THE API_DESIGN.md document SHALL include a TextAssetProvider section documenting the interface and usage patterns
2. THE JBP_ROADMAP.md document SHALL mark Phase 2 (Text Segments) as complete after implementation
3. THE JBP_CLEVEL_ASSESSMENT.md document SHALL update the Text Segments section to show implemented status
