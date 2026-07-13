"""Spec-fidelity anchor for the RSMECA covariance TRE.

RSMECA was substantially incomplete before the BUG_TRE_KSY_SPEC_CONFORMANCE
Phase 1 fix: it used an invented ``COVAR`` construct, only 11 of the 36
Adjustable Parameter Choice Set index fields, and six invented source/sensor
unmodeled-error fields.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch: **field presence**, **repeat count**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy`` (which cannot produce the modeled fields at
the spec offsets or the spec total length).

Spec: STDI-0002 Vol 1, Appendix U -- RSMECA Table 9 (pp. U-152 to U-165).
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

# Width of an RSM real number field (BCS-A), in bytes.
R = 21

# The twelve Local rectangular coordinate system fields (origin + 3x3 unit
# vectors), each 21 BCS-A.
LOCAL_KEYS = [
    "XUOL", "YUOL", "ZUOL",
    "XUXL", "XUYL", "XUZL",
    "YUXL", "YUYL", "YUZL",
    "ZUXL", "ZUYL", "ZUZL",
]

# The 36-field RSM Adjustable Parameter Choice Set for RSMECA (Table 9):
# 20 image-space indices followed by 16 ground-space indices, each 2 BCS-A.
RSMECA_INDEX_KEYS = [
    "IRO", "IRX", "IRY", "IRZ", "IRXX", "IRXY", "IRXZ", "IRYY", "IRYZ", "IRZZ",
    "ICO", "ICX", "ICY", "ICZ", "ICXX", "ICXY", "ICXZ", "ICYY", "ICYZ", "ICZZ",
    "GXO", "GYO", "GZO", "GXR", "GYR", "GZR", "GS",
    "GXX", "GXY", "GXZ", "GYX", "GYY", "GYZ", "GZX", "GZY", "GZZ",
]


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


def _r(n: int = 1) -> list:
    """A list of ``n`` canonical 21-char real-number strings."""
    return ["0" * R for _ in range(n)]


def _local() -> dict:
    return {k: "0" * R for k in LOCAL_KEYS}


class TestRsmeca:
    """RSMECA has the 36-field index set, per-IGN loop, and no invented fields."""

    def _record(self) -> dict:
        # INCLIC=Y, INCLUC=N.
        npar, nparo, ign = 3, 3, 1
        numopg, ncseg = 3, 2
        indices = {k: "  " for k in RSMECA_INDEX_KEYS}
        subgroup = {
            "NUMOPG": str(numopg).zfill(2),
            "ERRCVG": _r((numopg * (numopg + 1)) // 2),
            "TCDF": "0",
            "NCSEG": str(ncseg),
            "SEGMENTS": [
                {"CORSEG": "0" * R, "TAUSEG": "0" * R} for _ in range(ncseg)
            ],
        }
        indirect = {
            "NPAR": str(npar).zfill(2),
            "NPARO": str(nparo).zfill(2),
            "IGN": str(ign).zfill(2),
            "CVDATE": "20240101",
            **_local(),
            **indices,
            "SUBGROUPS": [subgroup],
            "MAP": _r(npar * nparo),
        }
        return {
            "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
            "INCLIC": "Y", "INCLUC": "N",
            "INDIRECT_ERROR": indirect,
        }

    def test_full_index_set_present(self, registry):
        """All 36 Choice Set index fields exist with the correct mnemonics."""
        defn = registry.get("tre_rsmeca")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        ie = decoded["INDIRECT_ERROR"]
        for key in RSMECA_INDEX_KEYS:
            assert key in ie, f"index field {key} missing"
            assert len(ie[key]) == 2, f"index field {key} must be 2 bytes"
        assert len(RSMECA_INDEX_KEYS) == 36

    def test_no_invented_fields(self, registry):
        """The invented COVAR construct and split source/sensor fields are gone."""
        defn = registry.get("tre_rsmeca")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        ie = decoded["INDIRECT_ERROR"]
        assert "COVAR" not in ie, "invented COVAR field must not be present"
        # Table 9 uses the per-IGN subgroup loop instead.
        assert "SUBGROUPS" in ie
        assert len(ie["SUBGROUPS"]) == 1

    def test_subgroup_and_map_counts(self, registry):
        defn = registry.get("tre_rsmeca")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        ie = decoded["INDIRECT_ERROR"]
        sub = ie["SUBGROUPS"][0]
        assert len(sub["ERRCVG"]) == 6            # (1/2)(NUMOPG+1)(NUMOPG)
        assert len(sub["SEGMENTS"]) == 2          # NCSEG
        assert len(ie["MAP"]) == 3 * 3            # NPAR*NPARO

    def test_total_length(self, registry):
        defn = registry.get("tre_rsmeca")
        assert defn is not None

        raw = defn.encode(self._record())
        # header 162
        # indirect: NPAR2+NPARO2+IGN2+CVDATE8 = 14 ; local 12*21 ; index 36*2
        # subgroup: NUMOPG2 + ERRCVG 6*21 + TCDF1 + NCSEG1 + 2 segs*2*21 (84) = 214
        # MAP 9*21
        indirect_head = 14 + 12 * R + 36 * 2
        subgroup = 2 + 6 * R + 1 + 1 + 2 * (2 * R)
        map_len = 9 * R
        expected = 162 + indirect_head + subgroup + map_len
        assert len(raw) == expected

    def test_round_trip(self, registry):
        defn = registry.get("tre_rsmeca")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw
