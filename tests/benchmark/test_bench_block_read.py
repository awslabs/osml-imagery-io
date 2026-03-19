"""Benchmark: pixel block read performance at spatial positions.

Measures the time to open a dataset, locate an image asset, and read a
pixel block at each of five spatial positions (UL, UR, LR, LL, C).
Each timed iteration is a complete cold-start operation (open → read →
close) so results reflect worst-case / first-access performance.

For grids smaller than 2×2, duplicate positions are deduplicated so only
distinct block locations are benchmarked.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import pytest
from aws.osml.io import IO, AssetType


@pytest.mark.benchmark
def test_bench_block_read(benchmark, block_read_params):
    """Benchmark: open dataset, get image asset, read one pixel block, close."""
    dataset_entry, position = block_read_params
    path = str(dataset_entry["path"])
    block_row = position["row"]
    block_col = position["col"]

    def run():
        reader = IO.open([path], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(image_keys[0])
        asset.get_block(block_row, block_col, 0)
        reader.close()

    benchmark.group = "block_read"
    benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)
