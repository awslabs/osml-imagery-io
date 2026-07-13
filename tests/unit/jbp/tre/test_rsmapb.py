"""Spec-fidelity anchor for the RSMAPB adjustable-parameter TRE.

``RSMAPB`` (Table 8) modeled the ``APBASE=Y`` basis matrix as a bare ``NBASIS``
count and never emitted the ``NPAR*NBASIS`` matrix-A element loop (``AEL``)
before the BUG_TRE_KSY_SPEC_CONFORMANCE Phase 2 fix; it also named the
Adjustable Parameter Type field ``APTS`` instead of the spec ``APTYP``.

The ``AEL`` count is also load-bearing for the navigator-removal rewrite
(DESIGN_REMOVE_ROOT_PARENT_NAVIGATORS Phase 2): the nested ``basis_matrix``
resolves ``NPAR`` from the enclosing (inherited) scope — formerly
``_parent.NPAR.to_i * NBASIS.to_i``. Both the ``LOCTYP=R`` (local-coordinate
block present) and ``LOCTYP=N`` (block absent) branches are exercised so the
inherited resolution is verified in both.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch — **field presence**, **repeat count**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy`` (which omits the entire AEL loop).

Spec: STDI-0002 Vol 1, Appendix U -- RSMAPB Table 8 (pp. U-128 to U-137).
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

# Width of an RSM real number field, in bytes.
R = 21

_V21 = "+1.00000000000000E+00"  # 21-char BCS scientific-notation value

# The twelve Local rectangular coordinate system fields (origin + 3x3 unit
# vectors).
LOCAL_KEYS = [
    "XUOL", "YUOL", "ZUOL",
    "XUXL", "XUYL", "XUZL",
    "YUXL", "YUYL", "YUZL",
    "ZUXL", "ZUYL", "ZUZL",
]


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


def _local() -> dict:
    return {k: "0" * R for k in LOCAL_KEYS}


class TestRsmapb:
    """RSMAPB emits the NPAR*NBASIS AEL matrix when APBASE=Y; field is APTYP."""

    NPAR = 3
    NBASIS = 3

    def _record(self) -> dict:
        # APTYP=I, LOCTYP=R, APBASE=Y.
        rn = "0" * R
        return {
            "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
            "NPAR": str(self.NPAR).zfill(2),
            "APTYP": "I",
            "LOCTYP": "R",
            "NSFX": rn, "NSFY": rn, "NSFZ": rn,
            "NOFFX": rn, "NOFFY": rn, "NOFFZ": rn,
            "LOCAL_COORD": _local(),
            "APBASE": "Y",
            "IMAGE_AP": {
                "NISAP": "02",
                "NISAPR": "01",
                "ROW_POWERS": [{"XPWR": "0", "YPWR": "0", "ZPWR": "0"}],
                "NISAPC": "01",
                "COL_POWERS": [{"XPWR": "0", "YPWR": "0", "ZPWR": "0"}],
            },
            "BASIS_DATA": {
                "NBASIS": str(self.NBASIS).zfill(2),
                "AEL": [rn for _ in range(self.NPAR * self.NBASIS)],
            },
            "PARVAL": [rn for _ in range(self.NPAR)],
        }

    def test_aptyp_field_present(self, registry):
        """The Adjustable Parameter Type field is named APTYP (not APTS)."""
        defn = registry.get("tre_rsmapb")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        assert "APTYP" in decoded, "field must be named APTYP"
        assert "APTS" not in decoded, "the pre-fix APTS name must be gone"
        assert decoded["APTYP"] == "I"

    def test_ael_matrix_count(self, registry):
        """APBASE=Y emits exactly NPAR*NBASIS matrix-A elements (LOCTYP=R)."""
        defn = registry.get("tre_rsmapb")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        assert "AEL" in decoded["BASIS_DATA"], "AEL matrix loop missing"
        assert len(decoded["BASIS_DATA"]["AEL"]) == self.NPAR * self.NBASIS

    def test_total_length(self, registry):
        defn = registry.get("tre_rsmapb")
        assert defn is not None

        raw = defn.encode(self._record())
        # header IID80+EDITION40+TID40+NPAR2 = 162
        # APTYP1+LOCTYP1 ; norm 6*21 ; local 12*21 ; APBASE1
        # image_ap NISAP2+NISAPR2+3(row pow)+NISAPC2+3(col pow) = 12
        # basis NBASIS2 + AEL (NPAR*NBASIS)*21 ; PARVAL NPAR*21
        ael = self.NPAR * self.NBASIS
        expected = (
            162 + 2 + 6 * R + 12 * R + 1 + 12 + 2 + ael * R + self.NPAR * R
        )
        assert len(raw) == expected

    def test_round_trip(self, registry):
        defn = registry.get("tre_rsmapb")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw


def test_rsmapb_basis_matrix_ael_count_loctyp_none(registry):
    """AEL holds exactly NPAR x NBASIS entries; NPAR resolved from inherited scope.

    The ``LOCTYP=N`` branch (no local-coordinate block) with NPAR!=NBASIS —
    the load-bearing navigator-rewrite check that the nested ``basis_matrix``
    reads ``NPAR`` from the enclosing scope rather than a removed ``_parent``
    navigator (DESIGN_REMOVE_ROOT_PARENT_NAVIGATORS Phase 2).
    """
    defn = registry.get("tre_rsmapb")
    assert defn is not None

    npar, nbasis = 2, 3
    record = {
        "IID": "IMG".ljust(80), "EDITION": "ED".ljust(40), "TID": "T".ljust(40),
        "NPAR": f"{npar:02d}", "APTYP": "I", "LOCTYP": "N",
        "NSFX": _V21, "NSFY": _V21, "NSFZ": _V21,
        "NOFFX": _V21, "NOFFY": _V21, "NOFFZ": _V21,
        "APBASE": "Y",
        "IMAGE_AP": {
            "NISAP": "02", "NISAPR": "00", "ROW_POWERS": [],
            "NISAPC": "00", "COL_POWERS": [],
        },
        "BASIS_DATA": {
            "NBASIS": f"{nbasis:02d}",
            "AEL": [_V21 for _ in range(npar * nbasis)],
        },
        "PARVAL": [_V21 for _ in range(npar)],
    }
    raw = defn.encode(record)
    decoded = defn.decode(raw)
    assert len(decoded["BASIS_DATA"]["AEL"]) == npar * nbasis
    assert defn.encode(decoded) == raw
