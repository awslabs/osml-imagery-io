# Implementation Plan: TIFF Writing (Phase 2)

## Overview

Implement `TIFFDatasetWriter` following the same architectural pattern as the JBP writer: in-memory assembly via `TIFFClientOpen` with write callbacks, tile-by-tile encoding, and flush-to-disk on `close()`. The implementation proceeds bottom-up: FFI write extensions → core writer → pixel layout conversion → encoding hints → Python bindings → property tests.

## Tasks

- [x] 1. Extend libtiff FFI layer with write-mode support
  - [x] 1.1 Add `TIFFWriteDirectory` declaration to `src/tiff/sys.rs`
    - Add `pub fn TIFFWriteDirectory(tif: *mut c_void) -> c_int;` to the extern block
    - _Requirements: 1.1, 2.6_

  - [x] 1.2 Add `PREDICTOR` tag constant to `src/tiff/tags.rs`
    - Add `pub const PREDICTOR: u32 = 317;` for compression pre-filtering
    - _Requirements: 4.9, 4.10, 4.11, 4.12_

  - [x] 1.3 Implement `MemoryWriteStreamData` and write callbacks in `src/tiff/ffi.rs`
    - Create `MemoryWriteStreamData` struct with `Vec<u8>` buffer and `pos: usize`
    - Implement `tiff_write_proc_writable()` callback that writes into the growable buffer
    - Implement `tiff_seek_proc_writable()` callback supporting SEEK_SET/CUR/END
    - Implement `tiff_size_proc_writable()` callback returning buffer length
    - _Requirements: 1.1_

  - [x] 1.4 Add `TiffHandle::from_write()` constructor and write methods to `src/tiff/ffi.rs`
    - Implement `from_write()` that opens `TIFFClientOpen` in `"w"` mode with write callbacks
    - Implement `write_encoded_tile(tile_index, data)` wrapping `TIFFWriteEncodedTile()`
    - Implement `set_field_u16(tag, value)` wrapping `TIFFSetField()` for u16
    - Implement `set_field_u32(tag, value)` wrapping `TIFFSetField()` for u32
    - Implement `write_directory()` wrapping `TIFFWriteDirectory()` for multi-IFD
    - Implement `into_bytes()` that calls `TIFFClose` and returns the `Vec<u8>` buffer
    - _Requirements: 1.1, 1.4, 2.6, 5.1_

  - [ ]* 1.5 Write unit tests for FFI write extensions in `src/tiff/ffi.rs`
    - Test `MemoryWriteStreamData` basic write and seek operations
    - Test `TiffHandle::from_write()` creates a valid handle
    - Test `write_encoded_tile()`, `set_field_u16()`, `set_field_u32()`, `write_directory()`
    - Test `into_bytes()` returns valid TIFF bytes
    - _Requirements: 1.1_

- [x] 2. Implement core `TIFFDatasetWriter` struct and `DatasetWriter` trait
  - [x] 2.1 Create `src/tiff/writer.rs` with `TIFFDatasetWriter` struct and asset queuing
    - Define `TIFFDatasetWriter` struct with `path`, `assets`, `asset_keys`, `metadata`, `closed` fields
    - Define `QueuedImageAsset` struct for queued assets
    - Implement `new(path)` constructor
    - Implement `add_asset()` with validation: reject non-Image types (`CodecError::Unsupported`), reject duplicate keys (`CodecError::DuplicateKey`), reject after close (`CodecError::Io`)
    - Implement `set_metadata()` storing the latest `MetadataProvider`
    - _Requirements: 2.1, 2.3, 2.4, 2.5, 10.1, 10.2_

  - [x] 2.2 Implement encoding hints parsing in `src/tiff/writer.rs`
    - Create `TiffEncodingHints` struct with `tile_width`, `tile_height`, `compression`, `predictor`, `planar_config`
    - Parse `"TileWidth"`, `"TileHeight"`, `"Compression"`, `"Predictor"`, `"PlanarConfiguration"` from `MetadataProvider`
    - Apply defaults: 256×256 tiles, Deflate compression, Horizontal predictor for LZW/Deflate, None for uncompressed
    - Return `CodecError::InvalidFormat` for unparseable values
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10, 4.11, 4.12, 10.3_

  - [x] 2.3 Implement `bsq_to_interleaved()` pixel layout conversion in `src/tiff/writer.rs`
    - Convert CHW (band-sequential) data to HWC (chunky/interleaved) format
    - Handle all bytes-per-sample sizes (1, 2, 4, 8 bytes)
    - For planar config, skip conversion and write bands as separate tile planes
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [x] 2.4 Implement edge tile padding in `src/tiff/writer.rs`
    - When image dimensions are not divisible by tile dimensions, pad edge tiles
    - Use `ImageAssetProvider.pad_pixel_value()` for fill bytes
    - Allocate full-tile buffer, copy actual data, write padded buffer
    - _Requirements: 5.4_

  - [x] 2.5 Implement `close()` with TIFF tag setting, tile writing, and file flush
    - Parse encoding hints from metadata (or use defaults)
    - Open `TiffHandle::from_write()` for in-memory assembly
    - For each queued image asset:
      - Set TIFF tags from `ImageAssetProvider` properties: ImageWidth, ImageLength, BitsPerSample, SamplesPerPixel, SampleFormat, PhotometricInterpretation, TileWidth, TileLength, Compression, Predictor, PlanarConfiguration
      - Iterate block grid, call `get_block()`, convert layout, pad edge tiles, write via `write_encoded_tile()`
      - Call `write_directory()` for multi-IFD support
    - Call `into_bytes()` to get assembled TIFF, write to disk via `std::fs::write()`
    - Make `close()` idempotent: second call returns `Ok(())`
    - Return `CodecError::Io` if output path is not writable
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.2, 2.6, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 5.1, 5.2, 5.3, 7.1, 7.2, 7.3_

  - [x] 2.6 Register the writer module in `src/tiff/mod.rs`
    - Add `mod writer;` and `pub use writer::TIFFDatasetWriter;`
    - _Requirements: 1.1_

  - [x] 2.7 Write unit tests for `TIFFDatasetWriter` in `src/tiff/writer.rs`
    - Test `new()` creates instance
    - Test `add_asset()` accepts image assets, rejects non-image, rejects duplicates, rejects after close
    - Test `close()` is idempotent
    - Test `set_metadata()` stores latest
    - Test encoding hint parsing: default values, each compression type, each predictor setting, predictor defaults with/without compression
    - Test `bsq_to_interleaved()` for 3-band and single-band
    - Test pixel type to SampleFormat mapping
    - Test PhotometricInterpretation selection (RGB vs MinIsBlack)
    - _Requirements: 2.1, 2.3, 2.4, 2.5, 1.3, 10.2, 4.2, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10, 4.11, 4.12, 6.1, 3.5, 3.6, 7.2_

- [x] 3. Checkpoint - Ensure Rust tests pass
  - Ensure all tests pass with `cargo test`, ask the user if questions arise.

- [x] 4. Wire up Python bindings
  - [x] 4.1 Update `src/bindings/io.rs` to create `TIFFDatasetWriter` for TIFF format strings
    - Replace the "Phase 2 scope" error branch for `"tif" | "tiff" | "gtif" | "gtiff" | "geotiff"` with `TIFFDatasetWriter::new()` construction
    - Update the existing `test_create_writer_tiff_unsupported` and `test_create_writer_tif_format_unsupported` tests to assert success instead of error
    - _Requirements: 8.1, 8.2, 8.3, 8.4_

  - [ ]* 4.2 Write Python unit tests in `tests/test_tiff_writer.py`
    - Test `IO.open(["out.tif"], "w", "tiff")` creates a writer
    - Test `IO.open(["out.tif"], "w", "tif")` creates a writer
    - Test context manager produces a valid TIFF file after writing an image
    - Test writing to an unwritable path raises IOError
    - Test `add_asset()` after `close()` raises error
    - Test `set_metadata()` multiple times uses latest
    - Test default encoding hints are applied when no metadata set
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 1.5, 2.5, 10.2, 10.3_

- [x] 5. Checkpoint - Ensure Python and Rust tests pass
  - Ensure all tests pass with `cargo test` and `pytest`, ask the user if questions arise.

- [x] 6. Add hypothesis strategies and property-based tests
  - [x] 6.1 Add TIFF writing strategies to `tests/property/strategies.py`
    - Add `tiff_encoding_hints()` strategy generating `BufferedMetadataProvider` with random TileWidth, TileHeight, Compression, Predictor, PlanarConfiguration
    - Add `tiff_writable_image()` composite strategy generating a `BufferedImageAssetProvider` with random pixel type (full set: UInt8–Float64), dimensions, bands, and populated blocks, plus a matching `BufferedMetadataProvider` with encoding hints
    - Build on existing `pixel_types()`, `image_dimensions()`, `band_counts()`, `block_sizes()`, `tiff_compression()` strategies
    - _Requirements: 9.1, 9.2, 9.3_

  - [x] 6.2 Extract shared `read_full_image()` helper to `tests/property/helpers.py`
    - Move the `_read_full_image()` function (duplicated across `test_image_roundtrip.py`, `test_tiff_roundtrip.py`, and others) into a shared `tests/property/helpers.py` module
    - The helper reads all blocks from an asset and reassembles into a CHW numpy array
    - Update existing test files to import from `helpers.py` instead of defining their own copies
    - _No new requirements — refactor for reuse_

  - [x] 6.3 Write TIFF-specific lossless pixel roundtrip property test in `tests/property/test_tiff_write_roundtrip.py`
    - **Property: TIFF Lossless Pixel Roundtrip**
    - Use `tiff_writable_image()` strategy from 6.1 to generate image + encoding hints
    - Write via `IO.open([path], "w", "tiff")`, read back via `IO.open([path], "r")`, assert pixel data identical
    - Test across all pixel types, band counts, compressions (None/LZW/Deflate), and planar configurations (Chunky/Planar)
    - Uses shared `read_full_image()` from 6.2
    - **Validates: Requirements 1.2, 2.1, 5.1, 5.4, 6.1, 6.2, 7.1, 7.3, 9.1, 9.2**

  - [x] 6.4 Write TIFF-specific metadata roundtrip property test in `tests/property/test_tiff_write_roundtrip.py`
    - **Property: TIFF Metadata Roundtrip**
    - Write TIFF with encoding hints, read back, assert TIFF tag values (Compression, TileWidth, TileLength, Predictor, PlanarConfiguration, SampleFormat, PhotometricInterpretation) match hints and image properties
    - This is TIFF-specific because TIFF tags differ entirely from NITF subheader fields
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 4.1, 4.3, 4.5, 4.6, 4.7, 4.9, 4.10, 4.11, 6.4, 7.2, 9.3**

  - [x] 6.5 Write TIFF-specific non-image asset rejection property test in `tests/property/test_tiff_write_roundtrip.py`
    - **Property: TIFF Non-Image Asset Rejection**
    - Call `add_asset()` with `AssetProvider.from_bytes()` using `AssetType.Text` / `AssetType.Data`, assert error raised
    - This is TIFF-specific because NITF accepts text/graphic/DES segments while TIFF only supports images
    - **Validates: Requirements 2.3**

  - [x] 6.6 Write format-parameterized idempotent close property test in `tests/property/test_writer_contracts.py`
    - **Property: Idempotent Close (NITF + TIFF)**
    - Parameterize over format configs: `[(".ntf", "nitf", {"IC": "NC"}), (".tif", "tiff", {})]`
    - Write an image, close writer, read file bytes, close again, read file bytes, assert identical
    - This property applies to both formats but has no existing property-based test for either — JBP only has Rust unit tests for idempotent close
    - Uses `random_image()` strategy and shared `read_full_image()` from 6.2
    - **Validates: Requirements 1.3**

  - [x] 6.7 Write format-parameterized duplicate key rejection property test in `tests/property/test_writer_contracts.py`
    - **Property: Duplicate Key Rejection (NITF + TIFF)**
    - Parameterize over format configs: `[(".ntf", "nitf"), (".tif", "tiff")]`
    - Generate random key strings, call `add_asset()` twice with same key, assert first succeeds, second raises error
    - JBP has a unit test (`test_add_duplicate_key_raises`) but no property-based version; this strengthens both formats
    - **Validates: Requirements 2.4**

  - [x] 6.8 Write format-parameterized multi-image ordering property test in `tests/property/test_writer_contracts.py`
    - **Property: Multi-Image Ordering (NITF + TIFF)**
    - Parameterize over format configs: `[(".ntf", "nitf", {"IC": "NC"}), (".tif", "tiff", {})]`
    - Add N (2–4) random image assets with distinct keys, write, read back, assert N assets returned with matching pixel data in insertion order
    - JBP has unit tests (`test_round_trip_preserves_order`) but no property-based version; this strengthens both formats
    - Uses shared `read_full_image()` from 6.2
    - **Validates: Requirements 2.2, 2.6**

  - [x] 6.9 Write TIFF-specific idempotent encoding property test in `tests/property/test_tiff_write_roundtrip.py`
    - **Property: TIFF Idempotent Encoding**
    - Write → read → write → read, assert second read matches first read
    - Reuses the same pattern as `TestIdempotentEncoding` in `test_image_roundtrip.py` but with TIFF format and encoding hints
    - Uses shared `read_full_image()` from 6.2
    - **Validates: Requirements 9.4**

  - [x] 6.10 Write Rust property tests for `bsq_to_interleaved` in `src/tiff/writer.rs`
    - `prop_bsq_to_interleaved_roundtrip`: CHW → HWC → CHW produces original data
    - `prop_bsq_to_interleaved_preserves_length`: output has same byte length as input
    - This is TIFF-internal and has no JBP equivalent
    - _Requirements: 6.1_

- [x] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass with `cargo test` and `pytest -m property`, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples and edge cases
- The implementation follows the same pattern as the JBP writer for consistency
- **Cross-format tests (6.6, 6.7, 6.8)** live in `test_writer_contracts.py` and are parameterized over NITF and TIFF, improving JBP test coverage for free
- **TIFF-specific tests (6.3, 6.4, 6.5, 6.9, 6.10)** live in `test_tiff_write_roundtrip.py` because they test TIFF-only behavior (encoding hints, TIFF tags, non-image rejection, planar config)
- **Shared helper (6.2)** eliminates the `_read_full_image()` duplication across 4+ test files
