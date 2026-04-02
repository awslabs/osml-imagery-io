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


def _extract_access_pattern(name: str) -> str | None:
    """Extract access pattern from benchmark name if present.

    Handles two ID formats:
    - Zarr: ``test_bench_zarr_read[WV Pan J2K-single_tile-local]``
    - Native: ``test_bench_native_read[WV Pan J2K-single_tile]``

    Returns:
        A human-readable access pattern string (e.g. ``"single tile"``),
        or ``None`` for benchmarks that don't carry an access pattern.
    """
    label = _extract_dataset_label(name)
    if "-" not in label:
        return None

    # Try 3-segment format first: label-pattern-backend
    rest, last = label.rsplit("-", 1)
    if last in ("local", "s3") and "-" in rest:
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

    ``"WV Pan J2K-single_tile-local"`` → ``"WV Pan J2K"``
    ``"WV Pan J2K-single_tile"`` → ``"WV Pan J2K"``
    """
    if "-" not in label:
        return label

    # Try 3-segment: strip backend then pattern
    rest, last = label.rsplit("-", 1)
    if last in ("local", "s3") and "-" in rest:
        maybe_dataset, pattern = rest.rsplit("-", 1)
        if pattern in _ACCESS_PATTERNS:
            return maybe_dataset

    # Try 2-segment: strip pattern only
    maybe_dataset, pattern = label.rsplit("-", 1)
    if pattern in _ACCESS_PATTERNS:
        return maybe_dataset

    return label


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

    When *group_name* contains ``tile_read``, an extra **Access Pattern** column
    is inserted between Dataset and Min.  For all other groups the original
    eight-column format is preserved unchanged.
    """
    is_tile_read = "tile_read" in group_name

    if is_tile_read:
        header = "| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |"
        separator = "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    else:
        header = "| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |"
        separator = "| --- | --- | --- | --- | --- | --- | --- | --- |"

    rows = [header, separator]

    sorted_entries = sorted(entries, key=lambda e: e.get("stats", {}).get("mean", 0))

    for entry in sorted_entries:
        name = entry.get("name", "")
        stats = entry.get("stats", {})
        operation = _extract_operation(name)
        label = _extract_dataset_label(name)

        if is_tile_read:
            access_pattern = _extract_access_pattern(name) or ""
            # Strip access-pattern and backend suffixes from the label so only
            # the dataset name is shown (e.g. "WV Pan J2K" instead of
            # "WV Pan J2K-single_tile-local").
            dataset = _strip_access_suffixes(label)
            row = (
                f"| {operation} "
                f"| {dataset} "
                f"| {access_pattern} "
                f"| {format_time(stats.get('min', 0))} "
                f"| {format_time(stats.get('max', 0))} "
                f"| {format_time(stats.get('mean', 0))} "
                f"| {format_time(stats.get('median', 0))} "
                f"| {format_time(stats.get('stddev', 0))} "
                f"| {stats.get('rounds', 0)} |"
            )
        else:
            row = (
                f"| {operation} "
                f"| {label} "
                f"| {format_time(stats.get('min', 0))} "
                f"| {format_time(stats.get('max', 0))} "
                f"| {format_time(stats.get('mean', 0))} "
                f"| {format_time(stats.get('median', 0))} "
                f"| {format_time(stats.get('stddev', 0))} "
                f"| {stats.get('rounds', 0)} |"
            )
        rows.append(row)

    return "\n".join(rows) + "\n\nAll times in milliseconds (ms)."


# ---------------------------------------------------------------------------
# Comparison summary
# ---------------------------------------------------------------------------

_READ_GROUPS = ("tile_read_native", "tile_read_zarr_local", "tile_read_zarr_s3")
_GROUP_LABELS = {
    "tile_read_native": "Native",
    "tile_read_zarr_local": "Zarr Local",
    "tile_read_zarr_s3": "Zarr S3",
}


def _comparison_key(entry: dict) -> tuple[str, str]:
    """Return (dataset, access_pattern) for a tile-read benchmark entry."""
    name = entry.get("name", "")
    dataset = _strip_access_suffixes(_extract_dataset_label(name))
    pattern = _extract_access_pattern(name) or ""
    return (dataset, pattern)


def generate_comparison_table(groups: dict[str, list[dict]]) -> str | None:
    """Build a side-by-side comparison of mean times across read groups.

    Returns a Markdown table string, or ``None`` if fewer than two read
    groups are present.
    """
    present = [g for g in _READ_GROUPS if g in groups]
    if len(present) < 2:
        return None

    # Collect mean times keyed by (dataset, pattern) per group
    means: dict[str, dict[tuple[str, str], float]] = {}
    for group_name in present:
        means[group_name] = {}
        for entry in groups[group_name]:
            key = _comparison_key(entry)
            means[group_name][key] = entry.get("stats", {}).get("mean", 0)

    # Union of all keys, sorted by native mean (or first available)
    all_keys = sorted(
        {k for m in means.values() for k in m},
        key=lambda k: means[present[0]].get(k, float("inf")),
    )

    if not all_keys:
        return None

    # Build header
    col_headers = " | ".join(_GROUP_LABELS[g] for g in present)
    header = f"| Dataset | Access Pattern | {col_headers} |"
    separator = "| --- | --- |" + " --- |" * len(present)

    rows = [header, separator]
    for dataset, pattern in all_keys:
        values = []
        for g in present:
            val = means[g].get((dataset, pattern))
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
        lines.append(f"### {group_name.replace('_', ' ').title()}")
        lines.append("")
        lines.append(generate_table(entries, group_name=group_name))
        lines.append("")

    return "\n".join(lines)


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
