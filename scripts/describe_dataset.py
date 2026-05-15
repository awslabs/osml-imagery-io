#!/usr/bin/env python3
"""Describe a dataset file and its contents.

This script uses the IO/DatasetReader APIs to dump information about
a dataset file, including overall dataset info and each asset. Supports
NITF (.ntf, .nitf, .nsf), TIFF/GeoTIFF (.tif, .tiff), and PNG (.png) formats.

Usage:
    python scripts/describe_dataset.py image.ntf
    python scripts/describe_dataset.py image.tif --metadata
    python scripts/describe_dataset.py image.png --metadata
"""

import argparse
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType  # noqa: E402
from aws.osml.io.tiff.utils import TagNameResolver  # noqa: E402

# TIFF tags that are internal file-structure lookup tables, not useful for
# human inspection.  These are skipped when formatting TIFF metadata.
_TIFF_SKIP_TAGS = {
    "StripOffsets",       # 273 — byte offsets to each strip
    "StripByteCounts",    # 279 — byte counts for each strip
    "TileOffsets",        # 324 — byte offsets to each tile
    "TileByteCounts",     # 325 — byte counts for each tile
    "FreeOffsets",        # 288 — byte offsets to free space
    "FreeByteCounts",     # 289 — byte counts of free space
    "JPEGTables",         # 347 — JPEG quantization/Huffman tables (binary blob)
}

# NITF/NSIF metadata fields that are internal to file parsing and not useful
# for human inspection.  These fall into two categories:
#
# 1. Segment counts and offset tables — needed to locate segments in the file
#    but redundant once the dataset is opened (the reader already resolved them).
# 2. TRE container fields — raw byte blobs (UDHD, XHD, IXSHD, etc.) whose
#    content is already decomposed into individual TREs in the metadata map.
_NITF_SKIP_FIELDS = {
    # File header: segment counts and offset/length tables
    "NUMI", "NUMS", "NUMX", "NUMT", "NUMDES", "NUMRES",
    "IMAGE_INFO", "GRAPHIC_INFO", "TEXT_INFO", "DES_INFO", "RES_INFO",
    "FL", "HL",
    # File header: TRE container fields
    "UDHDL", "UDHOFL", "UDHD",
    "XHDL", "XHDLOFL", "XHD",
    # Image subheader: TRE container fields
    "UDIDL", "UDOFL", "UDID",
    "IXSHDL", "IXSOFL", "IXSHD",
    # Graphic subheader: TRE container fields
    "SXSHDL", "SXSOFL", "SXSHD",
    # Text subheader: TRE container fields
    "TXSHDL", "TXSOFL", "TXSHD",
    # DES subheader: user-defined subheader container
    "DESSHL", "DESSHF",
}


def _is_tiff(asset) -> bool:
    """Return True if the asset's media type indicates TIFF."""
    return getattr(asset, "media_type", "") == "image/tiff"


def _is_nitf_file(path: Path) -> bool:
    """Return True if the file path looks like a NITF/NSIF file."""
    return path.suffix.lower() in {".ntf", ".nitf", ".nsf"}


def _filter_nitf_metadata(metadata_dict: dict) -> dict:
    """Remove internal NITF/NSIF fields from a metadata dictionary."""
    return {k: v for k, v in metadata_dict.items() if k not in _NITF_SKIP_FIELDS}


def format_metadata(metadata_dict: dict, indent: int = 4) -> str:
    """Format metadata dictionary for display."""
    if not metadata_dict:
        return " " * indent + "(no metadata)"
    return json.dumps(metadata_dict, indent=indent, default=str)


def format_tiff_metadata(metadata_dict: dict, indent: int = 4) -> str:
    """Format TIFF metadata with human-readable tag names, skipping internal tables."""
    if not metadata_dict:
        return " " * indent + "(no metadata)"
    resolver = TagNameResolver(metadata_dict)
    filtered = {
        name: value
        for name, value in resolver
        if name not in _TIFF_SKIP_TAGS
    }
    return json.dumps(filtered, indent=indent, default=str)


def describe_image_asset(asset, show_metadata: bool, is_nitf: bool = False) -> None:
    """Print details for an image asset."""
    # Check if this is a typed ImageAssetProvider with image-specific properties
    if hasattr(asset, 'num_columns'):
        print(f"    Dimensions: {asset.num_columns} x {asset.num_rows} pixels")
        print(f"    Bands: {asset.num_bands}")
        print(f"    Bits per pixel: {asset.num_bits_per_pixel} (actual: {asset.actual_bits_per_pixel})")
        print(f"    Pixel type: {asset.pixel_value_type}")
        print(f"    Block size: {asset.num_pixels_per_block_horizontal} x {asset.num_pixels_per_block_vertical}")
        print(f"    Block grid: {asset.block_grid_size}")
        print(f"    Resolution levels: {asset.num_resolution_levels}")

    if show_metadata:
        print("    Metadata:")
        meta = asset.metadata
        meta_dict = meta.entries()
        if _is_tiff(asset):
            print(format_tiff_metadata(meta_dict, indent=6))
        else:
            if is_nitf:
                meta_dict = _filter_nitf_metadata(meta_dict)
            print(format_metadata(meta_dict, indent=6))


def describe_text_asset(asset, show_metadata: bool, is_nitf: bool = False) -> None:
    """Print details for a text asset."""
    # Check if this is a typed TextAssetProvider
    if hasattr(asset, 'encoding'):
        print(f"    Encoding: {asset.encoding}")
        print(f"    Format: {asset.format}")
        try:
            text = asset.text
            preview = text[:100] + "..." if len(text) > 100 else text
            print(f"    Content preview: {preview!r}")
        except Exception as e:
            print(f"    Content: (error reading: {e})")

    if show_metadata:
        print("    Metadata:")
        meta = asset.metadata
        meta_dict = meta.entries()
        if is_nitf:
            meta_dict = _filter_nitf_metadata(meta_dict)
        print(format_metadata(meta_dict, indent=6))


def format_xml(element: ET.Element, indent: int = 6) -> str:
    """Pretty-print an XML Element with indentation."""
    ET.indent(element)
    xml_str = ET.tostring(element, encoding="unicode")
    prefix = " " * indent
    return "\n".join(prefix + line for line in xml_str.splitlines())


def is_sicd_sidd_segment(asset) -> bool:
    """Check if a data asset is a SICD or SIDD XML metadata segment.

    SICD/SIDD imagery stores XML metadata in DES segments whose DESID
    field contains an identifier like ``XML_DATA_CONTENT``,
    ``SICD_XML``, ``SIDD_XML``, or a ``urn:SICD``/``urn:SIDD`` URN.
    """
    try:
        meta = asset.metadata
        meta_dict = meta.entries()
        desid = (meta_dict.get("DESID") or "").strip()
        sicd_sidd_ids = {"XML_DATA_CONTENT", "SICD_XML", "SIDD_XML"}
        if desid in sicd_sidd_ids or "SICD" in desid or "SIDD" in desid:
            return True
    except Exception:
        # Metadata read may fail for malformed segments; treat as non-SICD/SIDD
        pass
    return False


def describe_data_asset(asset, show_metadata: bool, is_nitf: bool = False) -> None:
    """Print details for a data asset.

    For SICD/SIDD data segments the XML metadata is parsed and
    pretty-printed when ``--metadata`` is requested.
    """
    # Check if this is a typed DataAssetProvider
    if hasattr(asset, 'mime_type'):
        print(f"    MIME type: {asset.mime_type}")

    sicd_sidd = is_sicd_sidd_segment(asset)
    if sicd_sidd:
        print("    Content: SICD/SIDD XML metadata")

    if show_metadata:
        print("    Metadata:")
        meta = asset.metadata
        meta_dict = meta.entries()
        if is_nitf:
            meta_dict = _filter_nitf_metadata(meta_dict)
        print(format_metadata(meta_dict, indent=6))

        # Display the XML content for SICD/SIDD segments
        if sicd_sidd:
            try:
                if hasattr(asset, 'parse_as_xml'):
                    xml_element = asset.parse_as_xml()
                else:
                    # Fall back to parsing raw bytes when the asset is
                    # returned as a generic AssetProvider without the
                    # DataAssetProvider interface.
                    raw_io = asset.raw_asset
                    xml_element = ET.fromstring(raw_io.read())
                print("    XML Content:")
                print(format_xml(xml_element))
            except Exception as e:
                print(f"    XML Content: (error parsing: {e})")


def describe_generic_asset(asset, show_metadata: bool, is_nitf: bool = False) -> None:
    """Print details for a generic asset."""
    if show_metadata:
        print("    Metadata:")
        meta = asset.metadata
        meta_dict = meta.entries()
        if is_nitf:
            meta_dict = _filter_nitf_metadata(meta_dict)
        print(format_metadata(meta_dict, indent=6))


def describe_dataset(path: str, show_metadata: bool) -> int:
    """Describe a dataset file.

    Args:
        path: Path to the dataset file
        show_metadata: Whether to print metadata for file and segments

    Returns:
        0 on success, 1 on error
    """
    file_path = Path(path)

    if not file_path.exists():
        print(f"Error: File not found: {path}", file=sys.stderr)
        return 1

    print(f"Dataset: {file_path}")
    print(f"Size: {file_path.stat().st_size} bytes")
    print()

    try:
        with IO.open([str(file_path)], "r") as reader:
            is_nitf = _is_nitf_file(file_path)

            # Dataset-level metadata
            if show_metadata:
                print("File Metadata:")
                print("-" * 40)
                file_meta = reader.metadata
                meta_dict = file_meta.entries()
                if is_nitf:
                    meta_dict = _filter_nitf_metadata(meta_dict)
                print(format_metadata(meta_dict))
                print()

            # Get all asset keys
            all_keys = reader.get_asset_keys()

            # Group by asset type
            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            text_keys = reader.get_asset_keys(asset_type=AssetType.Text)
            data_keys = reader.get_asset_keys(asset_type=AssetType.Data)
            graphics_keys = reader.get_asset_keys(asset_type=AssetType.Graphics)

            print("Asset Summary:")
            print("-" * 40)
            print(f"  Total assets: {len(all_keys)}")
            print(f"  Images: {len(image_keys)}")
            print(f"  Text: {len(text_keys)}")
            print(f"  Data: {len(data_keys)}")
            print(f"  Graphics: {len(graphics_keys)}")
            print()

            # Describe each asset
            print("Assets:")
            print("-" * 40)

            for key in all_keys:
                asset = reader.get_asset(key)

                print(f"  [{key}]")
                print(f"    Type: {asset.asset_type}")
                print(f"    Title: {asset.title}")
                if asset.description:
                    print(f"    Description: {asset.description}")
                print(f"    Media type: {asset.media_type}")
                print(f"    Roles: {asset.roles}")

                # Type-specific details
                if asset.asset_type == AssetType.Image:
                    describe_image_asset(asset, show_metadata, is_nitf)
                elif asset.asset_type == AssetType.Text:
                    describe_text_asset(asset, show_metadata, is_nitf)
                elif asset.asset_type == AssetType.Data:
                    describe_data_asset(asset, show_metadata, is_nitf)
                else:
                    describe_generic_asset(asset, show_metadata, is_nitf)

                print()

        return 0

    except Exception as e:
        print(f"Error reading dataset: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1


def main():
    parser = argparse.ArgumentParser(
        description="Describe a dataset file and its contents.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Describe a NITF file
    python scripts/describe_dataset.py image.ntf

    # Describe a GeoTIFF file with full metadata
    python scripts/describe_dataset.py image.tif --metadata

    # Describe a PNG file with full metadata
    python scripts/describe_dataset.py image.png --metadata
"""
    )
    parser.add_argument(
        "path",
        help="Path to the dataset file (NITF, TIFF/GeoTIFF, PNG)"
    )
    parser.add_argument(
        "--metadata", "-m",
        action="store_true",
        help="Include metadata for file and each segment"
    )

    args = parser.parse_args()
    return describe_dataset(args.path, args.metadata)


if __name__ == "__main__":
    sys.exit(main())
