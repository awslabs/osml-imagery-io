"""Structural-feature test: signed-integer field sign preservation.

Exercises the ``s2``/``s4``/``s8`` KSY types through the public dict API using a
synthetic, unit-only ``.ksy`` written to ``tmp_path``. This is the structure-
agnostic engine behavior, so the fixture is not a real TRE.

Regression coverage for BUG_SIGNED_INT_STORED_AS_UNSIGNED.md: signed integer
fields holding negative values were bit-cast into an unsigned representation, so
a negative field read back as a huge positive magnitude. A dedicated signed
representation now preserves the sign end to end.

Ported from the removed ``tests/unit/tre/test_signed_int_parsing.py``. The prior
file drove reads through ``StructureAccessor``/``Value.as_int()``/``as_float()``
and writes through ``StructureWriter`` — all removed from the public surface. The
``Value.as_int()``/``as_float()`` type-coercion assertions tested the shape of a
removed class, not a behavior the dict API has, so they are dropped; the semantic
coverage (negative ints survive decode and an encode round trip) is preserved via
``decode``/``encode``. Per the port rules, the decode direction uses externally-
built ``struct.pack`` blobs so ``decode`` is proved against ground-truth bytes
rather than circularly against ``encode``'s own output.
"""

import struct

import pytest
from aws.osml.io import StructureRegistry

# A minimal structure with one signed field of each size. ``size`` is omitted so
# it is derived from the type (see definition.rs size-from-type handling).
_SIGNED_KSY = """\
meta:
  id: signed_fields
  title: Signed Field Test Structure
  endian: be
seq:
  - id: DELTA2
    type: s2
  - id: OFFSET4
    type: s4
  - id: BIG8
    type: s8
"""


@pytest.fixture
def signed_definition(tmp_path):
    """Write the signed-field KSY to a fresh temp search path and load it.

    A fresh ``StructureRegistry`` bound only to ``tmp_path`` keeps this synthetic
    fixture out of the shared structure registry.
    """
    ksy_path = tmp_path / "signed_fields.ksy"
    ksy_path.write_text(_SIGNED_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("signed_fields")
    assert defn is not None, "signed_fields definition not found"
    return defn


def _build_blob(delta2: int, offset4: int, big8: int) -> bytes:
    """Externally build the input blob so decode is checked against ground truth."""
    return (
        struct.pack(">h", delta2)
        + struct.pack(">i", offset4)
        + struct.pack(">q", big8)
    )


class TestSignedIntDecode:
    """Negative signed values must survive ``decode`` (external-blob direction)."""

    def test_negative_s2(self, signed_definition):
        decoded = signed_definition.decode(_build_blob(-1, 0, 0))
        assert decoded["DELTA2"] == -1

    def test_negative_s4(self, signed_definition):
        decoded = signed_definition.decode(_build_blob(0, -123456, 0))
        assert decoded["OFFSET4"] == -123456

    def test_negative_s8(self, signed_definition):
        decoded = signed_definition.decode(_build_blob(0, 0, -9_000_000_000))
        assert decoded["BIG8"] == -9_000_000_000

    def test_all_negative(self, signed_definition):
        # Previously a negative field read back as ~1.8e19 garbage.
        decoded = signed_definition.decode(_build_blob(-2, -3, -4))
        assert decoded["DELTA2"] == -2
        assert decoded["OFFSET4"] == -3
        assert decoded["BIG8"] == -4

    def test_positive_values_unaffected(self, signed_definition):
        decoded = signed_definition.decode(_build_blob(7, 12345, 9_000_000_000))
        assert decoded["DELTA2"] == 7
        assert decoded["OFFSET4"] == 12345
        assert decoded["BIG8"] == 9_000_000_000

    def test_extremes(self, signed_definition):
        decoded = signed_definition.decode(
            _build_blob(-32768, -2147483648, -(2**63))
        )
        assert decoded["DELTA2"] == -32768
        assert decoded["OFFSET4"] == -2147483648
        assert decoded["BIG8"] == -(2**63)


class TestSignedIntRoundTrip:
    """An ``encode`` -> ``decode`` round trip preserves negative signed values."""

    @pytest.mark.parametrize(
        "delta2,offset4,big8",
        [
            (-1, -1, -1),
            (-32768, -2147483648, -9_000_000_000),
            (32767, 2147483647, 9_000_000_000),
            (0, 0, 0),
        ],
    )
    def test_write_read_round_trip(self, signed_definition, delta2, offset4, big8):
        raw = signed_definition.encode(
            {"DELTA2": delta2, "OFFSET4": offset4, "BIG8": big8}
        )
        # The write path must match the externally-built layout byte-for-byte.
        assert raw == _build_blob(delta2, offset4, big8)
        decoded = signed_definition.decode(raw)
        assert decoded["DELTA2"] == delta2
        assert decoded["OFFSET4"] == offset4
        assert decoded["BIG8"] == big8
