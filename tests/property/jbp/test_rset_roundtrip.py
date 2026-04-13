"""Property-based tests for multi-file NITF R-set round-trip.

Feature: multifile-rset-writing, Property 8: Multi-file NITF R-set round-trip

For any valid image with random pixel type, dimensions, and band count, and
for any number of overview levels (1-3) with proportionally smaller dimensions,
writing via multi-path IO.open() and reading back via multi-path IO.open()
SHALL produce: (a) the same set of asset keys, (b) correct dimensions for each
asset, and (c) pixel-identical data for each level (lossless IC=NC).

**Validates: Requirements 1.1, 1.2, 1.3, 4.1, 7.1, 8.1, 8.2**
"""

import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, write_and_read_rset
from ..strategies import pyramid_image


@pytest.mark.property
class TestRsetRoundtrip:
    """Feature: multifile-rset-writing, Property 8: Multi-file NITF R-set round-trip"""

    @given(pyramid=pyramid_image())
    @pbt_settings
    def test_rset_roundtrip_lossless(self, pyramid):
        """Multi-file NITF R-set round-trip is lossless.

        **Validates: Requirements 1.1, 1.2, 1.3, 4.1, 7.1, 8.1, 8.2**
        """
        base_array, base_config, overviews = pyramid
        result = write_and_read_rset(base_array, base_config, overviews)

        # (a) All expected asset keys present
        assert "image:0" in result, "Base image key 'image:0' missing from result"
        for level, _, _ in overviews:
            key = f"image:0:overview:{level}"
            assert key in result, f"Overview key '{key}' missing from result"

        # Verify no unexpected keys
        expected_keys = {"image:0"} | {
            f"image:0:overview:{level}" for level, _, _ in overviews
        }
        assert set(result.keys()) == expected_keys, (
            f"Key mismatch: expected {expected_keys}, got {set(result.keys())}"
        )

        # (b) Correct dimensions for each asset
        assert result["image:0"].shape == base_array.shape, (
            f"Base image shape mismatch: expected {base_array.shape}, "
            f"got {result['image:0'].shape}"
        )
        for level, ovr_array, _ in overviews:
            key = f"image:0:overview:{level}"
            assert result[key].shape == ovr_array.shape, (
                f"Overview {level} shape mismatch: expected {ovr_array.shape}, "
                f"got {result[key].shape}"
            )

        # (c) Pixel-identical data for each level
        assert_lossless_match(base_array, result["image:0"])
        for level, ovr_array, _ in overviews:
            assert_lossless_match(ovr_array, result[f"image:0:overview:{level}"])
