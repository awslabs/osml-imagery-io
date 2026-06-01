"""Property-based tests for tile assembly across grid mismatches.

This module validates:
1. Arbitrary dims × block sizes → assembled full image matches spatial
   concatenation of source blocks (pixel-exact).
2. Output tiles from TileAssembler cover all pixels exactly once
   (no gaps, no overlaps).

These tests exercise the full write pipeline by constructing
BufferedImageAssetProviders with one block grid and writing to formats
that use a different output tile grid.
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)
from hypothesis import assume, given
from hypothesis import strategies as st

from .conftest import pbt_settings
from .helpers import assert_lossless_match, read_full_image
from .strategies import get_numpy_dtype, pixel_types


@st.composite
def mismatched_grid_image(
    draw,
    min_size: int = 32,
    max_size: int = 128,
):
    """Strategy for an image with source block size != output tile size.

    Returns (array, pixel_type, num_bands, num_rows, num_cols,
             source_block_w, source_block_h, output_tile_w, output_tile_h)
    """
    pixel_type = draw(st.sampled_from([
        PixelType.UInt8,
        PixelType.UInt16,
        PixelType.Int16,
    ]))
    num_bands = draw(st.integers(min_value=1, max_value=3))
    num_rows = draw(st.integers(min_value=min_size, max_value=max_size))
    num_cols = draw(st.integers(min_value=min_size, max_value=max_size))

    source_block_w = draw(st.sampled_from([16, 32, 64, 128]))
    source_block_h = draw(st.sampled_from([16, 32, 64, 128]))
    output_tile_w = draw(st.sampled_from([16, 32, 64, 128]))
    output_tile_h = draw(st.sampled_from([16, 32, 64, 128]))

    # Ensure grids actually differ for interesting test cases
    assume(source_block_w != output_tile_w or source_block_h != output_tile_h)

    dtype = get_numpy_dtype(pixel_type)
    array = draw(
        st.builds(
            lambda: np.arange(
                num_bands * num_rows * num_cols, dtype=dtype
            ).reshape(num_bands, num_rows, num_cols),
        )
        if dtype == np.uint8
        else st.just(
            np.arange(
                num_bands * num_rows * num_cols, dtype=dtype
            ).reshape(num_bands, num_rows, num_cols)
            % np.iinfo(dtype).max
        )
    )

    return (
        array, pixel_type, num_bands, num_rows, num_cols,
        source_block_w, source_block_h, output_tile_w, output_tile_h,
    )


def _make_deterministic_array(pixel_type, num_bands, num_rows, num_cols):
    """Create a deterministic gradient array for pixel-exact verification."""
    dtype = get_numpy_dtype(pixel_type)
    total = num_bands * num_rows * num_cols
    if np.issubdtype(dtype, np.integer):
        max_val = np.iinfo(dtype).max
        arr = (np.arange(total, dtype=np.int64) % (max_val + 1)).astype(dtype)
    else:
        arr = np.linspace(0, 1, total, dtype=dtype)
    return arr.reshape(num_bands, num_rows, num_cols)


# =============================================================================
# Property 1: assembled image matches spatial concatenation of source blocks
# =============================================================================


@pytest.mark.property
class TestAssembledImageMatchesSourceBlocks:
    """For arbitrary grid combinations, writing from a multi-block source and
    reading back produces pixel-identical data to the original array.

    This validates that TileAssembler correctly maps output tiles to source
    blocks regardless of the grid mismatch.
    """

    @given(
        pixel_type=pixel_types(),
        num_bands=st.integers(min_value=1, max_value=3),
        dims=st.tuples(
            st.integers(min_value=32, max_value=128),
            st.integers(min_value=32, max_value=128),
        ),
        source_block=st.sampled_from([16, 32, 64, 128]),
        output_tile=st.sampled_from([32, 64, 128, 256]),
    )
    @pbt_settings
    def test_tiff_retiling_pixel_exact(
        self, pixel_type, num_bands, dims, source_block, output_tile
    ):
        """TIFF write from mismatched-grid provider → read back → pixel match.

        Uses lossless TIFF (Deflate), so roundtrip must be exact.
        """
        # TIFF tiles must be multiples of 16
        assume(output_tile % 16 == 0)

        num_rows, num_cols = dims
        array = _make_deterministic_array(pixel_type, num_bands, num_rows, num_cols)

        # Source provider uses source_block grid
        src_bw = min(source_block, num_cols)
        src_bh = min(source_block, num_rows)

        metadata = BufferedMetadataProvider()
        metadata["322"] = str(output_tile)  # TileWidth
        metadata["323"] = str(output_tile)  # TileLength
        metadata["259"] = 8                 # Deflate compression
        metadata["284"] = 1                 # Chunky

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=src_bw,
            block_height=src_bh,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "tiff")
            writer.metadata = metadata
            writer.add_asset("image:0", provider, "Test", "test", ["data"])
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            assert_lossless_match(array, decoded)
        finally:
            path.unlink(missing_ok=True)

    @given(
        num_bands=st.integers(min_value=1, max_value=3),
        dims=st.tuples(
            st.integers(min_value=32, max_value=96),
            st.integers(min_value=32, max_value=96),
        ),
        source_block=st.sampled_from([16, 32, 64]),
    )
    @pbt_settings
    def test_j2k_retiling_lossless(self, num_bands, dims, source_block):
        """J2K lossless write from mismatched source → read → pixel match.

        Uses J2K_TILE_WIDTH/J2K_TILE_HEIGHT metadata to set output tile
        dimensions different from the source block grid.
        """
        pixel_type = PixelType.UInt8
        num_rows, num_cols = dims
        # J2K tile must be >= 2^decomposition_levels; use 64 for safety
        j2k_tile = 64
        assume(source_block != j2k_tile)

        array = _make_deterministic_array(pixel_type, num_bands, num_rows, num_cols)

        src_bw = min(source_block, num_cols)
        src_bh = min(source_block, num_rows)

        metadata = BufferedMetadataProvider()
        metadata["J2K_LOSSLESS"] = True
        metadata["J2K_TILE_WIDTH"] = j2k_tile
        metadata["J2K_TILE_HEIGHT"] = j2k_tile

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=src_bw,
            block_height=src_bh,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        with tempfile.NamedTemporaryFile(suffix=".j2k", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "j2k")
            writer.metadata = metadata
            writer.add_asset("image:0", provider, "Test", "test", ["data"])
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            assert_lossless_match(array, decoded)
        finally:
            path.unlink(missing_ok=True)

    @given(
        pixel_type=st.sampled_from([PixelType.UInt8, PixelType.UInt16]),
        num_bands=st.sampled_from([1, 2, 3, 4]),
        dims=st.tuples(
            st.integers(min_value=16, max_value=64),
            st.integers(min_value=16, max_value=64),
        ),
        source_block=st.sampled_from([16, 32, 64]),
    )
    @pbt_settings
    def test_png_from_multiblock_source(self, pixel_type, num_bands, dims, source_block):
        """PNG write from multi-block source → read → pixel match.

        PNG is always a single-image output; the assembler must
        reassemble the full image from multiple source blocks.
        """
        num_rows, num_cols = dims
        # Ensure source is actually multi-block
        assume(source_block < num_rows or source_block < num_cols)

        array = _make_deterministic_array(pixel_type, num_bands, num_rows, num_cols)

        src_bw = min(source_block, num_cols)
        src_bh = min(source_block, num_rows)

        metadata = BufferedMetadataProvider()
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=src_bw,
            block_height=src_bh,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "png")
            writer.metadata = metadata
            writer.add_asset("image:0", provider, "Test", "test", ["data"])
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            assert_lossless_match(array, decoded)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 2: output tiles cover all pixels exactly once
# =============================================================================


@pytest.mark.property
class TestOutputTilesCoverAllPixels:
    """For arbitrary grid combinations, the output tiles produced by writing
    cover every pixel in the source image exactly once (no gaps, no overlaps).

    Verified by reading back the written file and checking that each pixel
    position in the reassembled output matches the original.
    """

    @given(
        dims=st.tuples(
            st.integers(min_value=17, max_value=100),
            st.integers(min_value=17, max_value=100),
        ),
        source_block=st.sampled_from([16, 32, 48, 64]),
        output_tile=st.sampled_from([32, 64, 128]),
    )
    @pbt_settings
    def test_tiff_tiles_no_gaps_no_overlaps(self, dims, source_block, output_tile):
        """Every pixel appears exactly once across all output TIFF tiles.

        Uses non-block-aligned image dimensions to stress edge tiles.
        """
        assume(output_tile % 16 == 0)
        num_rows, num_cols = dims
        num_bands = 1
        pixel_type = PixelType.UInt16

        # Use sequential values so any duplication or gap is detectable
        dtype = get_numpy_dtype(pixel_type)
        total_pixels = num_rows * num_cols
        array = np.arange(total_pixels, dtype=dtype).reshape(1, num_rows, num_cols)

        src_bw = min(source_block, num_cols)
        src_bh = min(source_block, num_rows)

        metadata = BufferedMetadataProvider()
        metadata["322"] = str(output_tile)
        metadata["323"] = str(output_tile)
        metadata["259"] = 1  # No compression
        metadata["284"] = 1

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=src_bw,
            block_height=src_bh,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "tiff")
            writer.metadata = metadata
            writer.add_asset("image:0", provider, "Test", "test", ["data"])
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            # Read all tiles and verify coverage
            grid_rows, grid_cols = asset.block_grid_size
            decoded = read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()

            # Every pixel must match — sequential values make gaps/overlaps obvious
            assert_lossless_match(array, decoded)

            # Additional check: verify the pixel set is complete
            expected_set = set(range(total_pixels))
            actual_set = set(decoded.flatten().tolist())
            assert actual_set == expected_set, (
                f"Pixel coverage mismatch: "
                f"missing={expected_set - actual_set}, "
                f"extra={actual_set - expected_set}"
            )
        finally:
            path.unlink(missing_ok=True)
