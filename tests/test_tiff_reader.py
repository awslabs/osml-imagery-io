"""Tests for TIFF DatasetReader functionality.

This module tests the TIFF reader implementation through the Python bindings,
including IO.open(), get_asset_keys(), get_asset(), metadata access, block
reading, and error conditions.

The test data file (data/unit/small.tif) is a 1024x1024 single-band uint8
tiled TIFF created from data/unit/small.ntf via gdal_translate. It uses
Deflate compression with 256x256 tiles.

Requirements: 12.1, 12.2, 12.3, 12.4
"""

import os
import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, AssetType, PixelType

# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
SMALL_TIF = UNIT_DATA_DIR / "small.tif"
SMALL_NTF = UNIT_DATA_DIR / "small.ntf"


def skip_if_missing(path: Path):
    if not path.exists():
        pytest.skip(f"Test data file not available: {path}")


# =============================================================================
# IO.open() Tests (Requirement 12.1, 12.2)
# =============================================================================

class TestIOOpen:
    """Tests for IO.open() with TIFF files."""

    def test_open_tif_file(self):
        """IO.open() with .tif extension returns a reader."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        assert reader is not None

    def test_open_with_explicit_tiff_format(self):
        """IO.open() with explicit 'tiff' format works regardless of extension."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r", "tiff")
        assert reader is not None

    def test_open_with_explicit_tif_format(self):
        """IO.open() with explicit 'tif' format string works."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r", "tif")
        assert reader is not None

    def test_open_nonexistent_file_raises(self):
        """Opening a nonexistent .tif file raises an error."""
        with pytest.raises(Exception):
            IO.open(["nonexistent.tif"], "r")

    def test_open_invalid_tiff_raises(self):
        """Opening a file with invalid TIFF magic bytes raises an error."""
        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(b"not a tiff file at all")
            tmp = f.name
        try:
            with pytest.raises(Exception) as exc_info:
                IO.open([tmp], "r")
            assert "Invalid" in str(exc_info.value) or "magic" in str(exc_info.value).lower()
        finally:
            os.unlink(tmp)

    def test_write_mode_creates_writer(self):
        """TIFF write mode creates a writer successfully."""
        writer = IO.open(["output.tif"], "w", "tiff")
        assert writer is not None

    def test_context_manager(self):
        """TIFF reader supports context manager protocol."""
        skip_if_missing(SMALL_TIF)
        with IO.open([str(SMALL_TIF)], "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) > 0


# =============================================================================
# Asset Key Tests (Requirement 12.1)
# =============================================================================

class TestGetAssetKeys:
    """Tests for get_asset_keys() on TIFF files."""

    def test_single_image_segment(self):
        """Single-IFD TIFF has one image segment key."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        assert keys == ["image_segment_0"]

    def test_no_text_segments(self):
        """TIFF files have no text segments."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Text)
        assert keys == []

    def test_no_data_segments(self):
        """TIFF files have no data segments."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Data)
        assert keys == []

    def test_no_graphics_segments(self):
        """TIFF files have no graphics segments."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Graphics)
        assert keys == []

    def test_invalid_key_raises(self):
        """get_asset() with invalid key raises an error."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        with pytest.raises(Exception):
            reader.get_asset("nonexistent_key")


# =============================================================================
# Image Asset Properties (Requirement 12.1, 12.3)
# =============================================================================

class TestImageAssetProperties:
    """Tests for TIFFImageAssetProvider properties."""

    @pytest.fixture()
    def asset(self):
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        return reader.get_asset("image_segment_0")

    def test_asset_type_is_image(self, asset):
        assert asset.asset_type == AssetType.Image

    def test_key(self, asset):
        assert asset.key == "image_segment_0"

    def test_dimensions(self, asset):
        assert asset.num_columns == 1024
        assert asset.num_rows == 1024

    def test_bands(self, asset):
        assert asset.num_bands == 1

    def test_pixel_type(self, asset):
        assert asset.pixel_value_type == PixelType.UInt8

    def test_tile_dimensions(self, asset):
        # gdal_translate produces 256x256 tiles by default
        assert asset.num_pixels_per_block_horizontal == 256
        assert asset.num_pixels_per_block_vertical == 256

    def test_block_grid(self, asset):
        # 1024 / 256 = 4 tiles in each direction
        assert asset.block_grid_size == (4, 4)

    def test_single_resolution_level(self, asset):
        assert asset.num_resolution_levels == 1

    def test_image_shape(self, asset):
        assert asset.image_shape == (1, 1024, 1024)

    def test_block_shape(self, asset):
        assert asset.block_shape == (1, 256, 256)

    def test_has_block_valid(self, asset):
        for r in range(4):
            for c in range(4):
                assert asset.has_block(r, c, 0) is True

    def test_has_block_out_of_bounds(self, asset):
        assert asset.has_block(4, 0, 0) is False
        assert asset.has_block(0, 4, 0) is False


# =============================================================================
# Block Reading Tests (Requirement 12.3)
# =============================================================================

class TestBlockReading:
    """Tests for reading pixel data through get_block()."""

    @pytest.fixture()
    def asset(self):
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        return reader.get_asset("image_segment_0")

    def test_block_returns_numpy_array(self, asset):
        block = asset.get_block(0, 0, 0, None)
        assert isinstance(block, np.ndarray)

    def test_block_shape(self, asset):
        block = asset.get_block(0, 0, 0, None)
        assert block.shape == (1, 256, 256)

    def test_block_dtype(self, asset):
        block = asset.get_block(0, 0, 0, None)
        assert block.dtype == np.uint8

    def test_pixel_values_match_nitf(self):
        """Assembled TIFF tiles produce identical pixels to the NITF source."""
        skip_if_missing(SMALL_TIF)
        skip_if_missing(SMALL_NTF)

        tif_reader = IO.open([str(SMALL_TIF)], "r")
        tif_asset = tif_reader.get_asset("image_segment_0")

        # Assemble all tiles into a full image
        grid_rows, grid_cols = tif_asset.block_grid_size
        bh = tif_asset.num_pixels_per_block_vertical
        bw = tif_asset.num_pixels_per_block_horizontal
        h, w = tif_asset.num_rows, tif_asset.num_columns
        bands = tif_asset.num_bands

        full_tif = np.zeros((bands, h, w), dtype=np.uint8)
        for r in range(grid_rows):
            for c in range(grid_cols):
                block = tif_asset.get_block(r, c, 0, None)
                y0, x0 = r * bh, c * bw
                y1 = min(y0 + block.shape[1], h)
                x1 = min(x0 + block.shape[2], w)
                full_tif[:, y0:y1, x0:x1] = block[:, :y1 - y0, :x1 - x0]

        # Compare with NITF source
        ntf_reader = IO.open([str(SMALL_NTF)], "r")
        ntf_asset = ntf_reader.get_asset("image_segment_0")
        full_ntf = ntf_asset.get_block(0, 0, 0, None)

        np.testing.assert_array_equal(full_tif, full_ntf)

    def test_known_pixel_values(self, asset):
        """Top-left corner of the image has expected pixel values."""
        block = asset.get_block(0, 0, 0, None)
        # Known values from the small.ntf source (verified manually)
        expected_top_left = np.array([
            [125, 128, 126, 113, 123],
            [124, 125, 135, 117, 118],
            [131, 121, 125, 128, 122],
            [146, 122, 125, 125, 122],
            [125, 128, 122, 121, 123],
        ], dtype=np.uint8)
        np.testing.assert_array_equal(block[0, :5, :5], expected_top_left)

    def test_out_of_bounds_block_raises(self, asset):
        """get_block() with out-of-bounds coordinates raises an error."""
        with pytest.raises(Exception):
            asset.get_block(4, 0, 0, None)

    def test_invalid_resolution_level_raises(self, asset):
        """get_block() with resolution level > 0 raises an error."""
        with pytest.raises(Exception):
            asset.get_block(0, 0, 1, None)


# =============================================================================
# Metadata Tests (Requirement 12.4)
# =============================================================================

class TestMetadata:
    """Tests for metadata access through as_dict()."""

    def test_dataset_metadata(self):
        """Dataset-level metadata contains file-level TIFF info."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        meta = reader.metadata.as_dict()

        assert meta["ByteOrder"] == "LittleEndian"
        assert meta["NumberOfDirectories"] == 1
        assert meta["NumberOfImageSegments"] == 1

    def test_dataset_metadata_keys(self):
        """Dataset metadata has exactly the expected keys."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        meta = reader.metadata.as_dict()

        assert set(meta.keys()) == {"ByteOrder", "NumberOfDirectories", "NumberOfImageSegments"}

    def test_image_metadata(self):
        """Per-image metadata contains standard TIFF tags (numeric keys)."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        meta = asset.get_metadata().as_dict()

        # The reader returns numeric tag IDs as string keys.
        # Use TagNameResolver for human-readable access if needed.
        assert meta["256"] == 1024       # ImageWidth
        assert meta["257"] == 1024       # ImageLength
        assert meta["258"] == 8          # BitsPerSample
        assert meta["277"] == 1          # SamplesPerPixel
        assert meta["259"] == 8          # Compression (Deflate)
        assert meta["262"] == 1          # PhotometricInterpretation (MinIsBlack)
        assert meta["284"] == 1          # PlanarConfiguration (Chunky)
        assert meta["339"] == 1          # SampleFormat (UInt)
        assert meta["322"] == 256        # TileWidth
        assert meta["323"] == 256        # TileLength

    def test_tiff_section_matches_default(self):
        """as_dict('tiff') returns standard TIFF fields only (no Geo-prefixed keys)."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")
        meta_provider = asset.get_metadata()

        tiff_dict = meta_provider.as_dict("tiff")
        full_dict = meta_provider.as_dict()

        # 'tiff' section should be a subset of the full dict (no Geo keys)
        for key, value in tiff_dict.items():
            assert key in full_dict
            assert full_dict[key] == value

        # 'tiff' section should not contain any Geo-prefixed keys
        for key in tiff_dict:
            assert not key.startswith("Geo"), f"Unexpected Geo key in tiff section: {key}"

    def test_unknown_section_returns_empty(self):
        """as_dict() with unrecognized section name returns empty dict."""
        skip_if_missing(SMALL_TIF)
        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")

        assert asset.get_metadata().as_dict("unknown") == {}
        assert asset.get_metadata().as_dict("nitf") == {}
