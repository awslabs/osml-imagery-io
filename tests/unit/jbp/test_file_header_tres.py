"""Unit tests for NITF/NSIF file-header TREs through the public Python API.

File-header TREs live in the UDHD and XHD fields of the file header, so they
surface on the *dataset's* metadata provider rather than on any asset's. These
tests pin that behavior at the only level users see it, which is where
awslabs/osml-imagery-io#11 was reported: ``IO.open(...).metadata.entries()``
returned the raw ``XHD`` blob and no ``CSDIDA`` key.

Covered here:

* a write → read round trip through ``IO.open(..., "w")`` for both NITF 2.1 and
  NSIF 1.0, asserting field values survive and the raw container field is still
  present alongside the decoded CETAG dict;
* the checked-in ``data/unit/nitf21-8x8-1band-8bit-file-tres.ntf`` fixture,
  matching the reproduction in the bug report.

Per-TRE definition tests live in ``tests/unit/jbp/tre/``; this module exercises
the reader/writer path instead.
"""

from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

UNIT_DATA = Path("data/unit")
FILE_TRE_FIXTURE = UNIT_DATA / "nitf21-8x8-1band-8bit-file-tres.ntf"

# Mirrors the CSDIDA in the reporter's file (STDI-0002 Vol 1, App AS).
CSDIDA_FIELDS = {
    "DAY": "26",
    "MONTH": "JUL",
    "YEAR": "2021",
    "PLATFORM_CODE": "WV",
    "VEHICLE_ID": "03",
    "PASS": "03",
    "OPERATION": "000",
    "SENSOR_ID": "AA",
    "PRODUCT_ID": "P1",
    "RESERVED_1": "0000",
    "TIME": "20210726022422",
    "PROCESS_TIME": "20210726035421",
    "RESERVED_2": "00",
    "RESERVED_3": "01",
    "RESERVED_4": "N",
    "RESERVED_5": "N",
    "SOFTWARE_VERSION_NUMBER": "4.54.0",
}

SYSIDA_FIELDS = {
    "PLATFORM_ID_LEN": "003",
    "PLATFORM_ID": "WV3",
    "PAYLOAD_ID_LEN": "000",
    "SENSOR_ID_LEN": "003",
    "SENSOR_ID": "PAN",
}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _image_provider() -> BufferedImageAssetProvider:
    """An 8x8 single-band image — just enough to make a valid file."""
    img_meta = BufferedMetadataProvider()
    img_meta["IC"] = "NC"
    img_meta["IMODE"] = "B"
    img_meta["ICAT"] = "VIS"

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=8,
        num_rows=8,
        num_bands=1,
        block_width=8,
        block_height=8,
        pixel_type=PixelType.UInt8,
        metadata=img_meta,
    )
    array = np.array(
        [[(x + y) % 256 for x in range(8)] for y in range(8)], dtype=np.uint8
    ).reshape(1, 8, 8)
    provider.set_full_image(array)
    return provider


def _write_with_file_tres(path: Path, fmt: str, tres: dict) -> None:
    """Write a minimal image file whose *file header* carries `tres`."""
    file_meta = BufferedMetadataProvider()
    file_meta["FTITLE"] = "File-header TRE test"
    file_meta["OSTAID"] = "OSML_IO"
    file_meta["FDT"] = "20260101120000"
    for cetag, fields in tres.items():
        file_meta[cetag] = fields

    writer = IO.open([str(path)], "w", fmt)
    writer.metadata = file_meta
    writer.add_asset("image:0", _image_provider(), "8x8", "test", ["data"])
    writer.close()


def _assert_fields_match(actual: dict, expected: dict, cetag: str) -> None:
    """Compare TRE fields, tolerating the writer's BCS-A space padding."""
    for name, value in expected.items():
        assert name in actual, f"{cetag} is missing field {name}: got {sorted(actual)}"
        assert actual[name].strip() == value, (
            f"{cetag}.{name}: expected {value!r}, got {actual[name]!r}"
        )


# ---------------------------------------------------------------------------
# Round trip through the public API
# ---------------------------------------------------------------------------


class TestFileHeaderTreRoundTrip:
    """File-header TREs written by `IO.open(..., "w")` read back intact."""

    @pytest.mark.parametrize(
        "fmt,suffix",
        [("nitf", ".ntf"), ("nsif", ".nsif")],
    )
    def test_file_header_tres_round_trip(self, tmp_path, fmt, suffix):
        """CSDIDA and SYSIDA survive a write/read cycle on the dataset provider.

        Both `.ksy` file-header definitions name UDHD/XHD identically, so one
        code path serves NITF 2.1 and NSIF 1.0 — asserted here rather than
        assumed.
        """
        path = tmp_path / f"file-tres{suffix}"
        _write_with_file_tres(
            path, fmt, {"CSDIDA": CSDIDA_FIELDS, "SYSIDA": SYSIDA_FIELDS}
        )

        with IO.open([str(path)], "r") as dataset:
            entries = dataset.metadata.entries()

        assert "CSDIDA" in entries, (
            f"file-header TRE not surfaced as a CETAG key; got {sorted(entries)}"
        )
        assert "SYSIDA" in entries
        assert isinstance(entries["CSDIDA"], dict)
        _assert_fields_match(entries["CSDIDA"], CSDIDA_FIELDS, "CSDIDA")
        _assert_fields_match(entries["SYSIDA"], SYSIDA_FIELDS, "SYSIDA")

    def test_raw_xhd_field_present_alongside_decoded_tres(self, tmp_path):
        """The raw container field remains readable, hex-encoded per Phase 2b.

        `XHD` is declared `type: bytes` so binary CEDATA survives, which means
        the raw value is a lowercase hex string rather than text. `XHDL` counts
        the 3-byte `XHDLOFL` subfield along with the payload.
        """
        path = tmp_path / "file-tres.ntf"
        _write_with_file_tres(path, "nitf", {"CSDIDA": CSDIDA_FIELDS})

        with IO.open([str(path)], "r") as dataset:
            entries = dataset.metadata.entries()

        assert "XHD" in entries, "raw XHD container field should still be exposed"
        raw = entries["XHD"]
        assert isinstance(raw, str)
        assert raw == raw.lower()

        payload = bytes.fromhex(raw)
        assert payload.startswith(b"CSDIDA"), "XHD should hold the TRE envelope"
        assert int(entries["XHDL"]) == len(payload) + 3

        # The writer places every file-header TRE in XHD, so UDHD stays empty.
        assert entries["UDHDL"] == "00000"

        # And the decoded form is the intended access path.
        assert entries["CSDIDA"]["MONTH"] == "JUL"

    def test_file_tres_do_not_leak_onto_the_image_asset(self, tmp_path):
        """File-header TREs stay on the dataset, not on the image segment."""
        path = tmp_path / "file-tres.ntf"
        _write_with_file_tres(path, "nitf", {"CSDIDA": CSDIDA_FIELDS})

        with IO.open([str(path)], "r") as dataset:
            image_entries = dataset.get_asset("image:0").metadata.entries()

        assert "CSDIDA" not in image_entries


# ---------------------------------------------------------------------------
# Checked-in fixture
# ---------------------------------------------------------------------------


class TestCheckedInFixture:
    """The `data/unit/` fixture reproduces the bug report's file shape."""

    def test_fixture_exposes_file_header_tres(self):
        """`IO.open(fixture).metadata.entries()` contains both CETAG keys.

        This is the assertion from awslabs/osml-imagery-io#11 verbatim:
        `"CSDIDA" in e` was `False` before the fix.
        """
        if not FILE_TRE_FIXTURE.exists():
            pytest.skip(f"Unit test data not available: {FILE_TRE_FIXTURE}")

        with IO.open([str(FILE_TRE_FIXTURE)], "r") as dataset:
            entries = dataset.metadata.entries()

        assert "CSDIDA" in entries
        assert "SYSIDA" in entries
        _assert_fields_match(entries["CSDIDA"], CSDIDA_FIELDS, "CSDIDA")
        _assert_fields_match(entries["SYSIDA"], SYSIDA_FIELDS, "SYSIDA")

        # Raw container field is present and self-consistent with XHDL.
        payload = bytes.fromhex(entries["XHD"])
        assert int(entries["XHDL"]) == len(payload) + 3
        assert b"CSDIDA" in payload and b"SYSIDA" in payload

    def test_fixture_prefix_filter_selects_one_tre(self):
        """`entries(prefix)` filters file-header TREs by CETAG, as for segments."""
        if not FILE_TRE_FIXTURE.exists():
            pytest.skip(f"Unit test data not available: {FILE_TRE_FIXTURE}")

        with IO.open([str(FILE_TRE_FIXTURE)], "r") as dataset:
            filtered = dataset.metadata.entries("CSDIDA")

        assert list(filtered) == ["CSDIDA"]
        assert filtered["CSDIDA"]["PLATFORM_CODE"] == "WV"
