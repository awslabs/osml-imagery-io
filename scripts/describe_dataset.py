#!/usr/bin/env python3
"""Describe a dataset file and its contents.

This script uses the IO/DatasetReader APIs to dump information about
a dataset file, including overall dataset info and each asset.

Usage:
    python scripts/describe_dataset.py <path_to_file>
    python scripts/describe_dataset.py <path_to_file> --metadata
"""

import argparse
import json
import sys
from pathlib import Path

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType


def format_metadata(metadata_dict: dict, indent: int = 4) -> str:
    """Format metadata dictionary for display."""
    if not metadata_dict:
        return " " * indent + "(no metadata)"
    return json.dumps(metadata_dict, indent=indent, default=str)


def describe_image_asset(asset, show_metadata: bool) -> None:
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
        meta = asset.get_metadata()
        meta_dict = meta.as_dict()
        print(format_metadata(meta_dict, indent=6))


def describe_text_asset(asset, show_metadata: bool) -> None:
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
        meta = asset.get_metadata()
        meta_dict = meta.as_dict()
        print(format_metadata(meta_dict, indent=6))


def describe_data_asset(asset, show_metadata: bool) -> None:
    """Print details for a data asset."""
    # Check if this is a typed DataAssetProvider
    if hasattr(asset, 'mime_type'):
        print(f"    MIME type: {asset.mime_type}")
    
    if show_metadata:
        print("    Metadata:")
        meta = asset.get_metadata()
        meta_dict = meta.as_dict()
        print(format_metadata(meta_dict, indent=6))


def describe_generic_asset(asset, show_metadata: bool) -> None:
    """Print details for a generic asset."""
    if show_metadata:
        print("    Metadata:")
        meta = asset.get_metadata()
        meta_dict = meta.as_dict()
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
        with IO.open(str(file_path), "r") as reader:
            # Dataset-level metadata
            if show_metadata:
                print("File Metadata:")
                print("-" * 40)
                file_meta = reader.metadata
                meta_dict = file_meta.as_dict()
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
                    describe_image_asset(asset, show_metadata)
                elif asset.asset_type == AssetType.Text:
                    describe_text_asset(asset, show_metadata)
                elif asset.asset_type == AssetType.Data:
                    describe_data_asset(asset, show_metadata)
                else:
                    describe_generic_asset(asset, show_metadata)
                
                print()
        
        return 0
        
    except Exception as e:
        print(f"Error reading dataset: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1


def main():
    parser = argparse.ArgumentParser(
        description="Describe a dataset file and its contents."
    )
    parser.add_argument(
        "path",
        help="Path to the dataset file"
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
