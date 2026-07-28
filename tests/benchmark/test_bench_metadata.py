"""Benchmark: dataset metadata read performance.

Measures the time to open a dataset, read file-level metadata, enumerate
asset keys, and read image asset metadata. Each timed iteration is a
complete cold-start operation (open → read → close) so results reflect
worst-case / first-access performance.

Three source dimensions are exercised — an IO-abstraction cost ladder from
direct disk access, through virtualized IO, to network IO:

- ``local`` — open the dataset by path (mmap; direct disk access, the default).
- ``virtual`` — open a seekable, sized Python file-like handle so the reader is
  driven through the ``Remote`` ``OwnedBuffer`` range-read path (no full
  download).  This is the headline win the remote range-read design targets:
  metadata reads over a file-like handle complete via a few bounded range reads
  instead of a full download.  Runs offline via a byte-counting file-like handle
  — the recorded ``bytes_fetched`` metric quantifies the reduction against the
  file size.
- ``s3`` — the same ``Remote`` path over a **real** ``s3fs`` handle, so each
  read is an HTTP range GET against the configured bucket.  This is the true-S3,
  non-Zarr read the feature exists to enable.  Present only when
  ``OSML_IO_BENCHMARK_S3_BUCKET`` is set and ``s3fs`` + credentials are
  available.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import pytest
from aws.osml.io import IO, AssetType

from tests.benchmark.conftest import (
    ByteCountingStream,
    _format_from_path,
    _remote_capable,
    _s3_uri_for,
    open_counting_s3,
    s3_rounds,
)


def _read_all_metadata(reader) -> None:
    """Read file + image-asset metadata, forcing lazy parsing of every entry."""
    file_meta_dict = reader.metadata.entries()
    for key in file_meta_dict:
        _ = file_meta_dict[key]

    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    if image_keys:
        asset = reader.get_asset(image_keys[0])
        asset_meta_dict = asset.metadata.entries()
        for key in asset_meta_dict:
            _ = asset_meta_dict[key]


@pytest.mark.benchmark
def test_bench_metadata_read(benchmark, dataset_entry, source_mode):
    """Benchmark: open dataset, read file metadata, enumerate assets, read image metadata."""
    path = dataset_entry["path"]

    if source_mode == "local":
        path_str = str(path)

        def run():
            reader = IO.open([path_str], "r")
            _read_all_metadata(reader)
            reader.close()

        benchmark.group = "metadata"
        benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)
        benchmark.extra_info["source_mode"] = "local"
        return

    # virtual / s3 — both drive the Remote OwnedBuffer path; they differ only in
    # the handle (offline byte-counter vs. real s3fs handle).
    fmt = _format_from_path(path)
    if not _remote_capable(path):
        pytest.skip(
            f"{dataset_entry['label']} (format={fmt}) is not remote-capable; "
            "monolithic formats stay on the full-read path"
        )

    file_size = path.stat().st_size

    if source_mode == "s3":
        uri = _s3_uri_for(path)
        if uri is None:
            pytest.skip("s3 source mode requires OSML_IO_BENCHMARK_S3_BUCKET")

        def run():
            with open_counting_s3(uri) as handle:
                reader = IO.open(handle, "r", format=fmt)
                _read_all_metadata(reader)
                reader.close()

        benchmark.group = "metadata"
        benchmark.pedantic(run, warmup_rounds=0, rounds=s3_rounds(), iterations=1)

        # Untimed probe run to capture the bytes-fetched metric.
        with open_counting_s3(uri) as probe:
            reader = IO.open(probe, "r", format=fmt)
            _read_all_metadata(reader)
            reader.close()
            reads = probe.reads
            bytes_fetched = probe.bytes_fetched
    else:
        file_bytes = path.read_bytes()

        # Timed run: each iteration opens a fresh range-counting handle so the
        # measurement reflects cold-start range reads.
        def run():
            stream = ByteCountingStream(file_bytes)
            reader = IO.open(stream, "r", format=fmt)
            _read_all_metadata(reader)
            reader.close()
            return stream

        benchmark.group = "metadata"
        benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)

        # Re-run once, untimed, to capture the bytes-fetched metric.
        probe = ByteCountingStream(file_bytes)
        reader = IO.open(probe, "r", format=fmt)
        _read_all_metadata(reader)
        reader.close()
        reads = probe.reads
        bytes_fetched = probe.bytes_fetched

    benchmark.extra_info["source_mode"] = source_mode
    benchmark.extra_info["dataset_size_bytes"] = file_size
    benchmark.extra_info["bytes_fetched"] = bytes_fetched
    benchmark.extra_info["num_reads"] = len(reads)
    benchmark.extra_info["fetch_fraction"] = (
        bytes_fetched / file_size if file_size else 0.0
    )

    assert reads, "expected at least one bounded range read"
    # For files larger than the 64 KiB header prefetch, metadata must not pull
    # the whole file in a single read (that would be a full-download regression).
    if file_size > 64 * 1024:
        assert all(length < file_size for _, length in reads), (
            f"a single metadata read covered the whole file "
            f"({dataset_entry['label']}): {reads}"
        )
