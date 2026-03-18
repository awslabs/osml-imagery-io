"""Shared helper functions for property-based tests.

This module provides reusable utilities that are duplicated across multiple
test files, centralised here for consistency.

Helpers are split into three categories:

1. **Format-specific write/read helpers** — One per container format. Each
   handles temp file lifecycle, provider setup, IO.open for write and read,
   and full-image reassembly.

2. **Format-agnostic assertion helpers** — ``assert_lossless_match`` and
   ``assert_lossy_quality`` encapsulate the repeated comparison patterns.

3. **Masking helpers** — JBP-specific helpers for writing sparse-block images
   and verifying mask patterns survive roundtrip.
"""

import tempfile
from pathlib import Path

import numpy as np

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
)

from .quality import calculate_psnr, calculate_ssim, MIN_PSNR_DB, MIN_SSIM
from .strategies import get_numpy_dtype


# ---------------------------------------------------------------------------
# Full-image reassembly
# ---------------------------------------------------------------------------


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


# ---------------------------------------------------------------------------
# Format-specific write/read helpers
# ---------------------------------------------------------------------------


def write_and_read_jbp(
    array: np.ndarray,
    pixel_type,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    metadata_hints: dict,
    block_width: int = 64,
    block_height: int = 64,
    format: str = "nitf",
) -> np.ndarray:
    """Write a JBP/NITF file and read back the decoded image.

    Handles temp file lifecycle, provider setup, write, read, and
    full-image reassembly.  The *format* parameter allows testing NSIF
    by passing ``format="nsif"``.

    Args:
        array: Source image in BSQ layout (bands, rows, cols).
        pixel_type: PixelType enum value.
        num_bands: Number of bands.
        num_rows: Number of rows.
        num_cols: Number of columns.
        metadata_hints: Dict of metadata key/value pairs (e.g. ``{"IC": "NC"}``).
        block_width: Block width (clamped to image width internally).
        block_height: Block height (clamped to image height internally).
        format: Container format string for ``IO.open`` (``"nitf"`` or ``"nsif"``).

    Returns:
        Decoded image array in BSQ format (bands, rows, cols).
    """
    with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
        path = Path(f.name)

    try:
        metadata = BufferedMetadataProvider()
        for k, v in metadata_hints.items():
            metadata.set(k, v)

        provider = BufferedImageAssetProvider.create(
            key="image_segment_0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(num_cols, block_width),
            block_height=min(num_rows, block_height),
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        writer = IO.open([str(path)], "w", format)
        writer.add_asset(
            key="image_segment_0",
            provider=provider,
            title="Test Image",
            description="Property test image",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image_segment_0")
        decoded = read_full_image(asset, num_bands, num_rows, num_cols)
        reader.close()

        return decoded
    finally:
        if path.exists():
            path.unlink()


def write_and_read_tiff(
    array: np.ndarray,
    pixel_type,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    hints: dict,
) -> np.ndarray:
    """Write a TIFF file and read back the decoded image.

    Args:
        array: Source image in BSQ layout (bands, rows, cols).
        pixel_type: PixelType enum value.
        num_bands: Number of bands.
        num_rows: Number of rows.
        num_cols: Number of columns.
        hints: Dict of encoding hint strings (e.g. Compression, TileWidth, …).

    Returns:
        Decoded image array in BSQ format (bands, rows, cols).
    """
    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    try:
        metadata = BufferedMetadataProvider()
        for k, v in hints.items():
            metadata.set(k, v)

        tile_w = int(hints.get("TileWidth", "256"))
        tile_h = int(hints.get("TileHeight", "256"))

        provider = BufferedImageAssetProvider.create(
            key="image_segment_0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(num_cols, tile_w),
            block_height=min(num_rows, tile_h),
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        writer = IO.open([str(path)], "w", "tiff")
        writer.metadata = metadata
        writer.add_asset(
            key="image_segment_0",
            provider=provider,
            title="Test Image",
            description="Property test",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image_segment_0")
        decoded = read_full_image(asset, num_bands, num_rows, num_cols)
        reader.close()

        return decoded
    finally:
        if path.exists():
            path.unlink()


# ---------------------------------------------------------------------------
# Format-agnostic assertion helpers
# ---------------------------------------------------------------------------


def assert_lossless_match(original: np.ndarray, decoded: np.ndarray) -> None:
    """Assert exact pixel equality, with NaN-aware comparison for floats.

    Checks shape, dtype, and pixel values.  For floating-point arrays,
    NaN values are treated as equal.

    Raises:
        AssertionError: On any mismatch.
    """
    assert decoded.shape == original.shape, (
        f"Shape mismatch: expected {original.shape}, got {decoded.shape}"
    )
    assert decoded.dtype == original.dtype, (
        f"Dtype mismatch: expected {original.dtype}, got {decoded.dtype}"
    )

    if np.issubdtype(original.dtype, np.floating):
        arrays_equal = np.array_equal(decoded, original, equal_nan=True)
    else:
        arrays_equal = np.array_equal(decoded, original)

    assert arrays_equal, (
        f"Pixel values differ. "
        f"Max diff: {np.nanmax(np.abs(decoded.astype(np.float64) - original.astype(np.float64)))}"
    )


def assert_lossy_quality(
    original: np.ndarray,
    decoded: np.ndarray,
    min_psnr: float = MIN_PSNR_DB,
    min_ssim: float = MIN_SSIM,
) -> None:
    """Assert lossy-compression quality bounds (PSNR and SSIM).

    Also verifies shape and dtype preservation.

    Args:
        original: Original image array.
        decoded: Decoded image array.
        min_psnr: Minimum acceptable PSNR in dB.
        min_ssim: Minimum acceptable SSIM.

    Raises:
        AssertionError: On shape/dtype mismatch or quality below thresholds.
    """
    assert decoded.shape == original.shape, (
        f"Shape mismatch: expected {original.shape}, got {decoded.shape}"
    )
    assert decoded.dtype == original.dtype, (
        f"Dtype mismatch: expected {original.dtype}, got {decoded.dtype}"
    )

    psnr = calculate_psnr(original, decoded, use_actual_range=True)
    ssim = calculate_ssim(original, decoded)

    assert psnr >= min_psnr, (
        f"PSNR {psnr:.2f} dB is below minimum threshold {min_psnr} dB"
    )
    assert ssim >= min_ssim, (
        f"SSIM {ssim:.4f} is below minimum threshold {min_ssim}"
    )


# ---------------------------------------------------------------------------
# Masking helpers (JBP-specific)
# ---------------------------------------------------------------------------


def write_masked_jbp(
    array: np.ndarray,
    pixel_type,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    block_height: int,
    block_width: int,
    provided_blocks: set,
    metadata_hints: dict,
) -> Path:
    """Write a masked JBP/NITF file.  Returns the temp file path.

    The caller is responsible for cleanup (``path.unlink()``).  This is
    separate from :func:`write_and_read_jbp` because masked tests need
    access to the reader asset for ``has_block()`` / ``get_block()``
    inspection.

    Args:
        array: Full source image in BSQ layout (bands, rows, cols).
        pixel_type: PixelType enum value.
        num_bands: Number of bands.
        num_rows: Number of rows.
        num_cols: Number of columns.
        block_height: Block height.
        block_width: Block width.
        provided_blocks: Set of ``(block_row, block_col)`` tuples to write.
        metadata_hints: Dict of metadata key/value pairs (must include IC).

    Returns:
        Path to the written temp file.
    """
    with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
        path = Path(f.name)

    metadata = BufferedMetadataProvider()
    for k, v in metadata_hints.items():
        metadata.set(k, v)

    provider = BufferedImageAssetProvider.create(
        key="image_segment_0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=block_width,
        block_height=block_height,
        pixel_type=pixel_type,
        metadata=metadata,
    )

    for block_row, block_col in provided_blocks:
        start_row = block_row * block_height
        start_col = block_col * block_width
        end_row = min(start_row + block_height, num_rows)
        end_col = min(start_col + block_width, num_cols)
        block = array[:, start_row:end_row, start_col:end_col].copy()
        provider.set_block(block_row, block_col, block)

    writer = IO.open([str(path)], "w", "nitf")
    writer.add_asset(
        key="image_segment_0",
        provider=provider,
        title="Test Masked Image",
        description="Property test masked image",
        roles=["data"],
    )
    writer.close()

    return path


def assert_mask_preserved(
    asset,
    provided_blocks: set,
    num_block_rows: int,
    num_block_cols: int,
) -> None:
    """Assert that the mask pattern survived the roundtrip.

    Verifies that ``has_block()`` returns ``True`` for every block in
    *provided_blocks* and ``False`` for every other block in the grid.

    Args:
        asset: ImageAssetProvider obtained from the reader.
        provided_blocks: Set of ``(block_row, block_col)`` that were written.
        num_block_rows: Total block rows in the grid.
        num_block_cols: Total block columns in the grid.

    Raises:
        AssertionError: On any mask-pattern mismatch.
    """
    all_blocks = {
        (r, c) for r in range(num_block_rows) for c in range(num_block_cols)
    }
    expected_masked = all_blocks - provided_blocks

    actual_masked = set()
    for r in range(num_block_rows):
        for c in range(num_block_cols):
            if not asset.has_block(r, c, 0):
                actual_masked.add((r, c))

    assert actual_masked == expected_masked, (
        f"Masked block pattern mismatch.\n"
        f"Expected masked: {sorted(expected_masked)}\n"
        f"Actual masked: {sorted(actual_masked)}\n"
        f"Missing from actual: {sorted(expected_masked - actual_masked)}\n"
        f"Extra in actual: {sorted(actual_masked - expected_masked)}"
    )
