"""Property-based tests for GeoTIFF metadata roundtrip operations.

This module tests correctness properties for GeoTIFF metadata:
- GeoTIFF metadata write-read round-trip
- Idempotent GeoTIFF encoding

These validate that GeoTIFF metadata (GeoKeys, transformation tags) survives
write-read cycles through TIFFDatasetWriter and TIFFDatasetReader.
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from ..conftest import pbt_settings
from ..strategies import geotiff_metadata


def _normalize_json_value(val):
    """Normalize a JSON value for comparison.

    The Rust json_f64 helper converts whole-number doubles to integers
    (e.g. 0.0 → 0). This function applies the same normalization to
    Python values so roundtrip comparisons work correctly.
    """
    if isinstance(val, float):
        if val == int(val) and not (val != val) and abs(val) < 2**53:
            return int(val)
        return val
    if isinstance(val, list):
        return [_normalize_json_value(v) for v in val]
    return val


def _write_geotiff(path, hints):
    """Write a minimal GeoTIFF with the given metadata hints.

    Creates a small 64x64 single-band UInt8 image with the GeoTIFF
    encoding hints applied via BufferedMetadataProvider.set_json.
    All keys are raw numeric tag IDs (e.g. "34735", "33550").
    """
    metadata = BufferedMetadataProvider()

    # TIFF encoding hints (tile layout) — use numeric tag IDs
    metadata.set("322", "64")    # TileWidth
    metadata.set("323", "64")    # TileLength
    metadata.set("259", "None")  # Compression

    # GeoTIFF encoding hints (raw numeric tags)
    for key, value in hints.items():
        if isinstance(value, str):
            metadata.set(key, value)
        else:
            metadata.set_json(key, value)

    num_rows, num_cols, num_bands = 64, 64, 1
    array = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)

    provider = BufferedImageAssetProvider.create(
        key="image_segment_0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=num_cols,
        block_height=num_rows,
        pixel_type=PixelType.UInt8,
        metadata=metadata,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "tiff")
    writer.metadata = metadata
    writer.add_asset(
        key="image_segment_0",
        provider=provider,
        title="GeoTIFF Test",
        description="Property test",
        roles=["data"],
    )
    writer.close()


# GeoTIFF numeric tag IDs used for read-back verification
_GEOTIFF_TAGS = {"34735", "34736", "34737", "33550", "33922", "34264"}


def _read_geo_metadata(path):
    """Read back GeoTIFF metadata from a TIFF file.

    Returns a dict of only the GeoTIFF-related numeric tag entries from
    the first image segment.
    """
    reader = IO.open([str(path)], "r")
    asset = reader.get_asset("image_segment_0")
    full = asset.get_metadata().as_dict()
    return {k: v for k, v in full.items() if k in _GEOTIFF_TAGS}


# =============================================================================
# GeoTIFF metadata write-read round-trip
# =============================================================================


@pytest.mark.property
class TestGeoTiffMetadataRoundtrip:
    """GeoTIFF metadata write-read round-trip.

    For any valid combination of raw GeoTIFF tags, writing a GeoTIFF
    via TIFFDatasetWriter and reading it back via TIFFDatasetReader
    produces tag values identical to the input.
    """

    @given(geotiff_metadata())
    @pbt_settings
    def test_geotiff_metadata_roundtrip(self, hints):
        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_geotiff(path, hints)
            geo = _read_geo_metadata(path)

            # Every hint tag should appear in the read-back metadata
            for key, expected in hints.items():
                assert key in geo, f"Missing tag {key!r} in read-back metadata"
                actual = geo[key]
                normalized = _normalize_json_value(expected)
                assert actual == normalized, (
                    f"Mismatch for tag {key!r}: expected {normalized!r}, got {actual!r}"
                )
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Idempotent GeoTIFF encoding
# =============================================================================


@pytest.mark.property
class TestGeoTiffIdempotentEncoding:
    """Idempotent GeoTIFF encoding.

    For any valid GeoTIFF file produced by the writer, reading its metadata
    and writing a new file with those metadata values as encoding hints
    produces a file whose GeoTIFF metadata is identical to the original.
    """

    @given(geotiff_metadata())
    @pbt_settings
    def test_geotiff_idempotent_encoding(self, hints):
        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path1 = Path(f.name)
        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path2 = Path(f.name)

        try:
            # First write → read
            _write_geotiff(path1, hints)
            geo1 = _read_geo_metadata(path1)

            # Second write using read-back metadata as hints → read
            _write_geotiff(path2, geo1)
            geo2 = _read_geo_metadata(path2)

            # Metadata from second read must match first read exactly
            assert geo1 == geo2, (
                f"Idempotency violation:\n"
                f"  first read:  {geo1}\n"
                f"  second read: {geo2}"
            )
        finally:
            path1.unlink(missing_ok=True)
            path2.unlink(missing_ok=True)
