"""Spec-fidelity anchor for the RSMAPA adjustable-parameter TRE.

``RSMAPA`` (Table 7) omitted three ground-space Choice Set index fields —
``GYX``, ``GZX``, ``GZY`` — before the BUG_TRE_KSY_SPEC_CONFORMANCE Phase 2 fix,
so every field after the gap (``PARVAL``) was read 6 bytes early.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch — **field presence**, **repeat count**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy`` (which is 6 bytes short per record).

Spec: STDI-0002 Vol 1, Appendix U -- RSMAPA Table 7 (pp. U-113 to U-120).
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


class TestRsmapa:
    """RSMAPA carries all 36 Choice Set index fields, GYX/GZX/GZY included."""

    NPAR = 3

    def _record(self) -> dict:
        return {
            "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
            "NPAR": str(self.NPAR).zfill(2),
            **_local(),
            **_indices(),
            "PARVAL": ["0" * R for _ in range(self.NPAR)],
        }

    def test_restored_ground_fields_present(self, registry):
        """GYX, GZX, GZY exist as addressable 2-byte fields in spec order."""
        defn = registry.get("tre_rsmapa")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        for key in RESTORED_GROUND_KEYS:
            assert key in decoded, f"ground index field {key} missing"
            assert len(decoded[key]) == 2, f"{key} must be 2 bytes"
        # The full Choice Set is present.
        for key in INDEX_KEYS:
            assert key in decoded, f"index field {key} missing"
        assert len(INDEX_KEYS) == 36

    def test_total_length(self, registry):
        """Record length matches the Table 7 byte layout for a known NPAR."""
        defn = registry.get("tre_rsmapa")
        assert defn is not None

        raw = defn.encode(self._record())
        # header IID80+EDITION40+TID40+NPAR2 = 162
        # local 12*21 ; index 36*2 ; PARVAL NPAR*21
        expected = 162 + 12 * R + 36 * 2 + self.NPAR * R
        assert len(raw) == expected

    def test_round_trip(self, registry):
        defn = registry.get("tre_rsmapa")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw
