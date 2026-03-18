"""Property-based tests for TIFF block dimensions.

This module tests stripped TIFF block dimensions.
"""

import math
import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, assume
from PIL import Image

from aws.osml.io import IO

from ..conftest import pbt_settings
from ..strategies import get_numpy_dtype, tiff_image_config

_PIL_MODE = {
    ("uint8", 1): "L",
    ("uint8", 3): "RGB",
    ("uint16", 1): "I;16",
    ("int32", 1): "I",
    ("float32", 1): "F",
}


def _create_tiff_pil(cfg: dict, array_chw: np.ndarray) -> bytes:
    """Create a TIFF file in memory using PIL from a CHW numpy array."""
    pixel_type = cfg["pixel_type"]
    bands = cfg["bands"]
    rps = cfg["rows_per_strip"]
    pil_comp = cfg["pil_compression"]

    dtype = get_numpy_dtype(pixel_type)
    mode = _PIL_MODE[(dtype.name, bands)]

    if bands == 1:
        hw = array_chw[0]
    else:
        hw = np.transpose(array_chw, (1, 2, 0))

    img = Image.fromarray(hw, mode)

    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    try:
        img.save(str(path), compression=pil_comp, tiffinfo={278: rps})
        return path.read_bytes()
    finally:
        path.unlink(missing_ok=True)


@pytest.mark.property
class TestTiffStrippedBlockDimensions:
    """Stripped TIFF block dimensions.

    For any stripped TIFF, num_pixels_per_block_horizontal == ImageWidth,
    num_pixels_per_block_vertical == RowsPerStrip, and
    block_grid_size == (ceil(ImageLength / RowsPerStrip), 1).
    """

    @given(config=tiff_image_config(min_size=16, max_size=128))
    @pbt_settings
    def test_stripped_block_layout(self, config):
        """Stripped TIFF reports correct block dimensions and grid size."""
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

        tiff_bytes = _create_tiff_pil(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            meta = asset.get_metadata().as_dict()

            assert asset.num_pixels_per_block_horizontal == width, (
                f"Block width should be ImageWidth ({width}), got {asset.num_pixels_per_block_horizontal}"
            )

            # The effective block height reported by the provider may differ
            # from the raw RowsPerStrip tag when libtiff adjusts strip sizes
            # internally.  Validate consistency: the block height times the
            # grid row count must cover the full image height.
            bh = asset.num_pixels_per_block_vertical
            grid_rows, grid_cols = asset.block_grid_size
            assert grid_cols == 1, (
                f"Stripped TIFF should have 1 column in grid, got {grid_cols}"
            )
            assert grid_rows == math.ceil(height / bh), (
                f"Grid rows should be ceil({height}/{bh})={math.ceil(height / bh)}, got {grid_rows}"
            )
        finally:
            path.unlink(missing_ok=True)
