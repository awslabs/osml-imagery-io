# Technology Stack

## Languages

- Rust (2021 edition) - Core codec implementations
- Python 3.9+ - Bindings and user-facing API

## Build System

- Maturin 1.7+ - Builds Rust code as Python extension module
- Cargo - Rust package manager

## Key Dependencies

### Rust
- pyo3 - Python bindings
- numpy - NumPy array interop
- thiserror - Error handling
- serde_json - JSON serialization

### Python
- pytest - Testing framework
- pytest-cov - Coverage reporting
- ruff - Linting and formatting

## Common Commands

```bash
# Development build
maturin develop

# Release build
maturin build --release

# Run Python tests
pytest

# Run Rust tests (see note below)
cargo test

# Run benchmarks
cargo bench

# Lint Python
ruff check .

# Lint Rust
cargo clippy
```

## Running Rust Tests with PyO3

This project uses PyO3 for Python bindings, which requires access to the Python shared library at runtime. On macOS, Apple's System Integrity Protection (SIP) can strip `DYLD_*` environment variables, so you need to set this in your shell profile for it to persist.

**Note for Kiro**: The user's environment is already configured with the necessary library paths. Do NOT source `scripts/setup-rust-env.sh` or set `DYLD_LIBRARY_PATH` - just run `cargo test` directly.

### Permanent Setup (Recommended)

Add the following to your shell profile (`~/.zshrc` for zsh, `~/.bashrc` for bash):

```bash
# For macOS with conda/venv - enables cargo test with PyO3
export DYLD_LIBRARY_PATH="/opt/miniconda3/lib:$DYLD_LIBRARY_PATH"
# Or dynamically detect the path:
# export DYLD_LIBRARY_PATH="$(python3 -c 'import sysconfig; print(sysconfig.get_config_var(\"LIBDIR\"))')":$DYLD_LIBRARY_PATH
```

For Linux, use `LD_LIBRARY_PATH` instead:
```bash
export LD_LIBRARY_PATH="$(python3 -c 'import sysconfig; print(sysconfig.get_config_var(\"LIBDIR\"))')":$LD_LIBRARY_PATH
```

After adding to your profile, restart your terminal or run `source ~/.zshrc`.

### Quick Setup (Per-Session)

If you don't want to modify your profile, source the setup script:

```bash
source scripts/setup-rust-env.sh
cargo test
```

### Why is this needed?

PyO3 links against `libpython` at runtime. When using conda or venv, the Python shared library is in a non-standard location that the dynamic linker doesn't search by default. The `DYLD_LIBRARY_PATH` (macOS) or `LD_LIBRARY_PATH` (Linux) tells the linker where to find it.

## Running Python Tests

**Note for Kiro**: If the default `python3` or `pytest` commands fail with import errors (e.g., `ModuleNotFoundError: No module named 'math'`), use the conda Python explicitly:

```bash
# Build the extension with conda Python
PATH="/opt/miniconda3/bin:$PATH" maturin develop

# Run Python tests with conda Python
PATH="/opt/miniconda3/bin:$PATH" /opt/miniconda3/bin/python3 -m pytest tests/
```

This ensures the correct Python environment is used for both building and testing.

## Test Markers

```bash
# Run integration tests (requires data in data/integration/)
pytest -m integration

# Run benchmark tests
pytest -m benchmark
```
