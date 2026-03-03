# Requirements Document

## Introduction

This document defines the requirements for refactoring the JBP image writing code to use a symmetric block-based I/O architecture. Currently, the `BlockDecoder` trait enables block-based reading, but writing is handled inline in `JBPDatasetWriter`. This refactoring introduces a `BlockEncoder` trait to make reading and writing symmetric, enabling efficient transcoding of large images with different tile sizes.

## Glossary

- **BlockDecoder**: Existing trait for decoding image blocks from various compression formats
- **BlockEncoder**: New trait for encoding image blocks to various compression formats
- **ImageAssetProvider**: Trait providing block-based access to image data
- **Tile_Size**: The dimensions (width × height) of image blocks/tiles
- **Transcoding**: Converting an image from one format/configuration to another
- **IMODE**: Image interleave mode (B, P, R, S) determining how bands are organized
- **JBPDatasetWriter**: The dataset writer for creating NITF files

## Requirements

### Requirement 1: BlockEncoder Trait Definition

**User Story:** As a developer, I want a BlockEncoder trait symmetric to BlockDecoder, so that image encoding follows the same pattern as decoding.

#### Acceptance Criteria

1. THE system SHALL define a BlockEncoder trait with encode_block() method
2. THE BlockEncoder trait SHALL accept block coordinates (row, col) and pixel data
3. THE BlockEncoder trait SHALL have a finalize() method returning the encoded data
4. THE BlockEncoder trait SHALL have a compression_type() method returning the IC code
5. THE BlockEncoder trait SHALL have a block_grid_size() method returning (rows, cols)
6. THE BlockEncoder trait SHALL be Send + Sync for thread safety


### Requirement 2: UncompressedBlockEncoder Implementation

**User Story:** As a developer, I want an UncompressedBlockEncoder that implements BlockEncoder, so that uncompressed NITF writing uses the new architecture.

#### Acceptance Criteria

1. THE UncompressedBlockEncoder SHALL implement the BlockEncoder trait
2. THE UncompressedBlockEncoder SHALL support all IMODE values (B, P, R, S)
3. THE UncompressedBlockEncoder SHALL accept blocks in band-sequential format
4. THE UncompressedBlockEncoder SHALL convert to the target IMODE during encoding
5. THE UncompressedBlockEncoder SHALL handle edge blocks (partial blocks at boundaries)
6. THE UncompressedBlockEncoder SHALL return IC code "NC" from compression_type()

### Requirement 3: Block Encoder Factory

**User Story:** As a developer, I want a factory function to create BlockEncoders, so that the correct encoder is selected based on IC code.

#### Acceptance Criteria

1. THE system SHALL provide a create_block_encoder() factory function
2. THE factory SHALL accept IC code, dimensions, bands, bit depth, and encoding hints
3. WHEN IC is "NC", THE factory SHALL return an UncompressedBlockEncoder
4. WHEN IC is unsupported, THE factory SHALL return an UnsupportedCompression error

### Requirement 4: JBPDatasetWriter Refactoring

**User Story:** As a developer, I want JBPDatasetWriter to use BlockEncoder, so that writing follows the same pattern as reading.

#### Acceptance Criteria

1. THE JBPDatasetWriter SHALL use BlockEncoder for image data encoding
2. THE JBPDatasetWriter SHALL read blocks from the source ImageAssetProvider
3. THE JBPDatasetWriter SHALL pass blocks to the BlockEncoder for encoding
4. THE JBPDatasetWriter SHALL call finalize() to get the encoded image data
5. THE existing public API SHALL remain unchanged

### Requirement 5: Tile Size Conversion

**User Story:** As a developer, I want to write images with a different tile size than the source, so that I can optimize output for different use cases.

#### Acceptance Criteria

1. THE BlockEncoder SHALL support output tile sizes different from input tile sizes
2. WHEN output tile size differs from input, THE system SHALL read necessary input tiles
3. THE system SHALL assemble input tiles into output tiles correctly
4. THE system SHALL handle edge cases where tiles don't align evenly
5. FOR ALL tile size combinations, pixel values SHALL be preserved exactly

### Requirement 6: IMODE Conversion During Encoding

**User Story:** As a developer, I want to write images with a different IMODE than the source, so that I can change the interleave format.

#### Acceptance Criteria

1. THE BlockEncoder SHALL accept input in band-sequential format
2. THE BlockEncoder SHALL convert to the target IMODE during encoding
3. THE system SHALL support conversion to all IMODE values (B, P, R, S)
4. FOR ALL IMODE conversions, pixel values SHALL be preserved exactly

### Requirement 7: Round-Trip Consistency

**User Story:** As a developer, I want encoding and decoding to be symmetric, so that I can trust data integrity through transcoding.

#### Acceptance Criteria

1. FOR ALL valid image data, encoding then decoding SHALL produce equivalent pixel data
2. FOR ALL tile size combinations, round-trip SHALL preserve pixel values exactly
3. FOR ALL IMODE combinations, round-trip SHALL preserve pixel values exactly
4. THE decoded dimensions SHALL match the original dimensions

### Requirement 8: Error Handling

**User Story:** As a developer, I want descriptive error messages for encoding operations, so that I can diagnose issues effectively.

#### Acceptance Criteria

1. WHEN block coordinates are invalid, THE error SHALL include the coordinates and grid size
2. WHEN block data size is incorrect, THE error SHALL include expected and actual sizes
3. WHEN IMODE conversion fails, THE error SHALL include source and target IMODE
4. WHEN finalize() is called before all blocks, THE error SHALL indicate missing blocks
