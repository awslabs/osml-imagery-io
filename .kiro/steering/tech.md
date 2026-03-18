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
- pyo3 - Python bindings
- numpy - NumPy array interop
- thiserror - Error handling
- serde_json - JSON serialization
- proptest - Property-based testing framework (dev dependency)

### Python
- pytest - Testing framework
- pytest-cov - Coverage reporting
- ruff - Linting and formatting
- hypothesis - Property-based testing framework

## Common Commands

```bash
# Development build
maturin develop

# Release build
maturin build --release

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

When running commands that require Python (pytest, maturin, etc.), you MUST ensure the conda environment is activated. Each bash command runs in a fresh shell, so activate the environment as a separate step first, then run subsequent commands.

### Activating the Conda Environment

Before running any Python commands, activate the conda environment:

```bash
conda activate osml-imagery-io-dev
```

### Running Tests

After activating the conda environment:

```bash
# Run Python tests
pytest

# Run Python tests with verbose output
pytest -v

# Run specific test file
pytest tests/test_reader.py -v

# Run Rust tests (works directly, no conda needed)
cargo test
```

### Building

After activating the conda environment:

```bash
# Development build
maturin develop

# Release build
maturin build --release
```

### Linting

After activating the conda environment for Python linting:

```bash
# Python linting
ruff check .

# Rust linting (works directly, no conda needed)
cargo clippy
```

### If tests fail

If Python tests fail with import errors or module not found errors:
1. Ensure the conda env exists: `conda env list`
2. Activate it: `conda activate osml-imagery-io-dev`
3. Rebuild if needed: `maturin develop`
4. Run tests: `pytest`

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

# Run unit tests only (exclude property tests)
pytest -m "not property"

# Run property tests with verbose output
pytest -m property -v

# Show per-test durations to find slow tests
pytest -m property --durations=0
```

### Hypothesis Profiles

Property tests use hypothesis profiles defined in `tests/property/conftest.py`:

- `dev` (default): 10 examples, no shrink phase — fast iteration
- `ci`: 100 examples, full shrink phase — thorough coverage

Set the profile via the `HYPOTHESIS_PROFILE` environment variable. CI should set `HYPOTHESIS_PROFILE=ci`.
