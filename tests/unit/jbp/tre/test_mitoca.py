"""Spec-fidelity anchor for the MITOCA TRE (navigator-rewrite Category 1).

The ``_root``/``_parent`` navigators were dropped from the expression grammar
(DESIGN_REMOVE_ROOT_PARENT_NAVIGATORS Phase 2); the ``.ksy`` definitions that
used them were rewritten to bare field names. Under the flat-scope +
``inherited``-map model, a bare name in a nested type resolves against the merged
scope (consts < inherited enclosing scalars < local fields), so the rewrite must
be behavior-preserving.

``tre_mitoca`` is load-bearing: the nested ``component_entry`` reads
``COMPONENT_ID_LEN``, ``COMPONENT_INDEX_TYPE``, and ``VOLUME_COMPOSITE_INDEX``
from the enclosing structure (formerly ``_root.*``). These tests exercise both
the non-zero (fields present) and zero (fields absent) branches so the inherited
resolution is verified, not just parsed.

Spec: STDI-0002 Vol 1, Appendix O -- MITOCA.
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


def _mitoca_record(component_index_type: str, volume_composite_index: str,
                   component: dict) -> dict:
    """A minimal single-component MITOCA record.

    LOOK_COMPOSITE_INDEX='---' suppresses the four LOOK_CORNER fields, keeping
    the scene section compact. The Volume section is always present.
    """
    return {
        "SCENE_TYPE": "000", "SCENE_ID_LEN": "003", "SCENE_ID": "ABC",
        "LOOK_COMPOSITE_INDEX": "---", "LOOK_COMPOSITE_ID_LEN": "000",
        "NUM_VOLUMES": "000001",
        "LOOK_INSTANCE": "000001", "VOLUME_NUM": "000001",
        "SENSOR_ID": "SENSR1", "SENSOR_ID_TYPE": "TYP1", "MPLAN": "PLN",
        "VOLUME_COMPOSITE_INDEX": volume_composite_index,
        "VOLUME_COMPOSITE_ID_LEN": "003", "VOLUME_COMPOSITE_ID": "VID",
        "VOLUME_CORNER_1": _V21, "VOLUME_CORNER_2": _V21,
        "VOLUME_CORNER_3": _V21, "VOLUME_CORNER_4": _V21,
        "NUM_COMPONENTS": "001", "COMPONENTS_FLAG": "1",
        "NUM_ROWS": "00000001", "NUM_COLS": "00000001", "DSR": "0001.00",
        "COMPONENT_ID_LEN": "004",
        "COMPONENT_INDEX_TYPE": component_index_type,
        "COMPONENTS": [component],
    }


def _mitoca_component(with_ish: bool, with_offsets: bool) -> dict:
    comp = {
        "COMPONENT_ID": "COMP",  # size = COMPONENT_ID_LEN.to_i = 4
        "COMPONENT_CORNER_1": _V21, "COMPONENT_CORNER_2": _V21,
        "COMPONENT_CORNER_3": _V21, "COMPONENT_CORNER_4": _V21,
    }
    if with_ish:
        comp["ISH_INDEX"] = "001"
    if with_offsets:
        comp.update({
            "UPPER_LEFT_ROW": "00000000", "UPPER_LEFT_COL": "00000000",
            "UPPER_RIGHT_ROW": "00000000", "UPPER_RIGHT_COL": "00000000",
            "LOWER_RIGHT_ROW": "00000000", "LOWER_RIGHT_COL": "00000000",
            "LOWER_LEFT_ROW": "00000000", "LOWER_LEFT_COL": "00000000",
        })
    return comp


def test_mitoca_component_id_size_from_inherited_len(registry):
    """COMPONENT_ID width tracks the enclosing COMPONENT_ID_LEN via inherited scope."""
    defn = registry.get("tre_mitoca")
    assert defn is not None

    component = _mitoca_component(with_ish=False, with_offsets=False)
    record = _mitoca_record("0", "000", component)
    raw = defn.encode(record)
    decoded = defn.decode(raw)
    assert len(decoded["COMPONENTS"][0]["COMPONENT_ID"]) == 4
    assert defn.encode(decoded) == raw


def test_mitoca_nonzero_index_and_composite_emit_conditional_fields(registry):
    """Non-zero inherited COMPONENT_INDEX_TYPE and VOLUME_COMPOSITE_INDEX -> fields present."""
    defn = registry.get("tre_mitoca")
    assert defn is not None

    component = _mitoca_component(with_ish=True, with_offsets=True)
    record = _mitoca_record("1", "001", component)
    raw = defn.encode(record)
    decoded = defn.decode(raw)
    comp = decoded["COMPONENTS"][0]
    assert comp["ISH_INDEX"] == "001"
    assert "UPPER_LEFT_ROW" in comp
    assert "LOWER_LEFT_COL" in comp
    assert defn.encode(decoded) == raw


def test_mitoca_zero_index_and_composite_omit_conditional_fields(registry):
    """Zero inherited COMPONENT_INDEX_TYPE and VOLUME_COMPOSITE_INDEX -> fields absent.

    This is the load-bearing check: if the enclosing scalars were NOT seeded
    into the nested component_entry scope, the bare-name gates would fail to
    resolve and the fields' presence would not track the enclosing values.
    """
    defn = registry.get("tre_mitoca")
    assert defn is not None

    component = _mitoca_component(with_ish=False, with_offsets=False)
    record = _mitoca_record("0", "000", component)
    raw = defn.encode(record)
    decoded = defn.decode(raw)
    comp = decoded["COMPONENTS"][0]
    assert "ISH_INDEX" not in comp
    assert "UPPER_LEFT_ROW" not in comp
    assert "LOWER_LEFT_COL" not in comp
    assert defn.encode(decoded) == raw
