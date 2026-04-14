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


def _generate_tile_index(dataset_path: Path, cache: dict, tmp_dir: Path,
                         source_url: str | None = None) -> Path:
    """Generate tile index for a dataset, caching the result.

    Uses ``OversightMLParser`` + ``write_tile_index()`` to produce a Kerchunk
    JSON tile index. Results are cached by (dataset_path, source_url) so
    repeated calls return the previously generated index.

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

        parser = OversightMLParser(local_paths=str(dataset_path))
        store = parser(url=url)
        suffix = "_s3" if source_url else ""
        index_path = tmp_dir / f"{dataset_path.stem}{suffix}.tile_index.json"
        # Discover the first image segment dynamically to avoid brittle
        # hardcoded segment names (keys changed from image_segment_N to image:N).
        image_segments = _first_image_segments(store)
        write_tile_index(store, str(index_path), segments=image_segments)
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
# Native read parametrisation (access-pattern based)
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


def _discover_native_read_params():
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


_native_read_params, _native_read_ids = (
    _discover_native_read_params() if _resolved_datasets else ([], [])
)


@pytest.fixture(params=_native_read_params, ids=_native_read_ids)
def native_read_params(request):
    """Yield ``(dataset_entry, access_pattern, block_coords)`` for native IO benchmarks.

    Uses the same access patterns as ``zarr_read_params`` so results are
    directly comparable.
    """
    return request.param


# ---------------------------------------------------------------------------
# pytest-benchmark defaults & skip logic
# ---------------------------------------------------------------------------


def pytest_collection_modifyitems(config, items):
    """Skip benchmark tests that require datasets when none are configured."""
    if _resolved_datasets:
        return

    skip_marker = pytest.mark.skip(
        reason="No benchmark datasets configured. "
        "Add entries to data/benchmark/benchmark_datasets.yaml or set OSML_IO_BENCHMARK_DATA."
    )
    for item in items:
        # Only skip tests explicitly marked @pytest.mark.benchmark, not all
        # tests that happen to live under the tests/benchmark/ directory.
        if item.get_closest_marker("benchmark") is not None:
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
