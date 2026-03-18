"""Property-based tests for IO contracts.

This module contains property tests that validate IO factory behavior including:
- Format auto-detection
- Dataset round-trip consistency
- TIFF format detection and routing
"""

from pathlib import Path

import pytest
from aws.osml.io import IO

# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
SMALL_NTF = UNIT_DATA_DIR / "small.ntf"
SMALL_TIF = UNIT_DATA_DIR / "small.tif"
SAMPLE_NITF21 = UNIT_DATA_DIR / "sample_nitf21.ntf"
SAMPLE_NSIF10 = UNIT_DATA_DIR / "sample_nsif10.nsif"
MULTI_SEGMENT = UNIT_DATA_DIR / "multi_segment.ntf"


# =============================================================================
# Python Format Auto-Detection Tests
# =============================================================================

@pytest.mark.property
class TestFormatAutoDetection:
    """For any NITF or NSIF file opened via Python `IO.open()`, the returned reader
    SHALL be able to access all segments without the caller specifying the format.
    """

    def test_open_nitf_file_without_format_specification(self):
        """Test that IO.open() can open NITF files without specifying format.

        This is the core property test - format should be auto-detected from extension.
        """
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        # Open without specifying format - should auto-detect from .ntf extension
        reader = IO.open([str(SMALL_NTF)], "r")
        assert reader is not None, "IO.open() should return a reader for NITF files"

        # Should be able to get asset keys without errors
        keys = reader.get_asset_keys()
        assert isinstance(keys, list), "get_asset_keys() should return a list"

        # The file should have at least one segment
        assert len(keys) > 0, "NITF file should have at least one segment"

        # Each key should follow the expected pattern
        for key in keys:
            assert "_segment_" in key, f"Asset key '{key}' should follow pattern '{{type}}_segment_{{index}}'"

    def test_open_with_string_path(self):
        """Test that IO.open() accepts list of string paths."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r")
        assert reader is not None, "IO.open() should accept string paths"

        keys = reader.get_asset_keys()
        assert len(keys) > 0

    def test_has_asset_consistency(self):
        """Test that has_asset() is consistent with get_asset_keys().

        Property: For any key returned by get_asset_keys(), has_asset() should return True.
        """
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r")
        keys = reader.get_asset_keys()

        for key in keys:
            assert reader.has_asset(key), \
                f"has_asset('{key}') should return True for key from get_asset_keys()"

    def test_has_asset_false_for_invalid_key(self):
        """Test that has_asset() returns False for invalid keys."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r")

        # These should all return False
        assert not reader.has_asset("nonexistent_key")
        assert not reader.has_asset("")
        assert not reader.has_asset("invalid_segment_999")

    def test_get_asset_returns_provider(self):
        """Test that get_asset() returns an asset provider for valid keys."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r")
        keys = reader.get_asset_keys()

        if len(keys) > 0:
            asset = reader.get_asset(keys[0])
            assert asset is not None, "get_asset() should return an asset provider"

            # Asset should have expected properties
            assert hasattr(asset, 'key'), "Asset should have 'key' property"
            assert hasattr(asset, 'media_type'), "Asset should have 'media_type' property"

    def test_open_rejects_unsupported_extension(self):
        """Test that IO.open() rejects files with unsupported extensions."""
        with pytest.raises(Exception) as exc_info:
            IO.open(["nonexistent.jpg"], "r")

        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()

    def test_open_rejects_nonexistent_file(self):
        """Test that IO.open() raises error for nonexistent files."""
        with pytest.raises(Exception):
            IO.open(["nonexistent_file.ntf"], "r")

    def test_default_mode_is_read(self):
        """Test that default mode is 'r' (read)."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)])
        assert reader is not None

        keys = reader.get_asset_keys()
        assert len(keys) > 0


@pytest.mark.property
class TestIOOpenWithFormat:
    """Tests for IO.open() with explicit format specification."""

    def test_open_with_nitf_format(self):
        """Test IO.open() with explicit 'nitf' format."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r", "nitf")
        assert reader is not None

        keys = reader.get_asset_keys()
        assert len(keys) > 0

    def test_open_with_jbp_format(self):
        """Test IO.open() with 'jbp' format (auto-detect)."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r", "jbp")
        assert reader is not None

        keys = reader.get_asset_keys()
        assert len(keys) > 0

    def test_open_rejects_invalid_format(self):
        """Test IO.open() rejects invalid format strings."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        with pytest.raises(Exception) as exc_info:
            IO.open([str(SMALL_NTF)], "r", "invalid_format")

        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()


@pytest.mark.property
class TestIOCreate:
    """Tests for IO.open() with write mode."""

    def test_create_with_nitf_format(self, tmp_path):
        """Test IO.open() with 'w' mode and 'nitf' format."""
        output_path = tmp_path / "output.ntf"

        writer = IO.open([str(output_path)], "w", "nitf")
        assert writer is not None

    def test_create_with_nsif_format(self, tmp_path):
        """Test IO.open() with 'w' mode and 'nsif' format."""
        output_path = tmp_path / "output.nsif"

        writer = IO.open([str(output_path)], "w", "nsif")
        assert writer is not None

    def test_create_rejects_jbp_format(self, tmp_path):
        """Test IO.open() with 'w' mode rejects 'jbp' format (read-only format)."""
        output_path = tmp_path / "output.ntf"

        with pytest.raises(Exception) as exc_info:
            IO.open([str(output_path)], "w", "jbp")

        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()

    def test_create_rejects_invalid_format(self, tmp_path):
        """Test IO.open() with 'w' mode rejects invalid format strings."""
        output_path = tmp_path / "output.ntf"

        with pytest.raises(Exception) as exc_info:
            IO.open([str(output_path)], "w", "invalid_format")

        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()

    def test_create_requires_format(self, tmp_path):
        """Test IO.open() with 'w' mode requires format specification."""
        output_path = tmp_path / "output.ntf"

        with pytest.raises(Exception) as exc_info:
            IO.open([str(output_path)], "w")

        assert "format" in str(exc_info.value).lower() or "must be specified" in str(exc_info.value).lower()


@pytest.mark.property
class TestIOInvalidMode:
    """Tests for IO.open() with invalid mode."""

    def test_invalid_mode_rejected(self):
        """Test IO.open() rejects invalid mode strings."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        with pytest.raises(Exception) as exc_info:
            IO.open([str(SMALL_NTF)], "x")  # Invalid mode

        assert "mode" in str(exc_info.value).lower() or "Invalid" in str(exc_info.value)


@pytest.mark.property
class TestIOOpenPathsList:
    """Tests for IO.open() paths list parameter."""

    def test_single_element_list(self):
        """Test IO.open() with single-element list."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_NTF)], "r")
        assert reader is not None
        keys = reader.get_asset_keys()
        assert len(keys) > 0

    def test_multi_element_list_uses_first(self):
        """Test IO.open() with multi-element list uses first path."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")

        # Second path is invalid, but should be ignored since first is used
        reader = IO.open([str(SMALL_NTF), "nonexistent.ntf"], "r")
        assert reader is not None
        keys = reader.get_asset_keys()
        assert len(keys) > 0

    def test_empty_list_raises_value_error(self):
        """Test IO.open() with empty list raises ValueError."""
        with pytest.raises(ValueError) as exc_info:
            IO.open([], "r")

        assert "empty" in str(exc_info.value).lower()


# =============================================================================
# Dataset Round-Trip Consistency Tests
# =============================================================================

@pytest.mark.property
class TestDatasetRoundTripConsistency:
    """For any valid dataset written with JBPDatasetWriter, reading it back with
    JBPDatasetReader SHALL produce equivalent metadata and asset data.
    """

    def test_nitf21_round_trip_asset_count(self):
        """Test that NITF 2.1 round-trip preserves asset count."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        keys = reader.get_asset_keys()

        # The sample_nitf21.ntf was created with 1 image segment
        assert len(keys) == 1, f"Expected 1 asset, got {len(keys)}"
        assert "image_segment_0" in keys

    def test_nitf21_round_trip_asset_data(self):
        """Test that NITF 2.1 round-trip preserves asset data."""
        from aws.osml.io import AssetType

        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")

        # Verify asset properties
        assert asset.key == "image_segment_0"
        assert asset.asset_type == AssetType.Image
        assert asset.media_type == "application/vnd.nitf.image"

        # Verify raw data can be retrieved
        raw_data = asset.get_raw_asset()
        data = raw_data.read()

        # The sample was created with 64 bytes (8x8 grayscale)
        assert len(data) == 64, f"Expected 64 bytes, got {len(data)}"

    def test_nsif10_round_trip_asset_count(self):
        """Test that NSIF 1.0 round-trip preserves asset count."""
        if not SAMPLE_NSIF10.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NSIF10)], "r")
        keys = reader.get_asset_keys()

        # The sample_nsif10.nsif was created with 1 image segment
        assert len(keys) == 1, f"Expected 1 asset, got {len(keys)}"
        assert "image_segment_0" in keys

    def test_nsif10_round_trip_asset_data(self):
        """Test that NSIF 1.0 round-trip preserves asset data."""
        from aws.osml.io import AssetType

        if not SAMPLE_NSIF10.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SAMPLE_NSIF10)], "r")
        asset = reader.get_asset("image_segment_0")

        # Verify asset properties
        assert asset.key == "image_segment_0"
        assert asset.asset_type == AssetType.Image

        # Verify raw data can be retrieved
        raw_data = asset.get_raw_asset()
        data = raw_data.read()

        # The sample was created with 64 bytes (8x8 grayscale)
        assert len(data) == 64, f"Expected 64 bytes, got {len(data)}"

    def test_multi_segment_round_trip_asset_count(self):
        """Test that multi-segment NITF round-trip preserves all assets."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")
        keys = reader.get_asset_keys()

        # The multi_segment.ntf was created with:
        # - 2 image segments
        # - 1 text segment
        # - 1 DES segment
        assert len(keys) == 4, f"Expected 4 assets, got {len(keys)}"

        # Verify all expected keys are present
        assert "image_segment_0" in keys
        assert "image_segment_1" in keys
        assert "text_segment_0" in keys
        assert "des_segment_0" in keys

    def test_multi_segment_round_trip_image_data(self):
        """Test that multi-segment NITF preserves image data."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")

        # First image: 16x16 = 256 bytes
        asset0 = reader.get_asset("image_segment_0")
        data0 = asset0.get_raw_asset().read()
        assert len(data0) == 256, f"Expected 256 bytes for image_segment_0, got {len(data0)}"

        # Second image: 8x8 = 64 bytes
        asset1 = reader.get_asset("image_segment_1")
        data1 = asset1.get_raw_asset().read()
        assert len(data1) == 64, f"Expected 64 bytes for image_segment_1, got {len(data1)}"

    def test_multi_segment_round_trip_text_data(self):
        """Test that multi-segment NITF preserves text data."""
        from aws.osml.io import AssetType

        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")

        asset = reader.get_asset("text_segment_0")
        assert asset.asset_type == AssetType.Text
        assert asset.media_type == "text/plain"

        data = asset.get_raw_asset().read()
        expected_text = b"This is sample text content for testing."
        assert data == expected_text, f"Text data mismatch: {data}"

    def test_multi_segment_round_trip_des_data(self):
        """Test that multi-segment NITF preserves DES data."""
        from aws.osml.io import AssetType

        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(MULTI_SEGMENT)], "r")

        asset = reader.get_asset("des_segment_0")
        assert asset.asset_type == AssetType.Data
        assert asset.media_type == "application/octet-stream"

        data = asset.get_raw_asset().read()
        expected_data = b"Sample DES data content"
        assert data == expected_data, f"DES data mismatch: {data}"

    def test_round_trip_write_read_new_file(self, tmp_path):
        """Test complete round-trip: write new file, read it back."""
        from aws.osml.io import AssetProvider, AssetType

        output_path = tmp_path / "round_trip_test.ntf"

        # Create test data
        test_image_data = bytes([i % 256 for i in range(100)])
        test_text_data = b"Round-trip test text content"

        # Write file
        writer = IO.open([str(output_path)], "w", "nitf")

        image_asset = AssetProvider.from_bytes(
            key="image_segment_0",
            data=test_image_data,
            asset_type=AssetType.Image,
            title="Test Image",
        )
        writer.add_asset(
            key="image_segment_0",
            provider=image_asset,
            title="Test Image",
            description="",
            roles=["data"],
        )

        text_asset = AssetProvider.from_bytes(
            key="text_segment_0",
            data=test_text_data,
            asset_type=AssetType.Text,
            title="Test Text",
        )
        writer.add_asset(
            key="text_segment_0",
            provider=text_asset,
            title="Test Text",
            description="",
            roles=["metadata"],
        )

        writer.close()

        # Read file back
        reader = IO.open([str(output_path)], "r")
        keys = reader.get_asset_keys()

        # Verify asset count
        assert len(keys) == 2, f"Expected 2 assets, got {len(keys)}"

        # Verify image data
        image = reader.get_asset("image_segment_0")
        image_data_read = image.get_raw_asset().read()
        assert image_data_read == test_image_data, "Image data mismatch"

        # Verify text data
        text = reader.get_asset("text_segment_0")
        text_data_read = text.get_raw_asset().read()
        assert text_data_read == test_text_data, "Text data mismatch"

    def test_round_trip_preserves_asset_order(self, tmp_path):
        """Test that round-trip preserves the order of assets."""
        from aws.osml.io import AssetProvider, AssetType

        output_path = tmp_path / "order_test.ntf"

        # Write file with multiple images in specific order
        writer = IO.open([str(output_path)], "w", "nitf")

        for i in range(3):
            data = bytes([i] * 10)
            asset = AssetProvider.from_bytes(
                key=f"image_segment_{i}",
                data=data,
                asset_type=AssetType.Image,
                title=f"Image {i}",
            )
            writer.add_asset(
                key=f"image_segment_{i}",
                provider=asset,
                title=f"Image {i}",
                description="",
                roles=["data"],
            )

        writer.close()

        # Read back and verify order
        reader = IO.open([str(output_path)], "r")
        keys = reader.get_asset_keys()

        assert keys == ["image_segment_0", "image_segment_1", "image_segment_2"], \
            f"Asset order not preserved: {keys}"

        # Verify each asset has correct data
        for i in range(3):
            asset = reader.get_asset(f"image_segment_{i}")
            data = asset.get_raw_asset().read()
            expected = bytes([i] * 10)
            assert data == expected, f"Data mismatch for image_segment_{i}"


# =============================================================================
# TIFF Format Detection Tests
# =============================================================================


@pytest.mark.property
class TestTiffFormatDetection:
    """TIFF format auto-detection and explicit format routing.

    Verifies that the IO factory correctly routes .tif/.tiff extensions
    and explicit "tiff"/"tif" format strings to the TIFF reader.
    """

    def test_tif_extension_auto_detected(self):
        """IO.open() auto-detects .tif extension and returns a reader."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        assert reader is not None

        keys = reader.get_asset_keys()
        assert len(keys) > 0
        assert "image_segment_0" in keys

    def test_tiff_extension_auto_detected(self, tmp_path):
        """IO.open() auto-detects .tiff extension."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data file not available")

        # Copy small.tif to a .tiff extension
        tiff_path = tmp_path / "test.tiff"
        tiff_path.write_bytes(SMALL_TIF.read_bytes())

        reader = IO.open([str(tiff_path)], "r")
        assert reader is not None

        keys = reader.get_asset_keys()
        assert len(keys) > 0

    def test_explicit_tiff_format(self):
        """IO.open() with explicit 'tiff' format routes to TIFF reader."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_TIF)], "r", "tiff")
        assert reader is not None

        keys = reader.get_asset_keys()
        assert "image_segment_0" in keys

    def test_explicit_tif_format(self):
        """IO.open() with explicit 'tif' format routes to TIFF reader."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_TIF)], "r", "tif")
        assert reader is not None

        keys = reader.get_asset_keys()
        assert "image_segment_0" in keys

    def test_tiff_write_mode_supported(self):
        """TIFF write mode creates a writer successfully."""
        writer = IO.open(["output.tif"], "w", "tiff")
        assert writer is not None

    def test_tiff_has_asset_consistency(self):
        """has_asset() is consistent with get_asset_keys() for TIFF files."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        keys = reader.get_asset_keys()

        for key in keys:
            assert reader.has_asset(key), (
                f"has_asset('{key}') should be True for key from get_asset_keys()"
            )

        assert not reader.has_asset("nonexistent_key")
        assert not reader.has_asset("image_segment_999")

    def test_tiff_asset_has_expected_properties(self):
        """TIFF asset provider exposes expected properties."""
        if not SMALL_TIF.exists():
            pytest.skip("Test data file not available")

        reader = IO.open([str(SMALL_TIF)], "r")
        asset = reader.get_asset("image_segment_0")

        assert asset is not None
        assert hasattr(asset, "key")
        assert hasattr(asset, "num_columns")
        assert hasattr(asset, "num_rows")
        assert hasattr(asset, "num_bands")
        assert hasattr(asset, "pixel_value_type")
        assert asset.key == "image_segment_0"
