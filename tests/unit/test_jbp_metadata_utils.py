"""Example-based unit tests for all JBP metadata utility adapters."""

from datetime import datetime

import pytest
from aws.osml.io.jbp.utils import (
    DateTimeAdapter,
    IGEOLOAdapter,
    LocationAdapter,
    NitfDateTime,
    SecurityClassification,
    SecurityClassificationAdapter,
    TGTIDAdapter,
    UTMCoordinate,
)

# ── DateTimeAdapter Tests ──────────────────────────────────────────────


class TestDateTimeParse:
    """Tests for DateTimeAdapter.parse()."""

    def test_parse_complete_datetime(self):
        result = DateTimeAdapter.parse("20231215143022")
        assert result == datetime(2023, 12, 15, 14, 30, 22)

    def test_parse_partial_unknowns(self):
        result = DateTimeAdapter.parse("2023--15------")
        assert isinstance(result, NitfDateTime)
        assert result.year == 2023
        assert result.month is None
        assert result.day == 15
        assert result.hour is None
        assert result.minute is None
        assert result.second is None

    def test_parse_all_unknowns_except_year(self):
        result = DateTimeAdapter.parse("2023----------")
        assert isinstance(result, NitfDateTime)
        assert result.year == 2023
        assert result.month is None
        assert result.day is None
        assert result.hour is None
        assert result.minute is None
        assert result.second is None

    def test_parse_min_boundary(self):
        result = DateTimeAdapter.parse("00010101000000")
        assert result == datetime(1, 1, 1, 0, 0, 0)

    def test_parse_max_boundary(self):
        result = DateTimeAdapter.parse("99991231235959")
        assert result == datetime(9999, 12, 31, 23, 59, 59)

    def test_error_wrong_length_short(self):
        with pytest.raises(ValueError, match="14"):
            DateTimeAdapter.parse("2023121514")

    def test_error_wrong_length_long(self):
        with pytest.raises(ValueError, match="14"):
            DateTimeAdapter.parse("202312151430221")

    def test_error_out_of_range_month(self):
        with pytest.raises(ValueError, match="month"):
            DateTimeAdapter.parse("20231315143022")

    def test_error_out_of_range_day(self):
        with pytest.raises(ValueError, match="day"):
            DateTimeAdapter.parse("20231232143022")

    def test_error_out_of_range_hour(self):
        with pytest.raises(ValueError, match="hour"):
            DateTimeAdapter.parse("20231215243022")

    def test_error_out_of_range_minute(self):
        with pytest.raises(ValueError, match="minute"):
            DateTimeAdapter.parse("20231215146022")

    def test_error_out_of_range_second(self):
        with pytest.raises(ValueError, match="second"):
            DateTimeAdapter.parse("20231215143060")

    def test_error_non_numeric(self):
        with pytest.raises(ValueError):
            DateTimeAdapter.parse("2023AB15143022")


class TestDateTimeFormat:
    """Tests for DateTimeAdapter.format()."""

    def test_format_datetime_roundtrip(self):
        original = "20231215143022"
        result = DateTimeAdapter.format(DateTimeAdapter.parse(original))
        assert result == original

    def test_format_nitf_datetime_with_nones(self):
        ndt = NitfDateTime(year=2023, month=None, day=15, hour=None, minute=None, second=None)
        result = DateTimeAdapter.format(ndt)
        assert result == "2023--15------"
        assert len(result) == 14

    def test_format_all_unknowns_except_year(self):
        ndt = NitfDateTime(year=2023)
        result = DateTimeAdapter.format(ndt)
        assert result == "2023----------"
        assert len(result) == 14

    def test_format_datetime_object(self):
        dt = datetime(2023, 12, 15, 14, 30, 22)
        result = DateTimeAdapter.format(dt)
        assert result == "20231215143022"
        assert len(result) == 14


# ── IGEOLOAdapter Tests ────────────────────────────────────────────────


class TestIGEOLOGeographic:
    """Tests for IGEOLO parsing/formatting with ICORDS=G."""

    def test_parse_geographic_four_corners(self):
        # 342651N0975423W repeated 4 times
        corner = "342651N0975423W"
        igeolo = corner * 4
        result = IGEOLOAdapter.parse(igeolo, "G")
        assert len(result) == 4
        lat, lon = result[0]
        # 34°26'51" N = 34 + 26/60 + 51/3600 = 34.4475
        assert abs(lat - 34.4475) < 1e-6
        # 097°54'23" W = -(97 + 54/60 + 23/3600) = -97.90638...
        assert abs(lon - (-97.90638888888889)) < 1e-4

    def test_hemisphere_signs(self):
        # N → positive lat, S → negative lat, E → positive lon, W → negative lon
        n_e = "100000N0100000E"
        s_w = "100000S0100000W"
        n_w = "100000N0100000W"
        s_e = "100000S0100000E"
        igeolo = n_e + s_w + n_w + s_e
        result = IGEOLOAdapter.parse(igeolo, "G")
        assert result[0][0] > 0  # N → positive
        assert result[0][1] > 0  # E → positive
        assert result[1][0] < 0  # S → negative
        assert result[1][1] < 0  # W → negative
        assert result[2][0] > 0  # N → positive
        assert result[2][1] < 0  # W → negative
        assert result[3][0] < 0  # S → negative
        assert result[3][1] > 0  # E → positive

    def test_geographic_roundtrip(self):
        corner = "342651N0975423W"
        original = corner * 4
        coords = IGEOLOAdapter.parse(original, "G")
        result = IGEOLOAdapter.format(coords, "G")
        assert result == original


class TestIGEOLODecimal:
    """Tests for IGEOLO parsing/formatting with ICORDS=D."""

    def test_parse_decimal_four_corners(self):
        corner = "+34.442-097.906"
        igeolo = corner * 4
        result = IGEOLOAdapter.parse(igeolo, "D")
        assert len(result) == 4
        lat, lon = result[0]
        assert abs(lat - 34.442) < 1e-6
        assert abs(lon - (-97.906)) < 1e-6

    def test_decimal_roundtrip(self):
        corner = "+34.442-097.906"
        original = corner * 4
        coords = IGEOLOAdapter.parse(original, "D")
        result = IGEOLOAdapter.format(coords, "D")
        assert result == original


class TestIGEOLOUTM:
    """Tests for IGEOLO parsing/formatting with ICORDS=N."""

    def test_parse_utm_four_corners(self):
        corner = "140500001234567"
        igeolo = corner * 4
        result = IGEOLOAdapter.parse(igeolo, "N")
        assert len(result) == 4
        assert isinstance(result[0], UTMCoordinate)
        assert result[0].zone == 14
        assert result[0].easting == 50000
        assert result[0].northing == 1234567

    def test_utm_roundtrip(self):
        corner = "140500001234567"
        original = corner * 4
        coords = IGEOLOAdapter.parse(original, "N")
        result = IGEOLOAdapter.format(coords, "N")
        assert result == original


class TestIGEOLOMGRS:
    """Tests for IGEOLO parsing/formatting with ICORDS=U."""

    def test_parse_mgrs_four_corners(self):
        # 15-char MGRS strings (padded with spaces to 15 chars)
        mgrs_str = "18SUJ2337106519"
        igeolo = mgrs_str * 4
        result = IGEOLOAdapter.parse(igeolo, "U")
        assert len(result) == 4
        assert result[0] == "18SUJ2337106519"

    def test_parse_mgrs_strips_trailing_whitespace(self):
        mgrs_padded = "18SUJ23371     "
        igeolo = mgrs_padded * 4
        result = IGEOLOAdapter.parse(igeolo, "U")
        assert result[0] == "18SUJ23371"

    def test_mgrs_roundtrip_with_padding(self):
        mgrs_padded = "18SUJ23371     "
        original = mgrs_padded * 4
        coords = IGEOLOAdapter.parse(original, "U")
        result = IGEOLOAdapter.format(coords, "U")
        assert result == original


class TestIGEOLOErrors:
    """Tests for IGEOLO error handling."""

    def test_error_wrong_length(self):
        with pytest.raises(ValueError, match="60"):
            IGEOLOAdapter.parse("too_short", "G")

    def test_error_invalid_hemisphere(self):
        # Replace N with X in first corner
        corner = "342651X0975423W"
        igeolo = corner + "342651N0975423W" * 3
        with pytest.raises(ValueError, match="hemisphere"):
            IGEOLOAdapter.parse(igeolo, "G")

    def test_error_unsupported_icords(self):
        igeolo = "0" * 60
        with pytest.raises(ValueError, match="ICORDS"):
            IGEOLOAdapter.parse(igeolo, "Z")


# ── SecurityClassificationAdapter Tests ────────────────────────────────


class TestSecurityExtract:
    """Tests for SecurityClassificationAdapter.extract()."""

    def test_extract_with_fs_prefix(self):
        metadata = {"FSCLAS": "S", "FSCLSY": "US"}
        sec = SecurityClassificationAdapter.extract(metadata, "FS")
        assert sec.clas == "S"
        assert sec.clsy == "US"

    def test_extract_with_des_prefix(self):
        metadata = {"DESCLAS": "C"}
        sec = SecurityClassificationAdapter.extract(metadata, "DES")
        assert sec.clas == "C"

    def test_missing_field_defaults(self):
        sec = SecurityClassificationAdapter.extract({}, "FS")
        assert sec.clas == "U"
        assert sec.clsy == ""
        assert sec.code == ""
        assert sec.ctlh == ""
        assert sec.rel == ""
        assert sec.dctp == ""
        assert sec.dcdt == ""
        assert sec.dcxm == ""
        assert sec.dg == ""
        assert sec.dgdt == ""
        assert sec.cltx == ""
        assert sec.catp == ""
        assert sec.caut == ""
        assert sec.crsn == ""
        assert sec.srdt == ""
        assert sec.ctln == ""

    def test_all_five_prefixes(self):
        for prefix in ("FS", "IS", "TS", "SS", "DES"):
            key = f"{prefix}CLAS"
            metadata = {key: "S"}
            sec = SecurityClassificationAdapter.extract(metadata, prefix)
            assert sec.clas == "S"


class TestSecurityToDict:
    """Tests for SecurityClassificationAdapter.to_dict()."""

    def test_to_dict_produces_prefixed_keys(self):
        sec = SecurityClassification(clas="S", clsy="US")
        result = SecurityClassificationAdapter.to_dict(sec, "FS")
        assert result["FSCLAS"] == "S"
        assert result["FSCLSY"] == "US"

    def test_roundtrip(self):
        sec = SecurityClassification(
            clas="S", clsy="US", code="ABC", ctlh="XY", rel="NATO",
            dctp="DD", dcdt="20231215", dcxm="X1", dg="C", dgdt="20231216",
            cltx="Some text", catp="O", caut="Auth", crsn="A", srdt="20231217", ctln="CTL",
        )
        for prefix in ("FS", "IS", "TS", "SS", "DES"):
            d = SecurityClassificationAdapter.to_dict(sec, prefix)
            restored = SecurityClassificationAdapter.extract(d, prefix)
            assert restored == sec


# ── TGTIDAdapter Tests ─────────────────────────────────────────────────


class TestTGTIDParse:
    """Tests for TGTIDAdapter.parse()."""

    def test_parse_normal(self):
        result = TGTIDAdapter.parse("1234567890ABCDEUS")
        assert result.be_number == "1234567890"
        assert result.osuffix == "ABCDE"
        assert result.country == "US"

    def test_parse_all_blank(self):
        result = TGTIDAdapter.parse("                 ")
        assert result.be_number == "          "
        assert result.osuffix == "     "
        assert result.country == "  "


class TestTGTIDFormat:
    """Tests for TGTIDAdapter.format()."""

    def test_format_roundtrip(self):
        original = "1234567890ABCDEUS"
        result = TGTIDAdapter.format(TGTIDAdapter.parse(original))
        assert result == original

    def test_error_wrong_length(self):
        with pytest.raises(ValueError):
            TGTIDAdapter.parse("short")


# ── LocationAdapter Tests ──────────────────────────────────────────────


class TestLocationParse:
    """Tests for LocationAdapter.parse()."""

    def test_parse_zero(self):
        assert LocationAdapter.parse("0000000000") == (0, 0)

    def test_parse_max(self):
        assert LocationAdapter.parse("9999999999") == (99999, 99999)

    def test_parse_normal(self):
        assert LocationAdapter.parse("0012300456") == (123, 456)


class TestLocationFormat:
    """Tests for LocationAdapter.format()."""

    def test_format_roundtrip(self):
        original = "0012300456"
        row, col = LocationAdapter.parse(original)
        assert LocationAdapter.format(row, col) == original

    def test_error_wrong_length(self):
        with pytest.raises(ValueError, match="10"):
            LocationAdapter.parse("12345")

    def test_error_non_numeric(self):
        with pytest.raises(ValueError):
            LocationAdapter.parse("00123ABCDE")
