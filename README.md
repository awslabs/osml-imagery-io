# AWS OSML IO

Geospatial image format codecs for the OversightML ecosystem.

## Overview

AWS OSML IO provides high-performance Rust implementations of image format decoders and encoders for geospatial imagery formats. It is designed to be compatible with the [osml-imagery-toolkit](https://github.com/aws-solutions-library-samples/osml-imagery-toolkit) library.

### Supported Formats

- **NITF** - National Imagery Transmission Format (2.0, 2.1)
- **GeoTIFF** - Georeferenced TIFF images

## Installation

```bash
pip install aws-osml-io
```

## Usage

<!-- TODO: Add usage examples once codec implementations are complete -->

## Development

### Prerequisites

- Python 3.9+
- Rust 1.70+
- Conda (Miniconda or Anaconda)

### Setup

Create and activate the conda environment:

```bash
# Create environment from environment.yml
conda env create -f environment.yml

# Activate the environment
conda activate osml-imagery-io-dev

# If you use mise for runtime management, trust the project config
# This disables mise's Python/Node so conda takes precedence
mise trust

# Configure library paths for PyO3
source scripts/setup-dev-env.sh

# Build the extension module
maturin develop
```

To update the environment after changes to `environment.yml`:

```bash
conda env update -f environment.yml
```

### Building

```bash
# Build wheel
maturin build --release

# Build and install for development
maturin develop
```

### Testing

```bash
# Run Python tests
pytest

# Run Rust tests
cargo test
```

#### Property-Based Testing

This project uses property-based testing (PBT) to validate image codec correctness across many generated inputs. Property tests verify universal properties like roundtrip preservation, block access completeness, and metadata consistency.

```bash
# Run only property tests
pytest -m property

# Run only unit tests (exclude property tests)
pytest -m "not property"
```

For details on the PBT framework, available strategies, and how to add new properties, see [docs/PROPERTY_BASED_TESTING.md](docs/PROPERTY_BASED_TESTING.md).

### Linting

```bash
# Python
ruff check .

# Rust
cargo clippy
```

## License

This library is licensed under the Apache 2.0 License.
