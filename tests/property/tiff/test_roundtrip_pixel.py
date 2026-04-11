"""Property-based tests for TIFF pixel roundtrip operations.

This module tests:
- Pixel data roundtrip (stripped TIFFs via PIL → our reader)
- Lossless pixel roundtrip (our writer → our reader)
- Band subsetting preserves correct data
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, PixelType
from hypothesis import assume, given
from PIL import Image

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, assert_lossy_quality, read_full_image, write_and_read_tiff
from ..strategies import (
    get_numpy_dtype,
    tiff_image_config,
    tiff_writable_image,
)

# PIL mode mapping for creating images from numpy arrays.
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


# =============================================================================
# Pixel data roundtrip (PIL → our reader)
# =============================================================================


@pytest.mark.property
class TestTiffPixelRoundtrip:
    """Pixel data roundtrip (PIL writer → our reader).

    For any valid image configuration writable by PIL, writing a TIFF and
    reading it back through TIFFImageAssetProvider.get_block() produces
    byte-identical pixel data in band-sequential (CHW) format.
    """

    @given(config=tiff_image_config(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_stripped_roundtrip(self, config):
        """Stripped TIFF pixel data survives a write-read cycle exactly."""
        pixel_type = config["pixel_type"]
        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        rng = np.random.RandomState(42)
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
            asset = reader.get_asset("image:0")

            assert asset.num_columns == width
            assert asset.num_rows == height
            assert asset.num_bands == bands

            decoded = read_full_image(asset, bands, height, width)

            assert decoded.shape == array_chw.shape, (
                f"Shape mismatch: expected {array_chw.shape}, got {decoded.shape}"
            )

            if np.issubdtype(dtype, np.floating):
                np.testing.assert_array_equal(decoded, array_chw)
            else:
                np.testing.assert_array_equal(decoded, array_chw)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Lossless pixel roundtrip (our writer → our reader)
# =============================================================================


@pytest.mark.property
class TestTiffLosslessPixelRoundtrip:
    """TIFF lossless pixel roundtrip (native writer).

    For any supported pixel type, band count, image dimensions, lossless
    compression, and planar configuration, writing via TIFFDatasetWriter
    and reading back produces pixel data identical to the original input.
    """

    @given(tiff_writable_image(min_size=16, max_size=64, min_bands=1, max_bands=4))
    @pbt_settings
    def test_lossless_pixel_roundtrip(self, image_tuple):
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple
        decoded = write_and_read_tiff(array, pixel_type, num_bands, num_rows, num_cols, hints)
        assert_lossless_match(array, decoded)


# =============================================================================
# JPEG lossy pixel roundtrip (our writer → our reader)
# =============================================================================


@pytest.mark.property
class TestTiffJpegPixelRoundtrip:
    """TIFF pixel roundtrip including JPEG compression.

    Feature: tiff-jpeg-compression, Property 7: JPEG roundtrip fidelity

    For any valid image and compression setting drawn from the full set
    (including JPEG), writing via TIFFDatasetWriter and reading back
    produces faithful pixel data: exact match for lossless codecs,
    PSNR ≥ 30 dB for JPEG.
    """

    @given(tiff_writable_image(min_size=16, max_size=64, min_bands=1, max_bands=4, include_jpeg=True))
    @pbt_settings
    def test_pixel_roundtrip_with_jpeg(self, image_tuple):
        """Pixel data survives a write-read cycle for all compressions including JPEG."""
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple
        decoded = write_and_read_tiff(array, pixel_type, num_bands, num_rows, num_cols, hints)

        if hints["259"] == 7:
            # JPEG is lossy — verify quality bounds
            assert_lossy_quality(array, decoded)
        else:
            # Lossless codecs — exact match
            assert_lossless_match(array, decoded)


# =============================================================================
# Band subsetting preserves correct data
# =============================================================================


@pytest.mark.property
class TestTiffBandSubsetting:
    """Band subsetting preserves correct data.

    For any multi-band TIFF and any non-empty subset of band indices,
    get_block() with that subset returns only the requested bands.
    """

    @given(config=tiff_image_config(min_size=16, max_size=48, min_bands=3, max_bands=3))
    @pbt_settings
    def test_band_subset_matches_full_read(self, config):
        """Reading a band subset matches the same bands from a full read."""
        pixel_type = config["pixel_type"]
        assume(pixel_type == PixelType.UInt8)

        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        rng = np.random.RandomState(123)
        array_chw = rng.randint(0, 256, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff_pil(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            full_block = asset.get_block(0, 0, 0, None)

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
