"""Unit tests for the data-driven binary parser Python bindings.

This module tests the Python bindings for StructureRegistry, StructureAccessor,
StructureWriter, and Value classes.

Requirements: 14.1, 14.2, 14.3, 14.4, 14.5, 14.6
"""

import mmap
from pathlib import Path

import pytest
from aws.osml.io import (
    StructureAccessor,
    StructureDefinition,
    StructureRegistry,
    StructureWriter,
    Value,
)

# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
STRUCTURES_DIR = Path("data/structures")
SYNTHETIC_NITF = UNIT_DATA_DIR / "nitf21-256x256-3band-8bit-nc.ntf"


# =============================================================================
# StructureRegistry Tests (Requirement 14.1)
# =============================================================================

class TestStructureRegistry:
    """Tests for StructureRegistry class."""

    def test_registry_creation(self):
        """Test creating a new registry with default search paths."""
        registry = StructureRegistry()
        assert registry is not None
        # Should have at least the default search path
        paths = registry.search_paths()
        assert isinstance(paths, list)

    def test_registry_add_search_path(self):
        """Test adding a custom search path."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        paths = registry.search_paths()
        assert str(STRUCTURES_DIR) in paths

    def test_registry_get_existing_definition(self):
        """Test getting an existing structure definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))

        definition = registry.get("nitf_02.10_file_header")
        assert definition is not None
        assert isinstance(definition, StructureDefinition)
        assert definition.id == "nitf_02_10_file_header"

    def test_registry_get_nonexistent_definition(self):
        """Test getting a non-existent definition returns None."""
        registry = StructureRegistry()
        definition = registry.get("NonExistentStructure")
        assert definition is None

    def test_registry_list(self):
        """Test listing available structure names."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))

        names = registry.list()
        assert isinstance(names, list)
        # Verify NITF structures are listed with new naming convention
        assert any("nitf_" in name for name in names)

    def test_registry_reload(self):
        """Test reloading definitions from disk."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))

        # Should not raise
        registry.reload()

        # Definitions should still be available
        definition = registry.get("nitf_02.10_file_header")
        assert definition is not None

    def test_registry_register_runtime_definition(self):
        """Test registering a definition at runtime."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))

        # Get an existing definition
        original = registry.get("nitf_02.10_file_header")
        assert original is not None

        # Register it under a new name
        registry.register("CustomDefinition", original)

        # Should be retrievable under the new name
        custom = registry.get("CustomDefinition")
        assert custom is not None
        assert custom.id == original.id


# =============================================================================
# StructureDefinition Tests
# =============================================================================

class TestStructureDefinition:
    """Tests for StructureDefinition class."""

    @pytest.fixture
    def nitf_definition(self):
        """Get the NITF file header definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        return registry.get("nitf_02.10_file_header")

    def test_definition_id(self, nitf_definition):
        """Test getting definition ID."""
        assert nitf_definition.id == "nitf_02_10_file_header"

    def test_definition_title(self, nitf_definition):
        """Test getting definition title."""
        assert nitf_definition.title == "NITF 2.1 File Header"

    def test_definition_field_names(self, nitf_definition):
        """Test getting field names."""
        field_names = nitf_definition.field_names
        assert isinstance(field_names, list)
        assert "FHDR" in field_names
        assert "FVER" in field_names
        assert "CLEVEL" in field_names

    def test_definition_len(self, nitf_definition):
        """Test getting number of fields."""
        assert len(nitf_definition) > 0


# =============================================================================
# StructureAccessor Tests (Requirement 14.2)
# =============================================================================

class TestStructureAccessor:
    """Tests for StructureAccessor class with dict-like access."""

    @pytest.fixture
    def nitf_definition(self):
        """Get the NITF file header definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        return registry.get("nitf_02.10_file_header")

    @pytest.fixture
    def synthetic_data(self):
        """Load synthetic NITF header data."""
        with open(SYNTHETIC_NITF, "rb") as f:
            return f.read()

    def test_accessor_creation(self, nitf_definition, synthetic_data):
        """Test creating an accessor from definition and data."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)
        assert accessor is not None

    def test_accessor_getitem(self, nitf_definition, synthetic_data):
        """Test dict-like access via __getitem__."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        # Access string fields
        fhdr = accessor["FHDR"]
        assert isinstance(fhdr, Value)
        assert fhdr.as_str() == "NITF"

        fver = accessor["FVER"]
        assert fver.as_str() == "02.10"

    def test_accessor_getitem_unknown_field(self, nitf_definition, synthetic_data):
        """Test accessing unknown field raises KeyError."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        with pytest.raises(KeyError):
            _ = accessor["nonexistent_field"]

    def test_accessor_has(self, nitf_definition, synthetic_data):
        """Test checking field existence with has()."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        assert accessor.has("FHDR") is True
        assert accessor.has("FVER") is True
        assert accessor.has("nonexistent") is False

    def test_accessor_contains(self, nitf_definition, synthetic_data):
        """Test 'in' operator support."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        assert "FHDR" in accessor
        assert "FVER" in accessor
        assert "nonexistent" not in accessor

    def test_accessor_fields(self, nitf_definition, synthetic_data):
        """Test iterating over accessible field paths."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        fields = accessor.fields()
        assert isinstance(fields, list)
        assert "FHDR" in fields
        assert "FVER" in fields
        assert "CLEVEL" in fields

    def test_accessor_numeric_field(self, nitf_definition, synthetic_data):
        """Test accessing numeric fields."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        # CLEVEL is a BCS-N field
        clevel = accessor["CLEVEL"]
        assert clevel.as_int() == 3

    def test_accessor_data_property(self, nitf_definition, synthetic_data):
        """Test getting underlying data buffer."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        data = accessor.data
        assert isinstance(data, bytes)
        assert len(data) == len(synthetic_data)

    def test_accessor_definition_property(self, nitf_definition, synthetic_data):
        """Test getting structure definition."""
        accessor = StructureAccessor(nitf_definition, synthetic_data)

        definition = accessor.definition
        assert isinstance(definition, StructureDefinition)
        assert definition.id == nitf_definition.id


# =============================================================================
# StructureWriter Tests (Requirement 14.3)
# =============================================================================

class TestStructureWriter:
    """Tests for StructureWriter class with dict-like write access."""

    @pytest.fixture
    def nitf_definition(self):
        """Get the NITF file header definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        return registry.get("nitf_02.10_file_header")

    def test_writer_new_streaming(self, nitf_definition):
        """Test creating a streaming writer."""
        writer = StructureWriter.new_streaming(nitf_definition)
        assert writer is not None

    def test_writer_setitem_streaming(self, nitf_definition):
        """Test dict-like write via __setitem__ in streaming mode."""
        writer = StructureWriter.new_streaming(nitf_definition)

        # Write string fields in order (streaming mode requires order)
        writer["FHDR"] = "NITF"
        writer["FVER"] = "02.10"

        # Check field is set
        assert writer.is_set("FHDR") is True
        assert writer.is_set("FVER") is True

    def test_writer_set_method(self, nitf_definition):
        """Test set() method."""
        writer = StructureWriter.new_streaming(nitf_definition)

        writer.set("FHDR", "NITF")
        assert writer.is_set("FHDR") is True

    def test_writer_is_set(self, nitf_definition):
        """Test checking if field has been written."""
        writer = StructureWriter.new_streaming(nitf_definition)

        assert writer.is_set("FHDR") is False
        writer["FHDR"] = "NITF"
        assert writer.is_set("FHDR") is True

    def test_writer_buffer(self, nitf_definition):
        """Test getting current buffer contents."""
        writer = StructureWriter.new_streaming(nitf_definition)
        writer["FHDR"] = "NITF"

        buffer = writer.buffer()
        assert isinstance(buffer, bytes)
        assert b"NITF" in buffer


# =============================================================================
# Value Tests (Requirement 14.4)
# =============================================================================

class TestValue:
    """Tests for Value class type conversions."""

    @pytest.fixture
    def accessor(self):
        """Create accessor with synthetic data."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        definition = registry.get("nitf_02.10_file_header")

        with open(SYNTHETIC_NITF, "rb") as f:
            data = f.read()

        return StructureAccessor(definition, data)

    def test_value_as_str(self, accessor):
        """Test as_str() conversion."""
        value = accessor["FHDR"]
        result = value.as_str()
        assert isinstance(result, str)
        assert result == "NITF"

    def test_value_as_str_trimmed(self, accessor):
        """Test as_str() trims trailing padding."""
        # FTITLE has trailing spaces
        value = accessor["FTITLE"]
        result = value.as_str()
        assert not result.endswith(" ")

    def test_value_as_int(self, accessor):
        """Test as_int() conversion for numeric strings."""
        value = accessor["CLEVEL"]
        result = value.as_int()
        assert isinstance(result, int)
        assert result == 3

    def test_value_as_int_with_leading_zeros(self, accessor):
        """Test as_int() handles leading zeros."""
        value = accessor["NUMI"]
        result = value.as_int()
        assert isinstance(result, int)
        assert result == 1

    def test_value_as_float(self, accessor):
        """Test as_float() conversion."""
        value = accessor["CLEVEL"]
        result = value.as_float()
        assert isinstance(result, float)
        assert result == 3.0

    def test_value_as_bytes(self, accessor):
        """Test as_bytes() conversion."""
        value = accessor["FHDR"]
        result = value.as_bytes()
        assert isinstance(result, bytes)
        assert result == b"NITF"

    def test_value_repr(self, accessor):
        """Test string representation of Value."""
        value = accessor["FHDR"]
        repr_str = repr(value)
        assert "Value" in repr_str

    def test_value_len(self, accessor):
        """Test len() on Value."""
        value = accessor["FHDR"]
        assert len(value) == 4


# =============================================================================
# Raw View Tests (Requirement 14.5)
# =============================================================================

class TestRawView:
    """Tests for raw_view() returning bytes."""

    @pytest.fixture
    def accessor(self):
        """Create accessor with synthetic data."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        definition = registry.get("nitf_02.10_file_header")

        with open(SYNTHETIC_NITF, "rb") as f:
            data = f.read()

        return StructureAccessor(definition, data)

    def test_raw_view_returns_bytes(self, accessor):
        """Test raw_view() returns bytes."""
        raw = accessor.raw_view("FHDR")
        assert isinstance(raw, bytes)
        assert raw == b"NITF"

    def test_raw_view_field_size(self, accessor):
        """Test raw_view() returns correct size for known fields."""
        raw = accessor.raw_view("FHDR")
        assert len(raw) == 4

        raw = accessor.raw_view("FVER")
        assert len(raw) == 5

    def test_raw_view_consistency(self, accessor):
        """Test raw_view returns correct bytes for different fields."""
        raw_fhdr = accessor.raw_view("FHDR")
        raw_fver = accessor.raw_view("FVER")

        assert raw_fhdr == b"NITF"
        assert raw_fver == b"02.10"


# =============================================================================
# Memory-Mapped File Tests (Requirement 14.6)
# =============================================================================

class TestMmapSupport:
    """Tests for memory-mapped file input support."""

    @pytest.fixture
    def nitf_definition(self):
        """Get the NITF file header definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        return registry.get("nitf_02.10_file_header")

    def test_accessor_with_mmap(self, nitf_definition):
        """Test creating accessor from memory-mapped file."""
        file_size = Path(SYNTHETIC_NITF).stat().st_size
        if file_size == 0:
            pytest.skip("Synthetic NITF file is empty; cannot mmap empty file on all platforms.")
        with open(SYNTHETIC_NITF, "rb") as f, \
             mmap.mmap(f.fileno(), file_size, access=mmap.ACCESS_READ) as mm:
            accessor = StructureAccessor(nitf_definition, mm)

            # Should be able to access fields
            fhdr = accessor["FHDR"]
            assert fhdr.as_str() == "NITF"

    def test_accessor_with_memoryview(self, nitf_definition):
        """Test creating accessor from memoryview."""
        with open(SYNTHETIC_NITF, "rb") as f:
            data = f.read()

        mv = memoryview(data)
        accessor = StructureAccessor(nitf_definition, mv)

        fhdr = accessor["FHDR"]
        assert fhdr.as_str() == "NITF"

    def test_accessor_with_bytearray(self, nitf_definition):
        """Test creating accessor from bytearray."""
        with open(SYNTHETIC_NITF, "rb") as f:
            data = bytearray(f.read())

        accessor = StructureAccessor(nitf_definition, data)

        fhdr = accessor["FHDR"]
        assert fhdr.as_str() == "NITF"


# =============================================================================
# Round-Trip Tests
# =============================================================================

class TestRoundTrip:
    """Tests for read-write round-trip consistency."""

    @pytest.fixture
    def nitf_definition(self):
        """Get the NITF file header definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        return registry.get("nitf_02.10_file_header")

    @pytest.fixture
    def synthetic_data(self):
        """Load synthetic NITF header data."""
        with open(SYNTHETIC_NITF, "rb") as f:
            return f.read()

    def test_read_write_simple_fields(self, nitf_definition, synthetic_data):
        """Test reading and writing simple string fields."""
        # Read original values
        accessor = StructureAccessor(nitf_definition, synthetic_data)
        original_fhdr = accessor["FHDR"].as_str()
        original_fver = accessor["FVER"].as_str()

        # Write to a new structure using streaming mode
        writer = StructureWriter.new_streaming(nitf_definition)
        writer["FHDR"] = original_fhdr
        writer["FVER"] = original_fver

        # Verify written values match
        buffer = writer.buffer()
        assert buffer[:4] == b"NITF"
        assert buffer[4:9] == b"02.10"
