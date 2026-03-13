# Requirements Document

## Introduction

This feature adds TIFF format reading support to osml-imagery-io through custom FFI bindings to libtiff (BSD-licensed). The implementation follows the same patterns established by the JBP/NITF format: libtiff handles compression and tile I/O through custom FFI bindings, and the results are exposed through the existing Dataset API (`DatasetReader`, `ImageAssetProvider`, `MetadataProvider`). Users read TIFF pixels and metadata using the same interface they use for NITF — the only difference is the metadata dictionary contents.

The core design principle is that the format implementation operates on `&[u8]`, not files. All libtiff access goes through `TIFFClientOpen` with memory read/seek callbacks over a byte slice, identical to the OpenJPEG memory stream pattern in `src/jbp/j2k/ffi.rs`. libtiff never opens a file descriptor.

## Glossary

- **IO_Factory**: The `IO` Python class in `src/bindings/io.rs` that selects reader/writer implementations based on URI scheme and file extension
- **DatasetReader**: The trait defined in `src/traits/reader.rs` providing asset-based access to geospatial datasets
- **ImageAssetProvider**: The trait defined in `src/traits/image.rs` providing blocked/tiled access to image pixel data in band-sequential (CHW) format
- **MetadataProvider**: The trait defined in `src/traits/metadata.rs` providing access to metadata as key-value dictionaries
- **TIFFDatasetReader**: The TIFF implementation of `DatasetReader`, analogous to `JBPDatasetReader`
- **TIFFImageAssetProvider**: The TIFF implementation of `ImageAssetProvider`, providing tile-based pixel access
- **TIFFMetadataProvider**: The TIFF implementation of `MetadataProvider`, mapping TIFF tags to key-value pairs
- **TiffHandle**: The safe RAII wrapper around a raw libtiff `TIFF*` pointer, with `Drop` calling `TIFFClose`
- **MemoryReadStreamData**: The struct holding a pointer, length, and position for memory-based read/seek callbacks passed to `TIFFClientOpen`
- **IFD**: Image File Directory — the TIFF structure containing tags that describe an image. A TIFF file can contain multiple IFDs, each with its own independent set of tags. IFD 0 is the first (primary) image.
- **NewSubfileType**: TIFF tag (254) with bit flags classifying an IFD: bit 0 = reduced-resolution overview, bit 1 = multi-page image, bit 2 = transparency mask. Default is 0 (full-resolution image).
- **Chunky_Layout**: Pixel interleaving where samples are stored as RGBRGB... (PlanarConfiguration=1)
- **Planar_Layout**: Band-separated storage where samples are stored as RRR...GGG...BBB... (PlanarConfiguration=2)
- **Band_Sequential**: The output format (CHW) used by `ImageAssetProvider.get_block()`, where each band's pixels are contiguous
- **Stripped_TIFF**: A TIFF organized into horizontal strips rather than rectangular tiles
- **Tiled_TIFF**: A TIFF organized into rectangular tiles with TileWidth and TileLength tags
- **SampleFormat**: TIFF tag indicating the data type of pixel values (unsigned int, signed int, floating point)
- **libtiff**: The BSD-licensed C library for reading and writing TIFF files, linked dynamically

## Requirements

### Requirement 1: Raw FFI Declarations

**User Story:** As a developer, I want raw FFI declarations for libtiff functions, so that the safe wrapper layer can call libtiff without any third-party `-sys` crates.

#### Acceptance Criteria

1. THE sys_module SHALL declare `extern "C"` bindings for `TIFFClientOpen`, `TIFFClose`, `TIFFGetField`, `TIFFSetField`, `TIFFReadEncodedTile`, `TIFFWriteEncodedTile`, `TIFFTileSize`, `TIFFNumberOfTiles`, `TIFFSetDirectory`, `TIFFCurrentDirectory`, `TIFFNumberOfDirectories`, `TIFFReadTile`, and `TIFFWriteTile`
2. THE sys_module SHALL declare the `TIFFErrorHandler` and `TIFFSetErrorHandler` / `TIFFSetWarningHandler` function pointer types for error callback registration
3. THE sys_module SHALL declare the `TIFFClientOpen` callback function pointer types for read, write, seek, close, and size operations
4. THE sys_module SHALL use `*mut c_void` as the opaque TIFF handle type, consistent with libtiff's `TIFF*`
5. THE sys_module SHALL declare `TIFFIsTiled` and `TIFFStripSize`, `TIFFNumberOfStrips`, `TIFFReadEncodedStrip` for strip-based TIFF support

### Requirement 2: Safe RAII Wrapper

**User Story:** As a developer, I want a safe Rust wrapper around libtiff, so that resource cleanup is automatic and error handling is idiomatic.

#### Acceptance Criteria

1. THE TiffHandle SHALL wrap a raw libtiff `TIFF*` pointer and call `TIFFClose` in its `Drop` implementation
2. THE TiffHandle SHALL be constructable only through `TIFFClientOpen` with memory read/seek/close/size callbacks operating on a `&[u8]` byte slice
3. THE TiffHandle SHALL provide typed tag getter methods that return `Result` types for `u16`, `u32`, `f32`, `f64`, and string TIFF tag values
4. THE TiffHandle SHALL provide `read_encoded_tile(tile_index) -> Result<Vec<u8>>` and `read_encoded_strip(strip_index) -> Result<Vec<u8>>` methods
5. THE TiffHandle SHALL capture libtiff error and warning messages using a thread-local callback pattern, consistent with the OpenJPEG error handling in `src/jbp/j2k/ffi.rs`
6. THE TiffHandle SHALL implement `Send` to allow transfer between threads
7. THE MemoryReadStreamData SHALL hold a pointer to the byte slice data, the total length, and the current read position
8. WHEN `TIFFClientOpen` returns a null pointer, THE TiffHandle constructor SHALL return `CodecError::InvalidFormat` with a descriptive message including any captured libtiff error
9. THE TiffHandle SHALL provide `set_directory(index) -> Result<()>`, `current_directory() -> u16`, and `number_of_directories() -> u16` methods for IFD navigation

### Requirement 3: TIFF Tag Constants

**User Story:** As a developer, I want named constants for standard TIFF tags, so that tag access is readable and less error-prone than using raw numeric IDs.

#### Acceptance Criteria

1. THE tags_module SHALL define named constants for standard TIFF tags: `NEW_SUBFILE_TYPE`, `IMAGE_WIDTH`, `IMAGE_LENGTH`, `BITS_PER_SAMPLE`, `SAMPLES_PER_PIXEL`, `COMPRESSION`, `PHOTOMETRIC_INTERPRETATION`, `TILE_WIDTH`, `TILE_LENGTH`, `SAMPLE_FORMAT`, `PLANAR_CONFIGURATION`, `ROWS_PER_STRIP`, `STRIP_OFFSETS`, `STRIP_BYTE_COUNTS`, `TILE_OFFSETS`, and `TILE_BYTE_COUNTS`
2. THE tags_module SHALL define compression type constants: `COMPRESSION_NONE` (1), `COMPRESSION_LZW` (5), `COMPRESSION_DEFLATE` (8), `COMPRESSION_PACKBITS` (32773), and `COMPRESSION_ADOBE_DEFLATE` (32946)
3. THE tags_module SHALL define sample format constants: `SAMPLE_FORMAT_UINT` (1), `SAMPLE_FORMAT_INT` (2), `SAMPLE_FORMAT_FLOAT` (3)
4. THE tags_module SHALL define photometric interpretation constants: `PHOTOMETRIC_MINISBLACK` (1), `PHOTOMETRIC_RGB` (2), `PHOTOMETRIC_PALETTE` (3)
5. THE tags_module SHALL define planar configuration constants: `PLANAR_CONFIG_CONTIG` (1) for chunky layout and `PLANAR_CONFIG_SEPARATE` (2) for planar layout

### Requirement 4: Image Asset Provider

**User Story:** As a developer, I want a TIFF image asset provider that implements the `ImageAssetProvider` trait, so that TIFF pixel data is accessible through the same blocked/tiled interface used for NITF.

#### Acceptance Criteria

1. THE TIFFImageAssetProvider SHALL implement the `ImageAssetProvider` trait defined in `src/traits/image.rs`
2. WHEN `get_block(row, col, level, bands)` is called on a Tiled_TIFF, THE TIFFImageAssetProvider SHALL read the tile using `TIFFReadEncodedTile` and return pixel data in Band_Sequential (CHW) format
3. WHEN `get_block(row, col, level, bands)` is called on a Stripped_TIFF, THE TIFFImageAssetProvider SHALL treat strips as full-width blocks stacked vertically (each strip spans the full ImageWidth and is RowsPerStrip rows tall) and read using `TIFFReadEncodedStrip`
4. WHEN the source TIFF uses Chunky_Layout (PlanarConfiguration=1), THE TIFFImageAssetProvider SHALL deinterleave the pixel data to Band_Sequential format before returning
5. WHEN the source TIFF uses Planar_Layout (PlanarConfiguration=2), THE TIFFImageAssetProvider SHALL assemble per-band tile reads into Band_Sequential format before returning
6. THE TIFFImageAssetProvider `has_block()` method SHALL return `true` for all valid block coordinates, because TIFF tiles are not sparse
7. THE TIFFImageAssetProvider `num_resolution_levels()` SHALL return 1 for basic TIFF files without overviews
8. THE TIFFImageAssetProvider SHALL support pixel types: uint8, uint16, uint32, int8, int16, int32, float32, float64, mapping TIFF SampleFormat and BitsPerSample to the `PixelType` enum
9. THE TIFFImageAssetProvider SHALL support images with 1 to N bands (SamplesPerPixel)
10. WHEN `get_block()` is called with `bands` set to a subset of available bands, THE TIFFImageAssetProvider SHALL return only the requested bands in Band_Sequential format
11. WHEN `get_block()` is called with block coordinates outside the valid grid, THE TIFFImageAssetProvider SHALL return `CodecError::InvalidBlockCoordinates`
12. WHEN `get_block()` is called with `resolution_level` greater than 0, THE TIFFImageAssetProvider SHALL return `CodecError::InvalidResolutionLevel` because basic TIFF has only one resolution level
13. THE TIFFImageAssetProvider SHALL expose per-IFD TIFF tags through its `metadata()` method, returning a `MetadataProvider` containing the tags from the specific IFD that this image segment represents

### Requirement 5: Dataset Reader

**User Story:** As a developer, I want a TIFF dataset reader that implements the `DatasetReader` trait, so that TIFF files are opened and accessed through the same interface used for NITF.

#### Acceptance Criteria

1. THE TIFFDatasetReader SHALL implement the `DatasetReader` trait defined in `src/traits/reader.rs`
2. THE TIFFDatasetReader SHALL provide a `from_bytes(data: &[u8])` constructor that opens the TIFF via `TIFFClientOpen` with memory callbacks on the byte slice
3. THE TIFFDatasetReader SHALL provide an `open(path)` convenience method that reads the file into `Vec<u8>` then delegates to `from_bytes()`
4. THE TIFFDatasetReader SHALL enumerate all IFDs in the file and classify each by its `NewSubfileType` tag: IFDs where bit 0 is 0 (or `NewSubfileType` is absent) are full-resolution images; IFDs where bit 0 is 1 are reduced-resolution overviews (skipped in Phase 1)
5. THE TIFFDatasetReader SHALL expose one `ImageAssetProvider` per full-resolution IFD, keyed as `"image_segment_0"`, `"image_segment_1"`, etc., in IFD order
6. THE TIFFDatasetReader SHALL expose a dataset-level `MetadataProvider` containing only file-level information that is not specific to any single IFD (e.g., byte order, number of directories, number of image segments)
7. WHEN `get_asset("image_segment_N")` is called for a valid index N, THE TIFFDatasetReader SHALL return the TIFFImageAssetProvider for the corresponding full-resolution IFD
8. WHEN `get_asset()` is called with a key that does not match any image segment, THE TIFFDatasetReader SHALL return `CodecError::AssetNotFound`
9. WHEN `get_asset_keys(Some(AssetType::Image), None)` is called, THE TIFFDatasetReader SHALL return the keys for all full-resolution image segments (e.g., `["image_segment_0"]` for a single-image TIFF, `["image_segment_0", "image_segment_1"]` for a two-image TIFF)
10. WHEN `get_asset_keys(Some(AssetType::Text), None)` is called, THE TIFFDatasetReader SHALL return an empty list because TIFF files have no text segments
11. WHEN `from_bytes()` receives data that does not begin with a valid TIFF magic number (`II*\0` for little-endian or `MM\0*` for big-endian), THE TIFFDatasetReader SHALL return `CodecError::InvalidFormat` with a descriptive message
12. WHEN `close()` is called, THE TIFFDatasetReader SHALL release all resources held by the TiffHandle
13. WHEN a TIFF file contains only a single IFD, THE TIFFDatasetReader SHALL expose it as `"image_segment_0"` regardless of its `NewSubfileType` value

### Requirement 6: Metadata Provider

**User Story:** As a developer, I want TIFF tags exposed as metadata dictionaries at both the dataset and per-image-segment level, so that I can inspect image properties through the same `MetadataProvider` interface used for NITF.

#### Acceptance Criteria

1. THE TIFFMetadataProvider SHALL implement the `MetadataProvider` trait defined in `src/traits/metadata.rs`
2. THE TIFFMetadataProvider SHALL map standard TIFF tags to string key-value pairs using libtiff naming conventions (e.g., `"ImageWidth"`, `"ImageLength"`, `"BitsPerSample"`, `"SamplesPerPixel"`, `"Compression"`, `"PhotometricInterpretation"`)
3. THE TIFFMetadataProvider SHALL represent numeric tag values as JSON number values and string tag values as JSON string values
4. THE TIFFMetadataProvider SHALL include the following tags when present: ImageWidth, ImageLength, BitsPerSample, SamplesPerPixel, Compression, PhotometricInterpretation, PlanarConfiguration, SampleFormat, TileWidth, TileLength, RowsPerStrip
5. WHEN `as_dict(None)` is called, THE TIFFMetadataProvider SHALL return all available TIFF tag metadata
6. WHEN `as_dict(Some("tiff"))` is called, THE TIFFMetadataProvider SHALL return the TIFF tag metadata
7. WHEN `as_dict(Some(name))` is called with an unrecognized section name, THE TIFFMetadataProvider SHALL return an empty dictionary
8. THE TIFFMetadataProvider SHALL be usable at the per-image-segment level (tags from the specific IFD, returned by `ImageAssetProvider.metadata()`), since each IFD in a TIFF file has its own independent set of tags
9. THE dataset-level MetadataProvider (returned by `DatasetReader.metadata()`) SHALL contain only file-level information not specific to any single IFD: byte order (`"ByteOrder"`: `"LittleEndian"` or `"BigEndian"`), number of directories (`"NumberOfDirectories"`), and number of image segments (`"NumberOfImageSegments"`)

### Requirement 7: Build System Integration

**User Story:** As a developer, I want libtiff discovered and linked automatically during the build, so that the TIFF feature works out of the box in conda and system environments.

#### Acceptance Criteria

1. THE build_script SHALL add a `configure_libtiff()` function that runs only when the `libtiff` feature is enabled
2. THE build_script SHALL search for libtiff using pkg-config first (`pkg-config --libs libtiff-4`), then conda (`$CONDA_PREFIX/lib/libtiff.{dylib,so}`), then common system paths
3. THE build_script SHALL emit `cargo:rustc-link-lib=tiff` to dynamically link libtiff
4. THE build_script SHALL follow the same discovery pattern as the existing `configure_openjpeg()` function
5. THE cargo_manifest SHALL define a `libtiff` feature flag that is enabled by default
6. IF libtiff cannot be found during the build, THEN THE build_script SHALL print a warning message with installation instructions for macOS, Ubuntu, and Fedora

### Requirement 8: Format Detection and IO Factory Registration

**User Story:** As a developer, I want TIFF files automatically detected by the IO factory, so that `IO.open(["image.tif"], "r")` works without specifying the format explicitly.

#### Acceptance Criteria

1. WHEN the IO_Factory receives a path with `.tif` or `.tiff` extension in read mode, THE IO_Factory SHALL create a TIFFDatasetReader
2. WHEN the IO_Factory receives an explicit format of `"tiff"` or `"tif"` in read mode, THE IO_Factory SHALL create a TIFFDatasetReader regardless of file extension
3. WHEN the IO_Factory receives a path with `.tif` or `.tiff` extension in write mode with format `"tiff"`, THE IO_Factory SHALL return `CodecError::Unsupported` with a message indicating that TIFF writing is not yet implemented (Phase 2 scope)
4. WHEN the IO_Factory receives a TIFF file, THE IO_Factory SHALL validate the magic bytes (`II*\0` for little-endian or `MM\0*` for big-endian) during reader construction

### Requirement 9: Pixel Format Conversion

**User Story:** As a developer, I want correct pixel data regardless of the TIFF's internal layout, so that downstream code always receives band-sequential data with the correct data type.

#### Acceptance Criteria

1. WHEN the source TIFF has SampleFormat=1 (unsigned integer) with BitsPerSample of 8, 16, or 32, THE TIFFImageAssetProvider SHALL map the pixel type to `PixelType::UInt8`, `PixelType::UInt16`, or `PixelType::UInt32` respectively
2. WHEN the source TIFF has SampleFormat=2 (signed integer) with BitsPerSample of 8, 16, or 32, THE TIFFImageAssetProvider SHALL map the pixel type to `PixelType::Int8`, `PixelType::Int16`, or `PixelType::Int32` respectively
3. WHEN the source TIFF has SampleFormat=3 (floating point) with BitsPerSample of 32 or 64, THE TIFFImageAssetProvider SHALL map the pixel type to `PixelType::Float32` or `PixelType::Float64` respectively
4. WHEN the source TIFF has an unsupported combination of SampleFormat and BitsPerSample, THE TIFFDatasetReader SHALL return `CodecError::Unsupported` with a message describing the unsupported pixel format
5. WHEN converting from Chunky_Layout to Band_Sequential, THE TIFFImageAssetProvider SHALL correctly deinterleave N-band pixel data for any value of SamplesPerPixel from 1 to N

### Requirement 10: Compression Support

**User Story:** As a developer, I want to read TIFF files with common lossless compressions, so that the reader handles real-world TIFF files without requiring the caller to decompress manually.

#### Acceptance Criteria

1. THE TIFFImageAssetProvider SHALL support reading tiles and strips compressed with: no compression (1), LZW (5), Deflate/ZLib (8 and 32946), and PackBits (32773)
2. WHEN a TIFF file uses a compression method not in the supported set, THE TIFFDatasetReader SHALL return `CodecError::Unsupported` with a message identifying the unsupported compression type and its numeric code
3. THE TIFFImageAssetProvider SHALL delegate all decompression to libtiff internally, performing no custom decompression logic

### Requirement 11: Stripped TIFF Handling

**User Story:** As a developer, I want stripped TIFFs to work through the same tiled interface, so that callers do not need to distinguish between tiled and stripped TIFF layouts.

#### Acceptance Criteria

1. WHEN the source TIFF is a Stripped_TIFF (no TileWidth/TileLength tags), THE TIFFImageAssetProvider SHALL report `num_pixels_per_block_horizontal()` as the image width and `num_pixels_per_block_vertical()` as the RowsPerStrip value
2. WHEN the source TIFF is a Stripped_TIFF, THE TIFFImageAssetProvider SHALL compute `block_grid_size()` as `(ceil(ImageLength / RowsPerStrip), 1)` — a vertical stack of full-width strips, where the first dimension is the number of strips and the second dimension is 1 because each strip spans the entire image width
3. WHEN `get_block(row, 0, 0, None)` is called on a Stripped_TIFF, THE TIFFImageAssetProvider SHALL read the corresponding strip using `TIFFReadEncodedStrip` and return the data in Band_Sequential format

### Requirement 12: Python Bindings

**User Story:** As a developer using Python, I want to open and read TIFF files through the same Python API used for NITF, so that format differences are transparent.

#### Acceptance Criteria

1. THE Python bindings SHALL expose `TIFFDatasetReader` through the existing `IO.open()` factory, requiring no new Python classes for basic reading
2. WHEN `IO.open(["image.tif"], "r")` is called from Python, THE IO_Factory SHALL return a `DatasetReader` backed by `TIFFDatasetReader`
3. THE Python bindings SHALL allow accessing TIFF image blocks as NumPy arrays through the existing `ImageAssetProvider.get_block()` binding
4. THE Python bindings SHALL allow accessing TIFF metadata through the existing `MetadataProvider.as_dict()` binding

### Requirement 13: Module Structure and Feature Gating

**User Story:** As a developer, I want the TIFF module conditionally compiled behind a feature flag, so that builds without libtiff still compile successfully.

#### Acceptance Criteria

1. THE tiff_module SHALL be gated behind `#[cfg(feature = "libtiff")]` in `src/lib.rs`
2. THE tiff_module SHALL contain submodules: `sys`, `ffi`, `tags`, `image`, `reader`, and `metadata`
3. WHEN the `libtiff` feature is disabled, THE crate SHALL compile without any libtiff-related code or link directives
4. WHEN the `libtiff` feature is enabled, THE IO_Factory SHALL include TIFF format detection and reader creation
