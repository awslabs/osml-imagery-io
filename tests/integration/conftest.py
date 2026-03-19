"""Integration test configuration and fixtures.

Provides path resolution, manifest loading, tag-based filtering, and
session-scoped summary reporting for manifest-driven integration tests.

Tag filtering
~~~~~~~~~~~~~
Use ``--exclude-tags`` and ``--include-tags`` on the pytest CLI to filter
manifest entries by their ``tags`` field.  Both accept a comma-separated
list of tag names.

Examples::

    pytest tests/integration/ -m integration --exclude-tags slow
    pytest tests/integration/ -m integration --include-tags sicd,sidd
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
    """Register --exclude-tags and --include-tags CLI options."""
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
    """Skip all integration tests if data directory or manifest is missing."""
    base_path = get_integration_data_path()

    if not base_path.exists():
        skip = pytest.mark.skip(reason=f"Integration data directory not found: {base_path}")
        for item in items:
            if "integration" in item.keywords:
                item.add_marker(skip)
        return

    manifest_path = get_manifest_path()
    if not manifest_path.exists():
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
