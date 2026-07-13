"""Unit tests for the data-driven binary parser Python bindings.

Tests the generic binding surface — ``StructureRegistry`` /
``StructureDefinition`` and the ``encode``/``decode`` dict API — against the
NITF 2.1 file header. This is the structure-agnostic engine, not a TRE/DES
spec-fidelity anchor, so it lives here rather than under ``tests/unit/jbp/``.

Ported to the dict API (``DESIGN_TRE_DES_TESTING_REDESIGN.md`` Phase 3). The
path-based ``StructureAccessor`` / ``StructureWriter`` / ``Value`` classes are
removed from the public surface, so:

- Accessor reads (``accessor["FHDR"].as_str()``, ``.as_int()``, repeated
  ``.as_array()``) are ported to ``decode``, which returns a plain dict — scalars
  as strings, repeated fields as lists.
- Writer round-trips (``StructureWriter.new_streaming`` + ``set``/``__setitem__``
  + ``finish``/``buffer``) are ported to ``encode``, which returns the full
  encoded bytes in one call.
- Decode-input variety (``mmap`` / ``memoryview`` / ``bytearray``) — real
  end-user behavior — is retained, exercised through ``decode``.

Dropped, each because it asserted the *shape* of a removed class rather than a
behavior the dict API has (semantic coverage is preserved via ``decode`` /
``encode`` where it maps to a real behavior):

- ``Value.as_str()`` trailing-space trimming, ``as_int()`` / ``as_float()``
  numeric coercion, ``as_bytes()``, ``repr``, ``__len__`` — ``decode`` returns
  the raw field string; type coercion and the ``Value`` wrapper are gone.
- ``accessor.has()`` / ``in`` / ``.fields()`` / ``.data`` / ``.definition`` and
  ``raw_view()`` — dict membership (``in`` on the decoded dict) covers field
  presence; the accessor object and its buffer/definition/raw-view accessors are
  gone.
- ``writer.is_set()`` / ``.buffer()`` (partial-write introspection) — ``encode``
  is a single all-fields call with no incremental-write surface.
"""

import mmap
from pathlib import Path

import pytest
from aws.osml.io import StructureDefinition, StructureRegistry

# =============================================================================
# Test Data Paths
# =============================================================================

UNIT_DATA_DIR = Path("data/unit")
STRUCTURES_DIR = Path("data/structures")
SYNTHETIC_NITF = UNIT_DATA_DIR / "nitf21-256x256-3band-8bit-nc.ntf"

# IMRFCA has 4 repeated BCS-N fields, each with repeat-expr: 20 and size: 22.
_IMRFCA_ELEM = "0" * 22
_IMRFCA_LIST = [_IMRFCA_ELEM] * 20
_IMRFCA_TUPLE = tuple(_IMRFCA_LIST)
_IMRFCA_RAW = (_IMRFCA_ELEM.encode() * 20) * 4  # 4 fields × 20 elems × 22 bytes


# =============================================================================
# StructureRegistry Tests
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
# Decode (read path) Tests — ported from StructureAccessor / Value
# =============================================================================

class TestDecode:
    """Reads via ``decode``, which returns a plain dict.

    Ported from the removed ``StructureAccessor``/``Value`` read path. ``decode``
    returns each field as its raw string (no ``Value`` wrapper, no numeric
    coercion or trailing-space trimming — those were ``Value``-shape behaviors),
    and repeated fields as lists.
    """

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

    def test_decode_string_fields(self, nitf_definition, synthetic_data):
        """String fields decode to their raw values."""
        decoded = nitf_definition.decode(synthetic_data)
        assert decoded["FHDR"] == "NITF"
        assert decoded["FVER"] == "02.10"

    def test_decode_field_membership(self, nitf_definition, synthetic_data):
        """``in`` on the decoded dict reports field presence.

        Replaces the removed ``accessor.has()`` / ``in accessor`` /
        ``accessor.fields()`` surface.
        """
        decoded = nitf_definition.decode(synthetic_data)
        assert "FHDR" in decoded
        assert "FVER" in decoded
        assert "CLEVEL" in decoded
        assert "nonexistent_field" not in decoded

    def test_decode_numeric_field(self, nitf_definition, synthetic_data):
        """BCS-N fields decode to their raw digit string (no int coercion).

        CLEVEL is a 2-byte BCS-N field holding ``"03"``; the removed
        ``Value.as_int()`` coercion (→ ``3``) is not a dict-API behavior.
        """
        decoded = nitf_definition.decode(synthetic_data)
        assert decoded["CLEVEL"] == "03"
        assert decoded["NUMI"] == "001"

    def test_decode_repeated_field_returns_list(self):
        """A repeated field decodes to a list of element strings.

        Replaces the removed ``Value.as_array()`` -> ``list[Value]`` path.
        """
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        defn = registry.get("tre_imrfca")

        decoded = defn.decode(_IMRFCA_RAW)
        elements = decoded["XINC"]

        assert isinstance(elements, list)
        assert len(elements) == 20
        assert all(elem == _IMRFCA_ELEM for elem in elements)


# =============================================================================
# Encode (write path) Tests — ported from StructureWriter
# =============================================================================

class TestEncode:
    """Writes via ``encode``, which returns the full encoded bytes in one call.

    Ported from the removed ``StructureWriter`` streaming path. ``encode`` has no
    incremental-write surface (no ``is_set``/``buffer``), so those introspection
    assertions are dropped; the byte-level result is asserted directly.
    """

    def test_encode_repeated_field_list(self):
        """``encode`` accepts a list for a repeated field."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        defn = registry.get("tre_imrfca")

        raw = defn.encode(
            {
                "XINC": _IMRFCA_LIST,
                "XIDC": _IMRFCA_LIST,
                "YINC": _IMRFCA_LIST,
                "YIDC": _IMRFCA_LIST,
            }
        )
        assert raw == _IMRFCA_RAW

    def test_encode_repeated_field_tuple(self):
        """``encode`` accepts a tuple for a repeated field."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        defn = registry.get("tre_imrfca")

        raw = defn.encode(
            {
                "XINC": _IMRFCA_TUPLE,
                "XIDC": _IMRFCA_TUPLE,
                "YINC": _IMRFCA_TUPLE,
                "YIDC": _IMRFCA_TUPLE,
            }
        )
        assert raw == _IMRFCA_RAW


# =============================================================================
# Round-Trip Tests
# =============================================================================

class TestRoundTrip:
    """Read-write round-trip consistency via ``decode`` / ``encode``."""

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

    def test_header_round_trip(self, nitf_definition, synthetic_data):
        """``encode(decode(header)) == header`` for the NITF file header.

        The header's conditional fields (e.g. UDHOFL gated on UDHDL) require the
        full field set, so this re-encodes the whole decoded dict — a stronger
        check than the old writer test, which only wrote FHDR/FVER and inspected
        a buffer prefix.
        """
        decoded = nitf_definition.decode(synthetic_data)
        raw = nitf_definition.encode(decoded)
        assert raw[:4] == b"NITF"
        assert raw[4:9] == b"02.10"
        # Header prefix reproduces the on-disk bytes exactly.
        assert synthetic_data[: len(raw)] == raw

    def test_repeated_field_round_trip(self):
        """A list written for a repeated field reads back as an equal list."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        defn = registry.get("tre_imrfca")

        raw = defn.encode(
            {
                "XINC": _IMRFCA_LIST,
                "XIDC": _IMRFCA_LIST,
                "YINC": _IMRFCA_LIST,
                "YIDC": _IMRFCA_LIST,
            }
        )
        decoded = defn.decode(raw)
        assert decoded["XINC"] == _IMRFCA_LIST


# =============================================================================
# Memory-Mapped File / Buffer-Protocol Input Tests
# =============================================================================

class TestMmapSupport:
    """``decode`` accepts any buffer-protocol input — real end-user behavior."""

    @pytest.fixture
    def nitf_definition(self):
        """Get the NITF file header definition."""
        registry = StructureRegistry()
        registry.add_search_path(str(STRUCTURES_DIR))
        return registry.get("nitf_02.10_file_header")

    def test_decode_from_mmap(self, nitf_definition):
        """Decode input from a memory-mapped file."""
        file_size = Path(SYNTHETIC_NITF).stat().st_size
        if file_size == 0:
            pytest.skip("Synthetic NITF file is empty; cannot mmap empty file on all platforms.")
        with open(SYNTHETIC_NITF, "rb") as f, \
             mmap.mmap(f.fileno(), file_size, access=mmap.ACCESS_READ) as mm:
            decoded = nitf_definition.decode(mm)
            assert decoded["FHDR"] == "NITF"

    def test_decode_from_memoryview(self, nitf_definition):
        """Decode input from a memoryview."""
        with open(SYNTHETIC_NITF, "rb") as f:
            data = f.read()

        decoded = nitf_definition.decode(memoryview(data))
        assert decoded["FHDR"] == "NITF"

    def test_decode_from_bytearray(self, nitf_definition):
        """Decode input from a bytearray."""
        with open(SYNTHETIC_NITF, "rb") as f:
            data = bytearray(f.read())

        decoded = nitf_definition.decode(data)
        assert decoded["FHDR"] == "NITF"
