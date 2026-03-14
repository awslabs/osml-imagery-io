"""Property-based tests for TIFF writing roundtrip operations.

This module tests correctness properties specific to the TIFFDatasetWriter:
- Property 1: Lossless Pixel Roundtrip
- Property 2: Metadata Roundtrip
- Property 3: Idempotent Encoding
- Property 5: Non-Image Asset Rejection

These are TIFF-specific because they exercise TIFF encoding hints, TIFF tags,
planar configuration, and the TIFF-only restriction on non-image assets.

**Validates: Requirements 1.2, 2.1, 2.3, 3.1–3.7, 4.1, 4.3, 4.5–4.7,
4.9–4.11, 5.1, 5.4, 6.1, 6.2, 6.4, 7.1–7.3, 9.1–9.4**
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase, assume

from aws.osml.io import (
    IO,
    AssetProvider,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from .helpers import read_full_image
from .strategies import (
    get_numpy_dtype,
    tiff_writable_image,
    tiff_encoding_hints,
    tiff_writer_pixel_types,
    image_dimensions,
    band_counts,
    image_arrays,
)


pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


def _write_tiff(path, array, pixel_type, num_bands, num_rows, num_cols, hints):
    """Write a TIFF file using TIFFDatasetWriter via IO.open.

    Args:
        path: Output file path.
        array: CHW numpy array.
        pixel_type: PixelType enum.
        num_bands: Number of bands.
        num_rows: Number of rows.
        num_cols: Number of columns.
        hints: Dict of encoding hint strings (may be empty).
    """
    metadata = BufferedMetadataProvider()
    for k, v in hints.items():
        metadata.set(k, v)

    tile_w = int(hints.get("TileWidth", "256"))
    tile_h = int(hints.get("TileHeight", "256"))

    provider = BufferedImageAssetProvider.create(
        key="image_segment_0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, tile_w),
        block_height=min(num_rows, tile_h),
        pixel_type=pixel_type,
        metadata=metadata,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "tiff")
    writer.metadata = metadata
    writer.add_asset(
        key="image_segment_0",
        provider=provider,
        title="Test Image",
        description="Property test",
        roles=["data"],
    )
    writer.close()


# =============================================================================
# Property 1: Lossless Pixel Roundtrip
# =============================================================================


@pytest.mark.property
class TestTiffLosslessPixelRoundtrip:
    """Property 1: TIFF Lossless Pixel Roundtrip

    For any supported pixel type (UInt8–Float64), band count, image
    dimensions, lossless compression (None/LZW/Deflate), and planar
    configuration (Chunky/Planar), writing via TIFFDatasetWriter and
    reading back produces pixel data identical to the original input.

    # Feature: tiff-writing, Property 1: Lossless pixel roundtrip
    **Validates: Requirements 1.2, 2.1, 5.1, 5.4, 6.1, 6.2, 7.1, 7.3, 9.1, 9.2**
    """

    @given(tiff_writable_image(min_size=16, max_size=64, min_bands=1, max_bands=4))
    @pbt_settings
    def test_lossless_pixel_roundtrip(self, image_tuple):
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_tiff(path, array, pixel_type, num_bands, num_rows, num_cols, hints)

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")

            assert asset.num_columns == num_cols
            assert asset.num_rows == num_rows
            assert asset.num_bands == num_bands

            decoded = read_full_image(asset, num_bands, num_rows, num_cols)

            assert decoded.shape == array.shape, (
                f"Shape mismatch: expected {array.shape}, got {decoded.shape}"
            )
            assert decoded.dtype == array.dtype, (
                f"Dtype mismatch: expected {array.dtype}, got {decoded.dtype}"
            )
            np.testing.assert_array_equal(decoded, array)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 2: Metadata Roundtrip
# =============================================================================

# Mapping from hint strings to expected TIFF tag integer values
_COMPRESSION_TAG = {"None": 1, "LZW": 5, "Deflate": 8}
_PREDICTOR_TAG = {"None": 1, "Horizontal": 2}
_PLANAR_TAG = {"Chunky": 1, "Planar": 2}

# SampleFormat: unsigned=1, signed=2, float=3
# PixelType enums are not hashable, so use int() as keys
_SAMPLE_FORMAT = {
    int(PixelType.UInt8): 1, int(PixelType.UInt16): 1, int(PixelType.UInt32): 1,
    int(PixelType.Int8): 2, int(PixelType.Int16): 2, int(PixelType.Int32): 2,
    int(PixelType.Float32): 3, int(PixelType.Float64): 3,
}


@pytest.mark.property
class TestTiffMetadataRoundtrip:
    """Property 2: TIFF Metadata Roundtrip

    For any valid combination of encoding hints and image properties,
    writing a TIFF and reading back its per-IFD metadata shall report
    tag values matching the hints and image properties.

    # Feature: tiff-writing, Property 2: Metadata roundtrip
    **Validates: Requirements 3.1–3.7, 4.1, 4.3, 4.5–4.7, 4.9–4.11,
    6.4, 7.2, 9.3**
    """

    @given(tiff_writable_image(min_size=16, max_size=64, min_bands=1, max_bands=4))
    @pbt_settings
    def test_metadata_roundtrip(self, image_tuple):
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_tiff(path, array, pixel_type, num_bands, num_rows, num_cols, hints)

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            meta = asset.get_metadata().as_dict()

            # Image dimension tags
            assert meta["ImageWidth"] == num_cols
            assert meta["ImageLength"] == num_rows

            # Pixel format tags
            dtype = get_numpy_dtype(pixel_type)
            assert meta["BitsPerSample"] == dtype.itemsize * 8
            assert meta["SamplesPerPixel"] == num_bands
            assert meta["SampleFormat"] == _SAMPLE_FORMAT[int(pixel_type)]

            # Photometric interpretation
            expected_photo = 2 if num_bands >= 3 else 1
            assert meta["PhotometricInterpretation"] == expected_photo

            # Encoding hint tags
            assert meta["Compression"] == _COMPRESSION_TAG[hints["Compression"]]

            tile_w = int(hints["TileWidth"])
            tile_h = int(hints["TileHeight"])
            assert meta["TileWidth"] == tile_w
            assert meta["TileLength"] == tile_h

            # Predictor tag is only reported by libtiff when explicitly set
            # to a non-default value (i.e. Horizontal=2). When Predictor is
            # "None" (1) or compression is uncompressed, the tag may be absent.
            if hints["Predictor"] == "Horizontal" and hints["Compression"] != "None":
                assert meta.get("Predictor") == _PREDICTOR_TAG[hints["Predictor"]]

            assert meta["PlanarConfiguration"] == _PLANAR_TAG[hints["PlanarConfiguration"]]
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 5: Non-Image Asset Rejection
# =============================================================================


@pytest.mark.property
class TestTiffNonImageAssetRejection:
    """Property 5: TIFF Non-Image Asset Rejection

    For any asset whose asset_type() is Text or Data, calling add_asset()
    on a TIFFDatasetWriter shall raise an error. TIFF only supports images.

    # Feature: tiff-writing, Property 5: Non-image asset rejection
    **Validates: Requirements 2.3**
    """

    def test_text_asset_rejected(self):
        writer = IO.open(["reject_text.tif"], "w", "tiff")
        asset = AssetProvider.from_bytes(
            key="text_segment_0",
            data=b"Hello world",
            asset_type=AssetType.Text,
            title="Text",
        )
        with pytest.raises(Exception):
            writer.add_asset("text_segment_0", asset, "Text", "desc", ["metadata"])

    def test_data_asset_rejected(self):
        writer = IO.open(["reject_data.tif"], "w", "tiff")
        asset = AssetProvider.from_bytes(
            key="des_segment_0",
            data=b"\x00\x01\x02",
            asset_type=AssetType.Data,
            title="DES",
        )
        with pytest.raises(Exception):
            writer.add_asset("des_segment_0", asset, "DES", "desc", ["data"])


# =============================================================================
# Property 3: Idempotent Encoding
# =============================================================================


@pytest.mark.property
class TestTiffIdempotentEncoding:
    """Property 3: TIFF Idempotent Encoding

    For any valid image and encoding configuration, write → read → write → read
    yields the same pixel values as write → read.

    # Feature: tiff-writing, Property 3: Idempotent encoding
    **Validates: Requirements 9.4**
    """

    @given(tiff_writable_image(min_size=16, max_size=48, min_bands=1, max_bands=3))
    @pbt_settings
    def test_idempotent_encoding(self, image_tuple):
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path1 = Path(f.name)
        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path2 = Path(f.name)

        try:
            # First write → read
            _write_tiff(path1, array, pixel_type, num_bands, num_rows, num_cols, hints)
            reader1 = IO.open([str(path1)], "r")
            asset1 = reader1.get_asset("image_segment_0")
            decoded1 = read_full_image(asset1, num_bands, num_rows, num_cols)

            # Second write → read (using decoded1 as input)
            _write_tiff(path2, decoded1, pixel_type, num_bands, num_rows, num_cols, hints)
            reader2 = IO.open([str(path2)], "r")
            asset2 = reader2.get_asset("image_segment_0")
            decoded2 = read_full_image(asset2, num_bands, num_rows, num_cols)

            np.testing.assert_array_equal(decoded2, decoded1)
        finally:
            path1.unlink(missing_ok=True)
            path2.unlink(missing_ok=True)
