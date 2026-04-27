"""Manifest-driven integration tests for all imagery formats.

Consolidates NITF, GeoTIFF, and conformance tests into a single parametrized
framework. Each manifest entry is opened via IO.open(), validated with
format-agnostic sanity checks, then dispatched to format-specific checks
based on dataset metadata.
"""

import logging
from pathlib import Path
from typing import Optional

import numpy as np
import pytest
from aws.osml.io import IO, AssetType

from tests.integration import IntegrationEntry
from tests.integration.conftest import load_manifest

logger = logging.getLogger(__name__)


# =============================================================================
# Format Detection
# =============================================================================


def detect_format(reader) -> str:
    """Detect format from dataset metadata.

    NITF/NSIF files have an FHDR key in dataset metadata.
    TIFF files have numeric tag keys (e.g. "256" for ImageWidth).

    Returns:
        'nitf', 'tiff', or 'unknown'
    """
    meta = reader.metadata.as_dict()
    fhdr = meta.get("FHDR", "")
    if isinstance(fhdr, str) and fhdr.startswith(("NITF", "NSIF")):
        return "nitf"
    if "256" in meta or "ImageWidth" in meta:
        return "tiff"
    return "unknown"


# =============================================================================
# Validation Helpers
# =============================================================================


def is_plausible_epsg(value) -> bool:
    """Return True for integers 1-32767 inclusive."""
    return isinstance(value, int) and 1 <= value <= 32767


def run_agnostic_checks(reader, base_path: Path, entry: IntegrationEntry) -> None:
    """Format-agnostic sanity checks on a valid dataset.

    Verifies that the dataset can be opened, that all segment keys are
    enumerable, and that a valid asset provider is returned for each key.
    Image-specific checks (dimensions, bands, block reads) are only run
    when image segments are present.
    """
    # All keys should be enumerable
    all_keys = reader.get_asset_keys()
    assert isinstance(all_keys, list), f"get_asset_keys() did not return a list for {entry.path}"

    # Every key should resolve to a valid asset provider
    for key in all_keys:
        assert reader.has_asset(key), f"has_asset('{key}') returned False for {entry.path}"
        asset = reader.get_asset(key)
        assert asset is not None, f"get_asset('{key}') returned None for {entry.path}"

    # Dataset-level metadata should be non-empty
    file_meta = reader.metadata.as_dict()
    assert len(file_meta) > 0, f"Empty dataset metadata for {entry.path}"

    # Run image-specific checks only when image segments exist
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    for key in image_keys:
        _check_image_asset(reader.get_asset(key), entry)


def _check_image_asset(asset, entry: IntegrationEntry) -> None:
    """Validate an individual image asset: dimensions, bands, block read."""
    key = asset.key

    # Non-empty metadata
    meta_dict = asset.get_metadata().as_dict()
    assert len(meta_dict) > 0, f"Empty metadata for image asset '{key}' in {entry.path}"

    # Positive dimensions
    assert asset.num_rows > 0, f"num_rows not positive for '{key}' in {entry.path}"
    assert asset.num_columns > 0, f"num_columns not positive for '{key}' in {entry.path}"

    # Band count
    assert asset.num_bands >= 1, f"num_bands < 1 for '{key}' in {entry.path}"

    # Block grid size
    grid_rows, grid_cols = asset.block_grid_size
    assert grid_rows > 0 and grid_cols > 0, (
        f"Invalid block_grid_size for '{key}' in {entry.path}"
    )

    # Read first block
    block = asset.get_block(0, 0, 0)
    assert isinstance(block, np.ndarray), (
        f"get_block did not return ndarray for '{key}' in {entry.path}"
    )
    assert len(block.shape) == 3, f"Block shape not 3D for '{key}' in {entry.path}"
    assert block.shape[0] == asset.num_bands, (
        f"Block first dim {block.shape[0]} != num_bands {asset.num_bands} for '{key}' in {entry.path}"
    )


def run_nitf_checks(reader) -> None:
    """NITF-specific checks: magic bytes and blocking parameters."""
    # Verify raw bytes start with NITF or NSIF
    raw_bytes = reader.metadata.raw.read()
    assert raw_bytes[:4] in (b"NITF", b"NSIF"), (
        f"NITF magic bytes expected, got {raw_bytes[:4]!r}"
    )

    # Verify blocking parameters on image segments
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    for key in image_keys:
        asset = reader.get_asset(key)
        assert asset.num_pixels_per_block_horizontal > 0, (
            f"num_pixels_per_block_horizontal not positive for {key}"
        )
        assert asset.num_pixels_per_block_vertical > 0, (
            f"num_pixels_per_block_vertical not positive for {key}"
        )


def run_tiff_checks(reader) -> None:
    """GeoTIFF-specific checks: Geo metadata, model type, CRS, pixel scale, tiepoints."""
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    if not image_keys:
        return

    asset = reader.get_asset(image_keys[0])
    geo = asset.get_metadata().as_dict("Geo")

    # Check GeoModelType
    if "GeoModelType" in geo:
        assert geo["GeoModelType"] in ("Projected", "Geographic"), (
            f"GeoModelType={geo['GeoModelType']!r}, expected Projected or Geographic"
        )

    # Check CRS codes
    for crs_key in ("GeoProjectedCRS", "GeoGeographicCRS"):
        if crs_key in geo:
            assert is_plausible_epsg(geo[crs_key]), (
                f"{crs_key}={geo[crs_key]!r} is not a plausible EPSG code"
            )

    # Check GeoPixelScale
    if "GeoPixelScale" in geo:
        scale = geo["GeoPixelScale"]
        assert isinstance(scale, list) and len(scale) == 3, (
            f"GeoPixelScale={scale!r}, expected 3-element array"
        )
        assert all(isinstance(v, (int, float)) for v in scale), (
            f"GeoPixelScale contains non-numeric values: {scale!r}"
        )

    # Check GeoTiepoints
    if "GeoTiepoints" in geo:
        tiepoints = geo["GeoTiepoints"]
        assert isinstance(tiepoints, list) and len(tiepoints) > 0, (
            "GeoTiepoints should be a non-empty list"
        )
        for tp in tiepoints:
            assert isinstance(tp, list) and len(tp) == 6, (
                f"Tiepoint {tp!r} should be a 6-element array"
            )
            assert all(isinstance(v, (int, float)) for v in tp), (
                f"Tiepoint contains non-numeric values: {tp!r}"
            )


def determine_test_result(
    expected_valid: bool,
    exception_raised: bool,
    actual_exception: Optional[BaseException],
    expected_exception: Optional[str],
    expected_message: Optional[str],
) -> tuple[bool, str]:
    """Determine if a test passed based on expected vs actual outcomes.

    Returns:
        (passed, reason) tuple
    """
    if expected_valid:
        if exception_raised:
            return (False, f"Expected valid file but got exception: {actual_exception}")
        return (True, "File processed successfully as expected")

    # Expected invalid
    if not exception_raised:
        return (False, "Expected failure but file processed successfully")

    if expected_exception:
        # Walk the MRO to accept subclasses (e.g. OSError matches "Exception")
        actual_mro_names = [cls.__name__ for cls in type(actual_exception).__mro__]
        if expected_exception not in actual_mro_names:
            return (False, f"Expected {expected_exception}, got {type(actual_exception).__name__}")

    if expected_message:
        actual_msg = str(actual_exception)
        if expected_message not in actual_msg:
            return (False, f"Expected message containing '{expected_message}', got '{actual_msg}'")

    return (True, "Validation failed as expected")


# =============================================================================
# Parametrized Test
# =============================================================================

# Load manifest entries at module level for parametrization, filtered by
# --include-tags / --exclude-tags CLI options (see conftest.py).
_base_path, _manifest = load_manifest()
_test_cases = [(entry.path, entry) for entry in _manifest.entries]
_test_ids = [
    f"{Path(entry.path).name}-{entry.label}" if entry.label else Path(entry.path).name
    for entry in _manifest.entries
]


@pytest.mark.integration
@pytest.mark.parametrize(
    "path,entry",
    _test_cases
    if _test_cases
    else [
        pytest.param(
            "no_manifest",
            None,
            marks=pytest.mark.skip(reason="No manifest entries found"),
        )
    ],
    ids=_test_ids,
)
def test_integration(path: str, entry: Optional[IntegrationEntry], integration_summary):
    """Run integration test for a single manifest entry.

    For valid entries: opens via IO.open(), runs format-agnostic sanity checks,
    then dispatches format-specific checks based on detect_format().

    For invalid entries: verifies the expected exception is raised with the
    expected type and message.
    """
    if entry is None:
        pytest.skip("No manifest entries found")
        return

    integration_summary["total"] += 1

    # Resolve full file path
    file_path = _base_path / path

    if not file_path.exists():
        integration_summary["skipped"] += 1
        logger.warning("Test file not found, skipping: %s", file_path)
        pytest.skip(f"Test file not found: {path}")
        return

    exception_raised = False
    actual_exception: Optional[BaseException] = None

    try:
        reader = IO.open([str(file_path)], "r")

        # Always run checks — for expected-invalid entries the exception
        # will be caught and matched against expected_exception/expected_message
        run_agnostic_checks(reader, _base_path, entry)

        # Dispatch format-specific checks
        fmt = detect_format(reader)
        if fmt == "nitf":
            run_nitf_checks(reader)
        elif fmt == "tiff":
            run_tiff_checks(reader)

        reader.close()

    except Exception as e:
        exception_raised = True
        actual_exception = e

    # Determine result
    passed, reason = determine_test_result(
        expected_valid=entry.expected_valid,
        exception_raised=exception_raised,
        actual_exception=actual_exception,
        expected_exception=entry.expected_exception,
        expected_message=entry.expected_message,
    )

    if passed:
        integration_summary["passed"] += 1
    else:
        integration_summary["failed"] += 1

    assert passed, reason
