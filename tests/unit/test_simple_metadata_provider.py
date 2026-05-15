"""Tests for BufferedMetadataProvider functionality.

This module tests the BufferedMetadataProvider implementation through the Python bindings,
including the Mapping/MutableMapping protocol (construction, __setitem__, __getitem__,
__delitem__, __contains__, __len__, __iter__, entries, update, clear).

Requirements: 1.7
"""

import collections.abc

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
        assert len(provider) == 0
        assert provider.entries() == {}

    def test_construction_from_existing_provider(self):
        """Test creating BufferedMetadataProvider from an existing provider."""
        # Create source provider with some data
        source = BufferedMetadataProvider()
        source["KEY1"] = "value1"
        source["KEY2"] = "value2"
        source["PREFIX_A"] = "a_value"

        # Create new provider from source
        copied = BufferedMetadataProvider(source=source)

        # Verify all data was copied
        assert copied["KEY1"] == "value1"
        assert copied["KEY2"] == "value2"
        assert copied["PREFIX_A"] == "a_value"

        # Verify entries() returns all pairs
        assert len(copied.entries()) == 3

    def test_construction_from_provider_is_independent(self):
        """Test that copied provider is independent from source."""
        source = BufferedMetadataProvider()
        source["KEY"] = "original"

        copied = BufferedMetadataProvider(source=source)

        # Modify source
        source["KEY"] = "modified"
        source["NEW_KEY"] = "new_value"

        # Copied should be unchanged
        assert copied["KEY"] == "original"
        assert copied.get("NEW_KEY") is None

    def test_is_metadata_provider(self):
        """Test that BufferedMetadataProvider is a MetadataProvider."""
        provider = BufferedMetadataProvider()
        assert hasattr(provider, 'entries')
        assert hasattr(provider, 'raw')

    def test_isinstance_mapping(self):
        """Test that MetadataProvider is registered as Mapping."""
        provider = BufferedMetadataProvider()
        assert isinstance(provider, collections.abc.Mapping)

    def test_isinstance_mutable_mapping(self):
        """Test that BufferedMetadataProvider is registered as MutableMapping."""
        provider = BufferedMetadataProvider()
        assert isinstance(provider, collections.abc.MutableMapping)


# =============================================================================
# __setitem__ / __getitem__ Tests
# =============================================================================

class TestSetGetOperations:
    """Tests for __setitem__ and __getitem__ operations."""

    def test_set_and_get_single_value(self):
        """Test setting and getting a single value."""
        provider = BufferedMetadataProvider()

        provider["imode"] = "B"
        result = provider["imode"]

        assert result == "B"

    def test_set_overwrites_existing_value(self):
        """Test that __setitem__ overwrites existing values."""
        provider = BufferedMetadataProvider()

        provider["imode"] = "B"
        assert provider["imode"] == "B"

        provider["imode"] = "P"
        assert provider["imode"] == "P"

    def test_getitem_nonexistent_key_raises_keyerror(self):
        """Test that __getitem__ raises KeyError for nonexistent keys."""
        provider = BufferedMetadataProvider()

        try:
            provider["nonexistent"]
            assert False, "Expected KeyError"
        except KeyError:
            pass

    def test_get_nonexistent_key_returns_none(self):
        """Test that get() returns None for nonexistent keys."""
        provider = BufferedMetadataProvider()

        result = provider.get("nonexistent")
        assert result is None

    def test_get_with_default(self):
        """Test that get() returns the default when key is absent."""
        provider = BufferedMetadataProvider()

        result = provider.get("missing", "fallback")
        assert result == "fallback"

    def test_set_multiple_values(self):
        """Test setting multiple key-value pairs."""
        provider = BufferedMetadataProvider()

        provider["imode"] = "B"
        provider["ic"] = "NC"
        provider["nppbh"] = "256"
        provider["nppbv"] = "256"

        assert provider["imode"] == "B"
        assert provider["ic"] == "NC"
        assert provider["nppbh"] == "256"
        assert provider["nppbv"] == "256"

    def test_set_empty_string_value(self):
        """Test setting an empty string value."""
        provider = BufferedMetadataProvider()

        provider["empty"] = ""
        result = provider["empty"]

        assert result == ""

    def test_set_value_with_spaces(self):
        """Test setting a value containing spaces."""
        provider = BufferedMetadataProvider()

        provider["title"] = "Test Image Title"
        result = provider["title"]

        assert result == "Test Image Title"

    def test_set_native_types(self):
        """Test setting various native Python types."""
        provider = BufferedMetadataProvider()

        provider["str_key"] = "hello"
        provider["int_key"] = 42
        provider["float_key"] = 3.14
        provider["bool_key"] = True
        provider["none_key"] = None
        provider["list_key"] = [1, 2, 3]
        provider["dict_key"] = {"nested": "value"}

        assert provider["str_key"] == "hello"
        assert provider["int_key"] == 42
        assert provider["float_key"] == 3.14
        assert provider["bool_key"] is True
        assert provider["none_key"] is None
        assert provider["list_key"] == [1, 2, 3]
        assert provider["dict_key"] == {"nested": "value"}


# =============================================================================
# __contains__ Tests
# =============================================================================

class TestContainsOperator:
    """Tests for 'in' operator (__contains__)."""

    def test_contains_existing_key(self):
        """Test that 'in' returns True for existing keys."""
        provider = BufferedMetadataProvider()
        provider["KEY"] = "value"

        assert "KEY" in provider

    def test_contains_nonexistent_key(self):
        """Test that 'in' returns False for missing keys."""
        provider = BufferedMetadataProvider()

        assert "MISSING" not in provider


# =============================================================================
# __len__ Tests
# =============================================================================

class TestLenOperation:
    """Tests for len()."""

    def test_len_empty(self):
        """Test len on empty provider."""
        provider = BufferedMetadataProvider()
        assert len(provider) == 0

    def test_len_with_entries(self):
        """Test len reflects the number of entries."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"
        provider["c"] = "3"

        assert len(provider) == 3


# =============================================================================
# __iter__ Tests
# =============================================================================

class TestIterOperation:
    """Tests for iteration."""

    def test_iter_yields_keys(self):
        """Test that iterating yields all keys."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"
        provider["c"] = "3"

        keys = list(provider)
        assert set(keys) == {"a", "b", "c"}

    def test_iter_empty(self):
        """Test iteration on empty provider yields nothing."""
        provider = BufferedMetadataProvider()
        assert list(provider) == []


# =============================================================================
# __bool__ Tests
# =============================================================================

class TestBoolOperation:
    """Tests for bool()."""

    def test_bool_empty_is_false(self):
        """Test that empty provider is falsy."""
        provider = BufferedMetadataProvider()
        assert not provider

    def test_bool_nonempty_is_true(self):
        """Test that non-empty provider is truthy."""
        provider = BufferedMetadataProvider()
        provider["key"] = "value"
        assert provider


# =============================================================================
# keys / values / items Tests
# =============================================================================

class TestKeysValuesItems:
    """Tests for keys(), values(), items()."""

    def test_keys_returns_list(self):
        """Test that keys() returns a list of keys."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"

        keys = provider.keys()
        assert isinstance(keys, list)
        assert set(keys) == {"a", "b"}

    def test_values_returns_list(self):
        """Test that values() returns a list of values."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"

        values = provider.values()
        assert isinstance(values, list)
        assert set(values) == {"1", "2"}

    def test_items_returns_list_of_tuples(self):
        """Test that items() returns a list of (key, value) tuples."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"

        items = provider.items()
        assert isinstance(items, list)
        assert set(items) == {("a", "1"), ("b", "2")}


# =============================================================================
# __delitem__ Tests
# =============================================================================

class TestDelitemOperation:
    """Tests for del (remove) operation."""

    def test_delitem_existing_key(self):
        """Test deleting an existing key."""
        provider = BufferedMetadataProvider()
        provider["key"] = "value"

        del provider["key"]

        assert "key" not in provider
        assert provider.get("key") is None

    def test_delitem_nonexistent_key_raises_keyerror(self):
        """Test deleting a nonexistent key raises KeyError."""
        provider = BufferedMetadataProvider()

        try:
            del provider["nonexistent"]
            assert False, "Expected KeyError"
        except KeyError:
            pass

    def test_delitem_does_not_affect_other_keys(self):
        """Test that del only affects the specified key."""
        provider = BufferedMetadataProvider()
        provider["key1"] = "value1"
        provider["key2"] = "value2"

        del provider["key1"]

        assert "key1" not in provider
        assert provider["key2"] == "value2"


# =============================================================================
# update Tests
# =============================================================================

class TestUpdateOperation:
    """Tests for update() method."""

    def test_update_from_dict(self):
        """Test bulk update from a dictionary."""
        provider = BufferedMetadataProvider()
        provider.update({"a": "1", "b": "2", "c": "3"})

        assert provider["a"] == "1"
        assert provider["b"] == "2"
        assert provider["c"] == "3"
        assert len(provider) == 3

    def test_update_overwrites(self):
        """Test that update overwrites existing values."""
        provider = BufferedMetadataProvider()
        provider["a"] = "old"

        provider.update({"a": "new"})

        assert provider["a"] == "new"


# =============================================================================
# clear Tests
# =============================================================================

class TestClearOperation:
    """Tests for clear() operation."""

    def test_clear_removes_all_keys(self):
        """Test that clear() removes all key-value pairs."""
        provider = BufferedMetadataProvider()
        provider["key1"] = "value1"
        provider["key2"] = "value2"
        provider["key3"] = "value3"

        provider.clear()

        assert "key1" not in provider
        assert "key2" not in provider
        assert "key3" not in provider
        assert len(provider) == 0

    def test_clear_on_empty_provider(self):
        """Test that clear() on empty provider doesn't raise."""
        provider = BufferedMetadataProvider()

        # Should not raise
        provider.clear()

        assert len(provider) == 0


# =============================================================================
# entries Tests
# =============================================================================

class TestEntries:
    """Tests for entries() method."""

    def test_entries_returns_all_pairs(self):
        """Test that entries() without prefix returns all pairs."""
        provider = BufferedMetadataProvider()
        provider["imode"] = "B"
        provider["ic"] = "NC"
        provider["nppbh"] = "256"

        result = provider.entries()

        assert isinstance(result, dict)
        assert len(result) == 3
        assert result["imode"] == "B"
        assert result["ic"] == "NC"
        assert result["nppbh"] == "256"

    def test_entries_with_prefix_filters_keys(self):
        """Test that entries() with prefix filters keys correctly."""
        provider = BufferedMetadataProvider()
        provider["img_imode"] = "B"
        provider["img_ic"] = "NC"
        provider["file_header"] = "NITF"
        provider["file_version"] = "02.10"

        # Get only img_ prefixed keys
        img_fields = provider.entries("img_")

        assert len(img_fields) == 2
        assert "img_imode" in img_fields
        assert "img_ic" in img_fields
        assert "file_header" not in img_fields
        assert "file_version" not in img_fields

    def test_entries_with_prefix_no_matches(self):
        """Test entries() with prefix that matches no keys."""
        provider = BufferedMetadataProvider()
        provider["key1"] = "value1"
        provider["key2"] = "value2"

        result = provider.entries("nonexistent_")

        assert isinstance(result, dict)
        assert len(result) == 0

    def test_entries_with_empty_prefix(self):
        """Test entries() with empty string prefix returns all keys."""
        provider = BufferedMetadataProvider()
        provider["key1"] = "value1"
        provider["key2"] = "value2"

        result = provider.entries("")

        # Empty prefix should match all keys (all keys start with "")
        assert len(result) == 2

    def test_entries_on_empty_provider(self):
        """Test entries() on empty provider returns empty dict."""
        provider = BufferedMetadataProvider()

        result = provider.entries()

        assert isinstance(result, dict)
        assert len(result) == 0


# =============================================================================
# dict() conversion Tests
# =============================================================================

class TestDictConversion:
    """Tests for dict(provider) conversion via Mapping protocol."""

    def test_dict_conversion(self):
        """Test that dict(provider) works via Mapping protocol."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"

        d = dict(provider)
        assert d == {"a": "1", "b": "2"}

    def test_dict_equals_entries(self):
        """Test that dict(provider) equals provider.entries()."""
        provider = BufferedMetadataProvider()
        provider["x"] = "hello"
        provider["y"] = 42
        provider["z"] = [1, 2, 3]

        assert dict(provider) == provider.entries()


# =============================================================================
# __repr__ Tests
# =============================================================================

class TestRepr:
    """Tests for __repr__."""

    def test_repr_shows_class_name(self):
        """Test that repr contains the class name."""
        provider = BufferedMetadataProvider()
        provider["IC"] = "NC"

        r = repr(provider)
        assert "BufferedMetadataProvider" in r

    def test_repr_shows_field_count(self):
        """Test that repr shows the field count."""
        provider = BufferedMetadataProvider()
        provider["a"] = "1"
        provider["b"] = "2"

        r = repr(provider)
        assert "2 fields" in r


# =============================================================================
# Raw Property Tests
# =============================================================================

class TestRawProperty:
    """Tests for raw property."""

    def test_raw_returns_bytes_io(self):
        """Test that raw property returns a BytesIO-like object."""
        provider = BufferedMetadataProvider()
        provider["key"] = "value"

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
        provider["IMODE"] = "P"
        provider["IC"] = "NC"
        provider["NPPBH"] = "512"
        provider["NPPBV"] = "512"

        # Verify all hints are set
        hints = provider.entries()
        assert hints["IMODE"] == "P"
        assert hints["IC"] == "NC"
        assert hints["NPPBH"] == "512"
        assert hints["NPPBV"] == "512"

    def test_modify_copied_metadata(self):
        """Test workflow of copying and modifying metadata."""
        # Create original with some values
        original = BufferedMetadataProvider()
        original["IMODE"] = "B"
        original["IC"] = "NC"
        original["title"] = "Original Title"

        # Copy and modify
        modified = BufferedMetadataProvider(source=original)
        modified["IMODE"] = "P"  # Change IMODE
        modified["COMRAT"] = "01.0"  # Add new field

        # Verify original unchanged
        assert original["IMODE"] == "B"
        assert original.get("COMRAT") is None

        # Verify modified has changes
        assert modified["IMODE"] == "P"
        assert modified["IC"] == "NC"  # Preserved
        assert modified["title"] == "Original Title"  # Preserved
        assert modified["COMRAT"] == "01.0"  # Added
