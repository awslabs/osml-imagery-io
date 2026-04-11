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
| `overview` | Reduced-resolution image | TIFF overview IFDs (COG) |
| `metadata` | Metadata asset | NITF text segments, data extension segments |
| `graphic` | Graphic/annotation overlay | NITF graphic segments |

Roles are the primary way to distinguish between different kinds of assets without
parsing key strings. A Cloud Optimized GeoTIFF (COG) might contain a full-resolution
image and two overview images — all three are `image` type assets, but their roles
tell you which is the primary data and which are reduced-resolution versions:

```python
with IO.open(["cog.tif"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="image"):
        asset = dataset.get_asset(key)
        print(f"{key}: roles={asset.roles}")
    # image:0: roles=['data']
    # image:0:overview:1: roles=['overview']
    # image:0:overview:2: roles=['overview']
```

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
purpose. This is particularly useful for COG files where you want to separate
full-resolution images from overviews:

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
