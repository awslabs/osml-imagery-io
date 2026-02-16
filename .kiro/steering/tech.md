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

# Run Rust tests
cargo test

# Run benchmarks
cargo bench

# Lint Python
ruff check .

# Lint Rust
cargo clippy
```

## Test Markers

```bash
# Run integration tests (requires data in data/integration/)
pytest -m integration

# Run benchmark tests
pytest -m benchmark
```
