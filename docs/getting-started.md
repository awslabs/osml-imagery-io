# Getting Started

## Installation

### Prerequisites

- Python 3.9+
- Rust toolchain (2021 edition)
- Conda (recommended for managing native dependencies)

### Environment Setup

```bash
# Create the conda environment
conda env create -f environment.yml

# Activate the environment
conda activate osml-imagery-io-dev

# Configure library paths for PyO3
source scripts/setup-dev-env.sh

# Build the extension module
maturin develop

# Verify the setup
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

## Quickstart

### Reading a NITF File

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    # Discover available assets
    image_keys = dataset.get_asset_keys(asset_type="image")

    # Access the first image
    image = dataset.get_asset(image_keys[0])
    print(f"Image shape: {image.image_shape}")  # (bands, rows, cols)

    # Read a block
    block = image.get_block(0, 0, resolution_level=0)
    print(f"Block shape: {block.shape}")
```

### Writing a NITF File

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
import numpy as np

# Create encoding hints
metadata = BufferedMetadataProvider()
metadata.set("IC", "NC")        # No compression
metadata.set("IMODE", "B")      # Band interleave by block
metadata.set("NPPBH", "256")    # Block width
metadata.set("NPPBV", "256")    # Block height

# Create an image in memory
image_data = np.zeros((3, 512, 512), dtype=np.uint8)
provider = BufferedImageAssetProvider.create(
    key="image_0",
    num_columns=512,
    num_rows=512,
    num_bands=3,
    block_width=256,
    block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)
provider.set_full_image(image_data)

with IO.open(["output.ntf"], "w", "nitf") as writer:
    writer.add_asset("image_segment_0", provider)
```

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

# Run property-based tests only
pytest -m property

# Lint
ruff check .
cargo clippy
```

## Building the Documentation

```bash
# Build the extension first (autodoc needs to import the module)
maturin develop

# Build the docs
sphinx-build docs docs/_build/html

# Or using the Makefile
make -C docs html

# Preview locally
python -m http.server -d docs/_build/html 8000
```
