# Performance Benchmarks

This page shows benchmark results for the osml-imagery-io Python API. Results represent
cold-start timing — each iteration opens the dataset from scratch. OS page-cache effects
may still influence results for repeated iterations on the same file.

## Generating Results

To produce benchmark results you need imagery files configured in
`data/benchmark/benchmark_datasets.yaml`. See the example entries in that file for the
expected format.

### 1. Run the benchmarks

```bash
pytest -m benchmark --benchmark-autosave
```

Results are saved to `.benchmarks/`. You can also override the dataset directory with
the `OSML_IO_BENCHMARK_DATA` environment variable:

```bash
OSML_IO_BENCHMARK_DATA=/path/to/imagery pytest -m benchmark --benchmark-autosave
```

### 2. Generate the results fragment

```bash
python scripts/generate_benchmark_report.py
```

This reads the latest result from `.benchmarks/` and writes `docs/_benchmark_results.md`.
You can also point it at a specific file:

```bash
python scripts/generate_benchmark_report.py .benchmarks/Linux-CPython-3.12/0001_abc.json
```

### 3. Rebuild the docs

```bash
make html -C docs
```

## Results

```{include} _benchmark_results.md
```
