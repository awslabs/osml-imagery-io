# Benchmark Test Data

This directory contains imagery and configuration for performance benchmarking
of the `aws.osml.io` Python API.

## Quick Start

1. Place your benchmark imagery files in this directory (or set `OSML_IO_BENCHMARK_DATA`
   to point at an alternate location).
2. Edit `benchmark_datasets.yaml` to list the files you want to benchmark.
3. Run the benchmarks:

```bash
pytest -m benchmark --benchmark-autosave
```

Benchmarks are driven by `benchmark_datasets.yaml` in this directory. The file
uses a simple YAML schema:

```yaml
datasets:
  - path: "large_nitf.ntf"
    label: "Large NITF 2.1 (4-band J2K)"
  - path: "geotiff_sample.tif"
    label: "Cloud Optimized GeoTIFF"
  - path: "/absolute/path/to/another_file.ntf"
    label: "Absolute Path Example"
```

### Schema

The top-level key is `datasets`, which holds a list of entries:

| Field   | Required | Description |
|---------|----------|-------------|
| `path`  | Yes      | File path to the imagery. Relative paths are resolved from `OSML_IO_BENCHMARK_DATA` (if set) or this directory (`data/benchmark/`). Absolute paths are used as-is. |
| `label` | No       | Human-readable name shown in benchmark output. Defaults to the filename stem if omitted. |

When `datasets` is empty (`datasets: []`) or the file is missing, all benchmarks
are skipped gracefully with exit code 0.

If a configured path does not exist on disk, that dataset is skipped with a
logged warning — the remaining datasets still run.

## Environment Variable Override

Set `OSML_IO_BENCHMARK_DATA` to resolve relative paths against a different
base directory:

```bash
export OSML_IO_BENCHMARK_DATA=/path/to/your/data
pytest -m benchmark
```

## What Gets Benchmarked

The suite measures two operations using cold-start isolation (each timed
iteration opens the dataset from scratch):

- **Metadata read** — Opens the dataset, reads file-level metadata, enumerates
  asset keys, and reads image asset metadata.
- **Block read** — Reads pixel blocks from five spatial positions in the first
  image asset: upper-left (UL), upper-right (UR), lower-right (LR),
  lower-left (LL), and center (C). Duplicate positions are collapsed for
  small block grids.

## Running Benchmarks

```bash
# Run all Python benchmarks
pytest -m benchmark

# Save results to JSON (for report generation)
pytest -m benchmark --benchmark-autosave

# Verbose output
pytest -m benchmark -v
```

### Generating a Documentation Report

After running benchmarks with `--benchmark-autosave`, generate a Markdown
performance page for the Sphinx docs:

```bash
# Uses the latest result from .benchmarks/ automatically
python scripts/generate_benchmark_report.py

# Or point at a specific saved result
python scripts/generate_benchmark_report.py .benchmarks/Darwin-CPython-3.12-64bit/0001_abc.json

# Custom output path
python scripts/generate_benchmark_report.py -o my_report.md
```

### Rust Benchmarks (Criterion)

Rust-level benchmarks are separate and run via Cargo:

```bash
cargo bench
```

## Recommendations

- Include a variety of image sizes and formats.
- Use imagery representative of your production workloads.
- For reproducible results, document which files you used and their sizes.
- OS page cache effects may influence repeated iterations on the same file.
