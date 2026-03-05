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

## Test Markers

```bash
# Run integration tests (requires data in data/integration/)
pytest -m integration

# Run benchmark tests
pytest -m benchmark

# Run property-based tests only
pytest -m property

# Run unit tests only (exclude property tests)
pytest -m "not property"

# Run property tests with verbose output
pytest -m property -v
```
