# TIFF / GeoTIFF Implementation Roadmap

This roadmap describes the plan for adding GeoTIFF support to osml-imagery-io. Cloud Optimized GeoTIFF (COG) support is covered in a separate roadmap ({doc}`cog`) since it depends on both this work and the cross-format image pyramid support ({doc}`image-pyramid`). The implementation follows the same patterns established by the JBP format: a third-party C library (libtiff) handles the heavy lifting, we integrate it through custom FFI bindings, and expose it through the existing Dataset API so users can read and write pixels without caring about the underlying format.

**Status**: Phases 1–4 (TIFF reading, writing, GeoTIFF metadata, and JPEG compression) are complete. COG support is tracked separately.

## Design Principles

- **Same API, different format.** Users interact with `DatasetReader`, `DatasetWriter`, `ImageAssetProvider`, and `MetadataProvider` exactly as they do for NITF. The only place TIFF-specific details surface is in metadata dictionaries.
- **Multiple images per file.** A TIFF file can contain multiple IFDs, each defining a separate image (subfile) with its own tags. The `TIFFDatasetReader` enumerates all full-resolution IFDs (those where `NewSubfileType` bit 0 is 0) as separate `ImageAssetProvider`s keyed as `"image_segment_0"`, `"image_segment_1"`, etc. Reduced-resolution overview IFDs (bit 0 = 1) are associated with their parent full-resolution image and handled through the cross-format image pyramid support (see {doc}`image-pyramid`). Each `ImageAssetProvider` exposes per-image metadata through its `metadata()` method, since TIFF tags are per-IFD. There are no text, graphic, or data segments — only image segments and a dataset-level `MetadataProvider`.
- **libtiff via custom FFI.** We write our own `sys.rs` / `ffi.rs` bindings to libtiff (BSD-licensed), dynamically linked, following the same pattern as OpenJPEG and libjpeg-turbo. No third-party `-sys` crates.
- **GeoTIFF tags parsed in Rust.** GeoTIFF metadata (GeoKeys, ModelTiepoint, ModelPixelScale, ModelTransformation) is parsed directly from libtiff's tag interface rather than linking libgeotiff as a second dependency. The GeoKey directory spec is straightforward enough to implement in Rust.
- **Format implementation operates on `&[u8]`, not files.** The TIFF format implementation never touches the filesystem. Like JBP, the core constructor is `from_bytes(&[u8])` and all parsing operates on a byte slice. The IO layer is solely responsible for deciding how to produce those bytes (file read, mmap, future S3-backed mmap, etc.). libtiff is accessed via `TIFFClientOpen` with memory read/seek callbacks over the byte slice — the same pattern used for OpenJPEG's memory stream adapters.
- **COG as a separate roadmap.** Cloud Optimized GeoTIFF is a tiled GeoTIFF with overviews in a specific IFD layout. We complete basic GeoTIFF read/write first, then layer COG support on top. COG depends on both this roadmap and the cross-format image pyramid support. See {doc}`cog` for the COG roadmap.

## I/O Architecture

The project enforces a clean separation between I/O (how bytes are obtained) and format implementation (how bytes are interpreted). This boundary exists at the `&[u8]` / `Arc<[u8]>` level:

```
IO layer (src/bindings/io.rs)
│   Decides how to load bytes:
│   - File::read_to_end() (current)
│   - mmap (future)
│   - S3-backed virtual mmap (future)
│
├── &[u8] / Arc<[u8]>  ← the abstraction boundary
│
├── JBP format (src/jbp/)
│   └── JBPDatasetReader::from_bytes(&[u8])
│       └── OpenJPEG: memory stream callbacks on &[u8]
│       └── libjpeg-turbo: compress/decompress on &[u8]
│
└── TIFF format (src/tiff/)
    └── TIFFDatasetReader::from_bytes(&[u8])
        └── libtiff: TIFFClientOpen with memory read/seek callbacks on &[u8]
```

Both readers accept only `&[u8]` via `from_bytes()` — format implementations never touch the filesystem. The `bindings/io.rs` module is the single place that handles file paths: it memory-maps the file with `memmap2` and passes the resulting byte slice to the appropriate reader's `from_bytes()`.

The TIFF implementation follows the same pattern. `TIFFClientOpen()` accepts custom function pointers for read, seek, close, and size operations. Our callbacks perform pointer arithmetic on the `&[u8]` byte slice, identical to the `MemoryReadStreamData` pattern in `src/jbp/j2k/ffi.rs` for OpenJPEG. libtiff never opens a file descriptor.

This design means that when the IO layer is later updated to produce bytes via S3 range requests, every format reader benefits automatically with zero code changes.

## Rust Source Layout

```
src/tiff/
├── mod.rs              # Module root, feature gate
├── sys.rs              # Raw FFI declarations for libtiff (TIFFClientOpen, TIFFReadTile, etc.)
├── ffi.rs              # Safe wrappers (TiffHandle, RAII cleanup, tag access, tile I/O)
├── tags.rs             # TIFF tag constants and typed accessors
├── geotiff.rs          # GeoKey directory parsing, CRS metadata extraction
├── ifd.rs              # IFD (Image File Directory) navigation, overview detection
├── reader.rs           # TIFFDatasetReader (implements DatasetReader)
├── writer.rs           # TIFFDatasetWriter (implements DatasetWriter)
├── image.rs            # TIFFImageAssetProvider (implements ImageAssetProvider)
└── metadata.rs         # TIFF/GeoTIFF metadata → MetadataProvider mapping
```

## Phase 1: libtiff FFI Bindings and Basic TIFF Reading

**Objective**: Read uncompressed and Deflate/LZW-compressed tiled TIFFs through the Dataset API.

**Scope**:
- `sys.rs` — Raw FFI declarations for libtiff: `TIFFClientOpen`, `TIFFClose`, `TIFFGetField`, `TIFFSetField`, `TIFFReadTile`, `TIFFWriteTile`, `TIFFReadEncodedTile`, `TIFFWriteEncodedTile`, `TIFFTileSize`, `TIFFNumberOfTiles`, `TIFFSetDirectory`, `TIFFCurrentDirectory`, `TIFFNumberOfDirectories`. Note: we use `TIFFClientOpen` exclusively (not `TIFFOpen`) so libtiff never touches the filesystem — all I/O goes through our memory callbacks on `&[u8]`.
- `ffi.rs` — Safe RAII wrapper (`TiffHandle`) with `Drop` for `TIFFClose`, typed tag getters, tile read/write methods, error callback capture (same thread-local pattern as OpenJPEG). Includes `MemoryReadStreamData` struct and read/seek/size callbacks for `TIFFClientOpen`, following the same pattern as `src/jbp/j2k/ffi.rs`.
- `tags.rs` — Constants for standard TIFF tags (ImageWidth, ImageLength, BitsPerSample, SamplesPerPixel, Compression, PhotometricInterpretation, TileWidth, TileLength, SampleFormat, PlanarConfiguration, etc.)
- `image.rs` — `TIFFImageAssetProvider` implementing `ImageAssetProvider`:
  - `get_block()` maps to `TIFFReadEncodedTile()` with conversion to band-sequential (CHW) format
  - `has_block()` always returns `true` (TIFF tiles are not sparse like NITF masked images)
  - `num_resolution_levels` returns 1 for basic TIFF (no overviews)
  - Handles stripped TIFFs by treating strips as full-width blocks stacked vertically (each strip spans ImageWidth pixels across, RowsPerStrip pixels tall)
- `reader.rs` — `TIFFDatasetReader` implementing `DatasetReader`:
  - Core constructor: `from_bytes(&[u8])` — opens via `TIFFClientOpen` with memory callbacks on the byte slice
  - Convenience method: `open(path)` — reads file into `Vec<u8>` then delegates to `from_bytes()` (same pattern as `JBPDatasetReader`)
  - Enumerates all IFDs, classifying each by `NewSubfileType`: full-resolution images (bit 0 = 0) become `ImageAssetProvider`s, overview IFDs (bit 0 = 1) are skipped in Phase 1 (handled by the cross-format image pyramid support, see {doc}`image-pyramid`)
  - Exposes one `ImageAssetProvider` per full-resolution IFD, keyed as `"image_segment_0"`, `"image_segment_1"`, etc.
  - Each `ImageAssetProvider` exposes per-IFD metadata through its `metadata()` method (TIFF tags are per-IFD)
  - Exposes dataset-level `MetadataProvider` with file-level information only (byte order, number of directories, number of image segments) — IFD-specific tags live on the per-asset `MetadataProvider`
- `metadata.rs` — Maps TIFF tags to a flat `MetadataProvider` dictionary. Used at the per-image-segment level (IFD-specific tags like `"ImageWidth"`, `"BitsPerSample"`, `"Compression"`) and at the dataset level (file-level info like byte order and directory count). Tag names follow libtiff conventions.
- `build.rs` — Add `configure_libtiff()` with pkg-config + conda + system library fallback (same pattern as OpenJPEG)
- `Cargo.toml` — Add `libtiff` feature flag, enabled by default

**Supported pixel types**:
- uint8, uint16, uint32, int8, int16, int32, float32, float64
- 1 to N bands (SamplesPerPixel)
- Chunky (RGBRGB) and planar (RRR...GGG...BBB...) configurations

**Supported compressions** (handled by libtiff internally):
- None (uncompressed)
- LZW
- JPEG (code 7)
- Deflate/ZLib
- PackBits

**Format detection**: Register `.tif` and `.tiff` extensions in the `IO` factory for auto-detection.

**Tasks**:
- [x] Create `src/tiff/sys.rs` with libtiff FFI declarations
- [x] Create `src/tiff/ffi.rs` with safe `TiffHandle` wrapper
- [x] Create `src/tiff/tags.rs` with TIFF tag constants
- [x] Create `src/tiff/image.rs` with `TIFFImageAssetProvider`
- [x] Create `src/tiff/reader.rs` with `TIFFDatasetReader`
- [x] Create `src/tiff/metadata.rs` with tag-to-metadata mapping
- [x] Create `src/tiff/mod.rs` with feature gate
- [x] Update `build.rs` with libtiff discovery
- [x] Update `Cargo.toml` with `libtiff` feature
- [x] Register TIFF format in `IO` factory (format detection by extension and magic bytes `II*\0` / `MM\0*`)
- [x] Add Python bindings for `TIFFDatasetReader` and `TIFFImageAssetProvider`
- [x] Add unit tests with small synthetic TIFF files in `data/unit/`

## Phase 2: Basic TIFF Writing

**Objective**: Write tiled TIFFs from `BufferedImageAssetProvider` through the Dataset API.

**Scope**:
- `writer.rs` — `TIFFDatasetWriter` implementing `DatasetWriter`:
  - Writes to an in-memory buffer via `TIFFClientOpen` with memory write callbacks, then flushes to disk on `close()` (same pattern as JBP writer — the format implementation produces bytes, the IO layer writes them)
  - Reads encoding hints from `BufferedMetadataProvider` using numeric TIFF tag IDs:
    - `"322"` / `"323"` — tile dimensions (TileWidth / TileLength, default 256×256)
    - `"259"` — compression (`"None"`, `"LZW"`, `"Deflate"`, default `"Deflate"`)
    - Use `TagNameResolver` for human-readable name-to-number conversion
  - Writes image data tile-by-tile via `TIFFWriteEncodedTile()`
  - Sets standard TIFF tags from image properties (dimensions, bands, bit depth, sample format)
  - Handles band-sequential to chunky/planar conversion based on `PlanarConfiguration`
- Single image per file — `add_asset()` accepts one `ImageAssetProvider`; additional calls raise an error
- Non-image assets (text, graphics, data) raise `UnsupportedAsset` errors with a clear message

**Encoding hints** (via `BufferedMetadataProvider`):

| Field | Description | Example Values |
|-------|-------------|----------------|
| `Compression` | TIFF compression | `None`, `LZW`, `Deflate` (default) |
| `TileWidth` | Tile width in pixels | `256`, `512` (default: 256) |
| `TileHeight` | Tile height in pixels | `256`, `512` (default: 256) |
| `Predictor` | Compression predictor | `None`, `Horizontal` (default for LZW/Deflate) |

Encoding hints use numeric TIFF tag IDs as keys (e.g. `"259"` for Compression, `"322"` for TileWidth).
Use `TagNameResolver` to convert human-readable names to numeric keys:

```python
from aws.osml.io.tiff import TagNameResolver
tag_dict = metadata.as_dict()
resolver = TagNameResolver(tag_dict)
resolver["TileWidth"] = "256"       # stores under key "322"
resolver["Compression"] = "Deflate" # stores under key "259"
```

**Tasks**:
- [x] Create `src/tiff/writer.rs` with `TIFFDatasetWriter`
- [x] Add encoding hint parsing from `BufferedMetadataProvider`
- [x] Add Python bindings for `TIFFDatasetWriter`
- [x] Add write unit tests (write then read-back verification)
- [x] Update `IO` factory to support `IO.open(paths, "w", "tiff")`

## Phase 3: GeoTIFF Metadata

**Objective**: Parse and write GeoTIFF tags so geospatial metadata is available through `MetadataProvider`.

**Scope**:
- `geotiff.rs` — GeoTIFF metadata handling:
  - Parse GeoKey directory from tag 34735 (`GeoKeyDirectoryTag`)
  - Parse double params from tag 34736 (`GeoDoubleParamsTag`)
  - Parse ASCII params from tag 34737 (`GeoAsciiParamsTag`)
  - Parse `ModelTiepointTag` (33922) — ground control points
  - Parse `ModelPixelScaleTag` (33550) — pixel size in CRS units
  - Parse `ModelTransformationTag` (34264) — full affine transform
  - Map GeoKeys to human-readable metadata:
    - `GTModelTypeGeoKey` → `"GeoModelType"` (`"Projected"`, `"Geographic"`)
    - `GTRasterTypeGeoKey` → `"GeoRasterType"` (`"PixelIsArea"`, `"PixelIsPoint"`)
    - `ProjectedCSTypeGeoKey` → `"GeoProjectedCRS"` (EPSG code)
    - `GeographicTypeGeoKey` → `"GeoGeographicCRS"` (EPSG code)
  - Expose pixel-to-CRS affine transform as metadata fields:
    - `"GeoTransform"` — 6-element affine (GDAL convention: `[origin_x, pixel_width, rotation_x, origin_y, rotation_y, pixel_height]`)
    - `"GeoPixelScale"` — `[scale_x, scale_y, scale_z]`
    - `"GeoTiepoints"` — list of `[pixel_x, pixel_y, pixel_z, geo_x, geo_y, geo_z]` tuples
- Writer support: set GeoTIFF tags from metadata hints when writing
- Metadata fields are strings/JSON in the `MetadataProvider` dictionary, consistent with how NITF metadata is exposed

**Metadata mapping example**:
```python
with IO.open(["image.tif"], "r") as reader:
    meta = reader.metadata.as_dict()
    # Standard TIFF tags
    meta["ImageWidth"]       # "1024"
    meta["ImageLength"]      # "1024"
    meta["BitsPerSample"]    # "16"
    meta["Compression"]      # "Deflate"
    
    # GeoTIFF metadata
    meta["GeoModelType"]     # "Projected"
    meta["GeoProjectedCRS"]  # "32618"  (EPSG:32618 = UTM Zone 18N)
    meta["GeoTransform"]     # "[300000.0, 0.5, 0.0, 4500000.0, 0.0, -0.5]"
    meta["GeoPixelScale"]    # "[0.5, 0.5, 0.0]"
```

**Tasks**:
- [x] Create `src/tiff/geotiff.rs` with GeoKey directory parser
- [x] Add GeoTIFF tag reading to `TIFFDatasetReader` metadata
- [x] Add GeoTIFF tag writing to `TIFFDatasetWriter`
- [x] Add unit tests with GeoTIFF test files
- [x] Add integration tests with real-world GeoTIFF files in `data/integration/`

## Phase 4: JPEG Compression (Code 7)

**Objective**: Read and write JPEG-compressed TIFFs through the existing Dataset API, with configurable quality.

**Scope**:
- `tags.rs` — New constants: `COMPRESSION_JPEG` (7), `TIFFTAG_JPEGQUALITY` (65537), `PHOTOMETRIC_YCBCR` (6)
- `reader.rs` — Add `COMPRESSION_JPEG` to `SUPPORTED_COMPRESSIONS`; validate 8-bit sample depth for JPEG IFDs
- `writer.rs` — JPEG-specific logic in `TiffEncodingHints` and `write_image_ifd`:
  - Accept compression code `7` in encoding hints
  - Force predictor to 1 (None) for JPEG — predictor is incompatible
  - Set `PhotometricInterpretation` to YCbCr (6) for ≥3 bands, MinIsBlack (1) for <3 bands
  - Set `TIFFTAG_JPEGQUALITY` pseudo-tag before writing tiles (default quality 75, configurable via key `"65537"`)
  - Reject non-8-bit pixel types with `CodecError::InvalidFormat`
- Encoding hints migration — Tags 259, 317, and 284 now accept only numeric values (e.g. `7` instead of `"JPEG"`). Human-readable name resolution is handled by the Python `TagNameResolver` before values reach Rust.

**Encoding hints** (JPEG-specific):

| Key | Description | Valid Values | Default |
|-----|-------------|-------------|---------|
| `"259"` | Compression | `7` (JPEG) | — |
| `"65537"` | JPEG quality | `1`–`100` | `75` |

**Constraints**:
- JPEG compression requires 8-bit samples (`BitsPerSample = 8`)
- Predictor is always forced to 1 (None) regardless of metadata
- Band count must be 1 or ≥3 for JPEG

**Tasks**:
- [x] Add `COMPRESSION_JPEG`, `TIFFTAG_JPEGQUALITY`, `PHOTOMETRIC_YCBCR` constants to `tags.rs`
- [x] Migrate encoding hints parser from string to numeric values
- [x] Add JPEG writer logic (photometric, quality, predictor override, 8-bit validation)
- [x] Add `COMPRESSION_JPEG` to reader's `SUPPORTED_COMPRESSIONS`
- [x] Add 8-bit sample depth validation for JPEG in reader
- [x] Update Python test strategies and existing tests to use numeric values
- [x] Add JPEG roundtrip property tests with PSNR ≥ 30 dB fidelity check
- [x] Update documentation

## Testing Plan

### Unit Tests (Rust)

- `src/tiff/` inline `#[cfg(test)]` modules for:
  - FFI wrapper correctness (tag read/write, tile I/O)
  - GeoKey directory parsing
  - IFD navigation and overview detection
  - Metadata mapping
  - Pixel format conversions (chunky ↔ planar ↔ band-sequential)

### Unit Tests (Python)

- `tests/test_tiff_reader.py` — Read synthetic TIFF files, verify dimensions, pixel values, metadata
- `tests/test_tiff_writer.py` — Write then read-back, verify pixel-perfect roundtrip
- `tests/test_tiff_geotiff.py` — GeoTIFF metadata parsing and writing

### Property-Based Tests

Extend the existing property test suite under `tests/property/`:

- `tests/property/test_tiff_roundtrip.py` — Lossless roundtrip for TIFF format:
  - Generate random images with varying dimensions, bands, pixel types, tile sizes
  - Write as TIFF via `IO.open(paths, "w", "tiff")`, read back, verify pixel-perfect match
  - Test all supported compressions (None, LZW, Deflate)
  - Test both chunky and planar configurations
- Update `tests/property/test_io_contracts.py`:
  - Add TIFF format auto-detection tests (`.tif`, `.tiff` extensions, magic bytes)
  - Add TIFF to the dataset roundtrip consistency tests
- Update `tests/property/strategies.py`:
  - Add `tiff_compression` strategy (draws from `["None", "LZW", "Deflate"]`)
  - Add `tiff_encoding_hints` strategy (generates valid `BufferedMetadataProvider` for TIFF)
  - Add `output_format` strategy that draws from `["nitf", "tiff"]` for cross-format tests
- Update `tests/property/test_image_roundtrip.py`:
  - Parameterize existing roundtrip tests to run against both NITF and TIFF output formats where applicable

### Integration Tests

- Add GeoTIFF test files to `data/integration/` (gitignored, documented in README)
- Test with real-world GeoTIFF files from public sources
- Marker: `pytest -m integration` to run

## Example Script Updates

### `scripts/describe_dataset.py`

Already format-agnostic — uses `IO.open()` and iterates assets by type. Changes needed:

- [ ] Update help text and examples to mention TIFF/GeoTIFF files
- [ ] Add GeoTIFF-specific metadata display (CRS, pixel scale, transform) when detected
- [ ] Handle the single-image-segment model gracefully (no text/graphic/data sections to report)
- [ ] Add example usage: `python scripts/describe_dataset.py image.tif --metadata`

### `scripts/chip_image.py`

Already format-agnostic — reads blocks from any `ImageAssetProvider`. Changes needed:

- [ ] Update help text and examples to mention TIFF input files
- [ ] No code changes expected — the chip extraction logic works against the abstract `ImageAssetProvider` interface
- [ ] Add example usage: `python scripts/chip_image.py input.tif output.png --bbox 0 0 512 512`
- [ ] Test with GeoTIFF inputs to verify end-to-end

### `scripts/generate_synthetic_image.py`

Currently generates NITF files only. Changes needed:

- [ ] Add `--format` argument: `nitf` (default) or `tiff`
- [ ] When format is `tiff`, use `IO.open(paths, "w", "tiff")` instead of `"nitf"`
- [ ] Set TIFF-specific encoding hints on `BufferedMetadataProvider` (`Compression`, `TileWidth`, `TileHeight`) instead of NITF hints (`IC`, `IMODE`, `NPPBH`, `NPPBV`)
- [ ] Add `--compression` choices for TIFF: `None`, `LZW`, `Deflate`
- [ ] Optionally accept `--crs` / `--pixel-scale` / `--origin` to write GeoTIFF metadata on synthetic images
- [ ] Add example usage: `python scripts/generate_synthetic_image.py output.tif --format tiff --bands 3 --compression Deflate`
- [ ] Masked images are not applicable for TIFF — skip `--masked` when format is `tiff`

## Future Consideration: Palette Color (Indexed Color) Expansion

TIFF files can use `PhotometricInterpretation = 3` (Palette color), where each pixel is a single-byte index into a 256-entry RGB color lookup table stored in the `ColorMap` tag (320). These files report as 1-band uint8 but appear as full-color images in viewers like macOS Preview, which apply the palette automatically.

The current reader behavior is correct at the data level: it returns the raw index values as a 1-band image, which is what the file actually contains. However, downstream consumers (e.g. `chip_image.py` saving to PNG) will produce grayscale output because they have no knowledge of the palette.

Rather than embedding palette expansion into the reader itself, this should be implemented as a separate post-read operation or utility. Reasons:

- The raw index values are the actual pixel data. Some workflows (classification maps, thematic rasters) need the indices, not the expanded RGB.
- Palette expansion is a presentation concern, not a decoding concern.
- A standalone `expand_palette(image_asset) -> ndarray` function or a `PaletteExpander` wrapper would keep the reader simple and give users control over when expansion happens.

The `ColorMap` tag values are already accessible through the per-IFD `MetadataProvider` once Phase 1 is complete. A palette expansion tool would read the `ColorMap` and `PhotometricInterpretation` metadata, then map the 1-band indices to 3-band RGB.

This is low priority relative to the core read/write/GeoTIFF phases but worth revisiting once real-world palette-color GeoTIFFs appear in integration testing.

## Build and Environment

- `environment.yml` — Add `libtiff` to conda dependencies
- `build.rs` — Add `configure_libtiff()` function:
  - pkg-config: `pkg-config --libs libtiff-4`
  - Conda: `$CONDA_PREFIX/lib/libtiff.{dylib,so}`
  - System paths: `/usr/local/lib`, `/opt/homebrew/lib`, etc.
  - Link: `cargo:rustc-link-lib=tiff`
- `scripts/setup-dev-env.sh` — No changes expected (libtiff doesn't need special `DYLD_LIBRARY_PATH` handling beyond what conda provides)

## Dependencies and Licensing

| Library | License | Link Method | Notes |
|---------|---------|-------------|-------|
| libtiff | BSD-like (libtiff license) | Dynamic | Compatible with Apache-2.0 |
| libgeotiff | Not used | N/A | GeoKey parsing implemented in Rust |

No new crate dependencies. All TIFF functionality is implemented through custom FFI bindings to libtiff, consistent with the project's licensing requirements.
