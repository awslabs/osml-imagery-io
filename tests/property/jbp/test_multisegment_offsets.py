"""Property-based tests for multi-segment NITF file structure.

This module tests that writing multiple segment types (image, text, graphics,
DES) into a single NITF file produces correct segment counts, asset keys,
and preserves data for each segment through a round-trip.
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, AssetProvider, AssetType, BufferedImageAssetProvider, PixelType
from hypothesis import given
from hypothesis import strategies as st

from ..conftest import pbt_settings


def _create_image_provider(key, seed=0):
    """Create a minimal BufferedImageAssetProvider for testing."""
    provider = BufferedImageAssetProvider.create(
        key=key,
        num_columns=8,
        num_rows=8,
        num_bands=1,
        block_width=8,
        block_height=8,
        pixel_type=PixelType.UInt8,
    )
    data = np.full((1, 8, 8), seed % 256, dtype=np.uint8)
    provider.set_full_image(data)
    return provider

# Strategies for generating segment content
image_data_strategy = st.binary(min_size=1, max_size=256)
text_data_strategy = st.binary(min_size=1, max_size=500)
graphics_data_strategy = st.binary(min_size=1, max_size=500)
des_data_strategy = st.binary(min_size=1, max_size=500)

# Strategy for segment counts (at least 1 image, others optional)
segment_counts = st.fixed_dictionaries({
    "images": st.integers(min_value=1, max_value=3),
    "text": st.integers(min_value=0, max_value=2),
    "graphics": st.integers(min_value=0, max_value=2),
    "des": st.integers(min_value=0, max_value=2),
})


@pytest.mark.property
class TestMultiSegmentOffsets:
    """Property tests for multi-segment NITF files.

    Validates that writing a NITF file with a mix of segment types and
    reading it back preserves the segment counts, keys, and data.
    """

    @given(counts=segment_counts)
    @pbt_settings
    def test_segment_counts_preserved(self, counts):
        """For any combination of segment types written to a NITF file,
        reading it back SHALL return the correct number of asset keys
        per segment type.
        """
        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")

            expected_keys = []

            for i in range(counts["images"]):
                key = f"image:{i}"
                asset = _create_image_provider(key, seed=i)
                writer.add_asset(key, asset, f"Image {i}", "", ["data"])
                expected_keys.append(key)

            for i in range(counts["text"]):
                key = f"text:{i}"
                asset = AssetProvider.from_bytes(
                    key=key,
                    data=f"Text content {i}".encode(),
                    asset_type=AssetType.Text,
                    title=f"Text {i}",
                )
                writer.add_asset(key, asset, f"Text {i}", "", ["metadata"])
                expected_keys.append(key)

            for i in range(counts["graphics"]):
                key = f"graphic:{i}"
                asset = AssetProvider.from_bytes(
                    key=key,
                    data=bytes([i + 10] * 32),
                    asset_type=AssetType.Graphics,
                    title=f"Graphic {i}",
                )
                writer.add_asset(key, asset, f"Graphic {i}", "", ["annotation"])
                expected_keys.append(key)

            for i in range(counts["des"]):
                key = f"des:{i}"
                asset = AssetProvider.from_bytes(
                    key=key,
                    data=bytes([i + 20] * 16),
                    asset_type=AssetType.Data,
                    title=f"DES {i}",
                )
                writer.add_asset(key, asset, f"DES {i}", "", ["metadata"])
                expected_keys.append(key)

            writer.close()

            reader = IO.open([str(path)], "r")
            all_keys = reader.get_asset_keys()

            assert len(all_keys) == len(expected_keys), (
                f"Expected {len(expected_keys)} assets, got {len(all_keys)}"
            )

            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            text_keys = reader.get_asset_keys(asset_type=AssetType.Text)
            graphics_keys = reader.get_asset_keys(asset_type=AssetType.Graphics)
            data_keys = reader.get_asset_keys(asset_type=AssetType.Data)

            assert len(image_keys) == counts["images"]
            assert len(text_keys) == counts["text"]
            assert len(graphics_keys) == counts["graphics"]
            assert len(data_keys) == counts["des"]

            reader.close()

        finally:
            if path.exists():
                path.unlink()

    @given(
        text_bytes=text_data_strategy,
        graphics_bytes=graphics_data_strategy,
        des_bytes=des_data_strategy,
    )
    @pbt_settings
    def test_mixed_segment_data_roundtrip(
        self, text_bytes, graphics_bytes, des_bytes
    ):
        """For any NITF file containing one of each segment type, reading
        back the raw data for text, graphics, and DES segments SHALL return
        bytes identical to what was written. Image segments use
        BufferedImageAssetProvider (raw byte comparison not applicable).
        """
        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")

            # Image segment uses BufferedImageAssetProvider
            image_provider = _create_image_provider("image:0")
            writer.add_asset("image:0", image_provider, "image:0", "", ["data"])

            # Non-image segments use from_bytes
            non_image_segments = [
                ("text:0", text_bytes, AssetType.Text),
                ("graphic:0", graphics_bytes, AssetType.Graphics),
                ("des:0", des_bytes, AssetType.Data),
            ]

            for key, data, asset_type in non_image_segments:
                asset = AssetProvider.from_bytes(
                    key=key, data=data, asset_type=asset_type, title=key,
                )
                writer.add_asset(key, asset, key, "", ["data"])

            writer.close()

            reader = IO.open([str(path)], "r")

            # Verify image segment exists
            image_asset = reader.get_asset("image:0")
            assert image_asset is not None, "Missing asset: image:0"

            # Verify non-image segment data roundtrip
            for key, original_data, _ in non_image_segments:
                asset = reader.get_asset(key)
                assert asset is not None, f"Missing asset: {key}"
                read_data = asset.get_raw_asset().read()
                assert read_data == original_data, (
                    f"Data mismatch for {key}: "
                    f"wrote {len(original_data)} bytes, "
                    f"read {len(read_data)} bytes"
                )

            reader.close()

        finally:
            if path.exists():
                path.unlink()

    @given(counts=segment_counts)
    @pbt_settings
    def test_asset_type_filtering(self, counts):
        """For any multi-segment NITF file, get_asset_keys(asset_type=X)
        SHALL return only keys for segments of type X, and the union of
        all filtered key sets SHALL equal the unfiltered key set.
        """
        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")

            for i in range(counts["images"]):
                key = f"image:{i}"
                asset = _create_image_provider(key, seed=i)
                writer.add_asset(key, asset, key, "", ["data"])

            for i in range(counts["text"]):
                key = f"text:{i}"
                asset = AssetProvider.from_bytes(
                    key=key, data=b"text", asset_type=AssetType.Text, title=key,
                )
                writer.add_asset(key, asset, key, "", ["data"])

            for i in range(counts["graphics"]):
                key = f"graphic:{i}"
                asset = AssetProvider.from_bytes(
                    key=key, data=bytes(16), asset_type=AssetType.Graphics, title=key,
                )
                writer.add_asset(key, asset, key, "", ["data"])

            for i in range(counts["des"]):
                key = f"des:{i}"
                asset = AssetProvider.from_bytes(
                    key=key, data=bytes(8), asset_type=AssetType.Data, title=key,
                )
                writer.add_asset(key, asset, key, "", ["data"])

            writer.close()

            reader = IO.open([str(path)], "r")
            all_keys = set(reader.get_asset_keys())

            filtered_union = set()
            for asset_type in [AssetType.Image, AssetType.Text, AssetType.Graphics, AssetType.Data]:
                typed_keys = reader.get_asset_keys(asset_type=asset_type)
                for key in typed_keys:
                    asset = reader.get_asset(key)
                    assert asset.asset_type == asset_type, (
                        f"Asset {key} has type {asset.asset_type}, expected {asset_type}"
                    )
                filtered_union.update(typed_keys)

            assert filtered_union == all_keys, (
                f"Filtered union {filtered_union} != all keys {all_keys}"
            )

            reader.close()

        finally:
            if path.exists():
                path.unlink()
