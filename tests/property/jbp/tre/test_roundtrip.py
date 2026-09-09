"""Property-based round-trip tests for TRE structures via the dict API.

For every parseable TRE KSY definition, Hypothesis generates a nested value
dict (scalars, lists for repeats, dicts for nested types) and drives the
production ``StructureDefinition.encode`` / ``decode`` path. The test asserts a
**faithful round trip**: ``encode(decode(bytes)) == bytes``. The generated dict
may hold short (sub-field-width) values, so the first ``encode`` justifies/pads
them to the canonical on-disk form; from there ``decode`` is faithful (it returns
the field bytes verbatim, no trailing-space trim), so re-encoding the decoded
dict reproduces the bytes exactly with no canonicalization cycle.

Skip classification is **three-way and sourced from the loaded definition**
(``StructureDefinition.describe()``), never from a hardcoded YAML type set:

1. Primitive or resolved TypeRef -> generate.
2. Construct the parser cannot represent (unconditional unresolved /
   parameterized TypeRef) -> skip with an enumerated, logged reason.

Known core asymmetries surfaced by the cycle are marked ``xfail`` with a
``KNOWN_ASYMMETRY`` reason and burned down in Phase C'.
"""

from __future__ import annotations

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st

from tests.property.conftest import pbt_settings
from tests.property.jbp.tre.conftest import ALL_TRE_IDS, ALL_TRE_PATHS
from tests.property.jbp.tre.strategies import tre_instance

# Some TREs declare large fixed-count arrays (e.g. IOMAPA's 4096-entry lookup
# table) or wide variable-length fields. Generating those produces large inputs
# that trip Hypothesis's generation-size health checks — a property of the data
# format, not a test defect. Suppress those checks here (inheriting max_examples
# / phases from the active profile via ``pbt_settings``).
_tre_settings = settings(
    pbt_settings,
    suppress_health_check=[
        HealthCheck.data_too_large,
        HealthCheck.large_base_example,
        HealthCheck.too_slow,
    ],
)

# ---------------------------------------------------------------------------
# Skip / xfail classification
# ---------------------------------------------------------------------------

# TREs with a *core encode/decode asymmetry* surfaced by this suite. These are
# real production bugs, marked xfail(strict) and burned down in Phase C'. The
# value is the KNOWN_ASYMMETRY reason / BUG reference.
KNOWN_ASYMMETRY: dict[str, str] = {}


def _uncovered_nested_branches(descriptor: dict) -> list[str]:
    """Return descriptions of nested branches this TRE round-trips but cannot
    exercise (the generator leaves them empty), for the coverage ledger.

    A nested branch is genuinely uncovered only when its TypeRef names a type
    **not declared anywhere** in the structure's flat type map — the codec
    resolves all TypeRefs against that single flat map (regardless of declaration
    or reference scope) and seeds nested scopes with the enclosing values, so a
    sibling-scope nested TypeRef or a ``_root``/``_parent`` size expr is fully
    serialised and round-tripped. A reference that misses the flat map cannot be
    serialised and is left empty by the generator.

    Keyed on flat-map membership, **not** the per-scope ``type_resolved`` flag:
    that flag reads ``False`` for a sibling-scope nested TypeRef the codec
    nonetheless resolves.
    """
    uncovered: list[str] = []
    declared = set(descriptor["types"].keys())

    def scan(fields: list[dict], scope: str) -> None:
        for field in fields:
            if field["kind"] == "typeref" and field["type_name"] not in declared:
                uncovered.append(
                    f"{scope}{field['id']} -> {field['type_name']} "
                    f"(undeclared nested type)"
                )

    scan(descriptor["fields"], "")
    for type_name, type_fields in descriptor["types"].items():
        scan(type_fields, f"{type_name}.")
    return uncovered


def _unsupported_construct_reason(descriptor: dict) -> str | None:
    """Return an enumerated skip reason if this TRE has a construct the codec
    cannot represent for round-tripping, else None.

    Sourced entirely from ``describe()`` (the parser's authoritative view):

    - An **unconditional** TypeRef field naming a type **not declared anywhere**
      in the flat type map cannot be emitted at all (it cannot be left empty
      without an out-of-order error), so the whole TRE is unencodable. (A
      *conditional* undeclared TypeRef is fine: it stays inactive at the
      generated values and is simply never reached, so such TREs are tested with
      that branch left uncovered.)

    Keyed on flat-map membership, **not** the per-scope ``type_resolved`` flag:
    the codec resolves every TypeRef against the flat map regardless of
    declaration/reference scope, so only a reference that misses the flat map is
    genuinely unencodable.
    """
    declared = set(descriptor["types"].keys())
    for field in descriptor["fields"]:
        if (
            field["kind"] == "typeref"
            and field["type_name"] not in declared
            and not field["conditional"]
        ):
            return (
                f"unsupported construct: field {field['id']!r} references type "
                f"{field['type_name']!r}, which is not declared in the "
                f"structure's type map"
            )
    return None


# ---------------------------------------------------------------------------
# The property test
# ---------------------------------------------------------------------------


@pytest.mark.property
@pytest.mark.parametrize("tre_path", ALL_TRE_PATHS, ids=ALL_TRE_IDS)
@_tre_settings
@given(data=st.data())
def test_tre_round_trip(
    registry: StructureRegistry, tre_path: Path, data
) -> None:
    """Every generated TRE instance reaches an encode/decode fixed point."""
    stem = tre_path.stem
    defn = registry.get(stem)
    # Hard assertion: every tre_*.ksy must resolve in the registry.
    assert defn is not None, f"{stem!r} missing from registry"

    descriptor = defn.describe()

    reason = _unsupported_construct_reason(descriptor)
    if reason is not None:
        pytest.skip(f"{stem}: {reason}")

    values = data.draw(tre_instance(tre_path, descriptor))

    if stem in KNOWN_ASYMMETRY:
        # Strict xfail: the round trip must still be broken. When Phase C' fixes
        # the underlying asymmetry, this assertion starts failing — that is the
        # signal to remove the KNOWN_ASYMMETRY entry (the suite goes red until
        # the marker is dropped, exactly as an xfail(strict=True) would).
        with pytest.raises(Exception):
            _assert_faithful_round_trip(defn, values)
        pytest.xfail(KNOWN_ASYMMETRY[stem])

    _assert_faithful_round_trip(defn, values)


def _assert_faithful_round_trip(defn, values) -> None:
    """Encode the generated dict once, then assert ``decode``/``encode`` is
    faithful: ``encode(decode(bytes)) == bytes`` with no canonicalization cycle.

    The generated ``values`` may hold short (sub-field-width) entries, so the
    first ``encode`` justifies and pads them to the canonical on-disk form. From
    that byte form decode is faithful (no trailing-space trim), so re-encoding
    the decoded dict reproduces the bytes exactly.
    """
    raw = defn.encode(values)               # dict -> canonical on-disk bytes
    reencoded = defn.encode(defn.decode(raw))

    assert reencoded == raw, "encode(decode(bytes)) != bytes (not faithful)"
    assert defn.decode(reencoded) == defn.decode(raw), "dict not idempotent"


# ---------------------------------------------------------------------------
# Count/list atomicity — negative test
# ---------------------------------------------------------------------------


@pytest.mark.property
def test_count_list_mismatch_raises(registry: StructureRegistry) -> None:
    """``encode`` trusts the count field; a count/list mismatch must raise.

    ENGRDA's ``RECNT`` controls the length of ``RECORDS``. A dict whose
    ``RECNT`` disagrees with ``len(RECORDS)`` is invalid; the writer derives the
    repeat count from ``RECNT`` and rejects the mismatch rather than silently
    truncating or padding.
    """
    defn = registry.get("tre_engrda")
    assert defn is not None

    record = {
        "ENGLN": "03", "ENGLBL": "ABC", "ENGMTXC": "0001", "ENGMTXR": "0001",
        "ENGTYP": "A", "ENGDTS": "1", "ENGDATU": "NA", "ENGDATC": "00000002",
        "ENGDATA": "5859",  # bytes-typed: hex for b"XY"
    }
    # RECNT says 2 records, but only one is supplied.
    mismatched = {"RESRC": "SENSOR", "RECNT": "002", "RECORDS": [record]}

    with pytest.raises(ValueError):
        defn.encode(mismatched)


# ---------------------------------------------------------------------------
# Skip-list emission — definitive per-TRE coverage report
# ---------------------------------------------------------------------------


def test_emit_skip_and_xfail_list(registry: StructureRegistry) -> None:
    """Emit the definitive list of TREs that are skipped or xfail'd, with reasons.

    This is the suite's coverage ledger: every TRE not exercised by the cycle is
    here with an enumerated reason. "Type not in a hardcoded set" is never a
    reason — classification is sourced from ``describe()``.
    """
    skipped: dict[str, str] = {}
    xfailed: dict[str, str] = {}
    partial: dict[str, list[str]] = {}
    for path in ALL_TRE_PATHS:
        stem = path.stem
        defn = registry.get(stem)
        assert defn is not None, f"{stem!r} missing from registry"
        descriptor = defn.describe()
        reason = _unsupported_construct_reason(descriptor)
        if reason is not None:
            skipped[stem] = reason
        elif stem in KNOWN_ASYMMETRY:
            xfailed[stem] = KNOWN_ASYMMETRY[stem]
        branches = _uncovered_nested_branches(descriptor)
        # A skipped TRE's branches are subsumed by the skip reason; only report
        # partial coverage for TREs that otherwise round-trip.
        if branches and reason is None:
            partial[stem] = branches

    print("\n=== TRE round-trip coverage ledger ===")
    print(f"total TREs: {len(ALL_TRE_PATHS)}; "
          f"skipped: {len(skipped)}; xfail: {len(xfailed)}; "
          f"partial (nested branch uncovered): {len(partial)}; "
          f"fully exercised: "
          f"{len(ALL_TRE_PATHS) - len(skipped) - len(xfailed) - len(partial)}")
    for stem, reason in sorted(skipped.items()):
        print(f"  SKIP    {stem}: {reason}")
    for stem, reason in sorted(xfailed.items()):
        print(f"  XFAIL   {stem}: {reason}")
    for stem, branches in sorted(partial.items()):
        print(f"  PARTIAL {stem}: uncovered nested branches: {'; '.join(branches)}")

    # The only legal skip reason is an enumerated unsupported construct.
    for stem, reason in skipped.items():
        assert reason.startswith("unsupported construct:"), (
            f"{stem}: illegal skip reason {reason!r}"
        )
