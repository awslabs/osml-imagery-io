"""Tests for JBP DatasetWriter functionality.

This module tests the JBP writer implementation through the Python bindings,
including IO.create(), add_asset(), set_metadata(), close(), and round-trip
read/write operations.

Requirements: 19.4, 17.1, 17.2
"""

from pathlib import Path

import pytest
from aws.osml.io import IO, AssetProvider, AssetType

# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
SAMPLE_NITF21 = UNIT_DATA_DIR / "sample_nitf21.ntf"


# =============================================================================
# IO.create() Tests (Requirement 19.4)
# =============================================================================

class TestIOCreate:
    """Tests for IO.open() with write mode and format specification."""

    def test_create_nitf_writer(self, tmp_path):
        """Test creating a NITF writer."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")
        assert writer is not None

    def test_create_nitf21_writer(self, tmp_path):
        """Test creating a NITF 2.1 writer with explicit format."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf21")
        assert writer is not None

    def test_create_nsif_writer(self, tmp_path):
        """Test creating an NSIF writer."""
        output_path = tmp_path / "output.nsif"

        writer = IO.open([str(output_path)], "w", "nsif")
        assert writer is not None

    def test_create_nsif10_writer(self, tmp_path):
        """Test creating an NSIF 1.0 writer with explicit format."""
        output_path = tmp_path / "output.nsif"

        writer = IO.open([str(output_path)], "w", "nsif10")
        assert writer is not None

    def test_create_requires_format(self, tmp_path):
        """Test that creating a writer requires format specification."""
        output_path = tmp_path / "output.ntf"

        with pytest.raises(Exception) as exc_info:
            IO.open([str(output_path)], "w")

        assert "format" in str(exc_info.value).lower()

    def test_create_rejects_invalid_format(self, tmp_path):
        """Test that creating a writer rejects invalid formats."""
        output_path = tmp_path / "output.ntf"

        with pytest.raises(Exception) as exc_info:
            IO.open([str(output_path)], "w", "invalid_format")

        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()

    def test_create_rejects_jbp_format(self, tmp_path):
        """Test that 'jbp' format is rejected for writing (read-only)."""
        output_path = tmp_path / "output.ntf"

        with pytest.raises(Exception) as exc_info:
            IO.open([str(output_path)], "w", "jbp")

        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()


# =============================================================================
# add_asset() Tests (Requirement 19.4)
# =============================================================================

class TestAddAsset:
    """Tests for add_asset() functionality."""

    def test_add_image_asset(self, tmp_path):
        """Test adding an image asset."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")

        image_data = bytes([i % 256 for i in range(100)])
        asset = AssetProvider.from_bytes(
            key="image_segment_0",
            data=image_data,
            asset_type=AssetType.Image,
            title="Test Image",
        )

        writer.add_asset(
            key="image_segment_0",
            provider=asset,
            title="Test Image",
            description="A test image",
            roles=["data"],
        )

        writer.close()

        # Verify file was created
        assert output_path.exists()

    def test_add_text_asset(self, tmp_path):
        """Test adding a text asset."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")

        text_data = b"This is test text content."
        asset = AssetProvider.from_bytes(
            key="text_segment_0",
            data=text_data,
            asset_type=AssetType.Text,
            title="Test Text",
        )

        writer.add_asset(
            key="text_segment_0",
            provider=asset,
            title="Test Text",
            description="A test text segment",
            roles=["metadata"],
        )

        writer.close()

        assert output_path.exists()

    def test_add_data_asset(self, tmp_path):
        """Test adding a data (DES) asset."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")

        des_data = b"Binary DES content"
        asset = AssetProvider.from_bytes(
            key="des_segment_0",
            data=des_data,
            asset_type=AssetType.Data,
            title="Test DES",
        )

        writer.add_asset(
            key="des_segment_0",
            provider=asset,
            title="Test DES",
            description="A test DES segment",
            roles=["data"],
        )

        writer.close()

        assert output_path.exists()

    def test_add_multiple_assets(self, tmp_path):
        """Test adding multiple assets of different types."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")

        # Add image
        image_asset = AssetProvider.from_bytes(
            key="image_segment_0",
            data=bytes([0] * 64),
            asset_type=AssetType.Image,
            title="Image 1",
        )
        writer.add_asset("image_segment_0", image_asset, "Image 1", "", ["data"])

        # Add text
        text_asset = AssetProvider.from_bytes(
            key="text_segment_0",
            data=b"Text content",
            asset_type=AssetType.Text,
            title="Text 1",
        )
        writer.add_asset("text_segment_0", text_asset, "Text 1", "", ["metadata"])

        # Add DES
        des_asset = AssetProvider.from_bytes(
            key="des_segment_0",
            data=b"DES content",
            asset_type=AssetType.Data,
            title="DES 1",
        )
        writer.add_asset("des_segment_0", des_asset, "DES 1", "", ["data"])

        writer.close()

        assert output_path.exists()

    def test_add_duplicate_key_raises(self, tmp_path):
        """Test that adding a duplicate key raises an error."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")

        asset1 = AssetProvider.from_bytes(
            key="image_segment_0",
            data=bytes([0] * 64),
            asset_type=AssetType.Image,
            title="Image 1",
        )
        writer.add_asset("image_segment_0", asset1, "Image 1", "", ["data"])

        asset2 = AssetProvider.from_bytes(
            key="image_segment_0",
            data=bytes([1] * 64),
            asset_type=AssetType.Image,
            title="Image 2",
        )

        with pytest.raises(Exception) as exc_info:
            writer.add_asset("image_segment_0", asset2, "Image 2", "", ["data"])

        assert "Duplicate" in str(exc_info.value) or "key" in str(exc_info.value).lower()


# =============================================================================
# close() Tests (Requirement 19.4)
# =============================================================================

class TestClose:
    """Tests for close() producing valid NITF files."""

    def test_close_produces_valid_nitf(self, tmp_path):
        """Test that close() produces a valid NITF file."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")

        asset = AssetProvider.from_bytes(
            key="image_segment_0",
            data=bytes([128] * 64),
            asset_type=AssetType.Image,
            title="Test Image",
        )
        writer.add_asset("image_segment_0", asset, "Test Image", "", ["data"])

        writer.close()

        # Verify file starts with NITF magic
        with open(output_path, "rb") as f:
            magic = f.read(9)
            assert magic == b"NITF02.10"

    def test_close_produces_valid_nsif(self, tmp_path):
        """Test that close() produces a valid NSIF file."""
        output_path = tmp_path / "output.nsif"

        writer = IO.open([str(output_path)], "w", "nsif")

        asset = AssetProvider.from_bytes(
            key="image_segment_0",
            data=bytes([128] * 64),
            asset_type=AssetType.Image,
            title="Test Image",
        )
        writer.add_asset("image_segment_0", asset, "Test Image", "", ["data"])

        writer.close()

        # Verify file starts with NSIF magic
        with open(output_path, "rb") as f:
            magic = f.read(9)
            assert magic == b"NSIF01.00"

    def test_close_with_context_manager(self, tmp_path):
        """Test that context manager calls close() automatically."""
        output_path = tmp_path / "output.ntf"

        with IO.open([str(output_path)], "w", "nitf") as writer:
            asset = AssetProvider.from_bytes(
                key="image_segment_0",
                data=bytes([64] * 100),
                asset_type=AssetType.Image,
                title="Test Image",
            )
            writer.add_asset("image_segment_0", asset, "Test Image", "", ["data"])

        # File should exist after context manager exits
        assert output_path.exists()

        # Verify it's a valid NITF
        with open(output_path, "rb") as f:
            magic = f.read(4)
            assert magic == b"NITF"


# =============================================================================
# Round-Trip Tests (Requirements 17.1, 17.2)
# =============================================================================

class TestRoundTrip:
    """Tests for round-trip read/write operations."""

    def test_round_trip_single_image(self, tmp_path):
        """Test round-trip with a single image segment."""
        output_path = tmp_path / "round_trip.ntf"

        # Write
        original_data = bytes([i % 256 for i in range(256)])

        with IO.open([str(output_path)], "w", "nitf") as writer:
            asset = AssetProvider.from_bytes(
                key="image_segment_0",
                data=original_data,
                asset_type=AssetType.Image,
                title="Test Image",
            )
            writer.add_asset("image_segment_0", asset, "Test Image", "", ["data"])

        # Read back
        with IO.open([str(output_path)], "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) == 1
            assert "image_segment_0" in keys

            asset = reader.get_asset("image_segment_0")
            assert asset.asset_type == AssetType.Image

            read_data = asset.get_raw_asset().read()
            assert read_data == original_data

    def test_round_trip_text_segment(self, tmp_path):
        """Test round-trip with a text segment."""
        output_path = tmp_path / "round_trip_text.ntf"

        original_text = b"This is test text content for round-trip testing."

        with IO.open([str(output_path)], "w", "nitf") as writer:
            asset = AssetProvider.from_bytes(
                key="text_segment_0",
                data=original_text,
                asset_type=AssetType.Text,
                title="Test Text",
            )
            writer.add_asset("text_segment_0", asset, "Test Text", "", ["metadata"])

        with IO.open([str(output_path)], "r") as reader:
            keys = reader.get_asset_keys()
            assert "text_segment_0" in keys

            asset = reader.get_asset("text_segment_0")
            assert asset.asset_type == AssetType.Text

            read_data = asset.get_raw_asset().read()
            assert read_data == original_text

    def test_round_trip_multiple_segments(self, tmp_path):
        """Test round-trip with multiple segments of different types."""
        output_path = tmp_path / "round_trip_multi.ntf"

        image1_data = bytes([10] * 100)
        image2_data = bytes([20] * 50)
        text_data = b"Multi-segment test text"
        des_data = b"DES binary data"

        with IO.open([str(output_path)], "w", "nitf") as writer:
            # Add first image
            asset1 = AssetProvider.from_bytes(
                key="image_segment_0",
                data=image1_data,
                asset_type=AssetType.Image,
                title="Image 1",
            )
            writer.add_asset("image_segment_0", asset1, "Image 1", "", ["data"])

            # Add second image
            asset2 = AssetProvider.from_bytes(
                key="image_segment_1",
                data=image2_data,
                asset_type=AssetType.Image,
                title="Image 2",
            )
            writer.add_asset("image_segment_1", asset2, "Image 2", "", ["data"])

            # Add text
            text_asset = AssetProvider.from_bytes(
                key="text_segment_0",
                data=text_data,
                asset_type=AssetType.Text,
                title="Text",
            )
            writer.add_asset("text_segment_0", text_asset, "Text", "", ["metadata"])

            # Add DES
            des_asset = AssetProvider.from_bytes(
                key="des_segment_0",
                data=des_data,
                asset_type=AssetType.Data,
                title="DES",
            )
            writer.add_asset("des_segment_0", des_asset, "DES", "", ["data"])

        # Read back and verify
        with IO.open([str(output_path)], "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) == 4

            # Verify image 1
            img1 = reader.get_asset("image_segment_0")
            assert img1.get_raw_asset().read() == image1_data

            # Verify image 2
            img2 = reader.get_asset("image_segment_1")
            assert img2.get_raw_asset().read() == image2_data

            # Verify text
            txt = reader.get_asset("text_segment_0")
            assert txt.get_raw_asset().read() == text_data

            # Verify DES
            des = reader.get_asset("des_segment_0")
            assert des.get_raw_asset().read() == des_data

    def test_round_trip_preserves_order(self, tmp_path):
        """Test that round-trip preserves asset order."""
        output_path = tmp_path / "round_trip_order.ntf"

        with IO.open([str(output_path)], "w", "nitf") as writer:
            for i in range(5):
                asset = AssetProvider.from_bytes(
                    key=f"image_segment_{i}",
                    data=bytes([i] * 10),
                    asset_type=AssetType.Image,
                    title=f"Image {i}",
                )
                writer.add_asset(f"image_segment_{i}", asset, f"Image {i}", "", ["data"])

        with IO.open([str(output_path)], "r") as reader:
            keys = reader.get_asset_keys()
            expected = [f"image_segment_{i}" for i in range(5)]
            assert keys == expected

    def test_round_trip_nsif_format(self, tmp_path):
        """Test round-trip with NSIF format."""
        output_path = tmp_path / "round_trip.nsif"

        original_data = bytes([42] * 128)

        with IO.open([str(output_path)], "w", "nsif") as writer:
            asset = AssetProvider.from_bytes(
                key="image_segment_0",
                data=original_data,
                asset_type=AssetType.Image,
                title="NSIF Image",
            )
            writer.add_asset("image_segment_0", asset, "NSIF Image", "", ["data"])

        with IO.open([str(output_path)], "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) == 1

            asset = reader.get_asset("image_segment_0")
            read_data = asset.get_raw_asset().read()
            assert read_data == original_data

    def test_round_trip_large_data(self, tmp_path):
        """Test round-trip with larger data."""
        output_path = tmp_path / "round_trip_large.ntf"

        # 1MB of data
        original_data = bytes([i % 256 for i in range(1024 * 1024)])

        with IO.open([str(output_path)], "w", "nitf") as writer:
            asset = AssetProvider.from_bytes(
                key="image_segment_0",
                data=original_data,
                asset_type=AssetType.Image,
                title="Large Image",
            )
            writer.add_asset("image_segment_0", asset, "Large Image", "", ["data"])

        with IO.open([str(output_path)], "r") as reader:
            asset = reader.get_asset("image_segment_0")
            read_data = asset.get_raw_asset().read()
            assert read_data == original_data
