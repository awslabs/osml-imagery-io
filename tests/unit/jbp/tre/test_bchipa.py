"""Spec-fidelity anchor for the BCHIPA (Appendix AR) TRE.

``BCHIPA`` (Table AR.5-3) is fixed here: its ``INCLUDE_B`` / ``INCLUDE_C`` flags
previously followed ``INCLUDE_A`` immediately and Sections B and C were collapsed
into a single ``size-eos`` blob. The flags therefore decoded *inside* the Section
A payload rather than after it. The fix models Sections B and C as their real
field sequences so each ``INCLUDE_*`` flag lands at the correct offset and every
sub-field is individually addressable. These tests assert flag offsets, per-band
loop structure (including the Section B LUT double-loop and the Section C
``WEIGHTED`` / ``FORMULAIC`` branches), and spec-derived total lengths; they fail
against the pre-fix ``.ksy``.

The WEIGHT-gating tests also cover the navigator-removal rewrite
(DESIGN_REMOVE_ROOT_PARENT_NAVIGATORS Phase 2): WEIGHT is gated on the same-type
``MAPPING_TYPE`` field (formerly ``_parent.MAPPING_TYPE``), so a WEIGHTED mapping
carries WEIGHT and a non-WEIGHTED one omits it, with the decoded dict re-encoding
byte-for-byte.

Spec: STDI-0002 Vol 1 -- BCHIPA Appendix AR, Table AR.5-3 (pp. AR-42 to AR-60).
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


# Fixed prefix: SDE_UUID(36) + NUM_INSTS(5) + INSTANCE(5) + INCLUDE_A(1).
_BCHIPA_PREFIX = 36 + 5 + 5 + 1


def _bchipa_a_block(num_bwp: int, num_sde: int) -> tuple[dict, int]:
    """Section A fields (INCLUDE_A='Y') and their exact byte length."""
    block = {
        "ISID": "IMG0000001",
        "TOT_ORIG_BANDS": "00003",
        "TOT_CURR_BANDS": "00002",
        "NUM_BWP_IS": f"{num_bwp:03d}",
        "BWP_IS": [f"{i + 1:03d}" for i in range(num_bwp)],
        "NUM_RLVNT_SDE": f"{num_sde:03d}",
        "SDE_NAME": ["TEBANDSB".ljust(32) for _ in range(num_sde)],
        "SDE_STATUS": ["O" for _ in range(num_sde)],
    }
    length = 10 + 5 + 5 + 3 + num_bwp * 3 + 3 + num_sde * (32 + 1)
    return block, length


class TestBchipaSectionOffsets:
    """INCLUDE_B / INCLUDE_C must land after the full preceding block."""

    def test_include_flags_after_section_a(self, registry):
        """With INCLUDE_A='Y', INCLUDE_B/INCLUDE_C decode after Section A, not inside it."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        a_block, _ = _bchipa_a_block(num_bwp=1, num_sde=1)
        record = {
            "SDE_UUID": " " * 36,
            "NUM_INSTS": "00001",
            "INSTANCE": "00001",
            "INCLUDE_A": "Y",
            **a_block,
            "INCLUDE_B": "N",
            "INCLUDE_C": "N",
        }
        decoded = defn.decode(defn.encode(record))

        # The flags round-trip as their own single-byte fields (not swallowed
        # into an ISID/SECTION_BC blob).
        assert decoded["INCLUDE_B"] == "N"
        assert decoded["INCLUDE_C"] == "N"
        # Section A fields are individually addressable.
        assert decoded["ISID"] == "IMG0000001"
        assert decoded["NUM_BWP_IS"] == "001"
        # The pre-fix opaque blob field must be gone.
        assert "SECTION_BC_DATA" not in decoded

    def test_total_length_sections_a_only(self, registry):
        """Absolute length with INCLUDE_A='Y', B/C off matches the field layout."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        a_block, a_len = _bchipa_a_block(num_bwp=2, num_sde=1)
        record = {
            "SDE_UUID": " " * 36, "NUM_INSTS": "00001", "INSTANCE": "00001",
            "INCLUDE_A": "Y", **a_block, "INCLUDE_B": "N", "INCLUDE_C": "N",
        }
        raw = defn.encode(record)
        # prefix + section A + INCLUDE_B(1) + INCLUDE_C(1)
        assert len(raw) == _BCHIPA_PREFIX + a_len + 1 + 1

    def test_all_sections_excluded(self, registry):
        """INCLUDE_A/B/C all 'N' -> prefix + two trailing flags only."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        record = {
            "SDE_UUID": " " * 36, "NUM_INSTS": "00001", "INSTANCE": "00002",
            "INCLUDE_A": "N", "INCLUDE_B": "N", "INCLUDE_C": "N",
        }
        raw = defn.encode(record)
        assert len(raw) == _BCHIPA_PREFIX + 1 + 1


class TestBchipaSectionB:
    """Section B original-band records, incl. the NLUTS/NELUT LUT double-loop."""

    def _record(self, bands: list[dict]) -> dict:
        return {
            "SDE_UUID": " " * 36, "NUM_INSTS": "00001", "INSTANCE": "00002",
            "INCLUDE_A": "N",
            "INCLUDE_B": "Y",
            "NUM_ORIGINAL_BANDS": f"{len(bands):05d}",
            "ORIGINAL_BANDS": bands,
            "INCLUDE_C": "N",
        }

    def _band(self, num: int, nluts: int, nelut: int = 0) -> dict:
        band = {
            "ORIG_BAND_NUMBER": f"{num:05d}",
            "IREPBAND_ORIG": "R ",
            "ISUBCAT_ORIG": " " * 8,
            "IFC_ORIG": "N",
            "IMFLT_ORIG": "   ",
            "NLUTS_ORIG": str(nluts),
        }
        if nluts != 0:
            band["NELUT_ORIG"] = f"{nelut:05d}"
            band["LUTD_ORIG"] = list(range(nluts * nelut))
        return band

    def test_nluts_zero_omits_lut_fields(self, registry):
        """A band with NLUTS_ORIG=0 carries no NELUT_ORIG/LUTD_ORIG."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record([self._band(1, nluts=0)])))
        band = decoded["ORIGINAL_BANDS"][0]
        assert "NELUT_ORIG" not in band
        assert "LUTD_ORIG" not in band

    def test_lut_double_loop_count(self, registry):
        """LUTD_ORIG holds exactly NLUTS_ORIG x NELUT_ORIG one-byte entries."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        decoded = defn.decode(defn.encode(self._record([self._band(1, nluts=2, nelut=3)])))
        band = decoded["ORIGINAL_BANDS"][0]
        assert band["NELUT_ORIG"] == "00003"
        assert len(band["LUTD_ORIG"]) == 2 * 3

    def test_section_b_total_length(self, registry):
        """Section B length = NUM_ORIGINAL_BANDS(5) + sum of per-band records."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        bands = [self._band(1, nluts=0), self._band(2, nluts=2, nelut=3)]
        raw = defn.encode(self._record(bands))
        # per-band fixed = 5+2+8+1+3+1 = 20; band2 adds NELUT(5) + 6 LUT bytes.
        # Section B = NUM_ORIGINAL_BANDS(5) + both band records.
        section_b = 5 + (20) + (20 + 5 + 6)
        # prefix(incl. INCLUDE_A='N') + INCLUDE_B(1) + section B + INCLUDE_C(1).
        assert len(raw) == _BCHIPA_PREFIX + 1 + section_b + 1


class TestBchipaSectionC:
    """Section C mapping records, incl. WEIGHTED and FORMULAIC branches.

    The WEIGHTED / non-WEIGHTED tests double as the navigator-rewrite coverage
    for WEIGHT gated on same-type MAPPING_TYPE, asserting the decoded dict
    re-encodes byte-for-byte.
    """

    def _record(self, bands: list[dict]) -> dict:
        return {
            "SDE_UUID": " " * 36, "NUM_INSTS": "00001", "INSTANCE": "00002",
            "INCLUDE_A": "N", "INCLUDE_B": "N",
            "INCLUDE_C": "Y",
            "NUM_CURR_BANDS": f"{len(bands):05d}",
            "CURRENT_BANDS": bands,
        }

    def test_weighted_branch_emits_weight(self, registry):
        """MAPPING_TYPE='WEIGHTED' -> each original-band mapping carries WEIGHT(21)."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        band = {
            "CURR_BAND_NUMBER": "00001", "SEMANTIC_SIZE": "0003",
            "SEMANTIC_MEANING": "red", "NUM_ORIG_BANDS": "00002",
            "MAPPING_TYPE": "WEIGHTED".ljust(15),
            "ORIG_BAND_MAPPINGS": [
                {"ORIG_BND_NUM": "00001", "WEIGHT": "+5.00000000000000E-01"},
                {"ORIG_BND_NUM": "00002", "WEIGHT": "+5.00000000000000E-01"},
            ],
        }
        raw = defn.encode(self._record([band]))
        decoded = defn.decode(raw)
        mappings = decoded["CURRENT_BANDS"][0]["ORIG_BAND_MAPPINGS"]
        assert len(mappings) == 2
        for m in mappings:
            assert len(m["WEIGHT"]) == 21
        # Bare-name gate resolves and the decoded dict re-encodes exactly.
        assert defn.encode(decoded) == raw

    def test_non_weighted_branch_omits_weight(self, registry):
        """A non-WEIGHTED mapping type carries no WEIGHT field."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        band = {
            "CURR_BAND_NUMBER": "00001", "SEMANTIC_SIZE": "0000",
            "SEMANTIC_MEANING": "", "NUM_ORIG_BANDS": "00001",
            "MAPPING_TYPE": "AVERAGE".ljust(15),
            "ORIG_BAND_MAPPINGS": [{"ORIG_BND_NUM": "00003"}],
        }
        raw = defn.encode(self._record([band]))
        decoded = defn.decode(raw)
        mapping = decoded["CURRENT_BANDS"][0]["ORIG_BAND_MAPPINGS"][0]
        assert "WEIGHT" not in mapping
        assert defn.encode(decoded) == raw

    def test_formulaic_branch_emits_formula(self, registry):
        """MAPPING_TYPE='FORMULAIC' -> FORMULA_SIZE(3) + variable FORMULA present."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        band = {
            "CURR_BAND_NUMBER": "00002", "SEMANTIC_SIZE": "0000",
            "SEMANTIC_MEANING": "", "NUM_ORIG_BANDS": "00001",
            "MAPPING_TYPE": "FORMULAIC".ljust(15),
            "ORIG_BAND_MAPPINGS": [{"ORIG_BND_NUM": "00003"}],
            "FORMULA_SIZE": "008", "FORMULA": "255 - b1",
        }
        decoded = defn.decode(defn.encode(self._record([band])))
        current = decoded["CURRENT_BANDS"][0]
        assert current["FORMULA_SIZE"] == "008"
        assert current["FORMULA"] == "255 - b1"
        assert len(current["FORMULA"]) == 8

    def test_semantic_meaning_variable_width(self, registry):
        """SEMANTIC_MEANING width tracks SEMANTIC_SIZE across values."""
        defn = registry.get("tre_bchipa")
        assert defn is not None

        for size, text in [(3, "abc"), (0, ""), (10, "wavelength")]:
            band = {
                "CURR_BAND_NUMBER": "00001", "SEMANTIC_SIZE": f"{size:04d}",
                "SEMANTIC_MEANING": text, "NUM_ORIG_BANDS": "00000",
                "MAPPING_TYPE": "IDENTICAL".ljust(15),
            }
            decoded = defn.decode(defn.encode(self._record([band])))
            assert decoded["CURRENT_BANDS"][0]["SEMANTIC_MEANING"] == text


def test_bchipa_round_trip_all_sections(registry):
    """A record exercising Sections A, B, and C round-trips faithfully."""
    defn = registry.get("tre_bchipa")
    assert defn is not None

    a_block, _ = _bchipa_a_block(num_bwp=1, num_sde=1)
    record = {
        "SDE_UUID": " " * 36, "NUM_INSTS": "00001", "INSTANCE": "00001",
        "INCLUDE_A": "Y", **a_block,
        "INCLUDE_B": "Y", "NUM_ORIGINAL_BANDS": "00001",
        "ORIGINAL_BANDS": [{
            "ORIG_BAND_NUMBER": "00001", "IREPBAND_ORIG": "R ",
            "ISUBCAT_ORIG": " " * 8, "IFC_ORIG": "N", "IMFLT_ORIG": "   ",
            "NLUTS_ORIG": "1", "NELUT_ORIG": "00004", "LUTD_ORIG": [0, 1, 2, 3],
        }],
        "INCLUDE_C": "Y", "NUM_CURR_BANDS": "00001",
        "CURRENT_BANDS": [{
            "CURR_BAND_NUMBER": "00001", "SEMANTIC_SIZE": "0003",
            "SEMANTIC_MEANING": "red", "NUM_ORIG_BANDS": "00001",
            "MAPPING_TYPE": "WEIGHTED".ljust(15),
            "ORIG_BAND_MAPPINGS": [
                {"ORIG_BND_NUM": "00001", "WEIGHT": "+1.00000000000000E+00"}
            ],
        }],
    }
    raw = defn.encode(record)
    assert defn.encode(defn.decode(raw)) == raw
