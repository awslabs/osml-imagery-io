"""Spec-fidelity anchor for the MAPLOB (Appendix P) TRE.

``MAPLOB`` (Table P-6) sized the 2nd/3rd fields as ``ARV``/``BRV`` at 9 bytes
each instead of the spec ``LOD``/``LAD`` at 5 bytes each before the
BUG_TRE_KSY_SPEC_CONFORMANCE Phase 3 fix, so every trailing field was read 8
bytes late.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch -- **field presence**, **field width**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy``.

Spec: STDI-0002 Vol 1, Appendix P (GeoSDE) -- MAPLOB Table P-6 (pp. P-37 to P-38).
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


class TestMaplob:
    """MAPLOB: UNILOA(3), LOD(5), LAD(5), LSO(15), PSO(15) -- total 43."""

    def _record(self) -> dict:
        return {
            "UNILOA": "M  ",
            "LOD": "00001",
            "LAD": "00001",
            "LSO": "0" * 15,
            "PSO": "0" * 15,
        }

    def test_field_names_and_widths(self, registry):
        """LOD/LAD are the 5-byte interval fields (not ARV/BRV at 9)."""
        defn = registry.get("tre_maplob")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))

        assert "UNILOA" in decoded, "1st field must be named UNILOA"
        assert len(decoded["LOD"]) == 5, "LOD (Easting Interval) must be 5 bytes"
        assert len(decoded["LAD"]) == 5, "LAD (Northing Interval) must be 5 bytes"
        # The pre-fix 9-byte ARV/BRV mnemonics must be gone.
        assert "ARV" not in decoded
        assert "BRV" not in decoded
        assert "UNI" not in decoded

    def test_total_length(self, registry):
        """Record length matches Table P-6 (CEL 00043)."""
        defn = registry.get("tre_maplob")
        assert defn is not None

        raw = defn.encode(self._record())
        # UNILOA3 + LOD5 + LAD5 + LSO15 + PSO15
        assert len(raw) == 3 + 5 + 5 + 15 + 15
        assert len(raw) == 43

    def test_round_trip(self, registry):
        defn = registry.get("tre_maplob")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw
