"""Spec-fidelity anchor for the RSMDCB covariance TRE.

RSMDCB was substantially incomplete before the BUG_TRE_KSY_SPEC_CONFORMANCE
Phase 1 fix: it omitted the entire CRSCOV cross-covariance payload and most of
the adjustable-parameter block.

Each test asserts, against a spec-derived field configuration, the properties a
self-consistent round trip cannot catch: **field presence**, **repeat count**,
and **absolute total encoded length** taken from the spec's byte layout. They
fail against the pre-fix ``.ksy`` (which cannot produce the modeled fields at
the spec offsets or the spec total length).

Spec: STDI-0002 Vol 1, Appendix U -- RSMDCB Table 6 (pp. U-93 to U-105).
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


class TestRsmdcb:
    """RSMDCB models the CRSCOV payload as a nested per-image repeat.

    ``CRSCOV`` is, per Table 6, a per-image concatenation: for each of the
    ``NIMGE`` images, a block of ``NROWCB * NCOLCB[image]`` elements of 21
    BCS-A real, where ``NCOLCB`` is a field of that image's ``IMAGE_RECORDS``
    entry (it varies per image). The definition models this as a
    ``crscov_block(_index)`` repeat whose inner count is
    ``NROWCB.to_i * IMAGE_RECORDS[image_index].NCOLCB.to_i`` — the block
    boundaries are recovered by indexing back into the earlier image loop.

    The per-image ``NCOLCB`` values below are deliberately **distinct** (3 and
    5). A flat total-count model (the old ``size-eos`` blob) would decode the
    same total element count regardless of how it splits across images, so it
    could not tell a 3/5 split from a 4/4 split; asserting the individual block
    lengths is a spec-fidelity check a self-consistent round trip is blind to.
    """

    # Distinct per-image column counts prove the per-image block boundaries.
    _NCOLCB = [3, 5]

    def _record(self) -> dict:
        # INCAPD=Y, APTYP=I, LOCTYP=R, APBASE=Y; NIMGE=2 images.
        nrowcb, npar, nbasis = 3, 3, 3
        images = [
            {"IIDI": "A" * 80, "NCOLCB": str(self._NCOLCB[0]).zfill(2)},
            {"IIDI": "B" * 80, "NCOLCB": str(self._NCOLCB[1]).zfill(2)},
        ]
        ap_data = {
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
        }
        # One CRSCOV block per image; block i holds NROWCB * NCOLCB[i] elements.
        crscov_blocks = [{"CRSCOV": _r(nrowcb * ncolcb)} for ncolcb in self._NCOLCB]
        return {
            "IID": "I" * 80, "EDITION": "E" * 40, "TID": "T" * 40,
            "NROWCB": str(nrowcb).zfill(2),
            "NIMGE": "002",
            "IMAGE_RECORDS": images,
            "INCAPD": "Y",
            "AP_DATA": ap_data,
            "CRSCOV_BLOCKS": crscov_blocks,
        }

    def test_crscov_block_structure_and_total_length(self, registry):
        """CRSCOV decodes into per-image blocks with spec-derived boundaries."""
        defn = registry.get("tre_rsmdcb")
        assert defn is not None

        raw = defn.encode(self._record())
        decoded = defn.decode(raw)

        nrowcb = 3
        assert "CRSCOV_BLOCKS" in decoded, "CRSCOV covariance payload missing"
        blocks = decoded["CRSCOV_BLOCKS"]
        # One block per image (NIMGE=2).
        assert len(blocks) == len(self._NCOLCB)
        # Each block holds exactly NROWCB * NCOLCB[image] elements — the
        # ground-truth structural check the flat blob could not provide.
        for block, ncolcb in zip(blocks, self._NCOLCB):
            assert len(block["CRSCOV"]) == nrowcb * ncolcb
        # Total across all blocks is NROWCB * sum(NCOLCB).
        total = sum(len(b["CRSCOV"]) for b in blocks)
        assert total == nrowcb * sum(self._NCOLCB)

        # header 80+40+40+2+3 = 165
        # images 2*(80+2) = 164 ; INCAPD 1
        # ap_data: NPAR2+APTYP1+LOCTYP1 + 6*21 (norm) + 12*21 (local) + APBASE1
        #          + image_ap(NISAP2+NISAPR2+3 row pow+NISAPC2+3 col pow = 12)
        #          + NBASIS2 + AEL (NPAR*NBASIS=9)*21
        ap_len = 2 + 1 + 1 + 6 * R + 12 * R + 1 + 12 + 2 + 9 * R
        crscov_len = R * nrowcb * sum(self._NCOLCB)  # NROWCB * sum(NCOLCB)
        expected = 165 + 164 + 1 + ap_len + crscov_len
        assert len(raw) == expected

    def test_local_coord_and_basis_matrix_modeled(self, registry):
        """The LOCTYP=R block and APBASE=Y basis matrix are addressable fields."""
        defn = registry.get("tre_rsmdcb")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record()))
        ap = decoded["AP_DATA"]
        # LOCTYP=R local coordinate block present with all 12 fields.
        assert set(LOCAL_KEYS).issubset(ap["LOCAL_COORD"].keys())
        # APBASE=Y basis matrix: NPAR*NBASIS elements.
        assert len(ap["AEL"]) == 3 * 3

    def test_round_trip(self, registry):
        defn = registry.get("tre_rsmdcb")
        assert defn is not None
        raw = defn.encode(self._record())
        assert defn.encode(defn.decode(raw)) == raw
