# Getting Started

## Installation

```bash
pip install osml-imagery-io
```

That's it. The wheels are self-contained — Rust core and C codecs are bundled. No
system libraries, no C toolchain, no conda required at runtime.

## Read an Image

```python
from aws.osml.io import imread

pixels = imread("image.ntf")
print(pixels.shape)  # (3, 1024, 1024) — CHW layout (bands, height, width)
print(pixels.dtype)  # uint8
```

All images are returned as NumPy arrays in CHW layout `(bands, height, width)`,
which integrates directly with PyTorch and other ML frameworks.

### Read a Windowed Region

Extract a rectangular sub-region without loading the full image. The window is
`(x, y, width, height)` where `x` and `y` are column and row offsets:

```python
chip = imread("image.ntf", window=(100, 200, 256, 256))
print(chip.shape)  # (3, 256, 256)
```

### Select Specific Bands

Read only the bands you need by passing zero-based indices:

```python
red_green = imread("image.ntf", bands=[0, 1])
print(red_green.shape)  # (2, 1024, 1024)
```

## Save an Image

Write a NumPy array to a file. The format is inferred from the extension:

```python
from aws.osml.io import imsave
import numpy as np

data = np.random.randint(0, 255, (3, 512, 512), dtype=np.uint8)

imsave("output.tif", data)   # GeoTIFF with Deflate compression
imsave("output.ntf", data)   # NITF with JPEG 2000 lossless
imsave("output.png", data)   # PNG
```

2-D arrays `(height, width)` are treated as single-band images automatically:

```python
grayscale = np.zeros((256, 256), dtype=np.uint8)
imsave("gray.png", grayscale)
```

## Inspect Metadata

Get image properties and the full format-specific metadata without reading any pixels:

```python
from aws.osml.io import iminfo

info = iminfo("image.ntf")
print(f"{info.width}x{info.height}, {info.bands} bands, {info.dtype}")
```

The `metadata` attribute gives you the full format-specific metadata dictionary
for the image segment — NITF subheader fields and parsed TREs, or TIFF IFD tags:

```python
# Rational polynomial coefficients for geopositioning
rpc = info.metadata["RPC00B"]
print(rpc["LAT_OFF"], rpc["LINE_SCALE"])

# Acquisition context — mission, date
stdidc = info.metadata["STDIDC"]
print(stdidc["MISSION"], stdidc["ACQ_DATE"])

# Exploitation usability — GSD, sun angles
use00a = info.metadata["USE00A"]
print(use00a["MEAN_GSD"], use00a["SUN_EL"])
```

## Iterate Over Tiles

Process a large image in fixed-size tiles without loading it all into memory:

```python
from aws.osml.io import tiles

for tile in tiles("large_image.tif", tile_size=(256, 256)):
    print(f"Tile ({tile.tile_col}, {tile.tile_row}) at pixel ({tile.x}, {tile.y})")
    process(tile.data)
```

Edge tiles are smaller when the image dimensions are not evenly divisible by the tile
size. Add overlap between adjacent tiles for seamless stitching:

```python
for tile in tiles("large_image.tif", tile_size=(256, 256), overlap=(32, 32)):
    # stride = (224, 224), tiles share 32 pixels on each edge
    process(tile.data)
```

## What's Underneath

The convenience functions are a thin layer over a full-featured, block-level API.
Here is the same read expressed both ways, so you can see what the convenience layer
handles for you:

**Convenience — one line:**

```python
from aws.osml.io import imread

pixels = imread("image.ntf")
```

**Low-level equivalent:**

```python
from aws.osml.io import IO
import numpy as np

with IO.open("image.ntf", "r") as dataset:
    keys = dataset.get_asset_keys(asset_type="image", roles=["data"])
    image = dataset.get_asset(keys[0])

    bw = image.num_pixels_per_block_horizontal or image.num_columns
    bh = image.num_pixels_per_block_vertical or image.num_rows
    grid_rows, grid_cols = image.block_grid_size

    dtype = np.dtype(image.pixel_value_type.to_numpy_dtype())
    pixels = np.zeros((image.num_bands, image.num_rows, image.num_columns), dtype=dtype)

    for r in range(grid_rows):
        for c in range(grid_cols):
            block = image.get_block(r, c, resolution_level=0)
            y0, x0 = r * bh, c * bw
            pixels[:, y0:y0 + block.shape[1], x0:x0 + block.shape[2]] = block
```

When you need fine-grained control — custom metadata, block-level access, compression
tuning, multi-asset datasets — the full API is always available. Start with the
[Introduction](introduction.md) to understand the library's design, then explore:

- [Datasets and the IO Interface](datasets-and-io.md) — how files are organized into
  typed, keyed assets
- [Image Assets](image-assets.md) — block-level pixel access, block grids, sparse images
- [Metadata](metadata.md) — TREs, GeoKeys, file-level and image-level metadata
- [Writing Imagery Assets](image-assets-writing.md) — compression, georeferencing,
  encoding hints, multi-file pyramids

## Development Setup

If you are building the library from source or contributing, you will need the Rust
toolchain and a conda environment:

```bash
conda env create -f environment.yml
conda activate osml-imagery-io-dev
source scripts/setup-dev-env.sh
maturin develop
pytest
cargo test
```

See [CONTRIBUTING.md](https://github.com/awslabs/osml-imagery-io/blob/main/CONTRIBUTING.md)
for details.
