#!/usr/bin/env python3
"""Extract a chip (region) from an image via a Zarr tile index and save as PNG.

This script demonstrates cloud-native image access using a Kerchunk tile
index. The tile index can reference local files or S3-hosted imagery.

Usage:
    python scripts/chip_image_zarr.py index.json output.png --bbox 0 0 512 512
    python scripts/chip_image_zarr.py s3://bucket/index.parquet output.png --bbox 100 200 400 500
    python scripts/chip_image_zarr.py index.json output.png --bbox 0 0 256 256 --asset image:0
    python scripts/chip_image_zarr.py index.json output.png --bbox 0 0 256 256 --level 1

The bounding box is specified as: x_min y_min x_max y_max (column/row coordinates)
"""

import argparse
import sys
import time
from pathlib import Path

import numpy as np

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

# Register custom codecs before zarr opens the store
import aws.osml.io.zarr_codecs  # noqa: F401, E402


def chip_zarr(
    tile_index: str,
    output_path: str,
    bbox: tuple[int, int, int, int],
    asset: str | None = None,
    level: int | None = None,
    verbose: bool = False,
) -> int:
    """Extract a chip from a Zarr-backed image and save as PNG.

    Args:
        tile_index: URI to the Kerchunk tile index (JSON or Parquet)
        output_path: Path for the output PNG file
        bbox: Bounding box as (x_min, y_min, x_max, y_max)
        asset: Image asset key (uses first if not specified)
        level: Resolution level (0=base, 1=first overview, etc.)
        verbose: Print timing and diagnostic info

    Returns:
        0 on success, 1 on error
    """
    import zarr
    from PIL import Image
    from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
    from zarr.storage._fsspec import FsspecStore

    x_min, y_min, x_max, y_max = bbox

    try:
        t_open = time.perf_counter()
        fs = MultiReferenceFileSystem(
            fo=tile_index,
            asynchronous=True,
            remote_options={"asynchronous": True},
            skip_instance_cache=True,
        )
        store = FsspecStore(fs=fs, read_only=True, path="")
        root = zarr.open_group(store, mode="r", zarr_format=2)
        elapsed_open = (time.perf_counter() - t_open) * 1000

        if verbose:
            print(f"Open time: {elapsed_open:.1f} ms")

        # Resolve the target array path based on level
        # Multiscale indexes use paths like "0/data", "1/data", etc.
        # Single-level indexes use flat array keys like "image:0"
        multiscales = None
        try:
            ms = dict(root.attrs).get("multiscales", {})
            layout = ms.get("layout", [])
            if layout:
                multiscales = layout
        except Exception:
            pass

        if multiscales:
            # Multiscale index — resolve level
            target_level = level if level is not None else 0
            if target_level >= len(multiscales):
                print(f"Error: level {target_level} not found. "
                      f"Available: 0-{len(multiscales) - 1}", file=sys.stderr)
                return 1
            entry = multiscales[target_level]
            array_path = f"{entry['asset']}/data"
            print(f"Source: {tile_index}")
            print(f"Level: {target_level} (asset={entry['asset']}, "
                  f"scale={entry['transform']['scale']})")
        else:
            # Flat index — use segment key directly
            keys = list(root.array_keys())
            if not keys:
                print("Error: no arrays found in tile index", file=sys.stderr)
                return 1
            array_key = asset or keys[0]
            if array_key not in keys:
                print(f"Error: segment '{array_key}' not found. "
                      f"Available: {', '.join(keys)}", file=sys.stderr)
                return 1
            array_path = array_key
            print(f"Source: {tile_index}")
            print(f"Segment: {array_key}")

        arr = root[array_path]
        bands, img_h, img_w = arr.shape

        print(f"Image size: {img_w} x {img_h}, {bands} bands, dtype={arr.dtype}")
        print(f"Chunks: {arr.chunks}")
        print(f"Extracting region: ({x_min}, {y_min}) to ({x_max}, {y_max})")

        # Validate bounds
        x_min = max(0, x_min)
        y_min = max(0, y_min)
        x_max = min(img_w, x_max)
        y_max = min(img_h, y_max)

        if x_max <= x_min or y_max <= y_min:
            print(f"Error: invalid region after clamping to image bounds", file=sys.stderr)
            return 1

        # Read the region
        t_read = time.perf_counter()
        region = np.asarray(arr[:, y_min:y_max, x_min:x_max])
        elapsed_read = (time.perf_counter() - t_read) * 1000

        print(f"Chip size: {region.shape[2]} x {region.shape[1]} x {region.shape[0]} bands")
        if verbose:
            print(f"Read time: {elapsed_read:.1f} ms")
            print(f"Min/Max: {region.min()} / {region.max()}")

        # Convert to PIL-friendly format and save
        if region.shape[0] == 1:
            img_data = region[0]
        elif region.shape[0] in (3, 4):
            img_data = np.transpose(region, (1, 2, 0))
        else:
            img_data = region[0]
            print(f"Note: {region.shape[0]} bands — saving band 0 only")

        Image.fromarray(img_data).save(output_path)
        print(f"Saved: {output_path}")
        return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1


def main():
    parser = argparse.ArgumentParser(
        description="Extract a chip from an image via a Zarr tile index and save as PNG.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Chip from a local tile index
    python scripts/chip_image_zarr.py image.tile_index.json chip.png --bbox 0 0 512 512

    # Chip from an S3-hosted tile index
    python scripts/chip_image_zarr.py s3://bucket/index.parquet chip.png --bbox 0 0 1024 1024

    # Chip from a specific resolution level in a multiscale index
    python scripts/chip_image_zarr.py index.json chip.png --bbox 0 0 256 256 --level 2

    # Chip with timing info
    python scripts/chip_image_zarr.py index.json chip.png --bbox 0 0 512 512 -v
""",
    )
    parser.add_argument("tile_index", help="URI to the Kerchunk tile index (JSON or Parquet)")
    parser.add_argument("output", help="Path for the output PNG file")
    parser.add_argument(
        "--bbox", "-b", type=int, nargs=4, required=True,
        metavar=("X_MIN", "Y_MIN", "X_MAX", "Y_MAX"),
        help="Bounding box: x_min y_min x_max y_max (column/row coordinates)",
    )
    parser.add_argument("--asset", "-a", default=None, help="Image asset key (flat indexes)")
    parser.add_argument("--level", "-l", type=int, default=None, help="Resolution level (multiscale indexes)")
    parser.add_argument("-v", "--verbose", action="store_true", help="Print timing and diagnostic info")

    args = parser.parse_args()
    return chip_zarr(
        args.tile_index, args.output, tuple(args.bbox),
        asset=args.asset, level=args.level, verbose=args.verbose,
    )


if __name__ == "__main__":
    sys.exit(main())
