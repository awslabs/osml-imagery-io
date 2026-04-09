# Metadata

## MetadataProvider

```{eval-rst}
.. autoclass:: aws.osml.io.MetadataProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## BufferedMetadataProvider

```{eval-rst}
.. autoclass:: aws.osml.io.BufferedMetadataProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## TIFF Tag Dictionary Key Format

For TIFF/GeoTIFF files, the dictionary returned by `as_dict()` uses numeric
tag ID strings as keys. Each key is the string representation of the TIFF tag
number as defined in the TIFF 6.0 specification. For example, `ImageWidth`
(tag 256) appears under the key `"256"`, and `Compression` (tag 259) appears
under `"259"`.

This applies to all IFD-level tags, including GeoTIFF tags such as
`GeoKeyDirectory` (tag 34735), `ModelPixelScale` (tag 33550), and private-use
tags (32768+). GeoKey directory contents are not decoded into separate entries;
the raw TIFF tags are stored as-is under their numeric keys.

Dataset-level entries that are not TIFF tags (e.g. `"ByteOrder"`,
`"NumberOfDirectories"`) retain descriptive string keys.

```python
from aws.osml.io import IO

with IO.open(["image.tif"], "r") as dataset:
    meta = dataset.metadata.as_dict()

    width = meta["256"]          # ImageWidth
    height = meta["257"]         # ImageLength
    compression = meta["259"]    # Compression
    byte_order = meta["ByteOrder"]  # dataset-level, not a tag

    # Prefix filtering works on numeric keys
    tags_starting_with_3 = dataset.metadata.as_dict("3")
    # Returns keys like "322" (TileWidth), "339" (SampleFormat),
    # "34735" (GeoKeyDirectory), etc.
```

For convenient name-based access, use the `TagNameResolver` helper described
below.

## TagNameResolver

```{eval-rst}
.. autoclass:: aws.osml.io.tiff.utils.TagNameResolver
   :members:
   :undoc-members:
   :show-inheritance:
   :special-members: __getitem__, __contains__, __iter__, __len__
```

The `TagNameResolver` wraps a TIFF Tag_Dictionary and translates human-readable
tag names to their numeric keys. It ships with a default mapping covering
baseline TIFF 6.0 tags, GeoTIFF tags, and common GDAL tags.

```python
from aws.osml.io import IO
from aws.osml.io.tiff.utils import TagNameResolver

with IO.open(["image.tif"], "r") as dataset:
    meta = dataset.metadata.as_dict()
    tags = TagNameResolver(meta)

    # Name-based lookup
    width = tags["ImageWidth"]        # equivalent to meta["256"]
    scale = tags["ModelPixelScale"]   # equivalent to meta["33550"]

    # Safe access with default
    nodata = tags.get("GDALNoData", "nan")

    # Direct numeric access
    raw_geokeys = tags.by_number(34735)

    # Check presence
    if "Compression" in tags:
        print(tags["Compression"])

    # Custom mapping for vendor-specific tags
    custom = TagNameResolver(meta, custom_mapping={
        "MyVendorTag": 65000,
    })
    vendor_val = custom["MyVendorTag"]
```
