# osml-imagery-io

High-performance geospatial image format codecs with Python bindings, built in Rust.

osml-imagery-io provides read and write support for NITF (National Imagery Transmission Format) 2.0/2.1 files with JPEG 2000, HTJ2K, and JPEG DCT compression, plus a data-driven binary parser for TRE and DES extensions. GeoTIFF support is planned.

## Key Features

- Blocked (tiled) image access for efficient processing of large imagery
- JPEG 2000, HTJ2K, JPEG DCT, and uncompressed codecs
- Data-driven binary parser for NITF headers, TREs, and DES
- Masked (sparse) image support
- Band-sequential (channels-first) NumPy arrays compatible with PyTorch
- Encoding hints via metadata for flexible output control

```{toctree}
:maxdepth: 2
:caption: Contents

getting-started
user-guide/index
design/index
api/index
roadmap/index
```
