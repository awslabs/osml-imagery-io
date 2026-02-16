# Integration / Validation Test Data

This directory contains third-party test data used for integration and validation testing.

## Contents

This data includes both valid and invalid imagery samples used to validate the library's behavior against real-world and edge-case inputs.

## Obtaining the Data

This data is controlled by a third party and is not checked into the repository.

**For authorized users:**
- Contact the project maintainers for access instructions
- Download and extract the data into this directory

## Usage

Tests will automatically discover files in this directory. To run integration tests:

```bash
# Rust
cargo test --features integration

# Python
pytest -m integration
```

Tests will skip gracefully if this directory is empty.

## Environment Override

Set `OSML_IO_INTEGRATION_DATA` to use an alternate location:

```bash
export OSML_IO_INTEGRATION_DATA=/path/to/your/data
```
