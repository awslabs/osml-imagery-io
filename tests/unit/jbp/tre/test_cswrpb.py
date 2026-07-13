"""Spec-fidelity anchor for the CSWRPB TRE (navigator-rewrite Category 1).

The ``_root``/``_parent`` navigators were dropped from the expression grammar
(DESIGN_REMOVE_ROOT_PARENT_NAVIGATORS Phase 2); the ``.ksy`` definitions that
used them were rewritten to bare field names. Under the flat-scope +
``inherited``-map model, a bare name in a nested type resolves against the merged
scope (consts < inherited enclosing scalars < local fields), so the rewrite must
be behavior-preserving.

``tre_cswrpb`` is load-bearing: the nested ``warp_set_t`` reads ``SENSOR_TYPE``
from the enclosing structure to gate ``FL_WARP`` (formerly
``_root.SENSOR_TYPE``). These tests exercise both the framing (F) branch — where
``WRP_INTERP`` and the nested ``FL_WARP`` are present — and the scanner (S)
branch — where both are absent — so the inherited resolution is verified, not
just parsed.

Spec: STDI-0002 Vol 2, Appendix M -- CSWRPB.
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

_V21 = "+1.00000000000000E+00"  # 21-char BCS scientific-notation value


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


def _warp_set(sensor_type: str) -> dict:
    """One warp_set_t with zero-order polynomials (1 coeff each)."""
    warp = {
        "OFFSET_LINE": "0000001", "OFFSET_SAMP": "0000001",
        "SCALE_LINE": "0000001", "SCALE_SAMP": "0000001",
        "OFFSET_LINE_UNWRP": "0000001", "OFFSET_SAMP_UNWRP": "0000001",
        "SCALE_LINE_UNWRP": "0000001", "SCALE_SAMP_UNWRP": "0000001",
        "LINE_POLY_ORDER_M1": "0", "LINE_POLY_ORDER_M2": "0",
        "SAMP_POLY_ORDER_N1": "0", "SAMP_POLY_ORDER_N2": "0",
        "LINE_POLY_COEFFS": [_V21], "SAMP_POLY_COEFFS": [_V21],
    }
    if sensor_type == "F":
        warp["FL_WARP"] = "00.00000000"
    return warp


def test_cswrpb_framing_sensor_emits_fl_warp(registry):
    """SENSOR_TYPE='F' -> WRP_INTERP and nested FL_WARP present via inherited scope."""
    defn = registry.get("tre_cswrpb")
    assert defn is not None

    record = {
        "NUM_SETS_WARP_DATA": "1", "SENSOR_TYPE": "F", "WRP_INTERP": "1",
        "WARP_SETS": [_warp_set("F")], "RESERVED_LEN": "00000",
    }
    raw = defn.encode(record)
    decoded = defn.decode(raw)
    assert "FL_WARP" in decoded["WARP_SETS"][0]
    assert defn.encode(decoded) == raw


def test_cswrpb_scanner_sensor_omits_fl_warp(registry):
    """SENSOR_TYPE='S' -> nested FL_WARP absent; inherited gate resolves to false."""
    defn = registry.get("tre_cswrpb")
    assert defn is not None

    record = {
        "NUM_SETS_WARP_DATA": "1", "SENSOR_TYPE": "S",
        "WARP_SETS": [_warp_set("S")], "RESERVED_LEN": "00000",
    }
    raw = defn.encode(record)
    decoded = defn.decode(raw)
    assert "FL_WARP" not in decoded["WARP_SETS"][0]
    assert "WRP_INTERP" not in decoded
    assert defn.encode(decoded) == raw
