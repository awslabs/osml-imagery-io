# Image Assets and Block-Level Access

## Tiled Images

Large geospatial images are stored as tiled images. Rather than storing pixel data as
one continuous stream, the image is divided into a regular grid of fixed-size rectangular
regions called blocks (or tiles). Each block is addressed by its `(row, col)` position
in the grid. This lets you read small regions efficiently without loading the entire
image into memory.

```{image} /_static/images/block-grid.png
:alt: Block grid diagram showing a 4x3 grid of blocks addressed by row and column, with two masked blocks
:width: 600px
```

Both NITF and TIFF use this approach. In NITF, the image subheader defines the block
dimensions (NPPBH × NPPBV) and the block grid size (NBPR × NBPC). In TIFF, the
TileWidth and TileLength tags serve the same purpose. Stripped TIFFs are also supported
— strips are treated as full-width blocks stacked vertically. When the image dimensions
aren't evenly divisible by the block size, edge blocks along the right and bottom 
boundaries may be smaller than the nominal block size.

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    image = dataset.get_asset("image:0")

    bands, height, width = image.image_shape
    _, block_h, block_w = image.block_shape
    grid_rows, grid_cols = image.block_grid_size

    print(f"Image: {width}x{height}, {bands} bands")
    print(f"Blocks: {block_w}x{block_h}, grid {grid_cols}x{grid_rows}")
```

Some formats, notably NITF, also support masked (sparse) images where not every position
in the block grid contains data. The hatched cells in the diagram above represent masked
blocks — regions with no pixel data. A block mask table in the file identifies which
blocks are present, allowing empty blocks to be omitted entirely.

## Iterating Over Blocks

Use `has_block()` to check whether a block contains data before reading it. This lets
you skip masked regions and handle sparse imagery gracefully:

```python
grid_rows, grid_cols = image.block_grid_size

valid = []
masked = []

for row in range(grid_rows):
    for col in range(grid_cols):
        if image.has_block(row, col, resolution_level=0):
            block = image.get_block(row, col, resolution_level=0)
            valid.append((row, col))
            # Process block...
        else:
            masked.append((row, col))

print(f"Valid: {len(valid)}, Masked: {len(masked)}")
```

## Reading Blocks

`get_block()` returns a NumPy array with shape `(bands, rows, cols)`. Geospatial images
often carry more information than a simple RGB photograph. A panchromatic image has a
single band of grayscale intensity. An RGB image has three bands (red, green, blue).
Multispectral sensors capture additional bands beyond visible light — near-infrared
(NIR), short-wave infrared (SWIR), and others — typically 3 to 10 bands, each covering
a different wavelength range. Hyperspectral sensors push this further with hundreds of
narrow contiguous bands spanning the electromagnetic spectrum. SAR (Synthetic Aperture
Radar) imagery may store complex-valued data as separate magnitude/phase or in-phase/
quadrature (I/Q) band pairs.

The `bands` parameter lets you select which channels to decode. By default all bands are
returned, but you can pass a list of zero-based band indices to retrieve only the ones
you need. This is useful for extracting a natural-color composite from a multispectral
image, isolating a single spectral band for analysis, or reading just the magnitude
channel from a SAR product.

```python
# All bands at full resolution
block = image.get_block(0, 0, resolution_level=0)

# Natural color from a multispectral image (bands 3, 2, 1 = R, G, B)
rgb = image.get_block(0, 0, resolution_level=0, bands=[3, 2, 1])

# Near-infrared band for vegetation analysis
nir = image.get_block(0, 0, resolution_level=0, bands=[4])
```

The `resolution_level` parameter controls the decode resolution. Some compression
schemes, notably JPEG 2000, encode each block's data at multiple resolution levels.
This is a property of the compressed codestream within each block, not a separate set
of overview images. The block grid stays the same — each block simply contains fewer
pixels at higher level numbers.

| Level | Scale | Example (2048×2048 block) |
|-------|-------|---------------------------|
| 0 | 1:1 | 2048×2048 |
| 1 | 1:2 | 1024×1024 |
| 2 | 1:4 | 512×512 |
| 3 | 1:8 | 256×256 |

Uncompressed images and images using JPEG DCT compression have only one resolution
level (level 0). Check available levels with `image.num_resolution_levels`.

```python
# Iterate over all available resolution levels for a block
for level in range(image.num_resolution_levels):
    block = image.get_block(0, 0, resolution_level=level)
    bands, rows, cols = block.shape
    print(f"Level {level}: {cols}x{rows} ({bands} bands)")
```

## Cloud Optimized GeoTIFF (COG) Overviews

Cloud Optimized GeoTIFFs store reduced-resolution overview images as additional IFDs
alongside the full-resolution image. These overviews are pre-computed downsampled
versions of the primary image, useful for quick previews and multi-scale visualization
without decoding the full-resolution data.

The library exposes each overview IFD as a separate `ImageAssetProvider` with its own
key and role. Overview keys follow the pattern `image:{parent}:overview:{level}`, where
`{parent}` is the index of the full-resolution image and `{level}` is a one-based
overview index (1 = largest overview, 2 = next smaller, etc.):

```python
from aws.osml.io import IO

with IO.open(["cog.tif"], "r") as dataset:
    # List all image assets (full-res + overviews)
    all_keys = dataset.get_asset_keys(asset_type="image")
    # ['image:0', 'image:0:overview:1', 'image:0:overview:2']

    # Use roles to get only the full-resolution images
    data_keys = dataset.get_asset_keys(asset_type="image", roles=["data"])
    # ['image:0']

    # Use roles to get only the overviews
    overview_keys = dataset.get_asset_keys(asset_type="image", roles=["overview"])
    # ['image:0:overview:1', 'image:0:overview:2']
```

Each overview is a fully functional `ImageAssetProvider` — you can read blocks, check
dimensions, and access metadata just like a full-resolution image:

```python
    # Compare dimensions across levels
    for key in all_keys:
        asset = dataset.get_asset(key)
        print(f"{key}: {asset.num_columns}x{asset.num_rows}, roles={asset.roles}")
    # image:0: 4096x4096, roles=['data']
    # image:0:overview:1: 2048x2048, roles=['overview']
    # image:0:overview:2: 1024x1024, roles=['overview']

    # Read a block from an overview
    overview = dataset.get_asset("image:0:overview:1")
    block = overview.get_block(0, 0, resolution_level=0)
```

This is different from the `resolution_level` parameter on `get_block()`. Block-level
resolution levels are a JPEG 2000 decompression feature that produces smaller versions
of the same block. COG overviews are separate images with their own tile grids and
dimensions. The two mechanisms are complementary — a COG overview that uses JPEG 2000
compression could itself support multiple block-level resolution levels.

### Single-IFD Edge Case

If a TIFF file contains only one IFD and that IFD has `NewSubfileType` bit 0 set
(indicating a reduced-resolution image), the reader treats it as the primary image
with key `image:0` and role `data`. A single-IFD file is always the primary image
regardless of its `NewSubfileType` flag.
