# Property-Based Testing Framework

This document describes the property-based testing (PBT) framework for osml-imagery-io, explaining the conceptual model, organization, and how to extend it.

## Introduction

Property-based testing validates that code satisfies universal properties across many generated inputs, rather than testing specific examples. For image codecs, this approach is particularly valuable because:

1. **Combinatorial explosion**: Image parameters (dimensions, pixel types, band counts, compression modes, block sizes) create a vast input space impossible to cover with example-based tests
2. **Edge case discovery**: Random generation finds edge cases humans might miss
3. **Regression prevention**: Properties serve as executable specifications that catch regressions
4. **Shrinking**: When tests fail, PBT libraries automatically find minimal failing examples

## Conceptual Model

Every image file the library handles is a container format. Some containers are complex multi-segment archives (JBP/NITF), others are single-image wrappers (JPEG, PNG), but they all share the same structure: a container envelope with headers/metadata wrapping compressed pixel data.

### Container Formats

| Container | Extensions | Profiles / Variants | Internal Compression Options |
|---|---|---|---|
| JBP (NITF) | `.ntf`, `.nitf`, `.nsif`, `.nsf` | NITF 2.1, NSIF 1.0 | NC, C3/M3/I1 (JPEG DCT), C8/M8 (J2K), C4/M4 (VQ) |
| TIFF | `.tif`, `.tiff` | GeoTIFF, COG | None, LZW, Deflate, PackBits, JPEG, JPEG 2000 |
| JPEG 2000 | `.jp2`, `.j2k`, `.jpx` | JP2, JPX | Wavelet (lossy/lossless) — inherent to format |
| JPEG | `.jpg`, `.jpeg` | JFIF, Exif | DCT (lossy) — inherent to format |
| PNG | `.png` | — | Deflate (lossless) — inherent to format |

The key distinction is between containers that support multiple compression schemes (JBP, TIFF) and containers where the compression is inherent to the format (JPEG, PNG, JP2). For the first group, tests split by compression scheme within the container directory. For the second group, the container and codec are inseparable, so the test directory covers both.

### Profiles vs. Formats

Some "formats" are really profiles of an existing container — they use the same file structure with additional constraints:

| Profile | Base Container | What Differs |
|---|---|---|
| NSIF 1.0 | JBP | Header version string, minor field constraints |
| GeoTIFF | TIFF | Additional GeoKey tags for CRS/projection metadata |
| COG | TIFF | IFD ordering, mandatory tiling, overview placement |

Profiles do not need their own test directories. They are tested as variations within their base container's directory:

- NSIF → `jbp/test_nsif.py` or parametrized variants of existing JBP tests
- GeoTIFF → `tiff/test_roundtrip_geotiff.py` (metadata-focused)
- COG → `tiff/test_cog.py` (structural constraints: tile layout, IFD order, overview levels)

## Property Categories

The framework organizes properties into three categories:

### Roundtrip Properties

Verify that encoding then decoding preserves data:

- **Lossless Roundtrip Preservation** — For lossless compression (IC=NC or COMRAT=N001.0), decoded images must exactly match originals
- **Lossy Roundtrip Quality Bounds** — For lossy compression, decoded images must meet quality thresholds (PSNR ≥ 30 dB, SSIM ≥ 0.95)
- **Idempotent Encoding** — Re-encoding a decoded image produces consistent results

### Structural Properties

Verify block access and resolution level behavior:

- **Block Access Completeness** — All valid block coordinates return data
- **Block Reassembly Roundtrip** — Reading all blocks and reassembling equals the original
- **Invalid Block Coordinate Error Handling** — Invalid coordinates raise appropriate errors

### API Contract Properties

Verify API behavior and polymorphism:

- **Metadata Roundtrip Preservation** — Metadata survives encode/decode cycles
- **Dataset Round-Trip Consistency** — Written datasets can be read back equivalently
- **Format Auto-Detection** — `IO.open()` correctly detects formats from extensions

### Masking Properties (JBP-specific)

Verify masked image behavior across all masked IC codes (NM, M8, M3):

- **Mask Pattern Preservation** — `has_block()` returns the same true/false pattern after roundtrip
- **Masked Block Data Correctness** — For provided blocks, decoded data matches the original (exact for lossless, within quality bounds for lossy)
- **Pad Pixel Value Preservation** — The pad pixel value is accessible and correct after roundtrip

## Quality Thresholds

For lossy compression validation:

| Metric | Threshold | Description |
|--------|-----------|-------------|
| PSNR | ≥ 30 dB | Peak Signal-to-Noise Ratio |
| SSIM | ≥ 0.95 | Structural Similarity Index |

These thresholds ensure lossy compression maintains acceptable visual quality while allowing compression artifacts.

## Test Organization

The top-level split is by container format, matching the Rust source layout (`src/jbp/`, `src/tiff/`) and the unit test naming (`test_jbp_reader.py`, `test_tiff_reader.py`). Profiles (NSIF, GeoTIFF, COG) live within their base container's directory. Cross-format tests and shared infrastructure remain at the top level.

```
tests/property/
├── conftest.py                          # Shared fixtures, pytest configuration
├── helpers.py                           # Write/read helpers, assertion utilities
├── quality.py                           # PSNR/SSIM calculation
├── strategies.py                        # Shared hypothesis strategies
│
├── jbp/                                 # JBP container (NITF 2.1 + NSIF 1.0)
│   ├── __init__.py
│   ├── test_roundtrip_uncompressed.py   # IC=NC lossless roundtrip
│   ├── test_roundtrip_j2k.py           # IC=C8 lossy J2K roundtrip
│   ├── test_roundtrip_jpeg.py          # IC=C3, I1 lossy JPEG roundtrip
│   ├── test_idempotent.py              # Double-roundtrip encoding stability
│   ├── test_masking.py                 # IC=NM, M8, M3 mask properties
│   ├── test_metadata.py                # Metadata preservation
│   ├── test_blocks.py                  # Block access completeness/reassembly
│   ├── test_text_roundtrip.py          # Text segment roundtrip
│   ├── test_graphic_roundtrip.py       # Graphic segment roundtrip
│   └── test_writer_contracts.py        # Writer contract tests
│
├── tiff/                                # TIFF container (incl. GeoTIFF + COG)
│   ├── __init__.py
│   ├── test_roundtrip_pixel.py          # Lossless pixel roundtrip
│   ├── test_roundtrip_geotiff.py        # GeoTIFF metadata roundtrip
│   ├── test_metadata.py                 # Tag roundtrip, field types
│   ├── test_blocks.py                   # Strip/tile dims, band subsetting
│   ├── test_idempotent.py               # Double-roundtrip encoding stability
│   └── test_api.py                      # TIFF API contract tests
│
│   # --- Future single-codec containers ---
│
├── jp2/                    # (future) JPEG 2000 container (.jp2, .j2k)
│   ├── __init__.py
│   ├── test_roundtrip.py               # Lossless + lossy roundtrip
│   └── test_metadata.py                # JP2 box metadata
│
├── jpeg/                   # (future) JPEG container (.jpg — JFIF/Exif)
│   ├── __init__.py
│   └── test_roundtrip.py               # Lossy roundtrip with quality bounds
│
├── png/                    # (future) PNG container (.png)
│   ├── __init__.py
│   └── test_roundtrip.py               # Lossless roundtrip
│
├── test_api_contracts.py                # Cross-format API polymorphism
├── test_io_contracts.py                 # IO factory, format detection
└── test_strategies.py                   # Strategy validation
```

## Shared Helpers

`helpers.py` provides focused helper functions that eliminate the mechanical write/read/compare boilerplate from each test. Each test method stays self-contained and readable; the helpers handle the ceremony.

### Format-Specific Write/Read Helpers

One per container format. Each handles temp file lifecycle, provider setup, `IO.open` for write and read, and full-image reassembly.

```python
def write_and_read_jbp(
    array, pixel_type, num_bands, num_rows, num_cols,
    metadata_hints: dict,
    block_width=64, block_height=64,
    format="nitf",
) -> np.ndarray:
    """Write a JBP/NITF file and read back the decoded image.
    The format parameter allows testing NSIF by passing format="nsif".
    """

def write_and_read_tiff(
    array, pixel_type, num_bands, num_rows, num_cols,
    hints: dict,
) -> np.ndarray:
    """Write a TIFF file and read back the decoded image."""

# Future: write_and_read_jp2, write_and_read_jpeg, write_and_read_png
```

### Format-Agnostic Assertion Helpers

```python
def assert_lossless_match(original: np.ndarray, decoded: np.ndarray):
    """Assert exact pixel equality, with NaN-aware comparison for floats."""

def assert_lossy_quality(original: np.ndarray, decoded: np.ndarray):
    """Assert PSNR >= MIN_PSNR_DB and SSIM >= MIN_SSIM."""
```

### Masking Helpers (JBP-specific)

```python
def write_masked_jbp(
    array, pixel_type, num_bands, num_rows, num_cols,
    block_height, block_width, provided_blocks: set,
    metadata_hints: dict,
) -> Path:
    """Write a masked JBP/NITF file. Returns the temp file path."""

def assert_mask_preserved(
    asset, provided_blocks: set,
    num_block_rows: int, num_block_cols: int,
):
    """Assert that the mask pattern survived the roundtrip."""
```

With these helpers, a typical test method shrinks from ~50 lines to ~10:

```python
@given(random_image(min_size=16, max_size=64, min_bands=1, max_bands=3))
@pbt_settings
def test_uncompressed_roundtrip(self, image_tuple):
    array, pixel_type, num_bands, num_rows, num_cols = image_tuple
    decoded = write_and_read_jbp(
        array, pixel_type, num_bands, num_rows, num_cols,
        metadata_hints={"IC": "NC"},
    )
    assert_lossless_match(array, decoded)
```

## Strategies

`strategies.py` lives at the top level since strategies are shared across formats. Many strategies are format-agnostic (`random_image`, `image_dimensions`, `pixel_types`, `realistic_image_for_compression`) and are reused directly by multiple container tests.

Format-specific strategies are grouped by naming convention:

| Prefix | Container | Examples |
|---|---|---|
| `tiff_*` | TIFF | `tiff_image_config`, `tiff_writable_image`, `tiff_encoding_hints` |
| `jpeg_*` | JPEG (as JBP codec or standalone) | `jpeg_image_for_compression`, `jpeg_comrat` |
| `masked_*` | JBP masking | `masked_image`, `masked_jpeg_image` |
| `geotiff_*` | GeoTIFF profile | `geotiff_metadata` |
| (future) `jp2_*` | JP2 container | `jp2_image_config`, `jp2_metadata` |
| (future) `png_*` | PNG container | `png_image_config` |
| (future) `cog_*` | COG profile | `cog_overview_config` |

If the file grows beyond ~1500 lines, split into `strategies_jbp.py`, `strategies_tiff.py`, etc.

## Running Property Tests

```bash
# Run all property tests (dev profile, fast)
pytest -m property

# Run with CI profile (100 examples + shrink phase, thorough)
HYPOTHESIS_PROFILE=ci pytest -m property

# Run only unit tests (exclude property tests)
pytest -m "not property"

# Show per-test durations to find slow tests
pytest -m property --durations=0

# Run JBP property tests
pytest tests/property/jbp/ -v

# Run TIFF property tests
pytest tests/property/tiff/ -v

# Run specific test file
pytest tests/property/jbp/test_roundtrip_uncompressed.py -v
```

## Extending for Future Formats

### Single-Codec Containers (JPEG, JP2, PNG)

JPEG, JPEG 2000, and PNG are container formats where the compression is inherent. Each gets its own directory because they are distinct container formats with their own metadata structures, constraints, and edge cases. The shared helpers make adding them lightweight:

- `jpeg/test_roundtrip.py` uses `jpeg_image_for_compression` (already exists) and `assert_lossy_quality`. The only new code is the write/read path through `IO.open(..., "w", "jpeg")`.
- `jp2/test_roundtrip.py` uses `realistic_image_for_compression` (already exists) and `assert_lossy_quality` for lossy, `assert_lossless_match` for lossless.
- `png/test_roundtrip.py` uses `random_image` (already exists) and `assert_lossless_match`.

Adding a new container means writing one new `write_and_read_<format>` helper function, not duplicating 40 lines of boilerplate per test.

### Cloud Optimized GeoTIFF (COG)

COG is a profile of TIFF, not a separate container. COG tests belong in `tiff/test_cog.py` and focus on structural properties. Pixel roundtrip correctness is already covered by the base TIFF tests.

COG-specific test properties:

- All IFDs use tiled layout (no strips)
- Overview IFDs precede the full-resolution IFD
- IFD and metadata are at the start of the file
- GeoTIFF metadata is present

### NSIF

NSIF is a version variant of the JBP container, not a separate format. The file structure, IC codes, and compression schemes are identical to NITF 2.1. NSIF tests belong in `jbp/` and focus on:

- Version string roundtrip: write as `"nsif"`, read back, verify header identifies as NSIF 1.0
- Any NSIF-specific field constraints
- Optionally parametrize existing roundtrip tests over `format=["nitf", "nsif"]`

A dedicated `jbp/test_nsif.py` or `@pytest.mark.parametrize` on select tests suffices. There is no need to duplicate every compression roundtrip for NSIF because the codecs are identical; only the container header differs.

### Adding a New Format: Checklist

1. Create `tests/property/<format>/` with `__init__.py`
2. Add a `write_and_read_<format>()` helper to `helpers.py`
3. Create `test_roundtrip.py` using existing strategies and assertion helpers
4. If the format has unique metadata, add `test_metadata.py`
5. If the format is a profile of an existing container, add it as a test file within the base container's directory instead of creating a new directory
6. Add format-specific strategies to `strategies.py` (or a `strategies_<format>.py` if the file is getting large)
7. Update `test_io_contracts.py` to cover the new format's extension detection

## Hypothesis Configuration

Settings are centralized in `tests/property/conftest.py` using hypothesis profiles. All test files import a shared `pbt_settings` object instead of defining their own.

### Profiles

| Profile | `max_examples` | Shrink Phase | Typical Runtime | Use Case |
|---------|---------------|--------------|-----------------|----------|
| `dev` (default) | 10 | Skipped | Fast | Local development, rapid iteration |
| `ci` | 100 | Enabled | Slower | CI pipelines, thorough coverage |

The active profile is selected via the `HYPOTHESIS_PROFILE` environment variable, defaulting to `dev`:

```python
settings.load_profile(os.getenv("HYPOTHESIS_PROFILE", "dev"))
```

### Shared Settings

The `pbt_settings` object inherits `max_examples` and `phases` from the active profile and sets `deadline=None` for I/O-bound tests:

```python
from tests.property.conftest import pbt_settings

@given(random_image())
@pbt_settings
def test_something(self, image_tuple):
    ...
```

### Why Skip Shrink in Dev?

Shrinking is valuable for diagnosing failures but adds significant time to every run. The `dev` profile omits `Phase.shrink` so local iteration stays fast. When a failure needs investigation, switch to the `ci` profile to get minimal failing examples.

## Relationship to Unit Tests

- Property tests validate universal properties across many generated inputs (100+ iterations)
- Unit tests validate specific examples, edge cases, and error conditions
- Both are complementary and run together with `pytest`
- Use `pytest -m property` to run only property tests
- Use `pytest -m "not property"` to run only unit tests

## References

- [PyTorch Vision Issue #3912](https://github.com/pytorch/vision/issues/3912) - Roundtrip testing for image codecs
- [Hypothesis: Canonical Serialization](https://hypothesis.works/articles/canonical-serialization/) - Testing serialization with PBT
- [Hypothesis NumPy Strategies](https://hypothesis.readthedocs.io/en/latest/numpy.html) - Generating NumPy arrays
- [proptest Book](https://proptest-rs.github.io/proptest/intro.html) - Rust property-based testing
