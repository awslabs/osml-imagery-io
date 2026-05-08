"""Property-based tests for DTED read/write roundtrip.

Tests:
- Lossless roundtrip: encode(decode(data)) == data for all valid elevations
- Elevation range preserved through signed-magnitude encoding
- Null sentinel values (-32767) preserved through roundtrip
- Checksum validity after write
"""

import tempfile
from pathlib import Path
from typing import Tuple

import numpy as np
import pytest
from aws.osml.io import IO, AssetType, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
from hypothesis import given
from hypothesis import strategies as st
from hypothesis.extra.numpy import arrays

from ..conftest import pbt_settings

# ---------------------------------------------------------------------------
# DTED dimension lookup
# ---------------------------------------------------------------------------

DTED_DIMENSIONS = {
    (0, "I"): (121, 121),
    (0, "II"): (121, 61),
    (0, "III"): (121, 41),
    (0, "IV"): (121, 31),
    (0, "V"): (121, 21),
    (1, "I"): (1201, 1201),
    (1, "II"): (1201, 601),
    (1, "III"): (1201, 401),
    (1, "IV"): (1201, 301),
    (1, "V"): (1201, 201),
    (2, "I"): (3601, 3601),
    (2, "II"): (3601, 1801),
    (2, "III"): (3601, 1201),
    (2, "IV"): (3601, 901),
    (2, "V"): (3601, 601),
}


# ---------------------------------------------------------------------------
# Hypothesis strategies
# ---------------------------------------------------------------------------


@st.composite
def dted_writable_image(draw, use_small_dims: bool = True) -> Tuple[np.ndarray, dict]:
    """Generate a valid DTED elevation grid with metadata.

    When use_small_dims=True (default), generates small grids for fast testing.
    When False, generates full DTED-spec dimensions (slow).
    """
    if use_small_dims:
        num_lat_points = draw(st.integers(min_value=3, max_value=32))
        num_lon_lines = draw(st.integers(min_value=3, max_value=32))
    else:
        level = draw(st.sampled_from([0, 1]))
        zone = draw(st.sampled_from(["I", "II", "III", "IV", "V"]))
        num_lat_points, num_lon_lines = DTED_DIMENSIONS[(level, zone)]

    array = draw(arrays(
        dtype=np.int16,
        shape=(1, num_lat_points, num_lon_lines),
        elements=st.integers(min_value=-12000, max_value=9000),
    ))

    if draw(st.booleans()):
        null_count = draw(st.integers(min_value=1, max_value=max(1, num_lat_points * num_lon_lines // 10)))
        null_indices = draw(st.lists(
            st.tuples(
                st.integers(0, num_lat_points - 1),
                st.integers(0, num_lon_lines - 1),
            ),
            min_size=null_count,
            max_size=null_count,
        ))
        for r, c in null_indices:
            array[0, r, c] = -32767

    origin_lat = draw(st.floats(min_value=-90.0, max_value=89.0, allow_nan=False, allow_infinity=False))
    origin_lon = draw(st.floats(min_value=-180.0, max_value=179.0, allow_nan=False, allow_infinity=False))

    metadata = {
        "dted:level": "DTED1",
        "dted:origin_latitude": origin_lat,
        "dted:origin_longitude": origin_lon,
        "dted:latitude_interval": 30,
        "dted:longitude_interval": 30,
        "dted:security_code": "U",
        "dted:vertical_datum": "MSL",
        "dted:horizontal_datum": "WGS84",
        "dted:producer_code": "US",
        "dted:edition_number": "01",
        "dted:compilation_date": "0101",
        "dted:partial_cell_indicator": "00",
        "dted:absolute_horizontal_accuracy": "0050",
        "dted:absolute_vertical_accuracy": "0030",
        "dted:relative_vertical_accuracy": "0020",
        "dted:vertical_accuracy": 20,
    }

    return (array, metadata)


# ---------------------------------------------------------------------------
# Helper
# ---------------------------------------------------------------------------


def write_and_read_dted(array: np.ndarray, metadata: dict) -> np.ndarray:
    """Write a DTED file and read it back, returning the decoded image."""
    num_lat_points = array.shape[1]
    num_lon_lines = array.shape[2]

    with tempfile.NamedTemporaryFile(suffix=".dt1", delete=False) as f:
        path = Path(f.name)

    try:
        meta = BufferedMetadataProvider()
        for k, v in metadata.items():
            if isinstance(v, (int, float)):
                meta.set_json(k, v)
            else:
                meta.set(k, str(v))

        provider = BufferedImageAssetProvider.create(
            key="elevation",
            num_columns=num_lon_lines,
            num_rows=num_lat_points,
            num_bands=1,
            block_width=num_lon_lines,
            block_height=num_lat_points,
            pixel_type=PixelType.Int16,
            metadata=meta,
        )
        provider.set_full_image(array)

        writer = IO.open([str(path)], "w", "dted")
        writer.metadata = meta
        writer.add_asset(
            key="elevation",
            provider=provider,
            title="Elevation",
            description="DTED elevation data",
            roles=["data"],
        )
        writer.close()

        reader = IO.open([str(path)], "r")
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(keys[0])
        block = asset.get_block(0, 0, 0)
        reader.close()

        return block
    finally:
        if path.exists():
            path.unlink()


# ---------------------------------------------------------------------------
# Property Tests
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestDtedRoundtripLossless:
    """DTED write→read roundtrip is lossless for valid elevation data."""

    @given(dted_writable_image(use_small_dims=True))
    @pbt_settings
    def test_pixel_roundtrip_lossless(self, image_tuple):
        """All elevation values survive a write-read cycle exactly."""
        array, metadata = image_tuple
        decoded = write_and_read_dted(array, metadata)
        np.testing.assert_array_equal(decoded, array)

    @given(
        arrays(
            dtype=np.int16,
            shape=(1, 5, 5),
            elements=st.integers(min_value=-12000, max_value=9000),
        )
    )
    @pbt_settings
    def test_elevation_range_preserved(self, array):
        """Values in the valid DTED range survive signed-magnitude encoding."""
        metadata = {
            "dted:level": "DTED1",
            "dted:origin_latitude": 38.0,
            "dted:origin_longitude": -109.0,
            "dted:latitude_interval": 30,
            "dted:longitude_interval": 30,
            "dted:security_code": "U",
            "dted:vertical_datum": "MSL",
            "dted:horizontal_datum": "WGS84",
            "dted:producer_code": "US",
            "dted:edition_number": "01",
            "dted:compilation_date": "0101",
            "dted:partial_cell_indicator": "00",
            "dted:absolute_horizontal_accuracy": "0050",
            "dted:absolute_vertical_accuracy": "0030",
            "dted:relative_vertical_accuracy": "0020",
            "dted:vertical_accuracy": 20,
        }
        decoded = write_and_read_dted(array, metadata)
        np.testing.assert_array_equal(decoded, array)

    @given(st.integers(min_value=3, max_value=16), st.integers(min_value=3, max_value=16))
    @pbt_settings
    def test_null_values_preserved(self, rows, cols):
        """Null sentinel values (-32767) are preserved through roundtrip."""
        array = np.full((1, rows, cols), -32767, dtype=np.int16)
        metadata = {
            "dted:level": "DTED1",
            "dted:origin_latitude": 0.0,
            "dted:origin_longitude": 0.0,
            "dted:latitude_interval": 30,
            "dted:longitude_interval": 30,
            "dted:security_code": "U",
            "dted:vertical_datum": "MSL",
            "dted:horizontal_datum": "WGS84",
            "dted:producer_code": "US",
            "dted:edition_number": "01",
            "dted:compilation_date": "0101",
            "dted:partial_cell_indicator": "00",
            "dted:absolute_horizontal_accuracy": "0050",
            "dted:absolute_vertical_accuracy": "0030",
            "dted:relative_vertical_accuracy": "0020",
            "dted:vertical_accuracy": 20,
        }
        decoded = write_and_read_dted(array, metadata)
        np.testing.assert_array_equal(decoded, array)


@pytest.mark.property
class TestDtedChecksumValidity:
    """Written DTED files have valid per-record checksums."""

    @given(dted_writable_image(use_small_dims=True))
    @pbt_settings
    def test_checksum_valid_after_write(self, image_tuple):
        """All record checksums pass verification in written files."""
        import struct

        array, metadata = image_tuple
        num_lat_points = array.shape[1]
        num_lon_lines = array.shape[2]

        with tempfile.NamedTemporaryFile(suffix=".dt1", delete=False) as f:
            path = Path(f.name)

        try:
            meta = BufferedMetadataProvider()
            for k, v in metadata.items():
                if isinstance(v, (int, float)):
                    meta.set_json(k, v)
                else:
                    meta.set(k, str(v))

            provider = BufferedImageAssetProvider.create(
                key="elevation",
                num_columns=num_lon_lines,
                num_rows=num_lat_points,
                num_bands=1,
                block_width=num_lon_lines,
                block_height=num_lat_points,
                pixel_type=PixelType.Int16,
                metadata=meta,
            )
            provider.set_full_image(array)

            writer = IO.open([str(path)], "w", "dted")
            writer.metadata = meta
            writer.add_asset(
                key="elevation",
                provider=provider,
                title="Elevation",
                description="DTED",
                roles=["data"],
            )
            writer.close()

            data = path.read_bytes()

            # Verify structure
            assert data[:3] == b"UHL"
            assert data[80:83] == b"DSI"
            assert data[80 + 648:80 + 648 + 3] == b"ACC"

            # Verify each data record checksum
            data_offset = 80 + 648 + 2700
            record_size = 8 + num_lat_points * 2 + 4

            for col in range(num_lon_lines):
                rec_start = data_offset + col * record_size
                record = data[rec_start:rec_start + record_size]
                assert record[0] == 0xAA, f"Record {col} missing sentinel"

                payload = record[:-4]
                stored_checksum = struct.unpack(">I", record[-4:])[0]
                computed_checksum = sum(payload) & 0xFFFFFFFF
                assert computed_checksum == stored_checksum, (
                    f"Checksum mismatch at record {col}: "
                    f"computed={computed_checksum}, stored={stored_checksum}"
                )
        finally:
            if path.exists():
                path.unlink()
