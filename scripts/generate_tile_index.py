#!/usr/bin/env python3
"""Generate a Kerchunk tile index from a local imagery file.

This script creates a tile index that maps image tile coordinates to byte
ranges in the source file. The index can be saved as JSON or Parquet and
is compatible with fsspec's ReferenceFileSystem for cloud-native access.

Usage:
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf -o index.parquet
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf --list-segments
"""

import argparse
import sys
import time
from pathlib import Path

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType  # noqa: E402


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


def _patch_multi_range_refs(refs: dict, multi_range_refs: dict) -> dict:
    """Replace placeholder single-range entries with multi-range entries.

    For each key in *multi_range_refs*, the corresponding entry in *refs*
    is replaced with the multi-range form ``["url", [[offset, length], ...]]``.
    Single-range entries not in *multi_range_refs* are left unchanged.

    .. deprecated::
        Use :func:`aws.osml.io.virtualizarr_parsers._patch_multi_range_refs`
        or :func:`aws.osml.io.virtualizarr_parsers.write_tile_index` instead.
    """
    from aws.osml.io.virtualizarr_parsers import (
        _patch_multi_range_refs as _patch,
    )

    return _patch(refs, multi_range_refs)


def _write_json(vds, output: str, multi_range_refs: dict) -> None:
    """Write kerchunk refs as JSON, patching in multi-range entries."""
    import json

    from virtualizarr.accessor import dataset_to_kerchunk_refs

    kerchunk = dataset_to_kerchunk_refs(vds)
    if "refs" in kerchunk:
        kerchunk["refs"] = _patch_multi_range_refs(kerchunk["refs"], multi_range_refs)
    else:
        kerchunk = _patch_multi_range_refs(kerchunk, multi_range_refs)

    with open(output, "w") as f:
        json.dump(kerchunk, f)


def _write_parquet(vds, output: str, multi_range_refs: dict | None = None) -> None:
    """Write kerchunk refs as parquet using pyarrow engine.

    Works around a fastparquet + pandas 3.x + numpy 2.x incompatibility
    in the default ``to_kerchunk(format='parquet')`` path.
    """
    import fsspec
    from fsspec.implementations.reference import LazyReferenceMapper
    from virtualizarr.accessor import dataset_to_kerchunk_refs

    refs = dataset_to_kerchunk_refs(vds)
    if "refs" in refs:
        refs = refs["refs"]

    if multi_range_refs:
        refs = _patch_multi_range_refs(refs, multi_range_refs)

    fs, _ = fsspec.core.url_to_fs(output)
    out = LazyReferenceMapper.create(
        record_size=100_000,
        root=output,
        fs=fs,
        engine="pyarrow",
    )
    for k in sorted(refs):
        out[k] = refs[k]
    out.flush()


def generate_index(path: str, source_uri: str, output: str, segments: list[str] | None) -> int:
    """Generate a tile index and save it to disk."""
    from aws.osml.io.virtualizarr_parsers import OversightMLParser

    ext = Path(output).suffix.lower()
    if ext == ".json":
        fmt = "json"
    elif ext == ".parquet":
        fmt = "parquet"
    else:
        print(f"Error: Unsupported output extension '{ext}'. Use .json or .parquet", file=sys.stderr)
        return 1

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
        from virtualizarr.manifests import ManifestGroup, ManifestStore

        parser = OversightMLParser(local_path=path)
        ms = parser(url=source_uri)

        # Capture multi-range refs before any filtering
        multi_range_refs = getattr(ms, "multi_range_refs", {}) or {}

        # Filter segments BEFORE to_virtual_dataset() to avoid dimension
        # conflicts when segments have different image sizes.
        if segments:
            group = ms._group
            available = list(group.arrays.keys())
            missing = [s for s in segments if s not in group.arrays]
            if missing:
                print(f"Error: Segment(s) not found: {', '.join(missing)}", file=sys.stderr)
                print(f"Available: {', '.join(available)}", file=sys.stderr)
                return 1
            filtered_arrays = {k: v for k, v in group.arrays.items() if k in segments}
            attrs = group.metadata.attributes if group.metadata else None
            ms = ManifestStore(
                group=ManifestGroup(arrays=filtered_arrays, attributes=attrs)
            )
            # Filter multi-range refs to only include selected segments
            multi_range_refs = {
                k: v for k, v in multi_range_refs.items()
                if any(k.startswith(seg + "/") for seg in segments)
            }

        vds = ms.to_virtual_dataset()
    except (FileNotFoundError, ValueError) as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    elapsed_gen = time.perf_counter() - t0

    num_segments = len(vds.data_vars)
    num_multi = len(multi_range_refs)
    print(f"Generated index: {num_segments} segment(s) ({elapsed_gen:.3f}s)")
    if num_multi:
        print(f"  {num_multi} multi-range entries (interleaved tile-parts)")

    t1 = time.perf_counter()
    try:
        if fmt == "parquet":
            _write_parquet(vds, output, multi_range_refs)
        else:
            _write_json(vds, output, multi_range_refs)
    except (ImportError, ValueError) as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    elapsed_save = time.perf_counter() - t1

    out_path = Path(output)
    if out_path.is_dir():
        size = sum(f.stat().st_size for f in out_path.rglob("*") if f.is_file())
    else:
        size = out_path.stat().st_size
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
        description="Generate a Kerchunk tile index from a local imagery file.",
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
        "--list-segments",
        action="store_true",
        help="List available image segments and exit without generating an index.",
    )
    parser.add_argument(
        "--segments",
        nargs="+",
        metavar="KEY",
        help="Image segment keys to index (default: all segments). "
        "Use --list-segments to see available keys.",
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

    return generate_index(args.path, args.source_uri, output, args.segments)


if __name__ == "__main__":
    sys.exit(main())
