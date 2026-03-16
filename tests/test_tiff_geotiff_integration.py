"""Integration tests for GeoTIFF metadata parsing with real-world files.

This module reads GeoTIFF files from data/integration/ and verifies that
expected metadata fields (GeoModelType, GeoProjectedCRS or GeoGeographicCRS,
GeoPixelScale, GeoTiepoints) are present with plausible values.

Tests skip gracefully when integration data is not available.

Requirements: 11.1, 11.2, 11.3
"""

import logging
import os
from pathlib import Path

import pytest

from aws.osml.io import IO

logger = logging.getLogger(__name__)


# =============================================================================
# Configuration and Path Resolution
# =============================================================================


def get_integration_data_path() -> Path:
    """Get integration data path from environment or default."""
    env_path = os.environ.get("OSML_IO_INTEGRATION_DATA")
    if env_path:
        return Path(env_path)
    return Path("data/integration")


def discover_geotiff_files(base_path: Path) -> list[Path]:
    """Recursively discover GeoTIFF files in a directory."""
    if not base_path.exists():
        return []

    extensions = {".tif", ".tiff", ".geotiff"}
    files = []
    for path in base_path.rglob("*"):
        if path.is_file() and path.suffix.lower() in extensions:
            files.append(path)
    return sorted(files)


def geotiff_data_available() -> bool:
    """Check if any GeoTIFF files exist in the integration data directory."""
    return len(discover_geotiff_files(get_integration_data_path())) > 0


# Discover files at module load time for parametrization
_base_path = get_integration_data_path()
_geotiff_files = discover_geotiff_files(_base_path)
_test_params = [
    (str(f.relative_to(_base_path)), f) for f in _geotiff_files
]


# =============================================================================
# Validation Helpers
# =============================================================================


VALID_MODEL_TYPES = {"Projected", "Geographic"}
VALID_RASTER_TYPES = {"PixelIsArea", "PixelIsPoint"}


def _is_plausible_epsg(value) -> bool:
    """Check if a value looks like a valid EPSG code."""
    return isinstance(value, (int, float)) and 1 <= int(value) <= 32767


def _is_number_array(value, expected_len=None) -> bool:
    """Check if a value is a list of numbers with optional length check."""
    if not isinstance(value, list):
        return False
    if expected_len is not None and len(value) != expected_len:
        return False
    return all(isinstance(v, (int, float)) for v in value)


def _is_tiepoint_array(value) -> bool:
    """Check if a value is a valid tiepoint array (list of 6-element arrays)."""
    if not isinstance(value, list) or len(value) == 0:
        return False
    return all(
        isinstance(tp, list) and len(tp) == 6 and all(isinstance(v, (int, float)) for v in tp)
        for tp in value
    )


# =============================================================================
# Integration Tests
# =============================================================================


@pytest.mark.integration
class TestGeoTIFFIntegration:
    """Integration tests for GeoTIFF metadata parsing with real-world files."""

    @pytest.fixture(autouse=True)
    def check_data_available(self):
        """Skip all tests if no GeoTIFF integration data is available."""
        if not geotiff_data_available():
            pytest.skip(
                f"No GeoTIFF integration data at {get_integration_data_path()}. "
                "Place .tif/.tiff files in data/integration/ or set "
                "OSML_IO_INTEGRATION_DATA."
            )

    @pytest.mark.parametrize(
        "test_id,file_path",
        _test_params if _test_params else [
            pytest.param("no_files", None, marks=pytest.mark.skip(reason="No GeoTIFF files found"))
        ],
        ids=lambda x: x if isinstance(x, str) else str(x),
    )
    def test_geotiff_has_geo_metadata(self, test_id, file_path):
        """Verify that a GeoTIFF file produces Geo-prefixed metadata fields.

        Each real-world GeoTIFF should have at least GeoModelType and either
        GeoProjectedCRS or GeoGeographicCRS, plus spatial reference tags
        like GeoPixelScale or GeoTiepoints.
        """
        if file_path is None or not file_path.exists():
            pytest.skip(f"File not found: {file_path}")

        reader = IO.open([str(file_path)], "r")
        keys = reader.get_asset_keys()
        assert len(keys) > 0, f"No image segments in {test_id}"

        asset = reader.get_asset(keys[0])
        geo = asset.get_metadata().as_dict("Geo")
        reader.close()

        logger.info(f"{test_id}: Geo keys = {sorted(geo.keys())}")

        # A real GeoTIFF should have at least one Geo-prefixed field
        assert len(geo) > 0, f"No Geo-prefixed metadata in {test_id}"

        # All keys must be Geo-prefixed
        for key in geo:
            assert key.startswith("Geo"), f"Non-Geo key {key!r} in Geo-filtered dict"

    @pytest.mark.parametrize(
        "test_id,file_path",
        _test_params if _test_params else [
            pytest.param("no_files", None, marks=pytest.mark.skip(reason="No GeoTIFF files found"))
        ],
        ids=lambda x: x if isinstance(x, str) else str(x),
    )
    def test_geotiff_model_type_is_valid(self, test_id, file_path):
        """Verify GeoModelType, if present, has a valid value."""
        if file_path is None or not file_path.exists():
            pytest.skip(f"File not found: {file_path}")

        reader = IO.open([str(file_path)], "r")
        asset = reader.get_asset(reader.get_asset_keys()[0])
        geo = asset.get_metadata().as_dict("Geo")
        reader.close()

        if "GeoModelType" in geo:
            assert geo["GeoModelType"] in VALID_MODEL_TYPES, (
                f"{test_id}: GeoModelType={geo['GeoModelType']!r}, "
                f"expected one of {VALID_MODEL_TYPES}"
            )

    @pytest.mark.parametrize(
        "test_id,file_path",
        _test_params if _test_params else [
            pytest.param("no_files", None, marks=pytest.mark.skip(reason="No GeoTIFF files found"))
        ],
        ids=lambda x: x if isinstance(x, str) else str(x),
    )
    def test_geotiff_crs_is_plausible(self, test_id, file_path):
        """Verify GeoProjectedCRS or GeoGeographicCRS has a plausible EPSG code."""
        if file_path is None or not file_path.exists():
            pytest.skip(f"File not found: {file_path}")

        reader = IO.open([str(file_path)], "r")
        asset = reader.get_asset(reader.get_asset_keys()[0])
        geo = asset.get_metadata().as_dict("Geo")
        reader.close()

        has_projected = "GeoProjectedCRS" in geo
        has_geographic = "GeoGeographicCRS" in geo

        if has_projected:
            assert _is_plausible_epsg(geo["GeoProjectedCRS"]), (
                f"{test_id}: GeoProjectedCRS={geo['GeoProjectedCRS']!r} is not a plausible EPSG code"
            )
        if has_geographic:
            assert _is_plausible_epsg(geo["GeoGeographicCRS"]), (
                f"{test_id}: GeoGeographicCRS={geo['GeoGeographicCRS']!r} is not a plausible EPSG code"
            )

    @pytest.mark.parametrize(
        "test_id,file_path",
        _test_params if _test_params else [
            pytest.param("no_files", None, marks=pytest.mark.skip(reason="No GeoTIFF files found"))
        ],
        ids=lambda x: x if isinstance(x, str) else str(x),
    )
    def test_geotiff_pixel_scale_is_valid(self, test_id, file_path):
        """Verify GeoPixelScale, if present, is a 3-element number array."""
        if file_path is None or not file_path.exists():
            pytest.skip(f"File not found: {file_path}")

        reader = IO.open([str(file_path)], "r")
        asset = reader.get_asset(reader.get_asset_keys()[0])
        geo = asset.get_metadata().as_dict("Geo")
        reader.close()

        if "GeoPixelScale" in geo:
            scale = geo["GeoPixelScale"]
            assert _is_number_array(scale, expected_len=3), (
                f"{test_id}: GeoPixelScale={scale!r}, expected [scale_x, scale_y, scale_z]"
            )

    @pytest.mark.parametrize(
        "test_id,file_path",
        _test_params if _test_params else [
            pytest.param("no_files", None, marks=pytest.mark.skip(reason="No GeoTIFF files found"))
        ],
        ids=lambda x: x if isinstance(x, str) else str(x),
    )
    def test_geotiff_tiepoints_are_valid(self, test_id, file_path):
        """Verify GeoTiepoints, if present, is an array of 6-element tuples."""
        if file_path is None or not file_path.exists():
            pytest.skip(f"File not found: {file_path}")

        reader = IO.open([str(file_path)], "r")
        asset = reader.get_asset(reader.get_asset_keys()[0])
        geo = asset.get_metadata().as_dict("Geo")
        reader.close()

        if "GeoTiepoints" in geo:
            tiepoints = geo["GeoTiepoints"]
            assert _is_tiepoint_array(tiepoints), (
                f"{test_id}: GeoTiepoints has invalid structure, "
                f"expected list of [px, py, pz, gx, gy, gz] arrays"
            )

    def test_summary(self):
        """Log a summary of all GeoTIFF integration files found."""
        base_path = get_integration_data_path()
        files = discover_geotiff_files(base_path)

        if not files:
            pytest.skip("No GeoTIFF files found")

        success = 0
        failed = 0

        for file_path in files:
            rel = file_path.relative_to(base_path)
            try:
                reader = IO.open([str(file_path)], "r")
                asset = reader.get_asset(reader.get_asset_keys()[0])
                geo = asset.get_metadata().as_dict("Geo")
                reader.close()
                logger.info(f"  {rel}: {len(geo)} Geo fields — {sorted(geo.keys())}")
                success += 1
            except Exception as e:
                logger.warning(f"  {rel}: FAILED — {e}")
                failed += 1

        logger.info(f"GeoTIFF integration summary: {success} ok, {failed} failed out of {len(files)} files")
