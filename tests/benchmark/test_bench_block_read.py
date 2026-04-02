"""Benchmark: pixel block read performance via native IO.

Measures the time to open a dataset, locate an image asset, and read pixel
blocks through the native IO path using the same access patterns (single_tile,
small_roi, large_roi) as the Zarr read benchmarks so results are directly
comparable.

Each timed iteration is a complete cold-start operation (open → read →
close) so results reflect worst-case / first-access performance.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import platform
import sys

import pytest
from aws.osml.io import IO, AssetType


@pytest.mark.benchmark
def test_bench_native_read(benchmark, native_read_params):
    """Benchmark: read tiles via native IO matching Zarr access patterns."""
    dataset_entry, access_pattern, block_coords = native_read_params
    path = str(dataset_entry["path"])

    def run():
        reader = IO.open([path], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(image_keys[0])
        for block_row, block_col in block_coords:
            asset.get_block(block_row, block_col, 0)
        reader.close()

    benchmark.group = "tile_read_native"
    benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)

    benchmark.extra_info["access_pattern"] = access_pattern["name"]
    benchmark.extra_info["num_blocks_read"] = len(block_coords)
    benchmark.extra_info["cold_start"] = True
    benchmark.extra_info["python_version"] = sys.version
    benchmark.extra_info["platform"] = platform.platform()
    benchmark.extra_info["dataset_size_bytes"] = dataset_entry["path"].stat().st_size
