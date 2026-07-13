"""Structural-feature test: ``if:`` gated fields.

Exercises conditional field activation through the public dict API using a
synthetic, unit-only ``.ksy`` written to ``tmp_path``. A field with an ``if:``
condition is present on the wire only when the condition is true; ``encode`` must
emit it exactly when active and ``decode`` must consume it symmetrically, so the
byte layout shifts with the gating field's value.

This is the structure-agnostic counterpart to the per-TRE conditional anchors
(e.g. BANDSB's ``WAVE_LENGTH_UNIT``); 26 TREs in the corpus use ``if:`` gating.
"""

import pytest
from aws.osml.io import StructureRegistry

# FLAG gates the presence of OPT; TAIL sits after the gated field so an off-by-one
# in the conditional's byte accounting would corrupt it.
_COND_KSY = """\
meta:
  id: cond_field
  title: Conditional Test Structure
  endian: be
seq:
  - id: FLAG
    type: str
    size: 1
    encoding: BCS-A
  - id: OPT
    type: str
    size: 4
    encoding: BCS-A
    if: FLAG == "Y"
  - id: TAIL
    type: str
    size: 2
    encoding: BCS-A
"""


@pytest.fixture
def cond_definition(tmp_path):
    """Write the conditional KSY to a fresh temp search path and load it."""
    ksy_path = tmp_path / "cond_field.ksy"
    ksy_path.write_text(_COND_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("cond_field")
    assert defn is not None, "cond_field definition not found"
    return defn


class TestConditionalActivation:
    """The gated field is present iff its ``if:`` condition holds."""

    def test_condition_true_includes_field(self, cond_definition):
        raw = cond_definition.encode({"FLAG": "Y", "OPT": "DATA", "TAIL": "ZZ"})
        assert raw == b"YDATAZZ"
        decoded = cond_definition.decode(raw)
        assert decoded["OPT"] == "DATA"
        assert decoded["TAIL"] == "ZZ"

    def test_condition_false_omits_field(self, cond_definition):
        # OPT is over-provided but must not be emitted: the condition is false.
        raw = cond_definition.encode({"FLAG": "N", "OPT": "DATA", "TAIL": "ZZ"})
        assert raw == b"NZZ"
        decoded = cond_definition.decode(raw)
        assert "OPT" not in decoded
        assert decoded["TAIL"] == "ZZ"

    def test_condition_false_field_may_be_absent(self, cond_definition):
        # A false-condition field need not be supplied at all.
        raw = cond_definition.encode({"FLAG": "N", "TAIL": "ZZ"})
        assert raw == b"NZZ"
        assert "OPT" not in cond_definition.decode(raw)

    @pytest.mark.parametrize(
        "flag,expected",
        [("Y", b"YDATAZZ"), ("N", b"NZZ")],
    )
    def test_round_trip(self, cond_definition, flag, expected):
        raw = cond_definition.encode({"FLAG": flag, "OPT": "DATA", "TAIL": "ZZ"})
        assert raw == expected
        assert cond_definition.encode(cond_definition.decode(raw)) == raw
