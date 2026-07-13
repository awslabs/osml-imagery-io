"""Structural-feature test: count/list atomicity on encode.

Exercises the engine-wide invariant that a repeated field's supplied element
count must agree with the count field that drives ``repeat-expr``. Uses a
synthetic, unit-only ``.ksy`` written to ``tmp_path`` and drives everything
through the public dict API.

If the declared count exceeds the supplied list, ``encode`` cannot satisfy the
remaining elements and raises (a missing-required error). If the supplied list
exceeds the declared count, ``encode`` writes past the last expected field and
raises (a no-more-fields error). Either way, ``encode`` refuses to emit a record
whose length field disagrees with its element list, rather than silently
producing a self-inconsistent blob. This atomicity is what a self-consistent
round trip cannot catch.
"""

import pytest
from aws.osml.io import StructureRegistry

# NUM drives the element count of ITEMS via repeat-expr.
_COUNT_KSY = """\
meta:
  id: count_atom
  title: Count-Atomicity Test Structure
  endian: be
seq:
  - id: NUM
    type: str
    size: 2
    encoding: BCS-N
  - id: ITEMS
    type: str
    size: 3
    encoding: BCS-A
    repeat: expr
    repeat-expr: NUM.to_i
"""


@pytest.fixture
def count_definition(tmp_path):
    """Write the count-atomicity KSY to a fresh temp search path and load it."""
    ksy_path = tmp_path / "count_atom.ksy"
    ksy_path.write_text(_COUNT_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("count_atom")
    assert defn is not None, "count_atom definition not found"
    return defn


class TestCountAtomicity:
    """``encode`` rejects a count field that disagrees with the element list."""

    def test_matching_count_encodes(self, count_definition):
        raw = count_definition.encode({"NUM": "02", "ITEMS": ["abc", "def"]})
        assert raw == b"02abcdef"
        assert count_definition.decode(raw)["ITEMS"] == ["abc", "def"]

    def test_declared_count_exceeds_list_raises(self, count_definition):
        # NUM says 3 but only 2 items supplied: the third element is missing.
        with pytest.raises(Exception):
            count_definition.encode({"NUM": "03", "ITEMS": ["abc", "def"]})

    def test_list_exceeds_declared_count_raises(self, count_definition):
        # NUM says 1 but 2 items supplied: the second element has no field slot.
        with pytest.raises(Exception):
            count_definition.encode({"NUM": "01", "ITEMS": ["abc", "def"]})

    def test_zero_count_matches_empty_list(self, count_definition):
        raw = count_definition.encode({"NUM": "00", "ITEMS": []})
        assert raw == b"00"
        assert count_definition.decode(raw)["ITEMS"] == []
