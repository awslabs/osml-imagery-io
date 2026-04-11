"""Property-based tests for masked image operations.

Consolidates all masking tests into three property-focused classes:

- TestMaskPatternPreservation — mask pattern survives roundtrip for all masked IC codes
- TestMaskedBlockDataCorrectness — block data correct (lossless exact, lossy quality-bounded)
- TestPadPixelPreservation — pad pixel value accessible after roundtrip

The ``masked_image`` strategy generates IC=NM, M8, and M3, so every test
method covers all three compression modes unless filtered with ``assume()``.
"""

import numpy as np
import pytest
from aws.osml.io import IO
from hypothesis import assume, given

from ..conftest import pbt_settings
from ..helpers import (
    assert_mask_preserved,
    write_masked_jbp,
)
from ..quality import MIN_PSNR_DB, calculate_psnr
from ..strategies import (
    calculate_safe_j2k_decomposition_levels,
    masked_image,
)

# IC codes that use lossless compression (exact pixel match expected)
_LOSSLESS_IC = {"NM"}

# IC codes that use lossy compression (quality-bounded match expected)
_LOSSY_IC = {"M3", "M8"}


def _masking_hints(ic_value, block_height, block_width, num_rows, num_cols, num_bands=1):
    """Build metadata hints dict for a masked IC code."""
    hints = {"IC": ic_value}
    if ic_value == "M8":
        hints["COMRAT"] = "N1.0"
        decomp = calculate_safe_j2k_decomposition_levels(
            block_height, block_width, num_rows, num_cols
        )
        hints["J2K_DECOMPOSITION_LEVELS"] = str(decomp)
    elif ic_value == "M3":
        hints["COMRAT"] = "85.0"
        if num_bands == 3:
            hints["IMODE"] = "P"
    return hints


def _is_lossless(ic_value: str) -> bool:
    """Return True if the IC code uses lossless compression."""
    return ic_value in _LOSSLESS_IC


# ============================================================================
# Property: Mask Pattern Preservation
# ============================================================================


@pytest.mark.property
class TestMaskPatternPreservation:
    """Mask pattern survives roundtrip for all masked IC codes (NM, M8, M3).

    For any masked image, the set of block coordinates where has_block()
    returns true/false SHALL be identical before writing and after reading.
    """

    @given(masked_image(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_mask_pattern_preserved(self, image_tuple):
        """Mask pattern is identical after roundtrip for any masked IC code."""
        (array, pixel_type, num_bands, num_rows, num_cols,
         block_height, block_width, provided_blocks, ic_value) = image_tuple

        assume(len(provided_blocks) > 0)

        num_block_rows = (num_rows + block_height - 1) // block_height
        num_block_cols = (num_cols + block_width - 1) // block_width

        hints = _masking_hints(ic_value, block_height, block_width, num_rows, num_cols, num_bands)
        path = write_masked_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            block_height, block_width, provided_blocks,
            metadata_hints=hints,
        )

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            assert_mask_preserved(asset, provided_blocks, num_block_rows, num_block_cols)
            reader.close()
        finally:
            if path.exists():
                path.unlink()


# ============================================================================
# Property: Masked Block Data Correctness
# ============================================================================


@pytest.mark.property
class TestMaskedBlockDataCorrectness:
    """Block data is correct for all provided blocks.

    Lossless IC codes (NM) require exact pixel match.
    Lossy IC codes (M3, M8) require PSNR >= MIN_PSNR_DB.
    """

    @given(masked_image(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_lossless_masked_block_data(self, image_tuple):
        """For lossless masked IC codes (NM), provided blocks match exactly."""
        (array, pixel_type, num_bands, num_rows, num_cols,
         block_height, block_width, provided_blocks, ic_value) = image_tuple

        assume(len(provided_blocks) > 0)
        assume(_is_lossless(ic_value))

        hints = _masking_hints(ic_value, block_height, block_width, num_rows, num_cols, num_bands)
        path = write_masked_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            block_height, block_width, provided_blocks,
            metadata_hints=hints,
        )

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            for block_row, block_col in provided_blocks:
                assert asset.has_block(block_row, block_col, 0), (
                    f"has_block({block_row}, {block_col}) should be True"
                )

                decoded_block = asset.get_block(block_row, block_col, 0)

                start_row = block_row * block_height
                start_col = block_col * block_width
                end_row = min(start_row + block_height, num_rows)
                end_col = min(start_col + block_width, num_cols)
                original_block = array[:, start_row:end_row, start_col:end_col]

                assert decoded_block.shape == original_block.shape, (
                    f"Block ({block_row}, {block_col}) shape mismatch: "
                    f"expected {original_block.shape}, got {decoded_block.shape}"
                )

                np.testing.assert_array_equal(
                    decoded_block, original_block,
                    err_msg=f"Block ({block_row}, {block_col}) data mismatch"
                )

            reader.close()
        finally:
            if path.exists():
                path.unlink()

    @given(masked_image(min_size=64, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_lossy_masked_block_data(self, image_tuple):
        """For lossy masked IC codes (M3, M8), provided blocks meet quality bounds."""
        (array, pixel_type, num_bands, num_rows, num_cols,
         block_height, block_width, provided_blocks, ic_value) = image_tuple

        assume(len(provided_blocks) > 0)
        assume(not _is_lossless(ic_value))

        hints = _masking_hints(ic_value, block_height, block_width, num_rows, num_cols, num_bands)
        path = write_masked_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            block_height, block_width, provided_blocks,
            metadata_hints=hints,
        )

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            for block_row, block_col in provided_blocks:
                assert asset.has_block(block_row, block_col, 0), (
                    f"has_block({block_row}, {block_col}) should be True"
                )

                decoded_block = asset.get_block(block_row, block_col, 0)

                start_row = block_row * block_height
                start_col = block_col * block_width
                end_row = min(start_row + block_height, num_rows)
                end_col = min(start_col + block_width, num_cols)
                original_block = array[:, start_row:end_row, start_col:end_col]

                assert decoded_block.shape == original_block.shape, (
                    f"Block ({block_row}, {block_col}) shape mismatch"
                )

                psnr = calculate_psnr(original_block, decoded_block, use_actual_range=True)
                assert psnr >= MIN_PSNR_DB, (
                    f"Block ({block_row}, {block_col}) PSNR {psnr:.2f} dB "
                    f"below threshold {MIN_PSNR_DB} dB (IC={ic_value})"
                )

            reader.close()
        finally:
            if path.exists():
                path.unlink()


# ============================================================================
# Property: Pad Pixel Value Preservation
# ============================================================================


@pytest.mark.property
class TestPadPixelPreservation:
    """Pad pixel value is accessible after roundtrip."""

    @given(masked_image(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_pad_pixel_value_accessible(self, image_tuple):
        """pad_pixel_value is accessible and numeric on the read asset."""
        (array, pixel_type, num_bands, num_rows, num_cols,
         block_height, block_width, provided_blocks, ic_value) = image_tuple

        assume(len(provided_blocks) > 0)

        hints = _masking_hints(ic_value, block_height, block_width, num_rows, num_cols, num_bands)
        path = write_masked_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            block_height, block_width, provided_blocks,
            metadata_hints=hints,
        )

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            pad_value = asset.pad_pixel_value
            assert isinstance(pad_value, (int, float)), (
                f"pad_pixel_value should be numeric, got {type(pad_value)}"
            )

            reader.close()
        finally:
            if path.exists():
                path.unlink()
