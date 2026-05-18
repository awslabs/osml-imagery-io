"""Property-based tests for uncompressed (IC=NC) roundtrip operations.

This module tests lossless roundtrip preservation for uncompressed NITF and
NSIF images, including sub-byte (NBPP < 8) bit-packed imagery.
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, read_full_image, write_and_read_jbp
from ..strategies import bit_packed_image, random_image, sub_byte_image


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

    @given(random_image(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_uncompressed_roundtrip_nsif(self, image_tuple):
        """Lossless Roundtrip Preservation (IC=NC uncompressed, NSIF 1.0)

        For any valid image written as NSIF 1.0 with IC=NC (uncompressed),
        encoding then decoding SHALL produce an image that is exactly equal
        to the original.
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "NC"},
            format="nsif",
        )
        assert_lossless_match(array, decoded)

    @given(sub_byte_image(min_size=8, max_size=64, min_bands=1, max_bands=4))
    @pbt_settings
    def test_sub_byte_roundtrip(self, image_tuple):
        """Lossless Roundtrip Preservation (IC=NC, NBPP < 8)

        For any valid sub-byte image (1, 2, or 4 bits per pixel) with IC=NC,
        encoding then decoding SHALL produce pixel values that are exactly
        equal to the original.
        """
        from aws.osml.io import (
            IO,
            BufferedImageAssetProvider,
            BufferedMetadataProvider,
            PixelType,
        )

        array, nbpp, num_bands, num_rows, num_cols, block_size = image_tuple

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            metadata = BufferedMetadataProvider()
            metadata["IC"] = "NC"

            provider = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, block_size),
                block_height=min(num_rows, block_size),
                pixel_type=PixelType.UInt8,
                num_bits_per_pixel=nbpp,
                metadata=metadata,
            )
            provider.set_full_image(array)

            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image:0",
                provider=provider,
                title="Sub-byte Test",
                description="Property test sub-byte image",
                roles=["data"],
            )
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            assert asset.num_bits_per_pixel == nbpp
            assert asset.actual_bits_per_pixel == nbpp

            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            np.testing.assert_array_equal(
                decoded, array,
                err_msg=f"Sub-byte roundtrip failed for {nbpp}-bpp, "
                        f"{num_bands} bands, {num_rows}x{num_cols}",
            )
        finally:
            if path.exists():
                path.unlink()

    @given(bit_packed_image(min_size=8, max_size=64, min_bands=1, max_bands=4))
    @pbt_settings
    def test_bit_packed_roundtrip(self, image_tuple):
        """Lossless Roundtrip Preservation (IC=NC, non-byte-aligned NBPP)

        For any valid bit-packed image (1, 2, 4, or 12 bits per pixel) with
        IC=NC, encoding then decoding SHALL produce pixel values that are
        exactly equal to the original.
        """
        from aws.osml.io import (
            IO,
            BufferedImageAssetProvider,
            BufferedMetadataProvider,
            PixelType,
        )

        array, nbpp, num_bands, num_rows, num_cols, block_size = image_tuple

        pixel_type = PixelType.UInt8 if nbpp <= 8 else PixelType.UInt16

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            metadata = BufferedMetadataProvider()
            metadata["IC"] = "NC"

            provider = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, block_size),
                block_height=min(num_rows, block_size),
                pixel_type=pixel_type,
                num_bits_per_pixel=nbpp,
                metadata=metadata,
            )
            provider.set_full_image(array)

            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image:0",
                provider=provider,
                title="Bit-packed Test",
                description="Property test bit-packed image",
                roles=["data"],
            )
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            assert asset.num_bits_per_pixel == nbpp
            assert asset.actual_bits_per_pixel == nbpp

            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            np.testing.assert_array_equal(
                decoded, array,
                err_msg=f"Bit-packed roundtrip failed for {nbpp}-bpp, "
                        f"{num_bands} bands, {num_rows}x{num_cols}",
            )
        finally:
            if path.exists():
                path.unlink()
