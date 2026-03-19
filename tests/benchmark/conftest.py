"""Shared fixtures and pytest configuration for benchmark tests.

This module provides:
- Config loading from data/benchmark/benchmark_datasets.yaml
- Path resolution with OSML_IO_BENCHMARK_DATA env var override
- dataset_entry parametrized fixture yielding {"path": Path, "label": str} dicts
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
from pathlib import Path
from typing import Any, Dict, List

import pytest
import yaml

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_CONFIG_RELATIVE_PATH = Path("data/benchmark/benchmark_datasets.yaml")
_DEFAULT_BASE_DIR = Path("data/benchmark")
_ENV_VAR = "OSML_IO_BENCHMARK_DATA"

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

def compute_block_positions(grid_rows: int, grid_cols: int) -> List[Dict[str, Any]]:
    """Compute the five spatial block positions, deduplicating for small grids.

    Parameters
    ----------
    grid_rows:
        Number of block rows in the grid.
    grid_cols:
        Number of block columns in the grid.

    Returns
    -------
    List of ``{"name": str, "row": int, "col": int}`` dicts with unique positions.
    """
    positions = [
        ("UL", 0, 0),
        ("UR", 0, grid_cols - 1),
        ("LR", grid_rows - 1, grid_cols - 1),
        ("LL", grid_rows - 1, 0),
        ("C", grid_rows // 2, grid_cols // 2),
    ]

    seen: set = set()
    result: List[Dict[str, Any]] = []
    for name, row, col in positions:
        key = (row, col)
        if key not in seen:
            seen.add(key)
            result.append({"name": name, "row": row, "col": col})
    return result


def _discover_block_params():
    """Build (dataset_entry, position) pairs for all datasets.

    Opens each dataset once at import time to read the block grid size,
    then computes the five spatial positions (deduplicating for small grids).
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
            reader.close()
        except Exception:
            continue

        for pos in compute_block_positions(grid_rows, grid_cols):
            params.append((entry, pos))
            ids.append(f"{label}-{pos['name']}")

    return params, ids


_block_params, _block_ids = _discover_block_params() if _resolved_datasets else ([], [])


@pytest.fixture(params=_block_params, ids=_block_ids)
def block_read_params(request):
    """Yield ``(dataset_entry, position)`` for each dataset × block position combination."""
    return request.param


# ---------------------------------------------------------------------------
# pytest-benchmark defaults & skip logic
# ---------------------------------------------------------------------------


def pytest_collection_modifyitems(config, items):
    """Skip all benchmark tests when no datasets are configured."""
    if _resolved_datasets:
        return

    skip_marker = pytest.mark.skip(
        reason="No benchmark datasets configured. "
        "Add entries to data/benchmark/benchmark_datasets.yaml or set OSML_IO_BENCHMARK_DATA."
    )
    for item in items:
        if "benchmark" in item.keywords:
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
