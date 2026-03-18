"""Tests for BufferedMetadataProvider functionality.

This module tests the BufferedMetadataProvider implementation through the Python bindings,
including construction, set/get/remove/clear operations, and as_dict with prefix filtering.

Requirements: 1.7
"""


from aws.osml.io import BufferedMetadataProvider

# =============================================================================
# Construction Tests
# =============================================================================

class TestBufferedMetadataProviderConstruction:
    """Tests for BufferedMetadataProvider construction."""

    def test_empty_construction(self):
        """Test creating an empty BufferedMetadataProvider."""
        provider = BufferedMetadataProvider()
        assert provider is not None

        # Empty provider should have no keys
        all_data = provider.as_dict()
        assert isinstance(all_data, dict)
        assert len(all_data) == 0

    def test_construction_from_existing_provider(self):
        """Test creating BufferedMetadataProvider from an existing provider."""
        # Create source provider with some data
        source = BufferedMetadataProvider()
        source.set("KEY1", "value1")
        source.set("KEY2", "value2")
        source.set("PREFIX_A", "a_value")

        # Create new provider from source
        copied = BufferedMetadataProvider(source=source)

        # Verify all data was copied
        assert copied.get("KEY1") == "value1"
        assert copied.get("KEY2") == "value2"
        assert copied.get("PREFIX_A") == "a_value"

        # Verify as_dict returns all pairs
        all_data = copied.as_dict()
        assert len(all_data) == 3

    def test_construction_from_provider_is_independent(self):
        """Test that copied provider is independent from source."""
        source = BufferedMetadataProvider()
        source.set("KEY", "original")

        copied = BufferedMetadataProvider(source=source)

        # Modify source
        source.set("KEY", "modified")
        source.set("NEW_KEY", "new_value")

        # Copied should be unchanged
        assert copied.get("KEY") == "original"
        assert copied.get("NEW_KEY") is None

    def test_is_metadata_provider(self):
        """Test that BufferedMetadataProvider is a MetadataProvider."""
        provider = BufferedMetadataProvider()
        # BufferedMetadataProvider extends MetadataProvider, so it should have
        # all MetadataProvider methods
        assert hasattr(provider, 'as_dict')
        assert hasattr(provider, 'raw')


# =============================================================================
# Set/Get Operations Tests
# =============================================================================

class TestSetGetOperations:
    """Tests for set() and get() operations."""

    def test_set_and_get_single_value(self):
        """Test setting and getting a single value."""
        provider = BufferedMetadataProvider()

        provider.set("imode", "B")
        result = provider.get("imode")

        assert result == "B"

    def test_set_overwrites_existing_value(self):
        """Test that set() overwrites existing values."""
        provider = BufferedMetadataProvider()

        provider.set("imode", "B")
        assert provider.get("imode") == "B"

        provider.set("imode", "P")
        assert provider.get("imode") == "P"

    def test_get_nonexistent_key_returns_none(self):
        """Test that get() returns None for nonexistent keys."""
        provider = BufferedMetadataProvider()

        result = provider.get("nonexistent")
        assert result is None

    def test_set_multiple_values(self):
        """Test setting multiple key-value pairs."""
        provider = BufferedMetadataProvider()

        provider.set("imode", "B")
        provider.set("ic", "NC")
        provider.set("nppbh", "256")
        provider.set("nppbv", "256")

        assert provider.get("imode") == "B"
        assert provider.get("ic") == "NC"
        assert provider.get("nppbh") == "256"
        assert provider.get("nppbv") == "256"

    def test_set_empty_string_value(self):
        """Test setting an empty string value."""
        provider = BufferedMetadataProvider()

        provider.set("empty", "")
        result = provider.get("empty")

        assert result == ""

    def test_set_value_with_spaces(self):
        """Test setting a value containing spaces."""
        provider = BufferedMetadataProvider()

        provider.set("title", "Test Image Title")
        result = provider.get("title")

        assert result == "Test Image Title"


# =============================================================================
# Remove Operation Tests
# =============================================================================

class TestRemoveOperation:
    """Tests for remove() operation."""

    def test_remove_existing_key(self):
        """Test removing an existing key."""
        provider = BufferedMetadataProvider()
        provider.set("key", "value")

        removed = provider.remove("key")

        assert removed == "value"
        assert provider.get("key") is None

    def test_remove_nonexistent_key(self):
        """Test removing a nonexistent key returns None."""
        provider = BufferedMetadataProvider()

        removed = provider.remove("nonexistent")

        assert removed is None

    def test_remove_does_not_affect_other_keys(self):
        """Test that remove() only affects the specified key."""
        provider = BufferedMetadataProvider()
        provider.set("key1", "value1")
        provider.set("key2", "value2")

        provider.remove("key1")

        assert provider.get("key1") is None
        assert provider.get("key2") == "value2"


# =============================================================================
# Clear Operation Tests
# =============================================================================

class TestClearOperation:
    """Tests for clear() operation."""

    def test_clear_removes_all_keys(self):
        """Test that clear() removes all key-value pairs."""
        provider = BufferedMetadataProvider()
        provider.set("key1", "value1")
        provider.set("key2", "value2")
        provider.set("key3", "value3")

        provider.clear()

        assert provider.get("key1") is None
        assert provider.get("key2") is None
        assert provider.get("key3") is None
        assert len(provider.as_dict()) == 0

    def test_clear_on_empty_provider(self):
        """Test that clear() on empty provider doesn't raise."""
        provider = BufferedMetadataProvider()

        # Should not raise
        provider.clear()

        assert len(provider.as_dict()) == 0


# =============================================================================
# as_dict Tests
# =============================================================================

class TestAsDict:
    """Tests for as_dict() method."""

    def test_as_dict_returns_all_pairs(self):
        """Test that as_dict() without prefix returns all pairs."""
        provider = BufferedMetadataProvider()
        provider.set("imode", "B")
        provider.set("ic", "NC")
        provider.set("nppbh", "256")

        result = provider.as_dict()

        assert isinstance(result, dict)
        assert len(result) == 3
        assert result["imode"] == "B"
        assert result["ic"] == "NC"
        assert result["nppbh"] == "256"

    def test_as_dict_with_prefix_filters_keys(self):
        """Test that as_dict() with prefix filters keys correctly."""
        provider = BufferedMetadataProvider()
        provider.set("img_imode", "B")
        provider.set("img_ic", "NC")
        provider.set("file_header", "NITF")
        provider.set("file_version", "02.10")

        # Get only img_ prefixed keys
        img_fields = provider.as_dict("img_")

        assert len(img_fields) == 2
        assert "img_imode" in img_fields
        assert "img_ic" in img_fields
        assert "file_header" not in img_fields
        assert "file_version" not in img_fields

    def test_as_dict_with_prefix_no_matches(self):
        """Test as_dict() with prefix that matches no keys."""
        provider = BufferedMetadataProvider()
        provider.set("key1", "value1")
        provider.set("key2", "value2")

        result = provider.as_dict("nonexistent_")

        assert isinstance(result, dict)
        assert len(result) == 0

    def test_as_dict_with_empty_prefix(self):
        """Test as_dict() with empty string prefix returns all keys."""
        provider = BufferedMetadataProvider()
        provider.set("key1", "value1")
        provider.set("key2", "value2")

        result = provider.as_dict("")

        # Empty prefix should match all keys (all keys start with "")
        assert len(result) == 2

    def test_as_dict_on_empty_provider(self):
        """Test as_dict() on empty provider returns empty dict."""
        provider = BufferedMetadataProvider()

        result = provider.as_dict()

        assert isinstance(result, dict)
        assert len(result) == 0


# =============================================================================
# Raw Property Tests
# =============================================================================

class TestRawProperty:
    """Tests for raw property."""

    def test_raw_returns_bytes_io(self):
        """Test that raw property returns a BytesIO-like object."""
        provider = BufferedMetadataProvider()
        provider.set("key", "value")

        raw_io = provider.raw
        data = raw_io.read()

        # BufferedMetadataProvider returns empty bytes for raw
        assert isinstance(data, bytes)


# =============================================================================
# Integration Tests
# =============================================================================

class TestIntegration:
    """Integration tests for BufferedMetadataProvider."""

    def test_typical_encoding_hints_workflow(self):
        """Test typical workflow of setting encoding hints."""
        provider = BufferedMetadataProvider()

        # Set encoding hints (uppercase field names match .ksy definitions)
        provider.set("IMODE", "P")
        provider.set("IC", "NC")
        provider.set("NPPBH", "512")
        provider.set("NPPBV", "512")

        # Verify all hints are set
        hints = provider.as_dict()
        assert hints["IMODE"] == "P"
        assert hints["IC"] == "NC"
        assert hints["NPPBH"] == "512"
        assert hints["NPPBV"] == "512"

    def test_modify_copied_metadata(self):
        """Test workflow of copying and modifying metadata."""
        # Create original with some values
        original = BufferedMetadataProvider()
        original.set("IMODE", "B")
        original.set("IC", "NC")
        original.set("title", "Original Title")

        # Copy and modify
        modified = BufferedMetadataProvider(source=original)
        modified.set("IMODE", "P")  # Change IMODE
        modified.set("COMRAT", "01.0")  # Add new field

        # Verify original unchanged
        assert original.get("IMODE") == "B"
        assert original.get("COMRAT") is None

        # Verify modified has changes
        assert modified.get("IMODE") == "P"
        assert modified.get("IC") == "NC"  # Preserved
        assert modified.get("title") == "Original Title"  # Preserved
        assert modified.get("COMRAT") == "01.0"  # Added
