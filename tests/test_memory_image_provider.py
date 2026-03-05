"""Tests for BufferedImageAssetProvider Python bindings.

This module tests the BufferedImageAssetProvider implementation through the Python bindings,
including construction with metadata, set_full_image, and metadata round-trip.

Requirements: 2.1, 2.2, 2.3
"""

import numpy as np
import pytest

from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType


class TestBufferedImageAssetProviderConstruction:
    """Tests for BufferedImageAssetProvider construction."""

    def test_basic_construction(self):
        """Test creating a basic BufferedImageAssetProvider."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=256,
            num_rows=256,
        )
        assert provider is not None
        assert provider.key == "test_image"
        assert provider.num_columns == 256
        assert provider.num_rows == 256

    def test_construction_with_bands(self):
        """Test creating a multi-band image."""
        provider = BufferedImageAssetProvider.create(
            key="rgb_image",
            num_columns=512,
            num_rows=512,
            num_bands=3,
        )
        assert provider.num_bands == 3
        assert provider.irep == "RGB"

    def test_construction_with_block_size(self):
        """Test creating an image with custom block size."""
        provider = BufferedImageAssetProvider.create(
            key="tiled_image",
            num_columns=1024,
            num_rows=1024,
            block_width=128,
            block_height=128,
        )
        assert provider.num_pixels_per_block_horizontal == 128
        assert provider.num_pixels_per_block_vertical == 128

    def test_construction_with_pixel_type(self):
        """Test creating an image with different pixel types."""
        provider = BufferedImageAssetProvider.create(
            key="uint16_image",
            num_columns=256,
            num_rows=256,
            pixel_type=PixelType.UInt16,
        )
        assert provider.pixel_value_type == PixelType.UInt16
        assert provider.num_bits_per_pixel == 16


class TestBufferedImageAssetProviderWithMetadata:
    """Tests for BufferedImageAssetProvider with custom metadata."""

    def test_construction_with_metadata(self):
        """Test creating a BufferedImageAssetProvider with metadata."""
        # Create metadata with encoding hints
        metadata = BufferedMetadataProvider()
        metadata.set("IMODE", "P")
        metadata.set("NPPBH", "256")
        metadata.set("NPPBV", "256")

        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=512,
            num_rows=512,
            metadata=metadata,
        )

        assert provider is not None
        assert provider.key == "test_image"

    def test_metadata_round_trip(self):
        """Test that metadata is accessible after construction.
        
        **Validates: Requirements 2.2**
        """
        # Create metadata with encoding hints
        metadata = BufferedMetadataProvider()
        metadata.set("IMODE", "P")
        metadata.set("IC", "NC")
        metadata.set("NPPBH", "256")

        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=512,
            num_rows=512,
            metadata=metadata,
        )

        # Get metadata back from provider
        retrieved_metadata = provider.get_metadata()
        meta_dict = retrieved_metadata.as_dict()

        # Verify all values are present
        assert meta_dict.get("IMODE") == "P"
        assert meta_dict.get("IC") == "NC"
        assert meta_dict.get("NPPBH") == "256"

    def test_default_metadata_is_empty(self):
        """Test that default metadata is empty when not provided."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=256,
            num_rows=256,
        )

        metadata = provider.get_metadata()
        meta_dict = metadata.as_dict()

        assert len(meta_dict) == 0

    def test_metadata_with_all_encoding_hints(self):
        """Test metadata with all supported encoding hints."""
        metadata = BufferedMetadataProvider()
        metadata.set("IMODE", "B")
        metadata.set("IC", "NC")
        metadata.set("NPPBH", "512")
        metadata.set("NPPBV", "512")
        metadata.set("COMRAT", "01.0")

        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=1024,
            num_rows=1024,
            metadata=metadata,
        )

        retrieved = provider.get_metadata().as_dict()
        assert retrieved.get("IMODE") == "B"
        assert retrieved.get("IC") == "NC"
        assert retrieved.get("NPPBH") == "512"
        assert retrieved.get("NPPBV") == "512"
        assert retrieved.get("COMRAT") == "01.0"


class TestBufferedImageAssetProviderSetImage:
    """Tests for setting image data."""

    def test_set_full_image_grayscale(self):
        """Test setting full image data for grayscale image."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=64,
            num_rows=64,
            num_bands=1,
            block_width=64,
            block_height=64,
        )

        # Create test image data (bands, rows, cols)
        image_data = np.zeros((1, 64, 64), dtype=np.uint8)
        image_data[0, :, :] = 128  # Fill with gray

        provider.set_full_image(image_data)

        # Verify block exists
        assert provider.has_block(0, 0, 0)

    def test_set_full_image_rgb(self):
        """Test setting full image data for RGB image."""
        provider = BufferedImageAssetProvider.create(
            key="rgb_image",
            num_columns=64,
            num_rows=64,
            num_bands=3,
            block_width=64,
            block_height=64,
        )

        # Create RGB test image (bands, rows, cols)
        image_data = np.zeros((3, 64, 64), dtype=np.uint8)
        image_data[0, :, :] = 255  # Red channel
        image_data[1, :, :] = 128  # Green channel
        image_data[2, :, :] = 64   # Blue channel

        provider.set_full_image(image_data)

        # Verify block exists
        assert provider.has_block(0, 0, 0)

    def test_get_block_after_set_full_image(self):
        """Test retrieving block data after setting full image."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=64,
            num_rows=64,
            num_bands=1,
            block_width=64,
            block_height=64,
        )

        # Create test image with known pattern
        image_data = np.full((1, 64, 64), 200, dtype=np.uint8)
        provider.set_full_image(image_data)

        # Get block and verify data - shape is CHW (bands, rows, cols)
        block = provider.get_block(0, 0, 0)
        assert block.shape == (1, 64, 64)
        # All values should be 200
        assert np.all(block == 200)


class TestBufferedImageAssetProviderProperties:
    """Tests for BufferedImageAssetProvider properties."""

    def test_irep_mono(self):
        """Test IREP is MONO for single band images."""
        provider = BufferedImageAssetProvider.create(
            key="mono_image",
            num_columns=256,
            num_rows=256,
            num_bands=1,
        )
        assert provider.irep == "MONO"

    def test_irep_rgb(self):
        """Test IREP is RGB for 3-band images."""
        provider = BufferedImageAssetProvider.create(
            key="rgb_image",
            num_columns=256,
            num_rows=256,
            num_bands=3,
        )
        assert provider.irep == "RGB"

    def test_irep_multi(self):
        """Test IREP is MULTI for multi-band images."""
        provider = BufferedImageAssetProvider.create(
            key="multi_image",
            num_columns=256,
            num_rows=256,
            num_bands=4,
        )
        assert provider.irep == "MULTI"

    def test_image_shape(self):
        """Test image_shape property - returns CHW format (bands, rows, cols)."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=512,
            num_rows=256,
            num_bands=3,
        )
        assert provider.image_shape == (3, 256, 512)

    def test_block_shape(self):
        """Test block_shape property - returns CHW format (bands, rows, cols)."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=512,
            num_rows=512,
            num_bands=3,
            block_width=128,
            block_height=128,
        )
        assert provider.block_shape == (3, 128, 128)

    def test_block_grid_size(self):
        """Test block_grid_size property."""
        provider = BufferedImageAssetProvider.create(
            key="test_image",
            num_columns=512,
            num_rows=256,
            block_width=128,
            block_height=128,
        )
        # 512/128 = 4 blocks horizontal, 256/128 = 2 blocks vertical
        assert provider.block_grid_size == (2, 4)
