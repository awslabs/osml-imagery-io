"""Benchmark: DTED read performance.

Measures the time to:
- Open and parse a DTED file (header parsing)
- Read the full elevation grid (signed-magnitude decode + transpose)

Uses a synthetic DTED Level 1 file (1201x1201) generated at test time.
This isolates I/O parsing and transposition cost without compression overhead.

Run with::

    pytest tests/benchmark/test_bench_dted.py -m benchmark --benchmark-autosave
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, AssetType, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType


@pytest.fixture(scope="module")
def dted_test_file():
    """Generate a synthetic 1201x1201 DTED Level 1 file for benchmarking."""
    num_rows = 1201
    num_cols = 1201

    metadata = BufferedMetadataProvider()
    metadata.set_json("dted:origin_longitude", -109.0)
    metadata.set_json("dted:origin_latitude", 38.0)
    metadata.set_json("dted:longitude_interval", 30)
    metadata.set_json("dted:latitude_interval", 30)
    metadata.set("dted:level", "DTED1")
    metadata.set("dted:security_code", "U")
    metadata.set("dted:vertical_datum", "MSL")
    metadata.set("dted:horizontal_datum", "WGS84")
    metadata.set("dted:producer_code", "US")
    metadata.set("dted:edition_number", "01")
    metadata.set("dted:compilation_date", "2601")
    metadata.set("dted:partial_cell_indicator", "00")
    metadata.set("dted:absolute_horizontal_accuracy", "0050")
    metadata.set("dted:absolute_vertical_accuracy", "0030")
    metadata.set("dted:relative_vertical_accuracy", "0020")
    metadata.set_json("dted:vertical_accuracy", 20)

    provider = BufferedImageAssetProvider.create(
        key="elevation",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=1,
        block_width=num_cols,
        block_height=num_rows,
        pixel_type=PixelType.Int16,
        metadata=metadata,
    )

    rng = np.random.RandomState(42)
    array = rng.randint(-2000, 4000, (1, num_rows, num_cols), dtype=np.int16)
    provider.set_full_image(array)

    with tempfile.NamedTemporaryFile(suffix=".dt1", delete=False) as f:
        path = Path(f.name)

    writer = IO.open([str(path)], "w", "dted")
    writer.metadata = metadata
    writer.add_asset("elevation", provider, "Elevation", "Benchmark", ["data"])
    writer.close()

    yield path

    path.unlink(missing_ok=True)


@pytest.mark.benchmark
def test_bench_dted_open_and_parse(benchmark, dted_test_file):
    """Benchmark: open and parse DTED headers."""
    path = str(dted_test_file)

    def run():
        reader = IO.open([path], "r")
        reader.get_asset_keys(asset_type=AssetType.Image)
        reader.close()

    benchmark.group = "dted_parse"
    benchmark.pedantic(run, warmup_rounds=1, rounds=20, iterations=1)
    benchmark.extra_info["cold_start"] = True
    benchmark.extra_info["dataset_size_bytes"] = dted_test_file.stat().st_size


@pytest.mark.benchmark
def test_bench_dted_full_read(benchmark, dted_test_file):
    """Benchmark: read entire DTED cell (decode + transpose)."""
    path = str(dted_test_file)

    def run():
        reader = IO.open([path], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(keys[0])
        asset.get_block(0, 0, 0)
        reader.close()

    benchmark.group = "dted_full_read"
    benchmark.pedantic(run, warmup_rounds=1, rounds=10, iterations=1)
    benchmark.extra_info["cold_start"] = True
    benchmark.extra_info["grid_size"] = "1201x1201"
    benchmark.extra_info["pixel_count"] = 1201 * 1201
    benchmark.extra_info["dataset_size_bytes"] = dted_test_file.stat().st_size
