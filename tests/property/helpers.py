"""Shared helper functions for property-based tests.

This module provides reusable utilities that are duplicated across multiple
test files, centralised here for consistency.
"""

import numpy as np

from .strategies import get_numpy_dtype


def read_full_image(asset, num_bands: int, num_rows: int, num_cols: int) -> np.ndarray:
    """Read all blocks from an asset and reassemble into a CHW numpy array.

    Args:
        asset: ImageAssetProvider to read from.
        num_bands: Expected number of bands.
        num_rows: Expected number of rows.
        num_cols: Expected number of columns.

    Returns:
        Reassembled image array in BSQ format (bands, rows, cols).
    """
    block_grid_rows, block_grid_cols = asset.block_grid_size
    block_bands, block_rows, block_cols = asset.block_shape

    dtype = get_numpy_dtype(asset.pixel_value_type)
    result = np.zeros((num_bands, num_rows, num_cols), dtype=dtype)

    for block_row in range(block_grid_rows):
        for block_col in range(block_grid_cols):
            block = asset.get_block(block_row, block_col, 0)

            start_row = block_row * block_rows
            start_col = block_col * block_cols
            end_row = min(start_row + block.shape[1], num_rows)
            end_col = min(start_col + block.shape[2], num_cols)

            actual_rows = end_row - start_row
            actual_cols = end_col - start_col

            result[:, start_row:end_row, start_col:end_col] = block[:, :actual_rows, :actual_cols]

    return result
