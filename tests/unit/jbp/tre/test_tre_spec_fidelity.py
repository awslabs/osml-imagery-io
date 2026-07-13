"""Declarative spec-fidelity anti-regression gate for the fixed TREs.

This is the durable tripwire folded in from ``BUG_TRE_KSY_SPEC_CONFORMANCE.md``
Phase 5. It is a single parametrized table of externally-verified anchors —
one row per TRE fixed during that audit — asserted through the public
``encode``/``decode`` API. Its purpose is deliberately narrow and deliberately
redundant with the per-TRE ``tre/test_<name>.py`` anchors:

* the per-TRE files carry the *fine-grained* assertions (conditional activation,
  nested loop counts, interleaved field order, branch presence);
* **this** file is the *one place* where "adding a wrong size to any of the 14
  ``.ksy`` turns something red" is guaranteed as a class, and where a new TRE
  author extends coverage by adding exactly one ``SpecAnchor`` row.

Each anchor asserts two coarse, spec-derived invariants that a self-consistent
round trip cannot catch:

* the **absolute encoded length** of a known value record, and
* the **decoded width** of one or more spec-anchored top-level fields
  (favouring the widths that were previously wrong).

The anchors are hand-transcribed from the STDI-0002 spec tables (external ground
truth); the totals were derived field-by-field from those tables and cross-check
the per-TRE anchor files. Every row carries its ``spec`` citation.

**Extending the gate:** add one ``SpecAnchor(...)`` row with a known value dict,
its spec citation, the spec-derived total byte length, and the spec-derived
widths of the fields you want anchored.
"""

from dataclasses import dataclass, field
from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

# Width of an RSM real-number field (BCS-A), in bytes.
R = 21

# The twelve RSM Local rectangular coordinate system fields (origin + 3x3 unit
# vectors), each 21 BCS-A.
_LOCAL_KEYS = [
    "XUOL", "YUOL", "ZUOL",
    "XUXL", "XUYL", "XUZL",
    "YUXL", "YUYL", "YUZL",
    "ZUXL", "ZUYL", "ZUZL",
]

# The 36-field RSM Adjustable Parameter Choice Set (Tables 5/7/9): 20 image-space
# indices followed by 16 ground-space indices, each 2 BCS-A. GYX/GZX/GZY were the
# three fields omitted before the BUG_TRE_KSY_SPEC_CONFORMANCE Phase 2 fix.
_INDEX_KEYS = [
    "IRO", "IRX", "IRY", "IRZ", "IRXX", "IRXY", "IRXZ", "IRYY", "IRYZ", "IRZZ",
    "ICO", "ICX", "ICY", "ICZ", "ICXX", "ICXY", "ICXZ", "ICYY", "ICYZ", "ICZZ",
    "GXO", "GYO", "GZO", "GXR", "GYR", "GZR", "GS",
    "GXX", "GXY", "GXZ", "GYX", "GYY", "GYZ", "GZX", "GZY", "GZZ",
]


@dataclass(frozen=True)
class SpecAnchor:
    """One externally-verified spec anchor for a single named TRE.

    ``values`` is a known value record; ``total_bytes`` is its spec-derived
    absolute encoded length; ``field_widths`` maps top-level field names to their
    spec-derived decoded widths (favouring the previously-wrong widths).
    """

    tre: str
    spec: str
    values: dict
    total_bytes: int
    field_widths: dict = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Value-record builders for the structurally-complex TREs. Kept as small local
# helpers so each SpecAnchor row stays a single readable literal.
# ---------------------------------------------------------------------------


def _rn(n: int = 1) -> list:
    """A list of ``n`` canonical 21-char real-number strings."""
    return ["0" * R for _ in range(n)]


def _local() -> dict:
    return {k: "0" * R for k in _LOCAL_KEYS}


def _indices() -> dict:
    return {k: "  " for k in _INDEX_KEYS}


def _bandsb_values() -> dict:
    """BANDSB with EXISTENCE_MASK b14|b13 set for one band (App X, Table X.6-1).

    The b14 GSD group is 16 bytes/band and the b13 GSD-uncertainty group is
    14 bytes/band (two 7-byte values, no invented per-axis unit fields), so one
    band adds exactly 30 bytes to the 122-byte fixed header + mask.
    """
    mask = 0x00004000 | 0x00002000  # b14 | b13
    return {
        "COUNT": "00001",
        "RADIOMETRIC_QUANTITY": "RADIANCE".ljust(24),
        "RADIOMETRIC_QUANTITY_UNIT": "S",
        "SCALE_FACTOR": 1.0,
        "ADDITIVE_FACTOR": 0.0,
        "ROW_GSD": "0001.00",
        "ROW_GSD_UNIT": "M",
        "COL_GSD": "0001.00",
        "COL_GSD_UNIT": "M",
        "SPT_RESP_ROW": "0001.00",
        "SPT_RESP_UNIT_ROW": "M",
        "SPT_RESP_COL": "0001.00",
        "SPT_RESP_UNIT_COL": "M",
        "DATA_FLD_1": "00" * 48,
        "EXISTENCE_MASK": mask,
        "BAND_ROW_GSD": ["0001.00"],
        "BAND_ROW_GSD_UNIT": ["M"],
        "BAND_COL_GSD": ["0001.00"],
        "BAND_COL_GSD_UNIT": ["M"],
        "BAND_ROW_GSD_UNC": ["0000.10"],
        "BAND_COL_GSD_UNC": ["0000.10"],
    }


def _bchipa_values() -> dict:
    """BCHIPA with Section A present, B/C excluded (App AR, Table AR.5-3).

    prefix(47) + section A(62) + INCLUDE_B(1) + INCLUDE_C(1) = 111.
    """
    return {
        "SDE_UUID": " " * 36,
        "NUM_INSTS": "00001",
        "INSTANCE": "00001",
        "INCLUDE_A": "Y",
        "ISID": "IMG0000001",
        "TOT_ORIG_BANDS": "00003",
        "TOT_CURR_BANDS": "00002",
        "NUM_BWP_IS": "001",
        "BWP_IS": ["001"],
        "NUM_RLVNT_SDE": "001",
        "SDE_NAME": ["TEBANDSB".ljust(32)],
        "SDE_STATUS": ["O"],
        "INCLUDE_B": "N",
        "INCLUDE_C": "N",
    }


def _sourcb_values() -> dict:
    """SOURCB with one source carrying the conditional CDV10 date (App P, P-14).

    QSS != 'U' and QOD != 'Y' -> the 8-byte CDV10 downgrading date is present.
    header(21) + source base(885) + CDV10(8) = 914.
    """
    source = {
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
        "QSS": "C",
        "QOD": "N",
        "CDV10": "20240102",
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
    return {
        "IS_SCA": "000000001",
        "CPATCH": "P" * 10,
        "NUM_SOUR": "01",
        "SOURCES": [source],
    }


def _sensrb_values() -> dict:
    """SENSRB with one Module-12 time-stamped set, type code 06a (App Z, Z.3-1).

    Code 06a sizes TIME_STAMP_VALUE (12d) to 11 bytes via the note-h code->width
    consts map. Fixed prefix(96) + Module 12(32) + fixed suffix(8) = 136.
    """
    return {
        "GENERAL_DATA": "N",
        "SENSOR_ARRAY_DATA": "N",
        "SENSOR_CALIBRATION_DATA": "N",
        "IMAGE_FORMATION_DATA": "N",
        "REFERENCE_TIME": "0" * 12,
        "REFERENCE_ROW": "0" * 8,
        "REFERENCE_COLUMN": "0" * 8,
        "LATITUDE_OR_X": "0" * 11,
        "LONGITUDE_OR_Y": "0" * 12,
        "ALTITUDE_OR_Z": "0" * 11,
        "SENSOR_X_OFFSET": "0" * 8,
        "SENSOR_Y_OFFSET": "0" * 8,
        "SENSOR_Z_OFFSET": "0" * 8,
        "ATTITUDE_EULER_ANGLES": "N",
        "ATTITUDE_UNIT_VECTORS": "N",
        "ATTITUDE_QUATERNION": "N",
        "SENSOR_VELOCITY_DATA": "N",
        "POINT_SET_DATA": "00",
        "TIME_STAMPED_DATA_SETS": "01",
        "TIME_STAMPED_SETS": [{
            "TIME_STAMP_TYPE": "06a",
            "TIME_STAMP_COUNT": "0001",
            "TIME_STAMPS": [{
                "TIME_STAMP_TIME": "0" * 12,
                "TIME_STAMP_VALUE": "0" * 11,
            }],
        }],
        "PIXEL_REFERENCED_DATA_SETS": "00",
        "UNCERTAINTY_DATA": "000",
        "ADDITIONAL_PARAMETER_DATA": "000",
    }


def _rsmapa_values() -> dict:
    """RSMAPA, NPAR=3 (App U, Table 7). header(162)+local(252)+index(72)+PARVAL(63)."""
    return {
        "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
        "NPAR": "03",
        **_local(),
        **_indices(),
        "PARVAL": _rn(3),
    }


def _rsmdca_values() -> dict:
    """RSMDCA, NPAR=3 NIMGE=2 NPART=6 (App U, Table 5)."""
    return {
        "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
        "NPAR": "03", "NIMGE": "002", "NPART": "00006",
        "IMAGES": [
            {"IIDI": "A" * 80, "NPARI": "03"},
            {"IIDI": "B" * 80, "NPARI": "03"},
        ],
        **_local(),
        **_indices(),
        "DERCOV": _rn((6 * (6 + 1)) // 2),
    }


def _image_ap(nisapr: int, nisapc: int, powers_prefixed: bool) -> dict:
    """An image-space adjustable-parameter block.

    ``powers_prefixed`` selects the RSMDCB/RSMECB power-field names
    (``XPWRR``/``XPWRC``) vs. the RSMAPB plain ``XPWR``.
    """
    if powers_prefixed:
        row = [{"XPWRR": "0", "YPWRR": "0", "ZPWRR": "0"} for _ in range(nisapr)]
        col = [{"XPWRC": "0", "YPWRC": "0", "ZPWRC": "0"} for _ in range(nisapc)]
    else:
        row = [{"XPWR": "0", "YPWR": "0", "ZPWR": "0"} for _ in range(nisapr)]
        col = [{"XPWR": "0", "YPWR": "0", "ZPWR": "0"} for _ in range(nisapc)]
    return {
        "NISAP": str(nisapr + nisapc).zfill(2),
        "NISAPR": str(nisapr).zfill(2),
        "ROW_POWERS": row,
        "NISAPC": str(nisapc).zfill(2),
        "COL_POWERS": col,
    }


def _rsmapb_values() -> dict:
    """RSMAPB, NPAR=3 NBASIS=3, APTYP=I LOCTYP=R APBASE=Y (App U, Table 8)."""
    rn = "0" * R
    return {
        "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
        "NPAR": "03",
        "APTYP": "I", "LOCTYP": "R",
        "NSFX": rn, "NSFY": rn, "NSFZ": rn,
        "NOFFX": rn, "NOFFY": rn, "NOFFZ": rn,
        "LOCAL_COORD": _local(),
        "APBASE": "Y",
        "IMAGE_AP": _image_ap(1, 1, powers_prefixed=False),
        "BASIS_DATA": {"NBASIS": "03", "AEL": _rn(3 * 3)},
        "PARVAL": _rn(3),
    }


def _rsmdcb_values() -> dict:
    """RSMDCB, NIMGE=2 with distinct per-image NCOLCB (3,5) (App U, Table 6)."""
    ncolcb = [3, 5]
    nrowcb, npar, nbasis = 3, 3, 3
    ap_data = {
        "NPAR": str(npar).zfill(2), "APTYP": "I", "LOCTYP": "R",
        "NSFX": "0" * R, "NSFY": "0" * R, "NSFZ": "0" * R,
        "NOFFX": "0" * R, "NOFFY": "0" * R, "NOFFZ": "0" * R,
        "LOCAL_COORD": _local(),
        "APBASE": "Y",
        "IMAGE_AP": _image_ap(1, 1, powers_prefixed=True),
        "NBASIS": str(nbasis).zfill(2),
        "AEL": _rn(npar * nbasis),
    }
    return {
        "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
        "NROWCB": str(nrowcb).zfill(2),
        "NIMGE": "002",
        "IMAGE_RECORDS": [
            {"IIDI": "A" * 80, "NCOLCB": str(ncolcb[0]).zfill(2)},
            {"IIDI": "B" * 80, "NCOLCB": str(ncolcb[1]).zfill(2)},
        ],
        "INCAPD": "Y",
        "AP_DATA": ap_data,
        "CRSCOV_BLOCKS": [{"CRSCOV": _rn(nrowcb * n)} for n in ncolcb],
    }


def _rsmecb_values() -> dict:
    """RSMECB, INCLIC=Y INCLUC=N, one IGN subgroup (App U, Table 10)."""
    nparo, npar, nbasis = 3, 3, 3
    numopg, ncseg = 3, 2
    subgroup = {
        "NUMOPG": str(numopg).zfill(2),
        "ERRCVG": _rn((numopg * (numopg + 1)) // 2),
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
        "NPARO": str(nparo).zfill(2), "IGN": "01", "CVDATE": "20240101",
        "NPAR": str(npar).zfill(2), "APTYP": "I", "LOCTYP": "R",
        "NSFX": "0" * R, "NSFY": "0" * R, "NSFZ": "0" * R,
        "NOFFX": "0" * R, "NOFFY": "0" * R, "NOFFZ": "0" * R,
        "LOCAL_COORD": _local(),
        "APBASE": "Y",
        "IMAGE_AP": _image_ap(1, 1, powers_prefixed=True),
        "NBASIS": str(nbasis).zfill(2),
        "AEL": _rn(npar * nbasis),
        "SUBGROUPS": [subgroup],
        "MAP": _rn(npar * nparo),
    }
    return {
        "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
        "INCLIC": "Y", "INCLUC": "N",
        "ORIG_COV": orig_cov,
    }


def _rsmeca_values() -> dict:
    """RSMECA, INCLIC=Y INCLUC=N, full 36-field index set (App U, Table 9)."""
    npar, nparo = 3, 3
    numopg, ncseg = 3, 2
    subgroup = {
        "NUMOPG": str(numopg).zfill(2),
        "ERRCVG": _rn((numopg * (numopg + 1)) // 2),
        "TCDF": "0",
        "NCSEG": str(ncseg),
        "SEGMENTS": [
            {"CORSEG": "0" * R, "TAUSEG": "0" * R} for _ in range(ncseg)
        ],
    }
    indirect = {
        "NPAR": str(npar).zfill(2), "NPARO": str(nparo).zfill(2),
        "IGN": "01", "CVDATE": "20240101",
        **_local(),
        **_indices(),
        "SUBGROUPS": [subgroup],
        "MAP": _rn(npar * nparo),
    }
    return {
        "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
        "INCLIC": "Y", "INCLUC": "N",
        "INDIRECT_ERROR": indirect,
    }


def _rsmgga_values() -> dict:
    """RSMGGA, 1 plane, 2 grid points, TNUMRD=5 TNUMCD=6 (App U, Table 12).

    Grid-point coordinate widths track TNUMRD/TNUMCD (5/6), so a wrong width
    (e.g. the FNUMRD/FNUMCD confusion) shifts the total.
    """
    tnumrd, tnumcd = 5, 6
    return {
        "IID": "I" * 80, "EDITION": "E" * 40,
        "GGRSN": "001", "GGCSN": "001",
        "GGRFEP": "0" * 21, "GGCFEP": "0" * 21,
        "INTORD": "1", "NPLN": "001",
        "DELTAZ": "0" * 21, "DELTAX": "0" * 21, "DELTAY": "0" * 21,
        "ZPLN1": "0" * 21, "XIPLN1": "0" * 21, "YIPLN1": "0" * 21,
        "REFROW": "0" * 9, "REFCOL": "0" * 9,
        "TNUMRD": str(tnumrd).zfill(2), "TNUMCD": str(tnumcd).zfill(2),
        "FNUMRD": "1", "FNUMCD": "1",
        "DELTA_ORIGIN": [],
        "PLANES": [{
            "NXPTS": "001", "NYPTS": "002",
            "GRID_POINTS": [
                {"RCOORD": "1" * tnumrd, "CCOORD": "2" * tnumcd},
                {"RCOORD": "3" * tnumrd, "CCOORD": "4" * tnumcd},
            ],
        }],
    }


# ---------------------------------------------------------------------------
# The declarative anchor table: one row per TRE fixed during
# BUG_TRE_KSY_SPEC_CONFORMANCE. Each carries its STDI-0002 spec citation, a
# known value record, the spec-derived total encoded length, and the
# spec-derived widths of anchored top-level fields (favouring the ones that were
# previously wrong).
# ---------------------------------------------------------------------------
SPEC_ANCHORS = [
    SpecAnchor(
        tre="tre_rsmdcb",
        spec="STDI-0002 Vol 1, App U, Table 6 (pp. U-93 to U-105)",
        values=_rsmdcb_values(),
        total_bytes=1420,
        field_widths={"IID": 80, "EDITION": 40},
    ),
    SpecAnchor(
        tre="tre_rsmecb",
        spec="STDI-0002 Vol 1, App U, Table 10 (pp. U-187 to U-203)",
        values=_rsmecb_values(),
        total_bytes=1164,
        field_widths={"IID": 80, "INCLIC": 1},
    ),
    SpecAnchor(
        tre="tre_rsmeca",
        spec="STDI-0002 Vol 1, App U, Table 9 (pp. U-152 to U-165)",
        values=_rsmeca_values(),
        total_bytes=903,
        field_widths={"IID": 80, "INCLIC": 1},
    ),
    SpecAnchor(
        tre="tre_rsmapa",
        spec="STDI-0002 Vol 1, App U, Table 7 (pp. U-113 to U-120)",
        values=_rsmapa_values(),
        total_bytes=549,
        field_widths={"IID": 80, "GYX": 2},
    ),
    SpecAnchor(
        tre="tre_rsmdca",
        spec="STDI-0002 Vol 1, App U, Table 5 (pp. U-71 to U-79)",
        values=_rsmdca_values(),
        total_bytes=1099,
        field_widths={"IID": 80, "GYX": 2},
    ),
    SpecAnchor(
        tre="tre_rsmapb",
        spec="STDI-0002 Vol 1, App U, Table 8 (pp. U-128 to U-137)",
        values=_rsmapb_values(),
        total_bytes=809,
        field_widths={"IID": 80, "APTYP": 1},
    ),
    SpecAnchor(
        tre="tre_rsmgga",
        spec="STDI-0002 Vol 1, App U, Table 12 (p. U-217)",
        values=_rsmgga_values(),
        total_bytes=350,
        field_widths={"IID": 80, "TNUMRD": 2},
    ),
    SpecAnchor(
        tre="tre_prjpsb",
        spec="STDI-0002 Vol 1, App P, Table P-3 (pp. P-26 to P-28)",
        values={
            "PRN": "TC ", "PCO": "TC", "NUM_PRJ": "2",
            "PROJECTION_PARAMS": [
                {"PRJ": str(i).zfill(15)} for i in range(2)
            ],
            "XOR": "0" * 15, "YOR": "0" * 15,
        },
        total_bytes=66,  # PRN3 + PCO2 + NUM_PRJ1 + 2*PRJ15 + XOR15 + YOR15
        field_widths={"PRN": 3, "NUM_PRJ": 1},
    ),
    SpecAnchor(
        tre="tre_maplob",
        spec="STDI-0002 Vol 1, App P, Table P-6 (pp. P-37 to P-38)",
        values={
            "UNILOA": "M  ", "LOD": "00001", "LAD": "00001",
            "LSO": "0" * 15, "PSO": "0" * 15,
        },
        total_bytes=43,  # UNILOA3 + LOD5 + LAD5 + LSO15 + PSO15
        field_widths={"LOD": 5, "LAD": 5},  # the previously-wrong 9-byte widths
    ),
    SpecAnchor(
        tre="tre_sourcb",
        spec="STDI-0002 Vol 1, App P, Table P-14 (pp. P-93 to P-114)",
        values=_sourcb_values(),
        total_bytes=914,  # header 21 + source base 885 + CDV10 8
        field_widths={"IS_SCA": 9, "NUM_SOUR": 2},
    ),
    SpecAnchor(
        tre="tre_bandsb",
        spec="STDI-0002 Vol 1, App X, Table X.6-1 (pp. X-29 to X-30)",
        values=_bandsb_values(),
        total_bytes=152,  # fixed 118 + mask 4 + b14 group 16 + b13 group 14
        field_widths={"RADIOMETRIC_QUANTITY": 24, "ROW_GSD": 7},
    ),
    SpecAnchor(
        tre="tre_bchipa",
        spec="STDI-0002 Vol 1, App AR, Table AR.5-3 (pp. AR-42 to AR-60)",
        values=_bchipa_values(),
        total_bytes=111,  # prefix 47 + section A 62 + INCLUDE_B 1 + INCLUDE_C 1
        field_widths={"SDE_UUID": 36, "ISID": 10},
    ),
    SpecAnchor(
        tre="tre_fsynwa",
        spec="STDI-0002 Vol 1, App AF, Table AF-10 (pp. AF-38 to AF-39)",
        values={
            "START_FRAME_NUMBER": "000000001",
            "END_FRAME_NUMBER": "000000010",
            "CEDATA": [
                {"TRETAG": "ACCHZB", "TREL": "00005", "TREDATA": "6162636465"},
                {"TRETAG": "MENSRB", "TREL": "00003", "TREDATA": "78797a"},
            ],
        },
        total_bytes=48,  # 9+9 + (6+5+5) + (6+5+3)
        field_widths={"START_FRAME_NUMBER": 9, "END_FRAME_NUMBER": 9},
    ),
    SpecAnchor(
        tre="tre_sensrb",
        spec="STDI-0002 Vol 1, App Z, Table Z.3-1 (pp. Z-31 to Z-32, note h)",
        values=_sensrb_values(),
        total_bytes=136,  # prefix 96 + Module 12 (code 06a -> 11) 32 + suffix 8
        field_widths={"LATITUDE_OR_X": 11, "LONGITUDE_OR_Y": 12},
    ),
]

# Guard: the gate is seeded with the 14 TREs fixed during the spec audit.
assert len(SPEC_ANCHORS) == 14
assert len({a.tre for a in SPEC_ANCHORS}) == 14


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


@pytest.mark.parametrize("anchor", SPEC_ANCHORS, ids=lambda a: a.tre)
def test_spec_fidelity(registry, anchor):
    """Absolute encoded length and anchored field widths match the spec table.

    Perturbing a ``.ksy`` width/count/field-set for any of the 14 TREs shifts
    the total encoded length (or a decoded field width) and turns this red.
    """
    defn = registry.get(anchor.tre)
    assert defn is not None, f"{anchor.tre} definition not found"

    raw = defn.encode(anchor.values)
    assert len(raw) == anchor.total_bytes, (
        f"{anchor.tre}: encoded {len(raw)} bytes, spec expects "
        f"{anchor.total_bytes} ({anchor.spec})"
    )

    decoded = defn.decode(raw)
    for name, width in anchor.field_widths.items():
        assert name in decoded, f"{anchor.tre}: anchored field {name} missing"
        assert len(decoded[name]) == width, (
            f"{anchor.tre}: field {name} decoded to {len(decoded[name])} bytes, "
            f"spec expects {width} ({anchor.spec})"
        )
