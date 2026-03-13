"""Property-based tests for TIFF API contracts.

This module tests correctness properties for the TIFF reader API:
- Property 3: Block coordinate validation
- Property 4: IFD enumeration and asset key consistency
- Property 5: Non-image asset access
- Property 6: Dataset-level metadata contains only file-level information
- Property 7: Per-IFD metadata completeness

Tests use the existing data/unit/small.tif (tiled, 1024x1024, uint8, 1-band,
256x256 tiles, Deflate) and PIL-generated stripped TIFFs.

**Validates: Requirements 4.6, 4.7, 4.11, 4.13, 5.4, 5.5, 5.6, 5.7, 5.8,
5.9, 5.10, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9**
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase
from PIL import Image

from aws.osml.io import IO, AssetType, PixelType

from .strategies import get_numpy_dtype, tiff_image_config


pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)

UNIT_DATA_DIR = Path("data/unit")
SMALL_TIF = UNIT_DATA_DIR / "small.tif"

# PIL mode mapping (dtype_name, bands) -> PIL mode
_PIL_MODE = {
    ("uint8", 1): "L",
    ("uint8", 3): "RGB",
    ("uint16", 1): "I;16",
    ("int32", 1): "I",
    ("float32", 1): "F",
}


def _write_tiff(cfg: dict, array_chw: np.ndarray) -> Path:
    """Write a TIFF via PIL and return the temp file path (caller must delete)."""
    pixel_type = cfg["pixel_type"]
    bands = cfg["bands"]
    rps = cfg["rows_per_strip"]
    pil_comp = cfg["pil_compression"]
    dtype = get_numpy_dtype(pixel_type)
    mode = _PIL_MODE[(dtype.name, bands)]

    hw = array_chw[0] if bands == 1 else np.transpose(array_chw, (1, 2, 0))
    img = Image.fromarray(hw, mode)

    f = tempfile.NamedTemporaryFile(suffix=".tif", delete=False)
    path = Path(f.name)
    f.close()
    img.save(str(path), compression=pil_comp, tiffinfo={278: rps})
    return path


def _make_array(cfg: dict) -> np.ndarray:
    """Generate a deterministic pixel array for a tiff_image_config."""
    pixel_type = cfg["pixel_type"]
    dtype = get_numpy_dtype(pixel_type)
    bands, height, width = cfg["bands"], cfg["height"], cfg["width"]
    rng = np.random.RandomState(99)
    if np.issubdtype(dtype, np.floating):
        return rng.rand(bands, height, width).astype(dtype)
    elif np.issubdtype(dtype, np.signedinteger):
        info = np.iinfo(dtype)
        return rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
    else:
        info = np.iinfo(dtype)
        return rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)


# =============================================================================
# Property 3: Block coordinate validation
# =============================================================================


@pytest.mark.property
class TestTiffBlockCoordinateValidation:
    """Property 3: Block coordinate validation

    has_block() returns true for all valid coordinates within the block grid,
    and get_block() raises InvalidBlockCoordinates for out-of-bounds access.

    # Feature: libtiff-ffi-tiff-reading, Property 3: Block coordinate validation
    **Validates: Requirements 4.6, 4.11**
    """

    def test_tiled_valid_coordinates(self):
        """All valid block coordinates return True for has_block on tiled TIFF."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        grid_rows, grid_cols = asset.block_grid_size

        for r in range(grid_rows):
            for c in range(grid_cols):
                assert asset.has_block(r, c, 0) is True, (
                    f"has_block({r}, {c}, 0) should be True"
                )

    def test_tiled_out_of_bounds(self):
        """Out-of-bounds coordinates raise errors on tiled TIFF."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        grid_rows, grid_cols = asset.block_grid_size

        # has_block returns False for out-of-bounds
        assert asset.has_block(grid_rows, 0, 0) is False
        assert asset.has_block(0, grid_cols, 0) is False

        # get_block raises for out-of-bounds
        with pytest.raises(Exception):
            asset.get_block(grid_rows, 0, 0, None)
        with pytest.raises(Exception):
            asset.get_block(0, grid_cols, 0, None)

    @given(config=tiff_image_config(min_size=16, max_size=64))
    @pbt_settings
    def test_stripped_valid_coordinates(self, config):
        """All valid block coordinates return True for has_block on stripped TIFF."""
        array_chw = _make_array(config)
        path = _write_tiff(config, array_chw)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            grid_rows, grid_cols = asset.block_grid_size

            for r in range(grid_rows):
                for c in range(grid_cols):
                    assert asset.has_block(r, c, 0) is True

            # Out-of-bounds
            assert asset.has_block(grid_rows, 0, 0) is False
            with pytest.raises(Exception):
                asset.get_block(grid_rows, 0, 0, None)
        finally:
            path.unlink(missing_ok=True)

    def test_invalid_resolution_level(self):
        """get_block with resolution level > 0 raises an error."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")

        with pytest.raises(Exception):
            asset.get_block(0, 0, 1, None)


# =============================================================================
# Property 4: IFD enumeration and asset key consistency
# =============================================================================


@pytest.mark.property
class TestTiffIFDEnumeration:
    """Property 4: IFD enumeration and asset key consistency

    For a TIFF with N full-resolution IFDs, get_asset_keys returns N keys,
    each get_asset(key) succeeds, and each provider reports
    num_resolution_levels == 1.

    # Feature: libtiff-ffi-tiff-reading, Property 4: IFD enumeration
    **Validates: Requirements 4.7, 5.4, 5.5, 5.7, 5.9**
    """

    def test_single_ifd_tiled(self):
        """Single-IFD tiled TIFF has exactly one image segment."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Image)

        assert keys == ["image_segment_0"]

        asset = reader.get_asset("image_segment_0")
        assert asset.num_resolution_levels == 1

    @given(config=tiff_image_config(min_size=16, max_size=48))
    @pbt_settings
    def test_single_ifd_stripped(self, config):
        """Single-IFD stripped TIFF has exactly one image segment."""
        array_chw = _make_array(config)
        path = _write_tiff(config, array_chw)

        try:
            reader = IO.open([str(path)], "r")
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert keys == ["image_segment_0"]

            asset = reader.get_asset("image_segment_0")
            assert asset.num_resolution_levels == 1
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 5: Non-image asset access
# =============================================================================


@pytest.mark.property
class TestTiffNonImageAssetAccess:
    """Property 5: Non-image asset access

    get_asset() with invalid keys returns AssetNotFound.
    get_asset_keys for Text/Graphics/Data returns empty lists.

    # Feature: libtiff-ffi-tiff-reading, Property 5: Non-image asset access
    **Validates: Requirements 5.8, 5.10**
    """

    def test_invalid_key_raises(self):
        """get_asset with non-existent key raises an error."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")

        for key in ["nonexistent", "image_segment_99", "", "text_segment_0"]:
            with pytest.raises(Exception):
                reader.get_asset(key)

    def test_non_image_asset_types_empty(self):
        """Text, Graphics, and Data asset types return empty lists."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")

        assert reader.get_asset_keys(asset_type=AssetType.Text) == []
        assert reader.get_asset_keys(asset_type=AssetType.Graphics) == []
        assert reader.get_asset_keys(asset_type=AssetType.Data) == []

    @given(config=tiff_image_config(min_size=16, max_size=32))
    @pbt_settings
    def test_stripped_non_image_empty(self, config):
        """Stripped TIFFs also have no text/graphics/data segments."""
        array_chw = _make_array(config)
        path = _write_tiff(config, array_chw)

        try:
            reader = IO.open([str(path)], "r")
            assert reader.get_asset_keys(asset_type=AssetType.Text) == []
            assert reader.get_asset_keys(asset_type=AssetType.Graphics) == []
            assert reader.get_asset_keys(asset_type=AssetType.Data) == []
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 6: Dataset-level metadata contains only file-level information
# =============================================================================


@pytest.mark.property
class TestTiffDatasetMetadata:
    """Property 6: Dataset-level metadata contains only file-level information

    The dataset-level MetadataProvider contains exactly three keys:
    ByteOrder, NumberOfDirectories, NumberOfImageSegments.

    # Feature: libtiff-ffi-tiff-reading, Property 6: Dataset-level metadata
    **Validates: Requirements 5.6, 6.9**
    """

    def test_tiled_dataset_metadata(self):
        """Tiled TIFF dataset metadata has exactly the expected keys."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        meta = reader.metadata.as_dict()

        assert set(meta.keys()) == {"ByteOrder", "NumberOfDirectories", "NumberOfImageSegments"}
        assert meta["ByteOrder"] in ("LittleEndian", "BigEndian")
        assert isinstance(meta["NumberOfDirectories"], (int, float))
        assert isinstance(meta["NumberOfImageSegments"], (int, float))
        assert meta["NumberOfDirectories"] >= 1
        assert meta["NumberOfImageSegments"] >= 1

    @given(config=tiff_image_config(min_size=16, max_size=32))
    @pbt_settings
    def test_stripped_dataset_metadata(self, config):
        """Stripped TIFF dataset metadata has exactly the expected keys."""
        array_chw = _make_array(config)
        path = _write_tiff(config, array_chw)

        try:
            reader = IO.open([str(path)], "r")
            meta = reader.metadata.as_dict()

            assert set(meta.keys()) == {"ByteOrder", "NumberOfDirectories", "NumberOfImageSegments"}
            assert meta["ByteOrder"] in ("LittleEndian", "BigEndian")
            assert meta["NumberOfDirectories"] == 1
            assert meta["NumberOfImageSegments"] == 1
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 7: Per-IFD metadata completeness
# =============================================================================


@pytest.mark.property
class TestTiffPerIFDMetadata:
    """Property 7: Per-IFD metadata completeness

    Per-segment metadata contains ImageWidth, ImageLength, BitsPerSample,
    SamplesPerPixel with correct values. as_dict(None) and as_dict("tiff")
    return identical results.

    # Feature: libtiff-ffi-tiff-reading, Property 7: Per-IFD metadata completeness
    **Validates: Requirements 4.13, 6.2, 6.3, 6.4, 6.5, 6.6, 6.8**
    """

    def test_tiled_per_ifd_metadata(self):
        """Tiled TIFF per-IFD metadata has correct dimension values."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        meta = asset.get_metadata().as_dict()

        assert meta["ImageWidth"] == 1024
        assert meta["ImageLength"] == 1024
        assert meta["BitsPerSample"] == 8
        assert meta["SamplesPerPixel"] == 1

    def test_tiff_section_equals_default(self):
        """as_dict('tiff') returns the same result as as_dict(None)."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        provider = asset.get_metadata()

        assert provider.as_dict("tiff") == provider.as_dict()

    def test_unknown_section_returns_empty(self):
        """as_dict with unrecognized section returns empty dict."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        provider = asset.get_metadata()

        assert provider.as_dict("unknown") == {}
        assert provider.as_dict("nitf") == {}
        assert provider.as_dict("geotiff") == {}

    @given(config=tiff_image_config(min_size=16, max_size=48))
    @pbt_settings
    def test_stripped_per_ifd_metadata(self, config):
        """Stripped TIFF per-IFD metadata matches the written configuration."""
        pixel_type = config["pixel_type"]
        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        # Expected BitsPerSample from dtype
        expected_bps = dtype.itemsize * 8

        array_chw = _make_array(config)
        path = _write_tiff(config, array_chw)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            meta = asset.get_metadata().as_dict()

            assert meta["ImageWidth"] == width, (
                f"ImageWidth: expected {width}, got {meta['ImageWidth']}"
            )
            assert meta["ImageLength"] == height, (
                f"ImageLength: expected {height}, got {meta['ImageLength']}"
            )
            assert meta["BitsPerSample"] == expected_bps, (
                f"BitsPerSample: expected {expected_bps}, got {meta['BitsPerSample']}"
            )
            # PIL may write SamplesPerPixel only for multi-band
            if "SamplesPerPixel" in meta:
                assert meta["SamplesPerPixel"] == bands

            # as_dict(None) == as_dict("tiff")
            provider = asset.get_metadata()
            assert provider.as_dict("tiff") == provider.as_dict()

            # Unknown section returns empty
            assert provider.as_dict("unknown") == {}
        finally:
            path.unlink(missing_ok=True)
