"""Property-based tests for PyCallbackImageAssetProvider duck-typing support.

This module tests correctness properties of the callback adapter that wraps
Python-defined duck-typed objects as ImageAssetProvider trait objects for use
with DatasetWriter.add_asset().

Properties tested:
- Property 1: Write-Read Roundtrip Preservation
- Property 6: Python Exception Propagation
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, BufferedMetadataProvider, PixelType
from hypothesis import given
from hypothesis import strategies as st

from .conftest import pbt_settings
from .helpers import assert_lossless_match, read_full_image
from .strategies import (
    band_counts,
    block_sizes,
    image_arrays,
    image_dimensions,
    pixel_types,
)

# ---------------------------------------------------------------------------
# Duck-typed provider for property tests
# ---------------------------------------------------------------------------


class DuckTypedProvider:
    """A plain Python class implementing the ImageAssetProvider interface.

    Adapted for property tests — accepts all image configuration parameters
    and returns the provided data from get_block() with proper block slicing.
    Does NOT have a has_block method (to test default behavior).
    """

    def __init__(
        self,
        *,
        num_rows,
        num_cols,
        num_bands,
        pixel_type,
        block_width,
        block_height,
        data,
    ):
        self.key = "image:0"
        self.title = "Test Image"
        self.description = "Property test duck-typed provider"
        self.num_rows = num_rows
        self.num_columns = num_cols
        self.num_bands = num_bands
        self.num_bits_per_pixel = np.dtype(pixel_type.to_numpy_dtype()).itemsize * 8
        self.actual_bits_per_pixel = self.num_bits_per_pixel
        self.pixel_value_type = pixel_type
        self.num_pixels_per_block_horizontal = block_width
        self.num_pixels_per_block_vertical = block_height
        self.num_resolution_levels = 1
        self.pad_pixel_value = 0.0
        self._data = data

    def get_block(self, block_row, block_col, resolution_level, bands=None):
        bh = self.num_pixels_per_block_vertical
        bw = self.num_pixels_per_block_horizontal
        r0 = block_row * bh
        c0 = block_col * bw
        r1 = min(r0 + bh, self.num_rows)
        c1 = min(c0 + bw, self.num_columns)
        return self._data[:, r0:r1, c0:c1].copy()


# ---------------------------------------------------------------------------
# Write-and-read helper for callback provider
# ---------------------------------------------------------------------------


def write_and_read_callback(array, pixel_type, num_bands, num_rows, num_cols, block_width, block_height):
    """Write a NITF using a duck-typed callback provider and read back."""
    with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
        path = Path(f.name)
    try:
        metadata = BufferedMetadataProvider()
        metadata["IC"] = "NC"

        provider = DuckTypedProvider(
            num_rows=num_rows,
            num_cols=num_cols,
            num_bands=num_bands,
            pixel_type=pixel_type,
            block_width=block_width,
            block_height=block_height,
            data=array,
        )

        writer = IO.open([str(path)], "w", "nitf")
        writer.metadata = metadata
        writer.add_asset("image:0", provider, "Test", "Property test", ["data"])
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image:0")
        decoded = read_full_image(asset, num_bands, num_rows, num_cols)
        reader.close()
        return decoded
    finally:
        if path.exists():
            path.unlink()


# ---------------------------------------------------------------------------
# Property 1: Write-Read Roundtrip Preservation
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestWriteReadRoundtripPreservation:
    """Property 1: Write-Read Roundtrip Preservation.

    For any valid image configuration (pixel type from SUPPORTED_PIXEL_TYPES,
    dimensions, band count, block size) and a duck-typed Python provider
    returning valid block data, write to a lossless NITF file via DatasetWriter
    and re-read — pixel data must be identical to what get_block() returned.

    **Validates: Requirements 1.1, 1.2, 1.4, 1.5, 4.1, 4.2, 7.1, 7.3**
    """

    @given(
        pixel_type=pixel_types(),
        dims=image_dimensions(min_size=16, max_size=64),
        num_bands=band_counts(min_bands=1, max_bands=3),
        blk=block_sizes(),
        data=st.data(),
    )
    @pbt_settings
    def test_callback_roundtrip_preserves_pixels(self, pixel_type, dims, num_bands, blk, data):
        """Callback provider write-read roundtrip preserves pixel data exactly.

        **Validates: Requirements 1.1, 1.2, 1.4, 1.5, 4.1, 4.2, 7.1, 7.3**
        """
        num_rows, num_cols = dims
        block_height, block_width = blk

        # Clamp block sizes to image dimensions
        block_height = min(block_height, num_rows)
        block_width = min(block_width, num_cols)

        # Generate image array for this configuration
        array = data.draw(image_arrays(pixel_type, num_bands, num_rows, num_cols))

        decoded = write_and_read_callback(
            array, pixel_type, num_bands, num_rows, num_cols,
            block_width, block_height,
        )
        assert_lossless_match(array, decoded)


# ---------------------------------------------------------------------------
# Property 6: Python Exception Propagation
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestPythonExceptionPropagation:
    """Property 6: Python Exception Propagation.

    For any Python exception message string, if the provider's get_block()
    raises an exception with that message, the write should fail with a
    RuntimeError containing that message.

    **Validates: Requirements 5.1, 5.2**
    """

    @given(
        message=st.text(
            min_size=1,
            max_size=100,
            alphabet=st.characters(whitelist_categories=("L", "N", "P", "Z")),
        ),
    )
    @pbt_settings
    def test_exception_message_propagated(self, message):
        """Python exception messages propagate through the callback adapter.

        **Validates: Requirements 5.1, 5.2**
        """

        class FailingProvider(DuckTypedProvider):
            def __init__(self, msg):
                super().__init__(
                    num_rows=16,
                    num_cols=16,
                    num_bands=1,
                    pixel_type=PixelType.UInt8,
                    block_width=16,
                    block_height=16,
                    data=np.zeros((1, 16, 16), dtype=np.uint8),
                )
                self._error_message = msg

            def get_block(self, block_row, block_col, resolution_level, bands=None):
                raise ValueError(self._error_message)

        provider = FailingProvider(message)

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)
        try:
            metadata = BufferedMetadataProvider()
            metadata["IC"] = "NC"

            writer = IO.open([str(path)], "w", "nitf")
            writer.metadata = metadata
            writer.add_asset("image:0", provider, "Test", "Property test", ["data"])

            with pytest.raises(RuntimeError) as exc_info:
                writer.close()

            # Verify the original message is contained in the RuntimeError
            assert message in str(exc_info.value), (
                f"Expected message '{message}' not found in error: {exc_info.value}"
            )
        finally:
            if path.exists():
                path.unlink()
