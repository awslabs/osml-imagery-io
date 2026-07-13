"""Hypothesis strategies for generating nested TRE field-value dicts.

This drives the production ``StructureDefinition.encode``/``decode`` path: the
strategy produces a **nested dict** (scalars for flat fields, lists for repeats,
dicts for nested TypeRef fields) which ``encode`` then serialises.

Generation model — ``encode`` is the sole arbiter of *presence*
----------------------------------------------------------------
The strategy does **not** re-implement KSY condition evaluation in Python (the
old ``conditions.py`` is gone). It generates a value for *every* field the
definition lists — including conditional ones — and relies on the writer's own
condition evaluation (exposed through ``encode``) to skip inactive fields: a
value handed to ``encode`` for a field whose ``if:`` is currently false is
simply not written (the codec's ``is_field_active`` gate).

The one invariant the engine cannot repair is **count/list atomicity**: the
writer *trusts* a count field (e.g. ``RECNT``) and derives the repeat length
from it, so a list whose length disagrees with its controller is an ``encode``
error. The strategy therefore generates each repeated field's length by
**evaluating its ``repeat-expr`` against the already-drawn controller value**,
never independently. Likewise an expression-sized field's width is resolved from
the controller it references.

Field *classification* (primitive / nested / unsupported) is sourced from
``StructureDefinition.describe()`` — the parser's authoritative view — not from
a hand-maintained YAML type whitelist (which previously drifted out of sync with
the parser; see Review Decision 6). Size/encoding metadata for value generation
comes from the KSY via :mod:`ksy_schema`.

Three-tier generation (most specific wins; claimed-key sets must not overlap):

1. **Default** — generate every field generically; ``encode`` arbitrates
   presence; count/list and size relationships are honoured by expression
   evaluation against context.
2. **Per-field override** (:data:`FIELD_STRATEGIES`) — bias a single field's
   *value distribution* (reachability / edge cases). Value bias only.
3. **Group override** (:data:`GROUP_STRATEGIES`) — jointly generate a correlated
   field set in one shot. Owns count/list atomicity for its claimed fields.
"""

from __future__ import annotations

import re
import string
from pathlib import Path
from typing import Callable, Optional

from hypothesis import strategies as st
from hypothesis.strategies import SearchStrategy

from tests.property.jbp.tre.ksy_schema import KsyField, KsySchema, load_ksy_schema

# ---------------------------------------------------------------------------
# Caps (keep generated structures within Hypothesis size limits)
# ---------------------------------------------------------------------------

_MAX_EXPR_SIZE = 64     # cap for expression-sized field widths
_MAX_REPEAT_COUNT = 4   # cap for controller-driven repeat counts

# BCS-A: printable ASCII 0x20–0x7E
_BCS_A_ALPHABET = "".join(chr(c) for c in range(0x20, 0x7F))
# BCS-N: digits, space, +, -, ., /
_BCS_N_ALPHABET = string.digits + " +-./"

_TO_I_REF_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_.]*)\.to_i\b")
_IDENT_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
_NON_FIELD_TOKENS = frozenset({"to_i", "to_s", "_parent", "_root", "_io", "_index", "_"})


# ---------------------------------------------------------------------------
# Field descriptor: parser-authoritative classification + KSY metadata
# ---------------------------------------------------------------------------


class FieldInfo:
    """A field's parser classification merged with its KSY size/encoding metadata.

    ``kind`` comes from ``StructureDefinition.describe()`` (one of ``string``,
    ``bytes``, ``uint``, ``sint``, ``float``, ``typeref``); ``ksy`` is the
    matching :class:`KsyField` (``None`` only if the parser names a field the KSY
    ``seq`` does not — which does not happen for TREs).
    """

    __slots__ = ("id", "kind", "width", "type_name", "type_resolved",
                 "conditional", "repeated", "ksy")

    def __init__(self, desc: dict, ksy: Optional[KsyField]):
        self.id = desc["id"]
        self.kind = desc["kind"]
        self.width = desc["width"]
        self.type_name = desc["type_name"]
        self.type_resolved = desc["type_resolved"]
        self.conditional = desc["conditional"]
        self.repeated = desc["repeated"]
        self.ksy = ksy


def merge_descriptor(
    descriptors: list[dict], ksy_fields: list[KsyField]
) -> list[FieldInfo]:
    """Zip parser descriptors with KSY metadata, matched by field id."""
    by_id = {f.id: f for f in ksy_fields}
    return [FieldInfo(d, by_id.get(d["id"])) for d in descriptors]


# ---------------------------------------------------------------------------
# Scalar value strategies
# ---------------------------------------------------------------------------


def _bcs_a(size: int) -> SearchStrategy:
    return st.text(alphabet=_BCS_A_ALPHABET, min_size=size, max_size=size)


def _bcs_n_text(size: int) -> SearchStrategy:
    """BCS-N text of an exact width (digits/sign/decimal/slash/space)."""
    return st.text(alphabet=_BCS_N_ALPHABET, min_size=size, max_size=size)


def _digits(size: int) -> SearchStrategy:
    """Digits-only string of an exact width (for fields read as counts/sizes)."""
    return st.text(alphabet=string.digits, min_size=size, max_size=size)


# The PyO3 encode binding converts Python ints through i64, so values are capped
# to the signed 64-bit range regardless of declared (e.g. u8 = 8-byte) width.
_I64_MAX = 2 ** 63 - 1
_I64_MIN = -(2 ** 63)


def _int_strategy(kind: str, width: int) -> SearchStrategy:
    if kind == "sint":
        lo, hi = -(2 ** (8 * width - 1)), 2 ** (8 * width - 1) - 1
    else:
        lo, hi = 0, 2 ** (8 * width) - 1
    lo = max(lo, _I64_MIN)
    hi = min(hi, _I64_MAX)
    return st.integers(min_value=lo, max_value=hi)


def scalar_strategy(
    info: FieldInfo, size: Optional[int], *, numeric_only: bool
) -> SearchStrategy:
    """Strategy for one scalar (non-repeat, non-typeref) field value.

    ``numeric_only`` forces digit-only string content for fields whose value is
    consumed as a size or repeat count, so the drawn value is usable as a length
    without overflowing the field.
    """
    if info.kind in ("uint", "sint"):
        return _int_strategy(info.kind, info.width or 1)
    if info.kind == "float":
        # f4 round-trips exactly only at f32 precision; f8 at f64.
        width = 32 if (info.width or 4) == 4 else 64
        return st.floats(allow_nan=False, allow_infinity=False, width=width)
    if info.kind == "bytes":
        n = size if isinstance(size, int) else 1
        # decode returns a lowercase hex string for bytes fields; encode hex-decodes.
        return st.binary(min_size=n, max_size=n).map(lambda b: b.hex())

    # string-typed
    ksy = info.ksy
    if ksy is not None and ksy.size_eos:
        enc = (ksy.encoding or "").upper()
        if enc == "UTF-8":
            return st.text(min_size=0, max_size=64)
        return st.text(alphabet=_BCS_A_ALPHABET, min_size=0, max_size=64)

    n = size if isinstance(size, int) else 1
    if n <= 0:
        return st.just("")
    enc = (ksy.encoding or "").upper() if ksy is not None else ""
    if numeric_only:
        return _digits(n)
    if enc == "BCS-N":
        return _bcs_n_text(n)
    return _bcs_a(n)


# ---------------------------------------------------------------------------
# Expression evaluation against drawn context (count/list + size atomicity)
# ---------------------------------------------------------------------------


class _Ctx:
    """A nested scope's drawn scalar values plus links to parent / root scopes.

    ``local`` holds scalars drawn in this scope; ``parent`` and ``root`` are the
    enclosing and top-level :class:`_Ctx` (``None`` at the top). Expression
    references resolve ``_parent.X`` against ``parent`` and ``_root.X`` against
    ``root``; a bare ``X`` resolves against ``local``.
    """

    def __init__(self, parent: Optional["_Ctx"], root: Optional["_Ctx"]):
        self.local: dict = {}
        self.parent = parent
        self.root = root if root is not None else self

    def as_int(self, name: str) -> int:
        scope = self
        token = name
        # An explicit ``_root.``/``_parent.`` prefix pins the lookup to that
        # scope. A bare identifier walks the scope chain (local -> parent ->
        # root), mirroring how the codec resolves references against the
        # inherited-scalar snapshot threaded into every nested scope
        # (BUG_TRE_NESTED_SCOPE_THREADING). Resolving bare refs local-only would
        # under-count a cross-scope repeat controller (e.g. RSMAPB's ``AEL``
        # count ``NPAR.to_i * NBASIS.to_i`` where ``NPAR`` is an enclosing
        # scalar), breaking the count/list atomicity this strategy owns.
        walk_chain = True
        if token.startswith("_root."):
            scope, token, walk_chain = self.root, token[len("_root."):], False
        elif token.startswith("_parent."):
            scope, token, walk_chain = (self.parent or self), token[len("_parent."):], False
        if token.endswith(".to_i"):
            token = token[: -len(".to_i")]
        token = token.split(".")[-1]
        val = scope.local.get(token)
        if val is None and walk_chain:
            probe = scope.parent
            while val is None and probe is not None:
                val = probe.local.get(token)
                probe = probe.parent
        try:
            return int(str(val).strip())
        except (ValueError, TypeError):
            return 0


def _safe_eval_count(expr: str, ctx: _Ctx) -> int:
    """Evaluate a ``repeat-expr`` to an integer count against ``ctx``.

    Supports the forms present in the TRE corpus: integer literals, single
    field references (``COUNT.to_i``, ``NUMBER_DT``, ``_parent.NPAR.to_i``), and
    arithmetic over them (``*``, ``+``, ``-``, ``/``, parentheses), e.g.
    ``(NPAR.to_i * (NPAR.to_i + 1)) / 2``. Unknown references resolve to 0. The
    result is clamped to ``[0, controller-bounded]`` — controllers are generated
    small, so products stay bounded.
    """
    expr = expr.strip()
    # Replace every ``ref.to_i`` and bare identifier reference with its int value.
    # Tokenize on the reference grammar, substituting numbers.
    def repl_to_i(m: re.Match) -> str:
        return str(ctx.as_int(m.group(0)))

    substituted = _TO_I_REF_RE.sub(repl_to_i, expr)

    # Remaining bare identifiers (count fields with integer type, no .to_i).
    def repl_ident(m: re.Match) -> str:
        tok = m.group(0)
        if tok in _NON_FIELD_TOKENS:
            return tok
        return str(ctx.as_int(tok))

    substituted = _IDENT_RE.sub(repl_ident, substituted)

    # Only digits, operators, parentheses, whitespace remain — safe to eval.
    if not re.fullmatch(r"[0-9+\-*/() \t]*", substituted):
        return 0
    if not substituted.strip():
        return 0
    try:
        value = eval(substituted, {"__builtins__": {}}, {})  # noqa: S307 - sanitised
    except (ZeroDivisionError, SyntaxError, ValueError, TypeError):
        return 0
    try:
        return max(0, int(value))
    except (ValueError, TypeError):
        return 0


def _resolve_size(ksy: Optional[KsyField], ctx: _Ctx) -> Optional[int]:
    """Resolve a field's byte size to an int, or None if size-eos / variable."""
    if ksy is None:
        return None
    if ksy.size_eos:
        return None
    size = ksy.size
    if size is None:
        return None
    if isinstance(size, int):
        return size
    ref = str(size).strip()
    m = _TO_I_REF_RE.fullmatch(ref) or _TO_I_REF_RE.search(ref)
    if m:
        return min(_MAX_EXPR_SIZE, max(0, ctx.as_int(m.group(1))))
    return 0


# ---------------------------------------------------------------------------
# Override registries (tiers 2 and 3)
# ---------------------------------------------------------------------------

# Tier 2 — per-field value bias. Key: (stem, field_id) -> SearchStrategy.
FIELD_STRATEGIES: dict[tuple[str, str], SearchStrategy] = {
    # IOMAPA: MAP_SELECT == "1" activates a fixed 4096-entry lookup table whose
    # sheer size starves Hypothesis's generation budget. Bias MAP_SELECT to the
    # other methods so the common branches are exercised; the lookup-table branch
    # is intentionally left to a dedicated case rather than the default sweep.
    ("tre_iomapa", "MAP_SELECT"): st.sampled_from(["0", "2", "3"]),
    # RSMGGA: NPLN drives two repeats — PLANES (NPLN entries) and DELTA_ORIGIN
    # (NPLN-1 entries, grid planes 2..NPLN per Table 12). The generic controller
    # generator draws NPLN down to 0, which makes NPLN-1 == -1, a negative repeat
    # count the codec rightly rejects. The spec constrains NPLN to 002-999, so
    # bias it to a small in-range value (keeping both repeats bounded and
    # non-negative). Zero-filled to the field's 3-byte width.
    ("tre_rsmgga", "NPLN"): st.sampled_from(["002", "003", "004"]),
    # SENSRB: TIME_STAMP_TYPE (12a) and PIXEL_REFERENCE_TYPE (13a) index the
    # sensrb_value_widths const map to size TIME_STAMP_VALUE (12d) /
    # PIXEL_REFERENCE_VALUE (13e) via `MAP[TYPE]` (Table Z.3-1 note h). The
    # generic BCS-A generator draws arbitrary 3-char codes that miss the map and
    # (correctly) error on the subscript. Bias both to a representative spread of
    # real note-h codes across the width range (1..20) so the map lookup
    # resolves and the dependent width stays atomic.
    ("tre_sensrb", "TIME_STAMP_TYPE"): st.sampled_from(
        ["02a", "04b", "06a", "06b", "07a", "08a", "10a"]
    ),
    ("tre_sensrb", "PIXEL_REFERENCE_TYPE"): st.sampled_from(
        ["02a", "04b", "06a", "06b", "07a", "08a", "10a"]
    ),
}

# Tier 3 — group override. Key: stem -> (claimed_ids, factory).
GroupFactory = Callable[[], SearchStrategy]
GROUP_STRATEGIES: dict[str, tuple[frozenset[str], GroupFactory]] = {}


def register_group(stem: str, claimed: set[str], factory: GroupFactory) -> None:
    """Register a tier-3 group override, asserting claimed-key disjointness.

    Overlapping claimed-key sets across overrides for the same TRE are a
    load-time error (a single TRE has one group override, so the only overlap to
    guard is a double registration).
    """
    if stem in GROUP_STRATEGIES:
        existing = GROUP_STRATEGIES[stem][0]
        overlap = existing & frozenset(claimed)
        raise ValueError(
            f"group override for {stem!r} already registered; overlapping "
            f"claimed keys {sorted(overlap)}"
        )
    GROUP_STRATEGIES[stem] = (frozenset(claimed), factory)


# Width of an RSM real-number field (21 BCS-A), used by the RSMDCB group below.
_RSM_REAL_WIDTH = 21


@st.composite
def _rsmdcb_crscov_group(draw) -> dict:
    """Jointly generate RSMDCB's per-image count fields and the CRSCOV blocks.

    ``CRSCOV_BLOCKS`` is a per-image repeat (one ``crscov_block(_index)`` per
    image) whose inner ``CRSCOV`` count is
    ``NROWCB.to_i * IMAGE_RECORDS[image_index].NCOLCB.to_i`` — a subscript into
    the earlier ``IMAGE_RECORDS`` loop selected by the bound image index. The
    strategy's arithmetic-only ``_safe_eval_count`` cannot evaluate that
    subscript (it has no access to the array node or the ``image_index``
    parameter), so it would generate empty ``CRSCOV`` lists that the writer —
    correctly computing a non-zero count — rejects. Generate the correlated set
    in one shot instead: each block holds exactly ``NROWCB * NCOLCB[image]``
    elements, and both ``IMAGE_RECORDS`` and ``CRSCOV_BLOCKS`` have ``NIMGE``
    entries (count/list atomicity for the two ``NIMGE``-driven repeats).
    """
    nrowcb = draw(st.integers(min_value=1, max_value=_MAX_REPEAT_COUNT))
    nimge = draw(st.integers(min_value=1, max_value=_MAX_REPEAT_COUNT))
    ncolcb = [
        draw(st.integers(min_value=1, max_value=_MAX_REPEAT_COUNT))
        for _ in range(nimge)
    ]
    images = [
        {"IIDI": draw(_bcs_a(80)), "NCOLCB": str(c).zfill(2)} for c in ncolcb
    ]
    blocks = [
        {"CRSCOV": [draw(_bcs_a(_RSM_REAL_WIDTH)) for _ in range(nrowcb * c)]}
        for c in ncolcb
    ]
    return {
        "NROWCB": str(nrowcb).zfill(2),
        "NIMGE": str(nimge).zfill(3),
        "IMAGE_RECORDS": images,
        "CRSCOV_BLOCKS": blocks,
    }


register_group(
    "tre_rsmdcb",
    {"NROWCB", "NIMGE", "IMAGE_RECORDS", "CRSCOV_BLOCKS"},
    _rsmdcb_crscov_group,
)


# ---------------------------------------------------------------------------
# Composite generation
# ---------------------------------------------------------------------------


# A simple equality guard: ``FIELD == "LIT"`` (optionally ``_root.``-prefixed).
_EQ_GUARD_RE = re.compile(
    r'^\s*(?:_root\.|_parent\.)?([A-Za-z_][A-Za-z0-9_]*)\s*==\s*"([^"]*)"\s*$'
)


def _disabled_controllers(
    fields: list[FieldInfo], schema_types: dict[str, list[FieldInfo]]
) -> dict[str, set[str]]:
    """Map controller id -> literal values that must be avoided.

    A single (non-repeated) TypeRef that is conditional, **not populatable**, and
    guarded by a simple ``FIELD == "LIT"`` condition cannot be satisfied if its
    condition fires (we have no nested content to emit, so the field would be
    active-but-omitted and the writer stalls). Biasing ``FIELD`` away from
    ``"LIT"`` keeps that branch inactive; a populatable sibling branch (e.g.
    CSEXRB's ``SCANNER_DATA`` when ``SENSOR_TYPE != "F"``) activates instead.
    """
    forbidden: dict[str, set[str]] = {}
    for f in fields:
        if f.kind != "typeref" or f.repeated or not f.conditional:
            continue
        if _type_populatable(f.type_name, schema_types):
            continue
        ksy = f.ksy
        if ksy is None or ksy.condition is None:
            continue
        m = _EQ_GUARD_RE.match(ksy.condition)
        if m:
            forbidden.setdefault(m.group(1), set()).add(m.group(2))
    return forbidden


def _force_zero_controllers(
    fields: list[FieldInfo], schema_types: dict[str, list[FieldInfo]]
) -> frozenset[str]:
    """Controller field ids that must be 0 because they govern an empty list.

    A repeated TypeRef whose element type is unresolved or not populatable is
    generated as an empty list (its nested branch is left uncovered). The
    writer trusts the controller, so the controller must read as 0 to match the
    empty list. This finds those controllers among ``fields`` (one scope) by
    parsing each such field's ``repeat-expr`` for the names it references.
    """
    zero: set[str] = set()
    for f in fields:
        if not f.repeated or f.kind != "typeref":
            continue
        if _type_populatable(f.type_name, schema_types):
            continue
        if f.ksy is None or f.ksy.repeat_expr is None:
            continue
        for tok in _IDENT_RE.findall(f.ksy.repeat_expr):
            if tok not in _NON_FIELD_TOKENS:
                zero.add(tok)
    return frozenset(zero)


@st.composite
def _gen_fields(
    draw,
    fields: list[FieldInfo],
    schema_types: dict[str, list[FieldInfo]],
    ref_names: frozenset[str],
    stem: str,
    ctx: _Ctx,
    claimed: frozenset[str],
    group_values: dict,
) -> dict:
    """Generate a value dict for a field sequence in definition order.

    ``ctx`` accumulates drawn scalars so later expression sizes/counts resolve.
    ``claimed``/``group_values`` overlay a tier-3 group override's pre-drawn
    values for the claimed ids (top-level scope only).
    """
    out: dict = {}
    zero_controllers = _force_zero_controllers(fields, schema_types)
    disabled = _disabled_controllers(fields, schema_types)
    for info in fields:
        fid = info.id

        # A controller governing an empty (non-populatable) list must read 0 so
        # the writer's trusted count matches the empty list.
        if fid in zero_controllers and fid not in claimed:
            value: object
            if info.kind in ("uint", "sint"):
                value = 0
            else:
                size = _resolve_size(info.ksy, ctx)
                n = size if isinstance(size, int) and size > 0 else 1
                value = "0" * n
            out[fid] = value
            ctx.local[fid] = value
            continue

        # Tier 3: a group override owns this field — use its pre-drawn value.
        if fid in claimed:
            if fid in group_values:
                value = group_values[fid]
                out[fid] = value
                if not isinstance(value, (list, dict)):
                    ctx.local[fid] = value
            continue

        if info.kind == "typeref":
            out[fid] = draw(_gen_typeref(info, schema_types, ref_names, stem, ctx))
            continue

        if info.repeated:
            out[fid] = draw(_gen_repeat_scalar(info, ref_names, ctx))
            continue

        # Scalar field
        size = _resolve_size(info.ksy, ctx)
        numeric = fid in ref_names
        override = FIELD_STRATEGIES.get((stem, fid))
        forbid = disabled.get(fid)
        if override is not None:
            value = draw(override)
        elif forbid is not None and info.kind == "string":
            # Controller for a non-populatable guarded TypeRef: avoid the literal
            # that would activate that (unsatisfiable) branch.
            base = scalar_strategy(info, _resolve_size(info.ksy, ctx), numeric_only=numeric)
            value = draw(base.filter(lambda s, _f=forbid: s.rstrip(" ") not in _f))
        elif numeric:
            # Field is consumed as a size/count: keep it small and bounded so the
            # dependent width/length stays atomic. Integer-typed controllers get a
            # small int; text controllers get a digits-only zero-filled string.
            small = draw(st.integers(min_value=0, max_value=_MAX_REPEAT_COUNT))
            if info.kind in ("uint", "sint"):
                value = small
            else:
                n = size if isinstance(size, int) and size > 0 else 1
                cap = min(_MAX_REPEAT_COUNT, 10 ** n - 1)
                value = str(min(small, cap)).zfill(n)
        else:
            value = draw(scalar_strategy(info, size, numeric_only=False))
        out[fid] = value
        if not isinstance(value, (list, dict)):
            ctx.local[fid] = value

    return out


_LARGE_COUNT = 256  # above this, an unbounded fixed-count array starves generation


@st.composite
def _gen_repeat_scalar(
    draw, info: FieldInfo, ref_names: frozenset[str], ctx: _Ctx
) -> list:
    """Generate a list of scalar values for a repeated primitive field.

    The element count is derived from the field's ``repeat-expr`` evaluated
    against ``ctx`` (count/list atomicity), never drawn independently.

    A field with a very large *fixed literal* count (e.g. IOMAPA's 4096-entry
    table) would starve Hypothesis. Such fields in the corpus are conditional;
    we bias their controller off (tier-2) so they stay inactive, and emit a
    bounded list here — ``encode`` ignores it while the field is inactive. An
    *unconditional* large literal (e.g. RPC00B's 20-coeff arrays) is emitted in
    full, as the writer requires.
    """
    if info.ksy is not None and info.ksy.repeat_open:
        # eos/until: no count field to derive from. On encode the supplied list
        # length is authoritative, so draw a small non-empty count to exercise
        # the open-repeat encode path (see BUG_TRE_REPEAT_EOS_ENCODE_UNSUPPORTED).
        count = draw(st.integers(min_value=1, max_value=_MAX_REPEAT_COUNT))
    else:
        count = _safe_eval_count(str(info.ksy.repeat_expr), ctx) if info.ksy else 0
    if count > _LARGE_COUNT and info.conditional:
        count = 0
    size = _resolve_size(info.ksy, ctx)
    numeric = info.id in ref_names
    elem = scalar_strategy(info, size, numeric_only=numeric)
    return [draw(elem) for _ in range(count)]


def _type_populatable(
    type_name: Optional[str],
    schema_types: dict[str, list[FieldInfo]],
    _seen: Optional[frozenset[str]] = None,
) -> bool:
    """Whether a nested type can be safely populated with non-empty content.

    A type is populatable iff it — and every nested type it (transitively)
    references — is **declared in the structure's flat type map**. The codec
    resolves all TypeRefs against that single flat map regardless of where a type
    is declared or referenced, and seeds each nested scope with the enclosing
    scalar values, so ``_root.``/``_parent.`` size and repeat expressions resolve
    too. The only thing
    the generator cannot populate is a reference to a type that is **not declared
    anywhere** — that reference misses the flat map and the codec cannot
    serialise it; such a sub-tree is left empty and logged in the coverage
    ledger.

    A type referencing itself is treated as populatable at the recursion guard
    (its own fields are checked once).

    Note: this keys on flat-map membership, **not** on the per-scope
    ``type_resolved`` flag from ``describe()``. That flag reflects lexical
    resolution at the reference's own scope and reads ``False`` for a sibling-
    scope nested TypeRef even though the codec resolves it against the flat map.
    """
    if not type_name:
        return False
    fields = schema_types.get(type_name)
    if fields is None:
        return False
    seen = (_seen or frozenset()) | {type_name}
    for f in fields:
        if f.kind == "typeref":
            if f.type_name in seen:
                continue
            if not _type_populatable(f.type_name, schema_types, seen):
                return False
    return True


@st.composite
def _gen_typeref(
    draw,
    info: FieldInfo,
    schema_types: dict[str, list[FieldInfo]],
    ref_names: frozenset[str],
    stem: str,
    ctx: _Ctx,
) -> object:
    """Generate a nested dict (single) or list of nested dicts (repeated).

    Nested types not declared in the flat type map (a reference that resolves
    nowhere) are left empty (repeated -> ``[]``, single -> skipped). Such a field
    is either conditional (and inactive at the values we generate) or its nested
    branch is left uncovered (logged in the coverage ledger).
    """
    populatable = _type_populatable(info.type_name, schema_types)
    nested_fields = schema_types.get(info.type_name) if populatable else None

    if info.repeated:
        if not nested_fields:
            return []
        if info.ksy is not None and info.ksy.repeat_open:
            # eos/until: no count field to derive from. On encode the supplied
            # list length is authoritative, so draw a small non-empty count to
            # exercise the open-repeat encode path (e.g. FSYNWA's CEDATA group;
            # see BUG_TRE_REPEAT_EOS_ENCODE_UNSUPPORTED).
            count = draw(st.integers(min_value=1, max_value=_MAX_REPEAT_COUNT))
        else:
            count = _safe_eval_count(str(info.ksy.repeat_expr), ctx) if info.ksy else 0
        elems = []
        for _ in range(count):
            child = _Ctx(parent=ctx, root=ctx.root)
            elems.append(
                draw(_gen_fields(nested_fields, schema_types, ref_names, stem,
                                 child, frozenset(), {}))
            )
        return elems

    # Single nested field — only populate when resolved; otherwise omit so an
    # inactive conditional typeref does not force an unsupported nested type.
    if not nested_fields:
        return None
    child = _Ctx(parent=ctx, root=ctx.root)
    return draw(_gen_fields(nested_fields, schema_types, ref_names, stem,
                            child, frozenset(), {}))


@st.composite
def tre_instance(draw, ksy_path: Path, descriptor: dict) -> dict:
    """Generate a nested value dict for a whole TRE.

    ``descriptor`` is ``StructureDefinition.describe()`` for this TRE; the KSY at
    ``ksy_path`` supplies size/encoding metadata. The result is a nested dict
    suitable for ``StructureDefinition.encode``.
    """
    schema: KsySchema = load_ksy_schema(ksy_path)
    ref_names = _ref_field_names(schema)

    top = merge_descriptor(descriptor["fields"], schema.fields)
    nested: dict[str, list[FieldInfo]] = {
        type_name: merge_descriptor(type_descs, schema.types.get(type_name, []))
        for type_name, type_descs in descriptor["types"].items()
    }

    stem = ksy_path.stem
    ctx = _Ctx(parent=None, root=None)

    group = GROUP_STRATEGIES.get(stem)
    if group is not None:
        claimed, factory = group
        group_values = dict(draw(factory()))
    else:
        claimed, group_values = frozenset(), {}

    return draw(
        _gen_fields(top, nested, ref_names, stem, ctx, claimed, group_values)
    )


# ---------------------------------------------------------------------------
# Size / repeat-count reference detection
# ---------------------------------------------------------------------------


def _ref_field_names(schema: KsySchema) -> frozenset[str]:
    """Field ids whose value is consumed as an integer — size, repeat count, or
    an integer (``.to_i``) comparison in a condition.

    Such fields are generated digits-only (and small) so (a) the value is usable
    as a length/count without overflowing the field and (b) an integer condition
    like ``TRESHL.to_i >= 283`` evaluates *stably* across the canonicalization
    cycle (a free BCS-N value such as ``"+9 "`` reformats between encode/decode,
    flipping the branch). Includes references inside nested types.
    """
    refs: set[str] = set()

    def scan(fields: list[KsyField]) -> None:
        for f in fields:
            if isinstance(f.size, str):
                for m in _TO_I_REF_RE.finditer(f.size):
                    refs.add(m.group(1).split(".")[-1])
            if f.repeat_expr is not None:
                for tok in _IDENT_RE.findall(f.repeat_expr):
                    if tok not in _NON_FIELD_TOKENS:
                        refs.add(tok)
            if f.condition is not None:
                for m in _TO_I_REF_RE.finditer(f.condition):
                    refs.add(m.group(1).split(".")[-1])

    scan(schema.fields)
    for nested in schema.types.values():
        scan(nested)
    return frozenset(refs)
