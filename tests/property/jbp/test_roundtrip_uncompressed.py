"""Property-based tests for uncompressed (IC=NC) roundtrip operations.

This module tests lossless roundtrip preservation for uncompressed NITF images.
"""

import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..strategies import random_image
from ..helpers import write_and_read_jbp, assert_lossless_match


@pytest.mark.property
class TestLosslessRoundtrip:
    """Property tests for lossless encode/decode roundtrips.

    For any valid image with lossless compression settings (IC=NC or COMRAT=N001.0),
    encoding then decoding SHALL produce an image that is exactly equal to the
    original (same shape, same dtype, same pixel values).
    """

    @given(random_image(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_uncompressed_roundtrip(self, image_tuple):
        """Lossless Roundtrip Preservation (IC=NC uncompressed)

        For any valid image with IC=NC (uncompressed), encoding then decoding
        SHALL produce an image that is exactly equal to the original.
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "NC"},
        )
        assert_lossless_match(array, decoded)
