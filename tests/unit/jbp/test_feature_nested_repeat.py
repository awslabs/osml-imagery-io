"""Structural-feature test: nested ``types:`` + repeated sub-structures.

Exercises a repeated nested type through the public dict API using a synthetic,
unit-only ``.ksy`` written to ``tmp_path``. A repeated TypeRef field decodes to a
list of dicts (one per element) and encodes from the same shape, so this verifies
the list-of-dicts round trip and that the element count is driven by the count
field.

This is the structure-agnostic counterpart to the per-TRE nested-repeat anchors
(e.g. MITOCA's ``COMPONENTS``); 39 TREs in the corpus use nested types with
repeats.
"""

import pytest
from aws.osml.io import StructureRegistry

# NUM drives how many `point` sub-structures follow; each point is two BCS-N
# fields. TAIL follows the repeat so a miscount would misalign it.
_NEST_KSY = """\
meta:
  id: nest_repeat
  title: Nested Repeat Test Structure
  endian: be
types:
  point:
    seq:
      - id: X
        type: str
        size: 2
        encoding: BCS-N
      - id: Y
        type: str
        size: 2
        encoding: BCS-N
seq:
  - id: NUM
    type: str
    size: 2
    encoding: BCS-N
  - id: POINTS
    type: point
    repeat: expr
    repeat-expr: NUM.to_i
  - id: TAIL
    type: str
    size: 1
    encoding: BCS-A
"""


@pytest.fixture
def nest_definition(tmp_path):
    """Write the nested-repeat KSY to a fresh temp search path and load it."""
    ksy_path = tmp_path / "nest_repeat.ksy"
    ksy_path.write_text(_NEST_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("nest_repeat")
    assert defn is not None, "nest_repeat definition not found"
    return defn


class TestNestedRepeat:
    """A repeated nested type is a list of per-element dicts on both paths."""

    def test_two_elements(self, nest_definition):
        value = {
            "NUM": "02",
            "POINTS": [{"X": "01", "Y": "02"}, {"X": "03", "Y": "04"}],
            "TAIL": "!",
        }
        raw = nest_definition.encode(value)
        # NUM(2) + 2 points x (X 2 + Y 2) + TAIL(1) = 2 + 8 + 1 = 11 bytes.
        assert raw == b"0201020304!"
        decoded = nest_definition.decode(raw)
        assert decoded["POINTS"] == [
            {"X": "01", "Y": "02"},
            {"X": "03", "Y": "04"},
        ]
        assert decoded["TAIL"] == "!"

    def test_zero_elements(self, nest_definition):
        raw = nest_definition.encode({"NUM": "00", "POINTS": [], "TAIL": "!"})
        assert raw == b"00!"
        decoded = nest_definition.decode(raw)
        assert decoded["POINTS"] == []
        assert decoded["TAIL"] == "!"

    def test_element_count_tracks_count_field(self, nest_definition):
        # One extra element adds exactly one point (4 bytes).
        one = nest_definition.encode(
            {"NUM": "01", "POINTS": [{"X": "01", "Y": "02"}], "TAIL": "!"}
        )
        two = nest_definition.encode(
            {
                "NUM": "02",
                "POINTS": [{"X": "01", "Y": "02"}, {"X": "03", "Y": "04"}],
                "TAIL": "!",
            }
        )
        assert len(two) - len(one) == 4

    @pytest.mark.parametrize("count", [1, 2, 3])
    def test_round_trip(self, nest_definition, count):
        points = [{"X": f"{i:02d}", "Y": f"{i + 1:02d}"} for i in range(count)]
        raw = nest_definition.encode(
            {"NUM": f"{count:02d}", "POINTS": points, "TAIL": "!"}
        )
        assert nest_definition.encode(nest_definition.decode(raw)) == raw
