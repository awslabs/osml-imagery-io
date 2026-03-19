# Introduction

## What is osml-imagery-io?

The **osml-imagery-io** library provides low-level read and write access for data encoded
using common geospatial imagery formats. It was built using Rust to provide reliable high 
performance IO routines for both research and production workloads. Python bindings make 
it easy to use within the broader machine learning and data science communities. It supports
memory efficient, tiled access to large images and structured metadata encoded using the 
NITF, SICD, SIDD, TIFF, and GeoTIFF standards. Pixels are returned as NumPy arrays that 
integrate directly with ML frameworks like PyTorch and computer vision libraries like 
OpenCV and scikit-image.

The library is designed to serve as the low level IO layer for the
[OversightML](https://aws.amazon.com/solutions/guidance/processing-overhead-imagery-on-aws/)
ecosystem. It has a deliberately lean set of dependencies — just the Rust core, PyO3
bindings, JPEG, TIFF, and JPEG 2000 libraries, and NumPy — giving it a clean install 
footprint that works reliably in containerized environments, automation pipelines, and 
secure deployment environments where heavyweight dependency chains are a problem.

### How does this compare to other libraries?

Several established libraries work with geospatial imagery. osml-imagery-io occupies a
different niche than most of them.

**GDAL** supports hundreds of raster formats and provides reprojection, resampling,
virtual rasters, and a broad GIS-oriented abstraction layer. It is the right tool when
you need wide format coverage or prebuilt GIS operations but it is a heavyweight 
dependency that can be constraining for image scientists. Some IO operations have 
degraded performance exasperated by layers of virtual file abstractions.
osml-imagery-io operates at a lower level, giving direct access to individual segments,
TRE fields, block masks, compression parameters, TIFF tags, and GeoKeys — the kind of 
fine-grained control the image science community needs when the specifics of how data 
was collected, modified, and encoded matter as much as the pixel values themselves.

**SarPy** was a Python library from NGA for reading and writing SAR complex data in
SICD and SIDD formats. NGA
[ended support for SarPy](https://github.com/ngageoint/sarpy) in January 2026, pointing
users to a relatively imature [SarKit](https://github.com/ValkyrieSystems/sarkit) as its successor.
Instead of a dedicated SAR implementation, osml-imagery-io supports SICD and SIDD directly through
its robust NITF implementation. The sensor independent XML metadata is available from the data segment
— making it easy to integrate with other libraries that provide the SAR sensor models, image projections, and SAR processing algorithms (e.g.
[osml-imagery-toolkit](https://github.com/aws-solutions-library-samples/osml-imagery-toolkit)).

### What formats are supported?

#### Format Support

| Format | Read | Write | Status |
|--------|------|-------|--------|
| NITF 2.1 (JBP) | ✅ | ✅ | Primary implementation |
| NSIF 1.0 | ✅ | ✅ | Structurally identical to NITF 2.1 |
| NITF 2.0 | 🚧 | ❌ | In progress; legacy format |
| NSIF 1.1 | 🚧 | ❌ | In progress |
| SICD (via NITF) | ✅ | ✅ | Complex SAR data; pixel access and DES extraction for XML metadata |
| SIDD (via NITF) | ✅ | ✅ | Derived SAR products; pixel access and DES extraction for XML metadata |
| TIFF | ✅ | ✅ | Tiled and stripped; LZW, Deflate, PackBits, uncompressed |
| GeoTIFF | ✅ | ✅ | GeoKeys, ModelTiepoint, ModelPixelScale, ModelTransformation |
| Cloud Optimized GeoTIFF (COG) | 🚧 | 🚧 | In progress |

#### NITF Image Compression

| IC Codes | Compression Type | Read | Write | Notes |
|----------|-----------------|------|-------|-------|
| NC / NM | Uncompressed | ✅ | ✅ | All pixel types, all interleave modes (B, P, R, S). NM adds block mask and pad pixel support |
| C8 / M8 | JPEG 2000 | ✅ | ✅ | Lossy and lossless; multi-resolution decode; 1-38 bit depth; via OpenJPEG |
| CD / MD | HTJ2K (JPEG 2000 Part 15) | ✅ | ✅ | High-Throughput JPEG 2000; same capabilities as C8/M8 |
| C3 / M3 | JPEG DCT | ✅ | ✅ | 8-bit lossy only; mono, RGB, YCbCr601. 12-bit JPEG is not supported (requires libjpeg12; use C8 for >8-bit) |
| I1 | JPEG downsampled | ✅ | ✅ | Single-block thumbnail; 2048×2048 max dimension |
| C4 / M4 | Vector Quantization (VQ) | ❌ | ❌ | On roadmap. Legacy codebook-based compression (MIL-STD-188-199) |
| CC / MC | ZLIB/DEFLATE | ❌ | ❌ | On roadmap. Used for floating-point scientific data |
| C5 / M5 | JPEG Lossless | ❌ | ❌ | On roadmap. Predictive coding, 2-16 bit |
| C1 / M1 | Bi-Level (Group 3 fax) | ❌ | ❌ | On roadmap. 1-bit imagery for maps and line drawings |
| C7 / M7 | SARZip | ❌ | ❌ | On roadmap. Custom SAR compression per USAF.RDUCE-001 |
| C9 / M9 | H.264/AVC (MIE4NITF) | ❌ | ❌ | On roadmap. Motion imagery |
| CA / MA | H.265/HEVC (MIE4NITF) | ❌ | ❌ | On roadmap. Motion imagery |

### What standards were used?

The functionality described in this guide is based on the following specifications:

- **Joint BIIF Profile (JBP) v2024.1** — NITF 2.0 / 2.1 / NSIF 1.0 file format
- **STDI-0002 v2024.1** — Tagged Record Extension (TRE) and Data Extension Segment (DES) definitions
- **NGA SICD v1.3.0** — Sensor Independent Complex Data (SAR complex imagery)
- **NGA SIDD v3.0** — Sensor Independent Derived Data (SAR derived products)
- **MIL-STD-188-199** — Vector Quantization (VQ) decompression for NITF imagery
- **TIFF Revision 6.0** — Base TIFF file format
- **OGC GeoTIFF Standard** — Geospatial extensions for TIFF
- **OGC Cloud Optimized GeoTIFF (COG)** — Cloud-native GeoTIFF conventions

These standards are maintained by the [NSG Standards Registry (NGA)](https://nsgreg.nga.mil/)
and the [Open Geospatial Consortium (OGC)](https://www.ogc.org/standards/).