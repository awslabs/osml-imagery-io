"""Benchmark: tile index generation performance.

Measures the end-to-end time to scan an imagery file and produce a Kerchunk
JSON tile index via ``OversightMLParser`` + ``write_tile_index()``.  Each timed
iteration is a complete cold-start operation so results reflect worst-case /
first-access performance.

Three source dimensions are exercised — an IO-abstraction cost ladder from
direct disk access, through virtualized IO, to network IO:

- ``local`` — parse a local path; ``fsspec`` opens the file directly.
- ``virtual`` — parse a ``file://`` URL through a byte-counting ``file`` fsspec
  filesystem so the index-construction bootstrap (headers + tile-offset tables)
  runs over the ``Remote`` ``OwnedBuffer`` range-read path.  This is the
  chicken-and-egg case the remote range-read design targets:
  ``OversightMLParser`` over a virtualized URL builds the index via range reads
  instead of a full download.  Runs offline — no S3 required — and records the
  ``bytes_fetched`` metric so the range-read reduction is quantified.
- ``s3`` — parse a real ``s3://`` URL so the bootstrap issues HTTP range GETs
  against the configured bucket.  This is the true-S3, non-Zarr index build the
  feature exists to enable.  Present only when ``OSML_IO_BENCHMARK_S3_BUCKET`` is
  set and ``s3fs`` + credentials are available.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import platform
import sys

import pytest

zarr = pytest.importorskip("zarr")
fsspec = pytest.importorskip("fsspec")

from aws.osml.io import IO, AssetType  # noqa: E402
from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index  # noqa: E402

from tests.benchmark.conftest import (  # noqa: E402
    _first_image_segments,
    _format_from_path,
    _remote_capable,
    _s3_uri_for,
    counting_local_filesystem,
    counting_s3_filesystem,
    s3_rounds,
)


def _record_dataset_metadata(benchmark, dataset_entry) -> None:
    """Attach segment/tile/compression/platform info to the benchmark result."""
    path = str(dataset_entry["path"])
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
            try:
                meta = asset.metadata.entries()
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


@pytest.mark.benchmark
def test_bench_index_generation(benchmark, dataset_entry, source_mode, tmp_path):
    """Benchmark: generate tile index for a dataset (local path or virtualized URL)."""
    path = dataset_entry["path"]
    output = str(tmp_path / "index.json")

    if source_mode == "local":
        path_str = str(path)

        def run():
            parser = OversightMLParser()
            store = parser(path_str)
            write_tile_index(store, output, segments=_first_image_segments(store))

        benchmark.group = "index_generation"
        benchmark.pedantic(run, warmup_rounds=0, rounds=5, iterations=1)
        benchmark.extra_info["source_mode"] = "local"
        _record_dataset_metadata(benchmark, dataset_entry)
        return

    # virtual / s3 — ``OversightMLParser`` opens the URL itself, so route it
    # through a byte-counting fsspec filesystem to exercise (and measure) the
    # range-read bootstrap.  ``virtual`` uses a ``file://`` URL offline; ``s3``
    # uses a real ``s3://`` URL (HTTP range GETs against the bucket).
    fmt = _format_from_path(path)
    if not _remote_capable(path):
        pytest.skip(
            f"{dataset_entry['label']} (format={fmt}) is not remote-capable; "
            "monolithic formats stay on the full-read path"
        )

    if source_mode == "s3":
        url = _s3_uri_for(path)
        if url is None:
            pytest.skip("s3 source mode requires OSML_IO_BENCHMARK_S3_BUCKET")
        counting_fs = counting_s3_filesystem
        rounds = s3_rounds()  # network: keep the round count low
    else:
        url = path.resolve().as_uri()  # file:///abs/path
        counting_fs = counting_local_filesystem
        rounds = 5

    def run():
        with counting_fs():
            parser = OversightMLParser()
            store = parser(url)
            write_tile_index(store, output, segments=_first_image_segments(store))

    benchmark.group = "index_generation"
    benchmark.pedantic(run, warmup_rounds=0, rounds=rounds, iterations=1)

    # Untimed probe run to capture the bytes-fetched metric.
    file_size = path.stat().st_size
    with counting_fs() as counter:
        parser = OversightMLParser()
        store = parser(url)
        write_tile_index(store, output, segments=_first_image_segments(store))

    benchmark.extra_info["source_mode"] = source_mode
    benchmark.extra_info["bytes_fetched"] = counter["bytes_fetched"]
    benchmark.extra_info["num_reads"] = len(counter["reads"])
    benchmark.extra_info["fetch_fraction"] = (
        counter["bytes_fetched"] / file_size if file_size else 0.0
    )
    _record_dataset_metadata(benchmark, dataset_entry)

    assert counter["reads"], "expected at least one range read during parse"
