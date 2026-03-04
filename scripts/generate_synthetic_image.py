#!/usr/bin/env python3
"""Generate synthetic NITF test images with configurable parameters.

This script creates tiled NITF images with checkerboard patterns and tile IDs
for visual verification of pixel correctness. It uses the existing IO library
Python bindings to create images with configurable dimensions, tile sizes,
band configurations, pixel types, and interleave modes.

Usage:
    python scripts/generate_synthetic_image.py output.ntf
    python scripts/generate_synthetic_image.py output.ntf --width 1024 --height 1024
    python scripts/generate_synthetic_image.py output.ntf --bands 3 --pixel-type uint16
    python scripts/generate_synthetic_image.py output.ntf --tile-width 128 --tile-height 128

Examples:
    # Generate a default 512x512 grayscale image
    python scripts/generate_synthetic_image.py test.ntf

    # Generate a 1024x1024 RGB image with 128x128 tiles
    python scripts/generate_synthetic_image.py rgb_test.ntf --width 1024 --height 1024 \\
        --bands 3 --tile-width 128 --tile-height 128

    # Generate a 16-bit image with 11-bit actual precision
    python scripts/generate_synthetic_image.py hdr_test.ntf --pixel-type uint16 --abpp 11
"""

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import numpy as np

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))


@dataclass
class ImageConfig:
    """Configuration for synthetic image generation.
    
    Attributes:
        output_path: Path to the output NITF file
        width: Image width in pixels (default: 512)
        height: Image height in pixels (default: 512)
        tile_width: Tile width in pixels (default: 256)
        tile_height: Tile height in pixels (default: 256)
        num_bands: Number of bands - 1 (grayscale), 3 (RGB), or 5 (multispectral)
        pixel_type: Pixel data type - "uint8" or "uint16"
        abpp: Actual bits per pixel (defaults to full range for pixel_type)
        imode: Interleave mode - "B", "P", "R", or "S"
        compression: Compression type - "NC" (none) or "C8" (JPEG2000)
        comrat: Compression ratio for JPEG2000 (e.g., "N001.0" for lossless, "01.0" for 1.0 bpp)
    """
    output_path: str
    width: int = 512
    height: int = 512
    tile_width: int = 256
    tile_height: int = 256
    num_bands: int = 1
    pixel_type: str = "uint8"
    abpp: Optional[int] = None
    imode: str = "B"
    compression: str = "NC"
    comrat: Optional[str] = None
    
    def __post_init__(self) -> None:
        """Validate configuration and set defaults."""
        # Validate dimensions
        if self.width < 1:
            raise ValueError(f"Width must be at least 1, got {self.width}")
        if self.height < 1:
            raise ValueError(f"Height must be at least 1, got {self.height}")
        
        # Validate tile size
        if self.tile_width < 16:
            raise ValueError(f"Tile width must be at least 16, got {self.tile_width}")
        if self.tile_height < 16:
            raise ValueError(f"Tile height must be at least 16, got {self.tile_height}")
        if self.tile_width > 2048:
            raise ValueError(f"Tile width must not exceed 2048, got {self.tile_width}")
        if self.tile_height > 2048:
            raise ValueError(f"Tile height must not exceed 2048, got {self.tile_height}")
        
        # Validate band count
        if self.num_bands not in (1, 3, 5):
            raise ValueError(f"Number of bands must be 1, 3, or 5, got {self.num_bands}")
        
        # Validate pixel type
        if self.pixel_type not in ("uint8", "uint16"):
            raise ValueError(f"Pixel type must be 'uint8' or 'uint16', got '{self.pixel_type}'")
        
        # Validate IMODE
        if self.imode not in ("B", "P", "R", "S"):
            raise ValueError(f"IMODE must be 'B', 'P', 'R', or 'S', got '{self.imode}'")
        
        # Validate compression
        if self.compression not in ("NC", "C8"):
            raise ValueError(f"Compression must be 'NC' or 'C8', got '{self.compression}'")
        
        # Set ABPP to full bit depth if not specified
        max_bits = 8 if self.pixel_type == "uint8" else 16
        if self.abpp is None:
            self.abpp = max_bits
        
        # Validate ABPP is within range for pixel type
        if self.abpp < 1 or self.abpp > max_bits:
            raise ValueError(
                f"ABPP {self.abpp} must be between 1 and {max_bits} for {self.pixel_type}"
            )
    
    @property
    def numpy_dtype(self) -> np.dtype:
        """Return the NumPy dtype for this pixel type."""
        return np.uint8 if self.pixel_type == "uint8" else np.uint16
    
    @property
    def max_pixel_value(self) -> int:
        """Return the maximum pixel value based on ABPP."""
        return (1 << self.abpp) - 1
    
    @property
    def num_tiles_x(self) -> int:
        """Number of tiles in the horizontal direction."""
        return (self.width + self.tile_width - 1) // self.tile_width
    
    @property
    def num_tiles_y(self) -> int:
        """Number of tiles in the vertical direction."""
        return (self.height + self.tile_height - 1) // self.tile_height
    
    @property
    def total_tiles(self) -> int:
        """Total number of tiles in the image."""
        return self.num_tiles_x * self.num_tiles_y


class CheckerboardPattern:
    """Generates checkerboard colors for tiles.
    
    This class provides alternating colors for tiles in a checkerboard pattern,
    where adjacent tiles (horizontally or vertically) have different colors.
    Colors are scaled appropriately based on the configured bit depth.
    """
    
    # Base colors for checkerboard (scaled to 8-bit)
    # Using distinct colors that work for grayscale and RGB
    LIGHT_COLOR = (200, 180, 160)  # Light tan
    DARK_COLOR = (80, 100, 120)    # Dark blue-gray
    
    @staticmethod
    def get_tile_color(
        tile_x: int, 
        tile_y: int, 
        band: int,
        config: ImageConfig
    ) -> int:
        """Get the pixel value for a tile's background color.
        
        Generates alternating colors in a checkerboard pattern based on tile
        position. The color is scaled to the configured bit depth (ABPP).
        
        Args:
            tile_x: Tile column index (0-based)
            tile_y: Tile row index (0-based)
            band: Band index (0-based)
            config: Image configuration containing num_bands and max_pixel_value
            
        Returns:
            Pixel value scaled to the configured bit depth [0, max_pixel_value]
        """
        # Determine if this tile is light or dark based on checkerboard pattern
        is_light = (tile_x + tile_y) % 2 == 0
        
        if config.num_bands == 1:
            # Grayscale: use luminance values
            base = 200 if is_light else 80
        else:
            # Multi-band: use RGB components
            colors = CheckerboardPattern.LIGHT_COLOR if is_light else CheckerboardPattern.DARK_COLOR
            # Use the RGB component for bands 0-2, repeat band 0 for bands 3+
            base = colors[band] if band < 3 else colors[0]
        
        # Scale to configured bit depth
        # base is in 8-bit range [0, 255], scale to [0, max_pixel_value]
        scale = config.max_pixel_value / 255
        return int(base * scale)


class TileIDRenderer:
    """Renders tile ID numbers using bitmap digits.
    
    This class provides functionality to render numeric tile IDs in the center
    of tiles using a simple 5x7 bitmap font. The font is embedded directly in
    the class to avoid external dependencies.
    
    The renderer handles:
    - Multi-digit numbers (any positive integer)
    - Centered positioning within the tile
    - Tiles that are too small to render text (gracefully skips)
    - Contrasting text color relative to background
    """
    
    # 5x7 bitmap font for digits 0-9
    # Each digit is a list of 7 rows, each row is 5 bits (MSB = leftmost pixel)
    DIGIT_PATTERNS = {
        '0': [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1': [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2': [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3': [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4': [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5': [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6': [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7': [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8': [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9': [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
    }
    
    DIGIT_WIDTH = 5
    DIGIT_HEIGHT = 7
    DIGIT_SPACING = 1
    
    # Minimum padding around text (2 pixels on each side)
    MIN_PADDING = 2
    
    @classmethod
    def render_id(
        cls,
        tile_array: np.ndarray,
        tile_id: int,
        fg_color: int,
        config: ImageConfig
    ) -> None:
        """Render a tile ID in the center of the tile array.
        
        Renders the numeric tile ID using a bitmap font. The text is centered
        within the tile. If the tile is too small to fit the text with minimum
        padding, the rendering is skipped.
        
        Args:
            tile_array: NumPy array of shape (height, width, bands) to render into
            tile_id: The tile ID number to render (non-negative integer)
            fg_color: Foreground color value for the text
            config: Image configuration (used for reference, not modified)
            
        Note:
            This method modifies tile_array in place.
        """
        tile_h, tile_w, num_bands = tile_array.shape
        
        # Convert ID to string
        id_str = str(tile_id)
        
        # Calculate total width of rendered text
        text_width = len(id_str) * cls.DIGIT_WIDTH + (len(id_str) - 1) * cls.DIGIT_SPACING
        text_height = cls.DIGIT_HEIGHT
        
        # Calculate minimum tile size needed (text + padding on each side)
        min_width = text_width + 2 * cls.MIN_PADDING
        min_height = text_height + 2 * cls.MIN_PADDING
        
        # Check if tile is large enough
        if tile_w < min_width or tile_h < min_height:
            return  # Tile too small for text
        
        # Calculate starting position (centered)
        start_x = (tile_w - text_width) // 2
        start_y = (tile_h - text_height) // 2
        
        # Render each digit
        x_offset = start_x
        for digit_char in id_str:
            pattern = cls.DIGIT_PATTERNS.get(digit_char, cls.DIGIT_PATTERNS['0'])
            
            for row_idx, row_bits in enumerate(pattern):
                for col_idx in range(cls.DIGIT_WIDTH):
                    # Check if this pixel should be set (MSB = leftmost)
                    if row_bits & (1 << (cls.DIGIT_WIDTH - 1 - col_idx)):
                        y = start_y + row_idx
                        x = x_offset + col_idx
                        # Bounds check (should always pass given min size check)
                        if 0 <= y < tile_h and 0 <= x < tile_w:
                            # Set all bands to foreground color
                            tile_array[y, x, :] = fg_color
            
            x_offset += cls.DIGIT_WIDTH + cls.DIGIT_SPACING


class TileGenerator:
    """Generates pixel data for individual tiles.
    
    This class creates tile pixel data with checkerboard backgrounds and
    centered tile IDs. It handles edge tiles that may be smaller than the
    configured tile size when image dimensions are not exact multiples.
    
    The generator:
    - Creates tiles with the correct dimensions (full or partial for edges)
    - Fills tiles with checkerboard pattern colors
    - Renders tile IDs in the center with contrasting text color
    """
    
    @staticmethod
    def generate_tile(
        tile_x: int,
        tile_y: int,
        tile_id: int,
        config: ImageConfig
    ) -> np.ndarray:
        """Generate pixel data for a single tile.
        
        Creates a tile with a solid checkerboard background color and a
        centered tile ID. Edge tiles (at the right or bottom of the image)
        may be smaller than the configured tile size.
        
        Args:
            tile_x: Tile column index (0-based, left-to-right)
            tile_y: Tile row index (0-based, top-to-bottom)
            tile_id: Sequential tile ID for rendering (row-major order)
            config: Image configuration containing dimensions and pixel settings
            
        Returns:
            NumPy array of shape (actual_height, actual_width, num_bands)
            with dtype matching config.numpy_dtype
        """
        # Calculate actual tile dimensions (may be smaller for edge tiles)
        tile_start_x = tile_x * config.tile_width
        tile_start_y = tile_y * config.tile_height
        actual_width = min(config.tile_width, config.width - tile_start_x)
        actual_height = min(config.tile_height, config.height - tile_start_y)
        
        # Create tile array with appropriate dtype
        tile = np.zeros(
            (actual_height, actual_width, config.num_bands),
            dtype=config.numpy_dtype
        )
        
        # Fill with checkerboard background color for each band
        for band in range(config.num_bands):
            bg_color = CheckerboardPattern.get_tile_color(
                tile_x, tile_y, band, config
            )
            tile[:, :, band] = bg_color
        
        # Calculate contrasting foreground color for text
        # Use band 0 luminance to determine if background is light or dark
        bg_luminance = CheckerboardPattern.get_tile_color(tile_x, tile_y, 0, config)
        # Use max value (white) for dark backgrounds, 0 (black) for light backgrounds
        fg_color = config.max_pixel_value if bg_luminance < config.max_pixel_value // 2 else 0
        
        # Render tile ID in center
        TileIDRenderer.render_id(tile, tile_id, fg_color, config)
        
        return tile


class ImageWriter:
    """Writes synthetic images using the IO library.
    
    This class assembles tiles into a full image and writes it to a NITF file
    using the existing IO library Python bindings. The image data is converted
    to band-sequential format before writing.
    
    The writer:
    - Creates a BufferedImageAssetProvider with the correct configuration
    - Generates all tiles and sets them as the full image
    - Uses IO.open() to create a DatasetWriter
    - Writes the file using DatasetWriter.add_image_asset() and close()
    """
    
    @staticmethod
    def write_image(config: ImageConfig) -> None:
        """Generate and write a synthetic image.
        
        Creates a synthetic NITF image with checkerboard pattern and tile IDs.
        The image is written using the IO library's DatasetWriter with a
        BufferedImageAssetProvider that contains the proper image configuration.
        
        Args:
            config: Image configuration containing all parameters
            
        Raises:
            RuntimeError: If the IO library fails to create or write the file
            ImportError: If the IO library is not available
        """
        from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
        
        # Map pixel type string to PixelType enum
        pixel_type = PixelType.UInt8 if config.pixel_type == "uint8" else PixelType.UInt16
        
        # Create metadata provider with encoding hints (uppercase field names match .ksy definitions)
        metadata = BufferedMetadataProvider()
        metadata.set("IMODE", config.imode)
        metadata.set("IC", config.compression)
        
        if config.compression == "C8" and config.comrat:
            metadata.set("COMRAT", config.comrat)
        
        # Create BufferedImageAssetProvider with the correct configuration
        try:
            image_provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=config.width,
                num_rows=config.height,
                num_bands=config.num_bands,
                block_width=config.tile_width,
                block_height=config.tile_height,
                pixel_type=pixel_type,
                actual_bits_per_pixel=config.abpp,
                metadata=metadata,
                title="Synthetic Test Image",
                description=f"{config.width}x{config.height} {config.num_bands}-band "
                           f"{config.pixel_type} IMODE={config.imode}",
            )
        except Exception as e:
            raise RuntimeError(f"Failed to create image provider: {e}")
        
        # Generate all tiles and concatenate into full image
        full_image = ImageWriter._generate_full_image(config)
        
        # Convert to band-sequential format and set on provider
        bsq_image = ImageWriter._to_bsq(full_image, config)
        
        try:
            if config.pixel_type == "uint8":
                image_provider.set_full_image(bsq_image)
            else:
                image_provider.set_full_image_u16(bsq_image)
        except Exception as e:
            raise RuntimeError(f"Failed to set image data: {e}")
        
        # Create writer for NITF format
        try:
            writer = IO.open([config.output_path], "w", "nitf")
        except Exception as e:
            raise RuntimeError(f"Failed to create NITF writer for {config.output_path}: {e}")
        
        try:
            # Add image asset to writer using the BufferedImageAssetProvider
            writer.add_asset(
                key="image_segment_0",
                provider=image_provider,
                title="Synthetic Test Image",
                description="Generated checkerboard test pattern",
                roles=["data"],
            )
            
            # Close to write file
            writer.close()
        except Exception as e:
            raise RuntimeError(f"Failed to write image to {config.output_path}: {e}")
    
    @staticmethod
    def _generate_full_image(config: ImageConfig) -> np.ndarray:
        """Generate the full image by assembling tiles.
        
        Creates all tiles in row-major order and places them in the correct
        positions within the full image array.
        
        Args:
            config: Image configuration
            
        Returns:
            NumPy array of shape (height, width, num_bands) with dtype
            matching config.numpy_dtype
        """
        full_image = np.zeros(
            (config.height, config.width, config.num_bands),
            dtype=config.numpy_dtype
        )
        
        tile_id = 0
        for tile_y in range(config.num_tiles_y):
            for tile_x in range(config.num_tiles_x):
                tile = TileGenerator.generate_tile(tile_x, tile_y, tile_id, config)
                
                # Calculate position in full image
                start_y = tile_y * config.tile_height
                start_x = tile_x * config.tile_width
                end_y = start_y + tile.shape[0]
                end_x = start_x + tile.shape[1]
                
                full_image[start_y:end_y, start_x:end_x, :] = tile
                tile_id += 1
        
        return full_image
    
    @staticmethod
    def _to_bsq(image: np.ndarray, config: ImageConfig) -> np.ndarray:
        """Convert image array to band-sequential format.
        
        The BufferedImageAssetProvider expects band-sequential format
        (bands, rows, cols).
        
        Args:
            image: NumPy array of shape (height, width, bands)
            config: Image configuration (used for reference)
            
        Returns:
            NumPy array in band-sequential format (bands, height, width)
        """
        # Transpose from (height, width, bands) to (bands, height, width)
        return np.ascontiguousarray(np.transpose(image, (2, 0, 1)))


def parse_args(args: Optional[list] = None) -> ImageConfig:
    """Parse command-line arguments and return an ImageConfig.
    
    Args:
        args: Command-line arguments (defaults to sys.argv[1:])
        
    Returns:
        ImageConfig with parsed values
    """
    parser = argparse.ArgumentParser(
        description="Generate synthetic NITF test images with checkerboard patterns.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s output.ntf
      Generate a default 512x512 grayscale image

  %(prog)s output.ntf --width 1024 --height 1024 --bands 3
      Generate a 1024x1024 RGB image

  %(prog)s output.ntf --pixel-type uint16 --abpp 11
      Generate a 16-bit image with 11-bit actual precision
"""
    )
    
    # Positional argument for output path
    parser.add_argument(
        "output",
        help="Output file path (e.g., output.ntf)"
    )
    
    # Image dimension arguments
    parser.add_argument(
        "--width",
        type=int,
        default=512,
        metavar="PIXELS",
        help="Image width in pixels (default: 512)"
    )
    parser.add_argument(
        "--height",
        type=int,
        default=512,
        metavar="PIXELS",
        help="Image height in pixels (default: 512)"
    )
    
    # Tile size arguments
    parser.add_argument(
        "--tile-width",
        type=int,
        default=256,
        metavar="PIXELS",
        help="Tile width in pixels, 16-2048 (default: 256)"
    )
    parser.add_argument(
        "--tile-height",
        type=int,
        default=256,
        metavar="PIXELS",
        help="Tile height in pixels, 16-2048 (default: 256)"
    )
    
    # Band configuration
    parser.add_argument(
        "--bands",
        type=int,
        default=1,
        choices=[1, 3, 5],
        help="Number of bands: 1 (grayscale), 3 (RGB), or 5 (multispectral) (default: 1)"
    )
    
    # Pixel type arguments
    parser.add_argument(
        "--pixel-type",
        type=str,
        default="uint8",
        choices=["uint8", "uint16"],
        help="Pixel data type (default: uint8)"
    )
    parser.add_argument(
        "--abpp",
        type=int,
        default=None,
        metavar="BITS",
        help="Actual bits per pixel (default: 8 for uint8, 16 for uint16)"
    )
    
    # Interleave mode
    parser.add_argument(
        "--imode",
        type=str,
        default="B",
        choices=["B", "P", "R", "S"],
        help="Interleave mode: B (block), P (pixel), R (row), S (sequential) (default: B)"
    )
    
    # Compression options
    parser.add_argument(
        "--compression",
        type=str,
        default="NC",
        choices=["NC", "C8"],
        help="Compression: NC (none) or C8 (JPEG2000) (default: NC)"
    )
    parser.add_argument(
        "--comrat",
        type=str,
        default=None,
        metavar="RATIO",
        help="Compression ratio for JPEG2000: N001.0 (lossless), 01.0 (1.0 bpp), etc."
    )
    
    parsed = parser.parse_args(args)
    
    return ImageConfig(
        output_path=parsed.output,
        width=parsed.width,
        height=parsed.height,
        tile_width=parsed.tile_width,
        tile_height=parsed.tile_height,
        num_bands=parsed.bands,
        pixel_type=parsed.pixel_type,
        abpp=parsed.abpp,
        imode=parsed.imode,
        compression=parsed.compression,
        comrat=parsed.comrat,
    )


def main() -> int:
    """Main entry point for the synthetic image generator.
    
    Returns:
        Exit code: 0 on success, 1 on invalid arguments, 2 on IO error
    """
    try:
        config = parse_args()
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    except SystemExit as e:
        # argparse calls sys.exit on --help or errors
        return e.code if isinstance(e.code, int) else 1
    
    print(f"Generating synthetic image: {config.output_path}")
    print(f"  Dimensions: {config.width} x {config.height} pixels")
    print(f"  Tile size: {config.tile_width} x {config.tile_height} pixels")
    print(f"  Tiles: {config.num_tiles_x} x {config.num_tiles_y} = {config.total_tiles} total")
    print(f"  Bands: {config.num_bands}")
    print(f"  Pixel type: {config.pixel_type} (ABPP: {config.abpp})")
    print(f"  IMODE: {config.imode}")
    
    try:
        ImageWriter.write_image(config)
        print(f"\nSuccessfully generated: {config.output_path}")
        return 0
    except ImportError as e:
        print(f"\nerror: IO library not available: {e}", file=sys.stderr)
        return 2
    except RuntimeError as e:
        print(f"\nerror: {e}", file=sys.stderr)
        return 2
    except Exception as e:
        print(f"\nerror: Unexpected error during image generation: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
