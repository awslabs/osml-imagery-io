# Product Overview

OversightML's Imagery IO library (`osml-imagery-io`) is a Rust-core, Python-first library for reading and writing geospatial imagery. The goal is to get pixels from format-specific files into NumPy arrays as efficiently as possible — not to provide image processing, photogrammetry, or coordinate-transform pipelines.

## Supported Formats

- **NITF 2.1 / NSIF 1.0** — full specification-compliant read/write including all IMODE interleaves (B, P, R, S), compression types (NC, NM, C8/M8 JPEG 2000, CD/MD HTJ2K, C3/M3/I1 JPEG DCT), masked (sparse) variants, multi-segment files, R-set multi-file pyramids, and a data-driven TRE/DES parser covering all publicly available Tagged Record Extensions.
- **TIFF / GeoTIFF / Cloud Optimized GeoTIFF** — tiled read/write with Deflate and LZW (with horizontal differencing predictor), OGC GeoTIFF 1.1 GeoKeys, and COG overviews.
- **JPEG 2000 (incl. HTJ2K)**, **JPEG DCT**, and **PNG** — read/write support, primarily as interchange and quick-look formats alongside NITF and GeoTIFF.
- **SICD / SIDD** (SAR complex and derived data) — supported via the NITF implementation with DES payloads.

## Cloud-Native Access

The library provides three components that make compressed geospatial imagery work with the Zarr ecosystem:

- Custom **Zarr v3 codecs** that handle NITF endian/interleave conversion, JPEG 2000 multi-tile-part reassembly, and TIFF predictor reversal.
- **VirtualiZarr parsers** that build multi-resolution tile indexes (Kerchunk JSON/Parquet) for NITF, TIFF, and JPEG 2000 files.
- **MultiReferenceFileSystem** — an fsspec extension providing scatter-gather I/O for non-contiguous byte ranges (needed for JPEG 2000 codestreams with RLCP/RPCL progression orders).

Multi-resolution pyramids follow the OGC GeoZarr multiscales convention.

## Python API Shape

- Simple convenience functions: `imread`, `imsave`, `iminfo`, `tiles`.
- Full low-level API: `IO.open`, `get_asset_keys`, `get_asset`, segment/block access, block masks, resolution levels, encoding hints via metadata.
- Dataset model inspired by the STAC specification — each dataset maps to a STAC Item, assets map to typed STAC Assets.

## Target Users

- Developers working with geospatial imagery in ML and data-science pipelines.
- OversightML ecosystem users.
- Remote sensing and GIS applications that need compressed, cloud-native tile access without a heavyweight C toolchain.

## Repository

https://github.com/awslabs/osml-imagery-io
