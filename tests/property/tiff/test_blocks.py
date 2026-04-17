"""Property-based tests for TIFF block dimensions.

This module tests TIFF block dimensions for tiled TIFFs written by the
native writer.
"""

import math
import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO
from hypothesis import assume, given

from ..conftest import pbt_settings
from ..helpers import write_tiff_native_bytes
from ..strategies import get_numpy_dtype, tiff_image_config


@pytest.mark.property
class TestTiffBlockDimensions:
    """Tiled TIFF block dimensions.

    For any tiled TIFF, the block grid covers the full image and the
    block dimensions match the tile dimensions used during writing.
    """

    @given(config=tiff_image_config(min_size=16, max_size=128))
    @pbt_settings
    def test_block_layout(self, config):
        """TIFF reports correct block dimensions and grid size."""
        width, height = config["width"], config["height"]
        bands = config["bands"]
        pixel_type = config["pixel_type"]
        rps = config["rows_per_strip"]
        dtype = get_numpy_dtype(pixel_type)

        assume(not (rps >= height and dtype.itemsize > 1))

        rng = np.random.RandomState(7)
        if np.issubdtype(dtype, np.floating):
            array_chw = rng.rand(bands, height, width).astype(dtype)
        elif np.issubdtype(dtype, np.signedinteger):
            info = np.iinfo(dtype)
            array_chw = rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
        else:
            info = np.iinfo(dtype)
            array_chw = rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)

        tiff_bytes = write_tiff_native_bytes(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            asset.get_metadata().as_dict()

            bw = asset.num_pixels_per_block_horizontal
            bh = asset.num_pixels_per_block_vertical
            grid_rows, grid_cols = asset.block_grid_size

            # Block grid must cover the full image
            assert grid_rows == math.ceil(height / bh), (
                f"Grid rows should be ceil({height}/{bh})={math.ceil(height / bh)}, got {grid_rows}"
            )
            assert grid_cols == math.ceil(width / bw), (
                f"Grid cols should be ceil({width}/{bw})={math.ceil(width / bw)}, got {grid_cols}"
            )

            # Block dimensions must be at least as large as the image
            # dimensions divided by the grid size
            assert bw * grid_cols >= width, (
                f"Block grid does not cover image width: {bw}*{grid_cols} < {width}"
            )
            assert bh * grid_rows >= height, (
                f"Block grid does not cover image height: {bh}*{grid_rows} < {height}"
            )
        finally:
            path.unlink(missing_ok=True)
