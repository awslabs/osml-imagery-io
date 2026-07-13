"""Spec-fidelity anchor for the SOURCB (Appendix P) TRE.

``SOURCB`` (Table P-14) omitted the conditional ``CDV10n`` downgrading date
(8 BCS-A) between ``QODn`` and ``QLEn`` before the BUG_TRE_KSY_SPEC_CONFORMANCE
Phase 3 fix, so any source carrying that date desynchronized the remainder of
the record.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch -- **conditional activation** and
**absolute total encoded length** taken from the spec's byte layout. They fail
against the pre-fix ``.ksy``.

Spec: STDI-0002 Vol 1, Appendix P (GeoSDE) -- SOURCB Table P-14 (pp. P-93 to P-114).
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


# Byte length of one minimal source_record with all count fields zero and all
# unit gate fields (UNISQU/UNIPCI/UNIHKE) blank, EXCLUDING the conditional
# CDV10 downgrading date. Derived field-by-field from Table P-14.
_SOURCE_RECORD_BASE_LEN = (
    2      # NUM_BP (no polygons)
    + 10   # PRT
    + 20   # URF
    + 7    # EDN
    + 20   # NAM
    + 3    # CDP
    + 8    # CDV
    + 8    # CDV27
    + 80   # SRN
    + 9    # SCA
    + 3    # UNISQU (blank -> SQU omitted)
    + 3    # UNIPCI (blank -> PCI omitted)
    + 3    # WPC
    + 3    # NST
    + 3    # UNIHKE (blank -> HKE/LONHKE/LATHKE omitted)
    + 1    # QSS
    + 1    # QOD
    # CDV10 (8) conditionally here
    + 80   # QLE
    + 80   # CPY
    + 2    # NMI (no map items)
    + 2    # NLI (no line items)
    + 80   # DAG
    + 4    # DCD
    + 80   # ELL
    + 3    # ELC
    + 80   # DVR
    + 4    # VDCDVR
    + 80   # SDA
    + 4    # VDCSDA
    + 80   # PRN
    + 2    # PCO
    + 1    # NUM_PRJ (no prj params)
    + 15   # XOR
    + 15   # YOR
    + 3    # GRD
    + 80   # GRN
    + 4    # ZNA
    + 2    # NIN (no intersections)
)

# IS_SCA(9) + CPATCH(10) + NUM_SOUR(2)
_SOURCB_HEADER_LEN = 9 + 10 + 2

CDV10_LEN = 8


class TestSourcb:
    """SOURCB carries the conditional CDV10n downgrading date after QODn."""

    def _source_record(self, qss: str, qod: str) -> dict:
        rec = {
            "NUM_BP": "00",
            "POLYGONS": [],
            "PRT": "P" * 10,
            "URF": "U" * 20,
            "EDN": "E" * 7,
            "NAM": "N" * 20,
            "CDP": "000",
            "CDV": "20240101",
            "CDV27": "20240101",
            "SRN": "S" * 80,
            "SCA": "000000001",
            "UNISQU": "   ",
            "UNIPCI": "   ",
            "WPC": "000",
            "NST": "000",
            "UNIHKE": "   ",
            "QSS": qss,
            "QOD": qod,
            "QLE": "Q" * 80,
            "CPY": "C" * 80,
            "NMI": "00",
            "MAP_ITEMS": [],
            "NLI": "00",
            "LINE_ITEMS": [],
            "DAG": "D" * 80,
            "DCD": "DCD ",
            "ELL": "L" * 80,
            "ELC": "ELC",
            "DVR": "V" * 80,
            "VDCDVR": "VDCD",
            "SDA": "A" * 80,
            "VDCSDA": "VDCS",
            "PRN": "R" * 80,
            "PCO": "PC",
            "NUM_PRJ": "0",
            "PRJ_PARAMS": [],
            "XOR": "0" * 15,
            "YOR": "0" * 15,
            "GRD": "GRD",
            "GRN": "G" * 80,
            "ZNA": "0000",
            "NIN": "00",
            "INTERSECTIONS": [],
        }
        # CDV10 present unless QSS == "U" or QOD == "Y".
        if qss != "U" and qod != "Y":
            rec["CDV10"] = "20240102"
        return rec

    def _record(self, qss: str, qod: str) -> dict:
        return {
            "IS_SCA": "000000001",
            "CPATCH": "P" * 10,
            "NUM_SOUR": "01",
            "SOURCES": [self._source_record(qss, qod)],
        }

    def test_cdv10_present_when_downgrading_date_applies(self, registry):
        """QSS != 'U' and QOD != 'Y' -> CDV10 is emitted (8 BCS-A)."""
        defn = registry.get("tre_sourcb")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record("C", "N")))
        source = decoded["SOURCES"][0]
        assert "CDV10" in source and source["CDV10"] is not None, (
            "CDV10 downgrading date must be present when QSS != U and QOD != Y"
        )
        assert len(source["CDV10"]) == 8

    def test_cdv10_omitted_when_qss_unclassified(self, registry):
        """QSS == 'U' -> CDV10 is omitted; record is 8 bytes shorter."""
        defn = registry.get("tre_sourcb")
        assert defn is not None

        raw_present = defn.encode(self._record("C", "N"))
        raw_absent = defn.encode(self._record("U", "N"))
        assert len(raw_present) - len(raw_absent) == CDV10_LEN

    def test_cdv10_omitted_when_downgrading_required(self, registry):
        """QOD == 'Y' -> CDV10 is omitted; record is 8 bytes shorter."""
        defn = registry.get("tre_sourcb")
        assert defn is not None

        raw_present = defn.encode(self._record("C", "N"))
        raw_absent = defn.encode(self._record("C", "Y"))
        assert len(raw_present) - len(raw_absent) == CDV10_LEN

    def test_total_length_both_branches(self, registry):
        """Absolute record length matches the Table P-14 byte layout."""
        defn = registry.get("tre_sourcb")
        assert defn is not None

        with_date = _SOURCB_HEADER_LEN + _SOURCE_RECORD_BASE_LEN + CDV10_LEN
        without_date = _SOURCB_HEADER_LEN + _SOURCE_RECORD_BASE_LEN

        assert len(defn.encode(self._record("C", "N"))) == with_date
        assert len(defn.encode(self._record("U", "N"))) == without_date

    def test_round_trip_both_branches(self, registry):
        defn = registry.get("tre_sourcb")
        assert defn is not None
        for qss, qod in [("C", "N"), ("U", "N"), ("C", "Y")]:
            raw = defn.encode(self._record(qss, qod))
            assert defn.encode(defn.decode(raw)) == raw
