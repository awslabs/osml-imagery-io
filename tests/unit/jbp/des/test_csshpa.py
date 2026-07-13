"""Spec-fidelity anchor for the CSSHPA DES navigator rewrite.

DESIGN_REMOVE_ROOT_PARENT_NAVIGATORS Phase 3 (Category 2, B').

The ``CC_SOURCE`` field in ``des_csshpa`` was gated on ``_root._io.size >= 80``,
a stream-size proxy that (a) never evaluated as written once the navigators were
removed and (b) is not the predicate the spec defines. STDI-0002 Vol 2 App D
(Table D.4-1/D.4-3) says ``CC_SOURCE`` is present *only if SHAPE_USE is
CLOUD_SHAPES*, so the gate is now ``SHAPE_USE.strip == "CLOUD_SHAPES"``.

``SHAPE_USE`` is a sibling field parsed earlier in the same ``seq``, so this is
self-contained -- no ``inherited`` map, no stream size. These tests feed a
DESSHF byte buffer through the public ``encode``/``decode`` API, asserting
``CC_SOURCE`` presence toggles with ``SHAPE_USE``.

Spec: STDI-0002 Vol 2 Appendix D -- CSSHPA-CSSHPB.
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")

# Per spec, DESSHL is 0062 without CC_SOURCE, 0080 with it.
_LEN_WITHOUT_CC = 62
_LEN_WITH_CC = 80


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


def _csshpa_record(shape_use: str, *, with_cc: bool) -> dict:
    record = {
        "SHAPE_USE": shape_use.ljust(25),
        "SHAPE_CLASS": "POLYGON".ljust(10),
        "SHAPE1_NAME": "SHP", "SHAPE1_START": "000000",
        "SHAPE2_NAME": "SHX", "SHAPE2_START": "000100",
        "SHAPE3_NAME": "DBF", "SHAPE3_START": "000200",
    }
    if with_cc:
        record["CC_SOURCE"] = "PAN".ljust(18)
    return record


def test_csshpa_cloud_shapes_emits_cc_source(registry):
    """SHAPE_USE='CLOUD_SHAPES' -> CC_SOURCE present; sibling-field gate resolves true."""
    defn = registry.get("des_csshpa")
    assert defn is not None

    raw = defn.encode(_csshpa_record("CLOUD_SHAPES", with_cc=True))
    assert len(raw) == _LEN_WITH_CC
    decoded = defn.decode(raw)
    assert "CC_SOURCE" in decoded
    assert decoded["CC_SOURCE"].strip() == "PAN"
    assert defn.encode(decoded) == raw


def test_csshpa_non_cloud_shapes_omits_cc_source(registry):
    """SHAPE_USE != 'CLOUD_SHAPES' -> CC_SOURCE absent; sibling-field gate resolves false."""
    defn = registry.get("des_csshpa")
    assert defn is not None

    raw = defn.encode(_csshpa_record("IMAGE_SHAPE", with_cc=False))
    assert len(raw) == _LEN_WITHOUT_CC
    decoded = defn.decode(raw)
    assert "CC_SOURCE" not in decoded
    assert defn.encode(decoded) == raw
