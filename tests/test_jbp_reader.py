"""Tests for JBP DatasetReader functionality.

This module tests the JBP reader implementation through the Python bindings,
including IO.open(), get_asset_keys(), get_asset(), metadata access, and
context manager protocol.

Requirements: 19.1, 19.3, 19.5
"""

from pathlib import Path

import pytest

from aws.osml.io import IO, AssetType


# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
SMALL_NTF = UNIT_DATA_DIR / "small.ntf"
SAMPLE_NITF21 = UNIT_DATA_DIR / "sample_nitf21.ntf"
SAMPLE_NSIF10 = UNIT_DATA_DIR / "sample_nsif10.nsif"
MULTI_SEGMENT = UNIT_DATA_DIR / "multi_segment.ntf"


# =============================================================================
# IO.open() Tests (Requirement 19.3)
# =============================================================================

class TestIOOpen:
    """Tests for IO.open() with unit test NITF files."""

    def test_open_nitf21_file(self):
        """Test opening a NITF 2.1 file."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        assert reader is not None

    def test_open_nsif10_file(self):
        """Test opening an NSIF 1.0 file."""
        if not SAMPLE_NSIF10.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NSIF10)], "r")
        assert reader is not None

    def test_open_small_ntf(self):
        """Test opening a small NITF file."""
        if not SMALL_NTF.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SMALL_NTF)], "r")
        assert reader is not None

    def test_open_multi_segment_file(self):
        """Test opening a multi-segment NITF file."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        assert reader is not None

    def test_open_with_explicit_nitf_format(self):
        """Test opening with explicit 'nitf' format."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r", "nitf")
        assert reader is not None

    def test_open_with_explicit_jbp_format(self):
        """Test opening with explicit 'jbp' format."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r", "jbp")
        assert reader is not None

    def test_open_nonexistent_file_raises(self):
        """Test that opening a nonexistent file raises an error."""
        with pytest.raises(Exception):
            IO.open(["nonexistent_file.ntf"], "r")

    def test_open_unsupported_extension_raises(self):
        """Test that opening a file with unsupported extension raises."""
        with pytest.raises(Exception) as exc_info:
            IO.open(["file.jpg"], "r")
        assert "Unsupported" in str(exc_info.value) or "format" in str(exc_info.value).lower()


# =============================================================================
# get_asset_keys() Tests (Requirement 19.1)
# =============================================================================

class TestGetAssetKeys:
    """Tests for get_asset_keys() with type filtering."""

    def test_get_all_asset_keys(self):
        """Test getting all asset keys without filtering."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        keys = reader.get_asset_keys()
        
        assert isinstance(keys, list)
        assert len(keys) == 4  # 2 images + 1 text + 1 DES
        
        # Verify expected keys are present
        assert "image_segment_0" in keys
        assert "image_segment_1" in keys
        assert "text_segment_0" in keys
        assert "des_segment_0" in keys

    def test_get_asset_keys_filter_by_image_type(self):
        """Test filtering asset keys by Image type."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        
        assert len(keys) == 2
        assert "image_segment_0" in keys
        assert "image_segment_1" in keys
        assert "text_segment_0" not in keys
        assert "des_segment_0" not in keys

    def test_get_asset_keys_filter_by_text_type(self):
        """Test filtering asset keys by Text type."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Text)
        
        assert len(keys) == 1
        assert "text_segment_0" in keys

    def test_get_asset_keys_filter_by_data_type(self):
        """Test filtering asset keys by Data type."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Data)
        
        assert len(keys) == 1
        assert "des_segment_0" in keys

    def test_get_asset_keys_single_image_file(self):
        """Test getting asset keys from a single-image file."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        keys = reader.get_asset_keys()
        
        assert len(keys) == 1
        assert "image_segment_0" in keys

    def test_get_asset_keys_key_format(self):
        """Test that asset keys follow the expected format."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        keys = reader.get_asset_keys()
        
        for key in keys:
            # Keys should follow pattern: {type}_segment_{index}
            parts = key.split("_")
            assert len(parts) == 3
            assert parts[1] == "segment"
            assert parts[2].isdigit()


# =============================================================================
# get_asset() Tests (Requirement 19.1)
# =============================================================================

class TestGetAsset:
    """Tests for get_asset() returning correct asset providers."""

    def test_get_image_asset(self):
        """Test getting an image asset."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")
        
        assert asset is not None
        assert asset.key == "image_segment_0"
        assert asset.asset_type == AssetType.Image
        assert asset.media_type == "application/vnd.nitf.image"

    def test_get_text_asset(self):
        """Test getting a text asset."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        asset = reader.get_asset("text_segment_0")
        
        assert asset is not None
        assert asset.key == "text_segment_0"
        assert asset.asset_type == AssetType.Text
        assert asset.media_type == "text/plain"

    def test_get_data_asset(self):
        """Test getting a data (DES) asset."""
        if not MULTI_SEGMENT.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(MULTI_SEGMENT)], "r")
        asset = reader.get_asset("des_segment_0")
        
        assert asset is not None
        assert asset.key == "des_segment_0"
        assert asset.asset_type == AssetType.Data
        assert asset.media_type == "application/octet-stream"

    def test_get_asset_raw_data(self):
        """Test getting raw asset data."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")
        
        raw_io = asset.get_raw_asset()
        data = raw_io.read()
        
        assert isinstance(data, bytes)
        assert len(data) == 64  # 8x8 grayscale image

    def test_get_asset_nonexistent_raises(self):
        """Test that getting a nonexistent asset raises an error."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        
        with pytest.raises(Exception):
            reader.get_asset("nonexistent_key")

    def test_has_asset_true_for_existing(self):
        """Test has_asset() returns True for existing assets."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        
        assert reader.has_asset("image_segment_0") is True

    def test_has_asset_false_for_nonexistent(self):
        """Test has_asset() returns False for nonexistent assets."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        
        assert reader.has_asset("nonexistent_key") is False
        assert reader.has_asset("") is False
        assert reader.has_asset("image_segment_999") is False


# =============================================================================
# metadata().as_dict() Tests (Requirement 19.1)
# =============================================================================

class TestMetadata:
    """Tests for metadata().as_dict() with prefix filtering."""

    def test_get_file_metadata(self):
        """Test getting file-level metadata."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        metadata = reader.metadata
        
        assert metadata is not None

    def test_metadata_as_dict_all_fields(self):
        """Test getting all metadata fields."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        metadata = reader.metadata
        all_fields = metadata.as_dict()
        
        assert isinstance(all_fields, dict)
        assert len(all_fields) > 0
        
        # Should contain standard NITF header fields
        assert "fhdr" in all_fields or "FHDR" in all_fields

    def test_metadata_as_dict_with_prefix(self):
        """Test getting metadata fields with prefix filtering."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        metadata = reader.metadata
        
        # Get only fields starting with 'f' (file-level fields)
        f_fields = metadata.as_dict("f")
        
        assert isinstance(f_fields, dict)
        # All returned keys should start with 'f'
        for key in f_fields.keys():
            assert key.lower().startswith("f"), f"Key '{key}' should start with 'f'"

    def test_metadata_raw_bytes(self):
        """Test getting raw metadata bytes."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        metadata = reader.metadata
        raw_io = metadata.raw
        raw_bytes = raw_io.read()
        
        assert isinstance(raw_bytes, bytes)
        assert len(raw_bytes) > 0
        # Should start with NITF magic
        assert raw_bytes[:4] == b"NITF"

    def test_asset_metadata(self):
        """Test getting asset-level metadata."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        asset = reader.get_asset("image_segment_0")
        metadata = asset.get_metadata()
        
        assert metadata is not None
        
        all_fields = metadata.as_dict()
        assert isinstance(all_fields, dict)


# =============================================================================
# Context Manager Protocol Tests (Requirement 19.5)
# =============================================================================

class TestContextManager:
    """Tests for context manager protocol support."""

    def test_context_manager_basic(self):
        """Test basic context manager usage."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        with IO.open([str(SAMPLE_NITF21)], "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) == 1

    def test_context_manager_closes_on_exit(self):
        """Test that context manager closes the reader on exit."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        with reader:
            keys = reader.get_asset_keys()
            assert len(keys) == 1
        
        # After exiting context, reader should be closed
        # Attempting to use it should raise an error
        with pytest.raises(Exception):
            reader.get_asset_keys()

    def test_context_manager_with_exception(self):
        """Test that context manager closes even when exception occurs."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        
        try:
            with reader:
                raise ValueError("Test exception")
        except ValueError:
            pass
        
        # Reader should still be closed
        with pytest.raises(Exception):
            reader.get_asset_keys()

    def test_context_manager_nested(self):
        """Test nested context managers."""
        if not SAMPLE_NITF21.exists() or not MULTI_SEGMENT.exists():
            pytest.skip("Test data files not available")
        
        with IO.open([str(SAMPLE_NITF21)], "r") as reader1:
            with IO.open([str(MULTI_SEGMENT)], "r") as reader2:
                keys1 = reader1.get_asset_keys()
                keys2 = reader2.get_asset_keys()
                
                assert len(keys1) == 1
                assert len(keys2) == 4

    def test_explicit_close(self):
        """Test explicit close() method."""
        if not SAMPLE_NITF21.exists():
            pytest.skip("Test data file not available")
        
        reader = IO.open([str(SAMPLE_NITF21)], "r")
        keys = reader.get_asset_keys()
        assert len(keys) == 1
        
        reader.close()
        
        # After close, reader should not be usable
        with pytest.raises(Exception):
            reader.get_asset_keys()
