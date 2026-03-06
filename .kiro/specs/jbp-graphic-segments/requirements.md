# Requirements Document

## Introduction

This document specifies the requirements for implementing JBP Graphic Segments support in the osml-imagery-io library. Graphic segments in NITF files contain CGM (Computer Graphics Metafile) vector graphics data with associated metadata for display layering and positioning. The implementation will enable reading graphic segment metadata and raw CGM data, while leaving CGM parsing to external libraries.

## Glossary

- **Graphic_Segment**: A NITF segment containing CGM vector graphics data with a subheader describing display properties
- **CGM**: Computer Graphics Metafile - ISO/IEC 8632-3 standard for vector graphics interchange
- **SDLVL**: Display Level - z-order value (001-999) determining rendering order; higher values render on top
- **SALVL**: Attachment Level - indicates which segment this graphic attaches to (0 = unattached to image)
- **SLOC**: Segment Location - row/column offset relative to the attached segment's origin
- **SBND1**: Bounding Box Upper-Left - row/column coordinates of the graphic's upper-left corner
- **SBND2**: Bounding Box Lower-Right - row/column coordinates of the graphic's lower-right corner
- **SCOLOR**: Segment Color - indicates color capability ("C" for color, "M" for monochrome)
- **SFMT**: Segment Format - format of graphic data (always "C" for CGM in JBP)
- **CLEVEL**: Complexity Level - JBP conformance level constraining file features
- **GraphicsAssetProvider**: Trait interface for accessing graphic segment data and metadata
- **JBPGraphicsAssetProvider**: Concrete implementation of GraphicsAssetProvider for NITF files
- **MetadataProvider**: Interface for accessing segment metadata as key-value pairs

## Requirements

### Requirement 1: Graphic Subheader Parsing

**User Story:** As a developer, I want to parse graphic segment subheaders, so that I can access all graphic metadata fields defined in JBP Table 5.15-1.

#### Acceptance Criteria

1. WHEN a NITF file containing graphic segments is opened, THE JBPDatasetReader SHALL parse the graphic subheader fields: SY, SID, SNAME, SSCLAS, SSCLSY, SSCODE, SSCTLH, SSREL, SSDCTP, SSDCDT, SSDCXM, SSDG, SSDGDT, SSCLTX, SSCATP, SSCAUT, SSCRSN, SSCTLN, SSDWNG, SSDEVT, ENCRYP, SFMT, SSTRUCT, SDLVL, SALVL, SLOC, SBND1, SCOLOR, SBND2, SRES2, SXSHDL, SXSOFL, SXSHD
2. WHEN the SY field is not "SY", THE JBPDatasetReader SHALL return a parse error indicating invalid graphic segment marker
3. WHEN the SFMT field is not "C", THE JBPDatasetReader SHALL return a parse error indicating unsupported graphic format
4. WHEN the ENCRYP field is not "0", THE JBPDatasetReader SHALL return an error indicating encrypted graphics are not supported

### Requirement 2: Display Level Handling

**User Story:** As a developer, I want to access display level information, so that I can render graphics in the correct z-order.

#### Acceptance Criteria

1. THE MetadataProvider for a graphic segment SHALL expose the SDLVL field as an integer value between 001 and 999
2. WHEN multiple graphic segments exist in a file, THE JBPDatasetReader SHALL preserve unique SDLVL values for each segment
3. WHEN a graphic segment's SDLVL is queried via metadata, THE MetadataProvider SHALL return the display level as a string that can be parsed to an integer

### Requirement 3: Attachment Level Handling

**User Story:** As a developer, I want to access attachment level information, so that I can determine which image segment a graphic is attached to.

#### Acceptance Criteria

1. THE MetadataProvider for a graphic segment SHALL expose the SALVL field as an integer value
2. WHEN SALVL is 0, THE graphic segment SHALL be interpreted as unattached to any image segment
3. WHEN SALVL is greater than 0, THE graphic segment SHALL be interpreted as attached to the image segment with matching display level
4. WHEN a graphic segment's SALVL references a non-existent display level, THE JBPDatasetReader SHALL still parse the segment successfully (validation is caller's responsibility)

### Requirement 4: Location and Bounding Box Access

**User Story:** As a developer, I want to access graphic location and bounding box information, so that I can position graphics correctly relative to imagery.

#### Acceptance Criteria

1. THE MetadataProvider for a graphic segment SHALL expose SLOC as two integer values (row, column) representing the offset from the attached segment's origin
2. THE MetadataProvider for a graphic segment SHALL expose SBND1 as two integer values (row, column) representing the upper-left corner of the bounding box
3. THE MetadataProvider for a graphic segment SHALL expose SBND2 as two integer values (row, column) representing the lower-right corner of the bounding box
4. WHEN SBND1 row is greater than SBND2 row, THE bounding box SHALL be considered invalid but parsing SHALL still succeed
5. WHEN SBND1 column is greater than SBND2 column, THE bounding box SHALL be considered invalid but parsing SHALL still succeed

### Requirement 5: CGM Data Access

**User Story:** As a developer, I want to access raw CGM data, so that I can pass it to external CGM parsing libraries.

#### Acceptance Criteria

1. WHEN raw_asset() is called on a JBPGraphicsAssetProvider, THE provider SHALL return the complete CGM data bytes from the graphic segment data portion
2. THE JBPGraphicsAssetProvider SHALL return media_type as "image/cgm" for all graphic segments
3. WHEN the graphic segment data extends beyond file bounds, THE raw_asset() method SHALL return a CodecError
4. THE JBPGraphicsAssetProvider SHALL NOT parse or interpret the CGM data content

### Requirement 6: GraphicsAssetProvider Trait Implementation

**User Story:** As a developer, I want JBPGraphicsAssetProvider to implement the GraphicsAssetProvider trait, so that I can use polymorphic access patterns.

#### Acceptance Criteria

1. THE JBPGraphicsAssetProvider SHALL implement the GraphicsAssetProvider trait
2. THE JBPGraphicsAssetProvider SHALL implement all AssetProvider trait methods: key(), title(), description(), media_type(), roles(), asset_type(), raw_asset(), metadata(), as_any()
3. WHEN asset_type() is called, THE JBPGraphicsAssetProvider SHALL return AssetType::Graphics

### Requirement 7: Extended Subheader Data (TRE) Support

**User Story:** As a developer, I want to access TREs attached to graphic segments, so that I can read extended metadata.

#### Acceptance Criteria

1. WHEN SXSHDL is greater than 0, THE JBPDatasetReader SHALL parse the extended subheader data as TRE envelopes
2. WHEN SXSOFL indicates overflow, THE JBPDatasetReader SHALL resolve overflow TREs from the appropriate DES segment
3. THE MetadataProvider for a graphic segment SHALL expose parsed TREs through the standard TRE access interface

### Requirement 8: CLEVEL Aggregate Size Validation

**User Story:** As a developer, I want graphic segment sizes validated against CLEVEL constraints, so that I can ensure file conformance.

#### Acceptance Criteria

1. WHEN writing a NITF file at CLEVEL 03, THE aggregate size of all graphic segments SHALL NOT exceed 1 MB
2. WHEN writing a NITF file at CLEVEL 05 or higher, THE aggregate size of all graphic segments SHALL NOT exceed 2 MB
3. WHEN the aggregate graphic segment size exceeds the CLEVEL limit during writing, THE JBPDatasetWriter SHALL return a validation error

### Requirement 9: Python Bindings

**User Story:** As a Python developer, I want to access graphic segments through Python bindings, so that I can use the library from Python code.

#### Acceptance Criteria

1. THE PyGraphicsAssetProvider SHALL expose all AssetProvider properties: key, title, description, media_type, roles, asset_type
2. THE PyGraphicsAssetProvider SHALL expose get_raw_asset() returning a BytesIO object containing CGM data
3. THE PyGraphicsAssetProvider SHALL expose get_metadata() returning a PyMetadataProvider for accessing graphic metadata
4. WHEN a graphic segment is accessed via DatasetReader.get_asset(), THE returned provider SHALL be usable as a GraphicsAssetProvider in Python

### Requirement 10: Documentation Updates

**User Story:** As a developer, I want updated documentation, so that I can understand how to use the GraphicsAssetProvider API.

#### Acceptance Criteria

1. THE API_DESIGN.md document SHALL include a GraphicsAssetProvider section documenting the interface and usage patterns
2. THE JBP_ROADMAP.md document SHALL mark Phase 1 (Graphic Segments) as complete after implementation
3. THE JBP_CLEVEL_ASSESSMENT.md document SHALL update the Graphic Segments section to show implemented status
