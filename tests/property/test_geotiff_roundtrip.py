"""Property-based tests for GeoTIFF metadata roundtrip operations.

This module tests correctness properties for GeoTIFF metadata:
- Property 1: GeoTIFF metadata write-read round-trip
- Property 2: Idempotent GeoTIFF encoding

These validate that GeoTIFF metadata (GeoKeys, transformation tags) survives
write-read cycles through TIFFDatasetWriter and TIFFDatasetReader.

**Validates: Requirements 1.1–1.5, 3.1–3.5, 4.1–4.7, 5.1–5.3, 6.1–6.4,
7.1–7.8, 8.1–8.5, 9.1–9.6**
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from .strategies import geotiff_metadata


pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


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
    """
    metadata = BufferedMetadataProvider()

    # TIFF encoding hints (tile layout)
    metadata.set("TileWidth", "64")
    metadata.set("TileHeight", "64")
    metadata.set("Compression", "None")

    # GeoTIFF encoding hints
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


def _read_geo_metadata(path):
    """Read back Geo-prefixed metadata from a TIFF file.

    Returns a dict of only the Geo-prefixed metadata fields from the
    first image segment.
    """
    reader = IO.open([str(path)], "r")
    asset = reader.get_asset("image_segment_0")
    return asset.get_metadata().as_dict("Geo")


# =============================================================================
# Property 1: GeoTIFF metadata write-read round-trip
# =============================================================================


@pytest.mark.property
class TestGeoTiffMetadataRoundtrip:
    """Property 1: GeoTIFF metadata write-read round-trip

    For any valid combination of GeoTIFF metadata fields, writing a GeoTIFF
    via TIFFDatasetWriter with those fields as encoding hints and reading it
    back via TIFFDatasetReader produces metadata fields with identical values.

    # Feature: geotiff-metadata, Property 1: GeoTIFF metadata write-read round-trip
    **Validates: Requirements 1.1–1.5, 3.1–3.5, 4.1–4.7, 5.1–5.3, 6.1–6.4,
    7.1–7.8, 8.1–8.4, 9.1–9.6**
    """

    @given(geotiff_metadata())
    @pbt_settings
    def test_geotiff_metadata_roundtrip(self, hints):
        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_geotiff(path, hints)
            geo = _read_geo_metadata(path)

            # Every Geo-prefixed hint should appear in the read-back metadata
            for key, expected in hints.items():
                assert key in geo, f"Missing key {key!r} in read-back metadata"
                actual = geo[key]
                normalized = _normalize_json_value(expected)
                assert actual == normalized, (
                    f"Mismatch for {key!r}: expected {normalized!r}, got {actual!r}"
                )

            # All read-back keys should be Geo-prefixed
            for key in geo:
                assert key.startswith("Geo"), (
                    f"Non-Geo key {key!r} in Geo-filtered metadata"
                )
        finally:
            path.unlink(missing_ok=True)



# =============================================================================
# Property 2: Idempotent GeoTIFF encoding
# =============================================================================


@pytest.mark.property
class TestGeoTiffIdempotentEncoding:
    """Property 2: Idempotent GeoTIFF encoding

    For any valid GeoTIFF file produced by the writer, reading its metadata
    and writing a new file with those metadata values as encoding hints
    produces a file whose GeoTIFF metadata is identical to the original.

    # Feature: geotiff-metadata, Property 2: Idempotent GeoTIFF encoding
    **Validates: Requirements 8.5**
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
