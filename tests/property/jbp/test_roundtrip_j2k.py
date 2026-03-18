"""Property-based tests for JPEG 2000 (IC=C8) lossy roundtrip operations.

This module tests lossy roundtrip quality bounds for J2K compression in JBP.
"""

import numpy as np
import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..strategies import realistic_image_for_compression
from ..helpers import write_and_read_jbp, assert_lossy_quality


@pytest.mark.property
class TestLossyRoundtrip:
    """Property tests for lossy encode/decode roundtrips with quality bounds.

    For any valid image with lossy compression settings, encoding then decoding
    SHALL produce an image with PSNR >= 30 dB and SSIM >= 0.95, with preserved
    shape and pixel type.
    """

    @given(realistic_image_for_compression(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_lossy_j2k_roundtrip_quality(self, image_tuple):
        """For any valid image with lossy JPEG 2000 compression, encoding then
        decoding SHALL produce an image with PSNR >= 30 dB and SSIM >= 0.95,
        with preserved shape and pixel type.
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        # Calculate appropriate decomposition levels for image size
        min_dim = min(num_rows, num_cols)
        max_decomp_levels = max(1, int(np.floor(np.log2(min_dim))) - 1)
        decomp_levels = min(5, max_decomp_levels)

        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={
                "IC": "C8",
                "COMRAT": "02.0",
                "J2K_DECOMPOSITION_LEVELS": str(decomp_levels),
            },
        )
        assert_lossy_quality(array, decoded)
