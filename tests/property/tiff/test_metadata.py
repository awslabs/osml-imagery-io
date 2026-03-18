"""Property-based tests for TIFF metadata roundtrip operations.

This module tests:
- Encoding hints → TIFF tag metadata roundtrip
- Field type roundtrip (custom tags survive write → read)
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)
from hypothesis import given
from hypothesis import strategies as st

from ..conftest import pbt_settings
from ..strategies import (
    get_numpy_dtype,
    tiff_writable_image,
)

# Mapping from hint strings to expected TIFF tag integer values
_COMPRESSION_TAG = {"None": 1, "LZW": 5, "Deflate": 8}
_PREDICTOR_TAG = {"None": 1, "Horizontal": 2}
_PLANAR_TAG = {"Chunky": 1, "Planar": 2}

_SAMPLE_FORMAT = {
    int(PixelType.UInt8): 1, int(PixelType.UInt16): 1, int(PixelType.UInt32): 1,
    int(PixelType.Int8): 2, int(PixelType.Int16): 2, int(PixelType.Int32): 2,
    int(PixelType.Float32): 3, int(PixelType.Float64): 3,
}


def _write_tiff(path, array, pixel_type, num_bands, num_rows, num_cols, hints):
    """Write a TIFF file using TIFFDatasetWriter via IO.open."""
    metadata = BufferedMetadataProvider()
    for k, v in hints.items():
        metadata.set(k, v)

    tile_w = int(hints.get("322", "256"))   # TileWidth
    tile_h = int(hints.get("323", "256"))   # TileLength

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
# Encoding hints → TIFF tag metadata roundtrip
# =============================================================================


@pytest.mark.property
class TestTiffMetadataRoundtrip:
    """TIFF metadata roundtrip.

    For any valid combination of encoding hints and image properties,
    writing a TIFF and reading back its per-IFD metadata shall report
    tag values matching the hints and image properties.
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

            assert meta["256"] == num_cols       # ImageWidth
            assert meta["257"] == num_rows       # ImageLength

            dtype = get_numpy_dtype(pixel_type)
            expected_bps = dtype.itemsize * 8
            bps = meta["258"]
            if isinstance(bps, list):
                assert all(b == expected_bps for b in bps)
            else:
                assert bps == expected_bps

            assert meta["277"] == num_bands  # SamplesPerPixel

            sf = meta["339"]  # SampleFormat
            expected_sf = _SAMPLE_FORMAT[int(pixel_type)]
            if isinstance(sf, list):
                assert all(s == expected_sf for s in sf)
            else:
                assert sf == expected_sf

            expected_photo = 2 if num_bands >= 3 else 1
            assert meta["262"] == expected_photo

            assert meta["259"] == _COMPRESSION_TAG[hints["259"]]

            # Tile dimensions are passed through to libtiff as-is (they are
            # already multiples of 16 from the encoding hints strategy).
            # They may exceed image dimensions — libtiff stores them verbatim.
            assert meta["322"] == int(hints["322"])
            assert meta["323"] == int(hints["323"])

            if hints["317"] == "Horizontal" and hints["259"] != "None":
                assert meta.get("317") == _PREDICTOR_TAG[hints["317"]]

            assert meta["284"] == _PLANAR_TAG[hints["284"]]
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Custom tag metadata helpers
# =============================================================================

CUSTOM_TAG_MIN = 65000
CUSTOM_TAG_MAX = 65499


def _write_tiff_with_metadata(path, metadata_dict):
    """Write a minimal 16x16 TIFF with the given metadata tags."""
    meta = BufferedMetadataProvider()
    meta.set("322", "256")   # TileWidth
    meta.set("323", "256")   # TileLength
    meta.set("259", "None")  # Compression
    meta.set("284", "Chunky")  # PlanarConfiguration

    for k, v in metadata_dict.items():
        meta.set_json(k, v)

    array = np.zeros((1, 16, 16), dtype=np.uint8)
    provider = BufferedImageAssetProvider.create(
        key="image_segment_0",
        num_columns=16,
        num_rows=16,
        num_bands=1,
        block_width=16,
        block_height=16,
        pixel_type=PixelType.UInt8,
        metadata=meta,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "tiff")
    writer.metadata = meta
    writer.add_asset(
        key="image_segment_0",
        provider=provider,
        title="Test",
        description="Property test",
        roles=["data"],
    )
    writer.close()


def _read_tiff_metadata(path):
    """Read per-IFD metadata from a TIFF file."""
    reader = IO.open([str(path)], "r")
    asset = reader.get_asset("image_segment_0")
    return asset.get_metadata().as_dict()


# =============================================================================
# Hypothesis strategies for field type roundtrip
# =============================================================================


def _custom_tag_number():
    return st.integers(min_value=CUSTOM_TAG_MIN, max_value=CUSTOM_TAG_MAX)


def _ascii_value():
    return st.text(
        alphabet=st.characters(
            whitelist_categories=("L", "N", "P", "Z"), max_codepoint=126
        ),
        min_size=1, max_size=30,
    )


def _long_value():
    return st.integers(min_value=0, max_value=2**31 - 1)


def _slong_value():
    return st.integers(min_value=-(2**31), max_value=-1)


def _short_array_value():
    return st.lists(
        st.integers(min_value=0, max_value=65535), min_size=1, max_size=6
    )


def _sshort_array_value():
    return st.lists(
        st.integers(min_value=-32768, max_value=32767), min_size=2, max_size=6
    ).filter(lambda arr: any(x < 0 for x in arr))


def _double_array_value():
    return st.lists(
        st.floats(
            min_value=-1e10, max_value=1e10,
            allow_nan=False, allow_infinity=False,
        ),
        min_size=1, max_size=6,
    )


def _double_scalar_value():
    return st.floats(
        min_value=-1e10, max_value=1e10,
        allow_nan=False, allow_infinity=False,
    )


def _annotated_short_value():
    return st.integers(min_value=0, max_value=65535).map(
        lambda v: {"value": v, "type": 3}
    )


FIELD_TYPE_STRATEGIES = [
    ("ascii", _ascii_value, lambda w, r: r == w),
    ("long", _long_value, lambda w, r: r == w),
    ("slong", _slong_value, lambda w, r: r == w),
    (
        "short_array",
        _short_array_value,
        lambda w, r: isinstance(r, (list, int)) and (
            ([r] == w if isinstance(r, int) else r == w)
        ),
    ),
    (
        "sshort_array",
        _sshort_array_value,
        lambda w, r: isinstance(r, list) and r == w,
    ),
    (
        "double_array",
        _double_array_value,
        lambda w, r: (isinstance(r, list)
            and len(r) == len(w)
            and all(abs(float(a) - float(b)) < 1e-6 for a, b in zip(w, r)))
        or (
            isinstance(r, (int, float)) and len(w) == 1
            and abs(float(w[0]) - float(r)) < 1e-6
        ),
    ),
    (
        "double_scalar",
        _double_scalar_value,
        lambda w, r: isinstance(r, (int, float))
        and abs(float(w) - float(r)) < 1e-6,
    ),
    (
        "annotated_short",
        _annotated_short_value,
        lambda w, r: r == w["value"],
    ),
]


@st.composite
def tag_metadata(draw):
    """Generate a dict of 1-5 custom tags with random inferred types."""
    num_tags = draw(st.integers(min_value=1, max_value=5))
    tags = draw(
        st.lists(
            _custom_tag_number(), min_size=num_tags, max_size=num_tags, unique=True
        )
    )

    metadata = {}
    expectations = []
    for tag in tags:
        type_name, strategy_fn, cmp = draw(st.sampled_from(FIELD_TYPE_STRATEGIES))
        value = draw(strategy_fn())
        key = str(tag)
        metadata[key] = value
        expectations.append((key, value, type_name, cmp))

    return metadata, expectations


# =============================================================================
# Field type roundtrip (custom tags)
# =============================================================================


@pytest.mark.property
class TestTiffFieldTypeRoundtrip:
    """Field type roundtrip.

    For any set of custom TIFF tags with supported field types, writing
    the tags and reading them back shall produce equivalent values.
    """

    @given(data=tag_metadata())
    @pbt_settings
    def test_field_type_roundtrip(self, data):
        """Custom tags with inferred types survive a write-read cycle."""
        metadata_dict, expectations = data

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            path = Path(f.name)

        try:
            _write_tiff_with_metadata(path, metadata_dict)
            read_meta = _read_tiff_metadata(path)

            for key, written, type_name, cmp in expectations:
                assert key in read_meta, (
                    "Tag " + key + " (" + type_name + ") missing from read-back"
                )
                read_val = read_meta[key]
                assert cmp(written, read_val), (
                    "Tag " + key + " (" + type_name + ") roundtrip mismatch: "
                    "wrote " + repr(written) + ", read " + repr(read_val)
                )
        finally:
            path.unlink(missing_ok=True)
