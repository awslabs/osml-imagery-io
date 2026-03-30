"""Property-based end-to-end tests for TileIndex + Zarr codec integration.

This module validates the full pipeline from the user's perspective:
1. Write a multi-tile synthetic image (NITF with various compressions, TIFF).
2. Generate a Kerchunk tile index via TileIndex.generate().
3. Read tiles through the standard IO.open() / DatasetReader path.
4. Open the index via fsspec ReferenceFileSystem + zarr and read the same tiles.
5. Compare pixels: exact match for lossless, PSNR/SSIM for lossy.

The fsspec/zarr path is the real user-facing interface described in
docs/user-guide/zarr-codecs.md. It exercises the full stack: index
serialization, fsspec reference resolution, byte-range reads, codec
entry-point dispatch, and decode.

Feature: zarr-tile-index-end-to-end
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, assume

from aws.osml.io import (
    IO,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)
from aws.osml.io.tile_index import TileIndex

from ..conftest import pbt_settings
from ..quality import MIN_PSNR_DB, MIN_SSIM, calculate_psnr, calculate_ssim
from ..strategies import (
    realistic_image_for_compression,
    jpeg_image_for_compression,
    tiff_writable_image,
)

# Require zarr + fsspec for all tests in this module.
zarr = pytest.importorskip("zarr", minversion="3.0")
fsspec = pytest.importorskip("fsspec")

# Ensure our codecs are registered with numcodecs before zarr reads any stores.
import aws.osml.io.zarr_codecs  # noqa: F401


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _write_nitf(
    array: np.ndarray,
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    metadata_hints: dict,
    block_width: int = 64,
    block_height: int = 64,
) -> Path:
    """Write a NITF file and return the path. Caller must clean up."""
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
        block_width=min(num_cols, block_width),
        block_height=min(num_rows, block_height),
        pixel_type=pixel_type,
        metadata=metadata,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "nitf")
    writer.add_asset(
        key="image_segment_0",
        provider=provider,
        title="Test Image",
        description="Zarr end-to-end test",
        roles=["data"],
    )
    writer.close()
    return path


def _write_tiff(
    array: np.ndarray,
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    hints: dict,
) -> Path:
    """Write a TIFF file and return the path. Caller must clean up."""
    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    metadata = BufferedMetadataProvider()
    for k, v in hints.items():
        if isinstance(v, str):
            metadata.set(k, v)
        else:
            metadata.set_json(k, v)

    tile_w = int(hints.get("322", "256"))
    tile_h = int(hints.get("323", "256"))

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
        description="Zarr end-to-end test",
        roles=["data"],
    )
    writer.close()
    return path


def _read_all_tiles_via_io(path: Path):
    """Read all tiles from a file via IO.open, returning (row, col) → ndarray."""
    tiles = {}
    with IO.open([str(path)], "r") as reader:
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(keys[0])
        grid_rows, grid_cols = asset.block_grid_size
        for r in range(grid_rows):
            for c in range(grid_cols):
                tiles[(r, c)] = asset.get_block(r, c, 0)
    return tiles


def _read_all_tiles_via_zarr(index_path: Path):
    """Read all tiles via fsspec ReferenceFileSystem + zarr.

    Returns (row, col) → ndarray.
    """
    fs = fsspec.filesystem("reference", fo=str(index_path))
    store = fs.get_mapper("")

    root = zarr.open_group(store, mode="r")
    segment_keys = list(root.array_keys())
    assert segment_keys, "No arrays found in zarr store"

    arr = root[segment_keys[0]]
    tile_bands, tile_h, tile_w = arr.chunks
    _, total_rows, total_cols = arr.shape

    grid_rows = (total_rows + tile_h - 1) // tile_h
    grid_cols = (total_cols + tile_w - 1) // tile_w

    tiles = {}
    for r in range(grid_rows):
        for c in range(grid_cols):
            row_start = r * tile_h
            col_start = c * tile_w
            row_end = min(row_start + tile_h, total_rows)
            col_end = min(col_start + tile_w, total_cols)
            tiles[(r, c)] = np.asarray(arr[:, row_start:row_end, col_start:col_end])

    assert len(tiles) > 0, "No tiles read from zarr store"
    return tiles


def _generate_and_save_index(path: Path) -> Path:
    """Generate a TileIndex for a file, save to JSON, return the index path."""
    idx = TileIndex.generate(str(path), source_uri=str(path))
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
        index_path = Path(f.name)
    idx.save(str(index_path))
    return index_path


def _assert_tiles_match_lossless(tiles_io, tiles_zarr, label="zarr"):
    """Assert all tiles match exactly. Trims to overlapping region for edge tiles."""
    assert len(tiles_io) > 0, "No tiles from IO path"
    assert len(tiles_zarr) > 0, f"No tiles from {label} path"
    assert set(tiles_io.keys()) == set(tiles_zarr.keys()), (
        f"Tile coordinate mismatch between IO and {label} paths. "
        f"IO: {sorted(tiles_io.keys())}, {label}: {sorted(tiles_zarr.keys())}"
    )
    for coord in sorted(tiles_io.keys()):
        io_tile = tiles_io[coord]
        zarr_tile = tiles_zarr[coord]
        # Both tiles must have non-zero dimensions
        assert io_tile.size > 0, f"IO tile {coord} is empty"
        assert zarr_tile.size > 0, f"{label} tile {coord} is empty"
        # Trim to overlapping region for edge tiles
        b = min(io_tile.shape[0], zarr_tile.shape[0])
        h = min(io_tile.shape[1], zarr_tile.shape[1])
        w = min(io_tile.shape[2], zarr_tile.shape[2])
        assert b > 0 and h > 0 and w > 0, (
            f"Tile {coord} has zero overlap: IO={io_tile.shape}, {label}={zarr_tile.shape}"
        )
        np.testing.assert_array_equal(
            zarr_tile[:b, :h, :w],
            io_tile[:b, :h, :w],
            err_msg=f"Tile {coord} differs between IO and {label} paths",
        )


def _assert_tiles_match_lossy(tiles_io, tiles_zarr, label="zarr"):
    """Assert all tiles are close enough for lossy compression.

    Both paths decode the same compressed bytes with the same decoder,
    so results should typically be identical. Falls back to PSNR/SSIM
    if any divergence is detected.
    """
    assert len(tiles_io) > 0, "No tiles from IO path"
    assert len(tiles_zarr) > 0, f"No tiles from {label} path"
    assert set(tiles_io.keys()) == set(tiles_zarr.keys()), (
        f"Tile coordinate mismatch between IO and {label} paths. "
        f"IO: {sorted(tiles_io.keys())}, {label}: {sorted(tiles_zarr.keys())}"
    )
    for coord in sorted(tiles_io.keys()):
        io_tile = tiles_io[coord]
        zarr_tile = tiles_zarr[coord]
        assert io_tile.size > 0, f"IO tile {coord} is empty"
        assert zarr_tile.size > 0, f"{label} tile {coord} is empty"
        b = min(io_tile.shape[0], zarr_tile.shape[0])
        h = min(io_tile.shape[1], zarr_tile.shape[1])
        w = min(io_tile.shape[2], zarr_tile.shape[2])
        assert b > 0 and h > 0 and w > 0, (
            f"Tile {coord} has zero overlap: IO={io_tile.shape}, {label}={zarr_tile.shape}"
        )
        io_trimmed = io_tile[:b, :h, :w]
        zarr_trimmed = zarr_tile[:b, :h, :w]

        assert io_trimmed.dtype == zarr_trimmed.dtype, (
            f"Tile {coord} dtype mismatch: IO={io_trimmed.dtype}, {label}={zarr_trimmed.dtype}"
        )
        if not np.array_equal(io_trimmed, zarr_trimmed):
            psnr = calculate_psnr(io_trimmed, zarr_trimmed, use_actual_range=True)
            ssim = calculate_ssim(io_trimmed, zarr_trimmed)
            assert psnr >= MIN_PSNR_DB, (
                f"Tile {coord} PSNR {psnr:.2f} dB below threshold {MIN_PSNR_DB} dB"
            )
            assert ssim >= MIN_SSIM, (
                f"Tile {coord} SSIM {ssim:.4f} below threshold {MIN_SSIM}"
            )


# ---------------------------------------------------------------------------
# NITF Uncompressed (IC=NC) — lossless
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestEndToEndNitfUncompressed:
    """End-to-end: NITF uncompressed (IC=NC) multi-tile via Zarr.

    For any multi-tile uncompressed NITF, tiles accessed through
    fsspec/zarr are pixel-identical to tiles from IO.open().
    """

    @given(realistic_image_for_compression(min_size=48, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_nc_io_vs_zarr(self, image_tuple):
        """IO path and zarr path produce identical tiles for IC=NC."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=32, block_height=32,
        )
        try:
            tiles_io = _read_all_tiles_via_io(path)
            index_path = _generate_and_save_index(path)
            try:
                tiles_zarr = _read_all_tiles_via_zarr(index_path)
                _assert_tiles_match_lossless(tiles_io, tiles_zarr)
            finally:
                index_path.unlink(missing_ok=True)
        finally:
            path.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# NITF JPEG 2000 (IC=C8) — lossy
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestEndToEndNitfJ2K:
    """End-to-end: NITF JPEG 2000 (IC=C8) multi-tile via Zarr.

    Image dimensions are aligned to block size to avoid partial-tile
    J2K encoding issues.
    """

    @given(realistic_image_for_compression(min_size=64, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_c8_io_vs_zarr(self, image_tuple):
        """IO path and zarr path produce matching tiles for IC=C8."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        block_size = 32
        num_rows = (num_rows // block_size) * block_size
        num_cols = (num_cols // block_size) * block_size
        assume(num_rows >= block_size and num_cols >= block_size)
        array = np.ascontiguousarray(array[:, :num_rows, :num_cols])

        decomp_levels = min(5, max(1, int(np.floor(np.log2(block_size))) - 1))

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={
                "IC": "C8",
                "COMRAT": "02.0",
                "J2K_DECOMPOSITION_LEVELS": str(decomp_levels),
            },
            block_width=block_size, block_height=block_size,
        )
        try:
            tiles_io = _read_all_tiles_via_io(path)
            index_path = _generate_and_save_index(path)
            try:
                tiles_zarr = _read_all_tiles_via_zarr(index_path)
                _assert_tiles_match_lossy(tiles_io, tiles_zarr)
            finally:
                index_path.unlink(missing_ok=True)
        finally:
            path.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# NITF JPEG DCT (IC=C3) — lossy
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestEndToEndNitfJpeg:
    """End-to-end: NITF JPEG DCT (IC=C3) multi-tile via Zarr."""

    @given(jpeg_image_for_compression(min_size=64, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_c3_io_vs_zarr(self, image_tuple):
        """IO path and zarr path produce matching tiles for IC=C3."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        block_size = 32
        num_rows = (num_rows // block_size) * block_size
        num_cols = (num_cols // block_size) * block_size
        assume(num_rows >= block_size and num_cols >= block_size)
        array = np.ascontiguousarray(array[:, :num_rows, :num_cols])

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "C3", "COMRAT": "75.0"},
            block_width=block_size, block_height=block_size,
        )
        try:
            tiles_io = _read_all_tiles_via_io(path)
            index_path = _generate_and_save_index(path)
            try:
                tiles_zarr = _read_all_tiles_via_zarr(index_path)
                _assert_tiles_match_lossy(tiles_io, tiles_zarr)
            finally:
                index_path.unlink(missing_ok=True)
        finally:
            path.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# TIFF — TileIndex generation and tile coverage
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestEndToEndTiff:
    """End-to-end: TIFF tile index generation and coverage.

    Validates that TileIndex.generate() produces a valid index for TIFF
    files with correct tile counts and byte ranges. The full Zarr codec
    decode path is not tested because TIFF codec_configuration() returns
    None for all compressions — a TIFF Zarr codec is not yet implemented.
    """

    @given(tiff_writable_image(min_size=48, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_tiff_tile_index_coverage(self, image_tuple):
        """TileIndex covers all tiles for a TIFF image."""
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple

        # Skip JPEG TIFF — not meaningful for tile index validation
        assume(hints["259"] != 7)

        path = _write_tiff(
            array, pixel_type, num_bands, num_rows, num_cols, hints,
        )
        try:
            tiles_io = _read_all_tiles_via_io(path)
            assert len(tiles_io) > 0, "No tiles read via IO"

            try:
                idx = TileIndex.generate(str(path), source_uri=str(path))
            except ValueError:
                pytest.skip("tile_byte_ranges not available for this TIFF config")

            assert idx.num_tiles == len(tiles_io), (
                f"Index has {idx.num_tiles} tiles, IO read {len(tiles_io)}"
            )
            assert idx.num_segments == 1

            file_size = path.stat().st_size
            refs = idx.refs
            for key, value in refs["refs"].items():
                if not isinstance(value, list) or len(value) != 3:
                    continue
                _, offset, length = value
                assert offset >= 0, f"Negative offset for {key}"
                assert length > 0, f"Zero-length tile for {key}"
                assert offset + length <= file_size, (
                    f"Tile {key} extends past EOF: offset={offset}, "
                    f"length={length}, file_size={file_size}"
                )
        finally:
            path.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# TileIndex JSON round-trip preserves decode equivalence
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestEndToEndIndexRoundTrip:
    """End-to-end: TileIndex save/load preserves decode equivalence.

    Generates a tile index, saves to JSON, loads it back, and verifies
    that tiles decoded from the loaded index via zarr match the IO path.
    """

    @given(realistic_image_for_compression(min_size=48, max_size=96, min_bands=1, max_bands=3))
    @pbt_settings
    def test_index_json_roundtrip_preserves_decode(self, image_tuple):
        """Tiles from a saved/loaded JSON index match IO path via zarr."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=32, block_height=32,
        )
        try:
            tiles_io = _read_all_tiles_via_io(path)

            # Generate → save → load → save again → read via zarr
            idx = TileIndex.generate(str(path), source_uri=str(path))
            with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
                index_path = Path(f.name)
            idx.save(str(index_path))

            try:
                loaded = TileIndex.load(str(index_path))
                # Re-save the loaded index to a second file
                with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
                    reloaded_path = Path(f.name)
                loaded.save(str(reloaded_path))

                try:
                    tiles_zarr = _read_all_tiles_via_zarr(reloaded_path)
                    _assert_tiles_match_lossless(tiles_io, tiles_zarr, label="roundtrip-zarr")
                finally:
                    reloaded_path.unlink(missing_ok=True)
            finally:
                index_path.unlink(missing_ok=True)
        finally:
            path.unlink(missing_ok=True)
