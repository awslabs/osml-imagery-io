#!/usr/bin/env python3
"""Time tile reads from an S3-hosted image via the zarr interface.

Reads a rectangular region from a zarr-backed image using a Kerchunk tile
index (JSON or Parquet) and prints elapsed time, shape, and dtype.

Usage:
    python scripts/test_zarr_read.py <tile_index_uri> <x> <y> <width> <height>

Examples:
    # Read a 1024x1024 region starting at pixel (0, 0)
    python scripts/test_zarr_read.py \
        s3://bucket/image.tile_index.json 0 0 1024 1024

    # Read a 2048x2048 region starting at pixel (4096, 2048)
    python scripts/test_zarr_read.py \
        s3://bucket/image.tile_index.parquet 4096 2048 2048 2048

    # Specify a segment name (default: first segment)
    python scripts/test_zarr_read.py \
        s3://bucket/image.tile_index.json 0 0 512 512 \
        --segment image:0
"""

import argparse
import sys
import time
from pathlib import Path

# Add project root to path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

# Register custom codecs with numcodecs before zarr opens the store
import aws.osml.io.zarr_codecs  # noqa: F401, E402


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Time tile reads from an S3-hosted image via the zarr interface.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("tile_index", help="URI to the Kerchunk tile index (JSON or Parquet)")
    parser.add_argument("x", type=int, help="Column offset (pixels)")
    parser.add_argument("y", type=int, help="Row offset (pixels)")
    parser.add_argument("width", type=int, help="Region width (pixels)")
    parser.add_argument("height", type=int, help="Region height (pixels)")
    parser.add_argument("--segment", default=None, help="Image segment key (default: first segment)")
    parser.add_argument(
        "-o", "--output",
        default=None,
        help="Save the region as a PNG file (supports 8-bit and 16-bit)",
    )

    args = parser.parse_args()

    import numpy as np
    import zarr
    from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
    from zarr.storage._fsspec import FsspecStore

    print(f"Tile index: {args.tile_index}")
    print(f"Region:     x={args.x}, y={args.y}, w={args.width}, h={args.height}")
    print()

    t_open = time.perf_counter()

    # Create MultiReferenceFileSystem with asynchronous=True so it matches
    # zarr v3's async FsspecStore internals. The target filesystem (S3)
    # must also be async to avoid event-loop conflicts.
    # MultiReferenceFileSystem extends ReferenceFileSystem with support for
    # multi-range entries ["url", [[offset, length], ...]] needed for
    # JPEG 2000 codestreams with interleaved tile-parts.
    fs = MultiReferenceFileSystem(
        fo=args.tile_index,
        asynchronous=True,
        remote_options={"asynchronous": True},
        skip_instance_cache=True,
    )
    store = FsspecStore(fs=fs, read_only=True, path="")
    root = zarr.open_group(store, mode="r", zarr_format=2)
    elapsed_open = (time.perf_counter() - t_open) * 1000

    keys = list(root.array_keys())
    if not keys:
        print("Error: no arrays found in tile index", file=sys.stderr)
        return 1

    segment = args.segment or keys[0]
    if segment not in keys:
        print(f"Error: segment '{segment}' not found. Available: {', '.join(keys)}", file=sys.stderr)
        return 1

    arr = root[segment]
    print(f"Segment:    {segment}")
    print(f"Image:      {arr.shape[2]}x{arr.shape[1]}, {arr.shape[0]} band(s), dtype={arr.dtype}")
    print(f"Chunks:     {arr.chunks}")
    print(f"Open time:  {elapsed_open:.1f} ms")
    print()

    # Validate bounds
    x, y, w, h = args.x, args.y, args.width, args.height
    _, img_h, img_w = arr.shape
    if x < 0 or y < 0 or x + w > img_w or y + h > img_h:
        print(f"Error: region [{x}:{x+w}, {y}:{y+h}] exceeds image bounds [0:{img_w}, 0:{img_h}]",
              file=sys.stderr)
        return 1

    # Read the region and time it
    t_read = time.perf_counter()
    region = np.asarray(arr[:, y:y + h, x:x + w])
    elapsed_read = (time.perf_counter() - t_read) * 1000

    print(f"Read time:  {elapsed_read:.1f} ms")
    print(f"Shape:      {region.shape}")
    print(f"Dtype:      {region.dtype}")
    print(f"Min/Max:    {region.min()} / {region.max()}")

    if args.output:
        from PIL import Image

        # region shape is (bands, h, w) — squeeze single-band to (h, w)
        if region.shape[0] == 1:
            img_data = region[0]
        elif region.shape[0] == 3:
            # RGB: transpose to (h, w, 3)
            img_data = np.transpose(region, (1, 2, 0))
        else:
            # Multi-band: just take first band
            img_data = region[0]
            print(f"Note: {region.shape[0]} bands — saving band 0 only")

        img = Image.fromarray(img_data)
        img.save(args.output)
        print(f"Saved:      {args.output}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
