# Datasets and the IO Interface

## Opening a Dataset

The `IO` class is the entry point for reading and writing imagery files. It auto-detects
the format (NITF, TIFF/GeoTIFF, PNG, etc.) and returns a `DatasetReader` or `DatasetWriter`:

```python
from aws.osml.io import IO

# Read mode — returns a DatasetReader (format auto-detected)
with IO.open(["image.ntf"], "r") as dataset:
    print(type(dataset))  # DatasetReader

with IO.open(["image.tif"], "r") as dataset:
    print(type(dataset))  # DatasetReader

with IO.open(["image.png"], "r") as dataset:
    print(type(dataset))  # DatasetReader

# Write mode — returns a DatasetWriter
with IO.open(["output.ntf"], "w", "nitf") as writer:
    print(type(writer))  # DatasetWriter

with IO.open(["output.tif"], "w", "geotiff") as writer:
    print(type(writer))  # DatasetWriter

with IO.open(["output.png"], "w", "png") as writer:
    print(type(writer))  # DatasetWriter
```

Use the context manager (`with`) to ensure file handles are released when you're done.

## Dataset Structure

The dataset model in this library is inspired by the
[SpatioTemporal Asset Catalog (STAC)](https://stacspec.org/en) specification. STAC defines
a common structure for describing and cataloging geospatial assets — any file that
represents information about the Earth captured at a certain place and time. The core
building block in STAC is the **Item**, a GeoJSON feature that groups one or more related
**Assets** (the actual data files) together with shared metadata such as spatial extent,
temporal range, and provenance.

This library adopts the same conceptual model: a single `Dataset` maps to a STAC Item and
may contain multiple named assets. Just as a STAC Item for a satellite scene might include
separate assets for each spectral band, a thumbnail, a metadata sidecar, and ML-derived
annotations, a `Dataset` opened by this library can contain multiple images, structured
data payloads (e.g. SICD/SIDD XML), text reports, and vector graphic overlays — all
accessed through a uniform interface. The key insight is that real-world geospatial
products are rarely a single file; they are bundles of related assets that share a common
spatial and temporal context.

By aligning with the STAC data model, datasets produced or consumed by this library are
straightforward to publish as STAC Items and integrate with the broader STAC ecosystem of
catalogs, search APIs, and tooling. The library does not implement the STAC JSON format
itself, but the structural alignment means the mapping between an in-memory `Dataset` and
a STAC Item is direct: each asset key corresponds to a STAC Asset entry, asset types map
to STAC roles, and dataset-level metadata carries the information needed to populate Item
properties.

Each asset within a dataset has a type and a key that uniquely identifies it:

| Asset Type | Description | Examples |
|------------|-------------|---------|
| `image` | Raster imagery with blocked access | Satellite photos, SAR data |
| `data` | Structured data payloads | SICD/SIDD XML, overflow TREs |
| `text` | Plain text content | Mission reports, annotations |
| `graphics` | Vector graphics | CGM overlays |

### Asset Roles

Every asset also carries one or more semantic roles that describe its purpose. Roles
are aligned with the [STAC asset roles](https://github.com/radiantearth/stac-spec/blob/master/best-practices.md#asset-roles)
convention — short strings that communicate what an asset is for, independent of the
underlying file format.

| Role | Meaning | Assigned To |
|------|---------|-------------|
| `data` | Full-resolution image data | TIFF full-res IFDs, NITF image segments, JPEG, PNG |
| `overview` | Reduced-resolution image | COG overview IFDs, multi-file R-set images |
| `metadata` | Metadata asset | NITF text segments, data extension segments |
| `graphic` | Graphic/annotation overlay | NITF graphic segments |

Roles are the primary way to distinguish between different kinds of assets without
parsing key strings. See [Image Pyramids](#image-pyramids) below for how roles are
used to separate full-resolution images from reduced-resolution overviews.

## Image Pyramids

An image pyramid is a set of representations of the same image at progressively lower
resolutions. Pyramids enable efficient multi-scale access — a viewer can load a
low-resolution overview for navigation and fetch full-resolution tiles only for the
region of interest.

There are three ways multi-resolution data can be represented in geospatial imagery:

1. **Block-level resolution levels** — A single image whose compressed blocks can be
   decoded at multiple resolutions (e.g. JPEG 2000 wavelet decomposition). The block
   grid stays the same; each block just produces fewer pixels at higher level numbers.
   See [Reading Blocks](image-assets.md#reading-blocks) for details.

2. **Embedded overviews** — A single file containing multiple images at different
   resolutions. Cloud Optimized GeoTIFFs (COGs) store reduced-resolution overview
   images as additional IFDs alongside the full-resolution image.

3. **Multi-file pyramids** — Separate files for each resolution level. NITF R-sets
   are a common example: `image.ntf` is the full resolution, `image.ntf.r1` through
   `image.ntf.rN` are progressively reduced overviews.

This library exposes cases 2 and 3 through the same uniform interface: each resolution
level becomes a separate image asset with its own key and role. The full-resolution
image has role `data`, and each overview has role `overview`.

### Overview Asset Keys

Overview keys follow the pattern `image:{parent}:overview:{level}`, where `{parent}`
is the index of the full-resolution image and `{level}` is the overview number:

```python
from aws.osml.io import IO

with IO.open(["cog.tif"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="image"):
        asset = dataset.get_asset(key)
        print(f"{key}: {asset.num_columns}x{asset.num_rows}, roles={asset.roles}")
    # image:0: 4096x4096, roles=['data']
    # image:0:overview:1: 2048x2048, roles=['overview']
    # image:0:overview:2: 1024x1024, roles=['overview']
```

Each overview is a fully functional image asset — you can read blocks, check dimensions,
and access metadata just like a full-resolution image:

```python
    # Use roles to separate full-res from overviews
    data_keys = dataset.get_asset_keys(asset_type="image", roles=["data"])
    overview_keys = dataset.get_asset_keys(asset_type="image", roles=["overview"])

    # Read a block from an overview
    overview = dataset.get_asset("image:0:overview:1")
    block = overview.get_block(0, 0, resolution_level=0)
```

This is different from the `resolution_level` parameter on `get_block()`. Block-level
resolution levels are a decompression feature that produces smaller versions of the
same block. Overview assets are separate images with their own tile grids and
dimensions. The two mechanisms are complementary — an overview image that uses JPEG
2000 compression could itself support multiple block-level resolution levels.

### Multi-File Pyramids

When a dataset spans multiple files at different resolutions, pass all files to
`IO.open()` as a list. The library detects the R-set naming convention (`.rN` suffix)
and exposes each file as an overview asset, producing the same key and role structure
as embedded overviews:

```python
with IO.open(["image.ntf", "image.ntf.r1", "image.ntf.r2"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="image"):
        asset = dataset.get_asset(key)
        print(f"{key}: {asset.num_columns}x{asset.num_rows}, roles={asset.roles}")
    # image:0: 4096x4096, roles=['data']
    # image:0:overview:1: 2048x2048, roles=['overview']
    # image:0:overview:2: 1024x1024, roles=['overview']
```

The first path is always the full-resolution base image. The overview level is
extracted from the filename, not inferred from list order — these two calls produce
identical results:

```python
IO.open(["image.ntf", "image.ntf.r1", "image.ntf.r2"], "r")
IO.open(["image.ntf", "image.ntf.r2", "image.ntf.r1"], "r")
```

R-set detection is format-agnostic. Each file in the list is opened with its own
auto-detected format reader, so the convention works for any supported format.

:::{note}
R-sets are a de facto industry convention used by commercial data providers. They are
not part of the JBP/NITF specification — there is no internal metadata linking an
R-set file to its parent. The relationship is purely by filename convention.
:::

Some things to keep in mind with multi-file pyramids:

- The caller must provide the full list of paths explicitly. `IO.open()` does not
  scan the filesystem for sibling `.rN` files.
- When only one path is provided, behavior is identical to the single-file case.
- R-set overviews are associated with `image:0` (the primary image segment). If the
  base file contains multiple image segments, the R-sets apply to the primary image
  only.

## Discovering Assets

Use `get_asset_keys()` to list available assets, then `get_asset()` to retrieve
a specific one. You can filter by asset type, by role, or both:

```python
from aws.osml.io import IO

with IO.open(["complex_dataset.ntf"], "r") as dataset:
    # List keys by asset type
    image_keys = dataset.get_asset_keys(asset_type="image")
    text_keys = dataset.get_asset_keys(asset_type="text")
    data_keys = dataset.get_asset_keys(asset_type="data")
    graphics_keys = dataset.get_asset_keys(asset_type="graphics")

    print(f"Images: {len(image_keys)}, Text: {len(text_keys)}, "
          f"Data: {len(data_keys)}, Graphics: {len(graphics_keys)}")

    # Retrieve a specific asset
    image = dataset.get_asset("image:0")
```

### Filtering by Role

The `roles` parameter on `get_asset_keys()` lets you filter assets by their semantic
purpose. This is useful when a dataset contains both full-resolution images and
overviews:

```python
with IO.open(["cog.tif"], "r") as dataset:
    # Only full-resolution images
    data_keys = dataset.get_asset_keys(asset_type="image", roles=["data"])

    # Only overview images
    overview_keys = dataset.get_asset_keys(asset_type="image", roles=["overview"])

    # All image assets (no role filter)
    all_keys = dataset.get_asset_keys(asset_type="image")
```

When `roles` is omitted or `None`, all assets matching the `asset_type` filter are
returned. When both `asset_type` and `roles` are provided, both filters apply — only
assets that match the type and have at least one of the requested roles are returned.

NITF files can contain all four asset types. TIFF files contain only image assets —
each IFD (Image File Directory) in the file becomes a separate image asset keyed as
`"image:0"`, `"image:1"`, etc. Cloud Optimized GeoTIFFs additionally expose overview
IFDs as `"image:0:overview:1"`, `"image:0:overview:2"`, etc. PNG files contain a
single image keyed as `"image:0"`. Text, data, and graphics asset queries will return
empty lists for TIFF and PNG datasets.

## Dataset-Level Metadata

Every dataset exposes a `metadata` property with file-level fields. See the
[Metadata](metadata.md) section for details:

```python
with IO.open(["image.ntf"], "r") as dataset:
    file_metadata = dataset.metadata.as_dict()
```
