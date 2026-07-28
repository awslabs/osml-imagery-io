#!/usr/bin/env python3
"""Generate a MyST-compatible Markdown performance report from pytest-benchmark JSON output.

Reads the JSON file produced by pytest-benchmark and writes a Markdown fragment
suitable for inclusion in the project's Sphinx documentation site.

When no input file is given, the script searches ``.benchmarks/`` for the most
recent saved result (produced by ``--benchmark-autosave``).

Usage:
    python scripts/generate_benchmark_report.py
    python scripts/generate_benchmark_report.py .benchmarks/Linux-CPython-3.12/0001_abc.json
    python scripts/generate_benchmark_report.py -o docs/_benchmark_results.md
"""

import argparse
import json
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Time formatting
# ---------------------------------------------------------------------------

def format_time(seconds: float) -> str:
    """Format a time value as milliseconds (no unit suffix).

    Args:
        seconds: Time in seconds.

    Returns:
        Formatted string in milliseconds with no decimal places.
    """
    return f"{seconds * 1e3:.0f}"


# ---------------------------------------------------------------------------
# Input resolution
# ---------------------------------------------------------------------------

_DEFAULT_BENCHMARKS_DIR = Path(".benchmarks")


def find_latest_benchmark(benchmarks_dir: Path) -> Path | None:
    """Find the most recently modified JSON file under the .benchmarks/ tree.

    pytest-benchmark autosave writes files like:
        .benchmarks/<machine>/<NNNN>_<commit>.json

    Returns the path to the newest file, or None if no JSON files exist.
    """
    json_files = sorted(benchmarks_dir.rglob("*.json"), key=lambda p: p.stat().st_mtime)
    return json_files[-1] if json_files else None


# ---------------------------------------------------------------------------
# JSON parsing helpers
# ---------------------------------------------------------------------------

def parse_benchmark_json(path: Path) -> list[dict]:
    """Read and validate a pytest-benchmark JSON file.

    Args:
        path: Path to the JSON file.

    Returns:
        List of benchmark entry dicts.

    Raises:
        SystemExit: On missing file, parse error, or empty results.
    """
    if not path.is_file():
        print(f"Error: file not found: {path}", file=sys.stderr)
        sys.exit(1)

    try:
        raw = path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"Error: cannot read file: {exc}", file=sys.stderr)
        sys.exit(1)

    # pytest-benchmark occasionally produces corrupted files when a run is
    # interrupted or the file is appended to.  Use JSONDecoder to extract
    # the first complete JSON object rather than requiring the entire file
    # to be valid.
    try:
        decoder = json.JSONDecoder()
        data, _ = decoder.raw_decode(raw)
    except (json.JSONDecodeError, ValueError) as exc:
        print(f"Error: invalid JSON: {exc}", file=sys.stderr)
        sys.exit(1)

    benchmarks = data.get("benchmarks", [])
    if not benchmarks:
        print(f"Error: no benchmark results found in {path}", file=sys.stderr)
        sys.exit(1)

    return benchmarks


# ---------------------------------------------------------------------------
# Grouping and table generation
# ---------------------------------------------------------------------------

def _extract_dataset_label(name: str) -> str:
    """Extract the dataset label from a benchmark name.

    pytest-benchmark names look like ``test_bench_metadata_read[Large NITF]``
    or ``test_bench_block_read[Large NITF-UL]``.  We extract the portion
    inside the brackets as the dataset/parameter label.
    """
    if "[" in name and name.endswith("]"):
        return name[name.index("[") + 1 : -1]
    return name


def _extract_operation(name: str) -> str:
    """Extract a human-readable operation name from the test function name."""
    # Strip module prefix if present (e.g. "test_bench_metadata.py::test_bench_metadata_read[...]")
    if "::" in name:
        name = name.split("::")[-1]
    # Strip parameters
    if "[" in name:
        name = name[: name.index("[")]
    # Strip test_bench_ prefix
    if name.startswith("test_bench_"):
        name = name[len("test_bench_"):]
    return name


_ACCESS_PATTERNS = frozenset({"single_tile", "small_roi", "large_roi"})

# Backend/source suffixes that may trail a benchmark label. ``local``/``s3`` are
# the Zarr-read backends; ``virtual`` is the Remote-OwnedBuffer range-read source
# dimension added to the IO/metadata/index-gen benchmarks (bytes flow through a
# Python file-like handle — BytesIO / fsspec — rather than direct disk access).
_BACKEND_SUFFIXES = frozenset({"local", "s3", "virtual"})


def _extract_access_pattern(name: str) -> str | None:
    """Extract access pattern from benchmark name if present.

    Handles two ID formats:
    - Zarr: ``test_bench_zarr_read[WV Pan J2K-single_tile-local]``
    - IO: ``test_bench_io_read[WV Pan J2K-single_tile-virtual]``
      (and, historically, the backend-less ``WV Pan J2K-single_tile``)

    Returns:
        A human-readable access pattern string (e.g. ``"single tile"``),
        or ``None`` for benchmarks that don't carry an access pattern.
    """
    label = _extract_dataset_label(name)
    if "-" not in label:
        return None

    # Try 3-segment format first: label-pattern-backend
    rest, last = label.rsplit("-", 1)
    if last in _BACKEND_SUFFIXES and "-" in rest:
        _dataset, pattern = rest.rsplit("-", 1)
        if pattern in _ACCESS_PATTERNS:
            return pattern.replace("_", " ")

    # Try 2-segment format: label-pattern (no backend suffix)
    _dataset, pattern = label.rsplit("-", 1)
    if pattern in _ACCESS_PATTERNS:
        return pattern.replace("_", " ")

    return None


def _strip_access_suffixes(label: str) -> str:
    """Strip access pattern and optional backend suffix from a dataset label.

    ``"WV Pan J2K-single_tile-virtual"`` → ``"WV Pan J2K"``
    ``"WV Pan J2K-single_tile-local"`` → ``"WV Pan J2K"``
    ``"WV Pan J2K-single_tile"`` → ``"WV Pan J2K"``
    """
    if "-" not in label:
        return label

    # Try 3-segment: strip backend then pattern
    rest, last = label.rsplit("-", 1)
    if last in _BACKEND_SUFFIXES and "-" in rest:
        maybe_dataset, pattern = rest.rsplit("-", 1)
        if pattern in _ACCESS_PATTERNS:
            return maybe_dataset

    # Try 2-segment: strip pattern only
    maybe_dataset, pattern = label.rsplit("-", 1)
    if pattern in _ACCESS_PATTERNS:
        return maybe_dataset

    return label


def _strip_source_suffix(label: str) -> str:
    """Strip a trailing recognized source suffix from a dataset label.

    ``"Synth Medium C8-virtual"`` → ``"Synth Medium C8"``.  Used for the
    metadata/index-gen groups whose ids are ``dataset-source`` with no access
    pattern (unlike the tile-read ids handled by ``_strip_access_suffixes``).
    """
    if "-" not in label:
        return label
    rest, last = label.rsplit("-", 1)
    return rest if last in _BACKEND_SUFFIXES else label


def _clean_dataset(label: str) -> str:
    """Strip access-pattern and/or trailing source suffixes from a label.

    Strips an access pattern (and its trailing backend) first, then any bare
    trailing source suffix left on ids that carry no access pattern — so the
    Dataset cell shows only the dataset name for every id shape.
    """
    return _strip_source_suffix(_strip_access_suffixes(label))


def _extract_source(entry: dict) -> str | None:
    """Return the source/backend dimension for a benchmark entry, or ``None``.

    Prefers the explicit ``extra_info.source_mode`` recorded by the
    IO/metadata/index-gen benchmarks (``"local"`` / ``"virtual"``); falls back
    to a recognized trailing suffix on the benchmark id (``-local`` / ``-virtual``
    / ``-s3``).  Returns ``None`` when the entry carries no source dimension.
    """
    source = entry.get("extra_info", {}).get("source_mode")
    if source:
        return source

    label = _extract_dataset_label(entry.get("name", ""))
    if "-" not in label:
        return None
    _rest, last = label.rsplit("-", 1)
    return last if last in _BACKEND_SUFFIXES else None


def _extract_fetch_fraction(entry: dict) -> float | None:
    """Return the remote ``fetch_fraction`` from ``extra_info`` if present."""
    frac = entry.get("extra_info", {}).get("fetch_fraction")
    return float(frac) if isinstance(frac, (int, float)) else None


def group_benchmarks(benchmarks: list[dict]) -> dict[str, list[dict]]:
    """Group benchmark entries by their ``group`` field.

    Entries without a group are placed under ``"ungrouped"``.
    """
    groups: dict[str, list[dict]] = {}
    for entry in benchmarks:
        group = entry.get("group") or "ungrouped"
        groups.setdefault(group, []).append(entry)
    return groups


def generate_table(entries: list[dict], group_name: str = "") -> str:
    """Generate a MyST-compatible Markdown table for a list of benchmark entries.

    Columns are added on demand so each group shows only what it carries:

    - **Access Pattern** — when *group_name* contains ``tile_read``.
    - **Source** — when the group mixes ≥2 source dimensions in one group
      (e.g. the IO/metadata/index-gen benchmarks that parametrize
      ``local`` and ``virtual`` together). Groups with a single source — such as
      the per-backend Zarr groups (``tile_read_zarr_local`` /
      ``tile_read_zarr_s3``), whose group name already carries the backend —
      omit it, preserving the original layout.
    - **Fetch %** — when any entry records a ``fetch_fraction``; blank for
      direct-access (``local``) rows. Surfaces the range-read reduction the
      ``virtual`` / ``s3`` runs measure (bytes fetched ÷ file size).
    """
    is_tile_read = "tile_read" in group_name

    sources = {s for e in entries if (s := _extract_source(e)) is not None}
    show_source = len(sources) >= 2
    show_fetch = any(_extract_fetch_fraction(e) is not None for e in entries)

    headers = ["Operation", "Dataset"]
    if is_tile_read:
        headers.append("Access Pattern")
    if show_source:
        headers.append("Source")
    if show_fetch:
        headers.append("Fetch %")
    headers += ["Min", "Max", "Mean", "Median", "StdDev", "Rounds"]

    rows = ["| " + " | ".join(headers) + " |", "| " + " | ".join(["---"] * len(headers)) + " |"]

    # Sort by (dataset, access pattern, source, mean) so local/remote pairs for
    # the same dataset sit next to each other rather than interleaving by time.
    def _sort_key(e: dict) -> tuple:
        label = _extract_dataset_label(e.get("name", ""))
        return (
            _clean_dataset(label),
            _extract_access_pattern(e.get("name", "")) or "",
            _extract_source(e) or "",
            e.get("stats", {}).get("mean", 0),
        )

    for entry in sorted(entries, key=_sort_key):
        name = entry.get("name", "")
        stats = entry.get("stats", {})
        label = _extract_dataset_label(name)

        cells = [_extract_operation(name)]
        # Strip access-pattern/backend suffixes from the label whenever the group
        # surfaces those as their own columns, so the Dataset cell stays clean.
        if is_tile_read or show_source:
            cells.append(_clean_dataset(label))
        else:
            cells.append(label)
        if is_tile_read:
            cells.append(_extract_access_pattern(name) or "")
        if show_source:
            cells.append(_extract_source(entry) or "")
        if show_fetch:
            frac = _extract_fetch_fraction(entry)
            cells.append(f"{frac * 100:.1f}" if frac is not None else "")
        cells += [
            format_time(stats.get("min", 0)),
            format_time(stats.get("max", 0)),
            format_time(stats.get("mean", 0)),
            format_time(stats.get("median", 0)),
            format_time(stats.get("stddev", 0)),
            str(stats.get("rounds", 0)),
        ]
        rows.append("| " + " | ".join(cells) + " |")

    return "\n".join(rows) + "\n\nAll times in milliseconds (ms)."


# ---------------------------------------------------------------------------
# Comparison summary
# ---------------------------------------------------------------------------

_READ_GROUPS = ("tile_read_io", "tile_read_zarr_local", "tile_read_zarr_s3")
_GROUP_LABELS = {
    "tile_read_io": "IO",
    "tile_read_zarr_local": "Zarr Local",
    "tile_read_zarr_s3": "Zarr S3",
}


def _comparison_key(entry: dict) -> tuple[str, str]:
    """Return (dataset, access_pattern) for a tile-read benchmark entry."""
    name = entry.get("name", "")
    dataset = _clean_dataset(_extract_dataset_label(name))
    pattern = _extract_access_pattern(name) or ""
    return (dataset, pattern)


# Order sources deterministically within a group's columns: the IO-abstraction
# cost ladder — direct disk access → virtualized (Python file-like) IO → network.
_SOURCE_ORDER = {"local": 0, "virtual": 1, "s3": 2}


def generate_comparison_table(groups: dict[str, list[dict]]) -> str | None:
    """Build a side-by-side comparison of mean times across read paths.

    Each column is a (group, source) pair.  Groups that carry an internal source
    dimension (``tile_read_io`` — local/virtual/s3 in one group) expand into
    one column per source, so the range-read paths are compared rather than
    collapsed; the per-backend Zarr groups stay as a single column each (their
    backend is already encoded in the group name).

    Returns a Markdown table string, or ``None`` if fewer than two columns are
    present.
    """
    present = [g for g in _READ_GROUPS if g in groups]

    # Build the ordered list of (group, source, column_label) columns.
    columns: list[tuple[str, str | None, str]] = []
    for group_name in present:
        sources = {s for e in groups[group_name] if (s := _extract_source(e)) is not None}
        base = _GROUP_LABELS[group_name]
        if len(sources) >= 2:
            for src in sorted(sources, key=lambda s: _SOURCE_ORDER.get(s, 99)):
                columns.append((group_name, src, f"{base} ({src})"))
        else:
            columns.append((group_name, None, base))

    if len(columns) < 2:
        return None

    # Collect mean times keyed by (dataset, pattern) for each column.
    means: dict[int, dict[tuple[str, str], float]] = {}
    for col_idx, (group_name, src, _label) in enumerate(columns):
        means[col_idx] = {}
        for entry in groups[group_name]:
            if src is not None and _extract_source(entry) != src:
                continue
            means[col_idx][_comparison_key(entry)] = entry.get("stats", {}).get("mean", 0)

    all_keys = sorted(
        {k for m in means.values() for k in m},
        key=lambda k: means[0].get(k, float("inf")),
    )

    if not all_keys:
        return None

    col_headers = " | ".join(label for _g, _s, label in columns)
    header = f"| Dataset | Access Pattern | {col_headers} |"
    separator = "| --- | --- |" + " --- |" * len(columns)

    rows = [header, separator]
    for dataset, pattern in all_keys:
        values = []
        for col_idx in range(len(columns)):
            val = means[col_idx].get((dataset, pattern))
            values.append(format_time(val) if val is not None else "—")
        vals_str = " | ".join(values)
        rows.append(f"| {dataset} | {pattern} | {vals_str} |")

    return "\n".join(rows) + "\n\nAll times in milliseconds (ms)."


# ---------------------------------------------------------------------------
# Report assembly
# ---------------------------------------------------------------------------

def generate_report(benchmarks: list[dict]) -> str:
    """Produce a MyST Markdown fragment containing only benchmark result tables.

    This is intended to be included into ``docs/performance.md`` via a MyST
    ``include`` directive, so it deliberately omits page titles and explanatory
    prose.
    """
    lines: list[str] = []
    groups = group_benchmarks(benchmarks)

    # Comparison summary at the top
    comparison = generate_comparison_table(groups)
    if comparison:
        lines.append("### Read Performance Comparison")
        lines.append("")
        lines.append(comparison)
        lines.append("")

    for group_name, entries in groups.items():
        lines.append(f"### {_group_heading(group_name)}")
        lines.append("")
        lines.append(generate_table(entries, group_name=group_name))
        lines.append("")

    return "\n".join(lines)


# Section headings that ``str.title()`` would mangle (it lowercases the tail of
# each word, turning acronyms like "IO"/"S3" into "Io"/"S3").  Map them explicitly.
_GROUP_HEADINGS = {
    "tile_read_io": "Tile Read IO",
    "tile_read_zarr_local": "Tile Read Zarr Local",
    "tile_read_zarr_s3": "Tile Read Zarr S3",
}


def _group_heading(group_name: str) -> str:
    """Human-readable section heading for a benchmark group name."""
    return _GROUP_HEADINGS.get(group_name, group_name.replace("_", " ").title())


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Generate a MyST Markdown performance report from pytest-benchmark JSON."
    )
    parser.add_argument(
        "input",
        nargs="?",
        default=None,
        help="Path to a pytest-benchmark JSON file. "
        "If omitted, uses the latest result from .benchmarks/.",
    )
    parser.add_argument(
        "-o",
        "--output",
        default="docs/_benchmark_results.md",
        help="Output Markdown file path (default: docs/_benchmark_results.md).",
    )

    args = parser.parse_args(argv)

    # Resolve input path
    if args.input is not None:
        input_path = Path(args.input)
    else:
        input_path = find_latest_benchmark(_DEFAULT_BENCHMARKS_DIR)
        if input_path is None:
            print(
                "Error: no benchmark results found in .benchmarks/. "
                "Run benchmarks with: pytest -m benchmark --benchmark-autosave",
                file=sys.stderr,
            )
            return 1
        print(f"Using latest result: {input_path}")

    output_path = Path(args.output)

    # Validate output directory exists
    if not output_path.parent.exists():
        print(
            f"Error: output directory does not exist: {output_path.parent}",
            file=sys.stderr,
        )
        return 1

    benchmarks = parse_benchmark_json(input_path)
    report = generate_report(benchmarks)
    output_path.write_text(report, encoding="utf-8")

    print(f"Report written to {output_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
