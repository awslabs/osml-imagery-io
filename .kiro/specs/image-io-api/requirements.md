# Requirements Document

## Introduction

This document specifies the requirements for the foundational image IO API and data structures for the aws-osml-io library. The library provides high-performance geospatial image format codecs implemented in Rust with Python bindings via PyO3. The API is built around the concept of Datasets as collections of related assets (images, graphics, text, data), following STAC (SpatioTemporal Asset Catalog) design patterns. This specification focuses on establishing the base generic types and interfaces that format-specific implementations will extend.

## Glossary

- **Dataset**: A collection of related assets (images, graphics, text, data) with associated metadata
- **Asset**: A single resource within a dataset, identified by a unique string key
- **AssetType**: An enumeration of asset categories: Image, Text, Graphics, Data
- **AssetProvider**: An abstract interface for accessing asset content and metadata
- **ImageAssetProvider**: A specialized AssetProvider for blocked/tiled image access
- **MetadataProvider**: An interface for accessing raw bytes and dictionary-based metadata
- **Block**: A rectangular tile of image data used for efficient memory access
- **Resolution_Level**: A level in an image pyramid, where 0 is full resolution
- **IO_Factory**: The factory class that selects appropriate reader/writer implementations
- **DatasetReader**: An abstract class for reading datasets from storage
- **DatasetWriter**: An abstract class for writing datasets to storage

## Requirements

### Requirement 1: Dataset Reader Interface

**User Story:** As a developer, I want to read geospatial datasets through a unified interface, so that I can access imagery and metadata without knowing format-specific details.

#### Acceptance Criteria

1. THE DatasetReader SHALL provide a `get_asset(key: str)` method that returns an AssetProvider for the specified asset key
2. THE DatasetReader SHALL provide a `get_asset_keys(asset_type: Optional[AssetType], roles: Optional[List[str]])` method that returns a list of asset keys matching the filter criteria
3. THE DatasetReader SHALL provide a `has_asset(key: str)` method that returns true if an asset with the given key exists
4. THE DatasetReader SHALL provide a `get_metadata()` method that returns a MetadataProvider for dataset-level metadata
5. THE DatasetReader SHALL implement Python context manager protocol via `__enter__` and `__exit__` methods
6. THE DatasetReader SHALL provide a `close()` method that releases all resources
7. WHEN `get_asset` is called with a non-existent key, THEN THE DatasetReader SHALL raise a KeyError

### Requirement 2: Dataset Writer Interface

**User Story:** As a developer, I want to write geospatial datasets through a unified interface, so that I can produce imagery files without knowing format-specific encoding details.

#### Acceptance Criteria

1. THE DatasetWriter SHALL provide an `add_asset(key: str, provider: AssetProvider, title: str, description: str, roles: List[str])` method that adds an asset to the dataset
2. THE DatasetWriter SHALL provide a `set_metadata(metadata: MetadataProvider)` method that sets dataset-level metadata
3. THE DatasetWriter SHALL implement Python context manager protocol via `__enter__` and `__exit__` methods
4. THE DatasetWriter SHALL provide a `close()` method that finalizes the dataset and releases resources
5. WHEN `add_asset` is called with a duplicate key, THEN THE DatasetWriter SHALL raise a ValueError
6. WHEN `close` is called, THEN THE DatasetWriter SHALL flush all pending data to storage

### Requirement 3: IO Factory

**User Story:** As a developer, I want a simple factory function to open datasets, so that I can work with files without manually selecting format-specific implementations.

#### Acceptance Criteria

1. THE IO_Factory SHALL provide an `open(uri: str, mode: str)` function that returns a DatasetReader or DatasetWriter
2. WHEN mode is "r", THEN THE IO_Factory SHALL return a DatasetReader
3. WHEN mode is "w", THEN THE IO_Factory SHALL return a DatasetWriter
4. THE IO_Factory SHALL automatically detect the file format from the file extension and magic bytes
5. WHEN the file format cannot be determined, THEN THE IO_Factory SHALL raise an UnsupportedFormatError
6. THE IO_Factory SHALL accept local file paths as URI strings
7. THE IO_Factory SHALL accept S3 URLs (s3://bucket/key) as URI strings
8. THE low-level Rust implementation SHALL handle filesystem access details internally based on the URI scheme

### Requirement 4: Asset Provider Base Interface

**User Story:** As a developer, I want a common interface for all asset types, so that I can discover and access assets uniformly regardless of their content type.

#### Acceptance Criteria

1. THE AssetProvider SHALL provide a `get_key()` method that returns the unique string identifier for the asset
2. THE AssetProvider SHALL provide a `get_title()` method that returns a human-readable title
3. THE AssetProvider SHALL provide a `get_description()` method that returns a detailed description
4. THE AssetProvider SHALL provide a `get_media_type()` method that returns the MIME type of the asset
5. THE AssetProvider SHALL provide a `get_roles()` method that returns a list of semantic roles (e.g., "data", "thumbnail", "metadata")
6. THE AssetProvider SHALL provide a `get_asset_type()` method that returns the AssetType enumeration value
7. THE AssetProvider SHALL provide a `get_raw_asset()` method that returns the raw bytes as a BytesIO object
8. THE AssetProvider SHALL provide a `get_metadata()` method that returns a MetadataProvider for asset-level metadata

### Requirement 5: Image Asset Provider Interface

**User Story:** As a developer, I want efficient blocked access to large imagery, so that I can process images larger than available memory.

#### Acceptance Criteria

1. THE ImageAssetProvider SHALL provide a `has_block(block_row: int, block_col: int, resolution_level: int)` method that returns true if the specified block exists
2. THE ImageAssetProvider SHALL provide a `get_block(block_row: int, block_col: int, resolution_level: int, bands: List[int])` method that returns the block data as a numpy ndarray
3. THE ImageAssetProvider SHALL provide a `num_resolution_levels` property that returns the number of pyramid levels
4. THE ImageAssetProvider SHALL provide a `num_bands` property that returns the number of spectral bands
5. THE ImageAssetProvider SHALL provide `num_rows` and `num_columns` properties that return the image dimensions at full resolution
6. THE ImageAssetProvider SHALL provide `num_pixels_per_block_horizontal` and `num_pixels_per_block_vertical` properties that return the block dimensions
7. THE ImageAssetProvider SHALL provide `num_bits_per_pixel` and `actual_bits_per_pixel` properties for bit depth information
8. THE ImageAssetProvider SHALL provide a `pixel_value_type` property that returns the numpy dtype
9. THE ImageAssetProvider SHALL provide a `pad_pixel_value` property that returns the value used for padding incomplete blocks
10. THE ImageAssetProvider SHALL provide an `image_shape` property that returns a tuple of (rows, columns, bands)
11. THE ImageAssetProvider SHALL provide a `block_shape` property that returns a tuple of (block_rows, block_cols, bands)
12. THE ImageAssetProvider SHALL provide a `block_grid_size` property that returns a tuple of (num_block_rows, num_block_cols)
13. WHEN `get_block` is called with an invalid block coordinate, THEN THE ImageAssetProvider SHALL raise an IndexError
14. WHEN `get_block` is called with an invalid resolution level, THEN THE ImageAssetProvider SHALL raise a ValueError

### Requirement 6: Metadata Provider Interface

**User Story:** As a developer, I want to access both raw and structured metadata, so that I can work with format-specific headers and parsed metadata dictionaries.

#### Acceptance Criteria

1. THE MetadataProvider SHALL provide a `raw` property that returns the raw metadata bytes as a BytesIO object
2. THE MetadataProvider SHALL provide an `as_dict(name: Optional[str])` method that returns metadata as a dictionary
3. WHEN `as_dict` is called with a name parameter, THEN THE MetadataProvider SHALL return only the named metadata section
4. WHEN `as_dict` is called without a name parameter, THEN THE MetadataProvider SHALL return all metadata sections

### Requirement 7: Text Asset Provider Interface

**User Story:** As a developer, I want to access text content from datasets, so that I can read embedded text segments with proper encoding handling.

#### Acceptance Criteria

1. THE TextAssetProvider SHALL provide a `get_text()` method that returns the decoded text content as a string
2. THE TextAssetProvider SHALL provide a `get_encoding()` method that returns the character encoding (e.g., "UTF-8", "ASCII")
3. THE TextAssetProvider SHALL provide a `get_format()` method that returns the text format identifier

### Requirement 8: Data Asset Provider Interface

**User Story:** As a developer, I want to access structured data from datasets, so that I can parse embedded XML and JSON content.

#### Acceptance Criteria

1. THE DataAssetProvider SHALL provide a `get_mime_type()` method that returns the MIME type of the data
2. THE DataAssetProvider SHALL provide a `parse_as_xml()` method that returns an ElementTree object
3. THE DataAssetProvider SHALL provide a `parse_as_json()` method that returns a dictionary
4. WHEN `parse_as_xml` is called on non-XML content, THEN THE DataAssetProvider SHALL raise a ParseError
5. WHEN `parse_as_json` is called on non-JSON content, THEN THE DataAssetProvider SHALL raise a ParseError

### Requirement 9: Graphics Asset Provider Interface

**User Story:** As a developer, I want to access vector graphics and annotations from datasets, so that I can work with embedded graphic overlays.

#### Acceptance Criteria

1. THE GraphicsAssetProvider SHALL extend AssetProvider with graphics-specific access methods
2. THE GraphicsAssetProvider SHALL provide access to vector graphics data through the base `get_raw_asset()` method

### Requirement 10: Asset Type Enumeration

**User Story:** As a developer, I want a well-defined enumeration of asset types, so that I can filter and categorize assets consistently.

#### Acceptance Criteria

1. THE AssetType enumeration SHALL include Image, Text, Graphics, and Data variants
2. THE AssetType SHALL be usable as a filter parameter in `get_asset_keys`
3. THE AssetType SHALL be comparable for equality

### Requirement 11: Python Bindings

**User Story:** As a Python developer, I want to use this library from Python, so that I can integrate it with numpy and existing Python geospatial workflows.

#### Acceptance Criteria

1. THE library SHALL expose all public classes and functions via PyO3 bindings
2. THE library SHALL return numpy ndarrays from `get_block` methods
3. THE library SHALL support Python context managers for DatasetReader and DatasetWriter
4. THE library SHALL raise appropriate Python exceptions (KeyError, ValueError, IndexError, IOError) for error conditions
