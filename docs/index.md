# osml-imagery-io

High-performance geospatial image format codecs with Python bindings, built in Rust.

Flexible read/write for NITF, GeoTIFF, DTED, JPEG 2000, and more. Performant cloud-native
tile access with no complex dependencies. Built in Rust for speed with Python APIs for easy
integration with the latest ML frameworks and data science environments.

## Key Features

- **Self-contained wheels** — Rust core with bundled codecs, no system libraries or C toolchain required
- **Specification-compliant NITF read/write** — all four IMODE interleave modes, JPEG 2000, HTJ2K, JPEG DCT, and uncompressed compression with masked variants for sparse imagery
- **GeoTIFF / COG support** — tiled access with Deflate, LZW, and horizontal differencing predictor; Cloud Optimized GeoTIFF with overview IFDs
- **DTED elevation data** — levels 0–5 with signed-magnitude encoding and overlap-aware Zarr codec for seamless multi-cell mosaics
- **Cloud-native tile access** — VirtualiZarr parsers and format-aware Zarr v3 codecs for direct access to compressed tiles in S3
- **Data-driven TRE/DES parser** — definitions for all publicly available Tagged Record Extensions; Data Extension Segments as first-class objects
- **Band-sequential NumPy arrays** — channels-first layout compatible with PyTorch and ML frameworks
- **Simple and deep APIs** — `imread` / `imsave` / `tiles` for common tasks; full low-level API for format-specific control

## Supported Formats

| Format | Read | Write | Details |
|--------|------|-------|---------|
| NITF 2.1 / NSIF 1.0 | Yes | Yes | NC, NM, C8, M8, CD, MD, C3, M3, I1 compression; all IMODE variants |
| TIFF / GeoTIFF / COG | Yes | Yes | Uncompressed, Deflate, LZW; configurable tiling; OGC GeoTIFF 1.1 |
| DTED | Yes | Yes | Levels 0–5; `.dt0`–`.dt5`, `.avg`, `.min`, `.max` extensions |
| JPEG 2000 | Yes | Yes | Standalone JP2 files |
| JPEG | Yes | Yes | DCT baseline and progressive |
| PNG | Yes | Yes | Lossless interchange |

```{toctree}
:maxdepth: 2
:caption: Contents

user-guide/index
design/index
api/index
codecs/index
performance
roadmap/index
```
