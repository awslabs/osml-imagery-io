# Datasets and the IO Interface

## Opening a Dataset

The `IO` class is the entry point for reading and writing imagery files. It auto-detects
the format (NITF, TIFF/GeoTIFF, etc.) and returns a `DatasetReader` or `DatasetWriter`:

```python
from aws.osml.io import IO

# Read mode — returns a DatasetReader (format auto-detected)
with IO.open(["image.ntf"], "r") as dataset:
    print(type(dataset))  # DatasetReader

with IO.open(["image.tif"], "r") as dataset:
    print(type(dataset))  # DatasetReader

# Write mode — returns a DatasetWriter
with IO.open(["output.ntf"], "w", "nitf") as writer:
    print(type(writer))  # DatasetWriter

with IO.open(["output.tif"], "w", "geotiff") as writer:
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

## Discovering Assets

Use `get_asset_keys()` to list available assets by type, then `get_asset()` to retrieve
a specific one:

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
    image = dataset.get_asset("image_segment_0")
```

NITF files can contain all four asset types. TIFF files contain only image assets —
each IFD (Image File Directory) in the file becomes a separate image asset keyed as
`"image_segment_0"`, `"image_segment_1"`, etc. Text, data, and graphics asset queries
will return empty lists for TIFF datasets.
```

## Dataset-Level Metadata

Every dataset exposes a `metadata` property with file-level fields. See the
[Metadata](metadata.md) section for details:

```python
with IO.open(["image.ntf"], "r") as dataset:
    file_metadata = dataset.metadata.as_dict()
```
