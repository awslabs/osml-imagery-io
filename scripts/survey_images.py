#!/usr/bin/env python3
"""Survey a directory for image files and summarize key properties in a table.

Scans for NITF (.ntf, .nitf, .nsf), JPEG 2000 (.jp2, .j2k, .j2c),
TIFF/GeoTIFF (.tif, .tiff), and PNG (.png) files, opens each with the IO
reader, and prints a summary table of the first image segment found in each
file.

Usage:
    python scripts/survey_images.py <directory>
    python scripts/survey_images.py <directory> --recursive
"""

import argparse
import sys
from pathlib import Path

project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType  # noqa: E402
from aws.osml.io.tiff.utils import TagNameResolver  # noqa: E402

# File extensions to scan
EXTENSIONS = {".ntf", ".nitf", ".nsf", ".jp2", ".j2k", ".j2c", ".tif", ".tiff", ".png"}

# TIFF compression code to name mapping (common values)
TIFF_COMPRESSION = {
    "1": "None",
    "2": "CCITT RLE",
    "3": "CCITT Fax3",
    "4": "CCITT Fax4",
    "5": "LZW",
    "6": "OldJPEG",
    "7": "JPEG",
    "8": "Deflate",
    "32773": "PackBits",
    "34712": "JPEG2000",
}


def _human_size(nbytes: int) -> str:
    """Return a human-readable file size string."""
    for unit in ("B", "KB", "MB", "GB"):
        if nbytes < 1024:
            return f"{nbytes:.1f} {unit}" if unit != "B" else f"{nbytes} B"
        nbytes /= 1024
    return f"{nbytes:.1f} TB"


def survey_file(filepath: Path) -> dict | None:
    """Extract first image segment info from a file. Returns None if no images."""
    try:
        with IO.open([str(filepath)], "r") as reader:
            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            if not image_keys:
                return None

            asset = reader.get_asset(image_keys[0])
            if not hasattr(asset, "num_columns"):
                return None

            meta = asset.get_metadata().as_dict()
            media = getattr(asset, "media_type", "")

            # Format-specific fields
            if media == "image/tiff":
                resolver = TagNameResolver(meta)
                comp_code = resolver.get("Compression", "1")
                compression = TIFF_COMPRESSION.get(str(comp_code), str(comp_code))
                ic = "N/A"
                imode = "N/A"
            elif media == "image/png":
                compression = "Deflate"
                ic = "N/A"
                imode = "N/A"
            else:
                # NITF
                ic = meta.get("IC", "?")
                compression = {
                    "NC": "None", "NM": "None (masked)",
                    "C1": "BiLevel", "C3": "JPEG",
                    "C4": "VQ", "C5": "Lossless JPEG",
                    "C8": "JPEG2000", "I1": "Downsampled JPEG",
                    "M1": "BiLevel (masked)", "M3": "JPEG (masked)",
                    "M4": "VQ (masked)", "M5": "Lossless JPEG (masked)",
                    "M8": "JPEG2000 (masked)",
                }.get(ic, ic)
                imode = meta.get("IMODE", "?")

            return {
                "filename": filepath.name,
                "disk_size": _human_size(filepath.stat().st_size),
                "pixels": f"{asset.num_columns} x {asset.num_rows}",
                "bands": asset.num_bands,
                "pixel_type": str(asset.pixel_value_type),
                "compression": compression,
                "ic": ic,
                "imode": imode,
            }
    except Exception:
        return {
            "filename": filepath.name,
            "disk_size": "ERROR",
            "pixels": "N/A",
            "bands": "N/A",
            "pixel_type": "N/A",
            "compression": "N/A",
            "ic": "N/A",
            "imode": "N/A",
        }


def print_table(rows: list[dict]) -> None:
    """Print rows as a formatted table."""
    if not rows:
        print("No image files found.")
        return

    headers = {
        "filename": "Filename",
        "disk_size": "Disk Size",
        "pixels": "Pixels (W x H)",
        "bands": "Bands",
        "pixel_type": "Pixel Type",
        "compression": "Compression",
        "ic": "IC",
        "imode": "IMODE",
    }

    # Calculate column widths
    widths = {k: len(v) for k, v in headers.items()}
    for row in rows:
        for k in headers:
            widths[k] = max(widths[k], len(str(row.get(k, ""))))

    # Print header
    header_line = " | ".join(headers[k].ljust(widths[k]) for k in headers)
    sep_line = "-+-".join("-" * widths[k] for k in headers)
    print(header_line)
    print(sep_line)

    # Print rows
    for row in rows:
        line = " | ".join(str(row.get(k, "")).ljust(widths[k]) for k in headers)
        print(line)


def main():
    parser = argparse.ArgumentParser(
        description="Survey a directory for image files and print a summary table."
    )
    parser.add_argument("directory", help="Directory to scan")
    parser.add_argument(
        "-r", "--recursive", action="store_true",
        help="Scan subdirectories recursively"
    )
    args = parser.parse_args()

    scan_dir = Path(args.directory)
    if not scan_dir.is_dir():
        print(f"Error: {args.directory} is not a directory", file=sys.stderr)
        return 1

    # Collect matching files
    if args.recursive:
        files = sorted(f for f in scan_dir.rglob("*") if f.suffix.lower() in EXTENSIONS)
    else:
        files = sorted(f for f in scan_dir.iterdir() if f.suffix.lower() in EXTENSIONS)

    if not files:
        print(f"No image files found in {scan_dir}")
        return 0

    print(f"Scanning {len(files)} file(s) in {scan_dir}...\n")

    rows = []
    skipped = 0
    for f in files:
        result = survey_file(f)
        if result is None:
            skipped += 1
        else:
            rows.append(result)

    print_table(rows)

    if skipped:
        print(f"\n({skipped} file(s) skipped — no image segments)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
