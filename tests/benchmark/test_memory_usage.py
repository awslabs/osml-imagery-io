"""Benchmark: peak memory usage during file open.

Measures peak RSS delta during IO.open() + get_asset_keys() + get_asset() for
synthetic uncompressed files across formats. The goal is to establish a baseline
showing ~2x file-size memory overhead from the redundant full-buffer copy on open.

Synthetic files are ~50 MB each (uncompressed) to isolate the mmap->heap copy
cost without codec memory dominating.

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


def _measure_open_rss(path: str) -> int:
    """Measure RSS increase from opening a file and accessing its first asset.

    Uses a fork-based approach: measure RSS before and after the open sequence.
    Because ru_maxrss is monotonically increasing (peak over process lifetime),
    we use a subprocess to get an isolated measurement.
    """
    import subprocess

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
reader = IO.open(["{path}"], "r")
keys = reader.get_asset_keys(asset_type=AssetType.Image)
if keys:
    asset = reader.get_asset(keys[0])
reader.close()

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
def test_memory_open_peak_rss(synthetic_files, format_name):
    """Measure peak RSS delta during IO.open + get_asset_keys + get_asset.

    The expected baseline behavior (before the zero-copy fix) is that peak
    RSS grows by approximately 2x the file size: once for the mmap and once
    for the heap copy made inside from_bytes().
    """
    if format_name not in synthetic_files:
        pytest.skip(f"Synthetic {format_name} file not available")

    entry = synthetic_files[format_name]
    path = str(entry["path"])
    file_size = entry["size_bytes"]

    rss_delta = _measure_open_rss(path)

    ratio = rss_delta / file_size if file_size > 0 else 0

    # Report results
    print(f"\n{'='*60}")
    print(f"Format: {format_name}")
    print(f"File size: {file_size / (1024*1024):.1f} MB")
    print(f"RSS delta: {rss_delta / (1024*1024):.1f} MB")
    print(f"Ratio (RSS/file): {ratio:.2f}x")
    print(f"{'='*60}")

    # The test passes regardless — this is a measurement, not an assertion.
    # The ratio is recorded for before/after comparison.
    assert rss_delta >= 0, "RSS should not decrease during open"
