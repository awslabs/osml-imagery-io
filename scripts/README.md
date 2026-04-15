# Scripts

Example scripts demonstrating common workflows with the osml-imagery-io library. These are intended as user-facing examples that exercise key features of the Python API.

## User-Facing Scripts

### Discover & Inspect

| Script | Description |
|--------|-------------|
| `survey_datasets.py` | Scan a directory for image files and print a summary table of formats, dimensions, compression, etc. |
| `describe_dataset.py` | Dump detailed information about a single dataset file — assets, metadata, SICD/SIDD XML content. |

```bash
# Survey all images in a directory
python scripts/survey_datasets.py data/unit

# Recursively scan subdirectories
python scripts/survey_datasets.py data/integration -r

# Describe a NITF file with full metadata
python scripts/describe_dataset.py image.ntf --metadata
```

### Read & Chip

| Script | Description |
|--------|-------------|
| `chip_image_local.py` | Extract a rectangular region from a local file (NITF, TIFF, PNG) and save as PNG. |
| `chip_image_zarr.py` | Extract a region via a Zarr tile index (local or S3). Supports multiscale indexes with `--level`. |

```bash
# Chip from a local NITF file
python scripts/chip_image_local.py input.ntf chip.png --bbox 0 0 512 512

# Chip from a specific asset
python scripts/chip_image_local.py input.ntf chip.png --bbox 0 0 512 512 --asset image:1

# Chip from a Zarr tile index at resolution level 2
python scripts/chip_image_zarr.py index.json chip.png --bbox 0 0 256 256 --level 2

# Chip from an S3-hosted tile index with timing info
python scripts/chip_image_zarr.py s3://bucket/index.parquet chip.png --bbox 0 0 1024 1024 -v
```

### Generate & Write

| Script | Description |
|--------|-------------|
| `generate_synthetic_image.py` | Create a single-level test image with checkerboard pattern and tile IDs. Supports NITF, TIFF, PNG, J2K, JPEG with various compression modes. |
| `generate_synthetic_image_pyramid.py` | Create a multi-resolution image pyramid as a COG (single TIFF with overviews) or NITF R-set (separate files per level). Each tile is labeled with its resolution level and grid coordinates. |
| `generate_tile_index.py` | Build a Zarr tile index (JSON or Parquet) from a local imagery file for cloud-native access via fsspec/Zarr. |

```bash
# Generate a 1024x1024 RGB NITF with JPEG 2000 compression
python scripts/generate_synthetic_image.py test.ntf --width 1024 --height 1024 --bands 3 --compression j2k

# Generate a 3-level COG pyramid
python scripts/generate_synthetic_image_pyramid.py pyramid.tif --mode cog --bands 3 --levels 3

# Generate a 4-level NITF R-set pyramid
python scripts/generate_synthetic_image_pyramid.py pyramid.ntf --mode rset --levels 4

# Generate a Zarr tile index for cloud access
python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf -o index.json
```

### End-to-End Example

Generate a pyramid, index it, then chip from multiple resolution levels:

```bash
# 1. Generate a 3-level COG
python scripts/generate_synthetic_image_pyramid.py pyramid.tif --mode cog --bands 3

# 2. Build a tile index
python scripts/generate_tile_index.py pyramid.tif --source-uri file://$(pwd)/pyramid.tif -o pyramid.tile_index.json

# 3. Chip from each level
python scripts/chip_image_zarr.py pyramid.tile_index.json level0.png --bbox 384 384 640 640 --level 0
python scripts/chip_image_zarr.py pyramid.tile_index.json level1.png --bbox 128 128 384 384 --level 1
python scripts/chip_image_zarr.py pyramid.tile_index.json level2.png --bbox 0 0 256 256 --level 2
```

## Common Parameter Conventions

These scripts share consistent parameter naming:

- `--bbox / -b` — Bounding box as `X_MIN Y_MIN X_MAX Y_MAX` (column/row coordinates)
- `--asset / -a` — Image asset key (e.g., `image:0`, `image:0:overview:1`)
- `--level / -l` — Resolution level for multiscale indexes (0 = base)
- `--output / -o` — Output file path (when not a positional argument)
- `--width`, `--height` — Image dimensions in pixels
- `--tile-width`, `--tile-height` — Tile dimensions in pixels
- `--bands` — Number of image bands (1 or 3)
- `--pixel-type` — Pixel data type (`uint8` or `uint16`)
- `-v / --verbose` — Print timing and diagnostic info
- `-m / --metadata` — Include metadata in output
- `-r / --recursive` — Scan subdirectories

## Internal Scripts

These support the build and test infrastructure and are not user-facing examples:

| Script | Description |
|--------|-------------|
| `setup-dev-env.sh` | Configure `DYLD_LIBRARY_PATH` / `LD_LIBRARY_PATH` for PyO3 linking. |
| `generate_benchmark_data.py` | Generate data files for benchmark tests. |
| `generate_test_data.py` | Generate data files for unit tests. |
| `generate_benchmark_report.py` | Produce benchmark comparison reports. |
