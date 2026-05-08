# OversightML Imagery IO

[![CI](https://github.com/awslabs/osml-imagery-io/actions/workflows/ci.yml/badge.svg)](https://github.com/awslabs/osml-imagery-io/actions/workflows/ci.yml)
[![Docs](https://github.com/awslabs/osml-imagery-io/actions/workflows/docs.yml/badge.svg)](https://awslabs.github.io/osml-imagery-io/)
[![PyPI](https://img.shields.io/pypi/v/osml-imagery-io)](https://pypi.org/project/osml-imagery-io/)
[![Python](https://img.shields.io/badge/Python-3.9%2B-blue)](https://www.python.org/)
[![Rust](https://img.shields.io/badge/Rust-2021_edition-orange)](https://www.rust-lang.org/)

Flexible read/write for NITF, GeoTIFF, JPEG 2000, DTED, and more. Performant cloud-native
tile access with no complex dependencies. Built in Rust for speed with Python APIs for easy integration
with the latest ML frameworks and data science environments.

<p align="center">
  <img src="docs/_static/images/overview.png" alt="OversightML Imagery IO Overview Slide" width="720">
</p>

## Why This Library

- **`pip install` and go** — self-contained wheels with a Rust core and bundled codecs.
  No system libraries, no C toolchain, no conda. Minimal dependencies also mean a
  smaller attack surface and fewer packages to patch — a real consideration for
  production and security-sensitive deployments.

- **Specification-compliant NITF read/write** — supports all four IMODE interleave
  modes (B, P, R, S) and compression types including uncompressed, JPEG 2000, HTJ2K,
  and JPEG DCT — with masked variants for sparse imagery. An extensible, data-driven
  TRE parser ships with definitions for all publicly available Tagged Record Extensions,
  and Data Extension Segments are first-class objects you can read, create, and modify.
  SICD and SIDD SAR data are supported through the NITF implementation.

- **Cloud-native tile access that works with compressed data** — existing Zarr codecs
  cannot decode compressed tiles from TIFF or JPEG 2000 files because they lack support
  for format-specific context like TIFF predictor metadata and JPEG 2000 multi-tile-part
  reassembly. This library provides custom Zarr v3 codecs that solve these problems,
  along with VirtualiZarr parsers and a scatter-gather filesystem for non-contiguous
  byte ranges — making the promise of virtual Zarr access work with real compressed
  geospatial imagery.

- **Simple when you want it, deep when you need it** — `imread` / `imsave` / `tiles`
  for common tasks; full low-level API for format-specific control over segments,
  metadata, tiling, masks, and compression parameters.

## What This Library is Not

This is not a library of image operations or photogrammetry routines — there are no
orthorectification pipelines, pan-sharpening filters, or coordinate transforms here.
The goal is to get pixels from geospatial imagery formats into a NumPy array as
efficiently as possible so you can feed them into your ML framework, image processing
toolkit, or analysis pipeline of choice.

## Quick Start

```bash
pip install osml-imagery-io
```

```python
from aws.osml.io import imread, imsave, iminfo, tiles

# Inspect metadata without reading pixels
info = iminfo("image.ntf")
print(info.metadata["RPC00B"])     # Rational polynomial coefficients
print(info.metadata["STDIDC"])     # Acquisition context

# Read an image as a NumPy array, careful this is the whole image
pixels = imread("image.ntf")                              # shape: (bands, height, width)

# Read a single windowed region, much more practical
chip = imread("image.ntf", window=(100, 200, 256, 256))   # (x, y, width, height)

# Save to any supported format — inferred from extension
imsave("output.tif", chip)

# Iterate over tiles for memory-efficient processing of large images
for tile in tiles("large_image.tif", tile_size=(256, 256)):
    process(tile.data)
```

## Full Format Access

The convenience functions cover common tasks. When you need to work with the format
itself — multi-segment NITF files, TRE fields, block masks, resolution levels — the
full API is there.

```python
from aws.osml.io import IO

with IO.open("satellite_scene.ntf", "r") as dataset:
    # Navigate all segments in the file
    for key in dataset.get_asset_keys():
        print(key)  # "image:0", "image:1", "text:0", "data:0", ...

    image = dataset.get_asset("image:0")
    meta = image.metadata.as_dict()

    # Rational polynomial coefficients for geopositioning
    rpc = meta["RPC00B"]
    # TODO: Use coefficients to construct sensor model ...

    # Acquisition context — mission, pass, date
    stdidc = meta["STDIDC"]
    acq_date = stdidc["ACQ_DATE"]
    mission = stdidc["MISSION"]

    # Exploitation usability — GSD, sun angles, obliquity
    use00a = meta["USE00A"]
    gsd = use00a["MEAN_GSD"]
    sun_el = use00a["SUN_EL"]

    # Read a specific block at a reduced resolution level
    block = image.get_block(4, 7, resolution_level=2)
```

The dataset model is inspired by the
[SpatioTemporal Asset Catalog (STAC)](https://stacspec.org/en) specification — each
dataset maps to a STAC Item, assets map to STAC Assets with typed roles, and the
structural alignment makes it straightforward to publish datasets as STAC Items.

## Format Specific Features

### NITF / NSIF

| Capability | Details |
|------------|---------|
| Versions | NITF 2.1, NSIF 1.0 |
| Compression Options (IC) | NC, NM (uncompressed/masked), C8, M8 (JPEG 2000), CD, MD (HTJ2K), C3, M3, I1 (JPEG DCT) |
| Interleave (IMODE) | B (band), P (pixel), R (row), S (sequential) |
| TRE parser | Data-driven with definitions for all publicly available TREs |
| Data Extensions | Read and write DES payloads (SICD/SIDD XML, TRE overflow, etc.) |
| Pixel types | 8/16/32-bit unsigned, 16/32-bit signed, 32/64-bit float, 32/64-bit complex |
| Block masks | Sparse imagery via masked compression modes (NM, M8, MD, M3) |
| Multi-segment | Multiple image, text, graphic, and data segments per file |
| Multi-file pyramids | R-set resolution pyramids across separate files |

### TIFF / GeoTIFF / COG

| Capability | Details |
|------------|---------|
| Compression Options | Uncompressed, Deflate and LZW, — with horizontal differencing predictor |
| Tiling | Configurable tile dimensions (multiples of 16) |
| GeoKeys | OGC GeoTIFF 1.1 — CRS, pixel scale, tiepoints, affine transforms |
| COG | Cloud Optimized GeoTIFF with overview IFDs and correct NewSubfileType |
| Pixel types | 8/16/32-bit unsigned, 16/32-bit signed, 32/64-bit float |

### DTED (Digital Terrain Elevation Data)

| Capability | Details |
|------------|---------|
| Levels | 0, 1, 2, ... |
| Pixel type | Single-band Int16 (signed-magnitude encoding) |
| Extensions | `.dt0`–`.dt5`, `.avg`, `.min`, `.max` |
| Datum | WGS84 horizontal, MSL (EGM96) vertical |
| Zarr codec | Overlap-aware edge trimming for seamless multi-cell mosaics |

### Other Formats

JP2, JPEG, and PNG file formats are also supported for read and write. These lack robust metadata,
but they appear frequently as interchange formats for tiles and quick-look products alongside NITF 
and GeoTIFF imagery.

## Cloud-Native Access

Access tiles from NITF, TIFF, and JPEG 2000 files in S3 as virtual Zarr arrays — no
format conversion needed. Generate a lightweight tile index once, and the
[Zarr](https://zarr.dev/) / [xarray](https://xarray.dev/) / [Dask](https://www.dask.org/)
ecosystem treats the file as a native chunked array.

The library provides three components that together make this work for real compressed
geospatial data:

- **VirtualiZarr parsers** that scan NITF, TIFF, and JPEG 2000 files to build
  multi-resolution tile indexes (Kerchunk JSON/Parquet)
- **Format-aware Zarr v3 codecs** that handle the decoding problems standard codecs
  cannot — NITF endian swap and interleave conversion, JPEG 2000 tile-part
  reconstruction from non-contiguous byte ranges, TIFF predictor reversal using
  metadata from outside the tile data, and DTED boundary-post trimming for seamless
  multi-cell elevation mosaics without preprocessing
- **MultiReferenceFileSystem** — an fsspec extension that adds scatter-gather I/O for
  JPEG 2000 codestreams where a single tile's data is scattered across multiple
  non-contiguous locations in the file (common with RLCP/RPCL progression orders in
  satellite imagery)

Multi-resolution pyramids follow the
[GeoZarr multiscales convention](https://geozarr.org/) being developed by the OGC
GeoZarr Standards Working Group.

```python
from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

# Build a tile index (works for NITF, TIFF, JPEG 2000)
parser = OversightMLParser(local_paths="image.ntf")
store = parser(url="s3://my-bucket/imagery/image.ntf")
write_tile_index(store, "image.ntf.tile_index.json")
```

```python
import zarr
from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
from zarr.storage._fsspec import FsspecStore

# Open tiles directly from S3 — only fetches the bytes you need
fs = MultiReferenceFileSystem(
    fo="s3://my-bucket/imagery/image.ntf.tile_index.json",
    asynchronous=True,
    remote_options={"asynchronous": True},
    skip_instance_cache=True,
)
store = FsspecStore(fs=fs, read_only=True, path="")
root = zarr.open_group(store, mode="r", zarr_format=2)
tile = root["0/data"][0:3, 768:1024, 1024:1280]
```

See the [Cloud Imagery Access guide](https://awslabs.github.io/osml-imagery-io/user-guide/zarr-codecs.html) for the full
workflow.

## Documentation

Full documentation is published at **[awslabs.github.io/osml-imagery-io](https://awslabs.github.io/osml-imagery-io/)**.

- [Getting Started](https://awslabs.github.io/osml-imagery-io/user-guide/getting-started.html) — install, read, write, tile in a few lines
- [User Guide](https://awslabs.github.io/osml-imagery-io/user-guide/) — datasets, metadata, block access, writing, cloud access
- [API Reference](https://awslabs.github.io/osml-imagery-io/api/) — full Python API documentation
- [Design Documents](https://awslabs.github.io/osml-imagery-io/design/) — architecture and design decisions

## Development

```bash
conda env create -f environment.yml
conda activate osml-imagery-io-dev
source scripts/setup-dev-env.sh
maturin develop
pytest
cargo test
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Security

Please do not open a public GitHub issue to report security concerns. Follow the
reporting mechanisms described in [SECURITY](SECURITY.md).

## License

This project is licensed under the Apache 2.0 License. See the [LICENSE](LICENSE) file.
