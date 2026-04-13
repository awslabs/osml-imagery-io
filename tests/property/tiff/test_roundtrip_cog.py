"""Property-based tests for COG overview round-trip.

Feature: multifile-rset-writing, Property 9: COG overview round-trip

For any valid image with random pixel type, dimensions, and band count, and
for any number of overview levels (1-3) with proportionally smaller dimensions,
writing the full-resolution image and overview images to a single TIFF file
via TIFFDatasetWriter and reading them back SHALL produce: (a) the full-res
IFD with role "data", (b) overview IFDs with role "overview" in decreasing
dimension order, (c) correct dimensions for each asset, and (d) pixel-identical
data for each level (lossless compression).

**Validates: Requirements 5.1, 5.2, 5.3, 5.4, 8.1, 8.2**
"""

import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, write_and_read_cog
from ..strategies import pyramid_image


@pytest.mark.property
class TestCogRoundtrip:
    """Feature: multifile-rset-writing, Property 9: COG overview round-trip"""

    @given(pyramid=pyramid_image())
    @pbt_settings
    def test_cog_roundtrip_lossless(self, pyramid):
        """COG overview round-trip is lossless with correct IFD ordering.

        **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 8.1, 8.2**
        """
        base_array, base_config, overviews = pyramid
        result = write_and_read_cog(base_array, base_config, overviews)

        # (a) Full-res IFD has role "data"
        assert "image:0" in result, "Base image key 'image:0' missing from result"
        decoded_base, base_roles = result["image:0"]
        assert "data" in base_roles, (
            f"Full-res IFD should have role 'data', got roles: {base_roles}"
        )

        # (b) Overview IFDs have role "overview" in decreasing dimension order
        ovr_keys = []
        for level, _, _ in overviews:
            key = f"image:0:overview:{level}"
            assert key in result, f"Overview key '{key}' missing from result"
            decoded_ovr, ovr_roles = result[key]
            assert "overview" in ovr_roles, (
                f"Overview {level} should have role 'overview', got roles: {ovr_roles}"
            )
            ovr_keys.append(key)

        # Verify overview dimensions are in decreasing order when iterated
        # by the order they appear in the result keys
        if len(ovr_keys) > 1:
            ovr_areas = []
            for key in ovr_keys:
                decoded_ovr, _ = result[key]
                area = decoded_ovr.shape[1] * decoded_ovr.shape[2]
                ovr_areas.append(area)
            for i in range(len(ovr_areas) - 1):
                assert ovr_areas[i] >= ovr_areas[i + 1], (
                    f"Overview IFDs not in decreasing dimension order: "
                    f"areas = {ovr_areas}"
                )

        # (c) Correct dimensions for each asset
        assert decoded_base.shape == base_array.shape, (
            f"Base image shape mismatch: expected {base_array.shape}, "
            f"got {decoded_base.shape}"
        )
        for level, ovr_array, _ in overviews:
            key = f"image:0:overview:{level}"
            decoded_ovr, _ = result[key]
            assert decoded_ovr.shape == ovr_array.shape, (
                f"Overview {level} shape mismatch: expected {ovr_array.shape}, "
                f"got {decoded_ovr.shape}"
            )

        # (d) Pixel-identical data for each level
        assert_lossless_match(base_array, decoded_base)
        for level, ovr_array, _ in overviews:
            decoded_ovr, _ = result[f"image:0:overview:{level}"]
            assert_lossless_match(ovr_array, decoded_ovr)
