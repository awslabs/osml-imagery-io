# Benchmark Test Data

This directory contains imagery for performance benchmarking.

## Usage

Place your own test images in this directory to benchmark the library's performance against data relevant to your use case.

The benchmark suite will iterate over all supported image files found here.

## Running Benchmarks

```bash
# Rust (using Criterion)
cargo bench

# Python
pytest -m benchmark --benchmark-enable
```

## Environment Override

Set `OSML_IO_BENCHMARK_DATA` to use an alternate location:

```bash
export OSML_IO_BENCHMARK_DATA=/path/to/your/data
```

## Recommendations

- Include a variety of image sizes and formats
- Use imagery representative of your production workloads
- For reproducible results, document which files you used
