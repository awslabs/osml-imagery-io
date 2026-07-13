"""Spec-fidelity anchor for the FSYNWA (Appendix AF) TRE.

``FSYNWA`` (Appendix AF) was one of the two Phase 4 items
(BUG_TRE_KSY_SPEC_CONFORMANCE) that could not originally be corrected data-only
against the codec; it is now fixed. FSYNWA's encapsulated-TRE group is modeled
as a ``repeat: eos`` sequence now that the writer encodes open repeats.

FSYNWA wraps N encapsulated TREs as a repeating TRETAGn(6)/TRELn(5)/
TREDATAn(=TRELn) group. Per Table AF-10 "the number of TREs is not signaled and
is determined by parsing the TRE data" -- i.e. the group repeats to end of stream
(repeat: eos). The writer now encodes repeat: eos by trusting the supplied list
length, so FSYNWA is modeled as the spec-faithful TRETAGn/TRELn/TREDATAn
repeat: eos group rather than a single opaque size-eos blob.

Spec: STDI-0002 Vol 1 -- FSYNWA Appendix AF, Table AF-10 (pp. AF-38 to AF-39).
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


def test_fsynwa_models_distinct_encapsulated_tres(registry):
    """FSYNWA decodes N encapsulated TREs as distinct TRETAG/TREL/TREDATA groups.

    The definition models CEDATA as a ``repeat: eos`` group of (TRETAGn, TRELn,
    TREDATAn). TREDATA is a ``bytes`` field, so encode consumes and decode emits a
    lowercase hex string for its payload (the codec's symmetric bytes carrier).
    """
    defn = registry.get("tre_fsynwa")
    assert defn is not None

    # TREDATA is a bytes field: its value is a hex string whose byte length
    # equals TREL. "6162636465" == b"abcde" (5 bytes); "78797a" == b"xyz" (3).
    record = {
        "START_FRAME_NUMBER": "000000001",
        "END_FRAME_NUMBER": "000000010",
        "CEDATA": [
            {"TRETAG": "ACCHZB", "TREL": "00005", "TREDATA": "6162636465"},
            {"TRETAG": "MENSRB", "TREL": "00003", "TREDATA": "78797a"},
        ],
    }
    raw = defn.encode(record)
    decoded = defn.decode(raw)

    assert len(decoded["CEDATA"]) == 2
    assert decoded["CEDATA"][0]["TRETAG"] == "ACCHZB"
    assert decoded["CEDATA"][0]["TREDATA"] == "6162636465"
    assert decoded["CEDATA"][1]["TRETAG"] == "MENSRB"
    assert decoded["CEDATA"][1]["TREDATA"] == "78797a"
    # TREDATA width tracks TREL: 5 bytes -> 10 hex chars, 3 bytes -> 6 hex chars.
    assert len(decoded["CEDATA"][0]["TREDATA"]) == 10
    assert len(decoded["CEDATA"][1]["TREDATA"]) == 6
    # Faithful round trip: re-encoding the decoded dict reproduces the bytes.
    assert defn.encode(decoded) == raw
