"""Spec-fidelity anchor for the RSMDCA direct-parameter TRE.

``RSMDCA`` (Table 5) omitted three ground-space Choice Set index fields —
``GYX``, ``GZX``, ``GZY`` — before the BUG_TRE_KSY_SPEC_CONFORMANCE Phase 2 fix,
so every field after the gap (``DERCOV``) was read 6 bytes early.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch — **field presence**, **repeat count**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy`` (which is 6 bytes short per record).

Spec: STDI-0002 Vol 1, Appendix U -- RSMDCA Table 5 (pp. U-71 to U-79).
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

# Width of an RSM real number field, in bytes.
R = 21

# The twelve Local rectangular coordinate system fields (origin + 3x3 unit
# vectors).
LOCAL_KEYS = [
    "XUOL", "YUOL", "ZUOL",
    "XUXL", "XUYL", "XUZL",
    "YUXL", "YUYL", "YUZL",
    "ZUXL", "ZUYL", "ZUZL",
]

# The 36-field RSM Adjustable Parameter Choice Set (Tables 5/7): 20 image-space
# indices followed by 16 ground-space indices, each 2 bytes. The three fields
# GYX, GZX, GZY were the ones omitted before the fix.
INDEX_KEYS = [
    "IRO", "IRX", "IRY", "IRZ", "IRXX", "IRXY", "IRXZ", "IRYY", "IRYZ", "IRZZ",
    "ICO", "ICX", "ICY", "ICZ", "ICXX", "ICXY", "ICXZ", "ICYY", "ICYZ", "ICZZ",
    "GXO", "GYO", "GZO", "GXR", "GYR", "GZR", "GS",
    "GXX", "GXY", "GXZ", "GYX", "GYY", "GYZ", "GZX", "GZY", "GZZ",
]

# The fields that were missing before the fix (spec order among the G-fields).
RESTORED_GROUND_KEYS = ["GYX", "GZX", "GZY"]


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


def _local() -> dict:
    return {k: "0" * R for k in LOCAL_KEYS}


def _indices() -> dict:
    """The full 36-field Choice Set, all spaces (parameter not used)."""
    return {k: "  " for k in INDEX_KEYS}


class TestRsmdca:
    """RSMDCA carries all 36 Choice Set index fields; DERCOV then aligns."""

    NPAR = 3
    NIMGE = 2

    @property
    def NPART(self) -> int:
        return self.NPAR * self.NIMGE

    def _record(self) -> dict:
        return {
            "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
            "NPAR": str(self.NPAR).zfill(2),
            "NIMGE": str(self.NIMGE).zfill(3),
            "NPART": str(self.NPART).zfill(5),
            "IMAGES": [
                {"IIDI": chr(65 + i) * 80, "NPARI": str(self.NPAR).zfill(2)}
                for i in range(self.NIMGE)
            ],
            **_local(),
            **_indices(),
            "DERCOV": [
                "0" * R for _ in range((self.NPART * (self.NPART + 1)) // 2)
            ],
        }

    def test_restored_ground_fields_present(self, registry):
        """GYX, GZX, GZY exist as addressable 2-byte fields in spec order."""
        defn = registry.get("tre_rsmdca")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        for key in RESTORED_GROUND_KEYS:
            assert key in decoded, f"ground index field {key} missing"
            assert len(decoded[key]) == 2, f"{key} must be 2 bytes"

    def test_dercov_count_and_total_length(self, registry):
        """DERCOV holds NPART*(NPART+1)/2 elements and the record length matches."""
        defn = registry.get("tre_rsmdca")
        assert defn is not None

        raw = defn.encode(self._record())
        decoded = defn.decode(raw)

        expected_dercov = (self.NPART * (self.NPART + 1)) // 2
        assert len(decoded["DERCOV"]) == expected_dercov

        # header IID80+EDITION40+TID40+NPAR2+NIMGE3+NPART5 = 170
        # images NIMGE*(80+2) ; local 12*21 ; index 36*2 ; DERCOV n*21
        expected = (
            170
            + self.NIMGE * 82
            + 12 * R
            + 36 * 2
            + expected_dercov * R
        )
        assert len(raw) == expected

    def test_round_trip(self, registry):
        defn = registry.get("tre_rsmdca")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw
