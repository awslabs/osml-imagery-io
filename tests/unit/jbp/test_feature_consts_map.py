"""Structural-feature test: ``consts:`` map + ``[]`` subscript.

Exercises a field whose byte width is looked up from a file-level ``consts:`` map
subscripted by an earlier field's value (``size: width_map[CODE]``) through the
public dict API using a synthetic, unit-only ``.ksy`` written to ``tmp_path``.
The width is chosen at parse time by the const-map entry the ``CODE`` field
selects, so a different ``CODE`` yields a different payload width.

This is the structure-agnostic counterpart to SENSRB's ``sensrb_value_widths``
const-map subscript (the sole corpus user of this construct).
"""

import pytest
from aws.osml.io import StructureRegistry

# CODE selects an entry in width_map; VAL is sized by that entry. TAIL follows so
# a wrong subscript lookup would misalign it.
_CONST_KSY = """\
meta:
  id: const_map
  title: Consts-Map Subscript Test Structure
  endian: be
consts:
  width_map:
    "A": 3
    "B": 5
    "C": 1
seq:
  - id: CODE
    type: str
    size: 1
    encoding: BCS-A
  - id: VAL
    type: str
    size: width_map[CODE]
    encoding: BCS-A
  - id: TAIL
    type: str
    size: 1
    encoding: BCS-A
"""


@pytest.fixture
def const_definition(tmp_path):
    """Write the consts-map KSY to a fresh temp search path and load it."""
    ksy_path = tmp_path / "const_map.ksy"
    ksy_path.write_text(_CONST_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("const_map")
    assert defn is not None, "const_map definition not found"
    return defn


class TestConstsMapSubscript:
    """The subscripted const-map entry sets the following field's width."""

    def test_code_a_width_three(self, const_definition):
        raw = const_definition.encode({"CODE": "A", "VAL": "XYZ", "TAIL": "!"})
        assert raw == b"AXYZ!"
        decoded = const_definition.decode(raw)
        assert decoded["VAL"] == "XYZ"
        assert len(decoded["VAL"]) == 3
        assert decoded["TAIL"] == "!"

    def test_code_b_width_five(self, const_definition):
        raw = const_definition.encode({"CODE": "B", "VAL": "HELLO", "TAIL": "!"})
        assert raw == b"BHELLO!"
        decoded = const_definition.decode(raw)
        assert decoded["VAL"] == "HELLO"
        assert len(decoded["VAL"]) == 5

    def test_code_c_width_one(self, const_definition):
        raw = const_definition.encode({"CODE": "C", "VAL": "Z", "TAIL": "!"})
        assert raw == b"CZ!"
        decoded = const_definition.decode(raw)
        assert decoded["VAL"] == "Z"
        assert len(decoded["VAL"]) == 1

    @pytest.mark.parametrize("code,val", [("A", "XYZ"), ("B", "HELLO"), ("C", "Z")])
    def test_round_trip(self, const_definition, code, val):
        raw = const_definition.encode({"CODE": code, "VAL": val, "TAIL": "!"})
        assert const_definition.encode(const_definition.decode(raw)) == raw
