"""Unit tests for TiffTileCodec: serialization round-trips, defaults, encode rejection, and validation.

Validates: Requirements 11.3, 11.4, 11.5, 12.1, 12.2, 12.3
"""

import asyncio
import base64

import pytest
from aws.osml.io.zarr_codecs import TiffTileCodec


# Non-default config values used across round-trip tests
JPEG_TABLES_BYTES = b"\xff\xd8\xff\xdb\x00\x43\x00\x08\xff\xd9"
JPEG_TABLES_B64 = base64.b64encode(JPEG_TABLES_BYTES).decode("ascii")

NON_DEFAULT_KWARGS = {
    "compression": 7,
    "bits_per_sample": 16,
    "samples_per_pixel": 3,
    "photometric": 6,
    "planar_config": 2,
    "predictor": 2,
    "tile_width": 512,
    "tile_height": 512,
    "sample_format": 2,
    "jpeg_tables": JPEG_TABLES_B64,
}


class TestTiffTileCodecToFromDictRoundTrip:
    """to_dict() / from_dict() round-trip preserves all config values."""

    def test_round_trip_all_non_default(self):
        """Round-trip with all non-default values preserves every field."""
        codec = TiffTileCodec(**NON_DEFAULT_KWARGS)
        d = codec.to_dict()
        codec2 = TiffTileCodec.from_dict(d)

        assert codec2.compression == 7
        assert codec2.bits_per_sample == 16
        assert codec2.samples_per_pixel == 3
        assert codec2.photometric == 6
        assert codec2.planar_config == 2
        assert codec2.predictor == 2
        assert codec2.tile_width == 512
        assert codec2.tile_height == 512
        assert codec2.sample_format == 2
        assert codec2.jpeg_tables == JPEG_TABLES_B64
        assert codec2._jpeg_tables_bytes == JPEG_TABLES_BYTES
        assert codec2.to_dict() == d

    def test_round_trip_no_jpeg_tables(self):
        """Round-trip without jpeg_tables preserves None."""
        codec = TiffTileCodec(compression=5, predictor=2)
        d = codec.to_dict()
        codec2 = TiffTileCodec.from_dict(d)

        assert codec2.compression == 5
        assert codec2.predictor == 2
        assert codec2.jpeg_tables is None
        assert codec2._jpeg_tables_bytes is None
        assert codec2.to_dict() == d


class TestTiffTileCodecGetFromConfigRoundTrip:
    """get_config() / from_config() round-trip preserves all config values."""

    def test_round_trip_all_non_default(self):
        """Round-trip with all non-default values preserves every field."""
        codec = TiffTileCodec(**NON_DEFAULT_KWARGS)
        cfg = codec.get_config()
        codec2 = TiffTileCodec.from_config(cfg)

        assert codec2.compression == 7
        assert codec2.bits_per_sample == 16
        assert codec2.samples_per_pixel == 3
        assert codec2.photometric == 6
        assert codec2.planar_config == 2
        assert codec2.predictor == 2
        assert codec2.tile_width == 512
        assert codec2.tile_height == 512
        assert codec2.sample_format == 2
        assert codec2.jpeg_tables == JPEG_TABLES_B64
        assert codec2._jpeg_tables_bytes == JPEG_TABLES_BYTES
        assert codec2.get_config() == cfg

    def test_round_trip_no_jpeg_tables(self):
        """Round-trip without jpeg_tables preserves None."""
        codec = TiffTileCodec(compression=8, predictor=3)
        cfg = codec.get_config()
        codec2 = TiffTileCodec.from_config(cfg)

        assert codec2.compression == 8
        assert codec2.predictor == 3
        assert codec2.jpeg_tables is None
        assert codec2.get_config() == cfg

    def test_get_config_includes_id(self):
        """get_config() includes the codec_id as 'id'."""
        codec = TiffTileCodec()
        cfg = codec.get_config()
        assert cfg["id"] == TiffTileCodec.codec_id


class TestTiffTileCodecFromDictDefaults:
    """from_dict() with missing fields uses defaults."""

    def test_empty_configuration_uses_all_defaults(self):
        """from_dict with empty configuration dict uses all default values."""
        codec = TiffTileCodec.from_dict({"configuration": {}})

        assert codec.compression == 1
        assert codec.bits_per_sample == 8
        assert codec.samples_per_pixel == 1
        assert codec.photometric == 1
        assert codec.planar_config == 1
        assert codec.predictor == 1
        assert codec.tile_width == 256
        assert codec.tile_height == 256
        assert codec.sample_format == 1
        assert codec.jpeg_tables is None

    def test_partial_configuration_fills_defaults(self):
        """from_dict with partial config fills missing fields with defaults."""
        codec = TiffTileCodec.from_dict({"configuration": {"compression": 5, "tile_width": 128}})

        assert codec.compression == 5
        assert codec.tile_width == 128
        # All other fields should be defaults
        assert codec.bits_per_sample == 8
        assert codec.samples_per_pixel == 1
        assert codec.photometric == 1
        assert codec.planar_config == 1
        assert codec.predictor == 1
        assert codec.tile_height == 256
        assert codec.sample_format == 1
        assert codec.jpeg_tables is None

    def test_flat_dict_without_configuration_key(self):
        """from_dict accepts a flat dict (no 'configuration' wrapper)."""
        codec = TiffTileCodec.from_dict({"compression": 32773})
        assert codec.compression == 32773


class TestTiffTileCodecEncodeRejection:
    """encode() and _encode_single() raise NotImplementedError."""

    def test_encode_raises(self):
        """encode() raises NotImplementedError."""
        codec = TiffTileCodec()
        with pytest.raises(NotImplementedError, match="TiffTileCodec"):
            codec.encode(b"\x00")

    def test_encode_single_raises(self):
        """_encode_single() raises NotImplementedError."""
        codec = TiffTileCodec()
        with pytest.raises(NotImplementedError, match="TiffTileCodec"):
            asyncio.run(codec._encode_single(b"\x00", None))


class TestTiffTileCodecInvalidBase64:
    """Invalid base64 in jpeg_tables raises ValueError."""

    def test_invalid_base64_raises(self):
        """Constructor raises ValueError for non-base64 jpeg_tables."""
        with pytest.raises(ValueError, match="Invalid base64"):
            TiffTileCodec(jpeg_tables="!!!not-valid-base64!!!")

    def test_none_jpeg_tables_is_valid(self):
        """None jpeg_tables does not raise."""
        codec = TiffTileCodec(jpeg_tables=None)
        assert codec.jpeg_tables is None
        assert codec._jpeg_tables_bytes is None
