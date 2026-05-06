# Project Structure

## Anchor Map

Only the non-obvious locations are listed. The full tree can be discovered with directory tools.

- `src/` — Rust source (one module per format, plus shared traits and bindings).
- `python/aws/osml/io/` — Python namespace package. The compiled extension (`_io`) is loaded from here.
- `tests/` — Python tests, organized by kind: `unit/`, `property/`, `integration/`, `benchmark/`.
- `benches/` — Rust Criterion benchmarks.
- `docs/` — Sphinx site, Markdown sources via MyST.
- `data/` — Test data, three tiers (see below).
- `reference-materials/` — Format specification PDFs (gitignored except the README).
- `scripts/` — User-facing example scripts and dev setup.

## Rust Source (`src/`)

### Shared Modules

- `traits/` — Public Rust API: format-agnostic interfaces (`DatasetReader`, `DatasetWriter`, `ImageAssetProvider`, `MetadataProvider`, etc.).
- `buffered/` — In-memory implementations of the traits (e.g., `BufferedMetadataProvider`, `BufferedImageAssetProvider`).
- `parser/` — Internal, data-driven binary parser library used by format implementations to define field structure and (de)serialization for binary records like TREs and DES.
- `bindings/` — PyO3 Python bindings. One file per exposed type (e.g., `buffered_image.rs` wraps `BufferedImageAssetProvider`).
- `composite/`, `image/` — Shared infrastructure used across format modules.
- `error.rs`, `types.rs`, `lib.rs` — Error types, shared types (`AssetType`, `PixelType`), and the PyO3 module entry point.

### Format Modules

Each supported format lives in its own module at `src/`:

- `jbp/` — Joint BIIF Profile (NITF 2.1, NSIF 1.0, SICD, SIDD).
- `tiff/` — TIFF / GeoTIFF / COG.
- `j2k/` — JPEG 2000 (including HTJ2K) via custom OpenJPEG FFI.
- `jpeg/` — JPEG DCT via libjpeg-turbo.
- `png/` — PNG via the pure-Rust `png` crate.

### Conventions

- Rust files use `snake_case` matching the primary type they contain.
- Binding files are named for the Python-exposed type they wrap.
- Rust unit tests live inline with source using `#[cfg(test)]`.

## Python Package (`python/aws/osml/io/`)

- `__init__.py` — Re-exports the public API (`imread`, `imsave`, `iminfo`, `tiles`, `IO`).
- `convenience.py` — Convenience wrappers over the low-level `IO` API.
- `zarr_codecs.py` — Custom Zarr v3 codecs for NITF, TIFF, and JPEG 2000.
- `virtualizarr_parsers.py` — VirtualiZarr parsers that build multi-resolution tile indexes.
- `multi_reference_fs.py` — Scatter-gather fsspec filesystem for non-contiguous byte ranges.
- `jbp/`, `tiff/` — Format-specific Python helpers.

## Tests (`tests/`)

- `unit/` — Python unit tests (`test_*.py`).
- `property/` — Hypothesis property-based tests. Shared helpers in `conftest.py`, `strategies.py`, `helpers.py`, `quality.py`. Top-level modules cover cross-cutting properties (`test_api_contracts.py`, `test_io_contracts.py`, `test_strategies.py`, `test_callback_provider.py`, `test_convenience.py`, `test_stream_io.py`) and per-format suites live in subdirectories (`jbp/`, `tiff/`, `png/`, `zarr/`).
- `integration/` — Manifest-driven tests (`data/integration/manifest.yaml`).
- `benchmark/` — `pytest-benchmark` tests for end-to-end Python throughput.

## Documentation (`docs/`)

Sphinx site with MyST-Markdown. Top-level sections that new content should fit into:

- `user-guide/` — End-user guides (reading/writing, metadata, assets, quick-start, cloud access).
- `api/` — Python API reference (autodoc plus hand-written context).
- `design/` — Architecture and design documents.
- `codecs/` — Per-format/per-codec design notes (`jbp-block.md`, `jpeg.md`, `jpeg2000.md`, `tiff-tile.md`).
- `roadmap/` — Format implementation roadmaps.
- `internal/` — Working notes, bug investigations, TODOs. Excluded from the published site via `exclude_patterns` in `conf.py`.
- `performance.md` — End-to-end performance benchmarks.

## Test Data (`data/`)

Three tiers:

1. **`data/unit/`** — Small synthetic files, checked into git. Referenced by both Rust and Python unit tests.
2. **`data/integration/`** — Third-party validation data, gitignored. Override location with `OSML_IO_INTEGRATION_DATA`.
3. **`data/benchmark/`** — User-provided benchmark data, gitignored. Override location with `OSML_IO_BENCHMARK_DATA`.

## Testing Conventions

- Rust unit tests: inline in `src/` (`#[cfg(test)]`).
- Python unit tests: `tests/unit/`.
- Property tests: `tests/property/` with the `property` marker.
- Integration tests: `tests/integration/` with the `integration` marker.
- Benchmark tests: `tests/benchmark/` with the `benchmark` marker.
- Rust benchmarks: `benches/` with Criterion.

Property tests validate universal properties across many generated inputs; unit tests validate specific examples and edge cases. Both are run by default via `pytest`; filter with `-m property` or `-m "not property"`.
