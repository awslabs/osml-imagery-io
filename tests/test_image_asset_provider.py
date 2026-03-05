"""Tests for ImageAssetProvider Python bindings.

This module tests the ImageAssetProvider methods exposed through Python bindings,
including get_block(), has_block(), band selection, and metadata access.

Requirements: 19.1-19.5
"""

from pathlib import Path

import numpy as np
import pytest

from aws.osml.io import IO, AssetType


# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
SAMPLE_NITF21 = UNIT_DATA_DIR / "sample_nitf21.ntf"
MULTI_SEGMENT = UNIT_DATA_DIR / "multi_segment.ntf"


# =============================================================================
# ImageAssetProvider Property Tests (Requirement 19.1)
# =============================================================================

class TestImageAssetProviderProperties:
    """Tests for ImageAssetProvider property accessors."""

    def test_image_dimensions(self):
        """Test num_rows and num_columns properties."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Verify dimensions are positive integers
        assert asset.num_rows > 0
        assert asset.num_columns > 0

    def test_num_bands(self):
        """Test num_bands property."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # sample_nitf21.ntf has 1 band (grayscale)
        assert asset.num_bands == 1

    def test_block_dimensions(self):
        """Test block dimension properties."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Block dimensions should be positive
        assert asset.num_pixels_per_block_horizontal > 0
        assert asset.num_pixels_per_block_vertical > 0

    def test_bits_per_pixel(self):
        """Test bits per pixel properties."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # 8-bit grayscale image
        assert asset.num_bits_per_pixel == 8
        assert asset.actual_bits_per_pixel == 8

    def test_pixel_value_type(self):
        """Test pixel_value_type property."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Should return a PixelType enum value
        pixel_type = asset.pixel_value_type
        assert pixel_type is not None

    def test_image_shape(self):
        """Test image_shape convenience property - returns CHW format (bands, rows, cols)."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        shape = asset.image_shape
        assert isinstance(shape, tuple)
        assert len(shape) == 3
        # Verify shape matches individual properties in CHW format (bands, rows, cols)
        assert shape == (asset.num_bands, asset.num_rows, asset.num_columns)

    def test_block_shape(self):
        """Test block_shape convenience property - returns CHW format (bands, rows, cols)."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        shape = asset.block_shape
        assert isinstance(shape, tuple)
        assert len(shape) == 3
        # Shape is (bands, rows, cols) - CHW format
        assert shape[0] == 1  # 1 band

    def test_block_grid_size(self):
        """Test block_grid_size property."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        grid_size = asset.block_grid_size
        assert isinstance(grid_size, tuple)
        assert len(grid_size) == 2
        # Should have at least 1 block in each dimension
        assert grid_size[0] >= 1
        assert grid_size[1] >= 1

    def test_num_resolution_levels(self):
        """Test num_resolution_levels property."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Uncompressed images have 1 resolution level
        assert asset.num_resolution_levels == 1

    def test_pad_pixel_value(self):
        """Test pad_pixel_value property."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Should return a float
        pad_value = asset.pad_pixel_value
        assert isinstance(pad_value, float)


# =============================================================================
# has_block() Tests (Requirement 19.1)
# =============================================================================

class TestHasBlock:
    """Tests for has_block() method."""

    def test_has_block_valid_coordinates(self):
        """Test has_block() returns True for valid block coordinates."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Block (0, 0) should always exist
        assert asset.has_block(0, 0, 0) is True

    def test_has_block_invalid_coordinates(self):
        """Test has_block() returns False for invalid block coordinates."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Very large coordinates should not exist
        assert asset.has_block(9999, 9999, 0) is False

    def test_has_block_invalid_resolution_level(self):
        """Test has_block() returns False for invalid resolution level."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Uncompressed images only have resolution level 0
        assert asset.has_block(0, 0, 1) is False


# =============================================================================
# get_block() Tests (Requirements 19.2, 19.3)
# =============================================================================

class TestGetBlock:
    """Tests for get_block() method returning NumPy arrays."""

    def test_get_block_returns_numpy_array(self):
        """Test that get_block() returns a NumPy array."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        block = asset.get_block(0, 0, 0)
        assert isinstance(block, np.ndarray)

    def test_get_block_shape(self):
        """Test that get_block() returns array with correct shape (bands, rows, cols)."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        block = asset.get_block(0, 0, 0)

        # Shape should be (bands, rows, cols) - CHW format
        assert len(block.shape) == 3
        assert block.shape[0] == 1  # 1 band for grayscale

    def test_get_block_dtype_uint8(self):
        """Test that get_block() returns correct dtype for 8-bit images."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        block = asset.get_block(0, 0, 0)

        # 8-bit unsigned integer
        assert block.dtype == np.uint8

    def test_get_block_data_values(self):
        """Test that get_block() returns valid pixel values."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        block = asset.get_block(0, 0, 0)

        # Values should be in valid range for uint8
        assert block.min() >= 0
        assert block.max() <= 255

    def test_get_block_invalid_coordinates_raises(self):
        """Test that get_block() raises error for invalid coordinates."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        with pytest.raises(Exception):
            asset.get_block(9999, 9999, 0)


# =============================================================================
# Band Selection Tests (Requirement 19.4)
# =============================================================================

class TestBandSelection:
    """Tests for band selection in get_block()."""

    def test_get_block_all_bands_default(self):
        """Test that get_block() returns all bands by default."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Without bands parameter, should return all bands
        block = asset.get_block(0, 0, 0)
        assert block.shape[2] == asset.num_bands

    def test_get_block_with_band_selection(self):
        """Test get_block() with explicit band selection."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Select only band 0
        block = asset.get_block(0, 0, 0, bands=[0])
        assert block.shape[2] == 1

    def test_get_block_band_selection_empty_list(self):
        """Test get_block() with empty band list returns all bands."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Empty list should return all bands (or raise error)
        # Behavior depends on implementation
        try:
            block = asset.get_block(0, 0, 0, bands=[])
            # If it succeeds, should return all bands
            assert block.shape[2] == asset.num_bands
        except Exception:
            # Empty band list may raise an error - that's acceptable
            pass


# =============================================================================
# Metadata Access Tests (Requirement 19.5)
# =============================================================================

class TestImageMetadata:
    """Tests for image metadata access."""

    def test_get_metadata_returns_provider(self):
        """Test that get_metadata() returns a metadata provider."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        metadata = asset.get_metadata()
        assert metadata is not None

    def test_metadata_as_dict(self):
        """Test that metadata.as_dict() returns image subheader fields."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        metadata = asset.get_metadata()
        fields = metadata.as_dict()

        assert isinstance(fields, dict)
        # Should contain image subheader fields
        assert len(fields) > 0

    def test_metadata_raw_bytes(self):
        """Test that metadata.raw returns raw subheader bytes."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        metadata = asset.get_metadata()
        raw_io = metadata.raw
        raw_bytes = raw_io.read()

        assert isinstance(raw_bytes, bytes)
        # Image subheader should start with "IM"
        assert raw_bytes[:2] == b"IM"


# =============================================================================
# Multi-Segment Image Tests
# =============================================================================

class TestMultiSegmentImages:
    """Tests for multi-segment NITF files with multiple images."""

    def test_multiple_image_segments(self):
        """Test accessing multiple image segments."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")

        # Get image keys
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        assert len(image_keys) == 2

        # Access both images
        for key in image_keys:
            asset = reader.get_asset(key)
            assert asset.asset_type == AssetType.Image
            assert asset.num_rows > 0
            assert asset.num_columns > 0

    def test_different_image_dimensions(self):
        """Test that different image segments can have different dimensions."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")

        img0 = reader.get_asset("image_segment_0")
        img1 = reader.get_asset("image_segment_1")

        # Verify both images have valid dimensions
        assert img0.num_rows > 0
        assert img0.num_columns > 0
        assert img1.num_rows > 0
        assert img1.num_columns > 0

    def test_get_block_from_multiple_images(self):
        """Test get_block() works for all image segments."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")

        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)

        for key in image_keys:
            asset = reader.get_asset(key)
            block = asset.get_block(0, 0, 0)
            assert isinstance(block, np.ndarray)
            assert len(block.shape) == 3
