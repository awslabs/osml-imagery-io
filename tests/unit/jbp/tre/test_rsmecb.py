"""Spec-fidelity anchor for the RSMECB covariance TRE.

RSMECB was substantially incomplete before the BUG_TRE_KSY_SPEC_CONFORMANCE
Phase 1 fix: it ended at APBASE and never modeled the per-independent-subgroup
error covariance or the mapping matrix.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch: **field presence**, **repeat count**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy`` (which cannot produce the modeled fields at
the spec offsets or the spec total length).

Spec: STDI-0002 Vol 1, Appendix U -- RSMECB Table 10 (pp. U-187 to U-203).
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


def _image_ap(nisapr: int, nisapc: int) -> dict:
    """An image-space adjustable-parameter block with the given power counts."""
    return {
        "NISAP": str(nisapr + nisapc).zfill(2),
        "NISAPR": str(nisapr).zfill(2),
        "ROW_POWERS": [
            {"XPWRR": "0", "YPWRR": "0", "ZPWRR": "0"} for _ in range(nisapr)
        ],
        "NISAPC": str(nisapc).zfill(2),
        "COL_POWERS": [
            {"XPWRC": "0", "YPWRC": "0", "ZPWRC": "0"} for _ in range(nisapc)
        ],
    }


class TestRsmecb:
    """RSMECB models the per-subgroup error covariance and the mapping matrix."""

    def _record(self) -> dict:
        # INCLIC=Y, APTYP=I, LOCTYP=R, APBASE=Y, INCLUC=N.
        nparo, ign, npar, nbasis = 3, 1, 3, 3
        numopg, ncseg = 3, 2
        subgroup = {
            "NUMOPG": str(numopg).zfill(2),
            "ERRCVG": _r((numopg * (numopg + 1)) // 2),
            "TCDF": "0",
            "ACSMC": "N",
            "SEGMENT_DATA": {
                "NCSEG": str(ncseg),
                "SEGMENTS": [
                    {"CORSEG": "0" * R, "TAUSEG": "0" * R} for _ in range(ncseg)
                ],
            },
        }
        orig_cov = {
            "NPARO": str(nparo).zfill(2),
            "IGN": str(ign).zfill(2),
            "CVDATE": "20240101",
            "NPAR": str(npar).zfill(2),
            "APTYP": "I",
            "LOCTYP": "R",
            "NSFX": "0" * R, "NSFY": "0" * R, "NSFZ": "0" * R,
            "NOFFX": "0" * R, "NOFFY": "0" * R, "NOFFZ": "0" * R,
            "LOCAL_COORD": _local(),
            "APBASE": "Y",
            "IMAGE_AP": _image_ap(1, 1),
            "NBASIS": str(nbasis).zfill(2),
            "AEL": _r(npar * nbasis),
            "SUBGROUPS": [subgroup],
            "MAP": _r(npar * nparo),
        }
        return {
            "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
            "INCLIC": "Y", "INCLUC": "N",
            "ORIG_COV": orig_cov,
        }

    def test_subgroup_and_map_present_with_counts(self, registry):
        """Per-IGN subgroup loop and the NPAR*NPARO mapping matrix are modeled."""
        defn = registry.get("tre_rsmecb")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        oc = decoded["ORIG_COV"]

        assert len(oc["SUBGROUPS"]) == 1  # IGN entries
        sub = oc["SUBGROUPS"][0]
        # ERRCVG upper triangular: (1/2)(NUMOPG+1)(NUMOPG) with NUMOPG=3 -> 6.
        assert len(sub["ERRCVG"]) == 6
        # NCSEG=2 correlation segments.
        assert len(sub["SEGMENT_DATA"]["SEGMENTS"]) == 2
        # Mapping matrix: NPAR*NPARO = 9 elements.
        assert len(oc["MAP"]) == 3 * 3

    def test_total_length(self, registry):
        defn = registry.get("tre_rsmecb")
        assert defn is not None

        raw = defn.encode(self._record())
        # header 80+40+40+1+1 = 162
        # orig_cov: NPARO2+IGN2+CVDATE8 = 12 ; NPAR2+APTYP1+LOCTYP1 = 4
        #           + 6*21 (norm) + 12*21 (local) + APBASE1
        #           + image_ap 12 + NBASIS2 + AEL (NPAR*NBASIS=9)*21
        # subgroup: NUMOPG2 + ERRCVG 6*21 + TCDF1 + ACSMC1
        #           + SEGMENT_DATA(NCSEG1 + 2 segs * 2*21 = 84) = 85
        # MAP (NPAR*NPARO=9)*21
        oc_head = 12 + 4 + 6 * R + 12 * R + 1 + 12 + 2 + 9 * R
        subgroup = 2 + 6 * R + 1 + 1 + (1 + 2 * (2 * R))
        map_len = 9 * R
        expected = 162 + oc_head + subgroup + map_len
        assert len(raw) == expected

    def test_round_trip(self, registry):
        defn = registry.get("tre_rsmecb")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw
