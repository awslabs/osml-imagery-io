"""Property-based tests for stream I/O support.

This module tests universal correctness properties of the stream I/O path
through ``IO.open()`` — verifying that reading from a stream produces results
identical to reading from a file path.

Feature: stream-io-support
"""

import io
import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, AssetType, imsave
from hypothesis import given

from .conftest import pbt_settings
from .helpers import read_full_image
from .strategies import convenience_image

# Mapping from convenience strategy format strings to IO.open format strings
_FORMAT_MAP = {
    "nitf": "nitf",
    "geotiff": "tiff",
    "png": "png",
}


# =============================================================================
# Property 1: Stream Read Equivalence
# =============================================================================


@pytest.mark.property
class TestStreamReadEquivalence:
    """Property 1: Stream Read Equivalence.

    For any valid image and supported lossless format, create a DatasetReader
    via ``IO.open(stream, "r", format=fmt)`` where the stream contains the
    file bytes, and compare against a reader from ``IO.open(file_path, "r")``
    — asset keys, image dimensions, pixel types, and pixel data must be
    identical.

    **Validates: Requirements 1.1, 1.6, 5.1, 5.4**
    """

    @given(image_data=convenience_image())
    @pbt_settings
    def test_stream_read_equivalence(self, image_data):
        """Stream-based reader produces identical results to file-based reader.

        Feature: stream-io-support, Property 1: Stream Read Equivalence
        """
        array, pixel_type_name, format_string, path_suffix = image_data

        # Map the convenience format string to the IO.open format string
        io_format = _FORMAT_MAP[format_string]

        # Write the image to a temp file
        with tempfile.NamedTemporaryFile(
            suffix=path_suffix, delete=False
        ) as f:
            output_path = Path(f.name)

        try:
            # Write using imsave (NITF needs compression="none" for lossless)
            if format_string == "nitf":
                imsave(str(output_path), array, compression="none")
            else:
                imsave(str(output_path), array)

            # Read the file bytes into a BytesIO stream
            file_bytes = output_path.read_bytes()
            stream = io.BytesIO(file_bytes)

            # Open via IO.open(stream, "r", format=io_format)
            with IO.open(stream, "r", io_format) as stream_reader:
                stream_keys = stream_reader.get_asset_keys(
                    asset_type=AssetType.Image
                )

                # Open via IO.open(file_path, "r")
                with IO.open(str(output_path), "r") as file_reader:
                    file_keys = file_reader.get_asset_keys(
                        asset_type=AssetType.Image
                    )

                    # Compare asset keys
                    assert stream_keys == file_keys, (
                        f"Asset keys differ: stream={stream_keys}, "
                        f"file={file_keys}"
                    )

                    # Compare each asset's properties and pixel data
                    for key in file_keys:
                        file_asset = file_reader.get_asset(key)
                        stream_asset = stream_reader.get_asset(key)

                        # Compare image dimensions
                        assert stream_asset.num_columns == file_asset.num_columns, (
                            f"num_columns mismatch for '{key}': "
                            f"stream={stream_asset.num_columns}, "
                            f"file={file_asset.num_columns}"
                        )
                        assert stream_asset.num_rows == file_asset.num_rows, (
                            f"num_rows mismatch for '{key}': "
                            f"stream={stream_asset.num_rows}, "
                            f"file={file_asset.num_rows}"
                        )
                        assert stream_asset.num_bands == file_asset.num_bands, (
                            f"num_bands mismatch for '{key}': "
                            f"stream={stream_asset.num_bands}, "
                            f"file={file_asset.num_bands}"
                        )

                        # Compare pixel types
                        assert (
                            stream_asset.pixel_value_type
                            == file_asset.pixel_value_type
                        ), (
                            f"pixel_value_type mismatch for '{key}': "
                            f"stream={stream_asset.pixel_value_type}, "
                            f"file={file_asset.pixel_value_type}"
                        )

                        # Compare pixel data by reading all blocks
                        file_image = read_full_image(
                            file_asset,
                            file_asset.num_bands,
                            file_asset.num_rows,
                            file_asset.num_columns,
                        )
                        stream_image = read_full_image(
                            stream_asset,
                            stream_asset.num_bands,
                            stream_asset.num_rows,
                            stream_asset.num_columns,
                        )

                        np.testing.assert_array_equal(
                            stream_image,
                            file_image,
                            err_msg=(
                                f"Pixel data mismatch for '{key}'. "
                                f"format={format_string}, "
                                f"shape={array.shape}, "
                                f"dtype={pixel_type_name}"
                            ),
                        )
        finally:
            if output_path.exists():
                output_path.unlink()
