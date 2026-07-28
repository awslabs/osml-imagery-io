"""Shared fixtures and pytest configuration for benchmark tests.

This module provides:
- Config loading from data/benchmark/benchmark_datasets.yaml
- Path resolution with OSML_IO_BENCHMARK_DATA env var override
- dataset_entry parametrized fixture yielding {"path": Path, "label": str} dicts
- zarr_read_params parametrized fixture for Zarr read benchmarks (local + S3)
- pytest-benchmark defaults: warmup_rounds=0, min_rounds=5

Benchmark results represent cold-start timing. Each timed iteration opens the
dataset from scratch so measurements reflect worst-case / first-access
performance. OS page cache effects may still influence results for repeated
iterations on the same file.

Usage:
    pytest -m benchmark
    pytest -m benchmark --benchmark-autosave
"""

import contextlib
import io
import logging
import os
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import pytest
import yaml

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_CONFIG_RELATIVE_PATH = Path("data/benchmark/benchmark_datasets.yaml")
_DEFAULT_BASE_DIR = Path("data/benchmark")
_ENV_VAR = "OSML_IO_BENCHMARK_DATA"
_S3_BUCKET_ENV = "OSML_IO_BENCHMARK_S3_BUCKET"

# ---------------------------------------------------------------------------
# Config loading helpers (module-level, importable for testing)
# ---------------------------------------------------------------------------


def _find_project_root() -> Path:
    """Walk up from this file to find the project root (contains pyproject.toml)."""
    current = Path(__file__).resolve().parent
    while current != current.parent:
        if (current / "pyproject.toml").exists():
            return current
        current = current.parent
    # Fallback: two levels up from tests/benchmark/conftest.py
    return Path(__file__).resolve().parent.parent.parent


def load_benchmark_config(config_path: Path) -> List[Dict[str, Any]]:
    """Parse benchmark_datasets.yaml and return the raw dataset entries.

    Returns an empty list when the file is missing, unparseable, or contains
    no ``datasets`` key.
    """
    if not config_path.exists():
        logger.info("Benchmark config not found at %s — no datasets configured.", config_path)
        return []

    try:
        with open(config_path, "r") as fh:
            data = yaml.safe_load(fh)
    except yaml.YAMLError as exc:
        logger.warning("Failed to parse benchmark config %s: %s", config_path, exc)
        return []

    if not isinstance(data, dict):
        logger.warning("Benchmark config is not a mapping — expected top-level 'datasets' key.")
        return []

    datasets = data.get("datasets")
    if not isinstance(datasets, list):
        logger.warning("Benchmark config 'datasets' key is missing or not a list.")
        return []

    return datasets


def resolve_datasets(
    raw_entries: List[Dict[str, Any]],
    base_dir: Path,
) -> List[Dict[str, Any]]:
    """Resolve raw config entries to absolute paths, filtering non-existent files.

    Parameters
    ----------
    raw_entries:
        List of dicts with at least a ``path`` key and optional ``label``.
    base_dir:
        Base directory for resolving relative paths.

    Returns
    -------
    List of ``{"path": Path, "label": str}`` dicts for files that exist on disk.
    """
    resolved: List[Dict[str, Any]] = []
    for entry in raw_entries:
        if not isinstance(entry, dict) or "path" not in entry:
            logger.warning("Skipping benchmark dataset entry missing 'path': %s", entry)
            continue

        raw_path = Path(entry["path"])
        abs_path = raw_path if raw_path.is_absolute() else base_dir / raw_path
        abs_path = abs_path.resolve()

        if not abs_path.exists():
            logger.warning("Benchmark dataset not found, skipping: %s", abs_path)
            continue

        label = entry.get("label") or abs_path.stem
        resolved.append({"path": abs_path, "label": str(label)})

    return resolved


def get_base_dir(project_root: Path) -> Path:
    """Return the base directory for resolving relative dataset paths.

    Uses ``OSML_IO_BENCHMARK_DATA`` env var when set, otherwise falls back
    to ``<project_root>/data/benchmark/``.
    """
    env_override = os.environ.get(_ENV_VAR)
    if env_override:
        return Path(env_override).resolve()
    return (project_root / _DEFAULT_BASE_DIR).resolve()


# ---------------------------------------------------------------------------
# Resolve datasets at module load time so parametrize IDs are available
# ---------------------------------------------------------------------------

_project_root = _find_project_root()
_config_path = _project_root / _CONFIG_RELATIVE_PATH
_raw_entries = load_benchmark_config(_config_path)
_base_dir = get_base_dir(_project_root)
_resolved_datasets = resolve_datasets(_raw_entries, _base_dir)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(params=_resolved_datasets, ids=lambda d: d["label"])
def dataset_entry(request) -> Dict[str, Any]:
    """Yield ``{"path": Path, "label": str}`` for each available benchmark dataset."""
    return request.param


# ---------------------------------------------------------------------------
# Block read parametrisation
# ---------------------------------------------------------------------------

def compute_access_patterns(
    grid_rows: int,
    grid_cols: int,
    tile_h: int,
    tile_w: int,
    total_rows: int,
    total_cols: int,
) -> List[Dict[str, Any]]:
    """Compute access patterns from grid dimensions.

    Parameters
    ----------
    grid_rows:
        Number of tile rows in the grid.
    grid_cols:
        Number of tile columns in the grid.
    tile_h:
        Height of each tile in pixels.
    tile_w:
        Width of each tile in pixels.
    total_rows:
        Total image height in pixels.
    total_cols:
        Total image width in pixels.

    Returns
    -------
    List of dicts with ``"name"`` and ``"regions"`` keys. Each region is a
    ``(row_start, row_end, col_start, col_end)`` tuple satisfying
    ``0 <= start < end <= total``.
    """
    patterns: List[Dict[str, Any]] = []

    # Single tile — center of grid
    cr, cc = grid_rows // 2, grid_cols // 2
    patterns.append({
        "name": "single_tile",
        "regions": [(cr * tile_h, min((cr + 1) * tile_h, total_rows),
                     cc * tile_w, min((cc + 1) * tile_w, total_cols))],
    })

    # Small ROI — 3×3 block around center (capped at grid)
    r0 = max(0, cr - 1)
    r1 = min(grid_rows, cr + 2)
    c0 = max(0, cc - 1)
    c1 = min(grid_cols, cc + 2)
    patterns.append({
        "name": "small_roi",
        "regions": [(r0 * tile_h, min(r1 * tile_h, total_rows),
                     c0 * tile_w, min(c1 * tile_w, total_cols))],
    })

    # Large ROI — 10×10 block (only if grid is large enough)
    if grid_rows >= 10 and grid_cols >= 10:
        r0 = max(0, cr - 5)
        r1 = min(grid_rows, cr + 5)
        c0 = max(0, cc - 5)
        c1 = min(grid_cols, cc + 5)
        patterns.append({
            "name": "large_roi",
            "regions": [(r0 * tile_h, min(r1 * tile_h, total_rows),
                         c0 * tile_w, min(c1 * tile_w, total_cols))],
        })

    return patterns


# ---------------------------------------------------------------------------
# Tile index generation helper and cache
# ---------------------------------------------------------------------------


def _first_image_segments(store) -> list[str]:
    """Return a list containing the first image segment key from a ManifestStore.

    Inspects the store's arrays (flat) or groups (hierarchical) and returns
    the first key that looks like an image segment.  Returns an empty list
    if no image segments are found, which causes ``write_tile_index`` to
    include all segments.
    """
    group = store._group
    # Flat store: arrays are keyed by segment name (e.g. "image:0")
    if group.arrays:
        for key in group.arrays:
            if key.startswith("image:") or key.startswith("image_segment_"):
                return [key]
        # If no image-prefixed key, return the first array key
        first = next(iter(group.arrays), None)
        return [first] if first else []
    # Hierarchical store: subgroups are numbered ("0", "1", ...)
    if group.groups:
        first = next(iter(group.groups), None)
        return [first] if first else []
    return []


def _discover_rset_files(base_path: Path) -> list[str]:
    """Discover R-set companion files for a base NITF path.

    Looks for files named ``<base>.r1``, ``<base>.r2``, etc. alongside the
    base file.  Returns a list of all paths (base + companions) if any
    companions exist, otherwise returns a single-element list with the base.
    """
    paths = [str(base_path)]
    i = 1
    while True:
        companion = base_path.parent / f"{base_path.name}.r{i}"
        if companion.exists():
            paths.append(str(companion))
            i += 1
        else:
            break
    return paths


def _generate_tile_index(dataset_path: Path, cache: dict, tmp_dir: Path,
                         source_url: str | None = None) -> Path:
    """Generate tile index for a dataset, caching the result.

    Uses ``OversightMLParser`` + ``write_tile_index()`` to produce a Kerchunk
    JSON tile index. Results are cached by (dataset_path, source_url) so
    repeated calls return the previously generated index.

    Automatically discovers R-set companion files (``*.r1``, ``*.r2``, etc.)
    for NITF datasets so multi-resolution pyramids are fully indexed.

    Parameters
    ----------
    dataset_path:
        Absolute path to the local imagery file (used for parsing).
    cache:
        Dict mapping cache key → ``Path`` of generated index.
    tmp_dir:
        Directory where generated index files are written.
    source_url:
        URL to embed in the tile index references. When ``None``, uses a
        ``file://`` URI for the local path. Set to an ``s3://`` URI to
        produce an index that reads from S3.

    Returns
    -------
    Path to the generated tile index JSON file.
    """
    url = source_url or str(dataset_path)
    key = f"{dataset_path}|{url}"
    if key not in cache:
        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        # Parse the local file directly; the parser auto-discovers R-set
        # companion files (``*.r1``, ``*.r2``, …) from the base path.
        parser = OversightMLParser()
        store = parser(str(dataset_path))

        # For the S3 variant, index the local copy but rewrite chunk-reference
        # URLs to the S3 location the data will be served from. Map the base
        # path and each R-set companion to the corresponding S3 URL.
        url_overrides = None
        if source_url:
            local_paths = _discover_rset_files(dataset_path)
            url_overrides = {}
            for p in local_paths:
                rset = p[len(str(dataset_path)):]  # "" for base, ".r1", ...
                url_overrides[p] = f"{source_url}{rset}"

        suffix = "_s3" if source_url else ""
        index_path = tmp_dir / f"{dataset_path.stem}{suffix}.tile_index.json"
        # Discover the first image segment dynamically to avoid brittle
        # hardcoded segment names (keys changed from image_segment_N to image:N).
        image_segments = _first_image_segments(store)
        write_tile_index(
            store, str(index_path), segments=image_segments,
            url_overrides=url_overrides,
        )
        cache[key] = index_path
    return cache[key]


@pytest.fixture(scope="session")
def tile_index_cache(tmp_path_factory):
    """Session-scoped cache for generated tile indices."""
    return {}


# ---------------------------------------------------------------------------
# S3 benchmark support
# ---------------------------------------------------------------------------


@pytest.fixture(scope="session")
def s3_benchmark_bucket():
    """Return the S3 bucket URL for benchmarks, or skip if not configured."""
    bucket = os.environ.get(_S3_BUCKET_ENV)
    if not bucket:
        pytest.skip(f"S3 benchmarks require {_S3_BUCKET_ENV} environment variable")
    return bucket


# ---------------------------------------------------------------------------
# Virtual source dimension
# ---------------------------------------------------------------------------
#
# The ``local`` benchmarks open datasets by path (mmap; direct disk access).  The
# ``virtual`` source mode exercises the demand-paged ``Remote`` ``OwnedBuffer``
# path introduced by the remote range-read design: reads flow through a seekable,
# sized Python file-like handle (virtualized IO) and are served on demand instead
# of a full download.
#
# Everything here runs offline — no S3 credentials required — by reading the
# same local benchmark files through a byte-range-counting file-like object (or,
# for the parser/index-gen path, a byte-counting ``file://`` fsspec filesystem).
# This keeps ``--skip-s3`` / the offline default fully functional while still
# quantifying the range-read reduction via the recorded bytes-fetched metric.


class ByteCountingStream:
    """A seekable, sized file-like wrapper that counts every ``read`` range.

    Wraps an in-memory ``BytesIO`` and records ``(offset, length)`` for each
    read so a benchmark can assert the reader pulled byte ranges on demand
    (``Remote`` ``OwnedBuffer`` path) rather than downloading the whole file,
    and can report ``bytes_fetched`` quantitatively.

    Presented to ``IO.open`` this looks like an fsspec/s3fs handle: it exposes
    ``seekable()`` and a ``size`` attribute, so the binding routes it through
    the ``Remote`` backing for the block-capable formats.
    """

    def __init__(self, data: bytes):
        self._buf = io.BytesIO(data)
        self.size = len(data)
        self.reads: List[Tuple[int, int]] = []
        self.bytes_fetched = 0

    def seekable(self) -> bool:
        return True

    def seek(self, offset, whence=0):
        return self._buf.seek(offset, whence)

    def tell(self):
        return self._buf.tell()

    def read(self, n=-1):
        pos = self._buf.tell()
        b = self._buf.read(n)
        self.reads.append((pos, len(b)))
        self.bytes_fetched += len(b)
        return b

    def close(self):
        self._buf.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False


@contextlib.contextmanager
def counting_local_filesystem():
    """Register a byte-counting ``file`` fsspec filesystem for the duration.

    Yields a mutable ``{"bytes_fetched": int, "reads": [(offset, length), ...]}``
    counter that accumulates every ``read`` issued through fsspec's ``file``
    protocol.  Used by the index-gen ``remote`` benchmark, where
    ``OversightMLParser`` opens the ``file://`` URL itself: overriding the
    ``file`` protocol lets us observe the range reads it issues without S3.

    The original ``file`` implementation is restored on exit and the fsspec
    instance cache is cleared on both entry and exit so the override takes and
    releases cleanly regardless of prior fsspec use.
    """
    import fsspec
    from fsspec.implementations.local import LocalFileSystem

    counter: Dict[str, Any] = {"bytes_fetched": 0, "reads": []}

    class _CountingLocalFile:
        def __init__(self, inner):
            self._inner = inner

        def seekable(self) -> bool:
            return True

        def seek(self, offset, whence=0):
            return self._inner.seek(offset, whence)

        def tell(self):
            return self._inner.tell()

        def read(self, n=-1):
            pos = self._inner.tell()
            b = self._inner.read(n)
            counter["bytes_fetched"] += len(b)
            counter["reads"].append((pos, len(b)))
            return b

        @property
        def size(self):
            return self._inner.size

        def close(self):
            return self._inner.close()

        # fsspec's cat_file (which cat_ranges drives) uses the handle as a
        # context manager: ``with self.open(path) as f: f.seek(...); f.read(...)``.
        # ``with`` resolves __enter__/__exit__ on the TYPE, so __getattr__ can't
        # supply them — they must be defined explicitly or cat_ranges raises a
        # TypeError (which, under on_error="return", lands in the result list as a
        # non-bytes element and breaks the range-read path).
        def __enter__(self):
            return self

        def __exit__(self, *exc):
            self._inner.close()
            return False

        def __getattr__(self, name):
            return getattr(self._inner, name)

    class _CountingLocalFileSystem(LocalFileSystem):
        def _open(self, path, mode="rb", *args, **kwargs):
            return _CountingLocalFile(super()._open(path, mode, *args, **kwargs))

    original = fsspec.get_filesystem_class("file")
    fsspec.register_implementation("file", _CountingLocalFileSystem, clobber=True)
    LocalFileSystem.clear_instance_cache()
    try:
        yield counter
    finally:
        fsspec.register_implementation("file", original, clobber=True)
        LocalFileSystem.clear_instance_cache()


# Formats whose readers are wired through the ``Remote`` ``OwnedBuffer`` path
# (see ``remote_header_hint`` in ``src/bindings/io.rs``).  Only these can be
# exercised over a range-read handle; monolithic formats (PNG, standalone JPEG)
# stay on the full-read fallback and are skipped for the ``remote`` dimension.
_REMOTE_CAPABLE_FORMATS = frozenset({"nitf", "tiff", "geotiff", "j2k", "jp2", "dted"})


def _format_from_path(path: Path) -> Optional[str]:
    """Return the ``IO.open`` stream format for *path*, or ``None`` if unknown.

    Mirrors the extension→format mapping the parser and binding use so the
    ``remote`` source mode can pass an explicit ``format=`` to ``IO.open``
    (a file-like handle has no filename to auto-detect from).
    """
    ext = path.suffix.lower().lstrip(".")
    if ext in ("ntf", "nitf", "nsif", "nsf") or (
        len(ext) == 3 and ext.startswith("hr")
    ):
        return "nitf"
    if ext in ("tif", "tiff", "gtif", "gtiff"):
        return "tiff"
    if ext in ("j2k", "jp2", "jpx"):
        return "j2k"
    if ext in ("png",):
        return "png"
    if ext in ("jpg", "jpeg"):
        return "jpeg"
    if ext in ("dt0", "dt1", "dt2", "dt3", "dt4", "dt5", "avg", "min", "max"):
        return "dted"
    return None


def _remote_capable(path: Path) -> bool:
    """True when *path*'s format is wired through the ``Remote`` buffer path."""
    fmt = _format_from_path(path)
    return fmt is not None and fmt in _REMOTE_CAPABLE_FORMATS


def _s3_uri_for(dataset_path: Path) -> Optional[str]:
    """Return the ``s3://`` URI mirroring *dataset_path*, or ``None``.

    Uses the same local-path→S3-URI mapping as the Zarr S3 benchmarks (relative
    to the benchmark data dir), so a single ``aws s3 sync`` populates the bucket
    for both.  Returns ``None`` when ``OSML_IO_BENCHMARK_S3_BUCKET`` is unset.
    """
    bucket = os.environ.get(_S3_BUCKET_ENV)
    if not bucket:
        return None
    try:
        rel = dataset_path.relative_to(_base_dir)
    except ValueError:
        rel = Path(dataset_path.name)
    return f"{bucket.rstrip('/')}/{rel}"


class CountingS3File:
    """Wrap a real ``s3fs`` file handle, counting every ``read`` range.

    Presented to ``IO.open`` this is a seekable, sized file-like object backed
    by S3, so the binding routes it through the ``Remote`` ``OwnedBuffer`` path
    and each ``read`` becomes an HTTP range GET.  The ``(offset, length)`` log
    and ``bytes_fetched`` counter quantify the range-read reduction against the
    object size — the true-S3 counterpart to the offline ``ByteCountingStream``.
    """

    def __init__(self, inner):
        self._inner = inner
        self.reads: List[Tuple[int, int]] = []
        self.bytes_fetched = 0

    def seekable(self) -> bool:
        return True

    def seek(self, offset, whence=0):
        return self._inner.seek(offset, whence)

    def tell(self):
        return self._inner.tell()

    def read(self, n=-1):
        pos = self._inner.tell()
        b = self._inner.read(n)
        self.reads.append((pos, len(b)))
        self.bytes_fetched += len(b)
        return b

    @property
    def size(self):
        # s3fs exposes .size; fall back to the fs info if absent.
        return getattr(self._inner, "size", None)

    def close(self):
        return self._inner.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False

    def __getattr__(self, name):
        return getattr(self._inner, name)


def _close_s3_session(fs) -> None:
    """Best-effort teardown of an ``s3fs`` filesystem's aiohttp session.

    Each ``S3FileSystem`` opens an ``aiobotocore``/``aiohttp`` ``ClientSession``
    on a background event-loop thread.  Because the benchmark opens a fresh,
    uncached filesystem per round (for cold-start isolation) and never reuses
    them, those sessions would otherwise be reclaimed only at interpreter
    shutdown — where the GC prints noisy "Unclosed client session / Unclosed
    connector" diagnostics.  Closing the session when the helper's context
    manager exits keeps the benchmark log clean.

    Swallows every error: ``close_session`` itself races the background loop at
    shutdown (its own implementation does the same), and this is pure cosmetic
    cleanup that must never fail a benchmark.
    """
    try:
        fs.close_session(fs.loop, fs.s3)
    except Exception:
        pass


@contextlib.contextmanager
def open_counting_s3(uri: str):
    """Open *uri* via ``s3fs`` and yield a range-counting handle.

    Clears the s3fs instance cache first so each open is a cold-start with no
    cached directory listings or content.  Requires ``s3fs`` and AWS
    credentials in the environment.
    """
    import fsspec
    import s3fs

    s3fs.S3FileSystem.clear_instance_cache()
    fs = fsspec.filesystem("s3", skip_instance_cache=True)
    # cache_type="none": disable s3fs's read-ahead block cache so each read maps
    # to a direct range GET.  The Remote OwnedBuffer already caches fetched
    # ranges, so an s3fs cache on top is redundant — and, worse, its 5 MB
    # read-ahead blocks thrash (re-fetch) under OpenJPEG's seek-heavy scan and
    # make bytes_fetched under-count actual network traffic.
    inner = fs.open(uri, "rb", cache_type="none")
    handle = CountingS3File(inner)

    # Count bytes at BOTH fetch layers.  After the concurrent-read lock refactor,
    # an fsspec-backed source fetches ranges through ``fs.cat_ranges(...)`` on the
    # filesystem — the cursor-free path — bypassing the handle's ``.read()``
    # entirely.  Counting only ``.read()`` (which ``CountingS3File`` wraps) would
    # therefore observe ZERO reads for the range-read path and fail the
    # ``assert reads`` guard.  Wrapping ``cat_ranges`` on this fresh, uncached fs
    # instance tallies those range GETs into the same handle counters; the two
    # layers never fetch the same bytes, so summing is correct.  (cat_ranges'
    # internal GETs do not go through our ``inner`` handle, so there is no
    # double-count.)
    real_cat_ranges = fs.cat_ranges

    def counting_cat_ranges(paths, starts, ends, *args, **kwargs):
        result = real_cat_ranges(paths, starts, ends, *args, **kwargs)
        for start, chunk in zip(starts, result):
            handle.reads.append((start, len(chunk)))
            handle.bytes_fetched += len(chunk)
        return result

    fs.cat_ranges = counting_cat_ranges
    try:
        yield handle
    finally:
        handle.close()
        _close_s3_session(fs)


@contextlib.contextmanager
def counting_s3_filesystem():
    """Register a byte-counting ``s3`` fsspec filesystem for the duration.

    Yields a mutable ``{"bytes_fetched": int, "reads": [(offset, length), ...]}``
    counter accumulating every ``read`` issued through fsspec's ``s3`` protocol.
    Used by the index-gen ``s3`` benchmark, where ``OversightMLParser`` opens the
    ``s3://`` URL itself: wrapping the ``s3`` filesystem lets us observe the HTTP
    range GETs it issues.  The original ``s3`` implementation is restored on exit
    and the instance cache cleared on both entry and exit.

    Requires ``s3fs`` and AWS credentials.
    """
    import fsspec
    import s3fs

    counter: Dict[str, Any] = {"bytes_fetched": 0, "reads": []}

    class _CountingS3File:
        def __init__(self, inner):
            self._inner = inner

        def seekable(self) -> bool:
            return True

        def seek(self, offset, whence=0):
            return self._inner.seek(offset, whence)

        def tell(self):
            return self._inner.tell()

        def read(self, n=-1):
            pos = self._inner.tell()
            b = self._inner.read(n)
            counter["bytes_fetched"] += len(b)
            counter["reads"].append((pos, len(b)))
            return b

        @property
        def size(self):
            return getattr(self._inner, "size", None)

        def close(self):
            return self._inner.close()

        def __enter__(self):
            return self

        def __exit__(self, *exc):
            self.close()
            return False

        def __getattr__(self, name):
            return getattr(self._inner, name)

    class _CountingS3FileSystem(s3fs.S3FileSystem):
        def _open(self, path, mode="rb", *args, **kwargs):
            # Disable the read-ahead block cache (see open_counting_s3) so reads
            # map to direct range GETs and the byte count reflects real traffic.
            kwargs.setdefault("cache_type", "none")
            return _CountingS3File(super()._open(path, mode, *args, **kwargs))

        async def _cat_file(self, path, version_id=None, start=None, end=None, **kwargs):
            # After the concurrent-read lock refactor, the parser fetches ranges
            # through ``cat_ranges`` → ``_cat_ranges`` → ``_cat_file`` (async,
            # direct ``get_object`` GETs) — which NEVER go through ``_open``, so
            # the ``_open`` wrapper above sees zero bytes for the range-read path.
            # Counting at ``_cat_file`` (the seam every cat_ranges range funnels
            # through) tallies those GETs.  ``start``/``end`` bound the range; a
            # full-object read (both ``None``) counts its whole length.
            data = await super()._cat_file(
                path, version_id=version_id, start=start, end=end, **kwargs
            )
            counter["bytes_fetched"] += len(data)
            counter["reads"].append((start or 0, len(data)))
            return data

    original = fsspec.get_filesystem_class("s3")
    fsspec.register_implementation("s3", _CountingS3FileSystem, clobber=True)
    s3fs.S3FileSystem.clear_instance_cache()
    try:
        yield counter
    finally:
        # The parser opens the s3 filesystem itself, so it lands in s3fs's
        # class-level instance cache rather than in a local variable.  Close each
        # cached instance's aiohttp session before clearing the cache, so the run
        # exits without the GC's "Unclosed client session" noise (see
        # _close_s3_session).
        for cached_fs in list(s3fs.S3FileSystem._cache.values()):
            _close_s3_session(cached_fs)
        fsspec.register_implementation("s3", original, clobber=True)
        s3fs.S3FileSystem.clear_instance_cache()


def _source_mode_params() -> Tuple[List[str], List[str]]:
    """Return the ``(params, ids)`` for the ``source_mode`` fixture.

    The three modes form an IO-abstraction cost ladder: direct disk access →
    virtualized (Python file-like) IO → network IO.  ``local`` is always present
    (the default mmap path — keeps the offline default working).  ``virtual`` is
    added unconditionally: it drives the ``Remote`` range-read path through a
    Python file-like handle (offline, no network) so the range-read behavior is
    exercised without S3.  ``s3`` is added only when ``OSML_IO_BENCHMARK_S3_BUCKET``
    is set — it drives the same ``Remote`` path over a **real** s3fs handle (HTTP
    range GETs), the true-S3, non-Zarr read the feature exists to enable.
    Individual tests skip datasets whose format is not remote-capable.
    """
    modes = ["local", "virtual"]
    if os.environ.get(_S3_BUCKET_ENV):
        modes.append("s3")
    return modes, list(modes)


_source_mode_values, _source_mode_ids = _source_mode_params()


@pytest.fixture(params=_source_mode_values, ids=_source_mode_ids)
def source_mode(request) -> str:
    """Yield the benchmark source mode: ``"local"``, ``"virtual"``, or ``"s3"``.

    - ``local`` — open the dataset by path (mmap; direct disk access).
    - ``virtual`` — drive the ``Remote`` ``OwnedBuffer`` range-read path via a
      seekable, sized Python file-like handle (offline — no S3 required).
    - ``s3`` — drive the same ``Remote`` path over a real ``s3fs`` handle so
      reads are HTTP range GETs against the configured bucket (present only when
      ``OSML_IO_BENCHMARK_S3_BUCKET`` is set).
    """
    return request.param


# Default number of timed rounds for a network (``s3``) benchmark.  Each round is
# a full cold-start decode over the network, so a large real-world J2K can take
# seconds per round; 10 rounds (the local/virtual default) would run for minutes
# per case.  The network signal is per-op latency + bytes-fetched, not
# nanosecond variance, so a small round count is sufficient.  Override with
# ``OSML_IO_BENCHMARK_S3_ROUNDS``.
_DEFAULT_S3_ROUNDS = 3


def s3_rounds(default: int = _DEFAULT_S3_ROUNDS) -> int:
    """Timed-round count for ``s3`` benchmarks (env-overridable, min 1)."""
    raw = os.environ.get("OSML_IO_BENCHMARK_S3_ROUNDS")
    if raw is None:
        return default
    try:
        return max(1, int(raw))
    except ValueError:
        return default


# ---------------------------------------------------------------------------
# Zarr read parametrisation (local + S3)
# ---------------------------------------------------------------------------

# Temporary directory for tile indices generated at module load time.
# We use tempfile.mkdtemp() because pytest fixtures are not available during
# module-level parametrization.
_zarr_tmp_dir = Path(tempfile.mkdtemp(prefix="zarr_bench_"))
_zarr_index_cache: Dict[str, Path] = {}


def _discover_zarr_read_params() -> Tuple[
    List[Tuple[Dict[str, Any], Dict[str, Any], str, Optional[Dict[str, Any]], str]],
    List[str],
]:
    """Build (dataset_entry, access_pattern, index_path, remote_options, backend) tuples.

    Iterates ``_resolved_datasets``, generates a tile index for each, computes
    access patterns from grid dimensions, and produces parametrization entries
    for local (always) and S3 (when ``OSML_IO_BENCHMARK_S3_BUCKET`` is set).

    Returns ``(params_list, ids_list)`` or ``([], [])`` when dependencies are
    missing or no datasets are available.
    """
    # Dependency check — if zarr or fsspec are not importable, return empty
    # so the fixture is defined with no params (test files use importorskip).
    try:
        __import__("zarr")
        __import__("fsspec")
    except ImportError:
        return [], []

    s3_bucket = os.environ.get(_S3_BUCKET_ENV)

    params: List[Tuple[Dict[str, Any], Dict[str, Any], str, Optional[Dict[str, Any]], str]] = []
    ids: List[str] = []

    for entry in _resolved_datasets:
        dataset_path = entry["path"]
        label = entry["label"]
        try:
            # Generate tile index
            index_path = _generate_tile_index(dataset_path, _zarr_index_cache, _zarr_tmp_dir)

            # Open dataset to get grid dimensions for access patterns
            from aws.osml.io import IO, AssetType

            reader = IO.open([str(dataset_path)], "r")
            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            if not image_keys:
                reader.close()
                continue
            asset = reader.get_asset(image_keys[0])
            grid_rows, grid_cols = asset.block_grid_size
            tile_h = asset.num_pixels_per_block_vertical
            tile_w = asset.num_pixels_per_block_horizontal
            total_rows = asset.num_rows
            total_cols = asset.num_columns
            reader.close()

            # Compute access patterns for this dataset
            patterns = compute_access_patterns(
                grid_rows, grid_cols, tile_h, tile_w, total_rows, total_cols,
            )

            for pattern in patterns:
                pattern_name = pattern["name"]

                # Always add a local entry
                params.append((entry, pattern, str(index_path), None, "local"))
                ids.append(f"{label}-{pattern_name}-local")

                # Add S3 entry when bucket is configured
                if s3_bucket:
                    # Build S3 URI for the imagery file by mapping the local
                    # path relative to the benchmark data directory.
                    try:
                        rel = dataset_path.relative_to(_base_dir)
                    except ValueError:
                        rel = Path(dataset_path.name)
                    s3_data_uri = f"{s3_bucket.rstrip('/')}/{rel}"

                    # Generate a separate tile index whose references point
                    # at the S3 URI instead of the local file.
                    s3_index_path = _generate_tile_index(
                        dataset_path, _zarr_index_cache, _zarr_tmp_dir,
                        source_url=s3_data_uri,
                    )
                    remote_options: Dict[str, Any] = {
                        "remote_protocol": "s3",
                        "remote_options": {"anon": False, "asynchronous": True},
                    }
                    params.append((entry, pattern, str(s3_index_path), remote_options, "s3"))
                    ids.append(f"{label}-{pattern_name}-s3")

        except Exception:
            logger.warning("Failed to prepare Zarr read params for %s, skipping.", label, exc_info=True)
            continue

    return params, ids


_zarr_read_params, _zarr_read_ids = (
    _discover_zarr_read_params() if _resolved_datasets else ([], [])
)


@pytest.fixture(params=_zarr_read_params, ids=_zarr_read_ids)
def zarr_read_params(request):
    """Yield ``(dataset_entry, access_pattern, index_path, remote_options, backend)``.

    Local entries are always included. S3 entries are included only when
    ``OSML_IO_BENCHMARK_S3_BUCKET`` is set.
    """
    return request.param


# ---------------------------------------------------------------------------
# IO read parametrisation (access-pattern based)
# ---------------------------------------------------------------------------

def _compute_block_coords_for_region(
    row_start: int, row_end: int, col_start: int, col_end: int,
    tile_h: int, tile_w: int,
) -> List[Tuple[int, int]]:
    """Convert a pixel region to a list of (block_row, block_col) coordinates."""
    br_start = row_start // tile_h
    br_end = (row_end - 1) // tile_h + 1
    bc_start = col_start // tile_w
    bc_end = (col_end - 1) // tile_w + 1
    return [(r, c) for r in range(br_start, br_end) for c in range(bc_start, bc_end)]


def _discover_io_read_params():
    """Build (dataset_entry, access_pattern, block_coords) tuples.

    Uses the same access patterns as the Zarr read benchmarks so results
    are directly comparable.
    """
    params = []
    ids = []
    for entry in _resolved_datasets:
        path = str(entry["path"])
        label = entry["label"]
        try:
            from aws.osml.io import IO, AssetType

            reader = IO.open([path], "r")
            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            if not image_keys:
                reader.close()
                continue
            asset = reader.get_asset(image_keys[0])
            grid_rows, grid_cols = asset.block_grid_size
            tile_h = asset.num_pixels_per_block_vertical
            tile_w = asset.num_pixels_per_block_horizontal
            total_rows = asset.num_rows
            total_cols = asset.num_columns
            reader.close()
        except Exception:
            continue

        patterns = compute_access_patterns(
            grid_rows, grid_cols, tile_h, tile_w, total_rows, total_cols,
        )
        for pattern in patterns:
            block_coords = []
            for rs, re, cs, ce in pattern["regions"]:
                block_coords.extend(
                    _compute_block_coords_for_region(rs, re, cs, ce, tile_h, tile_w)
                )
            params.append((entry, pattern, block_coords))
            ids.append(f"{label}-{pattern['name']}")

    return params, ids


_io_read_params, _io_read_ids = (
    _discover_io_read_params() if _resolved_datasets else ([], [])
)


@pytest.fixture(params=_io_read_params, ids=_io_read_ids)
def io_read_params(request):
    """Yield ``(dataset_entry, access_pattern, block_coords)`` for IO read benchmarks.

    Uses the same access patterns as ``zarr_read_params`` so results are
    directly comparable.
    """
    return request.param


# ---------------------------------------------------------------------------
# pytest-benchmark defaults & skip logic
# ---------------------------------------------------------------------------


_DATASET_FIXTURES = frozenset({"dataset_entry", "io_read_params", "zarr_read_params"})


def pytest_collection_modifyitems(config, items):
    """Skip benchmark tests that require datasets when none are configured."""
    if _resolved_datasets:
        return

    skip_marker = pytest.mark.skip(
        reason="No benchmark datasets configured. "
        "Add entries to data/benchmark/benchmark_datasets.yaml or set OSML_IO_BENCHMARK_DATA."
    )
    for item in items:
        # Only skip benchmark-marked tests that depend on external dataset
        # fixtures. Self-contained benchmarks (e.g. memory_usage) run regardless.
        if item.get_closest_marker("benchmark") is not None:
            if _DATASET_FIXTURES & set(item.fixturenames):
                item.add_marker(skip_marker)


def pytest_configure(config):
    """Set pytest-benchmark defaults for cold-start isolation."""
    # These can still be overridden on the CLI.
    config.addinivalue_line("markers", "benchmark: marks benchmark tests")


@pytest.fixture(autouse=True)
def _benchmark_defaults(request):
    """Apply cold-start benchmark defaults (warmup_rounds=0, min_rounds=5).

    Only applies to tests using the ``benchmark`` fixture.
    """
    bench = request.node.funcargs.get("benchmark")
    if bench is not None:
        bench.extra_info["cold_start"] = True
