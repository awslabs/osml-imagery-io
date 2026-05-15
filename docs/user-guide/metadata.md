# Metadata

## The Simple Path

For a quick look at image properties without reading any pixels, `iminfo` gives
you dimensions, band count, pixel type, and block layout in one call:

```python
from aws.osml.io import iminfo

info = iminfo("image.ntf")
print(f"{info.width}x{info.height}, {info.bands} bands, {info.dtype}")
print(f"Block size: {info.block_size}")
print(f"Resolution levels: {info.num_resolution_levels}")
```

`iminfo` also includes the full format-specific metadata dictionary for the
image segment, so you can inspect compression, TREs, TIFF tags, and other
fields without dropping down to the low-level API:

```python
# NITF: subheader fields and TREs
info = iminfo("image.ntf")
print(info.metadata["IC"])          # "C8" (JPEG 2000)
print(info.metadata["IGEOLO"])      # 60-char geographic location string

# TREs are nested dicts
if "GEOLOB" in info.metadata:
    print(info.metadata["GEOLOB"]["ARV"])

# GeoTIFF: IFD tags keyed by numeric tag ID
info = iminfo("image.tif")
print(info.metadata["259"])         # Compression tag value
print(info.metadata["33550"])       # ModelPixelScale
```

The `metadata` dict is a snapshot — a plain Python dictionary captured when
`iminfo` is called. It is not a live reference to the file.

When you need more control — prefix filtering, dataset-level metadata, or
write-side metadata — the full `MetadataProvider` interface described below
gives you access to everything in the file.

## The MetadataProvider Interface

All assets and datasets expose metadata through the `MetadataProvider` interface,
regardless of the underlying file format. `MetadataProvider` implements the standard
Python `collections.abc.Mapping` protocol, so you access metadata fields the same
way you access items in a dictionary:

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    # Dataset-level metadata — dict-style access
    dataset.metadata["FTITLE"]              # KeyError if missing
    dataset.metadata.get("FTITLE")          # None if missing
    "FTITLE" in dataset.metadata            # membership test
    len(dataset.metadata)                   # number of fields

    # Bulk export
    all_meta = dataset.metadata.entries()   # full dict (single call, fast path)
    filtered = dataset.metadata.entries("FS")  # keys starting with "FS"

    # Asset-level metadata
    image = dataset.get_asset("image:0")
    image_meta = image.metadata.entries()
```

When writing, use `BufferedMetadataProvider` to build metadata. It implements
`collections.abc.MutableMapping`, so you write fields with dictionary syntax:

```python
from aws.osml.io import BufferedMetadataProvider

meta = BufferedMetadataProvider()
meta["FTITLE"] = "My File Title"
```

The dictionary keys and value types are format-specific. A NITF file uses
field names like `FTITLE` and `ISCLAS`; a GeoTIFF uses numeric tag IDs like
`"256"` and `"33550"`. There is no translation layer — you work directly with
the native field names from whatever format you opened.

The rest of this page covers each format's metadata conventions in detail.

---

## NITF / NSIF Metadata

NITF files carry metadata in fixed-width ASCII header fields, security
classification blocks, and Tagged Record Extensions (TREs). The library
exposes all of these through the Mapping interface using the standard NITF field names.

### Reading NITF Metadata

#### Header and Subheader Fields

Most NITF fields are ASCII strings — even numeric values like row counts and
compression ratios. A few TREs use binary integer fields, which come through
as Python `int` directly.

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    # File header fields — dict-style access
    title = dataset.metadata["FTITLE"]
    classification = dataset.metadata["FSCLAS"]       # "U", "C", "S", "TS", etc.

    # Image subheader fields
    image = dataset.get_asset("image:0")
    meta = image.metadata

    image_id = meta["IID1"]              # "IMG_00001"
    compression = meta["IC"]             # "C8"
    date_time = meta["IDATIM"]           # "20231215103045"

    # Numeric fields are ASCII strings — cast as needed
    num_rows = int(meta["NROWS"])         # 2048
    num_cols = int(meta["NCOLS"])         # 2048
    num_bands = int(meta["NBANDS"])       # 3

    # Coordinate strings — NITF packs 4 corners into a single field
    if "IGEOLO" in meta:
        geo = meta["IGEOLO"]              # 60-char geographic location string

    # Safe access for conditional fields
    comrat = meta.get("COMRAT")           # None if IC is "NC" or "NM"
```

#### TRE Fields as Nested Dictionaries

TRE (Tagged Record Extension) fields are grouped under their CETAG as nested
dictionaries. Each TRE with a known definition in the `StructureRegistry`
appears as a top-level key mapped to a dict of its fields:

```python
# Access TRE fields through nested dictionaries
geolob = meta["GEOLOB"]              # dict
arv = geolob["ARV"]                  # "000360000"
brv = geolob["BRV"]                  # "000360000"

# Or access in one step
arv = meta["GEOLOB"]["ARV"]

# TREs with repeated fields contain arrays
j2klra = meta["J2KLRA"]              # dict
layers = j2klra["LAYERS"]            # list of dicts
first_layer = layers[0]              # {"LAYER_ID": "000", "BITRATE": "0.031250"}
```

Unknown TREs (those without a definition in the registry) appear with their
raw data preserved:

```python
# Unknown TRE — raw hex data and byte length
unknown = meta["UNKNWN"]             # {"_raw": "0102030405", "_length": 5}
raw_hex = unknown["_raw"]
byte_count = unknown["_length"]
```

Overflow TREs stored in data extension segments are resolved automatically —
you don't need to chase them across segments.

#### Repeated Fields as Arrays

Repeated fields in the image subheader (like band info) appear as Python lists
instead of individual indexed entries:

```python
# Band info is a list of dicts, one per band
bands = meta["BAND_INFO"]            # list of dicts
for i, band in enumerate(bands):
    print(f"Band {i}: IREPBAND={band['IREPBAND']}, NLUTS={band['NLUTS']}")

# Access a specific band directly
first_band = meta["BAND_INFO"][0]
irepband = first_band["IREPBAND"]          # "R"
```

#### Prefix Filtering

Use `entries(prefix)` to retrieve a subset of metadata. For subheader fields,
the prefix matches field names. For TREs, the prefix matches the CETAG:

```python
# Get all fields starting with "FS" (file security fields)
# Returns: FSCLAS, FSCLSY, FSCODE, FSCTLH, FSREL, FSDCTP, ...
security = dataset.metadata.entries("FS")

# Get a specific TRE by CETAG
geolob_only = image.metadata.entries("GEOLOB")
# Returns: {"GEOLOB": {"ARV": "...", "BRV": "...", ...}}
```

#### Value Types Summary

The Python types you get back depend on how the field is defined in the
underlying structure definition:

| Definition | Python type | Example |
|------------|-------------|---------|
| `type: str` (most fields) | `str` | `"U"`, `"00002048"` |
| Binary integers (`u1`, `u2`, `u4`, `u8`) | `int` | `42` |
| Repeated fields (band info, etc.) | `list` of `dict` | `[{"IREPBAND": "R", ...}]` |
| Known TREs | `dict` of `dict` | `{"GEOLOB": {"ARV": "..."}}` |
| Unknown TREs | `dict` with `_raw`, `_length` | `{"_raw": "0102", "_length": 2}` |
| Binary byte fields | `str` (hex-encoded) | `"ff8000"` |

### Writing NITF Metadata

When writing NITF files, you control header fields by setting metadata on the
writer and on individual assets. The writer reads user-settable fields from
the metadata provider and falls back to sensible defaults when a field is absent.

#### File Header Fields

Set file-level metadata using `BufferedMetadataProvider` and assign it to the
writer's `metadata` property:

```python
from aws.osml.io import IO, BufferedMetadataProvider

file_meta = BufferedMetadataProvider()
file_meta["FTITLE"] = "Reconnaissance Mission 2026-03-15"
file_meta["ONAME"] = "Sensor Operator"
file_meta["OPHONE"] = "555-0100"
file_meta["FDT"] = "20260315120000"
file_meta["OSTAID"] = "STATION1"
file_meta["CLEVEL"] = "05"

# Security classification fields use the FS prefix
file_meta["FSCLAS"] = "S"
file_meta["FSCLSY"] = "US"
file_meta["FSCODE"] = "SECRET"
file_meta["FSREL"] = "USA GBR"

# FBKGC is a 3-byte binary field (RGB background color)
# Set it as a list of integers
file_meta["FBKGC"] = [255, 255, 255]

writer = IO.open(["output.ntf"], "w", "nitf")
writer.metadata = file_meta
# ... add assets and close
```

Fields you don't set keep their defaults — `FSCLAS` defaults to `"U"`,
`OSTAID` defaults to `"OSML_IO"`, `CLEVEL` defaults to `"03"`, and text
fields default to blank.

#### Image Subheader Fields

Image assets read several fields from metadata (`IID1`, `IDATIM`, `TGTID`,
`IID2`, `ISORCE`). The security classification block and category fields are
also metadata-driven:

```python
image_meta = BufferedMetadataProvider()

# Identification fields
image_meta["IID1"] = "IMG_00001"
image_meta["IDATIM"] = "20260315103045"
image_meta["ISORCE"] = "Satellite XYZ"

# Security fields use the IS prefix
image_meta["ISCLAS"] = "S"
image_meta["ISCLSY"] = "US"
image_meta["ISREL"] = "USA"

# Image category and coordinate representation
image_meta["ICAT"] = "SAR"
image_meta["ICORDS"] = "G"
```

Fields derived from the image data itself — `NROWS`, `NCOLS`, `PVTYPE`,
`IREP`, `NBPP`, `ABPP`, `NBANDS`, and blocking parameters — are always
computed from the `ImageAssetProvider` and cannot be overridden through
metadata.

#### Text, Graphic, and DES Subheader Fields

Text, graphic, and data extension segment subheaders follow the same pattern.
Set fields on the asset's metadata provider before adding it to the writer:

```python
# Text asset metadata (TS prefix for security fields)
text_meta = BufferedMetadataProvider()
text_meta["TXTDT"] = "20260315120000"
text_meta["TXTFMT"] = "STA"
text_meta["TSCLAS"] = "C"

# Graphic asset metadata (SS prefix for security fields)
graphic_meta = BufferedMetadataProvider()
graphic_meta["SFMT"] = "C"
graphic_meta["SDLVL"] = "002"
graphic_meta["SLOC"] = "0050000100"
graphic_meta["SSCLAS"] = "U"

# DES metadata (DES prefix for security fields, but DECLAS for classification)
des_meta = BufferedMetadataProvider()
des_meta["DESVER"] = "02"
des_meta["DECLAS"] = "U"
des_meta["DESCLSY"] = "US"
```

#### Security Classification Fields

Every NITF subheader contains the same 13-field security classification block.
The field names use a prefix that varies by segment type:

| Segment | Prefix | Example |
|---------|--------|---------|
| File header | `FS` | `FSCLAS`, `FSCLSY`, `FSCODE`, … |
| Image | `IS` | `ISCLAS`, `ISCLSY`, `ISCODE`, … |
| Text | `TS` | `TSCLAS`, `TSCLSY`, `TSCODE`, … |
| Graphic | `SS` | `SSCLAS`, `SSCLSY`, `SSCODE`, … |
| DES | `DE`/`DES` | `DECLAS`, `DESCLSY`, `DESCODE`, … |

The 13 fields in each block (after the prefix) are: `CLAS`, `CLSY`, `CODE`,
`CTLH`, `REL`, `DCTP`, `DCDT`, `DCXM`, `DG`, `DGDT`, `CLTX`, `CATP`,
`CAUT`, `CRSN`, `SRDT`, `CTLN`.

All default to `"U"` for classification and blank for everything else.

#### Computed vs. User-Settable Fields

Some fields are always computed by the writer and cannot be overridden:

- `FHDR`, `FVER` — determined by the output format (NITF 2.1 / NSIF 1.0)
- `FL`, `HL` — computed from actual file and header lengths
- `NUMI`, `NUMS`, `NUMT`, `NUMDES`, `NUMRES` — segment counts
- Segment length arrays (`LISH`/`LI`, `LSSH`/`LS`, etc.)
- `ENCRYP` — always `"0"` (unencrypted)
- Image dimensions, pixel type, blocking parameters — derived from image data

#### Writing TREs

Set TREs as nested dicts using dictionary syntax, matching the format returned by
the reader:

```python
image_meta["GEOLOB"] = {
    "ARV": "000360000",
    "BRV": "000180000",
    "LSO": "-077.0000000000",
    "PSO": "+038.0000000000",
}
```

Numeric fields (BCS-N encoding) are auto-formatted to their defined width —
short values are left-padded with zeros and overly-precise values are
reformatted to fit. You can pass natural representations:

```python
image_meta["ICHIPB"] = {
    "OP_ROW_11": "0.5",     # auto-padded to "0000000000.5" (12 bytes)
    "FI_ROW": "768",        # auto-padded to "00000768" (8 bytes)
    # ...
}
```

Text fields (BCS-A) are right-padded with spaces if short and rejected if
too long. Values that cannot fit any field after formatting raise an error.

#### Encoding Tolerance

NITF fields declare a character encoding that constrains what bytes are valid.
For example, BCS-NPI (Numeric Positive Integer) only permits digits and spaces
per the JBP specification. In practice, real-world NITF producers frequently
violate these constraints — the RPC00B TRE's `HEIGHT_SCALE` field is commonly
written with a leading `+` sign despite being declared BCS-NPI.

By default, the writer uses **permissive** validation: numeric fields (BCS-N
and BCS-NPI) accept any printable ASCII character (the BCS-A range, 0x20–0x7E).
This ensures that metadata read from real-world files can be written back
without error — a round-trip that would otherwise fail on spec-violating values.

If you need output that is strictly compliant with the NITF encoding
specifications, enable strict mode on the writer:

```python
with IO.open("output.ntf", "w", "nitf") as writer:
    writer.strict_encoding = True  # reject values that violate declared encodings
    writer.add_asset("image:0", provider, "Image", "", ["data"])
```

In strict mode, writing `"+0697"` to a BCS-NPI field raises a validation error
because `+` is not in the BCS-NPI character set. This is useful when producing
files that must pass formal conformance checks.

### Extending NITF Metadata with Structure Definitions

The metadata you access from NITF files is driven by a
data-driven parsing framework. The library uses declarative YAML-based
structure definition files (`.ksy` format, inspired by
[Kaitai Struct](https://kaitai.io/)) to describe binary layouts. These
definitions control both reading and writing — the same file that tells the
parser how to extract fields from a binary header also tells the writer how
to serialize them back.

This means you can extend the metadata the library understands by adding new
structure definition files. If a TRE, DES, or other NITF metadata structure
isn't already supported, you can write a `.ksy` definition for it and register
it with the `StructureRegistry`.

#### Configuring the StructureRegistry

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

#### How Definitions Are Used

Structure definitions drive both directions of the pipeline:

- When reading, the parser uses the definition to locate fields in the binary
  data, apply the correct encoding (BCS-A, BCS-N, etc.), evaluate conditional
  and repeated fields, and populate the metadata dictionary.

- When writing, the writer uses the same definition to serialize metadata
  values back into the correct binary layout, validating field sizes and
  encodings along the way.

Adding a new `.ksy` file for a TRE automatically enables both reading and
writing that TRE — no code changes required.

#### Lower-Level Access with StructureAccessor

For lower-level access with built-in type conversion, the `StructureAccessor`
returns `Value` objects with `as_str()`, `as_int()`, and `as_float()` methods
that handle NITF's ASCII-numeric conventions (e.g. parsing `"003"` as `3`).

#### Writing Your Own Structure Definitions

Structure definition files use a YAML-based format with support for field types,
conditional presence, repeat expressions, and nested structures. For the full
syntax reference, expression language details, and examples, see the
[Structure Definition Guide](structure-definitions.md).

---

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
    meta = dataset.metadata

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
    tags_3xx = dataset.metadata.entries("3")
    # Returns "322" (TileWidth), "323" (TileLength), "339" (SampleFormat),
    # "33550" (ModelPixelScale), "34735" (GeoKeyDirectory), etc.
```

### Using TagNameResolver for Name-Based Access

If you prefer human-readable tag names, wrap the dictionary with
`TagNameResolver`. It translates names like `"ImageWidth"` to the
corresponding numeric key (`"256"`) behind the scenes.

```python
from aws.osml.io import IO
from aws.osml.io.tiff.utils import TagNameResolver

with IO.open(["image.tif"], "r") as dataset:
    meta = dataset.metadata.entries()
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
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

metadata = BufferedMetadataProvider()
metadata["259"] = 8                              # Compression: Deflate
metadata["33550"] = [0.5, 0.5, 0.0]             # ModelPixelScale → DOUBLE array
metadata["42113"] = "nan"                        # GDALNoData → ASCII

# For field types that can't be inferred (e.g. UNDEFINED), use an annotation:
metadata["700"] = {"value": [60, 120, 109, 108], "type": 7}  # XMP as UNDEFINED bytes

# Attach metadata to the provider — the writer sources all IFD tags from here
provider = BufferedImageAssetProvider.create(
    key="image:0", num_columns=512, num_rows=512, num_bands=1,
    block_width=256, block_height=256, pixel_type=PixelType.UInt8,
    metadata=metadata,
)
provider.set_full_image(image_data)

with IO.open(["output.tif"], "w", "tiff") as writer:
    writer.add_asset("image:0", provider, "Image", "desc", ["data"])
```

#### Writing with TagNameResolver

`TagNameResolver` is bidirectional — you can use it to build write metadata
with human-readable names instead of numeric tag IDs. Assign values with
`resolver["TagName"] = value` and the resolver stores them under the correct
numeric key in the underlying dictionary.

For tags with well-known enumerated values (Compression, Predictor,
PlanarConfiguration, SampleFormat, PhotometricInterpretation, Orientation),
string values are resolved to their numeric equivalents automatically:

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
from aws.osml.io.tiff.utils import TagNameResolver

metadata = BufferedMetadataProvider()
tag_dict = metadata.entries()
resolver = TagNameResolver(tag_dict)

# Set tags by name — stored under the correct numeric key
resolver["TileWidth"] = 512
resolver["TileLength"] = 512
resolver["ModelPixelScale"] = [0.5, 0.5, 0.0]

# Enumerated values resolve automatically
resolver["Compression"] = "LZW"           # stored as 5
resolver["Compression"] = "Deflate"       # stored as 8
resolver["Predictor"] = "Horizontal"      # stored as 2
resolver["SampleFormat"] = "Float"        # stored as 3

# Integer values pass through unchanged
resolver["Compression"] = 5               # also works

# Write resolved keys back to the metadata provider
for key, value in tag_dict.items():
    metadata[key] = value

provider = BufferedImageAssetProvider.create(
    key="image:0", num_columns=512, num_rows=512, num_bands=1,
    block_width=512, block_height=512, pixel_type=PixelType.Float32,
    metadata=metadata,
)
provider.set_full_image(image_data)

with IO.open(["output.tif"], "w", "tiff") as writer:
    writer.add_asset("image:0", provider, "Image", "desc", ["data"])
```

The supported enumerated value names (case-insensitive) are:

| Tag | Accepted names |
|-----|---------------|
| Compression (259) | None, CCITTRLE, CCITTFax3, CCITTFax4, LZW, OJPEG, JPEG, Deflate, PackBits |
| PhotometricInterpretation (262) | MinIsWhite, MinIsBlack, RGB, Palette, Mask, YCbCr |
| PlanarConfiguration (284) | Chunky, Planar |
| Predictor (317) | None, Horizontal, FloatingPoint |
| SampleFormat (339) | UInt, Int, Float, Void |
| Orientation (274) | TopLeft, TopRight, BottomRight, BottomLeft, LeftTop, RightTop, RightBottom, LeftBottom |
