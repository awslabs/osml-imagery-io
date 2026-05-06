# Technology Stack

## Languages

- Rust (2021 edition) - Core codec implementations
- Python 3.9+ - Bindings and user-facing API

## Build System

- Maturin 1.7+ - Builds Rust code as Python extension module
- Cargo - Rust package manager

## License Requirements

This project has strict licensing requirements for all dependencies:

### Allowed Licenses
- Apache 2.0 (preferred)
- MIT (preferred)
- BSD-2-Clause, BSD-3-Clause
- ISC
- Zlib
- Public Domain / Unlicense

### Prohibited Licenses
- GPL (any version) - NOT ALLOWED
- LGPL (any version) - NOT ALLOWED
- AGPL - NOT ALLOWED
- Any copyleft license that would require this project to be released under the same license

### Implications
- Do NOT add crate dependencies with GPL/LGPL licenses
- When linking to system libraries (e.g., OpenJPEG), use dynamic linking to avoid license contamination
- For FFI bindings, write custom bindings rather than using `-sys` crates that may have incompatible licenses
- Always verify license compatibility before adding new dependencies using `cargo license` or similar tools

### Example: OpenJPEG
OpenJPEG (libopenjp2) is BSD-2-Clause licensed, which is compatible. However, some Rust wrapper crates like `openjpeg-sys` or `openjpeg2-sys` may have different licensing. We use custom FFI bindings to avoid any licensing issues.

## Key Dependencies

### Rust
- pyo3 - Python bindings (abi3-py39)
- numpy - NumPy array interop
- thiserror - Error handling
- serde, serde_json, serde_yaml - Serialization
- memmap2 - Memory-mapped file I/O (core to the reader architecture)
- png - Pure-Rust PNG codec
- rayon - Work-stealing thread pool for data parallelism
- proptest - Property-based testing framework (dev dependency)
- criterion - Rust micro-benchmark framework (dev dependency)
- tempfile - Temporary files in tests (dev dependency)

Native C libraries linked via custom FFI (for license-compatibility reasons):

- libopenjp2 (OpenJPEG) — JPEG 2000 / HTJ2K
- libjpeg-turbo — JPEG DCT
- libtiff — TIFF/GeoTIFF

### Python
- pytest - Testing framework
- pytest-cov - Coverage reporting
- ruff - Linting and formatting
- hypothesis - Property-based testing framework

## Static Analysis Tools

The project uses several static analysis tools to maintain code quality, enforce license compliance, and detect architectural issues. Each tool is run directly — no wrapper script.

### Tools

Tools enforced in CI (hard failures):

- **cargo clippy** — Rust linter with cognitive complexity checking. Config in `clippy.toml`. The `clippy::cognitive_complexity` lint is enabled crate-wide in `src/lib.rs`. CI runs `cargo clippy --lib --tests -- -W warnings -A clippy::approx_constant`.
- **cargo-deny** — License compliance, security advisories, duplicate/banned crate detection. Config in `deny.toml`.
- **cargo-machete** — Detects unused dependencies in `Cargo.toml`. Fast, works on stable.

Informational / local-only:

- **cargo-geiger** — Audits unsafe code usage across the crate and all dependencies. Not enforced in CI.
- **cargo-modules** — Visualizes internal module structure and dependency graph. Outputs DOT format. Not enforced in CI.

### Installing Analysis Tools

```bash
cargo install cargo-deny --locked
cargo install cargo-machete --locked
cargo install cargo-geiger --locked
cargo install cargo-modules --locked
```

### Running Analysis

```bash
# Lint (includes cognitive complexity warnings via #![warn(clippy::cognitive_complexity)] in lib.rs)
cargo clippy --all-targets -- -D warnings

# License compliance, security advisories, banned crates
cargo deny check

# Unused dependencies
cargo machete

# Unsafe code audit (informational)
cargo geiger

# Module dependency graph (informational)
cargo modules structure --lib
cargo modules dependencies --lib > target/metrics/module-deps.dot
```

### Kiro Hooks

Hooks are configured in `.kiro/hooks/`:

- **License & Advisory Check** (`cargo-deny-check.json`) — Runs `cargo deny check` at agent stop, only if Cargo.toml was modified.
- **Unused Dependencies Check** (`unused-deps-check.json`) — Runs `cargo machete` at agent stop, only if Cargo.toml was modified.
- **Complexity Review** (`complexity-review.json`) — Runs a clippy cognitive-complexity check on Rust code at agent stop.
- **Run Tests After Task** (`run-tests-post-task.json`) — After a spec task completes, rebuilds with `maturin develop` and runs `cargo test --lib` and `pytest -x -q`.


### CI Integration

The `static-analysis` job in `.github/workflows/ci.yml` runs cargo-deny and cargo-machete on every push/PR. Cognitive complexity is checked in the `lint` job via clippy (the `#![warn(clippy::cognitive_complexity)]` attribute in `lib.rs` combined with `-D warnings` makes it a hard failure).

## Cargo Feature Flags

### `static`

The `static` feature enables static linking of native C libraries (libopenjp2, libjpeg-turbo, libtiff) into the extension module. It is used exclusively in CI release builds to produce self-contained wheels that don't require users to have C libraries installed.

When enabled, `build.rs` reads `DEP_OPENJP2_ROOT`, `DEP_JPEG_ROOT`, and `DEP_TIFF_ROOT` environment variables to locate pre-compiled static archives (`.a` files) and emits `cargo:rustc-link-lib=static=...` directives. When not enabled, the existing dynamic linking path is used unchanged.

This feature is not in the `default` set — it must be explicitly activated. Local development always uses dynamic linking.

## Release Workflow

The project uses a GitHub Actions workflow (`.github/workflows/release.yml`) to build and publish wheels to PyPI. It is completely separate from the CI workflow (`ci.yml`).

- Triggered by pushing a tag matching `v*` (e.g., `v0.1.0`)
- Compiles OpenJPEG, libjpeg-turbo, and libtiff from source as static libraries for each target platform
- Builds abi3 wheels (Python 3.9+ stable ABI) for 4 platforms: Linux x86_64, Linux aarch64, macOS x86_64, macOS arm64
- Builds an sdist as fallback for unsupported platforms
- Publishes all artifacts to PyPI via OIDC trusted publishing (no API tokens)

See `RELEASING.md` at the project root for the full release process.

## Common Commands

```bash
# Development build
maturin develop

# Release build
maturin build --release

# Release build with static linking (CI only — requires DEP_*_ROOT env vars)
maturin build --release --features static,extension-module,openjpeg,libjpeg-turbo,libtiff

# Run Python tests
pytest

# Run Rust tests
cargo test

# Run benchmarks
cargo bench

# Lint Python
ruff check .

# Lint Rust
cargo clippy

# License check only
cargo deny check
```

## Development Environment Setup

This project requires both Rust and Python. Use conda to manage the Python environment.

### Initial Setup

```bash
# Create environment from environment.yml
conda env create -f environment.yml

# Activate the environment
conda activate osml-imagery-io-dev

# Configure library paths for PyO3
source scripts/setup-dev-env.sh

# Build the extension module
maturin develop

# Verify setup
pytest
cargo test
```

### Updating the Environment

After changes to `environment.yml`:

```bash
conda env update -f environment.yml
```

### Why source the setup script?

PyO3 links against `libpython` at runtime. The conda environment places the Python shared library in a non-standard location. The setup script sets `DYLD_LIBRARY_PATH` (macOS) or `LD_LIBRARY_PATH` (Linux) so `cargo test` can find it.

For permanent setup, add to your shell profile (`~/.zshrc` or `~/.bashrc`):

```bash
# Add to shell profile for permanent setup
source /path/to/osml-imagery-io/scripts/setup-dev-env.sh
```

## Instructions for Kiro

The conda environment `osml-imagery-io-dev` is automatically activated in Kiro's bash shell. All Python tools (pytest, maturin, ruff) are on PATH. Run commands directly without any prefix.

### Running Tests

```bash
# Run Python tests
pytest

# Run Python tests with verbose output
pytest -v

# Run specific test file
pytest tests/unit/test_parser.py -v

# Run Rust tests
cargo test
```

### Building

```bash
# Development build
maturin develop

# Release build
maturin build --release
```

### Linting

```bash
# Python linting
ruff check .

# Rust linting
cargo clippy
```

### If tests fail

If Python tests fail with import errors or module not found errors:
1. Ensure the conda env exists: `conda env list`
2. Rebuild if needed: `maturin develop`
3. Run tests: `pytest`

## Documentation Build System

The project uses Sphinx with MyST-Parser to build documentation from Markdown sources in `docs/`.

### Key Tools

- Sphinx - Documentation generator
- MyST-Parser - Markdown support for Sphinx (with `colon_fence` and `fieldlist` extensions)
- Furo - HTML theme
- sphinx-autodoc-typehints - Type hint rendering in API docs
- sphinxcontrib-mermaid - Mermaid diagram support
- pdflatex + latexmk - PDF output (optional, requires MacTeX)

### Building Docs

```bash
# HTML output
make html -C docs

# PDF output (requires pdflatex)
make pdf -C docs

# User Guide PDF only
make pdf-user-guide -C docs

# Clean build artifacts
make clean -C docs
```

### Documentation Structure

- `docs/api/` - Python API reference (autodoc + hand-written pages)
- `docs/design/` - Architecture and design documents (API design, parser design, property testing)
- `docs/internal/` - Internal working notes and bug investigations (excluded from published site via `exclude_patterns`)
- `docs/roadmap/` - Format implementation roadmaps (JBP, TIFF)
- `docs/user-guide/` - End-user guides for reading/writing imagery, metadata, assets

### Notes

- `docs/internal/` is excluded from the published Sphinx site but lives in the repo for developer reference.
- Intersphinx links to Python and NumPy docs are configured.
- LaTeX output handles Unicode emoji via `newunicodechar` substitutions in `conf.py`.

## Test Markers

```bash
# Run integration tests (requires data in data/integration/)
pytest -m integration

# Run benchmark tests
pytest -m benchmark

# Run property-based tests only (dev profile, fast)
pytest -m property

# Run property tests with CI profile (100 examples, thorough)
HYPOTHESIS_PROFILE=ci pytest -m property

# Run Python unit tests only
pytest tests/unit/

# Run unit tests only (exclude property tests)
pytest -m "not property"

# Run property tests with verbose output
pytest -m property -v

# Show per-test durations to find slow tests
pytest -m property --durations=0
```

### Integration Tests

Integration tests are manifest-driven. Test entries are defined in `data/integration/manifest.yaml`, each with a `path`, `label`, `tags`, and optional `expected_exception`/`expected_message` fields. Tests are parametrized from the manifest at collection time.

```bash
# Run all integration tests
pytest tests/integration/ -m integration

# Run with verbose output
pytest tests/integration/ -v -m integration

# Exclude entries tagged "slow"
pytest tests/integration/ -m integration --exclude-tags slow

# Exclude multiple tags
pytest tests/integration/ -m integration --exclude-tags slow,sicd

# Run only entries with a specific tag
pytest tests/integration/ -m integration --include-tags sidd

# Combine include and exclude
pytest tests/integration/ -m integration --include-tags sidd --exclude-tags slow
```

Tag filtering is done via `--exclude-tags` and `--include-tags` CLI options registered in `tests/integration/conftest.py`. Both accept comma-separated tag names and filter manifest entries before parametrization. Tags are freeform strings defined per entry in the manifest YAML.

Integration test data lives in `data/integration/` (gitignored). Override the location with the `OSML_IO_INTEGRATION_DATA` environment variable.

### Hypothesis Profiles

Property tests use hypothesis profiles defined in `tests/property/conftest.py`:

- `dev` (default): 10 examples, no shrink phase — fast iteration
- `ci`: 100 examples, full shrink phase — thorough coverage

Set the profile via the `HYPOTHESIS_PROFILE` environment variable. CI should set `HYPOTHESIS_PROFILE=ci`.

## Example Scripts

The `scripts/` directory contains user-facing example scripts that demonstrate common library workflows. See `scripts/README.md` for full documentation.

### Available Scripts

- `survey_datasets.py` — Scan a directory and summarize datasets in a table
- `describe_dataset.py` — Dump detailed info and metadata for a single dataset
- `chip_image_local.py` — Extract a region from a local file and save as PNG
- `chip_image_zarr.py` — Extract a region via a Zarr tile index (local or S3), supports `--level` for multiscale
- `generate_synthetic_image.py` — Create a single-level test image (NITF, TIFF, PNG, J2K, JPEG)
- `generate_synthetic_image_pyramid.py` — Create a multi-resolution pyramid (COG or NITF R-set)
- `generate_tile_index.py` — Build a Zarr tile index (JSON or Parquet) for cloud-native access

### When to Use

- To quickly generate test data for development or debugging, use `generate_synthetic_image.py` or `generate_synthetic_image_pyramid.py`.
- To inspect existing imagery files, use `survey_datasets.py` (directory scan) or `describe_dataset.py` (single file).
- To test the Zarr/cloud-native read path end-to-end: generate a pyramid → build a tile index → chip via Zarr.
- Internal scripts (`generate_benchmark_data.py`, `generate_test_data.py`, `generate_benchmark_report.py`, `setup-dev-env.sh`) support the build and are not user examples.

## Benchmarking

This project uses two distinct benchmarking tiers:

### Rust Micro-Benchmarks (Criterion)

Criterion benchmarks live in `benches/` and measure isolated Rust functions (e.g., interleave kernels, endian swap, block decode). These are useful for profiling and optimizing internal hot paths without the overhead of Python bindings or end-to-end I/O.

```bash
# Run all Criterion benchmarks
cargo bench

# Run a specific benchmark by name
cargo bench --bench decode_benchmarks

# Compile benchmarks without running (useful for CI)
cargo bench --no-run
```

When optimizing a code path:
1. Write the Criterion benchmark first, before making changes
2. Run it to establish a baseline on the unmodified code
3. Make the optimization
4. Re-run the benchmark to measure improvement
5. At each checkpoint, re-run to confirm cumulative gains

Criterion stores results in `target/criterion/` and automatically compares against the previous run, reporting percentage change.

### Python End-to-End Benchmarks (pytest-benchmark)

Python benchmarks use the `pytest -m benchmark` marker and exercise the full pipeline through the Python bindings (file open → decode → NumPy array). These measure real-world throughput including PyO3 overhead, GIL interactions, and memory copies into NumPy.

```bash
# Run Python benchmark tests
pytest -m benchmark

# Run with benchmark comparison
pytest -m benchmark --benchmark-compare
```

### When to Use Which

- Use Criterion when optimizing a specific Rust function or comparing algorithm variants (e.g., tiled vs naive transpose, serial vs parallel). No conda environment needed.
- Use pytest benchmarks when measuring user-visible performance through the Python API. Requires conda environment and `maturin develop`.
- For optimization work, always establish Criterion baselines before changes and re-run at checkpoints to validate incremental improvements.
