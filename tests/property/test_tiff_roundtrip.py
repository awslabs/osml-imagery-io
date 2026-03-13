"""Property-based tests for TIFF roundtrip and layout operations.

This module tests correctness properties for TIFF reading:
- Property 1: Pixel data roundtrip (stripped TIFFs via PIL)
- Property 2: Band subsetting preserves correct data
- Property 10: Stripped TIFF block dimensions

Test TIFFs are generated with PIL (Pillow), which writes stripped, chunky
(PlanarConfiguration=1) TIFFs. Tiled and planar layout tests are deferred
to Phase 2 when our own TIFF writer is available.

**Validates: Requirements 4.2, 4.3, 4.4, 4.5, 4.8, 4.9, 4.10, 9.1, 9.2,
9.3, 9.5, 10.1, 11.1, 11.2, 11.3**
"""

import math
import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase, assume
from PIL import Image

from aws.osml.io import IO, PixelType

from .strategies import (
    get_numpy_dtype,
    tiff_image_config,
)


# Hypothesis settings for I/O-bound TIFF tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


# PIL mode mapping for creating images from numpy arrays.
# PixelType is not hashable, so we key by (dtype_name, bands).
_PIL_MODE = {
    ("uint8", 1): "L",
    ("uint8", 3): "RGB",
    ("uint16", 1): "I;16",
    ("int32", 1): "I",
    ("float32", 1): "F",
}


def _create_tiff(cfg: dict, array_chw: np.ndarray) -> bytes:
    """Create a TIFF file in memory using PIL from a CHW numpy array.

    Args:
        cfg: Config dict from tiff_image_config strategy.
        array_chw: Pixel data in (bands, height, width) layout.

    Returns:
        TIFF file bytes.
    """
    pixel_type = cfg["pixel_type"]
    bands = cfg["bands"]
    rps = cfg["rows_per_strip"]
    pil_comp = cfg["pil_compression"]

    dtype = get_numpy_dtype(pixel_type)
    mode = _PIL_MODE[(dtype.name, bands)]

    if bands == 1:
        # Single-band: squeeze to (H, W)
        hw = array_chw[0]
    else:
        # Multi-band: transpose CHW -> HWC for PIL
        hw = np.transpose(array_chw, (1, 2, 0))

    img = Image.fromarray(hw, mode)

    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    try:
        img.save(str(path), compression=pil_comp, tiffinfo={278: rps})
        return path.read_bytes()
    finally:
        path.unlink(missing_ok=True)


def _read_full_image(asset, num_bands: int, num_rows: int, num_cols: int) -> np.ndarray:
    """Read all blocks from a TIFF asset and reassemble into CHW array."""
    grid_rows, grid_cols = asset.block_grid_size
    bh = asset.num_pixels_per_block_vertical
    bw = asset.num_pixels_per_block_horizontal
    dtype = get_numpy_dtype(asset.pixel_value_type)
    result = np.zeros((num_bands, num_rows, num_cols), dtype=dtype)

    for r in range(grid_rows):
        for c in range(grid_cols):
            block = asset.get_block(r, c, 0, None)
            y0, x0 = r * bh, c * bw
            y1 = min(y0 + block.shape[1], num_rows)
            x1 = min(x0 + block.shape[2], num_cols)
            result[:, y0:y1, x0:x1] = block[:, : y1 - y0, : x1 - x0]

    return result


# =============================================================================
# Property 1: Pixel data roundtrip
# =============================================================================


@pytest.mark.property
class TestTiffPixelRoundtrip:
    """Property 1: Pixel data roundtrip

    For any valid image configuration writable by PIL (pixel type from
    {uint8, uint16, int32, float32}, 1 or 3 bands, compressions from
    {none, LZW, Deflate, PackBits}), writing a TIFF and reading it back
    through TIFFImageAssetProvider.get_block() produces byte-identical
    pixel data in band-sequential (CHW) format.

    # Feature: libtiff-ffi-tiff-reading, Property 1: Pixel data roundtrip
    **Validates: Requirements 4.2, 4.3, 4.4, 4.5, 4.8, 4.9, 9.1, 9.2, 9.3, 9.5, 10.1, 11.3**
    """

    @given(config=tiff_image_config(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_stripped_roundtrip(self, config):
        """Stripped TIFF pixel data survives a write-read cycle exactly."""
        pixel_type = config["pixel_type"]
        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        # Generate deterministic pixel data
        rng = np.random.RandomState(42)
        if np.issubdtype(dtype, np.floating):
            array_chw = rng.rand(bands, height, width).astype(dtype)
        elif np.issubdtype(dtype, np.signedinteger):
            info = np.iinfo(dtype)
            array_chw = rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
        else:
            info = np.iinfo(dtype)
            array_chw = rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")

            # Verify dimensions match
            assert asset.num_columns == width
            assert asset.num_rows == height
            assert asset.num_bands == bands

            decoded = _read_full_image(asset, bands, height, width)

            assert decoded.shape == array_chw.shape, (
                f"Shape mismatch: expected {array_chw.shape}, got {decoded.shape}"
            )

            # Float arrays need NaN-aware comparison
            if np.issubdtype(dtype, np.floating):
                np.testing.assert_array_equal(decoded, array_chw)
            else:
                np.testing.assert_array_equal(decoded, array_chw)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 2: Band subsetting preserves correct data
# =============================================================================


@pytest.mark.property
class TestTiffBandSubsetting:
    """Property 2: Band subsetting preserves correct data

    For any multi-band TIFF and any non-empty subset of band indices,
    get_block() with that subset returns only the requested bands, matching
    the corresponding bands from a full read.

    # Feature: libtiff-ffi-tiff-reading, Property 2: Band subsetting
    **Validates: Requirements 4.10**
    """

    @given(config=tiff_image_config(min_size=16, max_size=48, min_bands=3, max_bands=3))
    @pbt_settings
    def test_band_subset_matches_full_read(self, config):
        """Reading a band subset matches the same bands from a full read."""
        pixel_type = config["pixel_type"]
        # Only uint8 supports 3 bands in PIL
        assume(pixel_type == PixelType.UInt8)

        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        rng = np.random.RandomState(123)
        array_chw = rng.randint(0, 256, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")

            # Full read
            full_block = asset.get_block(0, 0, 0, None)

            # Test several subsets
            for subset in [[0], [2], [0, 2], [1, 2], [0, 1, 2]]:
                sub_block = asset.get_block(0, 0, 0, subset)
                assert sub_block.shape[0] == len(subset), (
                    f"Expected {len(subset)} bands, got {sub_block.shape[0]}"
                )
                for i, band_idx in enumerate(subset):
                    np.testing.assert_array_equal(
                        sub_block[i],
                        full_block[band_idx],
                        err_msg=f"Band {band_idx} mismatch in subset {subset}",
                    )
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 10: Stripped TIFF block dimensions
# =============================================================================


@pytest.mark.property
class TestTiffStrippedBlockDimensions:
    """Property 10: Stripped TIFF block dimensions

    For any stripped TIFF, num_pixels_per_block_horizontal == ImageWidth,
    num_pixels_per_block_vertical == RowsPerStrip, and
    block_grid_size == (ceil(ImageLength / RowsPerStrip), 1).

    # Feature: libtiff-ffi-tiff-reading, Property 10: Stripped TIFF block dimensions
    **Validates: Requirements 11.1, 11.2**
    """

    @given(config=tiff_image_config(min_size=16, max_size=128))
    @pbt_settings
    def test_stripped_block_layout(self, config):
        """Stripped TIFF reports correct block dimensions and grid size."""
        width, height = config["width"], config["height"]
        bands = config["bands"]
        pixel_type = config["pixel_type"]
        dtype = get_numpy_dtype(pixel_type)

        rng = np.random.RandomState(7)
        if np.issubdtype(dtype, np.floating):
            array_chw = rng.rand(bands, height, width).astype(dtype)
        elif np.issubdtype(dtype, np.signedinteger):
            info = np.iinfo(dtype)
            array_chw = rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
        else:
            info = np.iinfo(dtype)
            array_chw = rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            meta = asset.get_metadata().as_dict()

            # PIL may silently adjust RowsPerStrip, so read the actual value
            actual_rps = meta["RowsPerStrip"]

            assert asset.num_pixels_per_block_horizontal == width, (
                f"Block width should be ImageWidth ({width}), got {asset.num_pixels_per_block_horizontal}"
            )
            assert asset.num_pixels_per_block_vertical == actual_rps, (
                f"Block height should be RowsPerStrip ({actual_rps}), got {asset.num_pixels_per_block_vertical}"
            )

            expected_grid = (math.ceil(height / actual_rps), 1)
            assert asset.block_grid_size == expected_grid, (
                f"Grid should be {expected_grid}, got {asset.block_grid_size}"
            )
        finally:
            path.unlink(missing_ok=True)
