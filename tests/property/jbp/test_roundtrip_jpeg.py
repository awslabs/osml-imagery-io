"""Property-based tests for JPEG DCT compression roundtrip operations.

This module tests the correctness properties for JPEG DCT compression,
including lossy roundtrip quality and downsampled JPEG (I1) roundtrip.
"""

import pytest
from hypothesis import assume, given

from ..conftest import pbt_settings
from ..helpers import (
    assert_lossy_quality,
    write_and_read_jbp,
)
from ..strategies import (
    jpeg_i1_image,
    jpeg_image_for_compression,
)


@pytest.mark.property
class TestJpegLossyRoundtrip:
    """Property tests for JPEG DCT lossy roundtrip quality.

    For any valid image with supported pixel type (UInt8 8-bit) and band
    configuration (mono, RGB, YCbCr, multiband), encoding with IC=C3 then
    decoding SHALL produce an image with:
    - PSNR >= 30 dB
    - SSIM >= 0.95
    - Identical shape (bands, rows, cols)
    - Identical pixel type (dtype)
    """

    @given(jpeg_image_for_compression(min_size=32, max_size=64, min_bands=1, max_bands=1))
    @pbt_settings
    def test_jpeg_8bit_mono_roundtrip(self, image_tuple):
        """JPEG DCT Lossy Roundtrip - 8-bit Mono."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "C3", "COMRAT": "75.0"},
        )
        assert_lossy_quality(array, decoded)

    @given(jpeg_image_for_compression(min_size=32, max_size=64, min_bands=3, max_bands=3))
    @pbt_settings
    def test_jpeg_8bit_rgb_roundtrip(self, image_tuple):
        """JPEG DCT Lossy Roundtrip - 8-bit RGB."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "C3", "COMRAT": "75.0", "IMODE": "P"},
        )
        assert_lossy_quality(array, decoded)

    @given(jpeg_image_for_compression(min_size=32, max_size=64, min_bands=2, max_bands=4))
    @pbt_settings
    def test_jpeg_8bit_multiband_roundtrip(self, image_tuple):
        """JPEG DCT Lossy Roundtrip - 8-bit Multiband."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        # Skip 3-band images (tested separately as RGB)
        assume(num_bands != 3)

        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "C3", "COMRAT": "75.0", "IMODE": "B"},
        )
        assert_lossy_quality(array, decoded)


@pytest.mark.property
class TestDownsampledJpegRoundtrip:
    """Property tests for downsampled JPEG (IC=I1) roundtrip operations.

    For any valid image with dimensions ≤2048×2048, encoding with IC=I1
    then decoding SHALL produce an image with acceptable quality
    (PSNR >= 30 dB, SSIM >= 0.95) and preserved dimensions.
    """

    @given(jpeg_i1_image(min_size=32, max_size=256))
    @pbt_settings
    def test_i1_jpeg_roundtrip(self, image_tuple):
        """Downsampled JPEG (I1) Roundtrip."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "I1", "COMRAT": "75.0"},
            block_width=num_cols,   # I1 is single block
            block_height=num_rows,
        )
        assert_lossy_quality(array, decoded)

    @given(jpeg_i1_image(min_size=32, max_size=128))
    @pbt_settings
    def test_i1_jpeg_mono_roundtrip(self, image_tuple):
        """Downsampled JPEG (I1) Roundtrip - Monochrome."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        assume(num_bands == 1)

        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "I1", "COMRAT": "75.0"},
            block_width=num_cols,
            block_height=num_rows,
        )
        assert_lossy_quality(array, decoded)

    @given(jpeg_i1_image(min_size=32, max_size=128))
    @pbt_settings
    def test_i1_jpeg_rgb_roundtrip(self, image_tuple):
        """Downsampled JPEG (I1) Roundtrip - RGB."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        assume(num_bands == 3)

        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "I1", "COMRAT": "75.0", "IMODE": "P"},
            block_width=num_cols,
            block_height=num_rows,
        )
        assert_lossy_quality(array, decoded)
