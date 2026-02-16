# Project Structure

```
.
├── src/                    # Rust source code
│   ├── lib.rs              # Library entry point
│   ├── error.rs            # Error types
│   ├── types.rs            # Common types
│   ├── traits/             # Core trait definitions
│   └── bindings/           # Python bindings (PyO3)
├── python/
│   └── aws/osml/io/        # Python package (namespace package)
├── tests/                  # Python tests
├── benches/                # Rust benchmarks (Criterion)
├── data/                   # Test data directory
│   ├── unit/               # Small synthetic test files (checked in)
│   ├── integration/        # 3rd party validation data (gitignored)
│   └── benchmark/          # User-provided benchmark data (gitignored)
└── specification/          # Reference specifications (gitignored)
```

## Test Data

Three categories of test data, consolidated under `data/`:

1. **Unit test data** (`data/unit/`) - Checked into git. Small synthetic files for unit tests. Both Rust and Python tests reference this location.

2. **Integration data** (`data/integration/`) - Gitignored. Third-party validation data with good/bad imagery examples. Override location with `OSML_IO_INTEGRATION_DATA` env var.

3. **Benchmark data** (`data/benchmark/`) - Gitignored. Users place their own imagery here for performance testing. Override location with `OSML_IO_BENCHMARK_DATA` env var.

## Conventions

- Rust unit tests: inline with source in `src/` using `#[cfg(test)]`
- Python tests: in `tests/` directory, run with pytest
- Rust benchmarks: in `benches/` directory, run with Criterion
