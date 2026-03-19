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


def group_benchmarks(benchmarks: list[dict]) -> dict[str, list[dict]]:
    """Group benchmark entries by their ``group`` field.

    Entries without a group are placed under ``"ungrouped"``.
    """
    groups: dict[str, list[dict]] = {}
    for entry in benchmarks:
        group = entry.get("group") or "ungrouped"
        groups.setdefault(group, []).append(entry)
    return groups


def generate_table(entries: list[dict]) -> str:
    """Generate a MyST-compatible Markdown table for a list of benchmark entries.

    Columns: Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds
    """
    header = "| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |"
    separator = "| --- | --- | --- | --- | --- | --- | --- | --- |"
    rows = [header, separator]

    for entry in entries:
        name = entry.get("name", "")
        stats = entry.get("stats", {})
        operation = _extract_operation(name)
        dataset = _extract_dataset_label(name)
        row = (
            f"| {operation} "
            f"| {dataset} "
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

    for group_name, entries in groups.items():
        lines.append(f"### {group_name.replace('_', ' ').title()}")
        lines.append("")
        lines.append(generate_table(entries))
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
