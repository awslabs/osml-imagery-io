# Datasets and the IO Interface

## The Simple Path

For most tasks you don't need to think about datasets or assets at all. The
convenience functions handle file opening, asset selection, and cleanup for you:

```python
from aws.osml.io import imread, imsave, iminfo

# Read → NumPy array
pixels = imread("image.ntf")

# Inspect without reading pixels
info = iminfo("image.ntf")
print(f"{info.width}x{info.height}, {info.bands} bands, {info.dtype}")

# Save — format inferred from extension
imsave("output.tif", pixels)
```

When you need more control — multi-segment files, per-asset metadata, specific
compression parameters, or write workflows that involve multiple assets — the
full dataset API described below gives you direct access to everything in the
file.

## Opening a Dataset

The `IO` class is the entry point for reading and writing imagery files. It auto-detects
the format (NITF, TIFF/GeoTIFF, PNG, etc.) and returns a `DatasetReader` or `DatasetWriter`:

```python
from aws.osml.io import IO

# Read mode — format auto-detected from extension
with IO.open(["image.ntf"], "r") as dataset:
    print(type(dataset))  # DatasetReader

# Write mode — format specified explicitly
with IO.open(["output.tif"], "w", "geotiff") as writer:
    print(type(writer))  # DatasetWriter
```

Use the context manager (`with`) to ensure file handles are released when you're done.

## Input Sources

`IO.open()` and the convenience functions (`imread`, `imsave`, `iminfo`, `tiles`)
accept two kinds of input:

### File paths (recommended for large files)

Pass a string path (or list of paths for multi-file pyramids). The library
memory-maps the file, so only the pages you access are loaded into RAM. This is
the most performant option — the operating system efficiently manages loading
imagery from disk into memory without requiring the entire file to be resident.

```python
from aws.osml.io import imread

pixels = imread("large_image.ntf")
```

### Python file-like objects

Any object with a standard `.read()` / `.write()` interface works — `io.BytesIO`,
fsspec handles, HTTP response bodies, or any duck-typed object with the required
methods. This is convenient when you already have bytes in memory, want to read
directly from cloud storage, or want to encode directly to a buffer without
touching the filesystem.

```python
import io
from aws.osml.io import IO, imread, imsave
import numpy as np

# Read from an in-memory buffer
png_bytes = download_image_bytes()
pixels = imread(io.BytesIO(png_bytes), format="png")

# Write directly to a buffer
data = np.random.randint(0, 255, (3, 256, 256), dtype=np.uint8)
buffer = io.BytesIO()
imsave(buffer, data, format="jpeg")
```

### Remote reading without a full download

The library reads remote objects with on-demand byte-range requests instead of
downloading the whole file. Opening the dataset and reading metadata or specific
tiles fetches only the bytes those operations touch (headers, offset tables, and
the requested tiles), so a multi-GB remote file becomes practical to inspect and
read from over the network.

There are three ways to point the library at a remote source, all equivalent:

```python
from aws.osml.io import IO, imread
import fsspec

# 1. A remote URL string — resolved to an fsspec filesystem internally.
pixels = imread("s3://bucket/large_image.ntf", format="nitf")

# 2. An explicit fsspec filesystem instance + path via filesystem=.
fs = fsspec.filesystem("s3")
with IO.open("s3://bucket/large_image.ntf", "r", format="nitf", filesystem=fs) as dataset:
    block = dataset.get_asset("image:0").get_block(0, 0)

# 3. A raw fsspec/s3fs file-like handle (also works — see note below).
with fsspec.open("s3://bucket/large_image.ntf", "rb") as handle:
    with IO.open(handle, "r", format="nitf") as dataset:
        block = dataset.get_asset("image:0").get_block(0, 0)  # fetches only this tile's ranges
```

The `filesystem=` parameter is accepted by `IO.open`, `imread`, `iminfo`, and
`tiles`. When it is omitted, a remote URL string is resolved to a filesystem
internally via `fsspec.core.url_to_fs`.

:::{note}
Prefer a URL string or `filesystem=` over a raw handle. When the library has the
`(filesystem, path)` pair it fetches a tile's scattered byte ranges
**concurrently** (via fsspec's `cat_ranges`), which is markedly faster over a
high-latency object store. A raw handle still works — the library recovers the
filesystem from the handle's `.fs`/`.path` where it can — but this is a
best-effort fallback. Passing `filesystem=` together with a file-like or
in-memory (`io.BytesIO`) source raises `ValueError`, because the two are
contradictory.
:::

Range reading applies to the **block-capable** formats — NITF, TIFF/GeoTIFF, JPEG
2000, and DTED. For these, `IO.open`, `iminfo`, `tiles`, and
`DatasetReader.get_block` all read incrementally. Peak memory tracks the fetched
ranges (plus a small header prefetch), not the file size.

The `format` argument is required for streams (there is no filename to infer from);
see [The `format` parameter](#the-format-parameter) below.

#### Tuning concurrency: the connection pool

Concurrent range fetches share the underlying s3fs/botocore connection pool,
which defaults to **10** connections (`MAX_POOL_CONNECTIONS`). For workloads that
fan many blocks out across threads, raising the pool can improve throughput —
construct the filesystem with a larger `max_pool_connections` and pass it via
`filesystem=`:

```python
import fsspec
from aws.osml.io import IO

fs = fsspec.filesystem("s3", config_kwargs={"max_pool_connections": 32})
with IO.open("s3://bucket/large_image.ntf", "r", format="nitf", filesystem=fs) as dataset:
    ...
```

This is **optional and environment-dependent** — measure before adopting it. The
default pool of 10 is already enough to fetch a single tile's parts concurrently;
the larger pool only pays off once many blocks are read in parallel (e.g. a
threaded region-of-interest read), and the actual benefit scales with per-request
latency (larger cross-region, smaller in-region).

### When the full file is read

Two cases fall back to reading the entire stream into memory via a single `.read()`:

- **Monolithic formats (PNG, standalone JPEG).** These have no sub-file structure to
  exploit — a whole-file read is the only decode strategy — so they are read in full
  even from a seekable handle.
- **Non-seekable or unknown-size streams** (e.g. a plain `io.BytesIO`, a pipe, or an
  HTTP body with no length). With no way to issue range reads, the library reads the
  stream fully, exactly as earlier versions did.

For these cases, the same trade-offs as before apply — the full file must fit in RAM,
and a remote source is downloaded before decoding begins.

### Bulk tile access via VirtualiZarr

For repeated, tile-parallel access to large remote imagery — especially building an
index once and serving many reads — the [VirtualiZarr tile-based access](zarr-codecs.md)
path remains preferred. It persists per-tile byte offsets so fsspec issues direct
range GETs per chunk with no re-scan:

```python
import zarr
import numpy as np
from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
from zarr.storage._fsspec import FsspecStore

fs = MultiReferenceFileSystem(
    fo="s3://bucket/image.ntf.tile_index.json",
    template_overrides={"base": "s3://bucket/imagery/"},
    asynchronous=True,
    remote_options={"asynchronous": True},
    skip_instance_cache=True,
)
store = FsspecStore(fs=fs, read_only=True, path="")
root = zarr.open_group(store, mode="r", zarr_format=2)

# Read only the tiles you need — no full-file download
tile = np.asarray(root["0/data"][0:3, 0:256, 0:256])
```

See [Cloud Imagery Access via Zarr](zarr-codecs.md) for the full workflow. Building
the index (`OversightMLParser`) over a remote `url` itself uses range reads, so even
index construction no longer requires a full download.

Alternatively, download the remote file to a local path first to get
memory-mapped performance:

```python
import tempfile
from aws.osml.io import imread

with tempfile.NamedTemporaryFile(suffix=".ntf") as tmp:
    tmp.write(remote_file.read())
    tmp.flush()
    pixels = imread(tmp.name)
```

### The `format` parameter

When working with streams, the library cannot infer the image format from a file
extension. The `format` parameter is **required** for all stream operations:

```python
# Raises ValueError — no format specified
imread(io.BytesIO(data))

# Works
imread(io.BytesIO(data), format="png")
```

Supported format strings: `"nitf"`, `"tiff"`, `"png"`, `"j2k"`, `"jpeg"`.

When using file paths, `format` remains optional — the library infers it from the
file extension. Recognized NITF extensions include `.ntf`, `.nitf`, `.nsif`, `.nsf`,
and `.hr1` through `.hr8` (High Resolution Elevation products).

### When streams are a good fit

- You are reading a block-capable format (NITF, TIFF/GeoTIFF, JPEG 2000, DTED) from
  a seekable, sized handle (fsspec/s3fs) and want range reads instead of a full
  download — including large remote files
- The file is small enough to fit in memory (PNG thumbnails, JPEG tiles, small
  NITF chips)
- You already have the bytes in memory (HTTP response bodies, message payloads)
- You want to encode output directly to a buffer without a temporary file (tile
  server responses)

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
| `mask` | Transparency mask (valid vs. nodata pixels) | TIFF transparency-mask IFDs |
| `metadata` | Metadata asset | NITF text segments, data extension segments |
| `graphic` | Graphic/annotation overlay | NITF graphic segments |

An asset may carry more than one role. A COG overview that is itself a
transparency mask, for example, carries both `overview` and `mask` — a query for
`roles=["overview"]` returns it alongside the image overviews, so consumers that
walk the pyramid for rendering should exclude assets that also carry `mask`.

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
from aws.osml.io import IO, AssetType

with IO.open(["cog.tif"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type=AssetType.Image):
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
    data_keys = dataset.get_asset_keys(asset_type=AssetType.Image, roles=["data"])
    overview_keys = dataset.get_asset_keys(asset_type=AssetType.Image, roles=["overview"])

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
    for key in dataset.get_asset_keys(asset_type=AssetType.Image):
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
auto-detected format reader, so users are free to select other encodings for the
overview files if desired.

:::{note}
R-sets are a de facto industry convention used by some data providers and image
analysis tools. They are not part of the JBP/NITF specification — there is no 
internal metadata linking an R-set file to its parent. The relationship is purely
by filename convention.
:::

#### Remote multi-file pyramids

Multi-file pyramids may live on a remote object store. When reading, each entry
in the list is resolved the same way a single remote path is (see [Remote
reading without a full download](#remote-reading-without-a-full-download)) —
remote entries open through fsspec with on-demand byte-range requests, local
entries stay memory-mapped. There are two ways to point `IO.open` at a remote
pyramid, mirroring the single-path forms:

```python
from aws.osml.io import IO
import fsspec

# 1. A list of remote URLs — each resolved to an fsspec filesystem internally.
urls = [
    "s3://bucket/image.ntf",
    "s3://bucket/image.ntf.r1",
    "s3://bucket/image.ntf.r2",
]
with IO.open(urls, "r") as dataset:
    print(dataset.get_asset_keys())  # image:0, image:0:overview:1, ...

# 2. A shared filesystem + scheme-less keys via filesystem=.
fs = fsspec.filesystem("s3")
keys = ["bucket/image.ntf", "bucket/image.ntf.r1", "bucket/image.ntf.r2"]
with IO.open(keys, "r", filesystem=fs) as dataset:
    overview = dataset.get_asset("image:0:overview:1")
```

The `.rN` naming convention is detected on the key regardless of scheme, so
`s3://bucket/image.ntf.r1` is recognized as an overview exactly like the local
`image.ntf.r1`. When `filesystem=` is passed, the single shared filesystem is
applied to every entry — the natural case when the base and its overviews live
under one bucket. Entries are decided per source, so a list mixing a local base
with remote overviews (or vice-versa) opens correctly.

Remote multi-file pyramids are **read mode only**. Passing `filesystem=` with a
list of paths in write mode raises `ValueError`, as does passing it with a list
of streams (a stream has no `(filesystem, path)` to resolve). Remote pyramids
are opened through `IO.open` directly — `imread`, `iminfo`, and `tiles` remain
single-image convenience wrappers.

Some things to keep in mind with multi-file pyramids:

- The caller must provide the full list of paths explicitly. `IO.open()` does not
  scan the filesystem for sibling `.rN` files.
- When only one path is provided, behavior is identical to the single-file case.
- R-set overviews are associated with `image:0` (the primary image segment). If the
  base file contains multiple image segments, the R-sets apply to the primary image
  only.
- The same multi-path pattern works for writing — see
  [Writing Multi-File R-Set Pyramids](image-assets-writing.md#writing-multi-file-r-set-pyramids).

### Streams and Explicit Roles

When sources are streams rather than file paths, there are no filenames to parse for
`.rN` suffixes. The `roles` parameter tells the library the purpose of each source
explicitly:

```python
import io
from aws.osml.io import IO, AssetType

base_stream = io.BytesIO(base_bytes)
overview1_stream = io.BytesIO(overview1_bytes)
overview2_stream = io.BytesIO(overview2_bytes)

with IO.open(
    [base_stream, overview1_stream, overview2_stream],
    "r",
    format="nitf",
    roles=[["data"], ["overview:1"], ["overview:2"]],
) as dataset:
    # Same asset key structure as file-path R-sets
    for key in dataset.get_asset_keys(asset_type=AssetType.Image):
        asset = dataset.get_asset(key)
        print(f"{key}: {asset.num_columns}x{asset.num_rows}")
    # image:0: 4096x4096
    # image:0:overview:1: 2048x2048
    # image:0:overview:2: 1024x1024
```

The `roles` parameter assigns semantic roles to each source in a multi-source
dataset:

| First argument | `roles` type | Description |
|----------------|--------------|-------------|
| Single source (`str` or stream) | `list[str]` | Roles for the single source |
| List of sources | `list[list[str]]` | One inner list per source (must match list length) |

Role strings:

| Role string | Meaning |
|-------------|---------|
| `"data"` | Base image (full resolution). If omitted, the first source is treated as the base. |
| `"overview:N"` | R-set overview at level N (N ≥ 1). Maps to the `image:0:overview:N` asset key. |

When `roles` is required:

- **List of streams** — always required (no filenames to detect from). Omitting raises `ValueError`.
- **List of file paths with `roles`** — explicit roles override `.rN` filename detection.
- **List of file paths without `roles`** — falls back to `.rN` detection (common convention).

```python
# Paths with explicit roles — bypasses .rN detection
IO.open(["base.ntf", "ovr.ntf"], "r", roles=[["data"], ["overview:1"]])

# Paths without roles — uses .rN detection
IO.open(["image.ntf", "image.ntf.r1"], "r")
```

## Transparency Masks

Cloud Optimized GeoTIFFs commonly carry a **transparency mask** — a separate 1-bit
IFD that marks which pixels of an associated image are valid versus nodata. The
TIFF specification (TIFF 6.0, p.37) defines these as IFDs with
`PhotometricInterpretation = 4`: the 1-bits define the interior (valid) region and
the 0-bits define the exterior (nodata).

This library exposes each mask as an ordinary image asset. Because the mask is
1-bit data, it is unpacked to one `uint8` byte per pixel (`0` or `1`) and flows
through the same API as any other image — `iminfo` lists it, `imread` returns a
`uint8` NumPy array, and `tiles`/`IO` treat it as image data. The public
`PixelType` is unchanged; no packed 1-bit representation crosses the NumPy
boundary.

### Mask Asset Keys

A mask's key is its associated image's key plus a `:mask` suffix, so the
association is structural and parseable:

```python
from aws.osml.io import IO, AssetType

with IO.open(["cog.tif"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type=AssetType.Image):
        asset = dataset.get_asset(key)
        print(f"{key}: roles={asset.roles}")
    # image:0: roles=['data']
    # image:0:mask: roles=['mask']
    # image:0:overview:1: roles=['overview']
    # image:0:overview:1:mask: roles=['overview', 'mask']
```

A mask of the full-resolution image is keyed `image:0:mask`; a mask of an overview
is keyed off that overview, e.g. `image:0:overview:1:mask`, and carries both the
`overview` and `mask` roles.

Association is order-based: the reader binds a mask IFD to the most recent
full-resolution or overview IFD, relying on the COG IFD-ordering guarantee (OGC
COG Recommendation 3). For a valid but non-COG-ordered TIFF where the ordering is
ambiguous, the mask is exposed as a standalone `image:N:mask`.

:::{note}
Reading a mask asset returns its validity bitmap as pixels. The library does
**not** yet auto-apply the mask as nodata/fill when reading the associated image —
that is a planned follow-on. To use a mask, read it explicitly and apply it
yourself.
:::

### Writing Masks

The writer round-trips a mask that already exists in the dataset model. Provide a
mask-role image asset keyed with a `:mask` suffix and it is written as a
`PhotometricInterpretation = 4`, `BitsPerSample = 1`, `SamplesPerPixel = 1` IFD in
COG-compliant order (image → its mask → image overviews → mask overviews).

Mask input is coerced to strict 0/1 on write: any nonzero sample becomes `1`. This
means you may supply a `uint8` mask stored as 0/255 (a common convention) and it
is packed correctly, matching the bilevel coercion used elsewhere in the library.

## Supported Compressions and Bit Depths

The TIFF reader accepts the following compression schemes: uncompressed, LZW,
Deflate (zlib), Adobe Deflate, PackBits, JPEG, and CCITT Group 3 / Group 4 fax
(the latter two are bilevel schemes typically used by 1-bit mask IFDs).

Sample bit depths are handled as follows:

- **8, 16, 32, 64 bits** — mapped directly to the corresponding NumPy dtype
  (`uint8`/`int8`, `uint16`/`int16`, `uint32`/`int32`, `float32`, `float64`).
- **1, 2, 4 bits** (sub-byte) — unpacked to one `uint8` per sample (MSB-first, with
  TIFF's per-row byte-boundary padding respected). A 1-bit sample yields `{0, 1}`,
  a 2-bit sample `{0..3}`, and a 4-bit sample `{0..15}`. This closes the class of
  Baseline TIFF sub-byte grayscale as well as transparency masks.
- **12-bit data** lives in a 16-bit storage container (libtiff stores widths in
  {1, 2, 4, 8, 16, 32, 64}) and round-trips losslessly as `uint16`. There is no
  significant-bits/ABPP surfacing — that is an NITF concept with no TIFF-tag
  equivalent, and the 16-bit container preserves every stored value.

## Discovering Assets

Use `get_asset_keys()` to list available assets, then `get_asset()` to retrieve
a specific one. You can filter by asset type, by role, or both:

```python
from aws.osml.io import IO, AssetType

with IO.open(["complex_dataset.ntf"], "r") as dataset:
    # List keys by asset type
    image_keys = dataset.get_asset_keys(asset_type=AssetType.Image)
    text_keys = dataset.get_asset_keys(asset_type=AssetType.Text)
    data_keys = dataset.get_asset_keys(asset_type=AssetType.Data)
    graphics_keys = dataset.get_asset_keys(asset_type=AssetType.Graphics)

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
    data_keys = dataset.get_asset_keys(asset_type=AssetType.Image, roles=["data"])

    # Only overview images
    overview_keys = dataset.get_asset_keys(asset_type=AssetType.Image, roles=["overview"])

    # All image assets (no role filter)
    all_keys = dataset.get_asset_keys(asset_type=AssetType.Image)
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
    file_metadata = dataset.metadata.entries()
```
