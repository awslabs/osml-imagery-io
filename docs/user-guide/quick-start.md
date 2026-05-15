# Quick Start

This page shows how to read, write, inspect, and tile geospatial imagery using the
convenience functions. Each example is self-contained and copy-pasteable. For the full
low-level API — block grids, metadata providers, compression controls — see the rest
of the user guide.

## Read an Image

Load an entire image as a NumPy array in CHW layout `(bands, height, width)`:

```python
from aws.osml.io import imread

pixels = imread("image.ntf")
print(pixels.shape)  # (3, 1024, 1024)
print(pixels.dtype)  # uint8
```

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

imsave("output.tif", data)          # GeoTIFF with Deflate compression
imsave("output.ntf", data)          # NITF with JPEG 2000 lossless
imsave("output.png", data)          # PNG
```

2-D arrays `(height, width)` are treated as single-band images automatically:

```python
grayscale = np.zeros((256, 256), dtype=np.uint8)
imsave("gray.png", grayscale)
```

## Inspect Image Metadata

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

Process a large image in fixed-size tiles without loading it all into memory. Tiles
are yielded lazily in row-major order (left-to-right, top-to-bottom):

```python
from aws.osml.io import tiles

for tile in tiles("large_image.tif", tile_size=(256, 256)):
    print(f"Tile ({tile.tile_col}, {tile.tile_row}) at pixel ({tile.x}, {tile.y})")
    print(f"  shape: {tile.data.shape}")
    # Process tile.data ...
```

Edge tiles are smaller when the image dimensions are not evenly divisible by the tile
size. Add overlap between adjacent tiles for seamless stitching:

```python
for tile in tiles("large_image.tif", tile_size=(256, 256), overlap=(32, 32)):
    # stride = (224, 224), tiles share 32 pixels on each edge
    process(tile.data)
```

## Convenience vs Low-Level API

The convenience functions are a thin layer over the full API. Here is the same read
and write operation shown both ways, so you can see what the convenience layer handles
for you.

### Reading

**Convenience — one line:**

```python
from aws.osml.io import imread

pixels = imread("image.ntf")
```

**Equivalent low-level API:**

```python
from aws.osml.io import IO, AssetType
import numpy as np

with IO.open("image.ntf", "r") as dataset:
    keys = dataset.get_asset_keys(asset_type=AssetType.Image, roles=["data"])
    image = dataset.get_asset(keys[0])

    # Determine block dimensions
    bw = image.num_pixels_per_block_horizontal or image.num_columns
    bh = image.num_pixels_per_block_vertical or image.num_rows
    grid_rows, grid_cols = image.block_grid_size

    # Allocate output and assemble blocks
    dtype = np.dtype(image.pixel_value_type.to_numpy_dtype())
    pixels = np.zeros((image.num_bands, image.num_rows, image.num_columns), dtype=dtype)

    for r in range(grid_rows):
        for c in range(grid_cols):
            block = image.get_block(r, c, resolution_level=0)
            y0, x0 = r * bh, c * bw
            pixels[:, y0:y0 + block.shape[1], x0:x0 + block.shape[2]] = block
```

### Writing

**Convenience — one line:**

```python
from aws.osml.io import imsave
import numpy as np

data = np.random.randint(0, 255, (3, 512, 512), dtype=np.uint8)
imsave("output.tif", data)
```

**Equivalent low-level API:**

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
from aws.osml.io.tiff.utils import TagNameResolver
import numpy as np

data = np.random.randint(0, 255, (3, 512, 512), dtype=np.uint8)

# Set up encoding hints
metadata = BufferedMetadataProvider()
tag_dict = metadata.as_dict()
resolver = TagNameResolver(tag_dict)
resolver["Compression"] = "Deflate"
resolver["TileWidth"] = 256
resolver["TileLength"] = 256
resolver["Predictor"] = 2
for key, value in tag_dict.items():
    if isinstance(value, str):
        metadata.set(key, value)
    else:
        metadata.set_json(key, value)

# Create the image asset
provider = BufferedImageAssetProvider.create(
    key="image_0",
    num_columns=512, num_rows=512, num_bands=3,
    block_width=256, block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)
provider.set_full_image(data)

# Write to disk
with IO.open("output.tif", "w", "geotiff") as writer:
    writer.add_asset("image_0", provider)
```

## Beyond the Basics

The convenience functions handle the most common cases. When you need fine-grained
control — custom metadata, block-level access, compression tuning, multi-asset
datasets — the full low-level API is always available.

- **Dataset and asset model** — understand how files are organized into typed, keyed
  assets: [Datasets and the IO Interface](datasets-and-io.md)
- **Block-level pixel access** — read individual blocks, work with block grids and
  sparse images: [Image Assets](image-assets.md)
- **Metadata** — inspect and manipulate file-level and image-level metadata, TREs,
  GeoKeys: [Metadata](metadata.md)
- **Writing imagery** — full control over compression, georeferencing, encoding hints,
  and multi-file pyramids: [Writing Imagery Assets](image-assets-writing.md)
