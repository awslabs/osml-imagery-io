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
- [Maturin](https://www.maturin.rs/)

### Setup

```bash
# Create virtual environment
python -m venv .venv
source .venv/bin/activate

# Install maturin
pip install maturin

# Build and install in development mode
maturin develop
```

### Building

```bash
# Build wheel
maturin build --release

# Build and install
maturin develop --release
```

### Testing

```bash
pip install -e ".[dev]"
pytest
```

## License

This library is licensed under the Apache 2.0 License.
