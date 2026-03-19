"""Benchmark: dataset metadata read performance.

Measures the time to open a dataset, read file-level metadata, enumerate
asset keys, and read image asset metadata. Each timed iteration is a
complete cold-start operation (open → read → close) so results reflect
worst-case / first-access performance.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import pytest
from aws.osml.io import IO, AssetType


@pytest.mark.benchmark
def test_bench_metadata_read(benchmark, dataset_entry):
    """Benchmark: open dataset, read file metadata, enumerate assets, read image metadata."""
    path = str(dataset_entry["path"])

    def run():
        reader = IO.open([path], "r")

        # File-level metadata — iterate all keys to force lazy parsing
        file_meta_dict = reader.metadata.as_dict()
        for key in file_meta_dict:
            _ = file_meta_dict[key]

        # Image asset metadata
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        if image_keys:
            asset = reader.get_asset(image_keys[0])
            asset_meta_dict = asset.get_metadata().as_dict()
            for key in asset_meta_dict:
                _ = asset_meta_dict[key]

        reader.close()

    benchmark.group = "metadata"
    benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)
