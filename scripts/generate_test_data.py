#!/usr/bin/env python3
"""Generate synthetic test data files for unit and property tests.

This script creates all test data files in data/unit/ using the IO library.
Now that the JBP writer honors metadata (see BUG_JBP_WRITER_FILE_METADATA.md),
every generated file has populated header fields (FTITLE, ONAME, OSTAID, etc.).

Generated files:
  nitf21-8x8-1band-8bit-nc.ntf
      Minimal NITF 2.1, 8x8 grayscale, no compression.
      Used by: round-trip tests (test_io_contracts.py)

  nsif10-8x8-1band-8bit-nc.nsif
      Minimal NSIF 1.0, 8x8 grayscale, no compression.
      Used by: round-trip tests (test_io_contracts.py)

  nitf21-multisegment-2img-1txt-1des.ntf
      NITF 2.1 with 2 image segments, 1 text, 1 DES.
      Used by: multi-segment round-trip tests (test_io_contracts.py)

  nitf21-64x64-3band-8bit-j2k.ntf
      NITF 2.1, 64x64 RGB, JPEG 2000 compression (IC=C8).
      Used by: J2K decode tests (test_zarr_codecs.py)

  nitf21-64x64-3band-8bit-jpeg.ntf
      NITF 2.1, 64x64 RGB, JPEG compression (IC=C3).
      Used by: JPEG decode tests (test_zarr_codecs.py)

  nitf21-256x256-3band-8bit-nc.ntf
      NITF 2.1, 256x256 RGB, no compression. Populated metadata.
      Used by: parser tests (test_parser.py), format auto-detection,
               IO contract tests (test_io_contracts.py)

  tiff-256x256-1band-8bit-tiled-deflate.tif
      Tiled TIFF, 256x256, 1-band uint8, 128x128 tiles, Deflate.
      Used by: TIFF API tests, IFD tag enumeration (Rust ffi.rs)

Run:
    python scripts/generate_test_data.py
"""

import sys
from pathlib import Path

import numpy as np

project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import (  # noqa: E402
    IO,
    AssetProvider,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

# ── Shared metadata helpers ──────────────────────────────────────────────────

def _nitf_file_metadata(ftitle: str) -> BufferedMetadataProvider:
    """Create a BufferedMetadataProvider with standard NITF file-level fields."""
    meta = BufferedMetadataProvider()
    meta["FTITLE"] = ftitle
    meta["ONAME"] = "OSML Test Generator"
    meta["OPHONE"] = "555-000-0000"
    meta["OSTAID"] = "OSML_IO"
    meta["FDT"] = "20260101120000"
    meta["FSCLAS"] = "U"
    return meta


def _nitf_image_metadata(*, ic: str = "NC", imode: str = "B",
                          icat: str = "VIS") -> BufferedMetadataProvider:
    """Create a BufferedMetadataProvider with NITF image-segment fields."""
    meta = BufferedMetadataProvider()
    meta["IC"] = ic
    meta["IMODE"] = imode
    meta["ICAT"] = icat
    return meta


# ── Generator functions ──────────────────────────────────────────────────────

def generate_nitf21_8x8(output_path: Path) -> None:
    """Minimal NITF 2.1 with one 8x8 grayscale image, no compression."""
    print(f"  {output_path.name} ...")

    file_meta = _nitf_file_metadata("Minimal 8x8 grayscale NITF 2.1")
    writer = IO.open([str(output_path)], "w", "nitf")
    writer.metadata = file_meta

    image_data = bytes([(x + y) % 256 for y in range(8) for x in range(8)])
    asset = AssetProvider.from_bytes(
        key="image:0",
        data=image_data,
        asset_type=AssetType.Image,
        title="8x8 Grayscale",
        description="Minimal test image",
    )
    writer.add_asset("image:0", asset, "8x8 Grayscale",
                     "Minimal test image", ["data"])
    writer.close()


def generate_nsif10_8x8(output_path: Path) -> None:
    """Minimal NSIF 1.0 with one 8x8 grayscale image, no compression."""
    print(f"  {output_path.name} ...")

    file_meta = _nitf_file_metadata("Minimal 8x8 grayscale NSIF 1.0")
    writer = IO.open([str(output_path)], "w", "nsif")
    writer.metadata = file_meta

    image_data = bytes([(x + y) % 256 for y in range(8) for x in range(8)])
    asset = AssetProvider.from_bytes(
        key="image:0",
        data=image_data,
        asset_type=AssetType.Image,
        title="8x8 Grayscale",
        description="Minimal test image",
    )
    writer.add_asset("image:0", asset, "8x8 Grayscale",
                     "Minimal test image", ["data"])
    writer.close()


def generate_multisegment(output_path: Path) -> None:
    """NITF 2.1 with 2 images, 1 text, 1 DES."""
    print(f"  {output_path.name} ...")

    file_meta = _nitf_file_metadata("Multi-segment NITF 2.1 test file")
    writer = IO.open([str(output_path)], "w", "nitf")
    writer.metadata = file_meta

    # Image 1: 16x16
    img1 = bytes([(x + y) % 256 for y in range(16) for x in range(16)])
    writer.add_asset(
        "image:0",
        AssetProvider.from_bytes("image:0", img1, AssetType.Image,
                                "First Image", "16x16 grayscale"),
        "First Image", "16x16 grayscale", ["data"],
    )

    # Image 2: 8x8
    img2 = bytes([(x * y) % 256 for y in range(8) for x in range(8)])
    writer.add_asset(
        "image:1",
        AssetProvider.from_bytes("image:1", img2, AssetType.Image,
                                "Second Image", "8x8 grayscale"),
        "Second Image", "8x8 grayscale", ["data"],
    )

    # Text segment
    writer.add_asset(
        "text:0",
        AssetProvider.from_bytes("text:0",
                                b"This is sample text content for testing.",
                                AssetType.Text, "Sample Text", "Test text"),
        "Sample Text", "Test text", ["metadata"],
    )

    # DES segment
    writer.add_asset(
        "des:0",
        AssetProvider.from_bytes("des:0",
                                b"Sample DES data content",
                                AssetType.Data, "Sample DES", "Test DES"),
        "Sample DES", "Test DES", ["metadata"],
    )

    writer.close()


def generate_nitf21_j2k(output_path: Path) -> None:
    """NITF 2.1, 64x64 RGB, JPEG 2000 compression."""
    print(f"  {output_path.name} ...")

    file_meta = _nitf_file_metadata("64x64 3-band J2K NITF 2.1")

    img_meta = _nitf_image_metadata(ic="C8", imode="B")

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=64,
        num_rows=64,
        num_bands=3,
        block_width=64,
        block_height=64,
        pixel_type=PixelType.UInt8,
        metadata=img_meta,
        title="64x64 RGB J2K",
        description="JPEG 2000 compressed test image",
    )

    # Deterministic checkerboard pattern
    rng = np.random.RandomState(42)
    array = rng.randint(0, 256, (3, 64, 64), dtype=np.uint8)
    provider.set_full_image(array)

    writer = IO.open([str(output_path)], "w", "nitf")
    writer.metadata = file_meta
    writer.add_asset("image:0", provider, "64x64 RGB J2K",
                     "JPEG 2000 compressed test image", ["data"])
    writer.close()


def generate_nitf21_jpeg(output_path: Path) -> None:
    """NITF 2.1, 64x64 RGB, JPEG compression (IC=C3)."""
    print(f"  {output_path.name} ...")

    file_meta = _nitf_file_metadata("64x64 3-band JPEG NITF 2.1")

    img_meta = _nitf_image_metadata(ic="C3", imode="P")

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=64,
        num_rows=64,
        num_bands=3,
        block_width=64,
        block_height=64,
        pixel_type=PixelType.UInt8,
        metadata=img_meta,
        title="64x64 RGB JPEG",
        description="JPEG compressed test image",
    )

    rng = np.random.RandomState(42)
    array = rng.randint(0, 256, (3, 64, 64), dtype=np.uint8)
    provider.set_full_image(array)

    writer = IO.open([str(output_path)], "w", "nitf")
    writer.metadata = file_meta
    writer.add_asset("image:0", provider, "64x64 RGB JPEG",
                     "JPEG compressed test image", ["data"])
    writer.close()


def generate_nitf21_256x256(output_path: Path) -> None:
    """NITF 2.1, 256x256 RGB, no compression, with populated metadata.

    This file replaces the old synthetic_nitf_header.bin and small.ntf.
    It has real metadata fields that parser tests can validate.
    """
    print(f"  {output_path.name} ...")

    file_meta = _nitf_file_metadata("Synthetic 256x256 RGB NITF 2.1")

    img_meta = _nitf_image_metadata(ic="NC", imode="B", icat="VIS")

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=256,
        num_rows=256,
        num_bands=3,
        block_width=256,
        block_height=256,
        pixel_type=PixelType.UInt8,
        metadata=img_meta,
        title="256x256 RGB",
        description="Uncompressed 256x256 3-band test image",
    )

    rng = np.random.RandomState(99)
    array = rng.randint(0, 256, (3, 256, 256), dtype=np.uint8)
    provider.set_full_image(array)

    writer = IO.open([str(output_path)], "w", "nitf")
    writer.metadata = file_meta
    writer.add_asset("image:0", provider, "256x256 RGB",
                     "Uncompressed 256x256 3-band test image", ["data"])
    writer.close()


def generate_tiff_tiled(output_path: Path) -> None:
    """Tiled TIFF, 256x256, 1-band uint8, 128x128 tiles, Deflate."""
    print(f"  {output_path.name} ...")

    metadata = BufferedMetadataProvider()
    metadata["322"] = 128   # TileWidth
    metadata["323"] = 128   # TileLength
    metadata["259"] = 8     # Compression = Deflate

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=256,
        num_rows=256,
        num_bands=1,
        block_width=128,
        block_height=128,
        pixel_type=PixelType.UInt8,
        metadata=metadata,
        title="256x256 Tiled TIFF",
        description="Tiled Deflate 1-band uint8",
    )

    rng = np.random.RandomState(77)
    array = rng.randint(0, 256, (1, 256, 256), dtype=np.uint8)
    provider.set_full_image(array)

    writer = IO.open([str(output_path)], "w", "tiff")
    writer.metadata = metadata
    writer.add_asset("image:0", provider, "256x256 Tiled TIFF",
                     "Tiled Deflate 1-band uint8", ["data"])
    writer.close()


def generate_dted_small(output_path: Path) -> None:
    """Small synthetic DTED Level 1 file (16x16 grid) for unit tests."""
    print(f"  {output_path.name} ...")

    metadata = BufferedMetadataProvider()
    metadata["dted:origin_longitude"] = -109.0
    metadata["dted:origin_latitude"] = 38.0
    metadata["dted:longitude_interval"] = 30
    metadata["dted:latitude_interval"] = 30
    metadata["dted:level"] = "DTED1"
    metadata["dted:security_code"] = "U"
    metadata["dted:vertical_datum"] = "MSL"
    metadata["dted:horizontal_datum"] = "WGS84"
    metadata["dted:producer_code"] = "US"
    metadata["dted:edition_number"] = "01"
    metadata["dted:compilation_date"] = "0101"
    metadata["dted:partial_cell_indicator"] = "00"
    metadata["dted:absolute_horizontal_accuracy"] = "0050"
    metadata["dted:absolute_vertical_accuracy"] = "0030"
    metadata["dted:relative_vertical_accuracy"] = "0020"
    metadata["dted:vertical_accuracy"] = 20

    num_rows = 16
    num_cols = 16

    provider = BufferedImageAssetProvider.create(
        key="elevation",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=1,
        block_width=num_cols,
        block_height=num_rows,
        pixel_type=PixelType.Int16,
        metadata=metadata,
    )

    # Deterministic elevation pattern with some negative values and null sentinel
    rng = np.random.RandomState(42)
    array = rng.randint(-500, 4000, (1, num_rows, num_cols), dtype=np.int16)
    array[0, 0, 0] = -32767  # null sentinel at corner
    array[0, 7, 7] = -32767  # null sentinel in middle
    provider.set_full_image(array)

    writer = IO.open([str(output_path)], "w", "dted")
    writer.metadata = metadata
    writer.add_asset("elevation", provider, "Elevation", "DTED test data", ["data"])
    writer.close()


def generate_dted_integration(output_path: Path) -> None:
    """Larger synthetic DTED Level 1 for integration tests (64x64)."""
    print(f"  {output_path.name} ...")

    metadata = BufferedMetadataProvider()
    metadata["dted:origin_longitude"] = -109.0
    metadata["dted:origin_latitude"] = 38.0
    metadata["dted:longitude_interval"] = 30
    metadata["dted:latitude_interval"] = 30
    metadata["dted:level"] = "DTED1"
    metadata["dted:security_code"] = "U"
    metadata["dted:vertical_datum"] = "MSL"
    metadata["dted:horizontal_datum"] = "WGS84"
    metadata["dted:producer_code"] = "US"
    metadata["dted:edition_number"] = "01"
    metadata["dted:compilation_date"] = "2601"
    metadata["dted:partial_cell_indicator"] = "00"
    metadata["dted:absolute_horizontal_accuracy"] = "0050"
    metadata["dted:absolute_vertical_accuracy"] = "0030"
    metadata["dted:relative_vertical_accuracy"] = "0020"
    metadata["dted:vertical_accuracy"] = 20

    num_rows = 64
    num_cols = 64

    provider = BufferedImageAssetProvider.create(
        key="elevation",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=1,
        block_width=num_cols,
        block_height=num_rows,
        pixel_type=PixelType.Int16,
        metadata=metadata,
    )

    # Terrain-like elevation: gradient + noise
    rng = np.random.RandomState(99)
    x = np.linspace(0, 1, num_cols)
    y = np.linspace(0, 1, num_rows)
    xx, yy = np.meshgrid(x, y)
    base = (xx * 2000 + yy * 1500 - 500).astype(np.int16)
    noise = rng.randint(-50, 50, (num_rows, num_cols), dtype=np.int16)
    array = np.clip(base.astype(np.int32) + noise.astype(np.int32), -12000, 9000).astype(np.int16)
    array = array.reshape(1, num_rows, num_cols)
    provider.set_full_image(array)

    writer = IO.open([str(output_path)], "w", "dted")
    writer.metadata = metadata
    writer.add_asset("elevation", provider, "Elevation", "DTED integration test", ["data"])
    writer.close()


# ── Verification ─────────────────────────────────────────────────────────────

def verify_file(file_path: Path) -> bool:
    """Read back a generated file and verify it has accessible assets."""
    try:
        reader = IO.open([str(file_path)], "r")
        keys = reader.get_asset_keys()
        for key in keys:
            assert reader.has_asset(key)
            assert reader.get_asset(key) is not None
        print(f"    ✓ {len(keys)} asset(s)")
        return True
    except Exception as e:
        print(f"    ✗ {e}")
        return False


# ── Main ─────────────────────────────────────────────────────────────────────

FILES = [
    ("nitf21-8x8-1band-8bit-nc.ntf", generate_nitf21_8x8),
    ("nsif10-8x8-1band-8bit-nc.nsif", generate_nsif10_8x8),
    ("nitf21-multisegment-2img-1txt-1des.ntf", generate_multisegment),
    ("nitf21-64x64-3band-8bit-j2k.ntf", generate_nitf21_j2k),
    ("nitf21-64x64-3band-8bit-jpeg.ntf", generate_nitf21_jpeg),
    ("nitf21-256x256-3band-8bit-nc.ntf", generate_nitf21_256x256),
    ("tiff-256x256-1band-8bit-tiled-deflate.tif", generate_tiff_tiled),
    ("dted-16x16-1band-int16.dt1", generate_dted_small),
]

INTEGRATION_FILES = [
    ("synth_dted_level1.dt1", generate_dted_integration),
]


def main():
    output_dir = project_root / "data" / "unit"
    output_dir.mkdir(parents=True, exist_ok=True)

    print("Generating unit test data files")
    print("=" * 50)

    ok = True
    for name, gen_fn in FILES:
        path = output_dir / name
        try:
            gen_fn(path)
            if not verify_file(path):
                ok = False
        except Exception as e:
            print(f"    ✗ FAILED: {e}")
            import traceback
            traceback.print_exc()
            ok = False

    # Generate integration test synthetic data
    integration_dir = project_root / "data" / "integration" / "synthetic"
    integration_dir.mkdir(parents=True, exist_ok=True)

    print("\nGenerating integration test data files")
    print("=" * 50)

    for name, gen_fn in INTEGRATION_FILES:
        path = integration_dir / name
        try:
            gen_fn(path)
            if not verify_file(path):
                ok = False
        except Exception as e:
            print(f"    ✗ FAILED: {e}")
            import traceback
            traceback.print_exc()
            ok = False

    print("=" * 50)
    if ok:
        print("All files generated and verified.")
    else:
        print("Some files failed.")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
