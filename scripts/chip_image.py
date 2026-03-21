#!/usr/bin/env python3
"""Extract a chip (region) from an image and save it as PNG.

This script demonstrates using the IO DatasetReader to extract a rectangular
region from a source image and save it using PIL/Pillow. Supports any format
the IO library can read, including NITF (.ntf) and TIFF/GeoTIFF (.tif, .tiff).

Usage:
    python scripts/chip_image.py input.ntf output.png --bbox 0 0 512 512
    python scripts/chip_image.py input.tif output.png --bbox 0 0 512 512
    python scripts/chip_image.py input.ntf output.png --bbox 100 200 300 400 --asset image_segment_0

The bounding box is specified as: x_min y_min x_max y_max (column/row coordinates)
"""

import argparse
import sys
from pathlib import Path

import numpy as np

try:
    from PIL import Image
except ImportError:
    print("Error: Pillow is required for PNG output.", file=sys.stderr)
    print("Install with: pip install Pillow", file=sys.stderr)
    sys.exit(1)

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetType  # noqa: E402


def extract_region(
    image_asset,
    x_min: int,
    y_min: int,
    x_max: int,
    y_max: int,
) -> np.ndarray:
    """Extract a rectangular region from an image asset.

    Args:
        image_asset: ImageAssetProvider to read from
        x_min: Left column (inclusive)
        y_min: Top row (inclusive)
        x_max: Right column (exclusive)
        y_max: Bottom row (exclusive)

    Returns:
        NumPy array with shape (bands, height, width)
    """
    # Get image and block dimensions
    img_width = image_asset.num_columns
    img_height = image_asset.num_rows
    block_width = image_asset.num_pixels_per_block_horizontal
    block_height = image_asset.num_pixels_per_block_vertical
    num_bands = image_asset.num_bands

    # Handle non-blocked images (block size 0 means single block = full image)
    if block_width == 0:
        block_width = img_width
    if block_height == 0:
        block_height = img_height

    # Clamp bounds to image dimensions
    x_min = max(0, x_min)
    y_min = max(0, y_min)
    x_max = min(img_width, x_max)
    y_max = min(img_height, y_max)

    chip_width = x_max - x_min
    chip_height = y_max - y_min

    if chip_width <= 0 or chip_height <= 0:
        raise ValueError(f"Invalid bounding box: ({x_min}, {y_min}) to ({x_max}, {y_max})")

    # Determine which blocks we need
    block_col_start = x_min // block_width
    block_col_end = (x_max - 1) // block_width + 1
    block_row_start = y_min // block_height
    block_row_end = (y_max - 1) // block_height + 1

    # Allocate output array - convert PixelType enum to numpy dtype
    dtype = np.dtype(image_asset.pixel_value_type.to_numpy_dtype())
    chip = np.zeros((num_bands, chip_height, chip_width), dtype=dtype)

    # Read and assemble blocks
    for block_row in range(block_row_start, block_row_end):
        for block_col in range(block_col_start, block_col_end):
            # Check if block exists (for masked images)
            if not image_asset.has_block(block_row, block_col, resolution_level=0):
                continue

            # Get block data (bands, rows, cols)
            block = image_asset.get_block(block_row, block_col, resolution_level=0)

            # Calculate block's pixel coordinates in image space
            block_x_start = block_col * block_width
            block_y_start = block_row * block_height

            # Calculate overlap with chip region
            src_x_start = max(0, x_min - block_x_start)
            src_y_start = max(0, y_min - block_y_start)
            src_x_end = min(block.shape[2], x_max - block_x_start)
            src_y_end = min(block.shape[1], y_max - block_y_start)

            # Calculate destination in chip
            dst_x_start = max(0, block_x_start - x_min)
            dst_y_start = max(0, block_y_start - y_min)
            dst_x_end = dst_x_start + (src_x_end - src_x_start)
            dst_y_end = dst_y_start + (src_y_end - src_y_start)

            # Copy the overlapping region
            chip[:, dst_y_start:dst_y_end, dst_x_start:dst_x_end] = \
                block[:, src_y_start:src_y_end, src_x_start:src_x_end]

    return chip


def save_as_png(chip: np.ndarray, output_path: Path) -> None:
    """Save a chip as PNG using PIL.

    Args:
        chip: NumPy array with shape (bands, height, width)
        output_path: Path to save the PNG file

    Raises:
        ValueError: If the image cannot be saved as PNG (e.g., too many bands)
    """
    num_bands = chip.shape[0]

    # Convert from band-sequential (CHW) to channels-last (HWC) for PIL
    if num_bands == 1:
        # Grayscale - squeeze to 2D
        image_hwc = chip[0]
        mode = "L"
    elif num_bands == 3:
        # RGB
        image_hwc = np.transpose(chip, (1, 2, 0))
        mode = "RGB"
    elif num_bands == 4:
        # RGBA
        image_hwc = np.transpose(chip, (1, 2, 0))
        mode = "RGBA"
    else:
        raise ValueError(
            f"Cannot save {num_bands}-band image as PNG. "
            f"PNG supports 1 (grayscale), 3 (RGB), or 4 (RGBA) bands."
        )

    # Handle different bit depths
    dtype = chip.dtype
    if dtype == np.uint16:
        if mode == "L":
            # For 16-bit grayscale, use PIL's native handling without mode parameter
            image_hwc = np.ascontiguousarray(image_hwc)
            pil_image = Image.fromarray(image_hwc)
            pil_image.save(output_path, "PNG")
            return
        else:
            # Scale 16-bit to 8-bit for RGB/RGBA
            image_hwc = (image_hwc / 256).astype(np.uint8)
    elif dtype not in (np.uint8, np.uint16):
        # Convert other types to uint8
        if np.issubdtype(dtype, np.floating):
            # Assume 0-1 range for floats
            image_hwc = (np.clip(image_hwc, 0, 1) * 255).astype(np.uint8)
        else:
            # Scale integer types to 0-255
            info = np.iinfo(dtype)
            image_hwc = ((image_hwc - info.min) / (info.max - info.min) * 255).astype(np.uint8)

    # Ensure contiguous array for PIL
    image_hwc = np.ascontiguousarray(image_hwc)

    # Create and save PIL image
    pil_image = Image.fromarray(image_hwc, mode=mode)
    pil_image.save(output_path, "PNG")


def chip_image(
    input_path: str,
    output_path: str,
    bbox: tuple[int, int, int, int],
    asset_key: str | None = None,
) -> int:
    """Extract a chip from an image and save as PNG.

    Args:
        input_path: Path to the input image file
        output_path: Path for the output PNG file
        bbox: Bounding box as (x_min, y_min, x_max, y_max)
        asset_key: Optional asset key (uses first image if not specified)

    Returns:
        0 on success, 1 on error
    """
    input_file = Path(input_path)
    output_file = Path(output_path)

    if not input_file.exists():
        print(f"Error: Input file not found: {input_path}", file=sys.stderr)
        return 1

    x_min, y_min, x_max, y_max = bbox

    try:
        with IO.open([str(input_file)], "r") as reader:
            # Find the image asset
            if asset_key:
                if not reader.has_asset(asset_key):
                    print(f"Error: Asset '{asset_key}' not found", file=sys.stderr)
                    return 1
                image_asset = reader.get_asset(asset_key)
            else:
                # Use first image asset
                image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
                if not image_keys:
                    print("Error: No image assets found in file", file=sys.stderr)
                    return 1
                asset_key = image_keys[0]
                image_asset = reader.get_asset(asset_key)

            # Print info about the source
            print(f"Source: {input_file}")
            print(f"Asset: {asset_key}")
            print(f"Image size: {image_asset.num_columns} x {image_asset.num_rows}")
            print(f"Bands: {image_asset.num_bands}")
            print(f"Pixel type: {image_asset.pixel_value_type}")
            print(f"Extracting region: ({x_min}, {y_min}) to ({x_max}, {y_max})")

            # Extract the chip
            chip = extract_region(image_asset, x_min, y_min, x_max, y_max)
            print(f"Chip size: {chip.shape[2]} x {chip.shape[1]} x {chip.shape[0]} bands")

            # Save as PNG
            save_as_png(chip, output_file)
            print(f"Saved: {output_file}")

        return 0

    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Error processing image: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1


def main():
    parser = argparse.ArgumentParser(
        description="Extract a chip from an image and save as PNG.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Extract a 512x512 chip from the top-left corner
    python scripts/chip_image.py input.ntf output.png --bbox 0 0 512 512

    # Extract a chip from a GeoTIFF file
    python scripts/chip_image.py input.tif output.png --bbox 0 0 512 512

    # Extract a region from a specific asset
    python scripts/chip_image.py input.ntf output.png --bbox 100 200 400 500 --asset image_segment_1
"""
    )
    parser.add_argument(
        "input",
        help="Path to the input image file (NITF, TIFF/GeoTIFF)"
    )
    parser.add_argument(
        "output",
        help="Path for the output PNG file"
    )
    parser.add_argument(
        "--bbox", "-b",
        type=int,
        nargs=4,
        required=True,
        metavar=("X_MIN", "Y_MIN", "X_MAX", "Y_MAX"),
        help="Bounding box: x_min y_min x_max y_max (column/row coordinates)"
    )
    parser.add_argument(
        "--asset", "-a",
        dest="asset_key",
        help="Asset key to extract from (uses first image if not specified)"
    )

    args = parser.parse_args()
    return chip_image(
        args.input,
        args.output,
        tuple(args.bbox),
        args.asset_key,
    )


if __name__ == "__main__":
    sys.exit(main())
