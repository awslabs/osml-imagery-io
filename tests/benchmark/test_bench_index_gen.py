"""Benchmark: tile index generation performance.

Measures the end-to-end time to scan a local imagery file and produce a
Kerchunk JSON tile index via ``OversightMLParser`` + ``write_tile_index()``.
Each timed iteration is a complete cold-start operation so results reflect
worst-case / first-access performance.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import platform
import sys

import pytest

zarr = pytest.importorskip("zarr")
fsspec = pytest.importorskip("fsspec")

from aws.osml.io import IO, AssetType
from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index


@pytest.mark.benchmark
def test_bench_index_generation(benchmark, dataset_entry, tmp_path):
    """Benchmark: generate tile index for a dataset."""
    path = str(dataset_entry["path"])
    output = str(tmp_path / "index.json")

    def run():
        parser = OversightMLParser(local_paths=path)
        store = parser(url=path)
        write_tile_index(store, output, segments=["image_segment_0"])

    benchmark.group = "index_generation"
    benchmark.pedantic(run, warmup_rounds=0, rounds=5, iterations=1)

    # Record extra info after the benchmark run
    num_segments = 0
    total_tiles = 0
    compression = dataset_entry.get("label", "unknown")

    try:
        reader = IO.open([path], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        num_segments = len(image_keys)
        for key in image_keys:
            asset = reader.get_asset(key)
            grid_rows, grid_cols = asset.block_grid_size
            total_tiles += grid_rows * grid_cols
            # Try to extract compression from asset metadata
            try:
                meta = asset.get_metadata().as_dict()
                if "IC" in meta:
                    compression = meta["IC"]
            except Exception:
                pass
        reader.close()
    except Exception:
        pass

    benchmark.extra_info["num_segments"] = num_segments
    benchmark.extra_info["total_tiles"] = total_tiles
    benchmark.extra_info["cold_start"] = True
    benchmark.extra_info["python_version"] = sys.version
    benchmark.extra_info["platform"] = platform.platform()
    benchmark.extra_info["dataset_size_bytes"] = dataset_entry["path"].stat().st_size
    benchmark.extra_info["compression"] = compression
