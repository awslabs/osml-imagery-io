"""Manifest-driven integration tests for all imagery formats.

Consolidates NITF, GeoTIFF, DTED, JPEG 2000, JPEG, and PNG tests into a
single parametrized framework. Each manifest entry is opened via IO.open(),
validated with format-agnostic metadata checks, then dispatched to
format-specific checks based on detected format.

Pixel reads (get_block) are skipped for entries tagged "slow" unless those
tags are explicitly included via --include-tags.
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

_EXTENSION_FORMAT_MAP: dict[str, str] = {
    ".ntf": "nitf",
    ".nitf": "nitf",
    ".nsf": "nitf",
    ".nsif": "nitf",
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


def detect_format(reader, file_path: Path) -> str:
    """Detect format from dataset metadata, falling back to file extension.

    Returns one of: 'nitf', 'tiff', 'dted', 'j2k', 'jpeg', 'png', 'unknown'.
    """
    meta = reader.metadata.entries()

    # NITF/NSIF: FHDR key in dataset metadata
    fhdr = meta.get("FHDR", "")
    if isinstance(fhdr, str) and fhdr.startswith(("NITF", "NSIF")):
        return "nitf"

    # TIFF: numeric tag keys or ImageWidth
    if "256" in meta or "ImageWidth" in meta:
        return "tiff"

    # DTED: UHL sentinel or recognized fields
    if "UHL" in meta or "OriginLongitude" in meta:
        return "dted"

    # Fall back to file extension
    ext = file_path.suffix.lower()
    return _EXTENSION_FORMAT_MAP.get(ext, "unknown")


# =============================================================================
# Validation Helpers
# =============================================================================


def is_plausible_epsg(value) -> bool:
    """Return True for integers 1-32767 inclusive."""
    return isinstance(value, int) and 1 <= value <= 32767


def run_agnostic_checks(reader, entry: IntegrationEntry, *, skip_assets: bool = False) -> None:
    """Format-agnostic metadata checks on a valid dataset.

    Verifies that the dataset can be opened, that all segment keys are
    enumerable, and that has_asset returns True for each key.  When
    *skip_assets* is False (default), also constructs each asset provider
    and validates image metadata (dimensions, bands, block grid).

    Set *skip_assets=True* for entries known to have pathologically slow
    asset construction (e.g. interleaved-overflow blocking).
    """
    all_keys = reader.get_asset_keys()
    assert isinstance(all_keys, list), f"get_asset_keys() did not return a list for {entry.path}"

    for key in all_keys:
        assert reader.has_asset(key), f"has_asset('{key}') returned False for {entry.path}"

    file_meta = reader.metadata.entries()
    assert len(file_meta) > 0, f"Empty dataset metadata for {entry.path}"

    if skip_assets:
        return

    for key in all_keys:
        asset = reader.get_asset(key)
        assert asset is not None, f"get_asset('{key}') returned None for {entry.path}"

    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    for key in image_keys:
        _check_image_metadata(reader.get_asset(key), entry)


def _check_image_metadata(asset, entry: IntegrationEntry) -> None:
    """Validate image asset metadata without reading pixels."""
    key = asset.key

    meta_dict = asset.metadata.entries()
    assert len(meta_dict) > 0, f"Empty metadata for image asset '{key}' in {entry.path}"

    assert asset.num_rows > 0, f"num_rows not positive for '{key}' in {entry.path}"
    assert asset.num_columns > 0, f"num_columns not positive for '{key}' in {entry.path}"
    assert asset.num_bands >= 1, f"num_bands < 1 for '{key}' in {entry.path}"

    grid_rows, grid_cols = asset.block_grid_size
    assert grid_rows > 0 and grid_cols > 0, (
        f"Invalid block_grid_size for '{key}' in {entry.path}"
    )


def run_pixel_checks(reader, entry: IntegrationEntry) -> None:
    """Read first present block from each image segment and validate array shape."""
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    for key in image_keys:
        asset = reader.get_asset(key)
        grid_rows, grid_cols = asset.block_grid_size

        # Find the first block with actual image data (not masked fill)
        block = None
        for row in range(grid_rows):
            for col in range(grid_cols):
                if asset.has_block(row, col, 0):
                    block = asset.get_block(row, col, 0)
                    break
            if block is not None:
                break

        if block is None:
            # All blocks are masked — skip pixel validation for this asset
            continue

        assert isinstance(block, np.ndarray), (
            f"get_block did not return ndarray for '{key}' in {entry.path}"
        )
        assert len(block.shape) == 3, f"Block shape not 3D for '{key}' in {entry.path}"
        assert block.shape[0] == asset.num_bands, (
            f"Block first dim {block.shape[0]} != num_bands {asset.num_bands} "
            f"for '{key}' in {entry.path}"
        )


# =============================================================================
# Format-Specific Checks
# =============================================================================


def run_nitf_checks(reader) -> None:
    """NITF-specific checks: magic bytes and blocking parameters."""
    raw_bytes = reader.metadata.raw.read()
    assert raw_bytes[:4] in (b"NITF", b"NSIF"), (
        f"NITF magic bytes expected, got {raw_bytes[:4]!r}"
    )

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
    geo = asset.metadata.entries("Geo")

    if "GeoModelType" in geo:
        assert geo["GeoModelType"] in ("Projected", "Geographic"), (
            f"GeoModelType={geo['GeoModelType']!r}, expected Projected or Geographic"
        )

    for crs_key in ("GeoProjectedCRS", "GeoGeographicCRS"):
        if crs_key in geo:
            assert is_plausible_epsg(geo[crs_key]), (
                f"{crs_key}={geo[crs_key]!r} is not a plausible EPSG code"
            )

    if "GeoPixelScale" in geo:
        scale = geo["GeoPixelScale"]
        assert isinstance(scale, list) and len(scale) == 3, (
            f"GeoPixelScale={scale!r}, expected 3-element array"
        )
        assert all(isinstance(v, (int, float)) for v in scale), (
            f"GeoPixelScale contains non-numeric values: {scale!r}"
        )

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


def run_dted_checks(reader) -> None:
    """DTED-specific checks: elevation data type and grid dimensions."""
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    assert len(image_keys) == 1, (
        f"DTED should have exactly 1 image segment, got {len(image_keys)}"
    )

    asset = reader.get_asset(image_keys[0])

    assert asset.num_bands == 1, f"DTED should be single-band, got {asset.num_bands}"
    assert asset.num_rows > 0, "DTED num_rows must be positive"
    assert asset.num_columns > 0, "DTED num_columns must be positive"


def run_j2k_checks(reader) -> None:
    """JPEG 2000 checks: single image segment, valid dimensions."""
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    assert len(image_keys) >= 1, "JPEG 2000 should have at least 1 image segment"

    asset = reader.get_asset(image_keys[0])
    assert asset.num_rows > 0, "J2K num_rows must be positive"
    assert asset.num_columns > 0, "J2K num_columns must be positive"
    assert asset.num_bands >= 1, f"J2K num_bands must be >= 1, got {asset.num_bands}"


def run_jpeg_checks(reader) -> None:
    """JPEG checks: single image, 1 or 3 bands, 8-bit."""
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    assert len(image_keys) == 1, f"JPEG should have 1 image segment, got {len(image_keys)}"

    asset = reader.get_asset(image_keys[0])
    assert asset.num_bands in (1, 3), f"JPEG bands should be 1 or 3, got {asset.num_bands}"


def run_png_checks(reader) -> None:
    """PNG checks: single image, 1-4 bands."""
    image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
    assert len(image_keys) == 1, f"PNG should have 1 image segment, got {len(image_keys)}"

    asset = reader.get_asset(image_keys[0])
    assert asset.num_bands in (1, 2, 3, 4), (
        f"PNG bands should be 1-4, got {asset.num_bands}"
    )


_FORMAT_CHECKS = {
    "nitf": run_nitf_checks,
    "tiff": run_tiff_checks,
    "dted": run_dted_checks,
    "j2k": run_j2k_checks,
    "jpeg": run_jpeg_checks,
    "png": run_png_checks,
}


# =============================================================================
# Result Determination
# =============================================================================


def determine_test_result(
    expected_valid: bool,
    exception_raised: bool,
    actual_exception: Optional[BaseException],
    expected_exception: Optional[str],
    expected_message: Optional[str],
) -> tuple[bool, str]:
    """Determine if a test passed based on expected vs actual outcomes."""
    if expected_valid:
        if exception_raised:
            return (False, f"Expected valid file but got exception: {actual_exception}")
        return (True, "File processed successfully as expected")

    if not exception_raised:
        return (False, "Expected failure but file processed successfully")

    if expected_exception:
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
def test_integration(
    path: str,
    entry: Optional[IntegrationEntry],
    integration_summary,
    request,
):
    """Run integration test for a single manifest entry.

    For valid entries: opens via IO.open(), runs format-agnostic metadata
    checks, dispatches format-specific checks, and optionally reads pixels.
    Pixel reads are skipped for entries tagged "slow" unless --include-tags
    explicitly includes "slow".

    For invalid entries: verifies the expected exception is raised with the
    expected type and message.
    """
    if entry is None:
        pytest.skip("No manifest entries found")
        return

    integration_summary["total"] += 1

    file_path = _base_path / path

    if not file_path.exists():
        integration_summary["skipped"] += 1
        logger.warning("Test file not found, skipping: %s", file_path)
        pytest.skip(f"Test file not found: {path}")
        return

    # Determine whether pixel reads should be performed for this entry.
    # Skip pixel reads for slow entries unless slow was explicitly included.
    include_raw = request.config.getoption("--include-tags", default="")
    include_tags = {t.strip() for t in include_raw.split(",") if t.strip()}
    skip_pixels = "slow" in entry.tags and "slow" not in include_tags

    exception_raised = False
    actual_exception: Optional[BaseException] = None

    try:
        reader = IO.open([str(file_path)], "r")

        run_agnostic_checks(reader, entry, skip_assets=skip_pixels)

        if not skip_pixels:
            fmt = detect_format(reader, file_path)
            check_fn = _FORMAT_CHECKS.get(fmt)
            if check_fn:
                check_fn(reader)

            run_pixel_checks(reader, entry)

        reader.close()

    except Exception as e:
        exception_raised = True
        actual_exception = e

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
