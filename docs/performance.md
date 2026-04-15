# Performance Benchmarks

This page shows benchmark results for the osml-imagery-io Python API. All benchmarks
use cold-start timing — each iteration opens the dataset from scratch with no warm
caches or pre-initialized state. OS page-cache effects may still influence results
for repeated iterations on the same file.

## Benchmark Groups

The benchmark suite produces five result groups:

- **Tile Read Native** — Block reads through the native IO path using access
  patterns (single tile, small ROI, large ROI) that match the Zarr benchmarks for
  direct comparison.
- **Tile Read Zarr Local** — Tile reads through `MultiReferenceFileSystem` +
  `zarr.open_group()` with a local file as the backing store. Uses hierarchical
  Kerchunk indexes with the appropriate codec (JbpBlockCodec for NITF,
  TiffTileCodec for TIFF/COG).
- **Tile Read Zarr S3** — Same Zarr path but with S3 as the backing store. Only
  included when `OSML_IO_BENCHMARK_S3_BUCKET` is set.
- **Index Generation** — End-to-end time to scan a local file and produce a Kerchunk
  JSON tile index via `OversightMLParser` + `write_tile_index()`. Includes
  multi-resolution index generation for COG and NITF R-set pyramids.
- **Metadata** — Time to open a dataset, read file-level and image asset metadata.

## Dataset Coverage

The benchmark suite exercises multiple format and compression combinations:

- NITF uncompressed (NC) — various sizes from 1MB to 64MB
- NITF JPEG (C3) — lossy compressed multi-band
- NITF JPEG 2000 (C8) — wavelet compressed, including large real-world imagery
- NITF SIDD — SAR-derived product with XML DES metadata
- TIFF uncompressed — exercises the TiffTileCodec in the Zarr path
- COG pyramid — multi-resolution TIFF with overview IFDs
- NITF R-set pyramid — multi-file NITF with overview levels

## Generating Results

Benchmark datasets are configured in `data/benchmark/benchmark_datasets.yaml`.
Dataset paths are resolved relative to `data/benchmark/` by default, or relative
to the directory specified by `OSML_IO_BENCHMARK_DATA`.

### 1. Generate synthetic datasets (optional)

```bash
python scripts/generate_benchmark_data.py
```

This creates synthetic NITF imagery in `data/integration/synthetic/` and appends
entries to `benchmark_datasets.yaml`.

For multi-resolution and TIFF coverage, also generate:

```bash
# Synthetic TIFF (exercises TiffTileCodec in the Zarr path)
python scripts/generate_synthetic_image.py data/integration/synthetic/synth_small_tiff.tif \
    --format tiff --width 1024 --height 1024 --bands 1 \
    --tile-width 256 --tile-height 256 --compression none

# COG pyramid (multi-resolution TIFF with overview IFDs)
python scripts/generate_synthetic_image_pyramid.py data/integration/synthetic/synth_cog_pyramid.tif \
    --mode cog --width 2048 --height 2048 --tile-width 256 --tile-height 256 --levels 3

# NITF R-set pyramid (multi-file NITF with overview levels)
python scripts/generate_synthetic_image_pyramid.py data/integration/synthetic/synth_rset_pyramid.ntf \
    --mode rset --width 2048 --height 2048 --tile-width 256 --tile-height 256 --levels 3
```

Set `OSML_IO_BENCHMARK_DATA=data/integration` when running benchmarks to include
the synthetic datasets.

### 2. Run the benchmarks

```bash
# Local benchmarks only
OSML_IO_BENCHMARK_DATA=data/integration pytest -m benchmark --benchmark-autosave

# Include S3 benchmarks (requires imagery uploaded to the bucket + credentials)
OSML_IO_BENCHMARK_DATA=data/integration \
OSML_IO_BENCHMARK_S3_BUCKET=s3://my-bucket/path \
pytest -m benchmark --benchmark-autosave
```

Results are saved to `.benchmarks/`.

### 3. Generate the results fragment

```bash
python scripts/generate_benchmark_report.py
```

This reads the latest result from `.benchmarks/` and writes `docs/_benchmark_results.md`.
You can also point it at a specific file:

```bash
python scripts/generate_benchmark_report.py .benchmarks/Linux-CPython-3.12/0001_abc.json
```

### 4. Rebuild the docs

```bash
make html -C docs
```

## Comparison Axes

The Tile Read Native and Tile Read Zarr Local groups use the same access patterns
(single tile, small ROI, large ROI) on the same datasets, so their results are
directly comparable:

- **Native IO vs Zarr-from-local**: Isolates the Zarr/fsspec/codec overhead.
  Compare `tile_read_native` against `tile_read_zarr_local` for the same dataset
  and access pattern.
- **Zarr-from-local vs Zarr-from-S3**: Measures the network latency impact.
  Compare `tile_read_zarr_local` against `tile_read_zarr_s3`.

## Results

```{include} _benchmark_results.md
```
