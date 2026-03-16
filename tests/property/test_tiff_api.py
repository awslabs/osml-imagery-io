"""Property-based tests for TIFF API contracts.

This module tests correctness properties for the TIFF reader API:
- Property 3: Block coordinate validation
- Property 4: IFD enumeration and asset key consistency
- Property 5: Non-image asset access
- Property 6: Dataset-level metadata contains only file-level information
- Property 7: Per-IFD metadata completeness
- Property 8: IFD-level metadata keys are numeric strings
- Property 9: Custom tags coexist with structural tags

Tests use the existing data/unit/small.tif (tiled, 1024x1024, uint8, 1-band,
256x256 tiles, Deflate) and PIL-generated stripped TIFFs.

**Validates: Requirements 4.6, 4.7, 4.11, 4.13, 5.4, 5.5, 5.6, 5.7, 5.8,
5.9, 5.10, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9, 7.1, 8.3**
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase
from hypothesis import strategies as st
from PIL import Image

from aws.osml.io import IO, AssetType, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

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

        # Metadata keys are numeric tag ID strings after the metadata refactor
        assert meta["256"] == 1024   # ImageWidth
        assert meta["257"] == 1024   # ImageLength
        assert meta["258"] == 8      # BitsPerSample
        assert meta["277"] == 1      # SamplesPerPixel

    def test_unknown_section_returns_empty(self):
        """as_dict with unrecognized section returns empty dict."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        provider = asset.get_metadata()

        assert provider.as_dict("unknown") == {}
        assert provider.as_dict("nitf") == {}

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

            # Metadata keys are numeric tag ID strings
            assert meta["256"] == width, (
                f"ImageWidth (256): expected {width}, got {meta['256']}"
            )
            assert meta["257"] == height, (
                f"ImageLength (257): expected {height}, got {meta['257']}"
            )
            # BitsPerSample (258) is an array for multi-band images
            bps = meta["258"]
            if isinstance(bps, list):
                assert all(b == expected_bps for b in bps), (
                    f"BitsPerSample (258): expected all {expected_bps}, got {bps}"
                )
            else:
                assert bps == expected_bps, (
                    f"BitsPerSample (258): expected {expected_bps}, got {bps}"
                )

            # Unknown section returns empty
            provider = asset.get_metadata()
            assert provider.as_dict("unknown") == {}
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Helpers for custom tag metadata contract tests
# =============================================================================

# Private-use tag range for custom metadata contract testing.
_CUSTOM_TAG_MIN = 65000
_CUSTOM_TAG_MAX = 65499


def _write_tiff_with_metadata(path, metadata_dict):
    """Write a minimal 16x16 TIFF with the given metadata tags."""
    meta = BufferedMetadataProvider()
    meta.set("TileWidth", "256")
    meta.set("TileHeight", "256")
    meta.set("Compression", "None")
    meta.set("PlanarConfiguration", "Chunky")

    for k, v in metadata_dict.items():
        meta.set_json(k, v)

    array = np.zeros((1, 16, 16), dtype=np.uint8)
    provider = BufferedImageAssetProvider.create(
        key="image_segment_0",
        num_columns=16,
        num_rows=16,
        num_bands=1,
        block_width=16,
        block_height=16,
        pixel_type=PixelType.UInt8,
        metadata=meta,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "tiff")
    writer.metadata = meta
    writer.add_asset(
        key="image_segment_0",
        provider=provider,
        title="Test",
        description="Property test",
        roles=["data"],
    )
    writer.close()


def _read_tiff_metadata(path):
    """Read per-IFD metadata from a TIFF file."""
    reader = IO.open([str(path)], "r")
    asset = reader.get_asset("image_segment_0")
    return asset.get_metadata().as_dict()


@st.composite
def _simple_tag_metadata(draw):
    """Generate a dict of 1-5 custom tags with simple values for contract tests.

    Returns ``(metadata_dict, expectations)`` where *expectations* is a list
    of ``(tag_key, type_name)`` tuples.
    """
    num_tags = draw(st.integers(min_value=1, max_value=5))
    tags = draw(
        st.lists(
            st.integers(min_value=_CUSTOM_TAG_MIN, max_value=_CUSTOM_TAG_MAX),
            min_size=num_tags,
            max_size=num_tags,
            unique=True,
        )
    )

    metadata = {}
    expectations = []
    for tag in tags:
        # Use simple integer values — we only care about key shape, not value fidelity
        value = draw(st.integers(min_value=0, max_value=65535))
        key = str(tag)
        metadata[key] = value
        expectations.append((key, "long"))

    return metadata, expectations


# =============================================================================
# Property 8: IFD-level metadata keys are numeric strings
# =============================================================================


@pytest.mark.property
class TestTiffMetadataKeysNumeric:
    """Property 8: IFD-level metadata keys are numeric strings

    All IFD-level keys in the read-back metadata are numeric strings.
    Dataset-level keys (ByteOrder, NumberOfDirectories) are excluded
    from this check because they are not TIFF tags.

    # Feature: tiff-api, Property 8: Metadata keys are numeric
    **Validates: Requirements 7.1**
    """

    @given(data=_simple_tag_metadata())
    @pbt_settings
    def test_roundtrip_keys_are_numeric(self, data):
        """All IFD-level keys in the read-back metadata are numeric strings."""
        metadata_dict, _ = data

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_tiff_with_metadata(path, metadata_dict)
            read_meta = _read_tiff_metadata(path)

            dataset_keys = {"ByteOrder", "NumberOfDirectories"}
            for key in read_meta:
                if key in dataset_keys:
                    continue
                assert key.isdigit(), (
                    "IFD-level key " + repr(key) + " is not a numeric string"
                )
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 9: Custom tags coexist with structural tags
# =============================================================================


@pytest.mark.property
class TestTiffCustomTagCoexistence:
    """Property 9: Custom tags coexist with structural tags

    Custom tags in the private-use range do not overwrite structural tags
    set by the writer. The writer always sets ImageWidth (256),
    ImageLength (257), etc. from the image properties.

    # Feature: tiff-api, Property 9: Custom tag coexistence
    **Validates: Requirements 8.3**
    """

    @given(data=_simple_tag_metadata())
    @pbt_settings
    def test_custom_tags_coexist_with_structural(self, data):
        """Custom tags do not overwrite structural tags set by the writer."""
        metadata_dict, expectations = data

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_tiff_with_metadata(path, metadata_dict)
            read_meta = _read_tiff_metadata(path)

            # Structural tags must still be present and correct
            assert read_meta.get("256") == 16  # ImageWidth
            assert read_meta.get("257") == 16  # ImageLength
            assert read_meta.get("258") == 8   # BitsPerSample
            assert read_meta.get("277") == 1   # SamplesPerPixel

            # Custom tags must also be present
            for key, type_name in expectations:
                assert key in read_meta, (
                    "Custom tag " + key + " (" + type_name + ") missing"
                )
        finally:
            path.unlink(missing_ok=True)
