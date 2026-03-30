# Cloud Imagery Access via Zarr

## The Challenge of Working with Large Geospatial Datasets

Geospatial imagery archives are massive. Government and commercial satellite programs
produce hundreds of petabytes of imagery, growing at tens to hundreds of terabytes per 
day. The traditional workflow of downloading entire multi-GB files before accessing 
any pixels is giving way to distributed access patterns. Machine Learning inference,
tile-level analytics running on data lakehouse architectures, and interactive 
visualizations, all benefit from services that can quickly fetch only the compressed 
bytes for the tiles they care about, decode them, and move on.

The challenge is that some well established formats, like NITF, were designed before 
cloud object stores existed. Their internal structure — headers, offset tables, 
interleaved bands, shared compression state — was built for fast local disk access, 
not HTTP range requests with network delays. At best consumers of these images execute 
multiple reads to gather header and pixel information from different parts of a file. 
In some cases entire files must be scanned to locate the region of interst. 

## Where Zarr Fits In

[Zarr](https://zarr.dev/) is a modern array storage format designed from the ground up
for chunked, compressed, cloud-native data. Each chunk in a Zarr array is independently
addressable and independently decodable. Zarr enjoys integration with a wide ecosystem: 
xarray for labeled array access, Dask for parallel and distributed computation, and 
fsspec for transparent cloud IO. If we can make our imagery accessible like Zarr chunks, 
the entire ecosystem works out of the box.

If we were starting from scratch, we might store our imagery as Zarr arrays and be
done. The reality is we have petabytes of imagery governed by well-defined standards 
and strong communities already stored in the cloud S3. Converting it all to Zarr or 
something like cloud-optimized GeoTIFFs is expensive, slow, and likely increase 
storage costs. What we need is a way to make the existing data behave like Zarr without 
actually transcoding it.

## Kerchunk: Making Old Data Behave Like New

[Kerchunk](https://fsspec.github.io/kerchunk/) is a community project that bridges
archival data formats and the Zarr ecosystem. Instead of converting files, it creates
lightweight reference files that describe where each chunk lives inside an existing
file — a JSON or Parquet document that maps Zarr chunk coordinates to byte ranges in
the original data. fsspec's `ReferenceFileSystem` consumes these references directly,
making the archival file appear as a virtual Zarr store.

Zarr is a great general abstraction for raster data, and Kerchunk is the bridge that
makes existing data behave like Zarr — without the cost of conversion. We keep our
geospatial imagery files exactly where they are in S3. A small index file (kilobytes to
low megabytes) sits alongside each one, and the Zarr ecosystem treats them as native
chunked arrays.

## What We Built

This library provides the two components needed to bridge existing imagery formats into the
Zarr ecosystem: a tile indexer and a set of custom codecs.

The tile indexer scans a source file and produces a Kerchunk v1 reference file. The
reference file maps every image tile to a byte range in the original data. The custom
codecs decode those bytes into NumPy arrays at read time. Together, they let the Zarr
ecosystem treat NITF and TIFF files as chunked arrays without any format conversion.

### Tile Indexing

The `TileIndex` class opens a local imagery file using the library's native reader
and walks each image segment. For each segment it determines the byte offset and
length of every tile relative to the start of the file.

How tile boundaries are discovered depends on the format and compression:

- Uncompressed tiles have fixed sizes. Offsets are computed arithmetically from the
  image dimensions, pixel type, and interleave mode.
- JPEG tiles are length-prefixed. The indexer scans the length headers sequentially
  to locate each tile boundary.
- JPEG 2000 codestreams use SOT (Start of Tile-part) markers that record the byte
  offset and length of each tile-part. If the codestream contains a TLM (Tile-part
  Length Marker) in its main header, the full tile index is available immediately
  without scanning. If no TLM is present, the indexer performs a sequential SOT scan.
- TIFF files store tile offsets and byte counts in IFD tags (`TileOffsets` and
  `TileByteCounts`). The indexer reads these directly.

The output is a Kerchunk v1 JSON (or Parquet) file. Each tile becomes a
`[url, byte_offset, byte_length]` triple pointing into the source file. Array
metadata (`.zarray`) records the shape, chunk size, data type, and codec
configuration. The index is typically kilobytes to low megabytes, regardless of
image size.

### Custom Codecs

The tile index tells fsspec where to fetch each tile's bytes, but those bytes are
still in the source format's native encoding. Standard Zarr codecs cannot decode
them. The library registers three Zarr v3 codecs that handle the format-specific
decoding:

`Jpeg2000Codec` decodes JPEG 2000 tile data. JPEG 2000 codestreams support
internal tiling, but the tiles are not self-contained. Each tile's compressed
data (the "tile-part") contains only the wavelet coefficients. The decoding
parameters — tile dimensions, quantization tables, wavelet decomposition levels,
component counts — live in the codestream's main header (the SIZ, COD, and QCD
markers). A decoder cannot reconstruct pixels from a tile-part alone. Existing
Zarr JPEG 2000 codecs assume each chunk is a complete, self-contained
codestream. They cannot decode a bare tile-part without the header.

We solve this by inlining the shared main header (base64-encoded, typically
100–500 bytes) in the codec configuration stored in `.zarray`. At decode time the
codec reconstructs a minimal single-tile codestream on the fly:

```
[main_header bytes] + [tile-part bytes] + [EOC marker]
```

OpenJPEG receives what looks like a normal single-tile codestream and decodes it.
This approach has precedent in the JPEG 2000 ecosystem. JPIP (the JPEG 2000
Interactive Protocol) streams individual tile-parts to clients that already hold
the main header. ESA adopted the same principle when adding TLM markers to
Sentinel-2 imagery to enable per-tile random access. Because the codec operates
on the J2K codestream directly, it works for standalone `.j2k`/`.jp2` files and
for J2K codestreams embedded in container formats like NITF.

`JpegCodec` decodes JPEG tiles. It carries format-specific parameters (color
space, interleave mode, bits per pixel) in its configuration and passes them to
the underlying Rust decoder.

`JbpBlockCodec` handles uncompressed NITF tiles. NITF raw pixel data uses
big-endian byte order and one of four interleave modes (band-interleaved by pixel,
band-interleaved by line, band-interleaved by block, or band-sequential). The
codec performs the endian swap and interleave conversion to produce standard NumPy
arrays.

All three codecs are registered with the Zarr codec registry via Python entry
points. They use URI-based names per the Zarr v3 specification to avoid conflicts
with existing codecs:

- `https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000`
- `https://awslabs.github.io/osml-imagery-io/codecs/jpeg`
- `https://awslabs.github.io/osml-imagery-io/codecs/jbp-block`

The URIs resolve to human-readable documentation. Implementations do not fetch
them at runtime.

### Supported Compression Types

#### NITF

| IC Code | Description | Codec | Notes |
|---------|-------------|-------|-------|
| NC / NM | Uncompressed | `JbpBlockCodec` | All pixel types, all interleave modes |
| C3 / M3 | JPEG DCT | `JpegCodec` | 8-bit lossy |
| I1 | JPEG downsampled | `JpegCodec` | Single-block thumbnail |
| C8 / M8 | JPEG 2000 Part 1 | `Jpeg2000Codec` | Lossy and lossless |
| CD / MD | HTJ2K (Part 15) | `Jpeg2000Codec` | High-Throughput JPEG 2000 |

#### TIFF

| Compression | Codec | Notes |
|-------------|-------|-------|
| Deflate / LZW | Native Zarr | Tiles are directly Zarr-compatible |
| Uncompressed | Native Zarr | Direct byte access |
| JPEG | `JpegCodec` | 8-bit lossy |

## Results

*TODO: This section will present benchmark results measuring tile access latency and
throughput through the Zarr/Kerchunk interface. Planned measurements include:*

- *Single-tile access latency from S3 using Zarr/fsspec (cold and warm)*
- *Comparison of Zarr/fsspec access versus direct local file access*
- *Memory usage under concurrent tile decoding*

*Benchmarks will use representative NITF files across compression types (C8, C3,
NC) and image sizes (100 MB, 1 GB, 10 GB+).*

## End-to-End Example: NITF Imagery in S3

The following example shows the complete workflow for indexing a NITF file and
accessing its tiles through xarray and Dask.

### Step 1: Generate the tile index

Run this once per file, typically as part of an ingest pipeline. The file must be
available locally for indexing. The `source_uri` parameter sets the S3 path that
tile references will point to.

```python
from aws.osml.io import TileIndex

index = TileIndex.generate(
    "local/image.ntf",
    source_uri="s3://my-bucket/imagery/image.ntf",
)
index.save("image.ntf.index.json")
```

Upload both files to S3:
TODO: Show the boto3 commands to do this upload

```
s3://my-bucket/imagery/image.ntf
s3://my-bucket/imagery/image.ntf.index.json
```

### Step 2: Open the imagery as a Zarr dataset

Codec registration happens automatically when the package is installed with the
`zarr` extras (`pip install osml-imagery-io[zarr]`). No explicit import is needed.

```python
import xarray as xr

ds = xr.open_zarr(
    "reference://",
    storage_options={
        "fo": "s3://my-bucket/imagery/image.ntf.index.json",
        "remote_protocol": "s3",
    },
)
```

### Step 3: Access tiles

fsspec issues HTTP range requests for only the bytes backing the requested tiles.
The registered codec decodes them into NumPy arrays.

TODO: Check how Zarr handles this, assume we can access arbitrary slices and Zarr will correctly only fetch necessary tiles.
TODO: Describe Zarr slicing and note difference to underlying tile boundaries defined by tile index.

```python
# Read a single tile region
tile = ds["image_segment_0"][0:3, 768:1024, 1024:1280].values
print(tile.shape)  # (3, 256, 256)
print(tile.dtype)  # uint8
```

### Step 4: Scale with Dask

TODO: Replace this with a much more interesting example. Do something real, compute a reduced resolution dataset, do some warping, extract features, etc.

Because the dataset is chunked, Dask can parallelize tile access across workers.
Each worker fetches and decodes only the tiles it needs.

```python
import dask.array as da

# Lazy — no data is fetched yet
image = ds["image_segment_0"].data

# Compute a downsampled overview across the full image
# Dask distributes tile fetches across workers
mean_band = image[0].mean(axis=0).compute()
```

### Using a local index for development
TODO: Consider removing this. It isn't really necessary. Maybe it is a sentence acknowledging fsspec abstracts away the filesystem.

For local testing, point to a file path instead of an S3 URI:

```python
ds = xr.open_zarr(
    "reference://",
    storage_options={"fo": "path/to/image.ntf.index.json"},
)
```
