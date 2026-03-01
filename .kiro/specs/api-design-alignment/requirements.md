# Requirements Document

## Introduction

This document specifies the requirements for aligning the API design document (`docs/API_DESIGN.md`) with the actual implementation in the codebase. The goal is to ensure consistency between documentation and code, adopt property-style accessors throughout the API, and remove deprecated concepts like fsspec filesystem abstraction.

## Glossary

- **API_DESIGN**: The design document located at `docs/API_DESIGN.md` that describes the intended API
- **IO_Class**: The factory class that provides `open()` method for creating readers/writers
- **DatasetReader**: Abstract class for reading geospatial datasets
- **DatasetWriter**: Abstract class for writing geospatial datasets
- **AssetProvider**: Base class for all asset types within a dataset
- **ImageAssetProvider**: Specialized provider for blocked/tiled image access
- **TextAssetProvider**: Specialized provider for text content access
- **DataAssetProvider**: Specialized provider for structured data (XML/JSON) access
- **GraphicsAssetProvider**: Specialized provider for vector graphics access
- **MemoryImageAssetProvider**: In-memory image provider for synthetic images
- **MetadataProvider**: Interface for accessing raw and structured metadata
- **SimpleMetadataProvider**: Mutable metadata provider for setting encoding hints
- **Property_Style_Accessor**: A Python property (getter) instead of a method with `get_` prefix
- **PyStructure_Classes**: Parser infrastructure classes (PyStructureAccessor, PyStructureDefinition, PyStructureRegistry, PyStructureWriter)

## Requirements

### Requirement 1: IO Class API Alignment

**User Story:** As a developer, I want the IO class to accept a list of URIs as documented, so that I can work with multi-file datasets.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL remove all mentions of fsspec filesystem parameter from IO.open() method
2. WHEN updating the implementation THEN the System SHALL change IO.open() to accept a list of URI strings instead of a single URI
3. WHEN updating the implementation THEN the System SHALL have implementations that only expect one file choose the first item in the list and ignore others
4. WHEN updating API_DESIGN.md THEN the System SHALL add the format parameter to IO.open() method signature
5. WHEN updating API_DESIGN.md THEN the System SHALL update the IO class diagram to reflect: `open(paths: List[str], mode: str, format: Optional[str])`
6. WHEN updating usage examples THEN the System SHALL change `streamline.open("file.nitf")` to use the actual module name `IO.open(["file.nitf"], "r")`

### Requirement 2: DatasetReader Property Accessors

**User Story:** As a developer, I want DatasetReader to use property-style accessors for metadata, so that the API is consistent and Pythonic.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL replace `get_metadata()` method with a `metadata` property in the DatasetReader class diagram
2. WHEN updating the Python bindings THEN the System SHALL change `get_metadata()` to a `metadata` property getter in PyDatasetReader
3. WHEN updating usage examples THEN the System SHALL change `dataset.get_metadata()` to `dataset.metadata`

### Requirement 3: DatasetWriter API Consolidation

**User Story:** As a developer, I want a unified add_asset API that handles all asset types including images, so that I don't need separate methods for different asset types.

#### Acceptance Criteria

1. WHEN updating the Python bindings THEN the System SHALL remove the separate `add_image_asset()` method from PyDatasetWriter
2. THE existing `add_asset()` method already accepts AssetProvider which MemoryImageAssetProvider subclasses through ImageAssetProvider
3. WHEN updating API_DESIGN.md THEN the System SHALL document that add_asset accepts any AssetProvider including MemoryImageAssetProvider
4. WHEN updating API_DESIGN.md THEN the System SHALL replace `set_metadata()` method with a `metadata` property setter

### Requirement 4: AssetProvider Property Accessors

**User Story:** As a developer, I want AssetProvider to use property-style accessors consistently, so that the API follows Python conventions.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL replace all `get_*()` methods with property accessors in the AssetProvider class diagram
2. WHEN updating API_DESIGN.md THEN the System SHALL show `key`, `title`, `description`, `media_type`, `roles`, `asset_type` as properties
3. WHEN updating API_DESIGN.md THEN the System SHALL add the `from_bytes()` static method to the AssetProvider class diagram
4. THE implementation already uses property-style accessors, so no code changes are needed for AssetProvider

### Requirement 5: ImageAssetProvider Optional Bands Parameter

**User Story:** As a developer, I want the bands parameter to be optional in get_block(), so that I can retrieve all bands by default.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL change `get_block(block_row, block_col, resolution_level, bands: List[int])` to have bands as optional
2. THE implementation already has bands as optional, so no code changes are needed

### Requirement 6: TextAssetProvider Property Accessors

**User Story:** As a developer, I want TextAssetProvider to use property-style accessors, so that the API is consistent with other providers.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL replace `get_text()` with a `text` property
2. WHEN updating API_DESIGN.md THEN the System SHALL replace `get_encoding()` with an `encoding` property
3. WHEN updating API_DESIGN.md THEN the System SHALL replace `get_format()` with a `format` property
4. WHEN updating the Python bindings THEN the System SHALL change `get_text()` method to a `text` property getter in PyTextAssetProvider

### Requirement 7: DataAssetProvider Property Accessors and XML Parsing

**User Story:** As a developer, I want DataAssetProvider to use property-style accessors and return proper XML objects, so that I can work with parsed data directly.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL replace `get_mime_type()` with a `mime_type` property
2. WHEN updating the Python bindings THEN the System SHALL change `get_mime_type()` to a `mime_type` property getter
3. WHEN updating the implementation THEN the System SHALL modify `parse_as_xml()` to return an ElementTree object instead of a string
4. IF security concerns exist with XML parsing THEN the System SHALL use defusedxml or equivalent safe parsing library

### Requirement 8: MemoryImageAssetProvider Documentation

**User Story:** As a developer, I want the MemoryImageAssetProvider documentation to match the actual implementation, so that I can create synthetic images correctly.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL update MemoryImageAssetProvider constructor to match actual `create()` static method signature
2. WHEN updating API_DESIGN.md THEN the System SHALL document `set_full_image()` and `set_block()` methods for setting image data
3. WHEN updating API_DESIGN.md THEN the System SHALL show that set_full_image accepts ndarray with shape (bands, rows, cols)

### Requirement 9: Remove Deprecated Concepts

**User Story:** As a developer, I want the documentation to only show supported features, so that I don't try to use deprecated or unimplemented functionality.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL remove FileDatasetReader class and all references
2. WHEN updating API_DESIGN.md THEN the System SHALL remove FileDatasetWriter class and all references  
3. WHEN updating API_DESIGN.md THEN the System SHALL remove FilesDatasetReader class and all references
4. WHEN updating API_DESIGN.md THEN the System SHALL remove FilesDatasetWriter class and all references
5. WHEN updating API_DESIGN.md THEN the System SHALL remove all mentions of fsspec throughout the document

### Requirement 10: Add SimpleMetadataProvider Documentation

**User Story:** As a developer, I want documentation for SimpleMetadataProvider, so that I can programmatically set metadata values for encoding hints.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL add SimpleMetadataProvider class to the documentation
2. WHEN updating API_DESIGN.md THEN the System SHALL document the `set()`, `get()`, `remove()`, and `clear()` methods
3. WHEN updating API_DESIGN.md THEN the System SHALL show that SimpleMetadataProvider extends MetadataProvider
4. WHEN updating API_DESIGN.md THEN the System SHALL include usage examples showing how to set encoding hints

### Requirement 11: Add PyStructure Classes Documentation

**User Story:** As a developer, I want documentation for the parser infrastructure classes, so that I can configure structure definitions for binary parsing.

#### Acceptance Criteria

1. WHEN updating API_DESIGN.md THEN the System SHALL add a new section describing the PyStructure classes
2. WHEN updating API_DESIGN.md THEN the System SHALL document PyStructureRegistry for managing structure definitions
3. WHEN updating API_DESIGN.md THEN the System SHALL document PyStructureDefinition for representing parsed KSY files
4. WHEN updating API_DESIGN.md THEN the System SHALL document PyStructureAccessor for reading structure fields from binary data
5. WHEN updating API_DESIGN.md THEN the System SHALL document PyStructureWriter for encoding values according to structure definitions
6. WHEN updating API_DESIGN.md THEN the System SHALL include usage examples for the parser infrastructure

### Requirement 12: Update Usage Examples

**User Story:** As a developer, I want the usage examples to reflect the actual API, so that I can copy and use them directly.

#### Acceptance Criteria

1. WHEN updating usage examples THEN the System SHALL use property accessors instead of getter methods
2. WHEN updating usage examples THEN the System SHALL use the correct import paths and class names
3. WHEN updating usage examples THEN the System SHALL remove any examples using deprecated features (fsspec, ImageOperation)
4. WHEN updating usage examples THEN the System SHALL add examples for SimpleMetadataProvider and PyStructure classes
