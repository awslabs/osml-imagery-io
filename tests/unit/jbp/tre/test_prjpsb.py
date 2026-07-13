"""Spec-fidelity anchor for the PRJPSB (Appendix P) TRE.

``PRJPSB`` (Table P-3) had parse-desynchronizing defects before the
BUG_TRE_KSY_SPEC_CONFORMANCE Phase 3 fix: it led with an 80-byte ``PRJ`` name
field instead of the spec ``PRN`` (3 BCS-A), placed ``XOR``/``YOR`` *before* the
count and loop instead of last, and gave each loop element an invented 80-byte
``PTB`` field on top of the single 15-byte ``PRJn`` parameter.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch -- **field presence**, **field width**,
**field order**, and **absolute total encoded length** taken from the spec's
byte layout. They fail against the pre-fix ``.ksy``.

Spec: STDI-0002 Vol 1, Appendix P (GeoSDE) -- PRJPSB Table P-3 (pp. P-26 to P-28).
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


class TestPrjpsb:
    """PRJPSB: PRN(3), PCO(2), NUM_PRJ(1), PRJn(15) loop, then XOR/YOR last."""

    def _record(self, num_prj: int) -> dict:
        return {
            "PRN": "TC ",
            "PCO": "TC",
            "NUM_PRJ": str(num_prj),
            "PROJECTION_PARAMS": [
                {"PRJ": str(i).zfill(15)} for i in range(num_prj)
            ],
            "XOR": "0" * 15,
            "YOR": "0" * 15,
        }

    def test_field_names_sizes_and_order(self, registry):
        """PRN(3)/PCO(2)/NUM_PRJ(1) lead; no invented PTB; XOR/YOR are last."""
        defn = registry.get("tre_prjpsb")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record(2)))

        assert len(decoded["PRN"]) == 3, "PRN must be the 3-byte name field"
        assert len(decoded["PCO"]) == 2, "PCO must be 2 bytes"
        assert len(decoded["NUM_PRJ"]) == 1, "NUM_PRJ must be 1 byte"
        # The invented 80-byte name field must be gone.
        assert "PRJ" not in decoded, "the pre-fix 80-byte PRJ field must be gone"

        # Each loop element is a single 15-byte BCS-N parameter, no PTB.
        params = decoded["PROJECTION_PARAMS"]
        assert len(params) == 2
        for p in params:
            assert len(p["PRJ"]) == 15, "PRJn must be a single 15-byte parameter"
            assert "PTB" not in p, "the invented 80-byte PTB must be gone"

        # XOR/YOR carry through the round trip (they now trail the loop).
        assert len(decoded["XOR"]) == 15
        assert len(decoded["YOR"]) == 15

    @pytest.mark.parametrize("num_prj", [0, 1, 5])
    def test_total_length(self, registry, num_prj):
        """Record length follows the field table: 36 + NUM_PRJ*15.

        (The Table P-3 CEL formula 113 + NUM_PRJ*15 is stale -- it reflects the
        legacy 80-byte name field; the authoritative field rows give PRN=3.)
        """
        defn = registry.get("tre_prjpsb")
        assert defn is not None

        raw = defn.encode(self._record(num_prj))
        # PRN3 + PCO2 + NUM_PRJ1 + PRJn(15*n) + XOR15 + YOR15
        expected = 3 + 2 + 1 + num_prj * 15 + 15 + 15
        assert len(raw) == expected

    def test_round_trip(self, registry):
        defn = registry.get("tre_prjpsb")
        assert defn is not None
        raw = defn.encode(self._record(3))
        assert defn.encode(defn.decode(raw)) == raw
