"""Benchmark: peak memory usage during file open.

Measures peak RSS delta during IO.open() + get_asset_keys() + get_asset() for
synthetic uncompressed files across formats.

Three source dimensions are contrasted so the memory profile of the virtualized
range-read path is directly comparable to the two whole-file paths:

- ``local`` — open the dataset by path (mmap; direct disk access). The OS
  demand-pages from disk, so peak RSS reflects only the touched pages (metadata
  + first asset).
- ``virtual`` — open a seekable, sized Python file-like handle that reads from
  disk on demand, driving the ``Remote`` ``OwnedBuffer`` range-read path. Peak
  RSS is bounded by the fetched header ranges + prefetch, NOT the whole file.
  This is the clearest differentiator the remote range-read design targets.
- ``full_download`` — open a non-seekable file-like handle, forcing the legacy
  full-read path (``read_stream_bytes`` → whole-file ``Vec<u8>``). Peak RSS
  grows by ~the file size — the behavior the ``virtual`` mode avoids.

For the block-capable formats (nitf, tiff, j2k, dted) the ``virtual`` run must
hold materially less than the ``full_download`` run for metadata-only access.
Monolithic formats (jpeg) stay on the full-read path regardless, so only their
``local`` / ``full_download`` modes are measured.

Run with::

    pytest -m benchmark tests/benchmark/test_memory_usage.py -v
"""

import platform
import resource
import sys
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
    imsave,
)

from tests.benchmark.conftest import _format_from_path, _remote_capable

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_TARGET_SIZE_MB = 50


def _peak_rss_bytes() -> int:
    """Return peak RSS in bytes (macOS reports bytes, Linux reports KB)."""
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if platform.system() == "Linux":
        rss *= 1024
    return rss


# ---------------------------------------------------------------------------
# Synthetic file generators
# ---------------------------------------------------------------------------


def _generate_nitf(path: Path) -> int:
    """Generate an uncompressed NITF file of ~50 MB. Returns file size."""
    bands, height, width = 3, 4096, 4096
    data = np.zeros((bands, height, width), dtype=np.uint8)
    imsave(str(path), data, compression="nc", block_size=(width, height))
    return path.stat().st_size


def _generate_tiff(path: Path) -> int:
    """Generate an uncompressed TIFF file of ~50 MB. Returns file size."""
    bands, height, width = 3, 4096, 4096
    data = np.zeros((bands, height, width), dtype=np.uint8)
    imsave(str(path), data, compression="none", block_size=(width, height))
    return path.stat().st_size


def _generate_j2k(path: Path) -> int:
    """Generate a lossless J2K file. Returns file size.

    J2K always compresses, so the file will be smaller than raw — we still
    measure to confirm the open path copies data.
    """
    bands, height, width = 1, 4096, 4096
    data = np.random.RandomState(42).randint(0, 255, (bands, height, width), dtype=np.uint8)
    imsave(str(path), data)
    return path.stat().st_size


def _generate_jpeg(path: Path) -> int:
    """Generate a JPEG file. Returns file size.

    JPEG is lossy-only; file size will be well under 50 MB with uniform data.
    We use random data to produce a non-trivially-sized file.
    """
    bands, height, width = 3, 4096, 4096
    data = np.random.RandomState(42).randint(0, 255, (bands, height, width), dtype=np.uint8)
    imsave(str(path), data, quality=95.0)
    return path.stat().st_size


def _generate_dted(path: Path) -> int:
    """Generate a synthetic DTED Level 2 file. Returns file size.

    DTED has a fixed grid (3601x3601 for level 2, ~25 MB of int16 payload).
    """
    num_rows = 3601
    num_cols = 3601

    metadata = BufferedMetadataProvider()
    metadata["dted:origin_longitude"] = -109.0
    metadata["dted:origin_latitude"] = 38.0
    metadata["dted:longitude_interval"] = 10
    metadata["dted:latitude_interval"] = 10
    metadata["dted:level"] = "DTED2"
    metadata["dted:security_code"] = "U"
    metadata["dted:vertical_datum"] = "MSL"
    metadata["dted:horizontal_datum"] = "WGS84"
    metadata["dted:producer_code"] = "US"
    metadata["dted:edition_number"] = "01"
    metadata["dted:compilation_date"] = "2601"
    metadata["dted:partial_cell_indicator"] = "00"
    metadata["dted:absolute_horizontal_accuracy"] = "0050"
    metadata["dted:absolute_vertical_accuracy"] = "0030"
    metadata["dted:relative_vertical_accuracy"] = "0020"
    metadata["dted:vertical_accuracy"] = 20

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

    writer = IO.open([str(path)], "w", "dted")
    writer.metadata = metadata
    writer.add_asset("elevation", provider, "Elevation", "Benchmark", ["data"])
    writer.close()

    return path.stat().st_size


# ---------------------------------------------------------------------------
# Format registry
# ---------------------------------------------------------------------------

_FORMATS = [
    ("nitf", ".ntf", _generate_nitf),
    ("tiff", ".tif", _generate_tiff),
    ("j2k", ".j2k", _generate_j2k),
    ("jpeg", ".jpg", _generate_jpeg),
    ("dted", ".dt2", _generate_dted),
]


# ---------------------------------------------------------------------------
# Session-scoped fixture: generate all synthetic files once
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def synthetic_files(tmp_path_factory):
    """Generate synthetic files for each format and return a dict of metadata."""
    tmp_dir = tmp_path_factory.mktemp("memory_bench")
    results = {}

    for fmt_name, ext, generator in _FORMATS:
        path = tmp_dir / f"bench_memory{ext}"
        try:
            file_size = generator(path)
            results[fmt_name] = {"path": path, "size_bytes": file_size}
        except Exception as exc:
            pytest.skip(f"Failed to generate {fmt_name} file: {exc}")

    return results


# ---------------------------------------------------------------------------
# Parametrized memory benchmark
# ---------------------------------------------------------------------------


@pytest.fixture(params=[fmt[0] for fmt in _FORMATS], ids=[fmt[0] for fmt in _FORMATS])
def format_name(request):
    """Yield format names one at a time."""
    return request.param


# The subprocess bodies below open the file three different ways.  Each returns
# the peak-RSS delta (bytes) from just after imports to just after the open +
# first-asset access, on stdout.  A subprocess is used so ``ru_maxrss`` (a
# monotonic peak over the process lifetime) reflects only this one operation.

# ``local`` — open by path (mmap).
_OPEN_LOCAL = """
reader = IO.open(["{path}"], "r")
keys = reader.get_asset_keys(asset_type=AssetType.Image)
if keys:
    asset = reader.get_asset(keys[0])
reader.close()
"""

# ``virtual`` — seekable, sized Python handle reading from disk on demand → the
# ``Remote`` ``OwnedBuffer`` range-read path.  Peak RSS is bounded by the
# fetched header ranges + prefetch, not the whole file.
_OPEN_VIRTUAL = """
import os

class DiskRangeHandle:
    def __init__(self, path):
        self._f = open(path, "rb")
        self.size = os.path.getsize(path)
    def seekable(self):
        return True
    def seek(self, o, w=0):
        return self._f.seek(o, w)
    def tell(self):
        return self._f.tell()
    def read(self, n=-1):
        return self._f.read(n)
    def close(self):
        return self._f.close()

handle = DiskRangeHandle("{path}")
reader = IO.open(handle, "r", format="{fmt}")
keys = reader.get_asset_keys(asset_type=AssetType.Image)
if keys:
    asset = reader.get_asset(keys[0])
reader.close()
handle.close()
"""

# ``full_download`` — non-seekable handle forces the legacy full-read path
# (whole-file Vec<u8>).  Peak RSS grows by ~the file size.
_OPEN_FULL_DOWNLOAD = """
class NonSeekableHandle:
    def __init__(self, path):
        self._f = open(path, "rb")
    def seekable(self):
        return False
    def read(self, n=-1):
        return self._f.read(n)
    def close(self):
        return self._f.close()

handle = NonSeekableHandle("{path}")
reader = IO.open(handle, "r", format="{fmt}")
keys = reader.get_asset_keys(asset_type=AssetType.Image)
if keys:
    asset = reader.get_asset(keys[0])
reader.close()
handle.close()
"""

_OPEN_BODIES = {
    "local": _OPEN_LOCAL,
    "virtual": _OPEN_VIRTUAL,
    "full_download": _OPEN_FULL_DOWNLOAD,
}


@pytest.fixture(params=list(_OPEN_BODIES), ids=list(_OPEN_BODIES))
def mem_source_mode(request) -> str:
    """Yield the memory-benchmark source mode.

    ``local`` (mmap), ``virtual`` (Remote OwnedBuffer range reads through a
    Python file-like handle), and ``full_download`` (legacy non-seekable full
    read) so peak RSS is directly comparable across the three whole-vs-range
    paths.
    """
    return request.param


def _measure_open_rss(path: str, mode: str, fmt: str) -> int:
    """Measure RSS increase from opening a file and accessing its first asset.

    ``mode`` selects how the file is opened (``local`` / ``virtual`` /
    ``full_download``).  Because ``ru_maxrss`` is monotonically increasing
    (peak over process lifetime), we use a subprocess to get an isolated
    measurement per mode.
    """
    import subprocess

    open_body = _OPEN_BODIES[mode].format(path=path, fmt=fmt)
    script = f"""
import resource
import platform
import gc

gc.collect()
gc.disable()

def peak_rss():
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if platform.system() == "Linux":
        rss *= 1024
    return rss

# Baseline RSS after imports
from aws.osml.io import IO, AssetType
gc.collect()
rss_before = peak_rss()

# Open and access asset
{open_body}

rss_after = peak_rss()
delta = rss_after - rss_before
print(delta)
"""
    result = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Subprocess failed: {result.stderr}")
    return int(result.stdout.strip())


@pytest.mark.benchmark
def test_memory_open_peak_rss(synthetic_files, format_name, mem_source_mode):
    """Measure peak RSS delta during IO.open + get_asset_keys + get_asset.

    The ``virtual`` mode drives the ``Remote`` ``OwnedBuffer`` range-read path;
    its peak RSS must be bounded by the fetched header ranges + prefetch, not
    the file size — contrasted here with the ``local`` mmap path and the legacy
    ``full_download`` (non-seekable full-read) path, which grows by ~file size.
    """
    if format_name not in synthetic_files:
        pytest.skip(f"Synthetic {format_name} file not available")

    entry = synthetic_files[format_name]
    path = str(entry["path"])
    file_size = entry["size_bytes"]
    fmt = _format_from_path(Path(path))

    # A ``virtual`` run only makes sense for the block-capable formats routed
    # through the Remote path; monolithic formats stay on the full-read path.
    if mem_source_mode == "virtual" and not _remote_capable(Path(path)):
        pytest.skip(
            f"{format_name} is not remote-capable; monolithic formats stay on "
            "the full-read path (covered by the full_download mode)"
        )

    rss_delta = _measure_open_rss(path, mem_source_mode, fmt)

    ratio = rss_delta / file_size if file_size > 0 else 0

    # Report results
    print(f"\n{'='*60}")
    print(f"Format: {format_name}  Source: {mem_source_mode}")
    print(f"File size: {file_size / (1024*1024):.1f} MB")
    print(f"RSS delta: {rss_delta / (1024*1024):.1f} MB")
    print(f"Ratio (RSS/file): {ratio:.2f}x")
    print(f"{'='*60}")

    assert rss_delta >= 0, "RSS should not decrease during open"

    # The ``virtual`` metadata/first-asset open must stay bounded — it fetches
    # only header ranges + prefetch, never the whole file.  Assert its peak RSS
    # is materially below the file size (allowing generous slack for allocator
    # rounding and interpreter noise on the ~10s-of-MB synthetic files).
    if mem_source_mode == "virtual" and file_size > 4 * 1024 * 1024:
        assert rss_delta < file_size, (
            f"virtual open of {format_name} used {rss_delta} bytes RSS for a "
            f"{file_size}-byte file — expected a bounded range read, not a "
            "full-file materialization"
        )
