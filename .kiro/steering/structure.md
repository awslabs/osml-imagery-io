# Project Structure

```
.
├── src/                    # Rust source code
│   ├── lib.rs              # Library entry point, PyO3 module registration
│   ├── error.rs            # Error types
│   ├── types.rs            # Common types (AssetType, PixelType)
│   ├── traits/             # Public Rust API - trait definitions
│   ├── buffered/           # In-memory implementations of traits
│   ├── parser/             # Internal parser library for format definitions
│   ├── jbp/                # JBP/NITF format implementation
│   └── bindings/           # Python bindings (PyO3)
├── python/
│   └── aws/osml/io/        # Python package (namespace package)
├── tests/                  # Python tests
│   ├── unit/               # Python unit tests
│   │   └── test_*.py       # Unit test modules
│   ├── benchmark/          # Python benchmark tests (pytest-benchmark)
│   ├── integration/        # Integration tests (manifest-driven)
│   └── property/           # Property-based tests (hypothesis)
│       ├── conftest.py     # Shared fixtures, pytest configuration
│       ├── strategies.py   # Reusable hypothesis strategies
│       ├── quality.py      # PSNR/SSIM calculation utilities
│       └── test_*.py       # Property test modules
├── benches/                # Rust benchmarks (Criterion)
├── docs/                   # Sphinx documentation site
│   ├── conf.py             # Sphinx configuration
│   ├── Makefile            # Build targets (html, pdf, etc.)
│   ├── index.md            # Documentation root
│   ├── getting-started.md  # Quick-start guide
│   ├── api/                # Python API reference (autodoc + hand-written)
│   ├── design/             # Architecture and design documents
│   ├── internal/           # Internal working notes (excluded from published site)
│   ├── roadmap/            # Format implementation roadmaps
│   ├── user-guide/         # End-user guides (reading/writing imagery, metadata, etc.)
│   ├── _static/            # Static assets (images, CSS)
│   └── _build/             # Generated output (gitignored)
├── data/                   # Test data directory
│   ├── unit/               # Small synthetic test files (checked in)
│   ├── integration/        # 3rd party validation data (gitignored)
│   └── benchmark/          # User-provided benchmark data (gitignored)
└── reference-materials/    # Reference specifications (gitignored)
```

## Rust Source Organization

The `src/` directory follows a modular structure designed for extensibility:

### Core Modules

- `traits/` - Public Rust API defining the core interfaces (`DatasetReader`, `DatasetWriter`, `ImageAssetProvider`, `MetadataProvider`, etc.). These traits are format-agnostic.

- `buffered/` - In-memory implementations of traits for convenience:
  - `BufferedMetadataProvider` - Mutable metadata storage for encoding hints
  - `BufferedImageAssetProvider` - In-memory image asset for synthetic images

- `parser/` - Internal parser library used by format implementations. Provides structure definitions, field accessors, and serialization for binary formats.

- `bindings/` - PyO3 Python bindings exposing the Rust API to Python. Each binding file wraps corresponding Rust types.

### Format Implementations

Each supported format has its own module under `src/`:

- `jbp/` - Joint BIIF Profile (JBP) implementation supporting NITF 2.0, NITF 2.1, and NSIF 1.0 formats

Future formats (e.g., TIFF/GeoTIFF) will be added as additional modules at this level.

### Naming Conventions

- Rust files use `snake_case` names
- File names should match the primary type they contain (e.g., `metadata.rs` contains `BufferedMetadataProvider`)
- Binding files are prefixed to indicate their purpose (e.g., `buffered_image.rs` for `PyBufferedImageAssetProvider`)

## Test Data

Three categories of test data, consolidated under `data/`:

1. **Unit test data** (`data/unit/`) - Checked into git. Small synthetic files for unit tests. Both Rust and Python tests reference this location.

2. **Integration data** (`data/integration/`) - Gitignored. Third-party validation data with good/bad imagery examples. Override location with `OSML_IO_INTEGRATION_DATA` env var.

3. **Benchmark data** (`data/benchmark/`) - Gitignored. Users place their own imagery here for performance testing. Override location with `OSML_IO_BENCHMARK_DATA` env var.

## Conventions

- Rust unit tests: inline with source in `src/` using `#[cfg(test)]`
- Python unit tests: in `tests/unit/` directory
- Python property tests: in `tests/property/` directory
- Python benchmark tests: in `tests/benchmark/` directory
- Python integration tests: in `tests/integration/` directory
- Rust benchmarks: in `benches/` directory, run with Criterion

## Property-Based Testing

Property tests are organized under `tests/property/` and validate universal correctness properties across many generated inputs.

### Key Files

- `conftest.py` - Shared fixtures (temp file handling) and pytest marker registration
- `strategies.py` - Reusable hypothesis strategies for generating images, block coordinates, and metadata
- `quality.py` - PSNR and SSIM calculation for lossy compression validation

### Test Modules

- `test_roundtrip.py` - Lossless/lossy roundtrip and idempotent encoding properties
- `test_block_access.py` - Block access completeness and reassembly properties
- `test_metadata.py` - Metadata preservation properties
- `test_api_contracts.py` - API polymorphism and contract tests
- `test_io_contracts.py` - IO factory and format auto-detection tests
- `test_strategies.py` - Strategy validation tests

### Relationship to Unit Tests

- Property tests validate universal properties across many generated inputs (100+ iterations)
- Unit tests validate specific examples, edge cases, and error conditions
- Both are complementary and run together with `pytest`
- Use `pytest -m property` to run only property tests
- Use `pytest -m "not property"` to run only unit tests
