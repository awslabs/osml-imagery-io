"""Property-based tests for TIFF idempotent encoding.

This module tests idempotent encoding (write → read → write → read).
"""

import numpy as np
import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import write_and_read_tiff
from ..strategies import tiff_writable_image


@pytest.mark.property
class TestTiffIdempotentEncoding:
    """TIFF idempotent encoding.

    For any valid image and encoding configuration, write → read → write → read
    yields the same pixel values as write → read.
    """

    @given(tiff_writable_image(min_size=16, max_size=48, min_bands=1, max_bands=3))
    @pbt_settings
    def test_idempotent_encoding(self, image_tuple):
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple

        decoded1 = write_and_read_tiff(array, pixel_type, num_bands, num_rows, num_cols, hints)
        decoded2 = write_and_read_tiff(decoded1, pixel_type, num_bands, num_rows, num_cols, hints)

        np.testing.assert_array_equal(decoded2, decoded1)
