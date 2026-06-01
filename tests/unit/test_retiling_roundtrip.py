"""Round-trip tests for retiling across grid mismatches.

This module tests the full write pipeline when the source provider's block
grid differs from the output format's tile grid:

1. BufferedImageAssetProvider.from_provider(block_width=X) → write TIFF/J2K/NITF
   → read back → pixel-exact match.
2. Raw provider with mismatched metadata tile sizes → write → read back → correct.

All tests use deterministic gradient/checkerboard patterns in temp directories.
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


def _gradient_array(pixel_type, num_bands, num_rows, num_cols):
    """Create a deterministic gradient pattern for pixel-exact verification."""
    dtype = np.dtype(pixel_type.to_numpy_dtype())
    total = num_bands * num_rows * num_cols
    if np.issubdtype(dtype, np.integer):
        max_val = np.iinfo(dtype).max
        arr = (np.arange(total, dtype=np.int64) % (max_val + 1)).astype(dtype)
    else:
        arr = np.linspace(0, 1, total, dtype=dtype)
    return arr.reshape(num_bands, num_rows, num_cols)


def _checkerboard_array(pixel_type, num_bands, num_rows, num_cols, block_size=8):
    """Create a checkerboard pattern for visual coverage verification."""
    dtype = np.dtype(pixel_type.to_numpy_dtype())
    if np.issubdtype(dtype, np.integer):
        high = min(np.iinfo(dtype).max, 255)
    else:
        high = 1.0

    single_band = np.zeros((num_rows, num_cols), dtype=dtype)
    for r in range(num_rows):
        for c in range(num_cols):
            if ((r // block_size) + (c // block_size)) % 2 == 0:
                single_band[r, c] = dtype.type(high)

    return np.stack([single_band] * num_bands, axis=0)


def _read_back_full(path, num_bands, num_rows, num_cols):
    """Read a written file and reassemble the full image."""
    reader = IO.open([str(path)], "r")
    asset = reader.get_asset("image:0")
    grid_rows, grid_cols = asset.block_grid_size
    _, block_h, block_w = asset.block_shape

    dtype = np.dtype(asset.pixel_value_type.to_numpy_dtype())
    result = np.zeros((num_bands, num_rows, num_cols), dtype=dtype)

    for br in range(grid_rows):
        for bc in range(grid_cols):
            block = asset.get_block(br, bc, 0)
            sr = br * block_h
            sc = bc * block_w
            er = min(sr + block.shape[1], num_rows)
            ec = min(sc + block.shape[2], num_cols)
            result[:, sr:er, sc:ec] = block[:, : er - sr, : ec - sc]

    reader.close()
    return result


# =============================================================================
# Tests: from_provider with different block sizes → write → read back
# =============================================================================


class TestFromProviderRetiling:
    """BufferedImageAssetProvider.from_provider() with different block_width/
    block_height produces correct output when written to various formats.
    """

    @pytest.mark.parametrize(
        "src_block,dst_block",
        [
            (32, 64),   # small source → larger output blocks
            (64, 32),   # large source → smaller output blocks
            (48, 64),   # non-power-of-2 source → power-of-2 output
            (128, 32),  # large source → much smaller output blocks
        ],
    )
    def test_tiff_from_provider_lossless(self, src_block, dst_block):
        """from_provider(block_width=X) → TIFF write → pixel-exact read-back."""
        num_rows, num_cols, num_bands = 100, 120, 2
        pixel_type = PixelType.UInt16
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        # Create source with src_block grid
        src_provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
            pixel_type=pixel_type,
        )
        src_provider.set_full_image(array)

        # Wrap with different block size
        rebuffered = BufferedImageAssetProvider.from_provider(
            src_provider,
            block_width=min(dst_block, num_cols),
            block_height=min(dst_block, num_rows),
        )

        # TIFF output tile matches the rebuffered block size
        tile_w = min(dst_block, num_cols)
        tile_h = min(dst_block, num_rows)
        # Round up to multiple of 16 for TIFF
        tile_w = ((tile_w + 15) // 16) * 16
        tile_h = ((tile_h + 15) // 16) * 16

        metadata = BufferedMetadataProvider()
        metadata["322"] = str(tile_w)
        metadata["323"] = str(tile_h)
        metadata["259"] = 8  # Deflate
        metadata["284"] = 1

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "tiff")
            writer.metadata = metadata
            writer.add_asset("image:0", rebuffered, "Test", "test", ["data"])
            writer.close()

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    @pytest.mark.parametrize("src_block", [32, 48, 64])
    def test_j2k_from_provider_lossless(self, src_block):
        """from_provider(block_width=X) → J2K lossless write → pixel-exact."""
        num_rows, num_cols, num_bands = 80, 96, 1
        pixel_type = PixelType.UInt8
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        src_provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
            pixel_type=pixel_type,
        )
        src_provider.set_full_image(array)

        # from_provider with different block size
        rebuffered = BufferedImageAssetProvider.from_provider(
            src_provider,
            block_width=64,
            block_height=64,
        )

        metadata = BufferedMetadataProvider()
        metadata["J2K_LOSSLESS"] = True
        metadata["J2K_TILE_WIDTH"] = 64
        metadata["J2K_TILE_HEIGHT"] = 64

        with tempfile.NamedTemporaryFile(suffix=".j2k", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "j2k")
            writer.metadata = metadata
            writer.add_asset("image:0", rebuffered, "Test", "test", ["data"])
            writer.close()

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    @pytest.mark.parametrize("src_block", [32, 64, 128])
    def test_nitf_from_provider_lossless(self, src_block):
        """from_provider(block_width=X) → NITF uncompressed write → pixel-exact."""
        num_rows, num_cols, num_bands = 100, 100, 3
        pixel_type = PixelType.UInt8
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        src_provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
            pixel_type=pixel_type,
        )
        src_provider.set_full_image(array)

        # Rebuffer to 64×64 blocks (NITF typical)
        rebuffered = BufferedImageAssetProvider.from_provider(
            src_provider,
            block_width=64,
            block_height=64,
        )

        metadata = BufferedMetadataProvider()
        metadata["IC"] = "NC"

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset("image:0", rebuffered, "Test", "test", ["data"])
            writer.close()

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    @pytest.mark.parametrize("src_block", [16, 32, 64])
    def test_png_from_multiblock_provider(self, src_block):
        """from_provider() with multi-block source → PNG write → pixel-exact."""
        num_rows, num_cols, num_bands = 50, 70, 3
        pixel_type = PixelType.UInt8
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        src_provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
            pixel_type=pixel_type,
        )
        src_provider.set_full_image(array)

        # PNG is always single-tile output, but source may be multi-block
        rebuffered = BufferedImageAssetProvider.from_provider(
            src_provider,
            block_width=num_cols,
            block_height=num_rows,
        )

        metadata = BufferedMetadataProvider()

        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "png")
            writer.metadata = metadata
            writer.add_asset("image:0", rebuffered, "Test", "test", ["data"])
            writer.close()

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Tests: raw provider with mismatched metadata tile sizes → write → read back
# =============================================================================


class TestRawProviderMismatchedTiles:
    """When a provider's block grid doesn't match the output tile size specified
    in metadata/encoding hints, the writer must still produce correct output.
    """

    @pytest.mark.parametrize(
        "src_block,out_tile",
        [
            (32, 64),   # Source tiles smaller than output
            (64, 32),   # Source tiles larger than output
            (128, 64),  # 2:1 ratio
            (48, 64),   # Non-aligned source
        ],
    )
    def test_tiff_mismatched_metadata_tiles(self, src_block, out_tile):
        """Provider block size != TIFF tile metadata → correct pixel output."""
        num_rows, num_cols, num_bands = 100, 120, 2
        pixel_type = PixelType.UInt16
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        # Round output tile to TIFF-required multiple of 16
        out_tile_aligned = ((out_tile + 15) // 16) * 16

        metadata = BufferedMetadataProvider()
        metadata["322"] = str(out_tile_aligned)  # TileWidth
        metadata["323"] = str(out_tile_aligned)  # TileLength
        metadata["259"] = 8                      # Deflate
        metadata["284"] = 1                      # Chunky

        # Source block intentionally differs from output tile
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
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

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    @pytest.mark.parametrize(
        "src_block,j2k_tile",
        [
            (32, 64),
            (128, 64),
            (64, 32),
        ],
    )
    def test_j2k_mismatched_metadata_tiles(self, src_block, j2k_tile):
        """Provider block size != J2K tile metadata → correct lossless output."""
        num_rows, num_cols, num_bands = 80, 96, 2
        pixel_type = PixelType.UInt8
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        metadata = BufferedMetadataProvider()
        metadata["J2K_LOSSLESS"] = True
        metadata["J2K_TILE_WIDTH"] = j2k_tile
        metadata["J2K_TILE_HEIGHT"] = j2k_tile

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
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

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    def test_nitf_source_blocks_differ_from_nppbh(self):
        """NITF writer handles source provider blocks != NPPBH/NPPBV."""
        num_rows, num_cols, num_bands = 128, 128, 1
        pixel_type = PixelType.UInt16
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        metadata = BufferedMetadataProvider()
        metadata["IC"] = "NC"

        # Source uses 32×32 blocks but NITF will want its own blocking
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=32,
            block_height=32,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset("image:0", provider, "Test", "test", ["data"])
            writer.close()

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Tests: edge cases — non-evenly-divisible dimensions
# =============================================================================


class TestEdgeCaseRetiling:
    """Edge cases: image dimensions not evenly divisible by source or output
    tile sizes, ensuring partial tiles are handled correctly.
    """

    @pytest.mark.parametrize(
        "num_rows,num_cols,src_block,out_tile",
        [
            (100, 100, 64, 64),   # Partial edge tiles in both directions
            (33, 47, 32, 64),     # Odd dimensions, small→large retile
            (65, 65, 128, 32),    # Large source, small output
            (17, 17, 16, 32),     # Just barely multi-block
            (255, 255, 64, 128),  # Large image, mismatched
        ],
    )
    def test_partial_edge_tiles_tiff(self, num_rows, num_cols, src_block, out_tile):
        """Non-evenly-divisible dims → TIFF retiling still pixel-exact."""
        num_bands = 1
        pixel_type = PixelType.UInt16
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        out_tile_aligned = ((out_tile + 15) // 16) * 16

        metadata = BufferedMetadataProvider()
        metadata["322"] = str(out_tile_aligned)
        metadata["323"] = str(out_tile_aligned)
        metadata["259"] = 1  # Uncompressed for speed
        metadata["284"] = 1

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(src_block, num_cols),
            block_height=min(src_block, num_rows),
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

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    def test_checkerboard_pattern_survives_retiling(self):
        """Checkerboard pattern verifies no pixel transposition during retiling."""
        num_rows, num_cols, num_bands = 100, 100, 3
        pixel_type = PixelType.UInt8
        array = _checkerboard_array(pixel_type, num_bands, num_rows, num_cols)

        metadata = BufferedMetadataProvider()
        metadata["322"] = "64"
        metadata["323"] = "64"
        metadata["259"] = 8   # Deflate
        metadata["284"] = 1

        # Source uses 32×32 blocks, output uses 64×64 tiles
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=32,
            block_height=32,
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

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)

    @pytest.mark.parametrize("pixel_type", [
        PixelType.UInt8,
        PixelType.UInt16,
        PixelType.Int16,
        PixelType.Float32,
    ])
    def test_all_pixel_types_retiling(self, pixel_type):
        """Retiling preserves data for all supported pixel types."""
        num_rows, num_cols, num_bands = 80, 80, 2
        array = _gradient_array(pixel_type, num_bands, num_rows, num_cols)

        metadata = BufferedMetadataProvider()
        metadata["322"] = "64"
        metadata["323"] = "64"
        metadata["259"] = 8   # Deflate
        metadata["284"] = 1

        # Source 32×32, output 64×64
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=32,
            block_height=32,
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

            decoded = _read_back_full(path, num_bands, num_rows, num_cols)
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)
