"""Property-based end-to-end tests for standalone JPEG 2000 (.j2k) read/write.

This module validates:
1. Lossless roundtrip — write then read produces identical pixels.
2. Lossy roundtrip — write then read meets quality bounds (PSNR/SSIM).
3. Tile API contract — reading via get_block() on the asset provider returns
   correct shape and covers the full image.
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)
from hypothesis import given

from ..conftest import pbt_settings
from ..helpers import assert_lossless_match, assert_lossy_quality, read_full_image
from ..strategies import j2k_writable_image

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _write_j2k(
    array: np.ndarray,
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    lossless: bool = True,
    compression_ratio: float = 10.0,
) -> Path:
    """Write a standalone J2K file and return the path. Caller must clean up."""
    with tempfile.NamedTemporaryFile(suffix=".j2k", delete=False) as f:
        path = Path(f.name)

    metadata = BufferedMetadataProvider()
    metadata["J2K_LOSSLESS"] = lossless
    if not lossless:
        metadata["J2K_COMPRESSION_RATIO"] = compression_ratio

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=num_cols,
        block_height=num_rows,
        pixel_type=pixel_type,
        metadata=metadata,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "j2k")
    writer.metadata = metadata
    writer.add_asset(
        key="image:0",
        provider=provider,
        title="Test Image",
        description="J2K end-to-end test",
        roles=["data"],
    )
    writer.close()
    return path


# ---------------------------------------------------------------------------
# Lossless roundtrip
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestJ2KLosslessRoundtrip:
    """Standalone J2K lossless roundtrip.

    For any valid image (UInt8, UInt16, Int16; 1-4 bands), writing a lossless
    .j2k file and reading it back via IO.open() produces pixel-identical data.
    """

    @given(j2k_writable_image(min_size=16, max_size=64))
    @pbt_settings
    def test_lossless_pixel_roundtrip(self, image_tuple):
        """Lossless J2K write/read cycle preserves every pixel exactly."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        path = _write_j2k(array, pixel_type, num_bands, num_rows, num_cols, lossless=True)
        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            assert_lossless_match(array, decoded)
        finally:
            path.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# Lossy roundtrip
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestJ2KLossyRoundtrip:
    """Standalone J2K lossy roundtrip quality bounds.

    For any valid image with lossy compression, writing then reading SHALL
    produce an image with PSNR >= 28 dB and SSIM >= 0.95, with preserved
    shape and dtype.
    """

    @given(j2k_writable_image(min_size=32, max_size=64))
    @pbt_settings
    def test_lossy_quality_bounds(self, image_tuple):
        """Lossy J2K roundtrip meets minimum quality thresholds."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        path = _write_j2k(
            array, pixel_type, num_bands, num_rows, num_cols,
            lossless=False, compression_ratio=2.0,
        )
        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            assert_lossy_quality(array, decoded)
        finally:
            path.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# Tile API contract
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestJ2KTileReadContract:
    """Standalone J2K tile API contract.

    For any valid .j2k file opened via IO.open(), the image asset exposes a
    block grid, and reading all blocks via get_block() produces data that
    covers the full image dimensions with correct shape and dtype.
    """

    @given(j2k_writable_image(min_size=16, max_size=64))
    @pbt_settings
    def test_tile_grid_covers_full_image(self, image_tuple):
        """All blocks from get_block() reassemble to the full image dimensions."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        path = _write_j2k(array, pixel_type, num_bands, num_rows, num_cols, lossless=True)
        try:
            reader = IO.open([str(path)], "r")
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert len(keys) > 0, "No image assets found"

            asset = reader.get_asset(keys[0])
            grid_rows, grid_cols = asset.block_grid_size
            block_bands, block_h, block_w = asset.block_shape

            assert grid_rows >= 1
            assert grid_cols >= 1
            assert block_bands == num_bands

            # Verify total coverage
            total_rows_covered = 0
            total_cols_covered = 0

            for r in range(grid_rows):
                for c in range(grid_cols):
                    block = asset.get_block(r, c, 0)
                    assert block.ndim == 3
                    assert block.shape[0] == num_bands

                    if r == 0:
                        total_cols_covered += block.shape[2]
                    if c == 0:
                        total_rows_covered += block.shape[1]

            assert total_rows_covered == num_rows, (
                f"Row coverage {total_rows_covered} != image height {num_rows}"
            )
            assert total_cols_covered == num_cols, (
                f"Col coverage {total_cols_covered} != image width {num_cols}"
            )

            # Full reassembly check
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            assert_lossless_match(array, decoded)

            reader.close()
        finally:
            path.unlink(missing_ok=True)
