# Introduction

## Why This Library

Engineers and scientists working with geospatial imagery face a recurring set of
problems: heavyweight dependencies that complicate deployment, format libraries that
expose a convenient subset of the spec but hide the details you actually need, and
cloud-native tooling that falls apart when the data is compressed. osml-imagery-io
was built to address these problems directly.

Three engineering decisions drove the architecture:

### Deep compliance with geospatial standards

The goal is to be a high performance reference implementation of the NITF/NSIF and 
GeoTIFF specifications suitable for production workloads. The library currently supports 
read/write for all NITF block interleave modes (IMODE == B, P, R, S), the most widely
used IC compression codes — uncompressed, JPEG 2000, HTJ2K, and JPEG DCT — with masked variants for sparse imagery, and a data-driven Tagged Record Extension decoder/encoder
that ships with definitions for all publicly available TREs. Data Extension Segments are first-class objects you can read, create, and modify. SICD and SIDD SAR metadata is 
available through the NITF implementation.

For GeoTIFF: OGC GeoTIFF 1.1 GeoKeys, Cloud Optimized GeoTIFF with correct
NewSubfileType, and predictor support for Deflate and LZW compression.

The goal is to give you direct access to individual segments, TRE fields, block
masks, compression parameters, TIFF tags, and GeoKeys — the format-level detail
that matters when the specifics of how data was collected, modified, and encoded
are as important as the pixel values themselves.

### Minimal, self-contained deployment

`pip install` with no system libraries, no C toolchain, no conda. Self-contained
wheels bundle the performant Rust core and C codecs. This is both a usability 
benefit — easy to get started — and a production benefit. Fewer dependencies mean 
a smaller attack surface and fewer packages to patch, which matters for 
containerized environments, automation pipelines, and security-sensitive deployments.

### Cloud-native tile access that works with compressed data

Existing Zarr codecs cannot decode compressed tiles from TIFF or JPEG 2000 files
because they lack support for format-specific context. TIFF predictor metadata lives
outside the tile data. JPEG 2000 tile-parts can be scattered across non-contiguous
byte ranges. These are not edge cases — they are the norm for satellite imagery.

The library provides VirtualiZarr parsers, format-aware Zarr v3 codecs, and a
scatter-gather filesystem extension that solve these specific problems. Generate a
lightweight tile index once, and the Zarr / xarray / Dask ecosystem treats the file
as a native chunked array. Multi-resolution pyramids follow the
[GeoZarr multiscales convention](https://geozarr.org/) being developed by the OGC
GeoZarr Standards Working Group.

## The Dataset Model

The library organizes files around a dataset model inspired by the
[SpatioTemporal Asset Catalog (STAC)](https://stacspec.org/en) specification. Each
file you open is a **dataset** (analogous to a STAC Item), and the segments inside
it — images, text, graphics, data extensions — are **assets** (analogous to STAC
Assets with typed roles).

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
    lat_off = rpc["LAT_OFF"]
    line_scale = rpc["LINE_SCALE"]

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

This structural alignment makes it straightforward to publish datasets as STAC Items
and integrate with the broader cloud-native geospatial ecosystem.

## Supported Formats

### File Formats

| Format | Read | Write | Notes |
|--------|------|-------|-------|
| NITF 2.1 / NSIF 1.0 (JBP) | ✅ | ✅ | Primary implementation |
| NITF 2.0 | 🚧 | ❌ | In progress; legacy format |
| TIFF | ✅ | ✅ | Tiled and stripped layouts; includes GeoTIFF metadata and COG support |
| PNG | ✅ | ✅ | Lossless; Deflate compression |
| JPEG 2000 (.j2k, .jp2) | ✅ | ✅ | Lossless and lossy; multi-resolution decode; via OpenJPEG |
| JPEG (.jpg, .jpeg) | ✅ | ✅ | Lossy; 8-bit; 1 or 3 bands |

NITF is the primary container format for defense and intelligence imagery. SICD and SIDD
(SAR complex and derived data) are supported directly through the NITF implementation —
the sensor-independent XML metadata is available from the data extension segments.
High Resolution Elevation (HRE) products (`.hr1` through `.hr8`) are NITF-based raster
elevation datasets and are auto-detected from their file extensions.

TIFF support includes GeoTIFF metadata (GeoKeys, ModelTiepoint, ModelPixelScale,
ModelTransformation) and Cloud Optimized GeoTIFF (COG) conventions. These are not
separate formats — GeoTIFF adds geospatial tags to a standard TIFF, and COG defines
a layout convention for efficient range-request access.

### NITF Image Compression

| IC Codes | Compression | Read | Write | Notes |
|----------|-------------|------|-------|-------|
| NC / NM | Uncompressed | ✅ | ✅ | All pixel types, all interleave modes (B, P, R, S). NM adds block mask and pad pixel support |
| C8 / M8 | JPEG 2000 | ✅ | ✅ | Lossy and lossless; multi-resolution decode; 1-38 bit depth; via OpenJPEG |
| CD / MD | HTJ2K (JPEG 2000 Part 15) | ✅ | ✅ | High-Throughput JPEG 2000; same capabilities as C8/M8 |
| C3 / M3 | JPEG DCT | ✅ | ✅ | 8-bit lossy only; mono, RGB, YCbCr601. 12-bit not supported (use C8 for >8-bit) |
| I1 | JPEG downsampled | ✅ | ✅ | Single-block thumbnail; 2048×2048 max dimension |
| C4 / M4 | Vector Quantization (VQ) | ❌ | ❌ | On roadmap. Legacy codebook-based compression (MIL-STD-188-199) |
| CC / MC | ZLIB/DEFLATE | ❌ | ❌ | On roadmap. Used for floating-point scientific data |
| C5 / M5 | JPEG Lossless | ❌ | ❌ | On roadmap. Predictive coding, 2-16 bit |
| C1 / M1 | Bi-Level (Group 3 fax) | ❌ | ❌ | On roadmap. 1-bit imagery for maps and line drawings |
| C7 / M7 | SARZip | ❌ | ❌ | On roadmap. Custom SAR compression per USAF.RDUCE-001 |
| C9 / M9 | H.264/AVC (MIE4NITF) | ❌ | ❌ | On roadmap. Motion imagery |
| CA / MA | H.265/HEVC (MIE4NITF) | ❌ | ❌ | On roadmap. Motion imagery |

### TIFF Compression

| Compression | Read | Write | Notes |
|-------------|------|-------|-------|
| Uncompressed | ✅ | ✅ | All pixel types |
| LZW | ✅ | ✅ | Lossless |
| Deflate (zlib) | ✅ | ✅ | Lossless |
| PackBits | ✅ | ✅ | Lossless; run-length encoding |
| JPEG | ✅ | ✅ | Lossy; 8-bit only |

## Standards

The library is built against the following specifications:

### NITF / Defense Imagery

Maintained by the [NSG Standards Registry (NGA)](https://nsgreg.nga.mil/):

- **Joint BIIF Profile (JBP) v2024.1** — NITF 2.0 / 2.1 / NSIF 1.0 file format
- **STDI-0002 v2024.1** — Tagged Record Extension (TRE) and Data Extension Segment (DES) definitions
- **NGA SICD v1.3.0** — Sensor Independent Complex Data (SAR complex imagery)
- **NGA SIDD v3.0** — Sensor Independent Derived Data (SAR derived products)
- **MIL-STD-188-199** — Vector Quantization (VQ) decompression for NITF imagery
- **NGA.IP.0002 v1.1** — Implementation Profile for High Resolution Elevation (HRE) Products

### TIFF / GeoTIFF

- **TIFF Revision 6.0** — Base TIFF file format (Adobe)
- **OGC GeoTIFF Standard** — Geospatial extensions for TIFF ([OGC](https://www.ogc.org/standards/))
- **OGC Cloud Optimized GeoTIFF (COG)** — Cloud-native GeoTIFF conventions ([OGC](https://www.ogc.org/standards/))

### Image Compression

- **ISO/IEC 15444-1 (JPEG 2000 Part 1)** — Wavelet-based image compression ([ISO/IEC JTC 1](https://www.iso.org/committee/45382.html))
- **ISO/IEC 15444-15 (HTJ2K)** — High-Throughput JPEG 2000 ([ISO/IEC JTC 1](https://www.iso.org/committee/45382.html))
- **ITU-T T.81 / ISO/IEC 10918-1 (JPEG)** — DCT-based lossy image compression ([ITU-T](https://www.itu.int/rec/T-REC-T.81) / [ISO/IEC JTC 1](https://www.iso.org/committee/45382.html))
- **ISO/IEC 15948 (PNG)** — Portable Network Graphics, lossless compression ([ISO/IEC JTC 1](https://www.iso.org/committee/45382.html) / [W3C](https://www.w3.org/TR/png-3/))
