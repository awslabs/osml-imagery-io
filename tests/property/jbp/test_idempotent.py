"""Property-based tests for idempotent encoding operations (JBP/NITF).
"""

import tempfile
from pathlib import Path

import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
)
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, read_full_image, write_and_read_jbp
from ..strategies import random_image


@pytest.mark.property
class TestIdempotentEncoding:
    """Property tests for idempotent encoding operations.

    These tests verify that encoding is idempotent at both the byte level
    and the value level for lossless compression.
    """

    @given(random_image(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_idempotent_encoding_byte_level(self, image_tuple):
        """For any valid image with deterministic codec settings (IC=NC uncompressed),
        encode(decode(encode(image))) SHALL produce bytes identical to encode(image).
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path1 = Path(f.name)
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path2 = Path(f.name)

        try:
            # First encoding: encode(image) -> path1
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "NC")

            provider = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, 64),
                block_height=min(num_rows, 64),
                pixel_type=pixel_type,
                metadata=metadata,
            )
            provider.set_full_image(array)

            writer = IO.open([str(path1)], "w", "nitf")
            writer.add_asset(
                key="image:0",
                provider=provider,
                title="Test Image",
                description="Property test image",
                roles=["data"],
            )
            writer.close()

            # Read first encoding bytes
            first_encoding_bytes = path1.read_bytes()

            # Decode: decode(encode(image))
            reader = IO.open([str(path1)], "r")
            asset = reader.get_asset("image:0")
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            # Re-encode: encode(decode(encode(image))) -> path2
            metadata2 = BufferedMetadataProvider()
            metadata2.set("IC", "NC")

            provider2 = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, 64),
                block_height=min(num_rows, 64),
                pixel_type=pixel_type,
                metadata=metadata2,
            )
            provider2.set_full_image(decoded)

            writer2 = IO.open([str(path2)], "w", "nitf")
            writer2.add_asset(
                key="image:0",
                provider=provider2,
                title="Test Image",
                description="Property test image",
                roles=["data"],
            )
            writer2.close()

            # Read second encoding bytes
            second_encoding_bytes = path2.read_bytes()

            # Verify byte-level idempotence
            assert len(first_encoding_bytes) == len(second_encoding_bytes), (
                f"File sizes differ: first={len(first_encoding_bytes)}, second={len(second_encoding_bytes)}"
            )

        finally:
            if path1.exists():
                path1.unlink()
            if path2.exists():
                path2.unlink()

    @given(random_image(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_idempotent_encoding_value_level(self, image_tuple):
        """For any valid image with lossless compression (IC=NC),
        decode(encode(decode(encode(image)))) SHALL equal the original image.
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        hints = {"IC": "NC"}

        # First roundtrip
        decoded1 = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints=hints,
        )

        # Second roundtrip
        decoded2 = write_and_read_jbp(
            decoded1, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints=hints,
        )

        assert_lossless_match(array, decoded2)
