"""Unit tests for zarr_codecs module: error handling, edge cases, and module structure.

Validates: Requirements 9.1, 12.2, 12.3, 14.1, 14.2, 14.3, 14.4, 14.5, 14.6
"""

from pathlib import Path

import numpy as np
import pytest
from aws.osml.io.zarr_codecs import (
    JbpBlockCodec,
    Jpeg2000Codec,
    JpegCodec,
    decode_jbp_block,
    decode_jpeg,
)


class TestDecodeBindingErrors:
    """Tests for ValueError conditions in the Rust decode bindings."""

    def test_jbp_block_data_length_mismatch(self):
        """decode_jbp_block raises ValueError when data length doesn't match expected size."""
        # 2x2x1 uint8 expects 4 bytes, pass 10
        with pytest.raises(ValueError, match="Data size mismatch"):
            decode_jbp_block(b"\x00" * 10, num_bands=1, block_height=2, block_width=2, nbpp=8, imode="B", pvtype="INT")

    def test_decode_jpeg_invalid_imode(self):
        """decode_jpeg raises ValueError for invalid imode string."""
        with pytest.raises(ValueError, match="Invalid interleave mode"):
            decode_jpeg(
                b"\x00", bits_per_pixel=8, num_bands=1,
                block_width=1, block_height=1, imode="X", color_space="MONO",
            )

    def test_decode_jpeg_invalid_color_space(self):
        """decode_jpeg raises ValueError for invalid color_space string."""
        with pytest.raises(ValueError, match="Invalid color space"):
            decode_jpeg(
                b"\x00", bits_per_pixel=8, num_bands=1,
                block_width=1, block_height=1, imode="B", color_space="INVALID",
            )

    def test_jbp_block_invalid_pvtype(self):
        """decode_jbp_block raises ValueError for invalid pvtype string."""
        with pytest.raises(ValueError, match="Invalid pixel value type"):
            decode_jbp_block(
                b"\x00" * 4, num_bands=1, block_height=2,
                block_width=2, nbpp=8, imode="B", pvtype="INVALID",
            )

    def test_jbp_block_invalid_imode(self):
        """decode_jbp_block raises ValueError for invalid imode string."""
        with pytest.raises(ValueError, match="Invalid interleave mode"):
            decode_jbp_block(b"\x00" * 4, num_bands=1, block_height=2, block_width=2, nbpp=8, imode="Z", pvtype="INT")

    def test_jbp_block_unsupported_nbpp_64_int(self):
        """decode_jbp_block raises ValueError for nbpp=64 with pvtype=INT."""
        with pytest.raises(ValueError, match="Unsupported nbpp"):
            decode_jbp_block(b"\x00" * 32, num_bands=1, block_height=2, block_width=2, nbpp=64, imode="B", pvtype="INT")

    def test_jbp_block_unsupported_nbpp_8_real(self):
        """decode_jbp_block raises ValueError for nbpp=8 with pvtype=R."""
        with pytest.raises(ValueError, match="Unsupported nbpp"):
            decode_jbp_block(b"\x00" * 4, num_bands=1, block_height=2, block_width=2, nbpp=8, imode="B", pvtype="R")


class TestCodecEncodeRejection:
    """All three codec encode() methods raise NotImplementedError."""

    def test_jpeg2000_encode_raises(self):
        import asyncio

        codec = Jpeg2000Codec()
        with pytest.raises(NotImplementedError, match="Jpeg2000Codec"):
            asyncio.run(codec._encode_single(b"\x00", None))

    def test_jpeg_encode_raises(self):
        import asyncio

        codec = JpegCodec(bits_per_pixel=8, num_bands=1, block_width=8, block_height=8, imode="B", color_space="MONO")
        with pytest.raises(NotImplementedError, match="JpegCodec"):
            asyncio.run(codec._encode_single(b"\x00", None))

    def test_jbp_block_encode_raises(self):
        import asyncio

        codec = JbpBlockCodec(num_bands=1, block_height=8, block_width=8, nbpp=8, imode="B", pvtype="INT")
        with pytest.raises(NotImplementedError, match="JbpBlockCodec"):
            asyncio.run(codec._encode_single(b"\x00", None))


class TestCodecConfigValidation:
    """Codec __init__ / from_dict raises ValueError for missing or invalid config."""

    def test_jpeg_from_dict_missing_fields(self):
        """JpegCodec.from_dict raises ValueError when required fields are missing."""
        with pytest.raises(ValueError, match="Missing required configuration"):
            JpegCodec.from_dict({"configuration": {"bits_per_pixel": 8}})

    def test_jbp_block_from_dict_missing_fields(self):
        """JbpBlockCodec.from_dict raises ValueError when required fields are missing."""
        with pytest.raises(ValueError, match="Missing required configuration"):
            JbpBlockCodec.from_dict({"configuration": {"num_bands": 1}})

    def test_jpeg2000_invalid_base64_main_header(self):
        """Jpeg2000Codec raises ValueError for invalid base64 main_header."""
        with pytest.raises(ValueError, match="Invalid base64"):
            Jpeg2000Codec(main_header="!!!not-valid-base64!!!")


class TestModuleExports:
    """Verify module __all__ and binding accessibility."""

    def test_zarr_codecs_all_exports(self):
        """zarr_codecs module __all__ contains all expected names."""
        import aws.osml.io.zarr_codecs as mod

        expected = {
            "Jpeg2000Codec", "JpegCodec", "JbpBlockCodec", "TiffTileCodec",
            "DtedTileCodec",
            "decode_jpeg2000", "decode_jpeg", "decode_jbp_block", "decode_tiff_tile",
            "decode_dted_tile",
        }
        assert set(mod.__all__) == expected

    def test_decode_bindings_accessible_from_io(self):
        """Decode binding functions are accessible from aws.osml.io._io."""
        import aws.osml.io._io as _io

        assert callable(getattr(_io, "decode_jpeg2000"))
        assert callable(getattr(_io, "decode_jpeg"))
        assert callable(getattr(_io, "decode_jbp_block"))


class TestCodecConfigRoundTrip:
    """Codec configuration serialization round-trip tests."""

    def test_jpeg2000_round_trip_no_header(self):
        """Jpeg2000Codec round-trip without main_header."""
        codec = Jpeg2000Codec(resolution_level=2)
        d = codec.to_dict()
        codec2 = Jpeg2000Codec.from_dict(d)
        assert codec2.to_dict() == d
        assert codec2.resolution_level == 2
        assert codec2._main_header_bytes is None

    def test_jpeg2000_round_trip_with_header(self):
        """Jpeg2000Codec round-trip with base64 main_header."""
        import base64

        header_bytes = b"\xff\x4f\xff\x51\x00\x0a\x00\x00"
        header_b64 = base64.b64encode(header_bytes).decode()
        codec = Jpeg2000Codec(main_header=header_b64, resolution_level=1)
        d = codec.to_dict()
        codec2 = Jpeg2000Codec.from_dict(d)
        assert codec2.to_dict() == d
        assert codec2._main_header_bytes == header_bytes
        assert codec2.resolution_level == 1

    def test_jpeg_round_trip(self):
        """JpegCodec round-trip."""
        codec = JpegCodec(
            bits_per_pixel=8,
            num_bands=3,
            block_width=256,
            block_height=256,
            imode="P",
            color_space="YCbCr601",
        )
        d = codec.to_dict()
        codec2 = JpegCodec.from_dict(d)
        assert codec2.to_dict() == d

    def test_jbp_block_round_trip(self):
        """JbpBlockCodec round-trip."""
        codec = JbpBlockCodec(
            num_bands=3,
            block_height=128,
            block_width=128,
            nbpp=16,
            imode="P",
            pvtype="SI",
        )
        d = codec.to_dict()
        codec2 = JbpBlockCodec.from_dict(d)
        assert codec2.to_dict() == d


# Test data paths
DATA_DIR = Path("data/unit")
J2K_NTF = DATA_DIR / "nitf21-64x64-3band-8bit-j2k.ntf"


class TestDecodeCorrectness:
    """Decode correctness tests using test data and synthetic inputs.

    Validates: Requirements 1.6, 1.7, 2.5, 2.6, 3.6, 3.7
    """

    # --- JPEG 2000 decode correctness ---

    def test_decode_jpeg2000_shape_and_dtype(self):
        """decode_jpeg2000 with a real J2K codestream produces correct shape and dtype.

        Opens the J2K-compressed NITF via IO, reads a block through the normal
        pipeline, then verifies the result has the expected 3D (bands, height, width)
        shape and a valid numeric dtype.

        Validates: Requirements 1.6, 1.7
        """
        from aws.osml.io import IO, AssetType

        if not J2K_NTF.exists():
            pytest.skip("J2K test data not available")

        reader = IO.open([str(J2K_NTF)], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        assert len(image_keys) > 0, "Expected at least one image segment"

        asset = reader.get_asset(image_keys[0])
        block = asset.get_block(0, 0, 0)

        # Requirement 1.6: shape is (bands, height, width)
        assert block.ndim == 3, f"Expected 3D array, got {block.ndim}D"
        bands, height, width = block.shape
        assert bands >= 1, "Expected at least 1 band"
        assert height > 0 and width > 0, "Expected positive spatial dimensions"

        # Requirement 1.7: dtype matches bit depth
        assert block.dtype in (
            np.uint8, np.uint16, np.int16, np.uint32, np.int32,
        ), f"Unexpected dtype {block.dtype}"

        reader.close()

    # --- JPEG decode correctness ---

    def test_decode_jpeg_shape_bsq(self):
        """decode_jpeg with a real JPEG stream produces (bands, height, width) BSQ output.

        Creates a small NITF with JPEG compression via the write path, reads the
        block back through IO, and verifies the output shape is BSQ format.

        Validates: Requirements 2.5, 2.6
        """
        import tempfile

        from aws.osml.io import IO, AssetType, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

        bands, rows, cols = 3, 32, 32
        pixel_data = np.random.randint(0, 256, (bands, rows, cols), dtype=np.uint8)

        metadata = BufferedMetadataProvider()
        metadata.set("IC", "C3")
        metadata.set("IMODE", "P")

        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=cols,
            num_rows=rows,
            num_bands=bands,
            block_width=cols,
            block_height=rows,
            pixel_type=PixelType.UInt8,
            metadata=metadata,
        )
        provider.set_full_image(pixel_data)

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image:0",
                provider=provider,
                title="Test Image",
                description="JPEG decode test",
                roles=["data"],
            )
            writer.close()

            reader = IO.open([str(path)], "r")
            image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
            asset = reader.get_asset(image_keys[0])
            block = asset.get_block(0, 0, 0)

            # Requirement 2.5: shape is (bands, height, width)
            assert block.ndim == 3
            assert block.shape == (bands, rows, cols), f"Expected ({bands}, {rows}, {cols}), got {block.shape}"

            # Requirement 2.6: BSQ format — band dimension first
            assert block.shape[0] == bands

            reader.close()
        finally:
            path.unlink(missing_ok=True)

    # --- JBP block decode: interleave modes ---

    def test_decode_jbp_block_bsq_uint8(self):
        """decode_jbp_block with BSQ (imode=S) uint8 data returns correct values.

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 2, 3, 4
        bsq_data = np.arange(bands * rows * cols, dtype=np.uint8)
        result = decode_jbp_block(
            bytes(bsq_data), num_bands=bands, block_height=rows,
            block_width=cols, nbpp=8, imode="S", pvtype="INT",
        )
        assert result.shape == (bands, rows, cols)
        assert result.dtype == np.uint8
        np.testing.assert_array_equal(result, bsq_data.reshape(bands, rows, cols))

    def test_decode_jbp_block_bib_mode_b(self):
        """decode_jbp_block with imode=B (band-interleaved-by-block) is equivalent to BSQ.

        For a single block, B and S have identical memory layout.

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 2, 2, 3
        bsq_data = np.arange(bands * rows * cols, dtype=np.uint8)
        result = decode_jbp_block(
            bytes(bsq_data), num_bands=bands, block_height=rows,
            block_width=cols, nbpp=8, imode="B", pvtype="INT",
        )
        assert result.shape == (bands, rows, cols)
        np.testing.assert_array_equal(result, bsq_data.reshape(bands, rows, cols))

    def test_decode_jbp_block_bip_to_bsq(self):
        """decode_jbp_block converts BIP (imode=P) to BSQ correctly.

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 3, 2, 2
        # Create known BSQ data
        bsq = np.array([
            [[1, 2], [3, 4]],     # band 0
            [[5, 6], [7, 8]],     # band 1
            [[9, 10], [11, 12]],  # band 2
        ], dtype=np.uint8)
        # Convert to BIP: pixel-interleaved (rows, cols, bands)
        bip = np.transpose(bsq, (1, 2, 0)).tobytes()

        result = decode_jbp_block(
            bip, num_bands=bands, block_height=rows,
            block_width=cols, nbpp=8, imode="P", pvtype="INT",
        )
        assert result.shape == (bands, rows, cols)
        np.testing.assert_array_equal(result, bsq)

    def test_decode_jbp_block_bil_to_bsq(self):
        """decode_jbp_block converts BIL (imode=R) to BSQ correctly.

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 2, 2, 3
        bsq = np.array([
            [[1, 2, 3], [4, 5, 6]],
            [[7, 8, 9], [10, 11, 12]],
        ], dtype=np.uint8)
        # BIL: row-interleaved — for each row, all bands in sequence
        bil = bytearray()
        for r in range(rows):
            for b in range(bands):
                bil.extend(bsq[b, r, :].tobytes())

        result = decode_jbp_block(
            bytes(bil), num_bands=bands, block_height=rows,
            block_width=cols, nbpp=8, imode="R", pvtype="INT",
        )
        assert result.shape == (bands, rows, cols)
        np.testing.assert_array_equal(result, bsq)

    def test_decode_jbp_block_all_interleave_modes(self):
        """decode_jbp_block handles all four interleave modes (B, P, R, S).

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 2, 2, 2
        bsq = np.arange(bands * rows * cols, dtype=np.uint8).reshape(bands, rows, cols)

        # B and S are both BSQ-equivalent for a single block
        for imode in ["B", "S"]:
            result = decode_jbp_block(
                bytes(bsq), num_bands=bands, block_height=rows,
                block_width=cols, nbpp=8, imode=imode, pvtype="INT",
            )
            np.testing.assert_array_equal(result, bsq, err_msg=f"Failed for imode={imode}")

        # P (BIP): pixel-interleaved
        bip = np.transpose(bsq, (1, 2, 0)).tobytes()
        result_p = decode_jbp_block(
            bip, num_bands=bands, block_height=rows,
            block_width=cols, nbpp=8, imode="P", pvtype="INT",
        )
        np.testing.assert_array_equal(result_p, bsq, err_msg="Failed for imode=P")

        # R (BIL): row-interleaved
        bil = bytearray()
        for r in range(rows):
            for b in range(bands):
                bil.extend(bsq[b, r, :].tobytes())
        result_r = decode_jbp_block(
            bytes(bil), num_bands=bands, block_height=rows,
            block_width=cols, nbpp=8, imode="R", pvtype="INT",
        )
        np.testing.assert_array_equal(result_r, bsq, err_msg="Failed for imode=R")

    # --- JBP block decode: pvtype/nbpp combinations ---

    @pytest.mark.parametrize("nbpp,pvtype,dtype", [
        (8, "INT", np.uint8),
        (16, "INT", np.uint16),
        (16, "SI", np.int16),
        (32, "R", np.float32),
        (64, "R", np.float64),
    ])
    def test_decode_jbp_block_pvtype_nbpp_combinations(self, nbpp, pvtype, dtype):
        """decode_jbp_block returns correct dtype for all valid pvtype/nbpp combos.

        Validates: Requirements 3.6
        """
        bands, rows, cols = 1, 2, 2
        bpp = nbpp // 8
        # Create zero-filled data in big-endian format (NITF native)
        data = b"\x00" * (bands * rows * cols * bpp)
        result = decode_jbp_block(
            data, num_bands=bands, block_height=rows,
            block_width=cols, nbpp=nbpp, imode="S", pvtype=pvtype,
        )
        assert result.shape == (bands, rows, cols)
        assert result.dtype == dtype

    def test_decode_jbp_block_uint16_values(self):
        """decode_jbp_block correctly decodes uint16 big-endian data.

        NITF stores multi-byte pixels in big-endian. The decoder should
        convert to native endian.

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 1, 2, 2
        # Create known uint16 values and convert to big-endian bytes
        values = np.array([[[256, 512], [1024, 2048]]], dtype=np.uint16)
        be_bytes = values.astype(">u2").tobytes()

        result = decode_jbp_block(
            be_bytes, num_bands=bands, block_height=rows,
            block_width=cols, nbpp=16, imode="S", pvtype="INT",
        )
        assert result.dtype == np.uint16
        np.testing.assert_array_equal(result, values)

    def test_decode_jbp_block_float32_values(self):
        """decode_jbp_block correctly decodes float32 big-endian data.

        Validates: Requirements 3.6, 3.7
        """
        bands, rows, cols = 1, 2, 2
        values = np.array([[[1.5, 2.5], [3.5, 4.5]]], dtype=np.float32)
        be_bytes = values.astype(">f4").tobytes()

        result = decode_jbp_block(
            be_bytes, num_bands=bands, block_height=rows,
            block_width=cols, nbpp=32, imode="S", pvtype="R",
        )
        assert result.dtype == np.float32
        np.testing.assert_array_equal(result, values)


class TestCodecABCConformance:
    """Verify codec classes subclass BytesBytesCodec and are discoverable via entry points.

    Validates: Requirements 1.1, 1.2, 1.3, 1.5
    """

    def test_jpeg2000_codec_is_bytes_bytes_codec(self):
        """Jpeg2000Codec is a subclass of zarr.abc.codec.BytesBytesCodec."""
        from zarr.abc.codec import BytesBytesCodec

        assert issubclass(Jpeg2000Codec, BytesBytesCodec)

    def test_jpeg_codec_is_bytes_bytes_codec(self):
        """JpegCodec is a subclass of zarr.abc.codec.BytesBytesCodec."""
        from zarr.abc.codec import BytesBytesCodec

        assert issubclass(JpegCodec, BytesBytesCodec)

    def test_jbp_block_codec_is_bytes_bytes_codec(self):
        """JbpBlockCodec is a subclass of zarr.abc.codec.BytesBytesCodec."""
        from zarr.abc.codec import BytesBytesCodec

        assert issubclass(JbpBlockCodec, BytesBytesCodec)

    def test_entry_point_resolves_jpeg2000(self):
        """Zarr codec registry resolves the JPEG 2000 URI to Jpeg2000Codec."""
        from zarr.registry import get_codec_class

        cls = get_codec_class("https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000")
        assert cls is Jpeg2000Codec

    def test_entry_point_resolves_jpeg(self):
        """Zarr codec registry resolves the JPEG URI to JpegCodec."""
        from zarr.registry import get_codec_class

        cls = get_codec_class("https://awslabs.github.io/osml-imagery-io/codecs/jpeg")
        assert cls is JpegCodec

    def test_entry_point_resolves_jbp_block(self):
        """Zarr codec registry resolves the JBP block URI to JbpBlockCodec."""
        from zarr.registry import get_codec_class

        cls = get_codec_class("https://awslabs.github.io/osml-imagery-io/codecs/jbp-block")
        assert cls is JbpBlockCodec

    # NOTE: Testing that importing codecs without zarr installed raises ImportError
    # is not feasible in this test suite. The zarr package is already imported at
    # module level (it's a test dependency and required for the codec classes to be
    # defined). Mocking the zarr import would require patching sys.modules before
    # the codec module is loaded, but since the codec classes inherit from
    # BytesBytesCodec at class definition time (not at call time), unloading and
    # reloading the module with zarr mocked out would break the already-defined
    # classes in this test file. A true test would require a subprocess with zarr
    # uninstalled, which is outside the scope of unit tests.
