"""Benchmark: tile read performance via Zarr/fsspec path.

Measures the time to read tiles through the ``MultiReferenceFileSystem`` +
``FsspecStore`` + ``zarr.open_group()`` pipeline, parametrized by backend
(local or S3) and access pattern (single tile, small ROI, large ROI).

Each timed iteration creates fresh filesystem, store, and group objects so
results reflect cold-start / first-access performance. No object reuse
across iterations.

The benchmark group is set dynamically to ``tile_read_zarr_local`` or
``tile_read_zarr_s3`` so the report generator produces separate tables per
backend.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import os
import platform
import sys

import numpy as np
import pytest

zarr = pytest.importorskip("zarr", minversion="3.0")
fsspec = pytest.importorskip("fsspec")

from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
from zarr.storage._fsspec import FsspecStore


@pytest.mark.benchmark
def test_bench_zarr_read(benchmark, zarr_read_params):
    """Benchmark: read tiles via Zarr (local or S3 backend)."""
    dataset_entry, access_pattern, index_path, remote_options, backend = zarr_read_params

    def run():
        fs_kwargs = {"fo": index_path, "skip_instance_cache": True, "asynchronous": True}
        if remote_options:
            fs_kwargs.update(remote_options)
        fs = MultiReferenceFileSystem(**fs_kwargs)
        store = FsspecStore(fs=fs, read_only=True, path="")
        root = zarr.open_group(store, mode="r", zarr_format=2)
        arr = root[list(root.array_keys())[0]]
        for row_start, row_end, col_start, col_end in access_pattern["regions"]:
            np.asarray(arr[:, row_start:row_end, col_start:col_end])

    benchmark.group = f"tile_read_zarr_{backend}"
    benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)

    # Record extra info after the benchmark run
    compression = dataset_entry.get("label", "unknown")
    benchmark.extra_info["access_pattern"] = access_pattern["name"]
    benchmark.extra_info["num_tiles_read"] = len(access_pattern["regions"])
    benchmark.extra_info["cold_start"] = True
    benchmark.extra_info["python_version"] = sys.version
    benchmark.extra_info["platform"] = platform.platform()
    benchmark.extra_info["dataset_size_bytes"] = dataset_entry["path"].stat().st_size
    benchmark.extra_info["compression"] = compression

    if backend == "s3":
        benchmark.extra_info["s3_region"] = os.environ.get("AWS_DEFAULT_REGION", "unknown")
