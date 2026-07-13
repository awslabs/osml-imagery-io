"""YAML-to-dataclass parser for KSY structure definition files.

Parses the `seq` and `types` sections of a KSY file and returns a list of
KsyField objects that the strategy builder can iterate over to generate
Hypothesis values.

Only the subset of KSY attributes used by TRE definitions is handled:
  - id, type, size, size-eos, encoding, if, repeat, repeat-expr
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional, Union

import yaml


@dataclass
class KsyField:
    id: str
    ksy_type: Optional[str] = None          # e.g. "str", "u1", "u4", a TypeRef name
    size: Optional[Union[int, str]] = None  # fixed int or expression string
    size_eos: bool = False
    encoding: Optional[str] = None          # BCS-A, BCS-N, ASCII, UTF-8
    condition: Optional[str] = None         # raw `if:` expression string
    repeat_expr: Optional[str] = None       # raw repeat-expr string (implies repeat: expr)
    # True for `repeat: eos` or `repeat: until` — a repeat whose element count is
    # not derivable from a count field. On encode the supplied list length is
    # authoritative, so the strategy emits a small non-empty list for these.
    repeat_open: bool = False


@dataclass
class KsySchema:
    id: str
    fields: list[KsyField] = field(default_factory=list)
    types: dict[str, list[KsyField]] = field(default_factory=dict)  # nested type name -> fields


def _parse_fields(seq: list[dict]) -> list[KsyField]:
    """Parse a KSY `seq` list into KsyField objects."""
    result = []
    for entry in seq:
        if not isinstance(entry, dict):
            continue
        field_id = entry.get("id", "")

        ksy_type = entry.get("type")
        if ksy_type is not None:
            ksy_type = str(ksy_type)

        raw_size = entry.get("size")
        if raw_size is not None:
            try:
                size = int(raw_size)
            except (TypeError, ValueError):
                size = str(raw_size)
        else:
            size = None

        size_eos = bool(entry.get("size-eos", False))
        encoding = entry.get("encoding")
        condition = entry.get("if")
        if condition is not None:
            condition = str(condition).strip()

        repeat = entry.get("repeat")
        repeat_expr = None
        if repeat == "expr":
            repeat_expr = str(entry["repeat-expr"])
        repeat_open = repeat in ("eos", "until")

        result.append(KsyField(
            id=field_id,
            ksy_type=ksy_type,
            size=size,
            size_eos=size_eos,
            encoding=encoding,
            condition=condition,
            repeat_expr=repeat_expr,
            repeat_open=repeat_open,
        ))
    return result


def load_ksy_schema(ksy_path: Path) -> KsySchema:
    """Load and parse a KSY file, returning a KsySchema."""
    with open(ksy_path, encoding="utf-8") as fh:
        raw = yaml.safe_load(fh)

    schema_id = raw.get("meta", {}).get("id", ksy_path.stem)

    seq = raw.get("seq", [])
    fields = _parse_fields(seq)

    types: dict[str, list[KsyField]] = {}
    for type_name, type_def in (raw.get("types") or {}).items():
        nested_seq = type_def.get("seq", [])
        types[type_name] = _parse_fields(nested_seq)

    return KsySchema(id=schema_id, fields=fields, types=types)
