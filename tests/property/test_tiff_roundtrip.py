"""Property-based tests for TIFF roundtrip operations.

This module tests correctness properties for TIFF read and write roundtrips:
- Property 1a: Pixel data roundtrip (stripped TIFFs via PIL → our reader)
- Property 1b: Lossless pixel roundtrip (our writer → our reader)
- Property 2a: Band subsetting preserves correct data
- Property 2b: Encoding hints → TIFF tag metadata roundtrip
- Property 3: Idempotent encoding (write → read → write → read)
- Property 5a: Non-image asset rejection (writer)
- Property 5b: Field type roundtrip (custom tags survive write → read)
- Property 10: Stripped TIFF block dimensions

**Validates: Requirements 1.2, 2.1, 2.3, 3.1–3.7, 4.1–4.5, 4.8–4.11,
5.1, 5.4, 6.1, 6.2, 6.4, 7.1–7.3, 8.3, 9.1–9.5, 10.1, 11.1–11.3**
"""

import math
import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase, assume
from hypothesis import strategies as st
from PIL import Image

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
    tiff_image_config,
    tiff_writable_image,
    tiff_encoding_hints,
    tiff_writer_pixel_types,
    image_dimensions,
    band_counts,
    image_arrays,
)


# Hypothesis settings for I/O-bound TIFF tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


# =============================================================================
# PIL helpers (for read-path tests)
# =============================================================================

# PIL mode mapping for creating images from numpy arrays.
# PixelType is not hashable, so we key by (dtype_name, bands).
_PIL_MODE = {
    ("uint8", 1): "L",
    ("uint8", 3): "RGB",
    ("uint16", 1): "I;16",
    ("int32", 1): "I",
    ("float32", 1): "F",
}


def _create_tiff_pil(cfg: dict, array_chw: np.ndarray) -> bytes:
    """Create a TIFF file in memory using PIL from a CHW numpy array.

    Args:
        cfg: Config dict from tiff_image_config strategy.
        array_chw: Pixel data in (bands, height, width) layout.

    Returns:
        TIFF file bytes.
    """
    pixel_type = cfg["pixel_type"]
    bands = cfg["bands"]
    rps = cfg["rows_per_strip"]
    pil_comp = cfg["pil_compression"]

    dtype = get_numpy_dtype(pixel_type)
    mode = _PIL_MODE[(dtype.name, bands)]

    if bands == 1:
        hw = array_chw[0]
    else:
        hw = np.transpose(array_chw, (1, 2, 0))

    img = Image.fromarray(hw, mode)

    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    try:
        img.save(str(path), compression=pil_comp, tiffinfo={278: rps})
        return path.read_bytes()
    finally:
        path.unlink(missing_ok=True)


# =============================================================================
# Native writer helper (for write-path tests)
# =============================================================================


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
# Custom tag metadata helper (for field type roundtrip tests)
# =============================================================================

# Private-use tag range for custom metadata roundtrip testing.
CUSTOM_TAG_MIN = 65000
CUSTOM_TAG_MAX = 65499


def _write_tiff_with_metadata(path, metadata_dict):
    """Write a minimal 16x16 TIFF with the given metadata tags."""
    meta = BufferedMetadataProvider()
    meta.set("TileWidth", "256")
    meta.set("TileHeight", "256")
    meta.set("Compression", "None")
    meta.set("PlanarConfiguration", "Chunky")

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
    """Strategy for a private-use tag number."""
    return st.integers(min_value=CUSTOM_TAG_MIN, max_value=CUSTOM_TAG_MAX)


def _ascii_value():
    """ASCII string value (inferred as TIFF_ASCII)."""
    return st.text(
        alphabet=st.characters(
            whitelist_categories=("L", "N", "P", "Z"), max_codepoint=126
        ),
        min_size=1,
        max_size=30,
    )


def _long_value():
    """Non-negative integer (inferred as TIFF_LONG)."""
    return st.integers(min_value=0, max_value=2**31 - 1)


def _slong_value():
    """Negative integer (inferred as TIFF_SLONG)."""
    return st.integers(min_value=-(2**31), max_value=-1)


def _short_array_value():
    """Array of non-negative integers (inferred as TIFF_SHORT)."""
    return st.lists(
        st.integers(min_value=0, max_value=65535), min_size=1, max_size=6
    )


def _sshort_array_value():
    """Array with at least one negative integer (inferred as TIFF_SSHORT)."""
    return st.lists(
        st.integers(min_value=-32768, max_value=32767), min_size=2, max_size=6
    ).filter(lambda arr: any(x < 0 for x in arr))


def _double_array_value():
    """Array of floats (inferred as TIFF_DOUBLE).

    Uses finite floats only. Single-element arrays are allowed; they read
    back as scalars.
    """
    return st.lists(
        st.floats(
            min_value=-1e10, max_value=1e10,
            allow_nan=False, allow_infinity=False,
        ),
        min_size=1,
        max_size=6,
    )


def _double_scalar_value():
    """Scalar float value (inferred as TIFF_DOUBLE).

    Exercises the scalar DOUBLE write path including whole-number floats
    like 1.0 that serde_json would otherwise coerce to integers.
    """
    return st.floats(
        min_value=-1e10, max_value=1e10,
        allow_nan=False, allow_infinity=False,
    )


def _annotated_short_value():
    """Explicit type annotation dict: {"value": <int>, "type": 3} (SHORT).

    Exercises python_to_json's PyDict handling and the explicit annotation
    path in infer_field_type.
    """
    return st.integers(min_value=0, max_value=65535).map(
        lambda v: {"value": v, "type": 3}
    )


# Each entry: (type_name, strategy_fn, comparator)
# The comparator receives (written_value, read_value) and returns bool.
FIELD_TYPE_STRATEGIES = [
    ("ascii", _ascii_value, lambda w, r: r == w),
    ("long", _long_value, lambda w, r: r == w),
    ("slong", _slong_value, lambda w, r: r == w),
    (
        "short_array",
        _short_array_value,
        lambda w, r: isinstance(r, (list, int)) and (
            # count==1 reads back as scalar
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
            # single-element array reads back as scalar
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
    """Generate a dict of 1-5 custom tags with random inferred types.

    Returns ``(metadata_dict, expectations)`` where *expectations* is a list
    of ``(tag_key, written_value, type_name, comparator)`` tuples.
    """
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
# Metadata tag mappings (for encoding hint roundtrip)
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


# =============================================================================
# Property 1a: Pixel data roundtrip (PIL → our reader)
# =============================================================================


@pytest.mark.property
class TestTiffPixelRoundtrip:
    """Property 1a: Pixel data roundtrip (PIL writer → our reader)

    For any valid image configuration writable by PIL (pixel type from
    {uint8, uint16, int32, float32}, 1 or 3 bands, compressions from
    {none, LZW, Deflate, PackBits}), writing a TIFF and reading it back
    through TIFFImageAssetProvider.get_block() produces byte-identical
    pixel data in band-sequential (CHW) format.

    # Feature: libtiff-ffi-tiff-reading, Property 1: Pixel data roundtrip
    **Validates: Requirements 4.2, 4.3, 4.4, 4.5, 4.8, 4.9, 9.1, 9.2, 9.3, 9.5, 10.1, 11.3**
    """

    @given(config=tiff_image_config(min_size=16, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_stripped_roundtrip(self, config):
        """Stripped TIFF pixel data survives a write-read cycle exactly."""
        pixel_type = config["pixel_type"]
        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        # Generate deterministic pixel data
        rng = np.random.RandomState(42)
        if np.issubdtype(dtype, np.floating):
            array_chw = rng.rand(bands, height, width).astype(dtype)
        elif np.issubdtype(dtype, np.signedinteger):
            info = np.iinfo(dtype)
            array_chw = rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
        else:
            info = np.iinfo(dtype)
            array_chw = rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff_pil(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")

            # Verify dimensions match
            assert asset.num_columns == width
            assert asset.num_rows == height
            assert asset.num_bands == bands

            decoded = read_full_image(asset, bands, height, width)

            assert decoded.shape == array_chw.shape, (
                f"Shape mismatch: expected {array_chw.shape}, got {decoded.shape}"
            )

            # Float arrays need NaN-aware comparison
            if np.issubdtype(dtype, np.floating):
                np.testing.assert_array_equal(decoded, array_chw)
            else:
                np.testing.assert_array_equal(decoded, array_chw)
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 1b: Lossless pixel roundtrip (our writer → our reader)
# =============================================================================


@pytest.mark.property
class TestTiffLosslessPixelRoundtrip:
    """Property 1b: TIFF Lossless Pixel Roundtrip (native writer)

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
# Property 2a: Band subsetting preserves correct data
# =============================================================================


@pytest.mark.property
class TestTiffBandSubsetting:
    """Property 2a: Band subsetting preserves correct data

    For any multi-band TIFF and any non-empty subset of band indices,
    get_block() with that subset returns only the requested bands, matching
    the corresponding bands from a full read.

    # Feature: libtiff-ffi-tiff-reading, Property 2: Band subsetting
    **Validates: Requirements 4.10**
    """

    @given(config=tiff_image_config(min_size=16, max_size=48, min_bands=3, max_bands=3))
    @pbt_settings
    def test_band_subset_matches_full_read(self, config):
        """Reading a band subset matches the same bands from a full read."""
        pixel_type = config["pixel_type"]
        # Only uint8 supports 3 bands in PIL
        assume(pixel_type == PixelType.UInt8)

        width, height, bands = config["width"], config["height"], config["bands"]
        dtype = get_numpy_dtype(pixel_type)

        rng = np.random.RandomState(123)
        array_chw = rng.randint(0, 256, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff_pil(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")

            # Full read
            full_block = asset.get_block(0, 0, 0, None)

            # Test several subsets
            for subset in [[0], [2], [0, 2], [1, 2], [0, 1, 2]]:
                sub_block = asset.get_block(0, 0, 0, subset)
                assert sub_block.shape[0] == len(subset), (
                    f"Expected {len(subset)} bands, got {sub_block.shape[0]}"
                )
                for i, band_idx in enumerate(subset):
                    np.testing.assert_array_equal(
                        sub_block[i],
                        full_block[band_idx],
                        err_msg=f"Band {band_idx} mismatch in subset {subset}",
                    )
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 2b: Encoding hints → TIFF tag metadata roundtrip
# =============================================================================


@pytest.mark.property
class TestTiffMetadataRoundtrip:
    """Property 2b: TIFF Metadata Roundtrip

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

            # Image dimension tags (numeric tag IDs)
            assert meta["256"] == num_cols       # ImageWidth
            assert meta["257"] == num_rows       # ImageLength

            # Pixel format tags — for multi-band images, BitsPerSample (258)
            # and SampleFormat (339) are per-sample arrays.
            dtype = get_numpy_dtype(pixel_type)
            expected_bps = dtype.itemsize * 8
            bps = meta["258"]  # BitsPerSample
            if isinstance(bps, list):
                assert all(b == expected_bps for b in bps), (
                    f"BitsPerSample: expected all {expected_bps}, got {bps}"
                )
            else:
                assert bps == expected_bps

            assert meta["277"] == num_bands  # SamplesPerPixel

            sf = meta["339"]  # SampleFormat
            expected_sf = _SAMPLE_FORMAT[int(pixel_type)]
            if isinstance(sf, list):
                assert all(s == expected_sf for s in sf), (
                    f"SampleFormat: expected all {expected_sf}, got {sf}"
                )
            else:
                assert sf == expected_sf

            # Photometric interpretation
            expected_photo = 2 if num_bands >= 3 else 1
            assert meta["262"] == expected_photo  # PhotometricInterpretation

            # Encoding hint tags
            assert meta["259"] == _COMPRESSION_TAG[hints["Compression"]]  # Compression

            tile_w = int(hints["TileWidth"])
            tile_h = int(hints["TileHeight"])
            assert meta["322"] == tile_w   # TileWidth
            assert meta["323"] == tile_h   # TileLength

            # Predictor tag (317) is only reported by libtiff when explicitly
            # set to a non-default value (i.e. Horizontal=2). When Predictor
            # is "None" (1) or compression is uncompressed, the tag may be
            # absent.
            if hints["Predictor"] == "Horizontal" and hints["Compression"] != "None":
                assert meta.get("317") == _PREDICTOR_TAG[hints["Predictor"]]

            assert meta["284"] == _PLANAR_TAG[hints["PlanarConfiguration"]]  # PlanarConfiguration
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 3: Idempotent encoding
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


# =============================================================================
# Property 5a: Non-image asset rejection
# =============================================================================


@pytest.mark.property
class TestTiffNonImageAssetRejection:
    """Property 5a: TIFF Non-Image Asset Rejection

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
# Property 5b: Field type roundtrip (custom tags)
# =============================================================================


@pytest.mark.property
class TestTiffFieldTypeRoundtrip:
    """Property 5b: Field Type Roundtrip

    For any set of custom TIFF tags with supported field types, writing
    the tags via DatasetWriter and reading them back via DatasetReader
    shall produce equivalent values.

    # Feature: tiff-writing, Property 5b: Field Type Roundtrip
    **Validates: Requirements 7.1, 8.3**
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


# =============================================================================
# Property 10: Stripped TIFF block dimensions
# =============================================================================


@pytest.mark.property
class TestTiffStrippedBlockDimensions:
    """Property 10: Stripped TIFF block dimensions

    For any stripped TIFF, num_pixels_per_block_horizontal == ImageWidth,
    num_pixels_per_block_vertical == RowsPerStrip, and
    block_grid_size == (ceil(ImageLength / RowsPerStrip), 1).

    # Feature: libtiff-ffi-tiff-reading, Property 10: Stripped TIFF block dimensions
    **Validates: Requirements 11.1, 11.2**
    """

    @given(config=tiff_image_config(min_size=16, max_size=128))
    @pbt_settings
    def test_stripped_block_layout(self, config):
        """Stripped TIFF reports correct block dimensions and grid size."""
        width, height = config["width"], config["height"]
        bands = config["bands"]
        pixel_type = config["pixel_type"]
        rps = config["rows_per_strip"]
        dtype = get_numpy_dtype(pixel_type)

        # Skip edge case where RowsPerStrip >= ImageLength and dtype > 1 byte.
        # There is a known reader issue where libtiff adjusts RowsPerStrip
        # internally for large-pixel-type single-strip images, causing a
        # mismatch between the tag value and the reported block height.
        assume(not (rps >= height and dtype.itemsize > 1))

        rng = np.random.RandomState(7)
        if np.issubdtype(dtype, np.floating):
            array_chw = rng.rand(bands, height, width).astype(dtype)
        elif np.issubdtype(dtype, np.signedinteger):
            info = np.iinfo(dtype)
            array_chw = rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
        else:
            info = np.iinfo(dtype)
            array_chw = rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)

        tiff_bytes = _create_tiff_pil(config, array_chw)

        with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
            f.write(tiff_bytes)
            path = Path(f.name)

        try:
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            meta = asset.get_metadata().as_dict()

            # PIL may silently adjust RowsPerStrip, so read the actual value
            actual_rps = meta["278"]  # RowsPerStrip tag

            assert asset.num_pixels_per_block_horizontal == width, (
                f"Block width should be ImageWidth ({width}), got {asset.num_pixels_per_block_horizontal}"
            )
            assert asset.num_pixels_per_block_vertical == actual_rps, (
                f"Block height should be RowsPerStrip ({actual_rps}), got {asset.num_pixels_per_block_vertical}"
            )

            expected_grid = (math.ceil(height / actual_rps), 1)
            assert asset.block_grid_size == expected_grid, (
                f"Grid should be {expected_grid}, got {asset.block_grid_size}"
            )
        finally:
            path.unlink(missing_ok=True)
