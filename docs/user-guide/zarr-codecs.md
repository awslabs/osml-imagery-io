# Cloud Imagery Access via Zarr

## The Challenge of Working with Large Geospatial Datasets

Geospatial imagery archives are massive. Government and commercial satellite programs
produce hundreds of petabytes of imagery, growing at tens to hundreds of terabytes per 
day. The traditional workflow of downloading entire multi-GB files before accessing 
any pixels is giving way to distributed access patterns. Machine Learning inference,
tile-level analytics pulling from global data lakehouses, and interactive 
visualizations, all benefit from services that can quickly fetch only the compressed 
bytes for the tiles they care about, decode them, and move on.

The challenge is that some well established geospatial formats were designed before 
cloud object stores existed. Their internal structure — headers, offset tables, 
interleaved bands, shared compression state — was built for fast local disk access, 
not HTTP range requests with network delays. At best consumers of these images execute 
multiple reads to gather header and pixel information from different parts of a file. 
In some cases entire files must be scanned sequentially to locate the region of interst. 

## Where Zarr Fits In

[Zarr](https://zarr.dev/) is a modern array storage format designed from the ground up
for chunked, compressed, cloud-native data. Each chunk in a Zarr array is independently
addressable and independently decodable. Zarr enjoys integration with a wide ecosystem: 
xarray for labeled array access, Dask for parallel and distributed computation, and 
fsspec for transparent cloud IO. 

If we were starting from scratch, we might store our imagery as Zarr arrays and be
done. The reality is we have petabytes of imagery governed by well-defined standards 
and strong communities already stored in the cloud S3. Converting it all to Zarr or 
something like cloud-optimized GeoTIFFs is expensive, slow, and will increase 
storage costs. What we need is a way to make the existing data behave like Zarr without 
actually transcoding it.

## VirtualiZarr: Making Old Data Behave Like New

[VirtualiZarr](https://github.com/zarr-developers/VirtualiZarr) is a community project
under the `zarr-developers` organization that creates virtual Zarr stores from archival
data formats. Instead of converting files, it builds lightweight reference layers that
describe where each chunk lives inside an existing file. These references map Zarr chunk
coordinates to byte ranges in the original data, making archival files appear as native
Zarr stores without copying or modifying a single byte.

VirtualiZarr provides a pluggable parser system and a `ManifestStore` abstraction that
represents a virtual Zarr store backed by chunk references. The references can be
serialized to several formats including [Kerchunk](https://fsspec.github.io/kerchunk/)
JSON, Kerchunk Parquet, or committed to an [Icechunk](https://icechunk.io/) transactional
store. On the consumer side, fsspec's `ReferenceFileSystem` reads Kerchunk references
directly, while Icechunk provides its own zarr-compatible store.

Zarr is a great general abstraction for raster data, and VirtualiZarr is the bridge that
makes existing data behave like Zarr — without the cost of conversion. We keep our
geospatial imagery files exactly where they are in S3. A small reference index
(kilobytes to low megabytes) sits alongside each one, and the Zarr ecosystem treats
them as native chunked arrays.

## What We Built

Bridging archival imagery formats into the Zarr ecosystem required solving three
problems that the existing tooling does not handle. Each problem led to a
component:

1. **Tile Indexing for Geospatial Formats** — Zarr needs to know where each tile
   lives in the source file. Archival formats store this information in format-specific 
   structures (SOT markers, IFD tags, length-prefixed headers) that VirtualiZarr cannot
   parse out of the box.

2. **New Support for Non-contiguous Chunk Data** — The Kerchunk reference spec 
   assumes each chunk maps to a single contiguous byte range. JPEG 2000 codestreams 
   with resolution-first progression orders (RLCP, RPCL) interleave tile-parts from
   different tiles, scattering a single tile's data across multiple
   non-contiguous locations in the file. Neither Kerchunk nor fsspec's
   `ReferenceFileSystem` can express or fetch this.

3. **Decoders for Geospatial Formats** — The fetched bytes are still in the source
   format's native encoding. NITF pixel data uses big-endian byte order and
   format-specific interleave modes. JPEG 2000 tile-parts are not
   self-contained codestreams. Standard Zarr codecs cannot decode any of these.

The following sections describe each component in pipeline order: the parser
produces the tile index, the filesystem fetches the bytes, and the codec decodes
them into pixels.

### Tile Indexing with VirtualiZarr Parsers

The library provides `OversightMLParser`, a single VirtualiZarr parser that makes
it possible to access any imagery format supported by this library as a virtual
dataset. The parser scans a file using the library's native reader and walks each
image segment, determining the byte offset and length of every tile relative to
the start of the file.

How tile boundaries are calculated depends on the format and compression:

- Uncompressed tiles have fixed sizes. Offsets are computed arithmetically from
  the image dimensions, pixel type, and interleave mode. Formats that support
  sparse tile arrays (ex: NITF masked images) may have existing metadata that
  contains these boundaries.
- JPEG tiles are length-prefixed. The parser scans the length headers
  sequentially to locate each tile boundary.
- JPEG 2000 codestreams use SOT (Start of Tile-part) markers that record the
  byte offset and length of each tile-part. If the codestream contains a TLM
  (Tile-part Length Marker) in its main header, the full tile index is available
  immediately without scanning. If no TLM is present, the parser performs a
  sequential SOT scan.
- TIFF files store tile offsets and byte counts in IFD tags (`TileOffsets` and
  `TileByteCounts`). The parser reads these directly.

When a tile's data spans multiple non-contiguous byte ranges (as with
interleaved JPEG 2000 tile-parts), the parser detects this and emits a
multi-range reference entry instead of a single-range entry. Contiguous
multi-part tiles are merged into a single range automatically.

The parsers produce the `ManifestStore` — a virtual Zarr store backed by chunk
references into the source file. This can be serialized to create a
format-agnostic tile index.

### MultiReferenceFileSystem: Scatter-Gather I/O for Non-Contiguous Chunks

The standard Kerchunk reference spec supports three forms per chunk key: inline
data, whole-file references, and single byte-range references. This covers most
formats, but breaks down for JPEG 2000 codestreams with interleaved tile-parts.

```{image} /_static/images/kerchunk-singlerange.png
:alt: Figure showing how J2K tile parts grouped by tile are referenced by kerchunk index.
:width: 700px
:align: center
```

JPEG 2000 supports several progression orders that control how compressed data
is organized in the codestream. Two of these — RLCP (Resolution-Layer-Component-
Position) and RPCL (Resolution-Position-Component-Layer) — interleave tile-parts
from different tiles. Instead of writing all of tile 0's data, then all of tile
1's data, the encoder writes resolution level 0 for every tile, then resolution
level 1 for every tile, and so on. A single tile's compressed bytes end up
scattered across multiple non-contiguous locations in the file.

The standard `ReferenceFileSystem` has no way to express "fetch these six byte 
ranges and concatenate them" for a single chunk. This is not a theoretical edge 
case. Satellite imagery from several commercial providers uses RPCL progression 
order, and the interleaved tile-part layout is common in large multi-resolution 
JPEG 2000 files.

`MultiReferenceFileSystem` is a drop-in subclass of fsspec's
`ReferenceFileSystem` that extends the Kerchunk reference spec with a fourth
form:

| Form | Format | Description |
|------|--------|-------------|
| Inline | `"base64:..."` or raw bytes | Inline data |
| Whole file | `["url"]` | Entire file |
| Single range | `["url", offset, length]` | One contiguous byte range |
| **Multi-range** | `["url", [[offset, length], ...]]` | **Multiple non-contiguous byte ranges** |

A multi-range entry is a 2-element list where the first element is the URL and
the second is a list of `[offset, length]` pairs. Each pair identifies one
tile-part's location in the file. The filesystem fetches all ranges and
concatenates them in order before handing the bytes to the codec.

For example, a tile with six tile-parts scattered across a file:

```json
{
  "image:0/0.0.0": [
    "s3://bucket/image.ntf",
    [[66132, 1518], [2534029, 3385], [7216065, 11460],
     [22566527, 38566], [74210429, 116812], [242293202, 339534]]
  ]
}
```

The URL appears once rather than being repeated for each sub-range. For a file
with 1,722 tiles and six tile-parts each, this saves roughly 775 KB of redundant
URL strings compared to a flat list of single-range entries.

```{image} /_static/images/kerchunk-multirange.png
:alt: Figure showing how J2K tile parts interleaved by resolution level are referenced by kerchunk index.
:width: 700px
:align: center
```

`MultiReferenceFileSystem` handles all standard reference types by delegating to
the parent `ReferenceFileSystem`. When it encounters a multi-range entry, it
performs scatter-gather I/O:

- **Sync path** (`_cat_common`): fetches each byte range sequentially and
  concatenates the results. Adequate for local files and testing.
- **Async path** (`_cat_file`): issues all byte-range fetches concurrently
  via `asyncio.gather` and concatenates in the original entry order. This is
  the path used by Zarr's async store and is critical for minimizing latency
  when reading from S3.

Use `MultiReferenceFileSystem` instead of `ReferenceFileSystem` when your tile
index may contain multi-range entries. It is fully backward-compatible — tile
indexes with only standard single-range entries work identically. The index
generator in this library automatically emits multi-range entries for tiles
with interleaved tile-parts and single-range entries for everything else, so
using `MultiReferenceFileSystem` as the default is the simplest approach.

```python
from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem

fs = MultiReferenceFileSystem(
    fo="s3://bucket/image.tile_index.json",
    asynchronous=True,
    remote_options={"asynchronous": True},
    skip_instance_cache=True,
)
```

The constructor accepts the same arguments as `ReferenceFileSystem`. Existing
code that uses `fsspec.filesystem("reference", ...)` can switch by replacing
the filesystem instantiation.

```{note}
This multi-range reference format is a novel extension to the Kerchunk
reference spec introduced by this project. The standard Kerchunk and Zarr
ecosystem does not handle the case of non-contiguous byte ranges for a single
chunk. If you encounter JPEG 2000 imagery with interleaved tile-parts
elsewhere, `MultiReferenceFileSystem` is the component that makes it work.
```

### Custom Codecs

Once the filesystem delivers the raw bytes for a chunk, those bytes are still in
the source format's native encoding. Standard Zarr codecs cannot decode them.
This library registers three Zarr v3 codecs that handle the format-specific
decoding:

`JbpBlockCodec` handles uncompressed NITF tiles. NITF raw pixel data uses
big-endian byte order and one of four interleave modes (band-interleaved by
pixel, band-interleaved by line, band-interleaved by block, or
band-sequential). The codec performs the endian swap and interleave conversion
to produce standard NumPy arrays.

`JpegCodec` decodes JPEG tiles. It carries format-specific parameters (color
space, interleave mode, bits per pixel) in its configuration and passes them to
the underlying Rust decoder.

`Jpeg2000Codec` decodes JPEG 2000 tile data. JPEG 2000 codestreams support
internal tiling, but the tiles are not self-contained. Each tile's compressed
data (the "tile-part") contains only the wavelet coefficients. The decoding
parameters — tile dimensions, quantization tables, wavelet decomposition levels,
component counts — live in the codestream's main header (the SIZ, COD, and QCD
markers). A decoder cannot reconstruct pixels from a tile-part alone. Existing
Zarr JPEG 2000 codecs assume each chunk is a complete, self-contained
codestream. They cannot decode a bare tile-part without the header.

We solve this by inlining the shared main header (base64-encoded, typically
100–500 bytes) in the codec configuration stored in `.zarray`. At decode time
the codec reconstructs a minimal single-tile codestream on the fly:

```
[main_header bytes] + [tile-part bytes] + [EOC marker]
```

OpenJPEG receives what looks like a normal single-tile codestream and decodes
it. This approach has precedent in the JPEG 2000 ecosystem. JPIP (the JPEG 2000
Interactive Protocol) streams individual tile-parts to clients that already hold
the main header. Because the codec operates on the J2K codestream directly, it
works for standalone `.j2k`/`.jp2` files and for J2K codestreams embedded in
container formats like NITF.

Note that the codec layer is intentionally pure — it performs no I/O. When
`MultiReferenceFileSystem` fetches and concatenates multiple tile-parts for an
interleaved codestream, the codec receives the complete concatenated bytes and
reconstructs the codestream exactly the same way. The filesystem handles the
scatter-gather complexity so codecs remain simple bytes-to-bytes transforms.

All three codecs are registered with the Zarr codec registry via Python entry
points. They use URI-based names per the Zarr v3 specification to avoid
conflicts with existing codecs:

- `https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000`
- `https://awslabs.github.io/osml-imagery-io/codecs/jpeg`
- `https://awslabs.github.io/osml-imagery-io/codecs/jbp-block`

The URIs resolve to human-readable documentation. Implementations do not fetch
them at runtime.

```{note}
No custom codec is needed for TIFF. The compression formats commonly used in
TIFF files (Deflate, LZW, and uncompressed) are already supported natively by
Zarr. The parser handles tile indexing, and Zarr's built-in codecs handle the
decoding.
```

## End-to-End Example: NITF Imagery in S3

The following example shows the complete workflow for indexing a NITF file and
accessing its tiles through xarray and Dask.

### Step 1: Generate the tile index

Run this once per file, typically as part of an ingest pipeline. The file must be
available locally for indexing. The `url` parameter sets the S3 path that tile
references will point to.

```{note}
Index generation requires the `virtualizarr` optional dependency:
`pip install osml-imagery-io[virtualizarr]`
```

```python
from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

# Generate index from a local file — works for NITF, JPEG 2000, TIFF, GeoTIFF
parser = OversightMLParser(local_path="local/image.ntf")
store = parser(url="s3://my-bucket/imagery/image.ntf")

# Serialize as Kerchunk JSON (or .parquet) with multi-range support
write_tile_index(store, "image.ntf.tile_index.json")
```

Upload the index to S3 alongside the image. This assumes the image itself is
already residing in the cloud.

```python
import boto3

s3 = boto3.client("s3")
s3.upload_file(
    "image.ntf.tile_index.json",
    "my-bucket",
    "imagery/image.ntf.tile_index.json",
)
```

### Step 2: Open and access tiles

Codec registration happens automatically when the package is installed with the
`zarr` extras (`pip install osml-imagery-io[zarr]`). No explicit import is needed.

Use `MultiReferenceFileSystem` to open the tile index. It handles both standard
single-range entries and multi-range entries for JPEG 2000 images with
interleaved tile-parts. When you slice into the dataset, the filesystem issues
HTTP range requests for only the bytes backing the requested tiles and the
registered codec decodes them into NumPy arrays.

```python
import zarr
from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
from zarr.storage._fsspec import FsspecStore

# MultiReferenceFileSystem is a drop-in replacement for ReferenceFileSystem
# that adds support for multi-range chunk references
fs = MultiReferenceFileSystem(
    fo="s3://my-bucket/imagery/image.ntf.tile_index.json",
    asynchronous=True,
    remote_options={"asynchronous": True, "profile": "my-profile"},
    skip_instance_cache=True,
)

store = FsspecStore(fs=fs, read_only=True, path="")
root = zarr.open_group(store, mode="r", zarr_format=2)

# Read a single tile region
import numpy as np

arr = root["image:0"]
tile = np.asarray(arr[0:3, 768:1024, 1024:1280])
print(tile.shape)  # (3, 256, 256)
print(tile.dtype)  # uint8
```

AWS credentials can also be provided through environment variables
(`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_PROFILE`) or any other
method supported by boto3 and fsspec.
