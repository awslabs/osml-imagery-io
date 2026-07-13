"""Spec-fidelity anchor for the BANDSB TRE.

Two concerns:

* The conditional ``WAVE_LENGTH_UNIT`` field is only parsed when wavelength bits
  (b24-b19) are set in the EXISTENCE_MASK.
  See: BUG_BANDSB_WAVE_LENGTH_UNIT_UNCONDITIONAL.md
* Spec-fidelity (BUG_TRE_KSY_SPEC_CONFORMANCE Phase 4): the GSD (b14/b13) and
  spatial-response (b11/b10) per-band groups. Per STDI-0002 Vol 1, App X
  Table X.6-1 (pp. X-29/X-30) each axis has ONE unit field, shared between the
  value and its uncertainty, and the fields interleave as value, uncertainty,
  unit. So the b13/b10 uncertainty contribution is **14 bytes/band** (two 7-byte
  values), not 16. The pre-fix ``.ksy`` invented a second per-axis unit field
  under b13/b10 (``BAND_ROW_GSD_UNC_UNIT``/``BAND_COL_GSD_UNC_UNIT`` and the SPT
  equivalents) AND grouped the b13/b10 fields into a separate trailing block, so
  both the byte count and the field order were wrong. These assertions — byte
  count, field absence, and exact interleaved order — fail against the pre-fix
  definition.

Spec: STDI-0002 Vol 1, App X, Table X.6-1 (pp. X-29 to X-30).
"""

import struct
from pathlib import Path

from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")


def _build_bandsb_header(count: int = 1) -> bytes:
    """Build the fixed portion of a BANDSB TRE (122 bytes before EXISTENCE_MASK).

    Fields (in order):
      COUNT                     5 BCS-N
      RADIOMETRIC_QUANTITY     24 BCS-A
      RADIOMETRIC_QUANTITY_UNIT 1 BCS-A
      SCALE_FACTOR              4 (IEEE754)
      ADDITIVE_FACTOR           4 (IEEE754)
      ROW_GSD                   7 BCS-N
      ROW_GSD_UNIT              1 BCS-A
      COL_GSD                   7 BCS-N
      COL_GSD_UNIT              1 BCS-A
      SPT_RESP_ROW              7 BCS-N
      SPT_RESP_UNIT_ROW         1 BCS-A
      SPT_RESP_COL              7 BCS-N
      SPT_RESP_UNIT_COL         1 BCS-A
      DATA_FLD_1               48
    Total = 5+24+1+4+4+7+1+7+1+7+1+7+1+48 = 118 bytes
    """
    buf = b""
    buf += f"{count:05d}".encode("ascii")  # COUNT (5)
    buf += b"RADIANCE" + b" " * 16  # RADIOMETRIC_QUANTITY (24)
    buf += b"S"  # RADIOMETRIC_QUANTITY_UNIT (1)
    buf += struct.pack(">f", 1.0)  # SCALE_FACTOR (4)
    buf += struct.pack(">f", 0.0)  # ADDITIVE_FACTOR (4)
    buf += b"0001.00"  # ROW_GSD (7)
    buf += b"M"  # ROW_GSD_UNIT (1)
    buf += b"0001.00"  # COL_GSD (7)
    buf += b"M"  # COL_GSD_UNIT (1)
    buf += b"0001.00"  # SPT_RESP_ROW (7)
    buf += b"M"  # SPT_RESP_UNIT_ROW (1)
    buf += b"0001.00"  # SPT_RESP_COL (7)
    buf += b"M"  # SPT_RESP_UNIT_COL (1)
    buf += b"\x00" * 48  # DATA_FLD_1 (48)
    assert len(buf) == 118
    return buf


def _get_bandsb_definition():
    """Load the BANDSB structure definition."""
    registry = StructureRegistry()
    registry.add_search_path(str(STRUCTURES_DIR))
    defn = registry.get("tre_bandsb")
    assert defn is not None, "tre_bandsb definition not found"
    return defn


class TestBandsbWaveLengthUnitConditional:
    """Tests for WAVE_LENGTH_UNIT conditionality on EXISTENCE_MASK bits b24-b19.

    Decode direction is driven by externally-built blobs (``struct.pack`` +
    hand-assembled header) so ``decode`` is exercised against ground-truth bytes
    rather than circularly against ``encode``'s own output.
    """

    def test_no_wavelength_bits_omits_wave_length_unit(self):
        """When no wavelength bits (b24-b19) are set, WAVE_LENGTH_UNIT must be absent."""
        header = _build_bandsb_header(count=1)
        mask = 0x00000000
        blob = header + struct.pack(">I", mask)

        defn = _get_bandsb_definition()
        decoded = defn.decode(blob)

        assert "EXISTENCE_MASK" in decoded
        assert decoded["EXISTENCE_MASK"] == 0
        assert "WAVE_LENGTH_UNIT" not in decoded

    def test_niirs_only_mask_omits_wave_length_unit(self):
        """Mask with only b26 (NIIRS) set should not produce WAVE_LENGTH_UNIT."""
        header = _build_bandsb_header(count=1)
        mask = 0x04000000  # b26 only
        niirs_data = b"3.5"  # 3 bytes per band, 1 band
        blob = header + struct.pack(">I", mask) + niirs_data

        defn = _get_bandsb_definition()
        decoded = defn.decode(blob)

        assert decoded["EXISTENCE_MASK"] == mask
        assert "WAVE_LENGTH_UNIT" not in decoded
        assert "NIIRS" in decoded

    def test_b24_set_includes_wave_length_unit(self):
        """When b24 (CWAVE) is set, WAVE_LENGTH_UNIT must be present."""
        header = _build_bandsb_header(count=1)
        mask = 0x01000000  # b24 only
        wave_unit = b"U"  # micrometers
        cwave_data = b"0000.55"  # 7 bytes per band, 1 band
        blob = header + struct.pack(">I", mask) + wave_unit + cwave_data

        defn = _get_bandsb_definition()
        decoded = defn.decode(blob)

        assert decoded["EXISTENCE_MASK"] == mask
        assert "WAVE_LENGTH_UNIT" in decoded
        assert decoded["WAVE_LENGTH_UNIT"] == "U"
        assert "CWAVE" in decoded

    def test_b19_set_includes_wave_length_unit(self):
        """When b19 (LBOUND/UBOUND) is set, WAVE_LENGTH_UNIT must be present."""
        header = _build_bandsb_header(count=1)
        mask = 0x00080000  # b19 only
        wave_unit = b"W"  # wavenumber
        lbound_data = b"0800.00"  # 7 bytes per band
        ubound_data = b"1200.00"  # 7 bytes per band
        blob = header + struct.pack(">I", mask) + wave_unit + lbound_data + ubound_data

        defn = _get_bandsb_definition()
        decoded = defn.decode(blob)

        assert decoded["EXISTENCE_MASK"] == mask
        assert "WAVE_LENGTH_UNIT" in decoded
        assert decoded["WAVE_LENGTH_UNIT"] == "W"
        assert "LBOUND" in decoded
        assert "UBOUND" in decoded

    def test_multiple_wavelength_bits_includes_wave_length_unit(self):
        """When multiple wavelength bits are set, WAVE_LENGTH_UNIT is still read once."""
        header = _build_bandsb_header(count=1)
        mask = 0x01800000  # b24 + b23 (CWAVE + FWHM)
        wave_unit = b"U"
        cwave_data = b"0000.55"  # 7 bytes per band
        fwhm_data = b"0000.10"  # 7 bytes per band
        blob = header + struct.pack(">I", mask) + wave_unit + cwave_data + fwhm_data

        defn = _get_bandsb_definition()
        decoded = defn.decode(blob)

        assert "WAVE_LENGTH_UNIT" in decoded
        assert decoded["WAVE_LENGTH_UNIT"] == "U"
        assert "CWAVE" in decoded
        assert "FWHM" in decoded


# ---------------------------------------------------------------------------
# Spec-fidelity: GSD (b14/b13) and spatial-response (b11/b10) group layout
# (App X, Table X.6-1, pp. X-29/X-30)
# ---------------------------------------------------------------------------
#
# Per the field table, each axis has ONE unit field shared between the value and
# its uncertainty, and the fields interleave in this on-disk order:
#   GSD group:  ROW_GSDn, ROW_GSD_UNCn, ROW_GSD_UNITn,
#               COL_GSDn, COL_GSD_UNCn, COL_GSD_UNITn
#   SPT group:  SPT_RESP_FUNCTION_ROWn, SPT_RESP_UNC_ROWn, SPT_RESP_UNIT_ROWn,
#               SPT_RESP_FUNCTION_COLn, SPT_RESP_UNC_COLn, SPT_RESP_UNIT_COLn
# so the b13/b10 uncertainty contribution is 14 bytes/band (two 7-byte values,
# no unit of their own). The pre-fix .ksy invented a second unit field per axis
# under b13/b10 (making the group 16) AND grouped all b13/b10 fields into a
# separate trailing block, so BOTH the byte count and the field order were wrong.

# EXISTENCE_MASK bits. b13/b10 each require their "parent" GSD/SPT group (b14/b11)
# to be set as well, per Table X.6-1.
_B14 = 0x00004000  # ROW_GSDn(7) ROW_GSD_UNITn(1) COL_GSDn(7) COL_GSD_UNITn(1) = 16/band
_B13 = 0x00002000  # ROW_GSD_UNCn(7) COL_GSD_UNCn(7)                            = 14/band
_B11 = 0x00000800  # SPT_RESP_FUNCTION/UNIT row+col                             = 16/band
_B10 = 0x00000400  # SPT_RESP_UNC_ROWn(7) SPT_RESP_UNC_COLn(7)                  = 14/band

# Byte offset where the per-band region starts: 118-byte fixed header + the
# 4-byte EXISTENCE_MASK (see _build_bandsb_header).
_PER_BAND_OFFSET = 118 + 4


def _bandsb_value(mask: int, count: int) -> dict:
    """A BANDSB value dict for ``encode`` with the fixed header and the per-band
    arrays required by ``mask`` populated for ``count`` bands.

    Only the b14/b13/b11/b10 groups are exercised here (the groups under test);
    the mask passed in must not enable other per-band bits.
    """
    value: dict = {
        "COUNT": f"{count:05d}",
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
    }
    if mask & _B14:
        value["BAND_ROW_GSD"] = ["0001.00"] * count
        value["BAND_ROW_GSD_UNIT"] = ["M"] * count
        value["BAND_COL_GSD"] = ["0001.00"] * count
        value["BAND_COL_GSD_UNIT"] = ["M"] * count
    if mask & _B13:
        value["BAND_ROW_GSD_UNC"] = ["0000.10"] * count
        value["BAND_COL_GSD_UNC"] = ["0000.10"] * count
    if mask & _B11:
        value["BAND_SPT_RESP_FUNCTION_ROW"] = ["0001.00"] * count
        value["BAND_SPT_RESP_UNIT_ROW"] = ["M"] * count
        value["BAND_SPT_RESP_FUNCTION_COL"] = ["0001.00"] * count
        value["BAND_SPT_RESP_UNIT_COL"] = ["M"] * count
    if mask & _B10:
        value["BAND_SPT_RESP_UNC_ROW"] = ["0000.10"] * count
        value["BAND_SPT_RESP_UNC_COL"] = ["0000.10"] * count
    return value


def _per_band_bytes(defn, mask: int) -> int:
    """Bytes that one extra band adds to the record under ``mask``."""
    one = len(defn.encode(_bandsb_value(mask, 1)))
    two = len(defn.encode(_bandsb_value(mask, 2)))
    return two - one


class TestBandsbUncertaintyGroupWidths:
    """b13/b10 per-band groups must be 14 bytes/band (no invented unit fields)."""

    def test_b13_group_is_14_bytes_per_band(self):
        """b13 (GSD uncertainty) contributes exactly 14 bytes/band, not 16.

        Isolated by differencing against a b14-only record: the b14 group is
        16 bytes/band, so (b14|b13) minus b14 leaves the b13 group alone.
        """
        defn = _get_bandsb_definition()
        b14_only = _per_band_bytes(defn, _B14)
        b14_and_b13 = _per_band_bytes(defn, _B14 | _B13)
        assert b14_only == 16, "b14 group must be 7+1+7+1 = 16 bytes/band"
        assert b14_and_b13 - b14_only == 14, (
            "b13 GSD-uncertainty group must be 7+7 = 14 bytes/band "
            "(the pre-fix .ksy carried two extra 1-byte unit fields = 16)"
        )

    def test_b10_group_is_14_bytes_per_band(self):
        """b10 (spatial-response uncertainty) contributes exactly 14 bytes/band."""
        defn = _get_bandsb_definition()
        b11_only = _per_band_bytes(defn, _B11)
        b11_and_b10 = _per_band_bytes(defn, _B11 | _B10)
        assert b11_only == 16, "b11 group must be 7+1+7+1 = 16 bytes/band"
        assert b11_and_b10 - b11_only == 14, (
            "b10 spatial-response-uncertainty group must be 7+7 = 14 bytes/band "
            "(the pre-fix .ksy carried two extra 1-byte unit fields = 16)"
        )

    def test_invented_b13_b10_unit_fields_absent(self):
        """The four invented per-band unit fields must not appear in a decode."""
        defn = _get_bandsb_definition()
        decoded = defn.decode(defn.encode(_bandsb_value(_B14 | _B13 | _B11 | _B10, 1)))
        for gone in (
            "BAND_ROW_GSD_UNC_UNIT",
            "BAND_COL_GSD_UNC_UNIT",
            "BAND_SPT_RESP_UNC_UNIT_ROW",
            "BAND_SPT_RESP_UNC_UNIT_COL",
        ):
            assert gone not in decoded, f"invented field {gone} must be removed"
        # The uncertainty payloads themselves are still present.
        assert "BAND_ROW_GSD_UNC" in decoded
        assert "BAND_SPT_RESP_UNC_ROW" in decoded

    def test_gsd_group_interleaved_field_order(self):
        """b14|b13 GSD fields serialize in spec order, unit AFTER its uncertainty.

        Encodes one band with a distinguishable value per field and asserts the
        exact byte layout: ROW_GSD, ROW_GSD_UNC, ROW_GSD_UNIT, COL_GSD,
        COL_GSD_UNC, COL_GSD_UNIT. The pre-fix layout grouped all b13 fields into
        a trailing block (ROW_GSD, ROW_GSD_UNIT, COL_GSD, COL_GSD_UNIT, ROW_UNC,
        COL_UNC), so this fails against it even though the byte *count* matches.
        """
        defn = _get_bandsb_definition()
        value = _bandsb_value(_B14 | _B13, count=1)
        value["BAND_ROW_GSD"] = ["RRRRRRR"]
        value["BAND_ROW_GSD_UNC"] = ["uuuuuuu"]
        value["BAND_ROW_GSD_UNIT"] = ["M"]
        value["BAND_COL_GSD"] = ["CCCCCCC"]
        value["BAND_COL_GSD_UNC"] = ["vvvvvvv"]
        value["BAND_COL_GSD_UNIT"] = ["R"]

        raw = defn.encode(value)
        region = raw[_PER_BAND_OFFSET:]
        assert region == b"RRRRRRRuuuuuuuMCCCCCCCvvvvvvvR", (
            "GSD fields must interleave value/uncertainty/unit per axis "
            "(Table X.6-1, p. X-29)"
        )

    def test_spt_group_interleaved_field_order(self):
        """b11|b10 spatial-response fields serialize in spec order (unit last per axis)."""
        defn = _get_bandsb_definition()
        value = _bandsb_value(_B11 | _B10, count=1)
        value["BAND_SPT_RESP_FUNCTION_ROW"] = ["FFFFFFF"]
        value["BAND_SPT_RESP_UNC_ROW"] = ["ggggggg"]
        value["BAND_SPT_RESP_UNIT_ROW"] = ["M"]
        value["BAND_SPT_RESP_FUNCTION_COL"] = ["HHHHHHH"]
        value["BAND_SPT_RESP_UNC_COL"] = ["iiiiiii"]
        value["BAND_SPT_RESP_UNIT_COL"] = ["R"]

        raw = defn.encode(value)
        region = raw[_PER_BAND_OFFSET:]
        assert region == b"FFFFFFFgggggggMHHHHHHHiiiiiiiR", (
            "SPT fields must interleave function/uncertainty/unit per axis "
            "(Table X.6-1, p. X-30)"
        )

    def test_round_trip(self):
        defn = _get_bandsb_definition()
        raw = defn.encode(_bandsb_value(_B14 | _B13 | _B11 | _B10, 2))
        assert defn.encode(defn.decode(raw)) == raw
