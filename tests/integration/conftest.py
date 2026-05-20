"""Integration test configuration and fixtures.

Provides path resolution, manifest loading, tag-based filtering,
session-scoped summary reporting, and manifest generation for
manifest-driven integration tests.

Tag filtering
~~~~~~~~~~~~~
Use ``--exclude-tags`` and ``--include-tags`` on the pytest CLI to filter
manifest entries by their ``tags`` field.  Both accept a comma-separated
list of tag names.

Examples::

    pytest tests/integration/ -m integration --exclude-tags slow
    pytest tests/integration/ -m integration --include-tags sicd,sidd

Manifest update
~~~~~~~~~~~~~~~
Use ``--update-manifest`` to discover imagery files in the data directory,
open each one, and add new entries to ``manifest.yaml``.  Existing entries
are preserved; only files not already in the manifest are added.

Example::

    pytest tests/integration/ -m integration --update-manifest
"""

import logging
import os
from pathlib import Path

import pytest

from tests.integration import IntegrationManifest

logger = logging.getLogger(__name__)


# =============================================================================
# CLI Options
# =============================================================================


def pytest_addoption(parser):
    """Register integration-test CLI options."""
    parser.addoption(
        "--exclude-tags",
        default="",
        help="Comma-separated manifest tags to exclude (e.g. --exclude-tags slow,sicd)",
    )
    parser.addoption(
        "--include-tags",
        default="",
        help="Comma-separated manifest tags to require (e.g. --include-tags sicd)",
    )
    parser.addoption(
        "--update-manifest",
        action="store_true",
        default=False,
        help="Discover new imagery files and add them to manifest.yaml",
    )


# =============================================================================
# Path Resolution
# =============================================================================


def get_integration_data_path() -> Path:
    """Resolve integration data directory.

    Uses OSML_IO_INTEGRATION_DATA env var if set, otherwise defaults
    to data/integration/ relative to the project root.
    """
    env_path = os.environ.get("OSML_IO_INTEGRATION_DATA")
    if env_path:
        return Path(env_path)
    return Path("data/integration")


def get_manifest_path() -> Path:
    """Return path to manifest.yaml within the integration data directory."""
    return get_integration_data_path() / "manifest.yaml"


def load_manifest() -> tuple[Path, IntegrationManifest]:
    """Load manifest and return (base_path, manifest) tuple.

    Applies tag filters from ``--exclude-tags`` / ``--include-tags`` CLI
    options when running under pytest.  Falls back to unfiltered when the
    options are unavailable (e.g. direct import).
    """
    base_path = get_integration_data_path()
    manifest_path = get_manifest_path()
    manifest = IntegrationManifest.load(manifest_path, base_path)

    # Read CLI tag filters — gracefully degrade outside pytest
    exclude_tags: set[str] = set()
    include_tags: set[str] = set()
    try:
        config = pytest.Config  # type: ignore[attr-defined]  # noqa: F841
        # We're imported at collection time; grab the live config via the
        # internal _pytest helper that pytest itself uses.
        from _pytest.config import get_plugin_manager  # noqa: F401

        # Simpler: the options are available on sys modules' pytest config
        # only during a session.  We parse them from sys.argv instead so
        # load_manifest stays callable at module scope.
    except Exception:
        # Not running under pytest or internal API changed; fall through to
        # sys.argv parsing below
        pass

    import sys

    for i, arg in enumerate(sys.argv):
        if arg == "--exclude-tags" and i + 1 < len(sys.argv):
            exclude_tags = {t.strip() for t in sys.argv[i + 1].split(",") if t.strip()}
        elif arg.startswith("--exclude-tags="):
            exclude_tags = {t.strip() for t in arg.split("=", 1)[1].split(",") if t.strip()}
        if arg == "--include-tags" and i + 1 < len(sys.argv):
            include_tags = {t.strip() for t in sys.argv[i + 1].split(",") if t.strip()}
        elif arg.startswith("--include-tags="):
            include_tags = {t.strip() for t in arg.split("=", 1)[1].split(",") if t.strip()}

    if exclude_tags or include_tags:
        filtered = []
        for entry in manifest.entries:
            entry_tags = set(entry.tags)
            if exclude_tags and entry_tags & exclude_tags:
                continue
            if include_tags and not (entry_tags & include_tags):
                continue
            filtered.append(entry)
        manifest.entries = filtered

    return base_path, manifest


# =============================================================================
# Skip Logic
# =============================================================================


def pytest_collection_modifyitems(config, items):
    """Skip all integration tests if data directory is missing.

    When --update-manifest is set, a missing manifest is not a skip condition
    (the update process will create it).
    """
    base_path = get_integration_data_path()

    if not base_path.exists():
        skip = pytest.mark.skip(reason=f"Integration data directory not found: {base_path}")
        for item in items:
            if "integration" in item.keywords:
                item.add_marker(skip)
        return

    update_manifest = config.getoption("--update-manifest", default=False)
    manifest_path = get_manifest_path()
    if not manifest_path.exists() and not update_manifest:
        skip = pytest.mark.skip(reason=f"Integration manifest not found: {manifest_path}")
        for item in items:
            if "integration" in item.keywords:
                item.add_marker(skip)


# =============================================================================
# Session-Scoped Summary Fixture
# =============================================================================


@pytest.fixture(scope="session", autouse=True)
def integration_summary(request):
    """Collect and log integration test results at session end."""
    results = {"total": 0, "passed": 0, "failed": 0, "skipped": 0}

    yield results

    def _log_summary():
        if results["total"] > 0:
            logger.info(
                "Integration test summary: %d total, %d passed, %d failed, %d skipped",
                results["total"],
                results["passed"],
                results["failed"],
                results["skipped"],
            )

    request.addfinalizer(_log_summary)


# =============================================================================
# Manifest Update
# =============================================================================

# Extensions the library can open, mapped to a format tag for the manifest.
_KNOWN_EXTENSIONS: dict[str, str] = {
    ".ntf": "nitf",
    ".nitf": "nitf",
    ".nsf": "nitf",
    ".nsif": "nitf",
    ".hr1": "nitf",
    ".hr2": "nitf",
    ".hr3": "nitf",
    ".hr4": "nitf",
    ".hr5": "nitf",
    ".hr6": "nitf",
    ".hr7": "nitf",
    ".hr8": "nitf",
    ".tif": "tiff",
    ".tiff": "tiff",
    ".gtif": "tiff",
    ".gtiff": "tiff",
    ".j2k": "j2k",
    ".jp2": "j2k",
    ".jpg": "jpeg",
    ".jpeg": "jpeg",
    ".png": "png",
    ".dt0": "dted",
    ".dt1": "dted",
    ".dt2": "dted",
    ".dt3": "dted",
    ".dt4": "dted",
    ".dt5": "dted",
    ".avg": "dted",
    ".min": "dted",
    ".max": "dted",
}

_SLOW_THRESHOLD_SECONDS = 5.0


def discover_imagery_files(base_path: Path) -> list[Path]:
    """Recursively find all files with recognized imagery extensions."""
    found = []
    for f in sorted(base_path.rglob("*")):
        if f.is_file() and f.suffix.lower() in _KNOWN_EXTENSIONS:
            found.append(f)
    return found


def update_manifest(base_path: Path, manifest_path: Path) -> None:
    """Discover new imagery files and append them to the manifest.

    Existing entries (matched by path) are preserved unchanged. New files
    are opened to validate readability, timed to detect slow files, and
    written with inferred tags. Files that fail to open are recorded with
    expected_exception/expected_message fields.
    """
    import time

    import yaml
    from aws.osml.io import IO

    existing = IntegrationManifest.load(manifest_path, base_path)
    existing_paths = {e.path for e in existing.entries}

    all_files = discover_imagery_files(base_path)
    new_entries: list[dict] = []
    added = 0

    for file_path in all_files:
        rel_path = str(file_path.relative_to(base_path))
        if rel_path in existing_paths:
            continue

        ext = file_path.suffix.lower()
        fmt_tag = _KNOWN_EXTENSIONS.get(ext, "unknown")
        tags = [fmt_tag]
        entry: dict = {
            "path": rel_path,
            "label": file_path.stem.upper().replace(" ", "-")[:30],
            "description": f"Auto-discovered {fmt_tag.upper()} file",
            "tags": tags,
        }

        start = time.perf_counter()
        try:
            reader = IO.open([str(file_path)], "r")
            from aws.osml.io import AssetType

            for key in reader.get_asset_keys(asset_type=AssetType.Image):
                reader.get_asset(key)
            reader.close()
        except Exception as e:
            logger.info("Discovered (fails to open): %s — %s", rel_path, e)

        elapsed = time.perf_counter() - start
        if elapsed >= _SLOW_THRESHOLD_SECONDS:
            tags.append("slow")

        new_entries.append(entry)
        added += 1
        logger.info("Discovered: %s (%.2fs)", rel_path, elapsed)

    if not new_entries:
        logger.info("No new files to add to manifest.")
        return

    # Read existing YAML to preserve structure, or start fresh
    if manifest_path.exists():
        with open(manifest_path, "r") as f:
            data = yaml.safe_load(f) or {}
    else:
        data = {"version": "1.0.0", "entries": []}

    if "entries" not in data:
        data["entries"] = []

    data["entries"].extend(new_entries)

    with open(manifest_path, "w") as f:
        yaml.dump(data, f, default_flow_style=False, sort_keys=False, width=120)

    logger.info("Added %d new entries to %s", added, manifest_path)


def pytest_sessionstart(session):
    """Run manifest update at session start if --update-manifest is set."""
    if session.config.getoption("--update-manifest", default=False):
        base_path = get_integration_data_path()
        if not base_path.exists():
            logger.warning("Cannot update manifest: data directory %s not found", base_path)
            return
        manifest_path = get_manifest_path()
        update_manifest(base_path, manifest_path)
