# OversightML Imagery IO

[![CI](https://github.com/awslabs/osml-imagery-io/actions/workflows/ci.yml/badge.svg)](https://github.com/awslabs/osml-imagery-io/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/python-3.9%2B-blue)](https://www.python.org/)
[![Rust](https://img.shields.io/badge/rust-2021_edition-orange)](https://www.rust-lang.org/)

High-performance read/write library for geospatial imagery formats, built in Rust with Python bindings.

OSML Imagery IO gives you direct, tiled access to NITF, GeoTIFF, JPEG 2000, JPEG, and PNG imagery — the formats that power defense, intelligence, and commercial satellite workflows. Pixels come back as NumPy arrays ready for PyTorch, OpenCV, or any ML pipeline.

## Key Features

- **Read and write** NITF 2.1, TIFF/GeoTIFF, Cloud Optimized GeoTIFF (COG), JPEG 2000, JPEG, and PNG
- **SICD/SIDD support** for SAR complex and derived data via the NITF implementation
- **Cloud-native access** through [Zarr](https://zarr.dev/), [VirtualiZarr](https://github.com/zarr-developers/VirtualiZarr), and [Kerchunk](https://fsspec.github.io/kerchunk/) integration — access tiles in S3 via HTTP range requests without downloading entire files
- **Rust performance with Python convenience** — a lean native core with PyO3 bindings and minimal dependencies
- **Fine-grained control** over segments, TRE fields, block masks, compression parameters, TIFF tags, and GeoKeys
- **Memory-efficient tiled access** to multi-GB imagery with block-level reads

## Cloud Imagery Access

Existing geospatial imagery sitting in S3 can be accessed as virtual Zarr arrays — no format conversion needed. The library provides:

- **VirtualiZarr parsers** that scan NITF, TIFF, and JPEG 2000 files to build lightweight tile indexes (Kerchunk JSON/Parquet)
- **Custom Zarr v3 codecs** that decode format-specific tile data (endian swap, interleave conversion, JPEG 2000 tile-part reconstruction)
- **MultiReferenceFileSystem** — an fsspec extension that handles scatter-gather I/O for JPEG 2000 codestreams with non-contiguous tile-parts

Generate a tile index once, upload it alongside your imagery, and the Zarr ecosystem ([xarray](https://xarray.dev/), [Dask](https://www.dask.org/)) treats the file as a native chunked array.

```python
from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

# Index a file (works for NITF, TIFF, JPEG 2000)
parser = OversightMLParser(local_path="image.ntf")
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
tile = root["image_segment_0"][0:3, 768:1024, 1024:1280]
```

## Quick Start

### Installation

```bash
pip install osml-imagery-io
```

### Reading Imagery

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    image_keys = dataset.get_asset_keys(asset_type="image")
    image = dataset.get_asset(image_keys[0])
    block = image.get_block(0, 0, resolution_level=0)
    print(block.shape)
```

### Writing Imagery

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
import numpy as np

metadata = BufferedMetadataProvider()
metadata.set("IC", "NC")
metadata.set("IMODE", "B")
metadata.set("NPPBH", "256")
metadata.set("NPPBV", "256")

image_data = np.zeros((3, 512, 512), dtype=np.uint8)
provider = BufferedImageAssetProvider.create(
    key="image_0", num_columns=512, num_rows=512, num_bands=3,
    block_width=256, block_height=256, pixel_type=PixelType.UInt8, metadata=metadata,
)
provider.set_full_image(image_data)

with IO.open(["output.ntf"], "w", "nitf") as writer:
    writer.add_asset("image_segment_0", provider)
```

## Supported Formats

| Format | Read | Write |
|--------|------|-------|
| NITF 2.1 / NSIF 1.0 | ✅ | ✅ |
| SICD / SIDD (via NITF) | ✅ | ✅ |
| TIFF / GeoTIFF / COG | ✅ | ✅ |
| JPEG 2000 (.j2k, .jp2) | ✅ | ✅ |
| JPEG | ✅ | ✅ |
| PNG | ✅ | ✅ |

## Development

```bash
conda env create -f environment.yml
conda activate osml-imagery-io-dev
source scripts/setup-dev-env.sh
maturin develop
pytest
cargo test
```

## Documentation

Full documentation is available at [docs/](docs/index.md), including API reference, user guides, and design documents.

## Contributing

This project welcomes contributions and suggestions. If you would like to submit a pull request, see our [Contribution Guide](CONTRIBUTING.md) for more information. We kindly ask that you do not open a public GitHub issue to report security concerns. Instead follow reporting mechanisms described in [SECURITY](SECURITY.md).

## License

This library is licensed under the Apache 2.0 License. See the [LICENSE](LICENSE) file.
