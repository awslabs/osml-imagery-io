# Requirements Document

## Introduction

This feature adds basic TIFF writing support to the osml-imagery-io library. The `TIFFDatasetWriter` implements the `DatasetWriter` trait to write tiled TIFF files from `BufferedImageAssetProvider` data, following the same architectural patterns established by the JBP writer. The writer produces bytes in memory via `TIFFClientOpen` with memory write callbacks and flushes to disk on `close()`. It supports configurable tile dimensions, compression (None/LZW/Deflate), and predictor settings via encoding hints in `BufferedMetadataProvider`. This is Phase 2 of the TIFF roadmap, building on the Phase 1 libtiff FFI bindings and basic TIFF reading already implemented.

## Glossary

- **TIFFDatasetWriter**: The Rust struct implementing `DatasetWriter` that writes tiled TIFF files using libtiff via FFI.
- **DatasetWriter**: The format-agnostic trait defining `add_asset()`, `set_metadata()`, and `close()` for writing geospatial datasets.
- **BufferedImageAssetProvider**: An in-memory `ImageAssetProvider` implementation used to supply pixel data and image properties to the writer.
- **BufferedMetadataProvider**: An in-memory `MetadataProvider` implementation used to supply encoding hints (tile size, compression, predictor) to the writer.
- **ImageAssetProvider**: The trait providing blocked/tiled image access with properties like dimensions, bands, pixel type, and block-based pixel retrieval.
- **TiffHandle**: The safe RAII wrapper around a libtiff `TIFF*` pointer, providing typed tag access and tile I/O methods.
- **Encoding_Hints**: Key-value pairs in `BufferedMetadataProvider` that control TIFF output parameters (compression, tile dimensions, predictor).
- **IO_Factory**: The `IO.open()` Python API that selects the appropriate reader or writer based on format string and file path.
- **PlanarConfiguration**: TIFF tag controlling whether pixel data is stored as chunky (interleaved RGBRGB) or planar (separate band planes RRR...GGG...BBB...).
- **Band_Sequential**: The internal pixel layout used by `ImageAssetProvider.get_block()`, where bands are stored as separate planes in CHW (channels, height, width) order.
- **Predictor**: A TIFF compression pre-filter that improves compression ratios by storing pixel differences instead of absolute values.

## Requirements

### Requirement 1: In-Memory TIFF Assembly via TIFFClientOpen

**User Story:** As a developer, I want the TIFF writer to assemble the TIFF file in memory using libtiff's `TIFFClientOpen` with custom write callbacks, so that the format implementation never touches the filesystem directly and follows the same I/O architecture as the JBP writer.

#### Acceptance Criteria

1. THE TIFFDatasetWriter SHALL open a libtiff handle via `TIFFClientOpen` in write mode ("w") with memory write/seek callbacks that operate on an internal `Vec<u8>` buffer.
2. WHEN `close()` is called, THE TIFFDatasetWriter SHALL flush the in-memory buffer to the output file path provided at construction.
3. WHEN `close()` is called on an already-closed writer, THE TIFFDatasetWriter SHALL return `Ok(())` without writing again (idempotent close).
4. WHEN `close()` is called, THE TIFFDatasetWriter SHALL call `TIFFClose` on the libtiff handle before flushing bytes to disk.
5. IF the output file path is not writable, THEN THE TIFFDatasetWriter SHALL return a `CodecError::Io` error on `close()`.

### Requirement 2: Multi-Image Asset Writing

**User Story:** As a developer, I want to write one or more images to a TIFF file through the `add_asset()` interface, so that I can create multi-image TIFF files using the same Dataset API used for NITF.

#### Acceptance Criteria

1. WHEN `add_asset()` is called with an `AssetProvider` whose `asset_type()` is `Image`, THE TIFFDatasetWriter SHALL accept the asset and queue it for writing as a separate IFD in the output TIFF file.
2. WHEN `add_asset()` is called multiple times with `asset_type()` `Image`, THE TIFFDatasetWriter SHALL accept each asset and queue each one for writing as a separate IFD, keyed as `"image_segment_0"`, `"image_segment_1"`, etc.
3. WHEN `add_asset()` is called with an `AssetProvider` whose `asset_type()` is `Text`, `Graphics`, or `Data`, THE TIFFDatasetWriter SHALL return a `CodecError::UnsupportedAsset` error with a message indicating that TIFF does not support non-image asset types.
4. WHEN `add_asset()` is called with a duplicate key, THE TIFFDatasetWriter SHALL return a `CodecError::DuplicateKey` error.
5. WHEN `add_asset()` is called after `close()`, THE TIFFDatasetWriter SHALL return a `CodecError::Io` error indicating the writer has been closed.
6. WHEN `close()` is called, THE TIFFDatasetWriter SHALL write each queued image asset as a separate IFD in the order the assets were added via `add_asset()`.

### Requirement 3: TIFF Tag Setting from Image Properties

**User Story:** As a developer, I want the writer to automatically set standard TIFF tags from the image properties of the provided `ImageAssetProvider`, so that the output TIFF file is self-describing and readable by any TIFF-compliant reader.

#### Acceptance Criteria

1. THE TIFFDatasetWriter SHALL set the `ImageWidth` tag (256) from `ImageAssetProvider.num_columns()`.
2. THE TIFFDatasetWriter SHALL set the `ImageLength` tag (257) from `ImageAssetProvider.num_rows()`.
3. THE TIFFDatasetWriter SHALL set the `BitsPerSample` tag (258) from `ImageAssetProvider.actual_bits_per_pixel()`.
4. THE TIFFDatasetWriter SHALL set the `SamplesPerPixel` tag (277) from `ImageAssetProvider.num_bands()`.
5. THE TIFFDatasetWriter SHALL set the `SampleFormat` tag (339) based on `ImageAssetProvider.pixel_value_type()`: `1` for unsigned integers, `2` for signed integers, `3` for floating point.
6. THE TIFFDatasetWriter SHALL set the `PhotometricInterpretation` tag (262) to `RGB` (2) when `SamplesPerPixel >= 3`, and to `MinIsBlack` (1) otherwise.
7. THE TIFFDatasetWriter SHALL set the `TileWidth` tag (322) and `TileLength` tag (323) from encoding hints or defaults.

### Requirement 4: Encoding Hint Parsing

**User Story:** As a developer, I want to control TIFF output parameters (tile size, compression, predictor) through encoding hints in `BufferedMetadataProvider`, so that I can tune the output format without changing code.

#### Acceptance Criteria

1. WHEN the dataset-level `MetadataProvider` contains a `"TileWidth"` key, THE TIFFDatasetWriter SHALL use the parsed integer value as the tile width in pixels.
2. WHEN the dataset-level `MetadataProvider` does not contain a `"TileWidth"` key, THE TIFFDatasetWriter SHALL use 256 as the default tile width.
3. WHEN the dataset-level `MetadataProvider` contains a `"TileHeight"` key, THE TIFFDatasetWriter SHALL use the parsed integer value as the tile height in pixels.
4. WHEN the dataset-level `MetadataProvider` does not contain a `"TileHeight"` key, THE TIFFDatasetWriter SHALL use 256 as the default tile height.
5. WHEN the dataset-level `MetadataProvider` contains a `"Compression"` key with value `"None"`, THE TIFFDatasetWriter SHALL set the Compression tag to 1 (no compression).
6. WHEN the dataset-level `MetadataProvider` contains a `"Compression"` key with value `"LZW"`, THE TIFFDatasetWriter SHALL set the Compression tag to 5.
7. WHEN the dataset-level `MetadataProvider` contains a `"Compression"` key with value `"Deflate"`, THE TIFFDatasetWriter SHALL set the Compression tag to 8.
8. WHEN the dataset-level `MetadataProvider` does not contain a `"Compression"` key, THE TIFFDatasetWriter SHALL default to Deflate compression (tag value 8).
9. WHEN the dataset-level `MetadataProvider` contains a `"Predictor"` key with value `"Horizontal"`, THE TIFFDatasetWriter SHALL set the Predictor tag to 2.
10. WHEN the dataset-level `MetadataProvider` contains a `"Predictor"` key with value `"None"`, THE TIFFDatasetWriter SHALL set the Predictor tag to 1 (no predictor).
11. WHEN the dataset-level `MetadataProvider` does not contain a `"Predictor"` key AND compression is LZW or Deflate, THE TIFFDatasetWriter SHALL default to Horizontal predictor (tag value 2).
12. WHEN the dataset-level `MetadataProvider` does not contain a `"Predictor"` key AND compression is None, THE TIFFDatasetWriter SHALL set the Predictor tag to 1 (no predictor).

### Requirement 5: Tile-by-Tile Image Data Writing

**User Story:** As a developer, I want the writer to read image data block-by-block from the `ImageAssetProvider` and write each tile via `TIFFWriteEncodedTile()`, so that large images can be written without loading the entire image into memory.

#### Acceptance Criteria

1. WHEN `close()` is called with a queued image asset, THE TIFFDatasetWriter SHALL iterate over all blocks in the `ImageAssetProvider` block grid and write each block as a TIFF tile via `TIFFWriteEncodedTile()`.
2. THE TIFFDatasetWriter SHALL compute the tile index for each block using the formula: `tile_index = block_row * tiles_across + block_col` (for chunky configuration) or the appropriate per-band tile index for planar configuration.
3. IF `TIFFWriteEncodedTile()` returns an error (negative value), THEN THE TIFFDatasetWriter SHALL return a `CodecError::Io` error with a message identifying the failed tile index.
4. WHEN the image dimensions are not evenly divisible by the tile dimensions, THE TIFFDatasetWriter SHALL pad edge tiles with the `ImageAssetProvider.pad_pixel_value()` to fill the full tile size.

### Requirement 6: Band-Sequential to TIFF Layout Conversion

**User Story:** As a developer, I want the writer to convert the band-sequential (CHW) pixel data from `ImageAssetProvider.get_block()` to the appropriate TIFF pixel layout, so that the output file conforms to the TIFF specification.

#### Acceptance Criteria

1. WHEN `PlanarConfiguration` is chunky (1), THE TIFFDatasetWriter SHALL convert band-sequential data (CHW: band0_all_pixels, band1_all_pixels, ...) to interleaved format (HWC: pixel0_all_bands, pixel1_all_bands, ...) before writing each tile.
2. WHEN `PlanarConfiguration` is planar/separate (2), THE TIFFDatasetWriter SHALL write each band as a separate tile plane, maintaining the band-sequential layout from `get_block()`.
3. THE TIFFDatasetWriter SHALL default to chunky `PlanarConfiguration` (1) when no explicit configuration is specified in encoding hints.
4. THE TIFFDatasetWriter SHALL set the `PlanarConfiguration` tag (284) in the TIFF output to match the layout used for writing.

### Requirement 7: Supported Pixel Types

**User Story:** As a developer, I want the writer to support all pixel types that the reader supports, so that any image readable by the library can also be written.

#### Acceptance Criteria

1. THE TIFFDatasetWriter SHALL support writing images with pixel types: UInt8, UInt16, UInt32, Int8, Int16, Int32, Float32, and Float64.
2. THE TIFFDatasetWriter SHALL set the `BitsPerSample` and `SampleFormat` tags correctly for each supported pixel type.
3. THE TIFFDatasetWriter SHALL support writing images with 1 to N bands (any `SamplesPerPixel` value).

### Requirement 8: Python Bindings for TIFFDatasetWriter

**User Story:** As a Python developer, I want to create TIFF files through the `IO.open()` API, so that I can write TIFF files using the same interface used for NITF.

#### Acceptance Criteria

1. WHEN `IO.open(paths, "w", "tiff")` is called, THE IO_Factory SHALL create a `TIFFDatasetWriter` for the specified path.
2. WHEN `IO.open(paths, "w", "tif")` is called, THE IO_Factory SHALL create a `TIFFDatasetWriter` for the specified path.
3. THE TIFFDatasetWriter SHALL be usable as a Python context manager (supporting `__enter__` and `__exit__`).
4. WHEN the context manager exits, THE TIFFDatasetWriter SHALL automatically call `close()`.

### Requirement 9: Write-Then-Read Roundtrip Correctness

**User Story:** As a developer, I want to verify that TIFF files written by the writer can be read back with identical pixel data, so that I can trust the writer produces valid TIFF files.

#### Acceptance Criteria

1. FOR ALL supported pixel types and band counts, writing an image via `TIFFDatasetWriter` and reading it back via `TIFFDatasetReader` SHALL produce pixel data identical to the original input (lossless roundtrip).
2. FOR ALL supported compression types (None, LZW, Deflate), the write-then-read roundtrip SHALL produce identical pixel data.
3. WHEN a TIFF file is written with specific encoding hints (tile size, compression, predictor), reading the file back SHALL report matching metadata values for those tags.
4. FOR ALL valid image dimensions and tile sizes, writing then parsing then writing again SHALL produce an equivalent TIFF file (idempotent encoding).

### Requirement 10: Dataset-Level Metadata

**User Story:** As a developer, I want to set dataset-level metadata on the writer, so that encoding hints and other metadata are available during the write process.

#### Acceptance Criteria

1. WHEN `set_metadata()` is called with a `MetadataProvider`, THE TIFFDatasetWriter SHALL store the metadata for use during `close()` when encoding hints are read.
2. WHEN `set_metadata()` is called multiple times, THE TIFFDatasetWriter SHALL use the most recently provided metadata.
3. WHEN `set_metadata()` is not called, THE TIFFDatasetWriter SHALL use default encoding hints (256×256 tiles, Deflate compression, Horizontal predictor).
