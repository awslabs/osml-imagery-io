"""Property-based tests for Zarr codec decode bindings.

Feature: zarr-codec-plugins
"""

import os

import numpy as np
import pytest
from aws.osml.io._io import decode_jbp_block
from hypothesis import given
from hypothesis import strategies as st

from ..conftest import pbt_settings
from ..helpers import write_and_read_jbp
from ..strategies import get_numpy_dtype, jpeg_image_for_compression, realistic_image_for_compression

# Valid (nbpp, pvtype, expected_dtype) combinations for JBP block
JBP_TYPE_COMBOS = [
    (8, "INT", np.uint8),
    (16, "INT", np.uint16),
    (16, "SI", np.int16),
    (32, "R", np.float32),
    (64, "R", np.float64),
]


@pytest.mark.property
class TestProperty2J2KDecodeShape:
    """Feature: zarr-codec-plugins, Property 2: J2K decode output shape and dtype

    For any valid JPEG 2000 codestream with known dimensions (bands, height, width)
    and bit depth, decode_jpeg2000 returns a NumPy ndarray with shape
    (bands, height, width) and correct dtype.

    **Validates: Requirements 1.6, 1.7**
    """

    @given(realistic_image_for_compression(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_prop_2_j2k_decode_output_shape_and_dtype(self, image_tuple):
        """Feature: zarr-codec-plugins, Property 2: J2K decode output shape and dtype"""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        # Calculate safe decomposition levels for the image dimensions
        min_dim = min(num_rows, num_cols)
        max_decomp_levels = max(1, int(np.floor(np.log2(min_dim))) - 1)
        decomp_levels = min(5, max_decomp_levels)

        # Write as J2K-compressed NITF and read back through the IO pipeline,
        # which internally uses decode_jpeg2000 for J2K decompression
        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={
                "IC": "C8",
                "COMRAT": "02.0",
                "J2K_DECOMPOSITION_LEVELS": str(decomp_levels),
            },
        )

        # Property: output is 3D with shape (bands, height, width)
        assert decoded.ndim == 3, f"Expected 3D array, got {decoded.ndim}D"
        assert decoded.shape[0] == num_bands, (
            f"Expected {num_bands} bands, got {decoded.shape[0]}"
        )
        assert decoded.shape[1] == num_rows, (
            f"Expected {num_rows} rows, got {decoded.shape[1]}"
        )
        assert decoded.shape[2] == num_cols, (
            f"Expected {num_cols} cols, got {decoded.shape[2]}"
        )

        # Property: dtype matches the pixel type
        expected_dtype = get_numpy_dtype(pixel_type)
        assert decoded.dtype == expected_dtype, (
            f"Expected dtype {expected_dtype}, got {decoded.dtype}"
        )


@pytest.mark.property
class TestProperty3JpegDecodeShape:
    """Feature: zarr-codec-plugins, Property 3: JPEG decode output shape

    For any valid JPEG stream and valid parameters, decode_jpeg returns
    an ndarray with shape (num_bands, block_height, block_width) in BSQ format.

    **Validates: Requirements 2.5, 2.6**
    """

    @given(jpeg_image_for_compression(min_size=32, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_prop_3_jpeg_decode_output_shape(self, image_tuple):
        """Feature: zarr-codec-plugins, Property 3: JPEG decode output shape"""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        decoded = write_and_read_jbp(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "C3", "COMRAT": "75.0"},
        )

        # Property: output is 3D with shape (bands, height, width)
        assert decoded.ndim == 3, f"Expected 3D array, got {decoded.ndim}D"
        assert decoded.shape[0] == num_bands, (
            f"Expected {num_bands} bands, got {decoded.shape[0]}"
        )
        assert decoded.shape[1] == num_rows, (
            f"Expected {num_rows} rows, got {decoded.shape[1]}"
        )
        assert decoded.shape[2] == num_cols, (
            f"Expected {num_cols} cols, got {decoded.shape[2]}"
        )

        # Property: dtype is uint8 for 8-bit JPEG
        assert decoded.dtype == np.uint8, (
            f"Expected dtype uint8, got {decoded.dtype}"
        )


@pytest.mark.property
class TestProperty4JbpBlockDecodeShape:
    """Feature: zarr-codec-plugins, Property 4: JBP block decode output shape and dtype

    For any valid raw pixel data buffer with dimensions (num_bands, block_height,
    block_width), bit depth nbpp, interleave mode imode, and pixel value type pvtype,
    decode_jbp_block returns an ndarray with shape (num_bands, block_height, block_width)
    and the correct dtype.

    **Validates: Requirements 3.6**
    """

    @given(
        num_bands=st.integers(min_value=1, max_value=4),
        block_height=st.integers(min_value=1, max_value=32),
        block_width=st.integers(min_value=1, max_value=32),
        type_combo=st.sampled_from(JBP_TYPE_COMBOS),
        imode=st.sampled_from(["B", "P", "R", "S"]),
    )
    @pbt_settings
    def test_prop_4_jbp_block_decode_shape_and_dtype(
        self, num_bands, block_height, block_width, type_combo, imode
    ):
        """Feature: zarr-codec-plugins, Property 4: JBP block decode output shape and dtype"""
        nbpp, pvtype, expected_dtype = type_combo
        bytes_per_pixel = nbpp // 8
        data_size = num_bands * block_height * block_width * bytes_per_pixel

        # Generate random data of the correct size
        data = os.urandom(data_size)

        result = decode_jbp_block(
            data,
            num_bands=num_bands,
            block_height=block_height,
            block_width=block_width,
            nbpp=nbpp,
            imode=imode,
            pvtype=pvtype,
        )

        # Property: shape is (num_bands, block_height, block_width)
        assert result.shape == (num_bands, block_height, block_width), (
            f"Expected shape ({num_bands}, {block_height}, {block_width}), got {result.shape}"
        )

        # Property: dtype matches expected
        assert result.dtype == expected_dtype, (
            f"Expected dtype {expected_dtype}, got {result.dtype}"
        )


def _codec_from_zarray(zarray: dict):
    """Build a codec instance from a Kerchunk .zarray dict.

    The TileIndex._build_zarray produces zarr v2 metadata with custom
    codecs in the ``filters`` list. Each filter has an ``id`` field
    and the codec-specific configuration keys.
    """
    from aws.osml.io.zarr_codecs import JbpBlockCodec, Jpeg2000Codec

    filters = zarray.get("filters", [])
    assert filters and len(filters) >= 1, "No filters in .zarray"
    config = filters[0]

    if "main_header" in config:
        return Jpeg2000Codec.from_dict({"configuration": config})

    if "pvtype" in config:
        return JbpBlockCodec(
            num_bands=config["num_bands"],
            block_height=config["block_height"],
            block_width=config["block_width"],
            nbpp=config["nbpp"],
            imode=config["imode"],
            pvtype=config["pvtype"],
        )

    raise ValueError(f"Cannot determine codec type from config keys: {list(config.keys())}")


@pytest.mark.property
class TestProperty7EndToEndDecodeEquivalence:
    """Feature: zarr-codec-plugins, Property 7: End-to-end decode equivalence

    For any tile in a Kerchunk index generated from a local imagery file,
    decoding the tile bytes through the Zarr codec produces pixel values
    identical to decoding via IO.open() + get_block().

    **Validates: Requirements 11.3, 15.3**
    """

    @given(realistic_image_for_compression(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_prop_7_end_to_end_nc(self, image_tuple):
        """Feature: zarr-codec-plugins, Property 7: End-to-end decode equivalence (NC)"""
        import json
        import tempfile
        from pathlib import Path

        from aws.osml.io import IO, AssetType, BufferedImageAssetProvider, BufferedMetadataProvider
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        # Write as uncompressed NITF
        metadata = BufferedMetadataProvider()
        metadata["IC"] = "NC"
        metadata["IMODE"] = "B"

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=num_cols,
            block_height=num_rows,
            pixel_type=pixel_type,
            metadata=metadata,
        )
        provider.set_full_image(array)

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image:0",
                provider=provider,
                title="Test Image",
                description="Property test image",
                roles=["data"],
            )
            writer.close()

            # Path A: Read via IO
            with IO.open([str(path)], "r") as reader:
                keys = reader.get_asset_keys(asset_type=AssetType.Image)
                asset = reader.get_asset(keys[0])
                block_via_io = asset.get_block(0, 0, 0)

            # Path B: Read via Kerchunk index + codec
            parser = OversightMLParser(local_paths=str(path))
            ms = parser(url=str(path))

            from aws.osml.io.virtualizarr_parsers import write_tile_index

            with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
                index_path = Path(f.name)
            write_tile_index(ms, str(index_path))

            try:
                with open(index_path) as f:
                    refs = json.load(f)

                # The store is always hierarchical: level 0 data at "0/data/"
                segment_key = "0/data"
                assert f"{segment_key}/.zarray" in refs["refs"], "No .zarray found for 0/data"

                zarray = json.loads(refs["refs"][f"{segment_key}/.zarray"])
                codec = _codec_from_zarray(zarray)

                # Read tile bytes from file
                tile_key = f"{segment_key}/0.0.0"
                tile_ref = refs["refs"][tile_key]
                with open(tile_ref[0], "rb") as f:
                    f.seek(tile_ref[1])
                    tile_bytes = f.read(tile_ref[2])

                block_via_codec_bytes = codec.decode(tile_bytes)

                # Codec returns flat bytes; reconstruct as numpy array
                expected_dtype = get_numpy_dtype(pixel_type)
                block_via_codec = np.frombuffer(block_via_codec_bytes, dtype=expected_dtype).reshape(
                    num_bands, num_rows, num_cols
                )

                # Property: identical pixel values
                np.testing.assert_array_equal(
                    block_via_codec, block_via_io,
                    err_msg="Codec path and IO path produced different pixels",
                )
            finally:
                index_path.unlink(missing_ok=True)
        finally:
            path.unlink(missing_ok=True)
