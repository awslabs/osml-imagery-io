"""Unit tests for BANDSB TRE parsing, specifically the conditional WAVE_LENGTH_UNIT field.

Verifies that WAVE_LENGTH_UNIT is only parsed when wavelength bits (b24-b19) are set
in the EXISTENCE_MASK. See: BUG_BANDSB_WAVE_LENGTH_UNIT_UNCONDITIONAL.md
"""

import struct
from pathlib import Path

from aws.osml.io import StructureAccessor, StructureRegistry

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
    """Tests for WAVE_LENGTH_UNIT conditionality on EXISTENCE_MASK bits b24-b19."""

    def test_no_wavelength_bits_omits_wave_length_unit(self):
        """When no wavelength bits (b24-b19) are set, WAVE_LENGTH_UNIT must be absent."""
        header = _build_bandsb_header(count=1)
        mask = 0x00000000
        blob = header + struct.pack(">I", mask)

        defn = _get_bandsb_definition()
        accessor = StructureAccessor(defn, blob)

        assert accessor.has("EXISTENCE_MASK")
        assert accessor["EXISTENCE_MASK"].as_int() == 0
        assert not accessor.has("WAVE_LENGTH_UNIT")

    def test_niirs_only_mask_omits_wave_length_unit(self):
        """Mask with only b26 (NIIRS) set should not produce WAVE_LENGTH_UNIT."""
        header = _build_bandsb_header(count=1)
        mask = 0x04000000  # b26 only
        niirs_data = b"3.5"  # 3 bytes per band, 1 band
        blob = header + struct.pack(">I", mask) + niirs_data

        defn = _get_bandsb_definition()
        accessor = StructureAccessor(defn, blob)

        assert accessor["EXISTENCE_MASK"].as_int() == mask
        assert not accessor.has("WAVE_LENGTH_UNIT")
        assert accessor.has("NIIRS")

    def test_b24_set_includes_wave_length_unit(self):
        """When b24 (CWAVE) is set, WAVE_LENGTH_UNIT must be present."""
        header = _build_bandsb_header(count=1)
        mask = 0x01000000  # b24 only
        wave_unit = b"U"  # micrometers
        cwave_data = b"0000.55"  # 7 bytes per band, 1 band
        blob = header + struct.pack(">I", mask) + wave_unit + cwave_data

        defn = _get_bandsb_definition()
        accessor = StructureAccessor(defn, blob)

        assert accessor["EXISTENCE_MASK"].as_int() == mask
        assert accessor.has("WAVE_LENGTH_UNIT")
        assert accessor["WAVE_LENGTH_UNIT"].as_str() == "U"
        assert accessor.has("CWAVE")

    def test_b19_set_includes_wave_length_unit(self):
        """When b19 (LBOUND/UBOUND) is set, WAVE_LENGTH_UNIT must be present."""
        header = _build_bandsb_header(count=1)
        mask = 0x00080000  # b19 only
        wave_unit = b"W"  # wavenumber
        lbound_data = b"0800.00"  # 7 bytes per band
        ubound_data = b"1200.00"  # 7 bytes per band
        blob = header + struct.pack(">I", mask) + wave_unit + lbound_data + ubound_data

        defn = _get_bandsb_definition()
        accessor = StructureAccessor(defn, blob)

        assert accessor["EXISTENCE_MASK"].as_int() == mask
        assert accessor.has("WAVE_LENGTH_UNIT")
        assert accessor["WAVE_LENGTH_UNIT"].as_str() == "W"
        assert accessor.has("LBOUND")
        assert accessor.has("UBOUND")

    def test_multiple_wavelength_bits_includes_wave_length_unit(self):
        """When multiple wavelength bits are set, WAVE_LENGTH_UNIT is still read once."""
        header = _build_bandsb_header(count=1)
        mask = 0x01800000  # b24 + b23 (CWAVE + FWHM)
        wave_unit = b"U"
        cwave_data = b"0000.55"  # 7 bytes per band
        fwhm_data = b"0000.10"  # 7 bytes per band
        blob = header + struct.pack(">I", mask) + wave_unit + cwave_data + fwhm_data

        defn = _get_bandsb_definition()
        accessor = StructureAccessor(defn, blob)

        assert accessor.has("WAVE_LENGTH_UNIT")
        assert accessor["WAVE_LENGTH_UNIT"].as_str() == "U"
        assert accessor.has("CWAVE")
        assert accessor.has("FWHM")
