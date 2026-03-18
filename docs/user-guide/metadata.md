# Metadata

## The MetadataProvider Interface

All assets and datasets expose metadata through the `MetadataProvider` interface. It
provides a uniform way to access key-value metadata regardless of the underlying format.

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    # Dataset-level metadata (e.g. NITF file header fields)
    all_meta = dataset.metadata.as_dict()

    # Filter by key prefix — returns all fields whose key starts with the prefix.
    # For a NITF file header, filtering by "FS" returns the file security fields:
    #   FSCLAS, FSCLSY, FSCODE, FSCTLH, FSREL, FSDCTP, FSDCDT, FSDCXM,
    #   FSDG, FSDGDT, FSCLTX, FSCATP, FSCAUT, FSCRSN, FSSRDT, FSCTLN
    security = dataset.metadata.as_dict("FS")

    # Asset-level metadata (e.g. NITF image subheader fields)
    image = dataset.get_asset("image_segment_0")
    image_meta = image.metadata.as_dict()
```

The `as_dict()` method returns all metadata as a Python dictionary. Pass an optional
prefix string to filter keys — useful for pulling out a group of related fields.

### Accessing Metadata Values

The dictionary returned by `as_dict()` maps string keys to native Python values.
The value types depend on how the field is defined in the underlying structure
definition:

- Fields defined as `type: str` (most NITF header fields) → Python `str`
- Fields defined as binary integers (`u1`, `u2`, `u4`, `u8`) → Python `int`
- Repeated fields → Python `list`
- Nested structures → Python `dict` with `_type` and `_data` keys
- Binary byte fields → Python `str` (hex-encoded if not valid UTF-8)

TODO: Check the previous list. Need to make sure the list and dict options actually
work as described. I thought repeated metadata fields used an _# notation and were flattened into the dict.

In NITF, most header and subheader fields are ASCII strings — even numeric
values like row counts and compression ratios. A few TREs use binary integer
fields (e.g. BANDSB's existence mask, NBLOCA's frame offsets), which come
through as Python `int` directly.

```python
image_meta = image.metadata.as_dict()

# String values — most NITF fields are ASCII strings
classification = image_meta["ISCLAS"]       # "U"
image_id = image_meta["IID1"]              # "IMG_00001"
compression = image_meta["IC"]             # "C8"

# Numeric fields stored as ASCII strings — use int() or float()
num_rows = int(image_meta["NROWS"])         # 2048
num_cols = int(image_meta["NCOLS"])         # 2048
num_bands = int(image_meta["NBANDS"])       # 3
bits_per_pixel = int(image_meta["NBPP"])    # 8

# Date/time strings — parse as needed
date_time = image_meta["IDATIM"]            # "20231215103045"
year = int(date_time[:4])                   # 2023

# Coordinate strings — NITF packs 4 corners into a single field
if "IGEOLO" in image_meta:
    geo = image_meta["IGEOLO"]              # 60-char geographic location string

# Safe access for optional or conditional fields
comrat = image_meta.get("COMRAT")           # None if IC is "NC" or "NM"

# Binary integer fields (some TREs) are already Python int
# e.g. BANDSB existence mask, NBLOCA frame offsets
```

For lower-level access with built-in type conversion, the `StructureAccessor`
returns `Value` objects with `as_str()`, `as_int()`, and `as_float()` methods
that handle NITF's ASCII-numeric conventions (e.g. parsing `"003"` as `3`).
See [Flexible Parsing with Structure Definitions](#flexible-parsing-with-structure-definitions)
below.

## Metadata Varies by Format

Geospatial imagery formats carry very different metadata. A NITF file includes
structured header fields, security markings, and Tagged Record Extensions (TREs)
that describe everything from sensor parameters to geolocation models. A GeoTIFF
carries TIFF tags, GeoKeys, and optional XML metadata. Other formats have their
own conventions.

This library exposes all format-specific metadata through the same `as_dict()`
dictionary interface. You get the native field names from whatever format you
opened — no translation or normalization layer sits between you and the data.

For NITF files, TREs appear transparently through `as_dict()`. Overflow TREs
stored in data extension segments are resolved automatically — you don't need to
chase them across segments. For TIFF and GeoTIFF files, TIFF tags and GeoKeys are
surfaced through the same interface using numeric tag IDs as keys.

The tradeoff is that field names and semantics are format-specific. If you write
code that reads `IGEOLO` from a NITF image, that same key won't exist in a
GeoTIFF. Plan for this when working across formats — check for key existence or
use `as_dict()` with a prefix to discover what's available.

## TIFF and GeoTIFF Metadata

For TIFF and GeoTIFF files, the metadata dictionary uses numeric TIFF tag IDs
as keys. Each key is the string representation of the tag number from the
TIFF 6.0 specification — for example, `"256"` for ImageWidth, `"259"` for
Compression, `"33550"` for ModelPixelScale.

This design means every tag in the IFD is preserved, including private-use
tags (32768+) and vendor-specific tags that would otherwise be dropped by a
hardcoded name list. The raw tag values are stored directly, with no
interpretation or transformation applied.

### Reading TIFF Metadata

```python
from aws.osml.io import IO

with IO.open(["image.tif"], "r") as dataset:
    meta = dataset.metadata.as_dict()

    # Tags are keyed by their numeric ID as a string
    width = meta["256"]           # ImageWidth
    height = meta["257"]          # ImageLength
    bits = meta["258"]            # BitsPerSample
    compression = meta["259"]     # Compression

    # GeoTIFF tags use the same numeric key convention
    pixel_scale = meta["33550"]   # ModelPixelScale — e.g. [0.5, 0.5, 0.0]
    tiepoints = meta["33922"]     # ModelTiepoint
    geokeys = meta["34735"]       # GeoKeyDirectory (raw SHORT array)

    # Dataset-level entries use descriptive string keys
    byte_order = meta["ByteOrder"]              # "LittleEndian"
    num_dirs = meta["NumberOfDirectories"]       # 3

    # Prefix filtering works on the numeric key strings
    tags_3xx = dataset.metadata.as_dict("3")
    # Returns "322" (TileWidth), "323" (TileLength), "339" (SampleFormat),
    # "33550" (ModelPixelScale), "34735" (GeoKeyDirectory), etc.
```

### Using TagNameResolver for Name-Based Access

If you prefer human-readable tag names, wrap the dictionary with
`TagNameResolver`. It translates names like `"ImageWidth"` to the
corresponding numeric key (`"256"`) behind the scenes.

```python
from aws.osml.io import IO
from aws.osml.io.tiff import TagNameResolver

with IO.open(["image.tif"], "r") as dataset:
    meta = dataset.metadata.as_dict()
    tags = TagNameResolver(meta)

    # Look up by name — same value as meta["256"]
    width = tags["ImageWidth"]
    height = tags["ImageLength"]

    # GeoTIFF tags work the same way
    scale = tags["ModelPixelScale"]
    geokeys = tags["GeoKeyDirectory"]

    # Safe access with a default value
    nodata = tags.get("GDALNoData", "nan")

    # Direct numeric access when you know the tag number
    raw = tags.by_number(34735)

    # Check if a tag is present
    if "Compression" in tags:
        print(f"Compression: {tags['Compression']}")

    # Iterate over all entries
    for key, value in tags:
        print(f"Tag {key}: {value}")
```

The resolver ships with a default mapping covering baseline TIFF 6.0 tags,
GeoTIFF tags, and common GDAL tags. You can extend it with custom mappings
for vendor-specific or application-specific tags:

```python
custom_tags = TagNameResolver(meta, custom_mapping={
    "MyVendorTag": 65000,
    "CloudCover": 65001,
})

vendor_val = custom_tags["MyVendorTag"]
cloud = custom_tags["CloudCover"]

# Custom mappings override defaults if there's a name collision
```

### Writing TIFF Metadata

When writing TIFF files, supply metadata using the same numeric key format.
The writer infers the TIFF field type from the JSON value type for common
cases. For types that can't be inferred, use an explicit type annotation.

```python
from aws.osml.io import IO

metadata = {
    "256": 512,                    # ImageWidth → inferred as LONG
    "257": 512,                    # ImageLength → inferred as LONG
    "259": 1,                      # Compression → inferred as LONG
    "33550": [0.5, 0.5, 0.0],     # ModelPixelScale → inferred as DOUBLE array
    "42113": "nan",                # GDALNoData → inferred as ASCII
}

# For field types that can't be inferred (e.g. UNDEFINED), use an annotation:
metadata["700"] = {"value": [60, 120, 109, 108], "type": 7}  # XMP as UNDEFINED bytes

with IO.open(["output.tif"], "w") as writer:
    writer.metadata = metadata
    # ... write image data
```

## Flexible Parsing with Structure Definitions

The metadata you see through `as_dict()` is driven by a data-driven parsing
framework under the hood. The library uses declarative YAML-based structure
definition files (`.ksy` format, inspired by [Kaitai Struct](https://kaitai.io/))
to describe binary layouts. These definitions control both reading and writing —
the same file that tells the parser how to extract fields from a binary header
also tells the writer how to serialize them back.

This means you can extend the metadata the library understands by adding new
structure definition files. If a TRE, DES, or other metadata structure isn't
already supported, you can write a `.ksy` definition for it and register it
with the `StructureRegistry`.

### Configuring the StructureRegistry

The `StructureRegistry` manages all structure definitions. By default it loads
definitions from the package's built-in `data/structures/` directory, which
includes NITF file headers, image subheaders, and many common TREs. You can
extend it with your own definitions:

```python
from aws.osml.io import StructureRegistry

# Create a registry (loads built-in definitions automatically)
registry = StructureRegistry()

# See what's already available
for name in registry.list():
    print(name)
# NITF_02.10_FileHeader, NITF_02.10_ImageSubheader, TRE_GEOLOB,
# TRE_RPC00B, TRE_SENSRB, TRE_USE00A, ... (70+ definitions)

# Add a directory containing your custom .ksy files
registry.add_search_path("/path/to/my/structures")

# Retrieve a specific definition
geolob_def = registry.get("TRE_GEOLOB")

# Reload definitions after editing .ksy files on disk
registry.reload()
```

You can also set the `OSML_IO_STRUCTURE_PATH` environment variable to add
search paths without changing code. Separate multiple paths with `:`.

```bash
export OSML_IO_STRUCTURE_PATH="/team/shared/structures:/project/custom/structures"
```

### How Definitions Are Used

Structure definitions drive both directions of the pipeline:

- When reading, the parser uses the definition to locate fields in the binary
  data, apply the correct encoding (BCS-A, BCS-N, etc.), evaluate conditional
  and repeated fields, and populate the metadata dictionary.

- When writing, the writer uses the same definition to serialize metadata
  values back into the correct binary layout, validating field sizes and
  encodings along the way.

Adding a new `.ksy` file for a TRE automatically enables both reading and
writing that TRE — no code changes required.

### Writing Your Own Structure Definitions

Structure definition files use a YAML-based format with support for field types,
conditional presence, repeat expressions, and nested structures. For the full
syntax reference, expression language details, and examples, see the
[Structure Definition Guide](structure-definitions.md).
