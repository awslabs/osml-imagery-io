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

import shutil
import tempfile
from pathlib import Path

import numpy as np
from aws.osml.io import (
    IO,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
)

from .quality import MIN_PSNR_DB, MIN_SSIM, calculate_psnr, calculate_ssim
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
            key="image:0",
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
            key="image:0",
            provider=provider,
            title="Test Image",
            description="Property test image",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image:0")
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
        hints: Dict of encoding hints. String values are set via ``set()``,
            non-string values (ints) via ``set_json()``.

    Returns:
        Decoded image array in BSQ format (bands, rows, cols).
    """
    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    try:
        metadata = BufferedMetadataProvider()
        for k, v in hints.items():
            if isinstance(v, str):
                metadata.set(k, v)
            else:
                metadata.set_json(k, v)

        tile_w = int(hints.get("322", "256"))   # TileWidth
        tile_h = int(hints.get("323", "256"))   # TileLength

        provider = BufferedImageAssetProvider.create(
            key="image:0",
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
            key="image:0",
            provider=provider,
            title="Test Image",
            description="Property test",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image:0")
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
        key="image:0",
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
        key="image:0",
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


def write_and_read_j2k(
    array: np.ndarray,
    pixel_type,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    lossless: bool = True,
) -> np.ndarray:
    """Write a JPEG 2000 file and read back the decoded image.

    Handles temp file lifecycle, provider setup, IO.open for write and read,
    and full-image reassembly.

    Args:
        array: Source image in BSQ layout (bands, rows, cols).
        pixel_type: PixelType enum value.
        num_bands: Number of bands.
        num_rows: Number of rows.
        num_cols: Number of columns.
        lossless: Whether to use lossless encoding (default True).

    Returns:
        Decoded image array in BSQ format (bands, rows, cols).
    """
    with tempfile.NamedTemporaryFile(suffix=".j2k", delete=False) as f:
        path = Path(f.name)

    try:
        metadata = BufferedMetadataProvider()
        metadata.set("J2K_LOSSLESS", str(lossless).lower())

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
            description="Property test",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image:0")
        decoded = read_full_image(asset, num_bands, num_rows, num_cols)
        reader.close()

        return decoded
    finally:
        if path.exists():
            path.unlink()


def write_and_read_jpeg(
    array: np.ndarray,
    pixel_type,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    quality: int = 75,
) -> np.ndarray:
    """Write a JPEG file and read back the decoded image.

    Handles temp file lifecycle, provider setup, IO.open for write and read,
    and full-image reassembly.

    Args:
        array: Source image in BSQ layout (bands, rows, cols).
        pixel_type: PixelType enum value (must be UInt8).
        num_bands: Number of bands (1 or 3).
        num_rows: Number of rows.
        num_cols: Number of columns.
        quality: JPEG quality parameter 1-100 (default 75).

    Returns:
        Decoded image array in BSQ format (bands, rows, cols).
    """
    with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as f:
        path = Path(f.name)

    try:
        metadata = BufferedMetadataProvider()
        metadata.set("JPEG_QUALITY", str(quality))

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

        writer = IO.open([str(path)], "w", "jpeg")
        writer.metadata = metadata
        writer.add_asset(
            key="image:0",
            provider=provider,
            title="Test Image",
            description="Property test",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image:0")
        decoded = read_full_image(asset, num_bands, num_rows, num_cols)
        reader.close()

        return decoded
    finally:
        if path.exists():
            path.unlink()


def write_and_read_png(
    array: np.ndarray,
    pixel_type,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    metadata_hints: dict | None = None,
) -> np.ndarray:
    """Write a PNG file and read back the decoded image.

    Handles temp file lifecycle, provider setup, IO.open for write and read,
    and full-image reassembly.

    Args:
        array: Source image in BSQ layout (bands, rows, cols).
        pixel_type: PixelType enum value.
        num_bands: Number of bands.
        num_rows: Number of rows.
        num_cols: Number of columns.
        metadata_hints: Optional dict of metadata key/value pairs for tEXt chunks.

    Returns:
        Decoded image array in BSQ format (bands, rows, cols).
    """
    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
        path = Path(f.name)

    try:
        metadata = BufferedMetadataProvider()
        if metadata_hints:
            for k, v in metadata_hints.items():
                if isinstance(v, str):
                    metadata.set(k, v)
                else:
                    metadata.set_json(k, v)

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

        writer = IO.open([str(path)], "w", "png")
        writer.metadata = metadata
        writer.add_asset(
            key="image:0",
            provider=provider,
            title="Test Image",
            description="Property test",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        asset = reader.get_asset("image:0")
        decoded = read_full_image(asset, num_bands, num_rows, num_cols)
        reader.close()

        return decoded
    finally:
        if path.exists():
            path.unlink()


# ---------------------------------------------------------------------------
# Multi-file R-set and COG helpers
# ---------------------------------------------------------------------------


def write_and_read_rset(
    base_array: np.ndarray,
    base_config: dict,
    overviews: list,
) -> dict:
    """Write multi-file NITF R-set and read back.

    Creates a temporary directory, writes the base image and overview images
    as separate NITF files via ``IO.open()`` with multiple paths, reads them
    back via multi-path ``IO.open()``, and returns decoded arrays keyed by
    asset key.

    Args:
        base_array: numpy array (bands, rows, cols) for base image.
        base_config: dict with num_columns, num_rows, num_bands,
            block_width, block_height, pixel_type.
        overviews: list of (level, ovr_array, ovr_config) tuples where
            each ovr_config has the same keys as base_config.

    Returns:
        dict mapping asset keys (e.g. ``"image:0"``,
        ``"image:0:overview:1"``) to decoded numpy arrays.
    """
    tmp_dir = tempfile.mkdtemp()
    try:
        base_path = str(Path(tmp_dir) / "base.ntf")
        rset_paths = [
            str(Path(tmp_dir) / f"base.ntf.r{level}")
            for level, _, _ in overviews
        ]
        all_paths = [base_path] + rset_paths

        # -- Write --
        writer = IO.open(all_paths, "w", "nitf")

        base_provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=base_config["num_columns"],
            num_rows=base_config["num_rows"],
            num_bands=base_config["num_bands"],
            block_width=base_config["block_width"],
            block_height=base_config["block_height"],
            pixel_type=base_config["pixel_type"],
        )
        base_provider.set_full_image(base_array)
        writer.add_asset("image:0", base_provider, "Base", "base image", ["data"])

        for level, ovr_array, ovr_config in overviews:
            ovr_key = f"image:0:overview:{level}"
            ovr_provider = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=ovr_config["num_columns"],
                num_rows=ovr_config["num_rows"],
                num_bands=ovr_config["num_bands"],
                block_width=ovr_config["block_width"],
                block_height=ovr_config["block_height"],
                pixel_type=ovr_config["pixel_type"],
            )
            ovr_provider.set_full_image(ovr_array)
            writer.add_asset(ovr_key, ovr_provider, f"Overview {level}", "overview", ["overview"])

        writer.close()

        # -- Read --
        reader = IO.open(all_paths, "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)

        result = {}
        for key in image_keys:
            asset = reader.get_asset(key)
            decoded = read_full_image(
                asset, asset.num_bands, asset.num_rows, asset.num_columns
            )
            result[key] = decoded

        reader.close()
        return result
    finally:
        shutil.rmtree(tmp_dir, ignore_errors=True)


def write_and_read_cog(
    base_array: np.ndarray,
    base_config: dict,
    overviews: list,
) -> dict:
    """Write single COG TIFF with overviews and read back.

    Creates a temporary directory, writes the base image and overview images
    into a single TIFF file via ``IO.open()``, reads it back, and returns
    decoded arrays with their roles keyed by asset key.

    Args:
        base_array: numpy array (bands, rows, cols) for base image.
        base_config: dict with num_columns, num_rows, num_bands,
            block_width, block_height, pixel_type.
        overviews: list of (level, ovr_array, ovr_config) tuples where
            each ovr_config has the same keys as base_config.

    Returns:
        dict mapping asset keys to ``(decoded_array, roles)`` tuples where
        roles is a list of strings (e.g. ``["data"]`` or ``["overview"]``).
    """
    tmp_dir = tempfile.mkdtemp()
    try:
        tiff_path = str(Path(tmp_dir) / "output.tif")

        # -- Write --
        writer = IO.open([tiff_path], "w", "tiff")

        base_provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=base_config["num_columns"],
            num_rows=base_config["num_rows"],
            num_bands=base_config["num_bands"],
            block_width=base_config["block_width"],
            block_height=base_config["block_height"],
            pixel_type=base_config["pixel_type"],
        )
        base_provider.set_full_image(base_array)
        writer.add_asset("image:0", base_provider, "Base", "base image", ["data"])

        for level, ovr_array, ovr_config in overviews:
            ovr_key = f"image:0:overview:{level}"
            ovr_provider = BufferedImageAssetProvider.create(
                key=ovr_key,
                num_columns=ovr_config["num_columns"],
                num_rows=ovr_config["num_rows"],
                num_bands=ovr_config["num_bands"],
                block_width=ovr_config["block_width"],
                block_height=ovr_config["block_height"],
                pixel_type=ovr_config["pixel_type"],
            )
            ovr_provider.set_full_image(ovr_array)
            writer.add_asset(ovr_key, ovr_provider, f"Overview {level}", "overview", ["overview"])

        writer.close()

        # -- Read --
        reader = IO.open([tiff_path], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)

        result = {}
        for key in image_keys:
            asset = reader.get_asset(key)
            decoded = read_full_image(
                asset, asset.num_bands, asset.num_rows, asset.num_columns
            )
            result[key] = (decoded, list(asset.roles))

        reader.close()
        return result
    finally:
        shutil.rmtree(tmp_dir, ignore_errors=True)
