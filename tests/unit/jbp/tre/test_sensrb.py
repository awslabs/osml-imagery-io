"""Spec-fidelity anchor for the SENSRB (Appendix Z) TRE.

``SENSRB`` (Appendix Z) was one of the two Phase 4 items
(BUG_TRE_KSY_SPEC_CONFORMANCE) that could not originally be corrected data-only
against the codec; it is now fixed. SENSRB's variable-width 12d/13e fields size
from a code->width lookup via the ``consts:`` / ``[]`` subscript operator.

Fields 12d ``TIME_STAMP_VALUE`` and 13e ``PIXEL_REFERENCE_VALUE`` take their byte
size from the *type code* named in the group's ``TYPE_MM`` field (12a / 13a):
per Table Z.3-1 note h the value inherits the size/range/character set of the
indexed parameter identified by that code (e.g. "06a" -> 11, "06b" -> 12,
"07a" -> 1). That is an arbitrary code->width mapping, not arithmetic over a
numeric field (contrast RSMGGA's RCOORD width = TNUMRD.to_i, a value read
directly as an integer). It is now expressed with a ``consts:`` code->width map
and the ``[]`` subscript operator: TIME_STAMP_VALUE / PIXEL_REFERENCE_VALUE size
from ``sensrb_value_widths[<type code>]``.

Two families of tests guard this:

* An ``encode``-driven check that the width tracks the type code across two
  codes of differing width (06a -> 11, 06b -> 12).
* A hand-built decode oracle (Tier-3): SENSRB byte strings assembled **by hand
  from public STDI-0002 spec widths** rather than from ``encode``. A
  wrong-but-symmetric width in the ``.ksy`` ``consts:`` table would encode and
  decode to the same wrong width and pass a round-trip check; feeding externally
  built bytes catches that, because the trailing fixed fields fail to align if
  the width is wrong.

Spec: STDI-0002 Vol 1, Appendix Z, Table Z.3-1 (pp. Z-31 to Z-32, note h).
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

# Public spec ground truth: a representative spread of note-h codes and the
# byte size of the parameter each indexes, read directly from Table Z.3-1.
# Deliberately duplicated here (not imported from the .ksy) so the fixture is an
# independent oracle.
NOTE_H_WIDTHS = {
    "02a": 20,   # DETECTION
    "03d": 12,   # RADIAL_DISTORT_1
    "04b": 3,    # MODE
    "05a": 12,   # REFERENCE_TIME
    "06a": 11,   # LATITUDE_OR_X
    "06b": 12,   # LONGITUDE_OR_Y
    "06c": 11,   # ALTITUDE_OR_Z
    "07a": 1,    # SENSOR_ANGLE_MODEL
    "08a": 10,   # ICX_NORTH_OR_X
    "10a": 9,    # VELOCITY_NORTH_OR_X
}


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


# ---------------------------------------------------------------------------
# encode-driven: width tracks the referenced parameter's type code
# ---------------------------------------------------------------------------


def _sensrb_minimal_with_time_stamp(type_code: str, value_width: int) -> dict:
    """A minimal SENSRB value dict carrying one time-stamped data set.

    All module flags are 'N' and the point/pixel/uncertainty/additional counts
    are zero; only Module 12 (Time Stamped Data Sets) is populated with one set
    holding one stamp, so the width of TIME_STAMP_VALUE (12d) is exercised in
    isolation.
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
            "TIME_STAMP_TYPE": type_code,
            "TIME_STAMP_COUNT": "0001",
            "TIME_STAMPS": [{
                "TIME_STAMP_TIME": "0" * 12,
                "TIME_STAMP_VALUE": "0" * value_width,
            }],
        }],
        "PIXEL_REFERENCED_DATA_SETS": "00",
        "UNCERTAINTY_DATA": "000",
        "ADDITIONAL_PARAMETER_DATA": "000",
    }


def test_sensrb_time_stamp_value_width_tracks_referenced_parameter(registry):
    """12d width follows the code in TIME_STAMP_TYPE across differing widths.

    Code "06a" -> 11-byte value; code "06b" -> 12-byte value. The spec-faithful
    definition sizes TIME_STAMP_VALUE from the type code via
    sensrb_value_widths[TIME_STAMP_TYPE], so both widths round-trip.
    """
    defn = registry.get("tre_sensrb")
    assert defn is not None

    def record(type_code: str, value_width: int) -> dict:
        return _sensrb_minimal_with_time_stamp(type_code, value_width)

    # 06a -> 11-byte value.
    raw_a = defn.encode(record("06a", 11))
    dec_a = defn.decode(raw_a)
    val_a = dec_a["TIME_STAMPED_SETS"][0]["TIME_STAMPS"][0]["TIME_STAMP_VALUE"]
    assert len(val_a) == 11, "TIME_STAMP_VALUE should be 11 bytes when type is 06a"

    # 06b -> 12-byte value.
    raw_b = defn.encode(record("06b", 12))
    dec_b = defn.decode(raw_b)
    val_b = dec_b["TIME_STAMPED_SETS"][0]["TIME_STAMPS"][0]["TIME_STAMP_VALUE"]
    assert len(val_b) == 12, "TIME_STAMP_VALUE should be 12 bytes when type is 06b"


# ---------------------------------------------------------------------------
# hand-built decode oracle (Tier-3): externally-built bytes at spec widths
# ---------------------------------------------------------------------------

# Fixed SENSRB prefix (all module flags 'N') up to and including the Module 11
# POINT_SET_DATA count of "00", built strictly from the .ksy field widths that
# are NOT under test. Ends right before TIME_STAMPED_DATA_SETS (Module 12).
_SENSRB_PREFIX = (
    "N"          # GENERAL_DATA
    "N"          # SENSOR_ARRAY_DATA
    "N"          # SENSOR_CALIBRATION_DATA
    "N"          # IMAGE_FORMATION_DATA
    + "0" * 12   # REFERENCE_TIME
    + "0" * 8    # REFERENCE_ROW
    + "0" * 8    # REFERENCE_COLUMN
    + "0" * 11   # LATITUDE_OR_X
    + "0" * 12   # LONGITUDE_OR_Y
    + "0" * 11   # ALTITUDE_OR_Z
    + "0" * 8    # SENSOR_X_OFFSET
    + "0" * 8    # SENSOR_Y_OFFSET
    + "0" * 8    # SENSOR_Z_OFFSET
    + "N"        # ATTITUDE_EULER_ANGLES
    + "N"        # ATTITUDE_UNIT_VECTORS
    + "N"        # ATTITUDE_QUATERNION
    + "N"        # SENSOR_VELOCITY_DATA
    + "00"       # POINT_SET_DATA (Module 11 count)
)

# Fixed SENSRB suffix following the last variable-width module (all counts zero).
_SENSRB_SUFFIX = (
    "00"         # PIXEL_REFERENCED_DATA_SETS
    "000"        # UNCERTAINTY_DATA
    "000"        # ADDITIONAL_PARAMETER_DATA
)


def _time_stamped_bytes(code: str, spec_width: int) -> bytes:
    """Hand-built SENSRB bytes carrying one Module-12 set with one stamp.

    ``TIME_STAMP_VALUE`` is sized to ``spec_width`` from the note-h table -- the
    ground-truth width, NOT what the definition would emit.
    """
    body = (
        _SENSRB_PREFIX
        + "01"                 # TIME_STAMPED_DATA_SETS = 1 set
        + code                 # 12a TIME_STAMP_TYPE (3 bytes)
        + "0001"               # 12b TIME_STAMP_COUNT = 1 stamp
        + "0" * 12             # 12c TIME_STAMP_TIME
        + "7" * spec_width     # 12d TIME_STAMP_VALUE at spec width
        + _SENSRB_SUFFIX
    )
    return body.encode("ascii")


def _pixel_referenced_bytes(code: str, spec_width: int) -> bytes:
    """Hand-built SENSRB bytes carrying one Module-13 set with one reference.

    ``PIXEL_REFERENCE_VALUE`` is sized to ``spec_width`` from the note-h table.
    """
    body = (
        _SENSRB_PREFIX
        + "00"                 # TIME_STAMPED_DATA_SETS = 0
        + "01"                 # PIXEL_REFERENCED_DATA_SETS = 1 set
        + code                 # 13a PIXEL_REFERENCE_TYPE (3 bytes)
        + "0001"               # 13b PIXEL_REFERENCE_COUNT = 1 reference
        + "0" * 8              # 13c PIXEL_REFERENCE_ROW
        + "0" * 8              # 13d PIXEL_REFERENCE_COLUMN
        + "7" * spec_width     # 13e PIXEL_REFERENCE_VALUE at spec width
        + "000"                # UNCERTAINTY_DATA
        + "000"                # ADDITIONAL_PARAMETER_DATA
    )
    return body.encode("ascii")


@pytest.mark.parametrize("code,spec_width", sorted(NOTE_H_WIDTHS.items()))
def test_time_stamp_value_consumes_spec_width(registry, code, spec_width):
    """12d TIME_STAMP_VALUE decodes at the note-h spec width, and tail aligns."""
    defn = registry.get("tre_sensrb")
    assert defn is not None

    raw = _time_stamped_bytes(code, spec_width)
    decoded = defn.decode(raw)

    value = decoded["TIME_STAMPED_SETS"][0]["TIME_STAMPS"][0]["TIME_STAMP_VALUE"]
    assert len(value) == spec_width, (
        f"code {code}: TIME_STAMP_VALUE should be {spec_width} bytes per note h"
    )
    # If the width were wrong the cursor would desync and these fixed trailing
    # counts would decode to garbage. Their exact values prove alignment.
    assert decoded["PIXEL_REFERENCED_DATA_SETS"] == "00"
    assert decoded["UNCERTAINTY_DATA"] == "000"
    assert decoded["ADDITIONAL_PARAMETER_DATA"] == "000"
    # Decoding consumed exactly the hand-authored bytes (no leftover, no overrun).
    assert defn.encode(decoded) == raw


@pytest.mark.parametrize("code,spec_width", sorted(NOTE_H_WIDTHS.items()))
def test_pixel_reference_value_consumes_spec_width(registry, code, spec_width):
    """13e PIXEL_REFERENCE_VALUE decodes at the note-h spec width, and tail aligns."""
    defn = registry.get("tre_sensrb")
    assert defn is not None

    raw = _pixel_referenced_bytes(code, spec_width)
    decoded = defn.decode(raw)

    value = decoded["PIXEL_REFERENCED_SETS"][0]["PIXEL_REFERENCES"][0][
        "PIXEL_REFERENCE_VALUE"
    ]
    assert len(value) == spec_width, (
        f"code {code}: PIXEL_REFERENCE_VALUE should be {spec_width} bytes per note h"
    )
    assert decoded["UNCERTAINTY_DATA"] == "000"
    assert decoded["ADDITIONAL_PARAMETER_DATA"] == "000"
    assert defn.encode(decoded) == raw
