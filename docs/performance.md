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
  `zarr.open_group()` with a local file as the backing store.
- **Tile Read Zarr S3** — Same Zarr path but with S3 as the backing store. Only
  included when `OSML_IO_BENCHMARK_S3_BUCKET` is set.
- **Index Generation** — End-to-end time to scan a local file and produce a Kerchunk
  JSON tile index via `OversightMLParser` + `write_tile_index()`.
- **Metadata** — Time to open a dataset, read file-level and image asset metadata.

## Generating Results

Benchmark datasets are configured in `data/benchmark/benchmark_datasets.yaml`.
Dataset paths are resolved relative to `data/benchmark/` by default, or relative
to the directory specified by `OSML_IO_BENCHMARK_DATA`.

### 1. Generate synthetic datasets (optional)

```bash
python scripts/generate_benchmark_data.py
```

This creates synthetic imagery in `data/integration/synthetic/` and appends entries
to `benchmark_datasets.yaml`. Set `OSML_IO_BENCHMARK_DATA=data/integration` when
running benchmarks to include them.

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
