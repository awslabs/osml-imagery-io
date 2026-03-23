"""Property-based tests for PNG pixel roundtrip operations.

This module tests:
- Pixel data roundtrip (lossless) — our writer → our reader
"""

import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, write_and_read_png
from ..strategies import png_writable_image


# =============================================================================
# Pixel roundtrip (lossless)
# =============================================================================


@pytest.mark.property
class TestPngPixelRoundtrip:
    """Pixel roundtrip (lossless).

    # Feature: png-format-support, Property 1: Pixel roundtrip (lossless)

    For all valid image configs (UInt8/UInt16, 1-4 bands), write then read
    produces identical pixels.

    **Validates: Requirements 6.1, 6.2, 5.2, 5.3, 5.4, 5.5, 5.6, 3.4, 3.8, 3.9, 3.10, 3.11, 3.12, 3.13, 3.14, 3.15**
    """

    @given(png_writable_image(min_size=16, max_size=64))
    @pbt_settings
    def test_pixel_roundtrip_lossless(self, image_tuple):
        """PNG pixel data survives a write-read cycle exactly."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        decoded = write_and_read_png(array, pixel_type, num_bands, num_rows, num_cols)
        assert_lossless_match(array, decoded)
