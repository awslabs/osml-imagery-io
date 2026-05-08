"""Unit tests for DtedTileCodec and decode_dted_tile binding."""

import struct

import numpy as np
import pytest
from aws.osml.io._io import decode_dted_tile
from aws.osml.io.zarr_codecs import DtedTileCodec

# ---------------------------------------------------------------------------
# Helpers for building synthetic DTED data records
# ---------------------------------------------------------------------------


def _encode_signed_magnitude(value: int) -> bytes:
    """Encode a single i16 value to DTED signed-magnitude big-endian."""
    if value < 0:
        mag = (-value) & 0x7FFF
        return struct.pack(">H", mag | 0x8000)
    return struct.pack(">H", value & 0x7FFF)


def _build_dted_records(elevations: np.ndarray) -> bytes:
    """Build raw DTED data records from a column-major elevation array.

    Parameters
    ----------
    elevations : np.ndarray
        Shape (num_lon_lines, num_lat_points) — column-major, south→north.

    Returns
    -------
    bytes
        Concatenated data records with headers and checksums.
    """
    num_lon_lines, num_lat_points = elevations.shape
    record_size = 8 + num_lat_points * 2 + 4
    buf = bytearray(num_lon_lines * record_size)

    for col in range(num_lon_lines):
        offset = col * record_size
        # Sentinel
        buf[offset] = 0xAA
        # Block count (3 bytes)
        buf[offset + 1] = 0
        buf[offset + 2] = (col >> 8) & 0xFF
        buf[offset + 3] = col & 0xFF
        # Longitude count (2 bytes)
        buf[offset + 4] = 0
        buf[offset + 5] = col & 0xFF
        # Latitude count (2 bytes)
        buf[offset + 6] = 0
        buf[offset + 7] = 0
        # Elevation posts (south→north)
        for post in range(num_lat_points):
            elev_bytes = _encode_signed_magnitude(int(elevations[col, post]))
            pos = offset + 8 + post * 2
            buf[pos] = elev_bytes[0]
            buf[pos + 1] = elev_bytes[1]
        # Checksum: sum of all preceding bytes in the record
        payload = buf[offset:offset + record_size - 4]
        checksum = sum(payload) & 0xFFFFFFFF
        struct.pack_into(">I", buf, offset + record_size - 4, checksum)

    return bytes(buf)


# ---------------------------------------------------------------------------
# Tests for decode_dted_tile binding
# ---------------------------------------------------------------------------


class TestDecodeDtedTile:
    """Tests for the Rust decode_dted_tile binding."""

    def test_basic_decode_shape_and_dtype(self):
        """decode_dted_tile returns correct shape and dtype."""
        num_lat_points = 4
        num_lon_lines = 3
        record_size = 8 + num_lat_points * 2 + 4

        # Create known elevation data (col-major, south→north)
        elevations = np.array([
            [100, 200, 300, 400],
            [500, 600, 700, 800],
            [900, 1000, 1100, 1200],
        ], dtype=np.int16)

        data = _build_dted_records(elevations)
        result = decode_dted_tile(
            data,
            num_lat_points=num_lat_points,
            num_lon_lines=num_lon_lines,
            record_size=record_size,
        )

        assert result.dtype == np.int16
        assert result.shape == (1, 4, 3)

    def test_column_to_row_transpose(self):
        """decode_dted_tile correctly transposes column-major south→north to row-major north→south."""
        num_lat_points = 4
        num_lon_lines = 3
        record_size = 8 + num_lat_points * 2 + 4

        # Column-major (south→north per column):
        #   col 0: [100, 200, 300, 400]
        #   col 1: [500, 600, 700, 800]
        #   col 2: [900, 1000, 1100, 1200]
        elevations = np.array([
            [100, 200, 300, 400],
            [500, 600, 700, 800],
            [900, 1000, 1100, 1200],
        ], dtype=np.int16)

        data = _build_dted_records(elevations)
        result = decode_dted_tile(
            data,
            num_lat_points=num_lat_points,
            num_lon_lines=num_lon_lines,
            record_size=record_size,
        )

        # Expected row-major (north→south):
        #   row 0 (north): [400, 800, 1200]
        #   row 1:         [300, 700, 1100]
        #   row 2:         [200, 600, 1000]
        #   row 3 (south): [100, 500, 900]
        expected = np.array([[[400, 800, 1200],
                              [300, 700, 1100],
                              [200, 600, 1000],
                              [100, 500, 900]]], dtype=np.int16)
        np.testing.assert_array_equal(result, expected)

    def test_signed_magnitude_negative_values(self):
        """decode_dted_tile correctly decodes signed-magnitude negative values."""
        num_lat_points = 2
        num_lon_lines = 2
        record_size = 8 + num_lat_points * 2 + 4

        # Include negative values
        elevations = np.array([
            [-100, 500],
            [200, -300],
        ], dtype=np.int16)

        data = _build_dted_records(elevations)
        result = decode_dted_tile(
            data,
            num_lat_points=num_lat_points,
            num_lon_lines=num_lon_lines,
            record_size=record_size,
        )

        # North→south: post[1] then post[0]
        expected = np.array([[[500, -300],
                              [-100, 200]]], dtype=np.int16)
        np.testing.assert_array_equal(result, expected)

    def test_null_sentinel_value(self):
        """decode_dted_tile decodes 0xFFFF as -32767 (null sentinel)."""
        num_lat_points = 2
        num_lon_lines = 1
        record_size = 8 + num_lat_points * 2 + 4

        # -32767 encodes as 0xFFFF in signed-magnitude
        elevations = np.array([[-32767, 100]], dtype=np.int16)

        data = _build_dted_records(elevations)
        result = decode_dted_tile(
            data,
            num_lat_points=num_lat_points,
            num_lon_lines=num_lon_lines,
            record_size=record_size,
        )

        expected = np.array([[[100], [-32767]]], dtype=np.int16)
        np.testing.assert_array_equal(result, expected)

    def test_data_size_mismatch_raises(self):
        """decode_dted_tile raises ValueError on wrong data length."""
        with pytest.raises(ValueError, match="Data size mismatch"):
            decode_dted_tile(
                b"\x00" * 10,
                num_lat_points=4,
                num_lon_lines=3,
                record_size=20,
            )

    def test_trim_parameters(self):
        """decode_dted_tile applies trim correctly."""
        num_lat_points = 4
        num_lon_lines = 4
        record_size = 8 + num_lat_points * 2 + 4

        # 4x4 grid with distinct values
        elevations = np.arange(16, dtype=np.int16).reshape(4, 4)
        data = _build_dted_records(elevations)

        # Trim 1 from each edge
        result = decode_dted_tile(
            data,
            num_lat_points=num_lat_points,
            num_lon_lines=num_lon_lines,
            record_size=record_size,
            trim_top=1,
            trim_bottom=1,
            trim_left=1,
            trim_right=1,
        )

        assert result.shape == (1, 2, 2)

    def test_trim_exceeds_dimensions_raises(self):
        """decode_dted_tile raises ValueError when trim exceeds dimensions."""
        num_lat_points = 4
        num_lon_lines = 3
        record_size = 8 + num_lat_points * 2 + 4

        elevations = np.zeros((3, 4), dtype=np.int16)
        data = _build_dted_records(elevations)

        with pytest.raises(ValueError, match="trim_top.*trim_bottom.*num_lat_points"):
            decode_dted_tile(
                data,
                num_lat_points=num_lat_points,
                num_lon_lines=num_lon_lines,
                record_size=record_size,
                trim_top=2,
                trim_bottom=3,
            )


# ---------------------------------------------------------------------------
# Tests for DtedTileCodec class
# ---------------------------------------------------------------------------


class TestDtedTileCodec:
    """Tests for the DtedTileCodec zarr codec class."""

    def test_codec_is_bytes_bytes_codec(self):
        """DtedTileCodec is a subclass of zarr.abc.codec.BytesBytesCodec."""
        from zarr.abc.codec import BytesBytesCodec

        assert issubclass(DtedTileCodec, BytesBytesCodec)

    def test_entry_point_resolves_dted(self):
        """Zarr codec registry resolves the DTED URI to DtedTileCodec."""
        from zarr.registry import get_codec_class

        cls = get_codec_class("https://awslabs.github.io/osml-imagery-io/codecs/dted")
        assert cls is DtedTileCodec

    def test_config_round_trip(self):
        """DtedTileCodec serialization round-trip preserves configuration."""
        codec = DtedTileCodec(
            num_lat_points=3601,
            num_lon_lines=3601,
            record_size=7214,
            trim_top=0,
            trim_bottom=1,
            trim_left=0,
            trim_right=1,
        )
        d = codec.to_dict()
        codec2 = DtedTileCodec.from_dict(d)
        assert codec2.to_dict() == d
        assert codec2.num_lat_points == 3601
        assert codec2.trim_bottom == 1

    def test_from_dict_with_defaults(self):
        """DtedTileCodec.from_dict uses defaults for missing fields."""
        codec = DtedTileCodec.from_dict({"configuration": {}})
        assert codec.num_lat_points == 1201
        assert codec.num_lon_lines == 1201
        assert codec.record_size == 2414
        assert codec.trim_top == 0

    def test_encode_raises(self):
        """DtedTileCodec.encode raises NotImplementedError."""
        codec = DtedTileCodec()
        with pytest.raises(NotImplementedError, match="DtedTileCodec"):
            codec.encode(b"\x00")

    def test_async_encode_raises(self):
        """DtedTileCodec._encode_single raises NotImplementedError."""
        import asyncio

        codec = DtedTileCodec()
        with pytest.raises(NotImplementedError, match="DtedTileCodec"):
            asyncio.run(codec._encode_single(b"\x00", None))

    def test_numcodecs_decode(self):
        """DtedTileCodec.decode (numcodecs path) returns correct array."""
        num_lat_points = 3
        num_lon_lines = 2
        record_size = 8 + num_lat_points * 2 + 4

        elevations = np.array([
            [10, 20, 30],
            [40, 50, 60],
        ], dtype=np.int16)

        data = _build_dted_records(elevations)
        codec = DtedTileCodec(
            num_lat_points=num_lat_points,
            num_lon_lines=num_lon_lines,
            record_size=record_size,
        )

        result = codec.decode(data)
        assert result.dtype == np.int16
        assert result.shape == (1, 3, 2)

        # North→south: post[2], post[1], post[0]
        expected = np.array([[[30, 60],
                              [20, 50],
                              [10, 40]]], dtype=np.int16)
        np.testing.assert_array_equal(result, expected)

    def test_get_config_and_from_config(self):
        """get_config/from_config round-trip for numcodecs."""
        codec = DtedTileCodec(
            num_lat_points=1201,
            num_lon_lines=1201,
            record_size=2414,
            trim_top=1,
            trim_right=1,
        )
        config = codec.get_config()
        assert config["id"] == "https://awslabs.github.io/osml-imagery-io/codecs/dted"
        codec2 = DtedTileCodec.from_config(config)
        assert codec2.trim_top == 1
        assert codec2.trim_right == 1


# ---------------------------------------------------------------------------
# Tests for VirtualiZarr integration
# ---------------------------------------------------------------------------


class TestDtedVirtualizarrIntegration:
    """Tests verifying the VirtualiZarr parser works with DTED files."""

    def test_build_codec_instance_dted(self):
        """_build_codec_instance returns DtedTileCodec for DTED assets."""
        from unittest.mock import MagicMock

        from aws.osml.io.virtualizarr_parsers import _build_codec_instance

        asset = MagicMock()
        asset.num_bands = 1
        asset.num_pixels_per_block_vertical = 1201
        asset.num_pixels_per_block_horizontal = 1201
        asset.num_bits_per_pixel = 16
        asset.codec_configuration.return_value = {
            "dted_codec": b"",
            "num_lat_points": b"\xb1\x04\x00\x00",  # 1201 LE
            "num_lon_lines": b"\xb1\x04\x00\x00",   # 1201 LE
            "record_size": b"\x6e\x09\x00\x00",     # 2414 LE
        }

        codec = _build_codec_instance(asset)
        assert isinstance(codec, DtedTileCodec)
        assert codec.num_lat_points == 1201
        assert codec.num_lon_lines == 1201
        assert codec.record_size == 2414

    def test_dted_file_produces_codec_configuration(self):
        """A real DTED file opened via IO provides codec_configuration for VirtualiZarr."""
        from pathlib import Path

        from aws.osml.io import IO, AssetType

        test_file = Path("data/unit/dted/n38w109.dt1")
        if not test_file.exists():
            pytest.skip("DTED test data not available")

        reader = IO.open([str(test_file)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(keys[0])

        codec_config = asset.codec_configuration()
        assert codec_config is not None
        assert "dted_codec" in codec_config

        byte_ranges = asset.tile_byte_ranges()
        assert byte_ranges is not None
        assert (0, 0) in byte_ranges

        reader.close()
