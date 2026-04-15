#!/usr/bin/env python3
"""Generate a synthetic multi-resolution image pyramid.

Creates a pyramid of resolution levels where each level is half the
dimensions of the previous one. Each tile is independently generated with
a checkerboard pattern and labeled with its resolution level and tile
coordinates (e.g., "L0 R2C3" for level 0, row 2, column 3).

Supports two output modes:
- COG (Cloud Optimized GeoTIFF): single TIFF file with overview IFDs
- R-set: separate NITF files using the R-set naming convention
  (base.ntf, base.ntf.r1, base.ntf.r2, ...)

Usage:
    python scripts/generate_synthetic_image_pyramid.py output.tif --mode cog
    python scripts/generate_synthetic_image_pyramid.py output.ntf --mode rset
    python scripts/generate_synthetic_image_pyramid.py output.tif --mode cog --levels 4
    python scripts/generate_synthetic_image_pyramid.py output.ntf --mode rset --width 2048 --height 2048

Examples:
    # 3-level COG (1024x1024 base, 512x512, 256x256)
    python scripts/generate_synthetic_image_pyramid.py pyramid.tif --mode cog

    # 4-level NITF R-set with RGB bands
    python scripts/generate_synthetic_image_pyramid.py pyramid.ntf --mode rset --levels 4 --bands 3

    # Large pyramid with custom tile size
    python scripts/generate_synthetic_image_pyramid.py big.tif --mode cog \\
        --width 4096 --height 4096 --tile-width 512 --tile-height 512
"""

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))


# Bitmap font for rendering tile labels — 5x7 pixels per character.
# Covers digits 0-9 and uppercase letters L, R, C used in labels.
_FONT = {
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
    'L': [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
    'R': [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
    'C': [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
    ' ': [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
}
_CHAR_W, _CHAR_H, _CHAR_GAP = 5, 7, 1
_LABEL_PAD = 4  # min pixels around label


@dataclass
class PyramidConfig:
    """Configuration for pyramid generation."""
    output_path: str
    mode: str = "cog"          # "cog" or "rset"
    width: int = 1024
    height: int = 1024
    tile_width: int = 256
    tile_height: int = 256
    num_bands: int = 1
    num_levels: int = 3
    pixel_type: str = "uint8"

    def __post_init__(self):
        if self.mode not in ("cog", "rset"):
            raise ValueError(f"Mode must be 'cog' or 'rset', got '{self.mode}'")
        if self.width < 64 or self.height < 64:
            raise ValueError("Width and height must be at least 64")
        if self.tile_width < 16 or self.tile_height < 16:
            raise ValueError("Tile dimensions must be at least 16")
        if self.num_levels < 1:
            raise ValueError("Must have at least 1 level")
        if self.num_bands not in (1, 3):
            raise ValueError("Bands must be 1 or 3")
        if self.pixel_type not in ("uint8", "uint16"):
            raise ValueError("Pixel type must be 'uint8' or 'uint16'")

        # Ensure we don't create levels smaller than one tile
        max_levels = 1
        w, h = self.width, self.height
        while w >= self.tile_width * 2 and h >= self.tile_height * 2:
            w //= 2
            h //= 2
            max_levels += 1
        if self.num_levels > max_levels:
            raise ValueError(
                f"Too many levels ({self.num_levels}) for {self.width}x{self.height} "
                f"with {self.tile_width}x{self.tile_height} tiles. Max: {max_levels}"
            )

    @property
    def dtype(self) -> np.dtype:
        return np.uint8 if self.pixel_type == "uint8" else np.uint16

    @property
    def max_val(self) -> int:
        return 255 if self.pixel_type == "uint8" else 65535

    def level_dims(self, level: int) -> tuple[int, int]:
        """Return (width, height) for a given level."""
        return self.width >> level, self.height >> level


# -- Tile color palette per level (hue shifts so levels are visually distinct) --
# Each entry is (light_rgb, dark_rgb) for the checkerboard.
_LEVEL_COLORS = [
    ((200, 180, 160), (80, 100, 120)),   # L0: tan / blue-gray
    ((160, 200, 160), (80, 120, 80)),     # L1: green tones
    ((180, 160, 200), (100, 80, 120)),    # L2: purple tones
    ((200, 200, 140), (100, 100, 60)),    # L3: yellow tones
    ((140, 200, 200), (60, 100, 100)),    # L4: cyan tones
    ((200, 160, 160), (120, 80, 80)),     # L5: red tones
]


def _render_label(tile: np.ndarray, label: str, fg: int) -> None:
    """Render a text label centered in the tile (all bands)."""
    h, w, bands = tile.shape
    text_w = len(label) * _CHAR_W + (len(label) - 1) * _CHAR_GAP
    if w < text_w + 2 * _LABEL_PAD or h < _CHAR_H + 2 * _LABEL_PAD:
        return
    sx = (w - text_w) // 2
    sy = (h - _CHAR_H) // 2
    cx = sx
    for ch in label:
        pattern = _FONT.get(ch, _FONT[' '])
        for row, bits in enumerate(pattern):
            for col in range(_CHAR_W):
                if bits & (1 << (_CHAR_W - 1 - col)):
                    tile[sy + row, cx + col, :] = fg
        cx += _CHAR_W + _CHAR_GAP


def _generate_level_image(
    config: PyramidConfig,
    level: int,
) -> np.ndarray:
    """Generate the full image for one resolution level.

    Each tile gets a checkerboard background color that varies by level,
    and a centered label like "L0 R2C3".
    """
    w, h = config.level_dims(level)
    tw, th = config.tile_width, config.tile_height
    cols_count = (w + tw - 1) // tw
    rows_count = (h + th - 1) // th

    light, dark = _LEVEL_COLORS[level % len(_LEVEL_COLORS)]
    image = np.zeros((h, w, config.num_bands), dtype=config.dtype)

    scale = config.max_val / 255.0

    for tr in range(rows_count):
        for tc in range(cols_count):
            y0 = tr * th
            x0 = tc * tw
            y1 = min(y0 + th, h)
            x1 = min(x0 + tw, w)
            actual_h = y1 - y0
            actual_w = x1 - x0

            is_light = (tr + tc) % 2 == 0
            rgb = light if is_light else dark

            tile = np.zeros((actual_h, actual_w, config.num_bands), dtype=config.dtype)
            for b in range(config.num_bands):
                val = int(rgb[b % 3] * scale)
                tile[:, :, b] = val

            # Determine foreground color for label
            lum = int(rgb[0] * scale)
            fg = config.max_val if lum < config.max_val // 2 else 0

            label = f"L{level} R{tr}C{tc}"
            _render_label(tile, label, fg)

            image[y0:y1, x0:x1, :] = tile

    return image


def _to_bsq(image: np.ndarray) -> np.ndarray:
    """Convert (H, W, bands) to (bands, H, W) contiguous."""
    return np.ascontiguousarray(np.transpose(image, (2, 0, 1)))


def write_cog(config: PyramidConfig) -> None:
    """Write a COG (single TIFF with overview IFDs)."""
    from aws.osml.io import IO, BufferedImageAssetProvider, PixelType

    pixel_type = PixelType.UInt8 if config.pixel_type == "uint8" else PixelType.UInt16

    writer = IO.open([config.output_path], "w", "tiff")

    for level in range(config.num_levels):
        w, h = config.level_dims(level)
        image = _generate_level_image(config, level)
        bsq = _to_bsq(image)

        if level == 0:
            key = "image:0"
            role = "data"
            title = "Base"
        else:
            key = f"image:0:overview:{level}"
            role = "overview"
            title = f"Overview {level}"

        provider = BufferedImageAssetProvider.create(
            key=key,
            num_columns=w,
            num_rows=h,
            num_bands=config.num_bands,
            block_width=config.tile_width,
            block_height=config.tile_height,
            pixel_type=pixel_type,
        )
        if config.pixel_type == "uint8":
            provider.set_full_image(bsq)
        else:
            provider.set_full_image_u16(bsq)

        writer.add_asset(key, provider, title, f"{w}x{h}", [role])
        print(f"  Level {level}: {w}x{h} ({key})")

    writer.close()
    size = Path(config.output_path).stat().st_size
    print(f"\nWrote: {config.output_path} ({size:,} bytes)")


def write_rset(config: PyramidConfig) -> None:
    """Write an NITF R-set (one file per level)."""
    from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

    pixel_type = PixelType.UInt8 if config.pixel_type == "uint8" else PixelType.UInt16

    base_path = config.output_path
    all_paths = [base_path]
    for i in range(1, config.num_levels):
        all_paths.append(f"{base_path}.r{i}")

    writer = IO.open(all_paths, "w", "nitf")

    for level in range(config.num_levels):
        w, h = config.level_dims(level)
        image = _generate_level_image(config, level)
        bsq = _to_bsq(image)

        if level == 0:
            key = "image:0"
            role = "data"
            title = "Base"
        else:
            key = f"image:0:overview:{level}"
            role = "overview"
            title = f"Overview {level}"

        metadata = BufferedMetadataProvider()
        metadata.set("IC", "NC")
        metadata.set("IMODE", "B")

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=w,
            num_rows=h,
            num_bands=config.num_bands,
            block_width=config.tile_width,
            block_height=config.tile_height,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        if config.pixel_type == "uint8":
            provider.set_full_image(bsq)
        else:
            provider.set_full_image_u16(bsq)

        writer.add_asset(key, provider, title, f"{w}x{h}", [role])
        print(f"  Level {level}: {w}x{h} → {all_paths[level]}")

    writer.close()

    print()
    for p in all_paths:
        size = Path(p).stat().st_size
        print(f"  {p}: {size:,} bytes")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate a synthetic multi-resolution image pyramid.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s pyramid.tif --mode cog
      3-level COG (1024→512→256)

  %(prog)s pyramid.ntf --mode rset --levels 4 --bands 3
      4-level RGB NITF R-set

  %(prog)s big.tif --mode cog --width 4096 --height 4096 --tile-width 512 --tile-height 512
      Large COG with 512x512 tiles
""",
    )
    parser.add_argument("output", help="Output file path")
    parser.add_argument("--mode", choices=["cog", "rset"], default="cog",
                        help="Output mode: cog (single TIFF) or rset (multi-file NITF)")
    parser.add_argument("--width", type=int, default=1024, help="Base level width (default: 1024)")
    parser.add_argument("--height", type=int, default=1024, help="Base level height (default: 1024)")
    parser.add_argument("--tile-width", type=int, default=256, help="Tile width (default: 256)")
    parser.add_argument("--tile-height", type=int, default=256, help="Tile height (default: 256)")
    parser.add_argument("--bands", type=int, default=1, choices=[1, 3], help="Number of bands (default: 1)")
    parser.add_argument("--levels", type=int, default=3, help="Number of resolution levels (default: 3)")
    parser.add_argument("--pixel-type", default="uint8", choices=["uint8", "uint16"],
                        help="Pixel data type (default: uint8)")

    args = parser.parse_args()

    try:
        config = PyramidConfig(
            output_path=args.output,
            mode=args.mode,
            width=args.width,
            height=args.height,
            tile_width=args.tile_width,
            tile_height=args.tile_height,
            num_bands=args.bands,
            num_levels=args.levels,
            pixel_type=args.pixel_type,
        )
    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    print(f"Generating {config.num_levels}-level pyramid ({config.mode.upper()})")
    print(f"  Base: {config.width}x{config.height}, {config.num_bands} band(s), {config.pixel_type}")
    print(f"  Tiles: {config.tile_width}x{config.tile_height}")
    print()

    try:
        if config.mode == "cog":
            write_cog(config)
        else:
            write_rset(config)
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 2


if __name__ == "__main__":
    sys.exit(main())
