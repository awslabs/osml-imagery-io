# Requirements Document

## Introduction

This document specifies the requirements for the Dataset Writer Encoding Hints feature. The feature enables passing format-specific encoding hints (such as NITF's IMODE, IC, NPPBH, NPPBV, COMRAT) through the existing MetadataProvider interface on assets, rather than having format-specific parameters leak into abstract interfaces like MemoryImageAssetProvider.

The goal is to maintain clean abstraction boundaries while allowing users to control format-specific encoding options when writing imagery files.

## Glossary

- **Encoding_Hint**: A format-specific configuration value that controls how image data is encoded when written to a file (e.g., IMODE, IC, NPPBH)
- **SimpleMetadataProvider**: A mutable helper class that implements MetadataProvider and allows setting key-value pairs programmatically
- **MetadataProvider**: An existing trait that provides access to raw and structured metadata from datasets and assets
- **JBPDatasetWriter**: The writer component responsible for creating NITF/NSIF files
- **MemoryImageAssetProvider**: An in-memory image asset provider used for creating synthetic images
- **IMODE**: NITF field specifying band interleave mode (B=block, P=pixel, R=row, S=sequential)
- **IC**: NITF field specifying image compression code
- **NPPBH**: NITF field specifying number of pixels per block horizontal (1-8192)
- **NPPBV**: NITF field specifying number of pixels per block vertical (1-8192)
- **COMRAT**: NITF field specifying compression ratio for compressed images

## Requirements

### Requirement 1: SimpleMetadataProvider Implementation

**User Story:** As a developer, I want a mutable metadata provider class, so that I can programmatically set encoding hints and other metadata values for assets.

#### Acceptance Criteria

1. THE SimpleMetadataProvider SHALL implement the MetadataProvider trait
2. THE SimpleMetadataProvider SHALL provide a `set(key, value)` method to store string key-value pairs
3. THE SimpleMetadataProvider SHALL provide a `get(key)` method to retrieve stored values
4. WHEN `as_dict(None)` is called, THE SimpleMetadataProvider SHALL return all stored key-value pairs as a HashMap
5. WHEN `as_dict(Some(prefix))` is called, THE SimpleMetadataProvider SHALL return only key-value pairs where the key starts with the given prefix
6. THE SimpleMetadataProvider SHALL be thread-safe (Send + Sync)
7. THE SimpleMetadataProvider SHALL be exposed to Python via PyO3 bindings
8. THE SimpleMetadataProvider SHALL provide a constructor that accepts an existing MetadataProvider and copies all its key-value pairs, allowing users to duplicate metadata and selectively update fields

### Requirement 2: MemoryImageAssetProvider Metadata Support

**User Story:** As a developer, I want to attach custom metadata to MemoryImageAssetProvider instances, so that I can pass encoding hints to the writer.

#### Acceptance Criteria

1. THE MemoryImageAssetProvider SHALL accept an optional MetadataProvider during construction
2. WHEN a MetadataProvider is provided, THE MemoryImageAssetProvider SHALL return it from the `metadata()` method
3. WHEN no MetadataProvider is provided, THE MemoryImageAssetProvider SHALL return an EmptyMetadataProvider from the `metadata()` method
4. THE MemoryImageAssetProvider SHALL remove the `imode` parameter from its configuration, as IMODE will be provided via metadata

### Requirement 3: JBPDatasetWriter Encoding Hint Reading

**User Story:** As a developer, I want the NITF writer to read encoding hints from asset metadata, so that I can control output format without modifying abstract interfaces.

#### Acceptance Criteria

1. WHEN writing an image segment, THE JBPDatasetWriter SHALL call `asset.metadata().as_dict(None)` to retrieve metadata
2. WHEN the metadata contains an "IMODE" field with value "B", "P", "R", or "S", THE JBPDatasetWriter SHALL use that value for the image interleave mode
3. WHEN the metadata contains an "IC" field, THE JBPDatasetWriter SHALL use that value for the image compression code
4. WHEN the metadata contains an "NPPBH" field with a numeric value between 1 and 8192, THE JBPDatasetWriter SHALL use that value for pixels per block horizontal
5. WHEN the metadata contains an "NPPBV" field with a numeric value between 1 and 8192, THE JBPDatasetWriter SHALL use that value for pixels per block vertical
6. WHEN the metadata contains a "COMRAT" field, THE JBPDatasetWriter SHALL use that value for the compression ratio
7. WHEN an encoding hint field is not present in metadata, THE JBPDatasetWriter SHALL use sensible default values (IMODE="B", IC="NC", NPPBH=image width, NPPBV=image height)

### Requirement 4: Encoding Hint Validation

**User Story:** As a developer, I want encoding hints to be validated at write time, so that I receive clear error messages for invalid configurations.

#### Acceptance Criteria

1. WHEN the metadata contains an "IMODE" field with an invalid value (not B, P, R, or S), THE JBPDatasetWriter SHALL return an error with a descriptive message
2. WHEN the metadata contains an "IC" field with a compression code requiring an unavailable codec, THE JBPDatasetWriter SHALL return an error with a descriptive message
3. WHEN the metadata contains an "NPPBH" field with a value less than 1 or greater than 8192, THE JBPDatasetWriter SHALL return an error with a descriptive message
4. WHEN the metadata contains an "NPPBV" field with a value less than 1 or greater than 8192, THE JBPDatasetWriter SHALL return an error with a descriptive message
5. WHEN the metadata contains block size values larger than image dimensions, THE JBPDatasetWriter SHALL auto-adjust the block size to match image dimensions and log a warning

### Requirement 5: Conflict Resolution

**User Story:** As a developer, I want clear rules for resolving conflicts between provider properties and metadata hints, so that the output is predictable.

#### Acceptance Criteria

1. WHEN provider properties (num_bands, pixel_type, num_rows, num_columns) conflict with metadata hints, THE JBPDatasetWriter SHALL use the provider property values for structural values
2. WHEN metadata hints specify encoding choices (IMODE, IC, NPPBH, NPPBV, COMRAT), THE JBPDatasetWriter SHALL use the metadata hint values
3. WHEN the provider has a different band count than implied by metadata IREP field, THE JBPDatasetWriter SHALL use the provider's band count and log a warning about the IREP mismatch

### Requirement 6: Python API Consistency

**User Story:** As a Python developer, I want to use the same field names for encoding hints that I see when reading files, so that I don't need to learn a separate naming convention.

#### Acceptance Criteria

1. THE encoding hint field names SHALL match the exact field names returned by the reader's metadata (e.g., "IMODE", "IC", "NPPBH", "NPPBV", "COMRAT")
2. THE SimpleMetadataProvider Python bindings SHALL accept string keys and string values
3. WHEN copying metadata from a reader to a writer, THE field names SHALL be identical without requiring translation
