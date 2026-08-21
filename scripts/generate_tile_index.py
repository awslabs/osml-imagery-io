#!/usr/bin/env python3
"""Generate a Kerchunk tile index from a local or remote imagery file.

This script creates a tile index that maps image tile coordinates to byte
ranges in the source file. The index can be saved as JSON or Parquet and
is compatible with fsspec's ReferenceFileSystem for cloud-native access.

The source may be a local file path or a remote URL (e.g.
``s3://bucket/image.ntf``). The index is built with on-demand byte-range reads,
so indexing a remote file does not download the whole object. Chunk references
in the index point at the source you pass here, unless ``--source-uri`` rewrites
them (see below).

``--zarr-format`` selects which Zarr version the index describes. Both are
Kerchunk reference files served through fsspec; the difference is the store keys
inside, and therefore which consumer path reads them — ``2`` (the default) uses
``.zgroup``/``.zarray`` keys read through the numcodecs registry, ``3`` uses
native ``zarr.json`` keys read through zarr's entry-point codec pipeline.

Usage:
    # Index a remote file directly — refs point at the S3 URL.
    python scripts/generate_tile_index.py s3://bucket/image.ntf

    # Index a local copy, but point refs at where the data will be served.
    python scripts/generate_tile_index.py image.ntf --source-uri s3://bucket/image.ntf

    # Native Zarr v3 index (JSON only).
    python scripts/generate_tile_index.py image.ntf --zarr-format 3

    # Parquet output, or list segments without indexing.
    python scripts/generate_tile_index.py s3://bucket/image.ntf -o index.parquet
    python scripts/generate_tile_index.py image.ntf --list-segments

A Parquet index needs pyarrow (``pip install "osml-imagery-io[zarr]"``) and must be
read back with ``MultiReferenceFileSystem``; a stock fsspec ``ReferenceFileSystem``
cannot open one. Parquet is a v2-only container — see ``--zarr-format``.
"""

import argparse
import sys
import time
from pathlib import Path

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType  # noqa: E402

# URL schemes that IO.open resolves to an fsspec filesystem for range reads.
_REMOTE_SCHEMES = ("s3://", "gs://", "gcs://", "az://", "abfs://", "http://", "https://")


def _is_remote_url(path: str) -> bool:
    """Return True if *path* is a remote URL rather than a local file path."""
    return path.startswith(_REMOTE_SCHEMES)


def _open_source(path: str):
    """Open *path* for reading, routing remote URLs through fsspec range reads.

    A bare URL string (``s3://…``) is passed to ``IO.open`` directly so it
    resolves to an fsspec filesystem; a local path is passed as-is.
    """
    return IO.open(path, "r")


def list_segments(path: str) -> int:
    """Print available image segment keys for a dataset file."""
    try:
        with _open_source(path) as reader:
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
            with _open_source(path) as reader:
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
    source_uri: str | None,
    output: str,
    segments: list[str] | None,
    zarr_format: int = 2,
) -> int:
    """Generate a tile index and save it to disk.

    Args:
        path: Local path or remote URL of the imagery to index. The index is
            built by reading this source with byte-range requests.
        source_uri: Optional URL to embed in the chunk references instead of
            *path*. Use this when indexing a local copy of data that will be
            served from a different (e.g. ``s3://``) location. When ``None``,
            references point at *path* itself.
        output: Output index path (``.json`` or ``.parquet``).
        segments: Optional list of image asset keys to include (default: all).
        zarr_format: ``2`` for the Kerchunk/Zarr v2 layout, ``3`` for the native
            Zarr v3 layout. Only the store keys differ; every other stage of
            this script is format-independent.
    """
    from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

    ext = Path(output).suffix.lower()
    if ext not in (".json", ".parquet"):
        print(f"Error: Unsupported output extension '{ext}'. Use .json or .parquet", file=sys.stderr)
        return 1

    # Reject the unsupported combination before parsing, which for a large
    # remote file is minutes of byte-range reads. ``write_tile_index`` raises
    # for it too, but only after the store has been built.
    if zarr_format == 3 and ext == ".parquet":
        print(
            "Error: Parquet output is not supported for --zarr-format 3: the Kerchunk "
            "Parquet container (fsspec's LazyReferenceMapper) indexes chunk references "
            "by position using the v2 '.zarray' shape/chunks, which a native v3 store "
            "does not have. Use a .json output path for v3 indexes, or --zarr-format 2 "
            "for Parquet.",
            file=sys.stderr,
        )
        return 1

    # The VirtualiZarr manifest requires an absolute posix path or a URI for
    # every chunk reference. Remote URLs already qualify; a local path is
    # resolved to an absolute path before indexing.
    if _is_remote_url(path):
        parse_source = path
    else:
        parse_source = str(Path(path).resolve())

    print(f"Source:       {path}")
    if source_uri and source_uri != path:
        print(f"Refs point at: {source_uri}")
    if segments:
        print(f"Segments:     {', '.join(segments)}")
    else:
        print("Segments:     all")
    print(f"Output:       {output}")
    print(f"Zarr format:  {zarr_format}")
    print()

    t0 = time.perf_counter()
    try:
        parser = OversightMLParser()
        store = parser(parse_source)
    except (FileNotFoundError, ValueError) as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    elapsed_gen = time.perf_counter() - t0

    multi_range_refs = getattr(store, "multi_range_refs", {}) or {}
    num_levels = len(store._group.groups)
    num_multi = len(multi_range_refs)
    print(f"Generated index: {num_levels} level(s) ({elapsed_gen:.3f}s)")
    if num_multi:
        print(f"  {num_multi} multi-range entries (interleaved tile-parts)")

    # Relocate chunk references only when --source-uri names a different
    # location than the source that was read. ``write_tile_index`` matches the
    # override key against both the raw and ``file://``-normalized forms, so the
    # resolved absolute path used for parsing is the correct key here.
    url_overrides = None
    if source_uri and source_uri != path:
        url_overrides = {parse_source: source_uri}

    t1 = time.perf_counter()
    try:
        write_tile_index(
            store, output,
            segments=segments,
            url_overrides=url_overrides,
            zarr_format=zarr_format,
        )
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
        description="Generate a Kerchunk tile index from a local or remote imagery file.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Index a remote NITF directly — chunk refs point at the S3 URL
    python scripts/generate_tile_index.py s3://my-bucket/image.ntf

    # Index a local file — chunk refs point at the local path
    python scripts/generate_tile_index.py image.ntf

    # Index a local copy but point refs at the S3 location it will be served from
    python scripts/generate_tile_index.py image.ntf \\
        --source-uri s3://my-bucket/image.ntf

    # Parquet output (v2 only)
    python scripts/generate_tile_index.py s3://my-bucket/image.ntf -o index.parquet

    # Native Zarr v3 index — read with zarr.open/xarray.open_zarr, no numcodecs
    python scripts/generate_tile_index.py image.ntf --zarr-format 3 -o index.v3.json

    # Index only specific segments
    python scripts/generate_tile_index.py s3://my-bucket/multi_segment.ntf \\
        --segments image:0 image:2

    # List available segments without generating an index
    python scripts/generate_tile_index.py image.ntf --list-segments
""",
    )
    parser.add_argument(
        "path",
        help="Path to the imagery file, or a remote URL such as "
        "s3://bucket/image.ntf (NITF, TIFF, J2K, JPEG, PNG)",
    )
    parser.add_argument(
        "--source-uri",
        help="Cloud URI to embed in tile references (e.g. s3://bucket/image.ntf). "
        "Use when indexing a local copy of data that will be served from a "
        "different location. When omitted, references point at the source above.",
    )
    parser.add_argument(
        "-o",
        "--output",
        help="Output file path. Extension determines format: .json or .parquet "
        "(.parquet needs pyarrow and MultiReferenceFileSystem to read back) "
        "(default: <input_stem>.tile_index.json)",
    )
    parser.add_argument(
        "--zarr-format",
        type=int,
        choices=(2, 3),
        default=2,
        help="Zarr version the index describes: 2 (default) emits the Kerchunk "
        ".zgroup/.zarray layout read through the numcodecs registry; 3 emits a "
        "native zarr.json layout read through zarr's entry-point codec pipeline. "
        "Format 3 requires .json output — Parquet is a v2-only container.",
    )
    parser.add_argument(
        "--list-segments",
        action="store_true",
        help="List available image segments and exit without generating an index.",
    )
    parser.add_argument(
        "--segments", "-a",
        nargs="+",
        metavar="KEY",
        help="Image asset keys to index (default: all). "
        "Use --list-segments to see available keys.",
    )

    args = parser.parse_args()

    if args.list_segments:
        return list_segments(args.path)

    output = args.output
    if output is None:
        stem = Path(args.path).stem
        output = f"{stem}.tile_index.json"

    return generate_index(
        args.path, args.source_uri, output, args.segments, args.zarr_format
    )


if __name__ == "__main__":
    sys.exit(main())
