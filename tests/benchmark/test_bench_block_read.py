"""Benchmark: pixel block read performance via the IO read path.

Measures the time to open a dataset, locate an image asset, and read pixel
blocks through the ``IO.open`` read path using the same access patterns
(single_tile, small_roi, large_roi) as the Zarr read benchmarks so results are
directly comparable.

Each timed iteration is a complete cold-start operation (open → read →
close) so results reflect worst-case / first-access performance.

Three source dimensions are exercised — an IO-abstraction cost ladder from
direct disk access, through virtualized IO, to network IO:

- ``local`` — open the dataset by path (mmap; direct disk access, the default).
- ``virtual`` — open a seekable, sized Python file-like handle so the reader is
  driven through the ``Remote`` ``OwnedBuffer`` range-read path.  Per-tile decode
  then pulls only the touched codestream/strip byte ranges on demand
  (callback-driven for TIFF and J2K) instead of downloading the whole file.
  Runs offline via a byte-counting file-like handle — the recorded
  ``bytes_fetched`` metric quantifies the per-tile range-read reduction against
  the file size.
- ``s3`` — the same ``Remote`` path over a **real** ``s3fs`` handle, so each
  per-tile read is an HTTP range GET against the configured bucket.  This is the
  true-S3, non-Zarr block read the feature exists to enable.  Present only when
  ``OSML_IO_BENCHMARK_S3_BUCKET`` is set and ``s3fs`` + credentials are
  available.

Run with::

    pytest -m benchmark --benchmark-autosave
"""

import os
import platform
import sys
from concurrent.futures import ThreadPoolExecutor

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

# Number of worker threads the ROI benchmark uses to read ``block_coords``
# concurrently.  Across-block concurrency (the Option 5 payoff) lets each
# worker's within-block ``cat_ranges`` overlap, all feeding the shared s3fs
# connection pool (botocore default ``max_pool_connections`` = 10).  Each block
# fans its ~6 tile-parts out concurrently, so ~2 workers (2×6 ≥ 10) already
# saturate the default pool; more workers do nothing until the pool is raised
# (see the design's Open Questions).  Default 2; override with
# ``OSML_IO_BENCHMARK_WORKERS``.
_DEFAULT_ROI_WORKERS = 2


def roi_workers(default: int = _DEFAULT_ROI_WORKERS) -> int:
    """Worker-thread count for the threaded ROI read (env-overridable, min 1)."""
    raw = os.environ.get("OSML_IO_BENCHMARK_WORKERS")
    if raw is None:
        return default
    try:
        return max(1, int(raw))
    except ValueError:
        return default


@pytest.mark.benchmark
def test_bench_io_read(benchmark, io_read_params, source_mode):
    """Benchmark: read tiles via the IO read path matching Zarr access patterns."""
    dataset_entry, access_pattern, block_coords = io_read_params
    path = dataset_entry["path"]

    workers = roi_workers()

    common_info = {
        "access_pattern": access_pattern["name"],
        "num_blocks_read": len(block_coords),
        "cold_start": True,
        "python_version": sys.version,
        "platform": platform.platform(),
        "dataset_size_bytes": path.stat().st_size,
        "roi_workers": workers,
    }

    def _read_blocks_serial(asset):
        for block_row, block_col in block_coords:
            asset.get_block(block_row, block_col, 0)

    def _read_blocks_concurrent(asset):
        """Read every block in ``block_coords`` through a bounded thread pool.

        ``get_block`` releases the GIL, so the workers' fetches overlap: within a
        block the ~6 tile-parts fan out via ``cat_ranges``, and across blocks the
        lock-free reader lets separate workers' fetches run concurrently — both
        feeding the shared s3fs connection pool.  A single worker degenerates to
        the serial loop.

        Only safe for **cursor-free** sources (``local`` mmap slices, ``s3``
        fsspec ``cat_ranges``).  The offline ``virtual`` byte-pin uses a
        single-cursor ``ByteCountingStream`` and must stay serial (see below).
        """
        if workers <= 1:
            _read_blocks_serial(asset)
            return
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futures = [
                pool.submit(asset.get_block, block_row, block_col, 0)
                for block_row, block_col in block_coords
            ]
            for future in futures:
                future.result()

    if source_mode == "local":
        path_str = str(path)

        def run():
            reader = IO.open([path_str], "r")
            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            asset = reader.get_asset(image_keys[0])
            _read_blocks_concurrent(asset)
            reader.close()

        benchmark.group = "tile_read_io"
        benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)
        benchmark.extra_info.update(common_info)
        benchmark.extra_info["source_mode"] = "local"
        return

    # virtual / s3 — callback-driven per-tile range reads for the block-capable
    # formats (TIFF, J2K).  Monolithic formats stay on the full-read fallback,
    # so a per-tile range-read benchmark is not meaningful for them.  Both modes
    # drive the same Remote path; they differ only in the handle (offline
    # byte-counter vs. real s3fs handle → HTTP range GETs).
    fmt = _format_from_path(path)
    if not _remote_capable(path):
        pytest.skip(
            f"{dataset_entry['label']} (format={fmt}) is not remote-capable; "
            "monolithic formats stay on the full-read path"
        )

    file_size = path.stat().st_size

    def _read_blocks(reader, concurrent):
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(image_keys[0])
        if concurrent:
            _read_blocks_concurrent(asset)
        else:
            _read_blocks_serial(asset)

    if source_mode == "s3":
        uri = _s3_uri_for(path)
        if uri is None:
            pytest.skip("s3 source mode requires OSML_IO_BENCHMARK_S3_BUCKET")

        # s3 uses a real fsspec handle: reads flow through the cursor-free
        # ``cat_ranges`` path, so the across-block ThreadPoolExecutor is safe and
        # is where the concurrency win is measured (wall-clock).  Byte accounting
        # for s3 is not meaningful here — ``cat_ranges`` bypasses the handle's
        # ``.read()``, so the probe records the serial pin below instead.
        def run():
            with open_counting_s3(uri) as handle:
                reader = IO.open(handle, "r", format=fmt)
                _read_blocks(reader, concurrent=True)
                reader.close()

        benchmark.group = "tile_read_io"
        benchmark.pedantic(run, warmup_rounds=0, rounds=s3_rounds(), iterations=1)

        # Serial probe run for the wire-level byte pin (Decision 5's "Option C"):
        # counting is unambiguous only when serial, so the probe reads serially.
        with open_counting_s3(uri) as probe:
            reader = IO.open(probe, "r", format=fmt)
            _read_blocks(reader, concurrent=False)
            reader.close()
            reads = probe.reads
            bytes_fetched = probe.bytes_fetched
    else:
        file_bytes = path.read_bytes()

        # virtual (offline) is the wire-level byte pin: a single-cursor
        # ``ByteCountingStream`` (non-fsspec) whose ``.read()`` count is
        # unambiguous only when serial.  It intentionally stays serial — the
        # across-block concurrency win is measured by the ``s3`` mode.  (Threading
        # a single cursor would also race it and trip the containment invariant's
        # debug re-entry assertion.)
        def run():
            stream = ByteCountingStream(file_bytes)
            reader = IO.open(stream, "r", format=fmt)
            _read_blocks(reader, concurrent=False)
            reader.close()

        benchmark.group = "tile_read_io"
        benchmark.pedantic(run, warmup_rounds=0, rounds=10, iterations=1)

        # Re-run once, untimed, to capture the bytes-fetched metric.
        probe = ByteCountingStream(file_bytes)
        reader = IO.open(probe, "r", format=fmt)
        _read_blocks(reader, concurrent=False)
        reader.close()
        reads = probe.reads
        bytes_fetched = probe.bytes_fetched

    benchmark.extra_info.update(common_info)
    benchmark.extra_info["source_mode"] = source_mode
    benchmark.extra_info["bytes_fetched"] = bytes_fetched
    benchmark.extra_info["num_reads"] = len(reads)
    benchmark.extra_info["fetch_fraction"] = (
        bytes_fetched / file_size if file_size else 0.0
    )

    assert reads, "expected at least one bounded range read"
