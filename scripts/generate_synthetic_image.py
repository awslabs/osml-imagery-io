#!/usr/bin/env python3
"""Generate synthetic test images with configurable parameters.

This script creates tiled images with checkerboard patterns and tile IDs
for visual verification of pixel correctness. It uses the existing IO library
Python bindings to create images with configurable dimensions, tile sizes,
band configurations, pixel types, and interleave modes.

Supported output formats:
- NITF (default) - National Imagery Transmission Format (.ntf)
- TIFF/GeoTIFF - Tagged Image File Format (.tif, .tiff)
- PNG - Portable Network Graphics (.png)
- J2K - JPEG 2000 codestream (.j2k, .jp2)
- JPEG - JPEG/JFIF (.jpg, .jpeg)

The script also supports generating masked images where some blocks are empty,
useful for testing sparse imagery handling (NITF only).

Usage:
    python scripts/generate_synthetic_image.py output.ntf
    python scripts/generate_synthetic_image.py output.tif --format tiff
    python scripts/generate_synthetic_image.py output.png --format png
    python scripts/generate_synthetic_image.py output.j2k --format j2k
    python scripts/generate_synthetic_image.py output.jpg --format jpeg
    python scripts/generate_synthetic_image.py output.ntf --width 1024 --height 1024
    python scripts/generate_synthetic_image.py output.ntf --bands 3 --pixel-type uint16
    python scripts/generate_synthetic_image.py output.ntf --tile-width 128 --tile-height 128
    python scripts/generate_synthetic_image.py output.ntf --masked --mask-pattern checkerboard

Examples:
    # Generate a default 512x512 grayscale NITF image
    python scripts/generate_synthetic_image.py test.ntf

    # Generate a 512x512 grayscale TIFF image
    python scripts/generate_synthetic_image.py test.tif --format tiff

    # Generate a GeoTIFF image (alias for tiff)
    python scripts/generate_synthetic_image.py test.tif --format geotiff

    # Generate a 512x512 grayscale PNG image
    python scripts/generate_synthetic_image.py test.png --format png

    # Generate a 1024x1024 RGB image with 128x128 tiles
    python scripts/generate_synthetic_image.py rgb_test.ntf --width 1024 --height 1024 \\
        --bands 3 --tile-width 128 --tile-height 128

    # Generate a 16-bit image with 11-bit actual precision
    python scripts/generate_synthetic_image.py hdr_test.ntf --pixel-type uint16 --abpp 11

    # Generate a masked image with checkerboard pattern (NITF only)
    python scripts/generate_synthetic_image.py masked_test.ntf --masked --mask-pattern checkerboard

    # Generate a masked image with random pattern (50% blocks masked)
    python scripts/generate_synthetic_image.py masked_random.ntf --masked --mask-pattern random --mask-ratio 0.5

    # Generate a masked J2K image
    python scripts/generate_synthetic_image.py masked_j2k.ntf --masked --compression j2k

    # Generate a NITF with JPEG compression (IC=C3)
    python scripts/generate_synthetic_image.py jpeg_test.ntf --compression jpeg

    # Generate a NITF with JPEG 2000 compression (IC=C8)
    python scripts/generate_synthetic_image.py j2k_test.ntf --compression j2k

    # Generate a TIFF with LZW compression
    python scripts/generate_synthetic_image.py test.tif --format tiff --compression lzw

    # Generate a lossless JPEG 2000 RGB image
    python scripts/generate_synthetic_image.py test.j2k --format j2k --bands 3

    # Generate a JPEG RGB image at quality 90
    python scripts/generate_synthetic_image.py test.jpg --format jpeg --bands 3
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


# Valid compression choices per output format. The keys are the
# format-agnostic compression names accepted by --compression.
_VALID_COMPRESSIONS = {
    "nitf":    ("none", "jpeg", "j2k"),
    "tiff":    ("none", "lzw", "deflate"),
    "geotiff": ("none", "lzw", "deflate"),
    "png":     ("deflate",),
    "j2k":     ("j2k",),
    "jpeg":    ("jpeg",),
}

# Default compression when --compression auto is used.
_DEFAULT_COMPRESSION = {
    "nitf":    "none",
    "tiff":    "none",
    "geotiff": "none",
    "png":     "deflate",
    "j2k":     "j2k",
    "jpeg":    "jpeg",
}


@dataclass
class ImageConfig:
    """Configuration for synthetic image generation.

    Attributes:
        output_path: Path to the output file
        format: Output format - "nitf" (default), "tiff", "geotiff", "png", "j2k", or "jpeg"
        width: Image width in pixels (default: 512)
        height: Image height in pixels (default: 512)
        tile_width: Tile width in pixels (default: 256)
        tile_height: Tile height in pixels (default: 256)
        num_bands: Number of bands - 1 (grayscale), 3 (RGB), 4 (RGBA), or 5 (multispectral)
        pixel_type: Pixel data type - "uint8" or "uint16"
        abpp: Actual bits per pixel (defaults to full range for pixel_type)
        imode: Interleave mode - "B", "P", "R", or "S" (NITF only)
        compression: Format-agnostic compression type: "none", "jpeg", "j2k",
            "lzw", "deflate". Not all combinations are valid for all formats.
            Use "auto" for format-appropriate default.
        comrat: Compression ratio for JPEG2000 (NITF only)
        masked: Enable masked output mode (NITF only)
        mask_pattern: Mask pattern type (checkerboard, border, random, single)
        mask_ratio: Fraction of blocks to mask for random pattern (0.0-1.0)
    """
    output_path: str
    format: str = "nitf"
    width: int = 512
    height: int = 512
    tile_width: int = 256
    tile_height: int = 256
    num_bands: int = 1
    pixel_type: str = "uint8"
    abpp: Optional[int] = None
    imode: str = "B"
    compression: str = "auto"
    comrat: Optional[str] = None
    masked: bool = False
    mask_pattern: str = "checkerboard"
    mask_ratio: float = 0.5

    def __post_init__(self) -> None:
        """Validate configuration and set defaults."""
        # Normalize format: "geotiff" is an alias for "tiff"
        valid_formats = ("nitf", "tiff", "geotiff", "png", "j2k", "jpeg")
        if self.format not in valid_formats:
            raise ValueError(
                f"Format must be one of {valid_formats}, got '{self.format}'"
            )

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
        if self.format == "png":
            if self.num_bands not in (1, 3, 4):
                raise ValueError(
                    f"PNG supports 1, 3, or 4 bands, got {self.num_bands}"
                )
        elif self.format == "jpeg":
            if self.num_bands not in (1, 3):
                raise ValueError(
                    f"JPEG supports 1 (grayscale) or 3 (RGB) bands, got {self.num_bands}"
                )
        elif self.format == "j2k":
            if self.num_bands not in (1, 3, 4):
                raise ValueError(
                    f"J2K supports 1, 3, or 4 bands, got {self.num_bands}"
                )
        elif self.num_bands not in (1, 3, 5):
            raise ValueError(f"Number of bands must be 1, 3, or 5, got {self.num_bands}")

        # Validate pixel type
        if self.pixel_type not in ("uint8", "uint16"):
            raise ValueError(f"Pixel type must be 'uint8' or 'uint16', got '{self.pixel_type}'")

        # JPEG compression only supports uint8 (both as a format and as NITF compression)
        if self.format == "jpeg" and self.pixel_type != "uint8":
            raise ValueError("JPEG format only supports uint8 pixel type")

        # Validate IMODE
        if self.imode not in ("B", "P", "R", "S"):
            raise ValueError(f"IMODE must be 'B', 'P', 'R', or 'S', got '{self.imode}'")

        # Resolve "auto" to format-appropriate default
        if self.compression == "auto":
            self.compression = _DEFAULT_COMPRESSION[self.format]

        # Validate compression is allowed for this format
        valid = _VALID_COMPRESSIONS[self.format]
        if self.compression not in valid:
            raise ValueError(
                f"Compression '{self.compression}' is not valid for "
                f"{self.format.upper()}. Use one of: {', '.join(valid)}"
            )

        # JPEG compression requires uint8 regardless of output format
        if self.compression == "jpeg" and self.pixel_type != "uint8":
            raise ValueError("JPEG compression only supports uint8 pixel type")

        # Validate mask pattern
        if self.mask_pattern not in ("checkerboard", "border", "random", "single"):
            raise ValueError(
                f"Mask pattern must be 'checkerboard', 'border', 'random', or 'single', "
                f"got '{self.mask_pattern}'"
            )

        # Validate mask ratio
        if not 0.0 <= self.mask_ratio <= 1.0:
            raise ValueError(f"Mask ratio must be between 0.0 and 1.0, got {self.mask_ratio}")

        # Masking is only supported for NITF
        if self.masked and self.format not in ("nitf",):
            raise ValueError(
                f"Masked output is not supported for {self.format.upper()} format"
            )

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

    @property
    def effective_ic(self) -> str:
        """Get the effective NITF IC value based on compression and masked settings.

        Maps format-agnostic compression names to NITF IC codes:
        - none  → NC (or NM when masked)
        - jpeg  → C3 (or M3 when masked)
        - j2k   → C8 (or M8 when masked)

        Returns:
            NITF IC code string
        """
        ic_map = {"none": "NC", "jpeg": "C3", "j2k": "C8"}
        masked_map = {"NC": "NM", "C3": "M3", "C8": "M8"}
        ic = ic_map.get(self.compression, self.compression)
        if self.masked:
            ic = masked_map.get(ic, ic)
        return ic

    @property
    def io_format(self) -> str:
        """Return the format string expected by IO.open().

        Maps "geotiff" alias to "tiff" since the IO library treats them
        identically for writing. J2K and JPEG map directly.
        """
        if self.format in ("tiff", "geotiff"):
            return "tiff"
        return self.format

    def get_provided_blocks(self) -> set:
        """Get the set of block coordinates that should have data based on mask pattern.

        Returns:
            Set of (tile_y, tile_x) tuples for blocks that should be provided.
            When masked is False, returns all blocks.
        """
        all_blocks = {(y, x) for y in range(self.num_tiles_y) for x in range(self.num_tiles_x)}

        if not self.masked:
            return all_blocks

        if self.mask_pattern == "checkerboard":
            # Alternating pattern - keep blocks where (row + col) is even
            return {(y, x) for y, x in all_blocks if (y + x) % 2 == 0}

        elif self.mask_pattern == "border":
            # Only edge blocks present
            return {(y, x) for y, x in all_blocks
                    if y == 0 or y == self.num_tiles_y - 1 or x == 0 or x == self.num_tiles_x - 1}

        elif self.mask_pattern == "random":
            # Random subset based on mask_ratio (ratio of blocks to KEEP, not mask)
            import random
            # Use a deterministic seed based on image dimensions for reproducibility
            random.seed(self.width * self.height + self.num_tiles_x * self.num_tiles_y)
            all_blocks_list = sorted(list(all_blocks))
            num_to_keep = max(1, int(len(all_blocks_list) * (1.0 - self.mask_ratio)))
            return set(random.sample(all_blocks_list, num_to_keep))

        elif self.mask_pattern == "single":
            # Only one block present (center block)
            center_y = self.num_tiles_y // 2
            center_x = self.num_tiles_x // 2
            return {(center_y, center_x)}

        return all_blocks


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

    This class assembles tiles into a full image and writes it using the
    existing IO library Python bindings. Supports NITF and TIFF output formats.
    The image data is converted to band-sequential format before writing.

    The writer:
    - Creates a BufferedImageAssetProvider with the correct configuration
    - Generates all tiles and sets them as the full image
    - Uses IO.open() to create a DatasetWriter for the target format
    - Writes the file using DatasetWriter.add_image_asset() and close()
    """

    @staticmethod
    def write_image(config: ImageConfig) -> None:
        """Generate and write a synthetic image.

        Creates a synthetic image with checkerboard pattern and tile IDs.
        The image is written using the IO library's DatasetWriter with a
        BufferedImageAssetProvider that contains the proper image configuration.

        For NITF output, IMODE and IC metadata are set on the provider.
        For TIFF output, these NITF-specific fields are omitted.

        Args:
            config: Image configuration containing all parameters

        Raises:
            RuntimeError: If the IO library fails to create or write the file
            ImportError: If the IO library is not available
        """
        from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

        # Map pixel type string to PixelType enum
        pixel_type = PixelType.UInt8 if config.pixel_type == "uint8" else PixelType.UInt16

        # Create metadata provider with encoding hints
        metadata = BufferedMetadataProvider()

        # NITF-specific metadata (IMODE, IC, COMRAT)
        if config.io_format == "nitf":
            metadata.set("IMODE", config.imode)
            metadata.set("IC", config.effective_ic)
            if config.compression == "j2k" and config.comrat:
                metadata.set("COMRAT", config.comrat)
            if config.compression == "jpeg" and config.comrat:
                metadata.set("COMRAT", config.comrat)

        # TIFF-specific metadata: use TagNameResolver to convert human-readable
        # tag names to the numeric string keys the writer expects.
        if config.io_format == "tiff":
            from aws.osml.io.tiff import TagNameResolver

            tag_dict = metadata.as_dict()
            resolver = TagNameResolver(tag_dict)
            resolver["TileWidth"] = str(config.tile_width)
            resolver["TileLength"] = str(config.tile_height)
            # Map compression CLI value to the writer's expected string values.
            # Default is "None" because macOS Preview cannot render tiled
            # TIFFs with Deflate compression.
            tiff_compression_map = {"none": "None", "lzw": "LZW", "deflate": "Deflate"}
            resolver["Compression"] = tiff_compression_map[config.compression]
            # Write resolved numeric keys back into the metadata provider
            for key, value in tag_dict.items():
                metadata.set(key, str(value) if not isinstance(value, str) else value)

        # J2K-specific metadata (lossless flag)
        if config.io_format == "j2k":
            # J2K format always uses j2k compression; lossless is the default
            metadata.set("J2K_LOSSLESS", "true")

        # JPEG-specific metadata (quality)
        if config.io_format == "jpeg":
            metadata.set("JPEG_QUALITY", "85")

        # Build description string
        if config.io_format == "nitf":
            desc = (f"{config.width}x{config.height} {config.num_bands}-band "
                    f"{config.pixel_type} IMODE={config.imode}")
        else:
            desc = (f"{config.width}x{config.height} {config.num_bands}-band "
                    f"{config.pixel_type}")

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
                description=desc,
            )
        except Exception as e:
            raise RuntimeError(f"Failed to create image provider: {e}")

        # Generate all tiles and concatenate into full image
        full_image = ImageWriter._generate_full_image(config)

        # Convert to band-sequential format and set on provider
        bsq_image = ImageWriter._to_bsq(full_image, config)

        # Get the set of blocks to provide (all blocks if not masked)
        provided_blocks = config.get_provided_blocks()

        try:
            if config.masked:
                # For masked images, use selective set_block() calls
                ImageWriter._set_blocks_selectively(
                    image_provider, bsq_image, config, provided_blocks
                )
            else:
                # For non-masked images, set the full image
                if config.pixel_type == "uint8":
                    image_provider.set_full_image(bsq_image)
                else:
                    image_provider.set_full_image_u16(bsq_image)
        except Exception as e:
            raise RuntimeError(f"Failed to set image data: {e}")

        # Create writer for the target format
        try:
            writer = IO.open([config.output_path], "w", config.io_format)
        except Exception as e:
            raise RuntimeError(f"Failed to create {config.format} writer for {config.output_path}: {e}")

        try:
            # For TIFF, set dataset-level metadata with encoding hints (tile size, etc.)
            if config.io_format == "tiff":
                writer.metadata = metadata
            # For J2K, set dataset-level metadata with encoding hints
            if config.io_format == "j2k":
                writer.metadata = metadata
            # For JPEG, set dataset-level metadata with quality hint
            if config.io_format == "jpeg":
                writer.metadata = metadata
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

    @staticmethod
    def _set_blocks_selectively(
        image_provider,
        bsq_image: np.ndarray,
        config: ImageConfig,
        provided_blocks: set,
    ) -> None:
        """Set only the provided blocks on the image provider.

        This method extracts individual blocks from the full image and sets
        them on the provider using set_block(). Blocks not in provided_blocks
        are skipped, creating a sparse/masked image.

        Args:
            image_provider: BufferedImageAssetProvider to set blocks on
            bsq_image: Full image in BSQ format (bands, rows, cols)
            config: Image configuration
            provided_blocks: Set of (tile_y, tile_x) tuples for blocks to set
        """
        for tile_y in range(config.num_tiles_y):
            for tile_x in range(config.num_tiles_x):
                if (tile_y, tile_x) not in provided_blocks:
                    continue

                # Calculate pixel coordinates for this block
                start_y = tile_y * config.tile_height
                start_x = tile_x * config.tile_width
                end_y = min(start_y + config.tile_height, config.height)
                end_x = min(start_x + config.tile_width, config.width)

                # Extract block data (bands, rows, cols)
                block_data = bsq_image[:, start_y:end_y, start_x:end_x]

                # Ensure contiguous array
                block_data = np.ascontiguousarray(block_data)

                # Set block on provider
                if config.pixel_type == "uint8":
                    image_provider.set_block(tile_y, tile_x, block_data)
                else:
                    image_provider.set_block_u16(tile_y, tile_x, block_data)


def parse_args(args: Optional[list] = None) -> ImageConfig:
    """Parse command-line arguments and return an ImageConfig.

    Args:
        args: Command-line arguments (defaults to sys.argv[1:])

    Returns:
        ImageConfig with parsed values
    """
    parser = argparse.ArgumentParser(
        description="Generate synthetic test images with checkerboard patterns.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s output.ntf
      Generate a default 512x512 grayscale NITF image

  %(prog)s output.ntf --compression jpeg
      Generate a NITF with JPEG compression (IC=C3)

  %(prog)s output.ntf --compression j2k
      Generate a NITF with JPEG 2000 compression (IC=C8)

  %(prog)s output.tif --format tiff
      Generate a 512x512 grayscale TIFF image

  %(prog)s output.png --format png
      Generate a 512x512 grayscale PNG image

  %(prog)s output.ntf --width 1024 --height 1024 --bands 3
      Generate a 1024x1024 RGB image

  %(prog)s output.ntf --pixel-type uint16 --abpp 11
      Generate a 16-bit image with 11-bit actual precision
"""
    )

    # Positional argument for output path
    parser.add_argument(
        "output",
        help="Output file path (e.g., output.ntf, output.tif)"
    )

    # Output format
    parser.add_argument(
        "--format",
        type=str,
        default="nitf",
        choices=["nitf", "tiff", "geotiff", "png", "j2k", "jpeg"],
        help="Output format: nitf (default), tiff, geotiff, png, j2k, or jpeg"
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
        choices=[1, 3, 4, 5],
        help="Number of bands: 1 (grayscale), 3 (RGB), 4 (RGBA, PNG only), or 5 (multispectral) (default: 1)"
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
        default="auto",
        choices=["auto", "none", "jpeg", "j2k", "lzw", "deflate"],
        help=(
            "Compression type (format-agnostic). "
            "NITF: none, jpeg (IC=C3), j2k (IC=C8). "
            "TIFF: none, lzw, deflate. "
            "PNG: deflate (always). "
            "J2K: j2k (always). "
            "JPEG: jpeg (always). "
            "Default: auto (format-appropriate default)"
        )
    )
    parser.add_argument(
        "--comrat",
        type=str,
        default=None,
        metavar="RATIO",
        help="Compression ratio for JPEG2000 or JPEG: N001.0 (J2K lossless), 01.0 (J2K 1.0 bpp), 75.0 (JPEG quality), etc."
    )

    # Masking options
    parser.add_argument(
        "--masked",
        action="store_true",
        help="Enable masked output mode. NITF only: maps none→NM, jpeg→M3, j2k→M8"
    )
    parser.add_argument(
        "--mask-pattern",
        type=str,
        default="checkerboard",
        choices=["checkerboard", "border", "random", "single"],
        help="Mask pattern: checkerboard, border, random, or single (default: checkerboard)"
    )
    parser.add_argument(
        "--mask-ratio",
        type=float,
        default=0.5,
        metavar="RATIO",
        help="Fraction of blocks to mask for random pattern, 0.0-1.0 (default: 0.5)"
    )

    parsed = parser.parse_args(args)

    return ImageConfig(
        output_path=parsed.output,
        format=parsed.format,
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
        masked=parsed.masked,
        mask_pattern=parsed.mask_pattern,
        mask_ratio=parsed.mask_ratio,
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
    print(f"  Format: {config.format}")
    print(f"  Dimensions: {config.width} x {config.height} pixels")
    print(f"  Tile size: {config.tile_width} x {config.tile_height} pixels")
    print(f"  Tiles: {config.num_tiles_x} x {config.num_tiles_y} = {config.total_tiles} total")
    print(f"  Bands: {config.num_bands}")
    print(f"  Pixel type: {config.pixel_type} (ABPP: {config.abpp})")
    if config.io_format == "nitf":
        print(f"  IMODE: {config.imode}")
        print(f"  IC: {config.effective_ic}")
    elif config.io_format == "j2k":
        print(f"  Compression: {config.compression}")
    elif config.io_format == "jpeg":
        print(f"  Quality: 85")
    else:
        print(f"  Compression: {config.compression}")
    if config.masked:
        provided_blocks = config.get_provided_blocks()
        print(f"  Masked: yes (pattern: {config.mask_pattern})")
        print(f"  Provided blocks: {len(provided_blocks)} of {config.total_tiles}")

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
