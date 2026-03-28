#!/usr/bin/env python3
"""Generate a Kerchunk v1 tile index from a local imagery file.

This script creates a tile index that maps image tile coordinates to byte
ranges in the source file. The index can be saved as JSON or Parquet and
is compatible with fsspec's ReferenceFileSystem for cloud-native access.

Usage:
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf -o index.parquet
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf --segments image_segment_0 image_segment_2
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf --list-segments
"""

import argparse
import json
import sys
import time
from pathlib import Path

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType  # noqa: E402
from aws.osml.io.tile_index import TileIndex  # noqa: E402


def list_segments(path: str) -> int:
    """Print available image segment keys for a dataset file."""
    try:
        with IO.open([path], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
    except Exception as e:
        print(f"Error opening {path}: {e}", file=sys.stderr)
        return 1

    if not keys:
        print(f"No image segments found in {path}")
        return 0

    print(f"Image segments in {path}:")
    for key in keys:
        asset = None
        try:
            with IO.open([path], "r") as reader:
                asset = reader.get_asset(key)
                dims = f"{asset.num_columns}x{asset.num_rows}, {asset.num_bands} band(s)"
                grid = asset.block_grid_size
                tiles = grid[0] * grid[1]
                print(f"  {key}  ({dims}, {tiles} tiles)")
        except Exception:
            print(f"  {key}")

    return 0


def generate_index(
    path: str,
    source_uri: str,
    output: str,
    segments: list[str] | None,
    pretty: bool,
) -> int:
    """Generate a tile index and save it to disk."""
    print(f"Source file:  {path}")
    print(f"Source URI:   {source_uri}")
    if segments:
        print(f"Segments:     {', '.join(segments)}")
    else:
        print("Segments:     all")
    print(f"Output:       {output}")
    print()

    t0 = time.perf_counter()
    try:
        idx = TileIndex.generate(path, source_uri=source_uri, segments=segments)
    except FileNotFoundError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    except KeyError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    elapsed_gen = time.perf_counter() - t0

    print(f"Generated index: {idx.num_segments} segment(s), {idx.num_tiles} tile(s) ({elapsed_gen:.3f}s)")

    # Pretty-print JSON if requested (write manually instead of save())
    t1 = time.perf_counter()
    if pretty and output.endswith(".json"):
        with open(output, "w") as f:
            json.dump(idx.refs, f, indent=2)
    else:
        try:
            idx.save(output)
        except (ValueError, ImportError) as e:
            print(f"Error: {e}", file=sys.stderr)
            return 1
    elapsed_save = time.perf_counter() - t1

    size = Path(output).stat().st_size
    print(f"Saved {output} ({_human_size(size)}, {elapsed_save:.3f}s)")
    return 0


def _human_size(nbytes: int) -> str:
    """Format a byte count as a human-readable string."""
    for unit in ("B", "KB", "MB", "GB"):
        if nbytes < 1024:
            return f"{nbytes:.1f} {unit}" if unit != "B" else f"{nbytes} {unit}"
        nbytes /= 1024
    return f"{nbytes:.1f} TB"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate a Kerchunk v1 tile index from a local imagery file.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Generate a JSON tile index for a NITF file
    python scripts/generate_tile_index.py image.ntf \\
        --source-uri s3://my-bucket/image.ntf

    # Generate a Parquet tile index
    python scripts/generate_tile_index.py image.ntf \\
        --source-uri s3://my-bucket/image.ntf -o index.parquet

    # Index only specific segments
    python scripts/generate_tile_index.py multi_segment.ntf \\
        --source-uri s3://my-bucket/multi_segment.ntf \\
        --segments image_segment_0 image_segment_2

    # List available segments without generating an index
    python scripts/generate_tile_index.py image.ntf --list-segments

    # Pretty-print the JSON output
    python scripts/generate_tile_index.py image.ntf \\
        --source-uri s3://my-bucket/image.ntf --pretty
""",
    )
    parser.add_argument(
        "path",
        help="Path to the local imagery file (NITF, TIFF, J2K, JPEG, PNG)",
    )
    parser.add_argument(
        "--source-uri",
        help="Cloud URI to embed in tile references (e.g. s3://bucket/image.ntf). "
        "Required unless --list-segments is used.",
    )
    parser.add_argument(
        "-o",
        "--output",
        help="Output file path. Extension determines format: .json or .parquet "
        "(default: <input_stem>.tile_index.json)",
    )
    parser.add_argument(
        "--segments",
        nargs="+",
        metavar="KEY",
        help="Image segment keys to index (default: all segments). "
        "Use --list-segments to see available keys.",
    )
    parser.add_argument(
        "--list-segments",
        action="store_true",
        help="List available image segments and exit without generating an index.",
    )
    parser.add_argument(
        "--pretty",
        action="store_true",
        help="Pretty-print JSON output with indentation (JSON only).",
    )

    args = parser.parse_args()

    if args.list_segments:
        return list_segments(args.path)

    if not args.source_uri:
        parser.error("--source-uri is required when generating a tile index")

    output = args.output
    if output is None:
        stem = Path(args.path).stem
        output = f"{stem}.tile_index.json"

    return generate_index(args.path, args.source_uri, output, args.segments, args.pretty)


if __name__ == "__main__":
    sys.exit(main())
