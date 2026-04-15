"""Unit tests for _build_codec_instance() TIFF path.

Validates that TIFF codec configurations (containing a 'compression' key)
are correctly mapped to TiffTileCodec instances, including LE byte
normalization and jpeg_tables base64 encoding.

Requirements: 14.1, 14.2
"""

import base64
import struct
from unittest.mock import MagicMock

import pytest

virtualizarr = pytest.importorskip("virtualizarr", minversion="2.0")

from aws.osml.io.virtualizarr_parsers import _build_codec_instance
from aws.osml.io.zarr_codecs import TiffTileCodec


def _make_mock_asset(codec_config, num_bands=3, num_bits_per_pixel=8,
                     block_h=256, block_w=256):
    """Create a mock asset with the given codec_configuration dict."""
    asset = MagicMock()
    asset.codec_configuration.return_value = codec_config
    asset.num_bands = num_bands
    asset.num_bits_per_pixel = num_bits_per_pixel
    asset.num_pixels_per_block_vertical = block_h
    asset.num_pixels_per_block_horizontal = block_w
    return asset


class TestBuildCodecInstanceTiffPath:
    """Verify _build_codec_instance() produces TiffTileCodec for TIFF configs.

    Requirements: 14.1, 14.2
    """

    def test_compression_key_produces_tiff_tile_codec(self):
        """A config with 'compression' key returns a TiffTileCodec instance."""
        config = {
            "compression": struct.pack("<H", 5),       # LZW
            "bits_per_sample": struct.pack("<H", 8),
            "samples_per_pixel": struct.pack("<H", 3),
            "photometric": struct.pack("<H", 2),        # RGB
            "planar_config": struct.pack("<H", 1),
            "predictor": struct.pack("<H", 2),           # horizontal
            "tile_width": struct.pack("<I", 256),
            "tile_height": struct.pack("<I", 256),
            "sample_format": struct.pack("<H", 1),
        }
        asset = _make_mock_asset(config)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        assert codec.compression == 5
        assert codec.bits_per_sample == 8
        assert codec.samples_per_pixel == 3
        assert codec.photometric == 2
        assert codec.planar_config == 1
        assert codec.predictor == 2
        assert codec.tile_width == 256
        assert codec.tile_height == 256
        assert codec.sample_format == 1
        assert codec.jpeg_tables is None

    def test_jpeg_tables_raw_bytes_are_base64_encoded(self):
        """Raw jpeg_tables bytes are base64-encoded before passing to TiffTileCodec."""
        # Simulate JPEG tables with SOI/EOI markers
        jpeg_tables_raw = b"\xff\xd8\xff\xdb\x00\x43" + b"\x00" * 64 + b"\xff\xd9"
        config = {
            "compression": struct.pack("<H", 7),       # JPEG
            "bits_per_sample": struct.pack("<H", 8),
            "samples_per_pixel": struct.pack("<H", 3),
            "photometric": struct.pack("<H", 6),        # YCbCr
            "planar_config": struct.pack("<H", 1),
            "predictor": struct.pack("<H", 1),
            "tile_width": struct.pack("<I", 512),
            "tile_height": struct.pack("<I", 512),
            "sample_format": struct.pack("<H", 1),
            "jpeg_tables": jpeg_tables_raw,
        }
        asset = _make_mock_asset(config)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        assert codec.compression == 7
        # jpeg_tables should be base64-encoded string
        expected_b64 = base64.b64encode(jpeg_tables_raw).decode("ascii")
        assert codec.jpeg_tables == expected_b64
        # Verify round-trip: decoding the stored b64 gives back original bytes
        assert base64.b64decode(codec.jpeg_tables) == jpeg_tables_raw

    def test_2byte_le_values_normalized_to_integers(self):
        """2-byte LE values (u16) are normalized to Python ints."""
        config = {
            "compression": struct.pack("<H", 32773),   # PackBits
            "bits_per_sample": struct.pack("<H", 16),
            "samples_per_pixel": struct.pack("<H", 1),
            "photometric": struct.pack("<H", 1),        # MinIsBlack
            "planar_config": struct.pack("<H", 1),
            "tile_width": struct.pack("<I", 128),
            "tile_height": struct.pack("<I", 128),
            "sample_format": struct.pack("<H", 2),      # INT (signed)
        }
        asset = _make_mock_asset(config, num_bands=1)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        # All u16 fields should be ints, not bytes
        assert isinstance(codec.compression, int)
        assert codec.compression == 32773
        assert isinstance(codec.bits_per_sample, int)
        assert codec.bits_per_sample == 16
        assert isinstance(codec.samples_per_pixel, int)
        assert codec.samples_per_pixel == 1
        assert isinstance(codec.sample_format, int)
        assert codec.sample_format == 2

    def test_4byte_le_values_normalized_to_integers(self):
        """4-byte LE values (u32) are normalized to Python ints."""
        config = {
            "compression": struct.pack("<H", 8),       # Deflate
            "bits_per_sample": struct.pack("<H", 8),
            "samples_per_pixel": struct.pack("<H", 4),
            "photometric": struct.pack("<H", 2),
            "planar_config": struct.pack("<H", 1),
            "predictor": struct.pack("<H", 1),
            "tile_width": struct.pack("<I", 512),
            "tile_height": struct.pack("<I", 1024),
            "sample_format": struct.pack("<H", 1),
        }
        asset = _make_mock_asset(config, num_bands=4)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        # u32 fields should be ints, not bytes
        assert isinstance(codec.tile_width, int)
        assert codec.tile_width == 512
        assert isinstance(codec.tile_height, int)
        assert codec.tile_height == 1024

    def test_defaults_from_asset_when_keys_missing(self):
        """Missing config keys fall back to asset metadata defaults."""
        config = {
            "compression": struct.pack("<H", 5),
        }
        asset = _make_mock_asset(
            config, num_bands=4, num_bits_per_pixel=16,
            block_h=128, block_w=64,
        )
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        assert codec.compression == 5
        # Defaults from asset
        assert codec.bits_per_sample == 16   # from num_bits_per_pixel
        assert codec.samples_per_pixel == 4  # from num_bands
        assert codec.tile_width == 64        # from block_w
        assert codec.tile_height == 128      # from block_h
        # Defaults from code
        assert codec.photometric == 1
        assert codec.planar_config == 1
        assert codec.predictor == 1
        assert codec.sample_format == 1
        assert codec.jpeg_tables is None
