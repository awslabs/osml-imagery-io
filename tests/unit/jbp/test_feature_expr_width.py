"""Structural-feature test: expression-sized field widths (``size: X.to_i``).

Exercises a field whose byte width is an expression over an earlier field
(``size: LEN.to_i``) through the public dict API using a synthetic, unit-only
``.ksy`` written to ``tmp_path``. The payload width tracks the value of the
length field, so the encoded length changes with ``LEN`` and ``decode`` must
consume exactly that many bytes.

This is the structure-agnostic counterpart to the many TREs whose variable-length
fields are sized from a preceding count/length field; 16 TREs in the corpus use a
``.to_i`` size expression.
"""

import pytest
from aws.osml.io import StructureRegistry

# PAYLOAD's width is read from the preceding LEN field; TAIL follows so a wrong
# width would misalign it.
_EXPR_KSY = """\
meta:
  id: expr_width
  title: Expression-Width Test Structure
  endian: be
seq:
  - id: LEN
    type: str
    size: 2
    encoding: BCS-N
  - id: PAYLOAD
    type: str
    size: LEN.to_i
    encoding: BCS-A
  - id: TAIL
    type: str
    size: 1
    encoding: BCS-A
"""


@pytest.fixture
def expr_definition(tmp_path):
    """Write the expression-width KSY to a fresh temp search path and load it."""
    ksy_path = tmp_path / "expr_width.ksy"
    ksy_path.write_text(_EXPR_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("expr_width")
    assert defn is not None, "expr_width definition not found"
    return defn


class TestExpressionWidth:
    """The payload width equals the decoded value of the length field."""

    def test_width_five(self, expr_definition):
        raw = expr_definition.encode({"LEN": "05", "PAYLOAD": "HELLO", "TAIL": "!"})
        assert raw == b"05HELLO!"
        decoded = expr_definition.decode(raw)
        assert decoded["PAYLOAD"] == "HELLO"
        assert len(decoded["PAYLOAD"]) == 5
        assert decoded["TAIL"] == "!"

    def test_width_three(self, expr_definition):
        raw = expr_definition.encode({"LEN": "03", "PAYLOAD": "ABC", "TAIL": "!"})
        assert raw == b"03ABC!"
        decoded = expr_definition.decode(raw)
        assert decoded["PAYLOAD"] == "ABC"
        assert len(decoded["PAYLOAD"]) == 3

    def test_width_zero(self, expr_definition):
        raw = expr_definition.encode({"LEN": "00", "PAYLOAD": "", "TAIL": "!"})
        assert raw == b"00!"
        decoded = expr_definition.decode(raw)
        assert decoded["PAYLOAD"] == ""
        assert decoded["TAIL"] == "!"

    @pytest.mark.parametrize("length,payload", [(1, "A"), (5, "HELLO"), (10, "0123456789")])
    def test_round_trip(self, expr_definition, length, payload):
        raw = expr_definition.encode(
            {"LEN": f"{length:02d}", "PAYLOAD": payload, "TAIL": "!"}
        )
        assert expr_definition.encode(expr_definition.decode(raw)) == raw
