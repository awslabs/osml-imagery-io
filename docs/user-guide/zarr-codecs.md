# Cloud Imagery Access via Zarr

## The Challenge of Working with Large Geospatial Datasets

Geospatial imagery archives are massive. Government and commercial satellite programs
produce hundreds of petabytes of imagery, growing at tens to hundreds of terabytes per 
day. The traditional workflow of downloading entire multi-GB files before accessing 
any pixels is giving way to distributed access patterns. Machine Learning inference,
tile-level analytics running on data lakehouse architectures, and interactive 
visualizations, all benefit from services that can quickly fetch only the compressed 
bytes for the tiles they care about, decode them, and move on.

The challenge is that some well established geospatial formats were designed before 
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

This library provides the two components needed to bridge existing imagery formats into the
Zarr ecosystem: VirtualiZarr parsers for tile indexing and a set of custom codecs.

### Tile Indexing with VirtualiZarr Parsers

The library provides `OversightMLParser`, a single VirtualiZarr parser that makes it possible
to access any imagery format supported by this library as a virtual dataset. The parser scans
a file using the library's native reader and walks each image segment, determining the byte
offset and length of every tile relative to the start of the file. 

How tile boundaries are calculated depends on the format and compression:

- Uncompressed tiles have fixed sizes. Offsets are computed arithmetically from the
  image dimensions, pixel type, and interleave mode. Formats that support sparse tile
  arrays (ex: NITF masked images) may have existing metadata that contains these 
  boundaries.
- JPEG tiles are length-prefixed. The parser scans the length headers sequentially
  to locate each tile boundary.
- JPEG 2000 codestreams use SOT (Start of Tile-part) markers that record the byte
  offset and length of each tile-part. If the codestream contains a TLM (Tile-part
  Length Marker) in its main header, the full tile index is available immediately
  without scanning. If no TLM is present, the parser performs a sequential SOT scan.
- TIFF files store tile offsets and byte counts in IFD tags (`TileOffsets` and
  `TileByteCounts`). The parser reads these directly.

The parsers produce the `ManifestStore` — a virtual Zarr store backed by chunk
references into the source file. This can be serialized to create a format-agnostic
tile index.

### Custom Codecs

The tile index tells fsspec where to fetch each tile's bytes, but those bytes are
still in the source format's native encoding. Standard Zarr codecs cannot decode
them. This library registers three Zarr v3 codecs that handle the format-specific
decoding:

`JbpBlockCodec` handles uncompressed NITF tiles. NITF raw pixel data uses
big-endian byte order and one of four interleave modes (band-interleaved by pixel,
band-interleaved by line, band-interleaved by block, or band-sequential). The
codec performs the endian swap and interleave conversion to produce standard NumPy
arrays.

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
100–500 bytes) in the codec configuration stored in `.zarray`. At decode time the
codec reconstructs a minimal single-tile codestream on the fly:

```
[main_header bytes] + [tile-part bytes] + [EOC marker]
```

OpenJPEG receives what looks like a normal single-tile codestream and decodes it.
This approach has precedent in the JPEG 2000 ecosystem. JPIP (the JPEG 2000
Interactive Protocol) streams individual tile-parts to clients that already hold
the main header. Because the codec operates on the J2K codestream directly, it 
works for standalone `.j2k`/`.jp2` files and for J2K codestreams embedded in 
container formats like NITF.

All three codecs are registered with the Zarr codec registry via Python entry
points. They use URI-based names per the Zarr v3 specification to avoid conflicts
with existing codecs:

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
available locally for indexing. The `url` parameter sets the S3 path that tile
references will point to.

```{note}
Index generation requires the `virtualizarr` optional dependency:
`pip install osml-imagery-io[virtualizarr]`
```

```python
from aws.osml.io.virtualizarr_parsers import OversightMLParser

# Generate index from a local file — works for NITF, JPEG 2000, TIFF, GeoTIFF
parser = OversightMLParser(local_path="local/image.ntf")
manifest_store = parser(url="s3://my-bucket/imagery/image.ntf")

# Convert to xarray virtual dataset and save as Kerchunk Parquet
vds = manifest_store.to_virtual_dataset()
vds.vz.to_kerchunk("image.ntf.index.parquet", format="parquet")
```

Upload the index to S3 alongside the image. This assumes the image itself is
already residing in the cloud.

```python
import boto3
import os

s3 = boto3.client("s3")

# Upload each file in the Parquet directory
for root, dirs, files in os.walk("image.ntf.index.parquet"):
    for filename in files:
        local_path = os.path.join(root, filename)
        s3_key = f"imagery/{os.path.relpath(local_path, '.')}"
        s3.upload_file(local_path, "my-bucket", s3_key)
```

### Step 2: Open and access tiles

Codec registration happens automatically when the package is installed with the
`zarr` extras (`pip install osml-imagery-io[zarr]`). No explicit import is needed.
When you slice into the dataset, fsspec issues HTTP range requests for only the
bytes backing the requested tiles and the registered codec decodes them into NumPy
arrays.

```python
import xarray as xr

ds = xr.open_zarr(
    "reference://",
    storage_options={
        "fo": "s3://my-bucket/imagery/image.ntf.index.parquet/",
        "remote_protocol": "s3",
        "remote_options": {"profile": "my-profile"},
    },
)

# Read a single tile region
tile = ds["image_segment_0"][0:3, 768:1024, 1024:1280].values
print(tile.shape)  # (3, 256, 256)
print(tile.dtype)  # uint8
```

AWS credentials can also be provided through environment variables
(`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_PROFILE`) or any other
method supported by boto3 and fsspec.
