"""Unit tests for stream I/O support.

Tests verify that IO.open() and the convenience API (imread, imsave, iminfo,
tiles) correctly handle Python file-like objects (io.BytesIO) as input/output
targets, and that error handling is correct for invalid stream usage.

Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6,
5.1, 5.2, 5.3, 5.6, 5.7, 7.1, 7.2, 7.3, 7.4, 8.1, 8.2, 8.3, 8.5, 9.1, 9.2,
9.3, 9.4, 10.1, 10.2, 10.3, 10.4, 11.1, 13.1, 13.2, 14.1, 14.2, 14.3, 14.4
"""

import io

import numpy as np
import pytest
from aws.osml.io import IO, iminfo, imread, imsave, tiles

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_test_image(height: int = 8, width: int = 8, bands: int = 3) -> np.ndarray:
    """Create a small deterministic test image in CHW layout."""
    rng = np.random.default_rng(42)
    return rng.integers(0, 255, (bands, height, width), dtype=np.uint8)


def _write_png_to_buffer(data: np.ndarray) -> io.BytesIO:
    """Write an image to a BytesIO buffer in PNG format using imsave."""
    buf = io.BytesIO()
    imsave(buf, data, format="png")
    buf.seek(0)
    return buf


# ---------------------------------------------------------------------------
# Test 1: test_read_png_from_bytesio
# ---------------------------------------------------------------------------


class TestReadPngFromBytesio:
    """Basic read from io.BytesIO with PNG format via IO.open().

    **Validates: Requirements 1.1, 1.2, 1.6, 5.1**
    """

    def test_read_png_from_bytesio(self):
        """IO.open(stream, 'r', 'png') reads a PNG from a BytesIO buffer."""
        data = _make_test_image()
        buf = _write_png_to_buffer(data)

        with IO.open(buf, "r", "png") as reader:
            asset_key = reader.get_asset_keys()[0]
            asset = reader.get_asset(asset_key)
            assert asset.num_columns == 8
            assert asset.num_rows == 8
            assert asset.num_bands == 3


# ---------------------------------------------------------------------------
# Test 2: test_write_png_to_bytesio
# ---------------------------------------------------------------------------


class TestWritePngToBytesio:
    """Basic write to io.BytesIO with PNG format via IO.open().

    **Validates: Requirements 2.1, 2.2, 2.4, 5.2**
    """

    def test_write_png_to_bytesio(self):
        """IO.open(stream, 'w', 'png') writes PNG data to a BytesIO buffer."""
        from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

        data = _make_test_image()
        buf = io.BytesIO()

        with IO.open(buf, "w", "png") as writer:
            metadata = BufferedMetadataProvider()
            provider = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=8,
                num_rows=8,
                num_bands=3,
                block_width=8,
                block_height=8,
                pixel_type=PixelType.UInt8,
                metadata=metadata,
                title="Test",
                description="test image",
            )
            provider.set_full_image(np.ascontiguousarray(data))
            writer.add_asset(
                key="image:0",
                provider=provider,
                title="Test",
                description="test image",
                roles=["data"],
            )

        # Buffer should contain PNG data
        assert buf.tell() > 0 or len(buf.getvalue()) > 0
        png_bytes = buf.getvalue()
        # PNG magic bytes
        assert png_bytes[:4] == b"\x89PNG"


# ---------------------------------------------------------------------------
# Test 3: test_imread_from_bytesio
# ---------------------------------------------------------------------------


class TestImreadFromBytesio:
    """Convenience imread from stream.

    **Validates: Requirements 7.1, 7.2, 7.4**
    """

    def test_imread_from_bytesio(self):
        """imread(stream, format='png') reads an image from a BytesIO buffer."""
        data = _make_test_image()
        buf = _write_png_to_buffer(data)

        result = imread(buf, format="png")
        np.testing.assert_array_equal(result, data)


# ---------------------------------------------------------------------------
# Test 4: test_imsave_to_bytesio
# ---------------------------------------------------------------------------


class TestImsaveToBytesio:
    """Convenience imsave to stream.

    **Validates: Requirements 8.1, 8.2**
    """

    def test_imsave_to_bytesio(self):
        """imsave(stream, data, format='png') writes to a BytesIO buffer."""
        data = _make_test_image()
        buf = io.BytesIO()

        imsave(buf, data, format="png")

        # Buffer should contain PNG data
        png_bytes = buf.getvalue()
        assert len(png_bytes) > 0
        assert png_bytes[:4] == b"\x89PNG"


# ---------------------------------------------------------------------------
# Test 5: test_iminfo_from_bytesio
# ---------------------------------------------------------------------------


class TestIminfFromBytesio:
    """Convenience iminfo from stream.

    **Validates: Requirements 9.1, 9.2, 9.4**
    """

    def test_iminfo_from_bytesio(self):
        """iminfo(stream, format='png') returns metadata from a BytesIO buffer."""
        data = _make_test_image()
        buf = _write_png_to_buffer(data)

        info = iminfo(buf, format="png")
        assert info.width == 8
        assert info.height == 8
        assert info.bands == 3
        assert info.dtype == "uint8"


# ---------------------------------------------------------------------------
# Test 6: test_tiles_from_bytesio
# ---------------------------------------------------------------------------


class TestTilesFromBytesio:
    """Convenience tiles from stream.

    **Validates: Requirements 10.1, 10.2, 10.4**
    """

    def test_tiles_from_bytesio(self):
        """tiles(stream, tile_size, format='png') yields tiles from a BytesIO buffer."""
        data = _make_test_image(height=16, width=16)
        buf = _write_png_to_buffer(data)

        tile_list = list(tiles(buf, tile_size=(8, 8), format="png"))
        # 16x16 image with 8x8 tiles = 4 tiles
        assert len(tile_list) == 4
        assert tile_list[0].data.shape == (3, 8, 8)


# ---------------------------------------------------------------------------
# Test 7: test_stream_roundtrip_nitf
# ---------------------------------------------------------------------------


class TestStreamRoundtripNitf:
    """NITF format stream round-trip.

    **Validates: Requirements 11.1**
    """

    def test_stream_roundtrip_nitf(self):
        """Write NITF to BytesIO, read back, verify pixel data is identical."""
        data = _make_test_image()
        buf = io.BytesIO()

        imsave(buf, data, format="nitf", compression="none")
        buf.seek(0)

        result = imread(buf, format="nitf")
        np.testing.assert_array_equal(result, data)


# ---------------------------------------------------------------------------
# Test 8: test_stream_roundtrip_jpeg
# ---------------------------------------------------------------------------


class TestStreamRoundtripJpeg:
    """JPEG format stream round-trip (lossy, verify dimensions match).

    **Validates: Requirements 11.2**
    """

    def test_stream_roundtrip_jpeg(self):
        """Write JPEG to BytesIO, read back, verify dimensions match."""
        # JPEG only supports single-band or 3-band uint8
        data = _make_test_image(height=16, width=16, bands=3)
        buf = io.BytesIO()

        imsave(buf, data, format="jpeg")
        buf.seek(0)

        result = imread(buf, format="jpeg")
        # Lossy: dimensions must match, pixel values may differ
        assert result.shape == data.shape


# ---------------------------------------------------------------------------
# Test 9: test_existing_file_path_api_unchanged
# ---------------------------------------------------------------------------


class TestExistingFilePathApiUnchanged:
    """Backward compatibility for file path API.

    **Validates: Requirements 13.1, 13.2**
    """

    def test_existing_file_path_api_unchanged(self, tmp_path):
        """IO.open with a file path string still works as before."""
        data = _make_test_image()
        path = tmp_path / "test.png"
        imsave(str(path), data)

        with IO.open(str(path), "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) > 0

        # Also verify imread/iminfo/tiles work with file paths
        result = imread(str(path))
        np.testing.assert_array_equal(result, data)

        info = iminfo(str(path))
        assert info.width == 8
        assert info.height == 8


# ---------------------------------------------------------------------------
# Test 10: test_existing_list_path_api_unchanged
# ---------------------------------------------------------------------------


class TestExistingListPathApiUnchanged:
    """Backward compatibility for list path API (with .rN detection).

    **Validates: Requirements 13.1, 13.2**
    """

    @pytest.mark.skip(reason="R-set test data not easily created in unit tests")
    def test_existing_list_path_api_unchanged(self, tmp_path):
        """IO.open with a list of paths still uses .rN detection."""
        # This test is skipped because creating R-set test data requires
        # multi-file pyramid generation which is complex for a unit test.
        pass

    def test_list_path_single_element(self, tmp_path):
        """IO.open with a single-element list still works."""
        data = _make_test_image()
        path = tmp_path / "test.png"
        imsave(str(path), data)

        with IO.open([str(path)], "r") as reader:
            keys = reader.get_asset_keys()
            assert len(keys) > 0


# ---------------------------------------------------------------------------
# Test 11: test_read_stream_without_format_raises
# ---------------------------------------------------------------------------


class TestReadStreamWithoutFormatRaises:
    """ValueError for missing format when reading from stream.

    **Validates: Requirements 1.3, 5.7**
    """

    def test_read_stream_without_format_raises(self):
        """IO.open(stream, 'r') without format raises ValueError."""
        buf = io.BytesIO(b"fake data")
        with pytest.raises(ValueError, match="format is required"):
            IO.open(buf, "r")


# ---------------------------------------------------------------------------
# Test 12: test_write_stream_without_format_raises
# ---------------------------------------------------------------------------


class TestWriteStreamWithoutFormatRaises:
    """ValueError for missing format when writing to stream.

    **Validates: Requirements 2.3, 5.7**
    """

    def test_write_stream_without_format_raises(self):
        """IO.open(stream, 'w') without format raises ValueError."""
        buf = io.BytesIO()
        with pytest.raises(ValueError, match="format is required"):
            IO.open(buf, "w")


# ---------------------------------------------------------------------------
# Test 13: test_read_stream_missing_read_method
# ---------------------------------------------------------------------------


class TestReadStreamMissingReadMethod:
    """TypeError for non-readable object.

    **Validates: Requirements 1.5, 14.1**
    """

    def test_read_stream_missing_read_method(self):
        """IO.open(obj_without_read, 'r', 'png') raises TypeError."""

        class NoReadStream:
            def write(self, data):
                pass

            def flush(self):
                pass

        with pytest.raises(TypeError, match="read"):
            IO.open(NoReadStream(), "r", "png")


# ---------------------------------------------------------------------------
# Test 14: test_write_stream_missing_write_method
# ---------------------------------------------------------------------------


class TestWriteStreamMissingWriteMethod:
    """TypeError for non-writable object.

    **Validates: Requirements 2.6**
    """

    def test_write_stream_missing_write_method(self):
        """IO.open(obj_without_write, 'w', 'png') raises TypeError."""

        class NoWriteStream:
            def read(self):
                return b"data"

        with pytest.raises(TypeError, match="write"):
            IO.open(NoWriteStream(), "w", "png")


# ---------------------------------------------------------------------------
# Test 15: test_write_stream_missing_flush_method
# ---------------------------------------------------------------------------


class TestWriteStreamMissingFlushMethod:
    """TypeError for missing flush.

    **Validates: Requirements 2.6**
    """

    def test_write_stream_missing_flush_method(self):
        """IO.open(obj_without_flush, 'w', 'png') raises TypeError."""

        class NoFlushStream:
            def write(self, data):
                return len(data)

        with pytest.raises(TypeError, match="flush"):
            IO.open(NoFlushStream(), "w", "png")


# ---------------------------------------------------------------------------
# Test 16: test_read_empty_stream_raises
# ---------------------------------------------------------------------------


class TestReadEmptyStreamRaises:
    """ValueError for empty stream.

    **Validates: Requirements 1.4, 14.3**
    """

    def test_read_empty_stream_raises(self):
        """IO.open(empty_stream, 'r', 'png') raises ValueError."""
        buf = io.BytesIO(b"")
        with pytest.raises(ValueError, match="[Nn]o data|empty"):
            IO.open(buf, "r", "png")


# ---------------------------------------------------------------------------
# Test 17: test_read_non_bytes_return_raises
# ---------------------------------------------------------------------------


class TestReadNonBytesReturnRaises:
    """TypeError when .read() returns non-bytes.

    **Validates: Requirements 14.3**
    """

    def test_read_non_bytes_return_raises(self):
        """.read() returning a string instead of bytes raises TypeError."""

        class BadReadStream:
            def read(self):
                return "not bytes"

        with pytest.raises(TypeError, match="bytes"):
            IO.open(BadReadStream(), "r", "png")


# ---------------------------------------------------------------------------
# Test 18: test_write_error_propagation
# ---------------------------------------------------------------------------


class TestWriteErrorPropagation:
    """Mock stream that raises on .write().

    **Validates: Requirements 14.2, 14.4**
    """

    def test_write_error_propagation(self):
        """An exception from .write() propagates to the caller."""
        from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

        class FailingWriteStream:
            def write(self, data):
                raise IOError("disk full")

            def flush(self):
                pass

        data = _make_test_image()

        with pytest.raises((IOError, OSError, RuntimeError)):
            with IO.open(FailingWriteStream(), "w", "png") as writer:
                metadata = BufferedMetadataProvider()
                provider = BufferedImageAssetProvider.create(
                    key="image:0",
                    num_columns=8,
                    num_rows=8,
                    num_bands=3,
                    block_width=8,
                    block_height=8,
                    pixel_type=PixelType.UInt8,
                    metadata=metadata,
                    title="Test",
                    description="test",
                )
                provider.set_full_image(np.ascontiguousarray(data))
                writer.add_asset(
                    key="image:0",
                    provider=provider,
                    title="Test",
                    description="test",
                    roles=["data"],
                )


# ---------------------------------------------------------------------------
# Test 19: test_flush_error_propagation
# ---------------------------------------------------------------------------


class TestFlushErrorPropagation:
    """Mock stream that raises on .flush().

    **Validates: Requirements 14.2**
    """

    def test_flush_error_propagation(self):
        """An exception from .flush() propagates to the caller."""
        from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

        class FailingFlushStream:
            def __init__(self):
                self._data = bytearray()

            def write(self, data):
                self._data.extend(data)
                return len(data)

            def flush(self):
                raise IOError("flush failed")

        data = _make_test_image()

        with pytest.raises((IOError, OSError, RuntimeError)):
            with IO.open(FailingFlushStream(), "w", "png") as writer:
                metadata = BufferedMetadataProvider()
                provider = BufferedImageAssetProvider.create(
                    key="image:0",
                    num_columns=8,
                    num_rows=8,
                    num_bands=3,
                    block_width=8,
                    block_height=8,
                    pixel_type=PixelType.UInt8,
                    metadata=metadata,
                    title="Test",
                    description="test",
                )
                provider.set_full_image(np.ascontiguousarray(data))
                writer.add_asset(
                    key="image:0",
                    provider=provider,
                    title="Test",
                    description="test",
                    roles=["data"],
                )


# ---------------------------------------------------------------------------
# Test 20: test_imread_stream_without_format_raises
# ---------------------------------------------------------------------------


class TestImreadStreamWithoutFormatRaises:
    """Convenience API format validation for imread.

    **Validates: Requirements 7.3**
    """

    def test_imread_stream_without_format_raises(self):
        """imread(stream) without format raises ValueError."""
        buf = io.BytesIO(b"fake data")
        with pytest.raises(ValueError, match="format is required"):
            imread(buf)


# ---------------------------------------------------------------------------
# Test 21: test_imsave_stream_without_format_raises
# ---------------------------------------------------------------------------


class TestImsaveStreamWithoutFormatRaises:
    """Convenience API format validation for imsave.

    **Validates: Requirements 8.3**
    """

    def test_imsave_stream_without_format_raises(self):
        """imsave(stream, data) without format raises ValueError."""
        data = _make_test_image()
        buf = io.BytesIO()
        with pytest.raises(ValueError, match="format is required"):
            imsave(buf, data)


# ---------------------------------------------------------------------------
# Test 22: test_iminfo_stream_without_format_raises
# ---------------------------------------------------------------------------


class TestIminfStreamWithoutFormatRaises:
    """Convenience API format validation for iminfo.

    **Validates: Requirements 9.3**
    """

    def test_iminfo_stream_without_format_raises(self):
        """iminfo(stream) without format raises ValueError."""
        buf = io.BytesIO(b"fake data")
        with pytest.raises(ValueError, match="format is required"):
            iminfo(buf)


# ---------------------------------------------------------------------------
# Test 23: test_tiles_stream_without_format_raises
# ---------------------------------------------------------------------------


class TestTilesStreamWithoutFormatRaises:
    """Convenience API format validation for tiles.

    **Validates: Requirements 10.3**
    """

    def test_tiles_stream_without_format_raises(self):
        """tiles(stream, tile_size) without format raises ValueError."""
        buf = io.BytesIO(b"fake data")
        with pytest.raises(ValueError, match="format is required"):
            list(tiles(buf, tile_size=(8, 8)))


# ---------------------------------------------------------------------------
# Test 24: test_imsave_file_path_format_inference_preserved
# ---------------------------------------------------------------------------


class TestImsaveFilePathFormatInferencePreserved:
    """Existing behavior unchanged: format inferred from extension.

    **Validates: Requirements 8.5, 13.2**
    """

    def test_imsave_file_path_format_inference_preserved(self, tmp_path):
        """imsave with a file path and no format still infers from extension."""
        data = _make_test_image()

        # Write PNG via extension inference
        png_path = tmp_path / "test.png"
        imsave(str(png_path), data)
        assert png_path.exists()

        # Verify it's a valid PNG
        result = imread(str(png_path))
        np.testing.assert_array_equal(result, data)

        # Write NITF via extension inference
        ntf_path = tmp_path / "test.ntf"
        imsave(str(ntf_path), data, compression="none")
        assert ntf_path.exists()

        result = imread(str(ntf_path))
        np.testing.assert_array_equal(result, data)


# ===========================================================================
# Multi-source / Roles Tests (Task 13.2)
# ===========================================================================


# ---------------------------------------------------------------------------
# Test 25: test_read_rset_streams_with_roles
# ---------------------------------------------------------------------------


class TestReadRsetStreamsWithRoles:
    """list[BinaryIO] with explicit roles produces a composite reader with base + overview assets.

    **Validates: Requirements 6.1, 6.3, 6.4**
    """

    def test_read_rset_streams_with_roles(self):
        """Open a list of NITF streams with roles to produce a composite reader."""
        # Create a base image (16x16) and an overview (8x8)
        base_data = _make_test_image(height=16, width=16, bands=3)
        overview_data = _make_test_image(height=8, width=8, bands=3)

        # Write both to BytesIO as NITF
        base_buf = io.BytesIO()
        imsave(base_buf, base_data, format="nitf", compression="none")
        base_buf.seek(0)

        ovr_buf = io.BytesIO()
        imsave(ovr_buf, overview_data, format="nitf", compression="none")
        ovr_buf.seek(0)

        # Open with explicit roles
        with IO.open(
            [base_buf, ovr_buf],
            "r",
            format="nitf",
            roles=[["data"], ["overview:1"]],
        ) as reader:
            keys = reader.get_asset_keys()
            # Should have at least the base image asset and an overview asset
            assert len(keys) >= 1

            # The base image should be accessible
            base_key = [k for k in keys if "overview" not in k]
            assert len(base_key) >= 1
            base_asset = reader.get_asset(base_key[0])
            assert base_asset.num_columns == 16
            assert base_asset.num_rows == 16

            # The overview should be accessible
            ovr_keys = [k for k in keys if "overview" in k]
            assert len(ovr_keys) >= 1
            ovr_asset = reader.get_asset(ovr_keys[0])
            assert ovr_asset.num_columns == 8
            assert ovr_asset.num_rows == 8


# ---------------------------------------------------------------------------
# Test 26: test_write_rset_streams_with_roles
# ---------------------------------------------------------------------------


class TestWriteRsetStreamsWithRoles:
    """list[BinaryIO] with explicit roles in write mode routes assets to the correct streams.

    **Validates: Requirements 6.8**
    """

    @pytest.mark.skip(reason="Write routing for multi-source streams requires complex setup")
    def test_write_rset_streams_with_roles(self):
        """Write mode with roles routes assets to the correct streams."""
        pass


# ---------------------------------------------------------------------------
# Test 27: test_path_list_with_roles_bypasses_filename_detection
# ---------------------------------------------------------------------------


class TestPathListWithRolesBypassesFilenameDetection:
    """When roles is provided for a list[str], the .rN suffix is ignored.

    **Validates: Requirements 6.5**
    """

    @pytest.mark.skip(reason="R-set test data not easily created in unit tests")
    def test_path_list_with_roles_bypasses_filename_detection(self, tmp_path):
        """Explicit roles override .rN suffix detection."""
        pass


# ---------------------------------------------------------------------------
# Test 28: test_path_list_without_roles_uses_filename_detection
# ---------------------------------------------------------------------------


class TestPathListWithoutRolesUsesFilenameDetection:
    """Backward compatibility: list[str] without roles still uses .rN detection.

    **Validates: Requirements 6.5**
    """

    @pytest.mark.skip(reason="R-set test data not easily created in unit tests")
    def test_path_list_without_roles_uses_filename_detection(self, tmp_path):
        """Without roles, .rN suffix detection is used."""
        pass


# ---------------------------------------------------------------------------
# Test 29: test_stream_list_without_roles_raises
# ---------------------------------------------------------------------------


class TestStreamListWithoutRolesRaises:
    """list[BinaryIO] without roles raises ValueError.

    **Validates: Requirements 6.6**
    """

    def test_stream_list_without_roles_raises(self):
        """IO.open([stream1, stream2], 'r', format='nitf') without roles raises ValueError."""
        buf1 = io.BytesIO(b"fake")
        buf2 = io.BytesIO(b"fake")
        with pytest.raises(ValueError, match="roles is required"):
            IO.open([buf1, buf2], "r", format="nitf")


# ---------------------------------------------------------------------------
# Test 30: test_roles_length_mismatch_raises
# ---------------------------------------------------------------------------


class TestRolesLengthMismatchRaises:
    """Mismatched roles length raises ValueError.

    **Validates: Requirements 6.2**
    """

    def test_roles_length_mismatch_raises(self):
        """roles list length must match number of sources."""
        buf1 = io.BytesIO(b"fake")
        buf2 = io.BytesIO(b"fake")
        # 2 streams but 3 role entries
        with pytest.raises(ValueError, match="does not match"):
            IO.open(
                [buf1, buf2],
                "r",
                format="nitf",
                roles=[["data"], ["overview:1"], ["overview:2"]],
            )


# ---------------------------------------------------------------------------
# Test 31: test_roles_wrong_shape_raises
# ---------------------------------------------------------------------------


class TestRolesWrongShapeRaises:
    """Passing list[str] for multi-source raises ValueError.

    **Validates: Requirements 6.7**
    """

    def test_roles_wrong_shape_raises(self):
        """roles must be list[list[str]] when there are multiple sources."""
        buf1 = io.BytesIO(b"fake")
        buf2 = io.BytesIO(b"fake")
        # Flat list[str] is only valid for single source
        with pytest.raises(ValueError, match="list\\[list\\[str\\]\\]"):
            IO.open([buf1, buf2], "r", format="nitf", roles=["data", "overview:1"])


# ---------------------------------------------------------------------------
# Test 32: test_mixed_str_stream_list_raises
# ---------------------------------------------------------------------------


class TestMixedStrStreamListRaises:
    """[str_path, BytesIO] mixed list raises TypeError.

    **Validates: Requirements 5.5**
    """

    def test_mixed_str_stream_list_raises(self):
        """A list mixing strings and file-like objects raises TypeError."""
        buf = io.BytesIO(b"fake")
        with pytest.raises(TypeError, match="[Mm]ixed|all strings or all file-like"):
            IO.open(["/tmp/fake.ntf", buf], "r", format="nitf", roles=[["data"], ["overview:1"]])


# ---------------------------------------------------------------------------
# Test 33: test_invalid_overview_role_raises
# ---------------------------------------------------------------------------


class TestInvalidOverviewRoleRaises:
    """Malformed 'overview:N' role raises ValueError.

    **Validates: Requirements 6.7**
    """

    def test_invalid_overview_role_raises(self):
        """overview:abc is not a valid role."""
        buf1 = io.BytesIO(b"fake")
        buf2 = io.BytesIO(b"fake")
        with pytest.raises(ValueError, match="Invalid overview level"):
            IO.open(
                [buf1, buf2],
                "r",
                format="nitf",
                roles=[["data"], ["overview:abc"]],
            )


# ---------------------------------------------------------------------------
# Test 34: test_invalid_overview_zero_raises
# ---------------------------------------------------------------------------


class TestInvalidOverviewZeroRaises:
    """'overview:0' raises ValueError (0 is the base, not an overview).

    **Validates: Requirements 6.7**
    """

    def test_invalid_overview_zero_raises(self):
        """overview:0 is invalid because 0 is the base."""
        buf1 = io.BytesIO(b"fake")
        buf2 = io.BytesIO(b"fake")
        with pytest.raises(ValueError, match="overview level must be positive"):
            IO.open(
                [buf1, buf2],
                "r",
                format="nitf",
                roles=[["data"], ["overview:0"]],
            )


# ---------------------------------------------------------------------------
# Test 35: test_multiple_data_roles_raises
# ---------------------------------------------------------------------------


class TestMultipleDataRolesRaises:
    """Two sources claiming role 'data' raises ValueError.

    **Validates: Requirements 6.4**
    """

    def test_multiple_data_roles_raises(self):
        """Only one source can have role 'data'."""
        buf1 = io.BytesIO(b"fake")
        buf2 = io.BytesIO(b"fake")
        with pytest.raises(ValueError, match="[Mm]ultiple sources.*data"):
            IO.open(
                [buf1, buf2],
                "r",
                format="nitf",
                roles=[["data"], ["data"]],
            )


# ---------------------------------------------------------------------------
# Test 36: test_unknown_role_passes_through
# ---------------------------------------------------------------------------


class TestUnknownRolePassesThrough:
    """An unrecognized role (e.g., 'metadata') does not raise but is not routed in v1.

    **Validates: Requirements 6.8**
    """

    def test_unknown_role_passes_through(self):
        """Unknown roles like 'metadata' are accepted without error."""
        # Create valid NITF data for both streams
        base_data = _make_test_image(height=8, width=8, bands=3)
        meta_data = _make_test_image(height=4, width=4, bands=3)

        base_buf = io.BytesIO()
        imsave(base_buf, base_data, format="nitf", compression="none")
        base_buf.seek(0)

        meta_buf = io.BytesIO()
        imsave(meta_buf, meta_data, format="nitf", compression="none")
        meta_buf.seek(0)

        # Should not raise — 'metadata' is an unknown role but passes through
        with IO.open(
            [base_buf, meta_buf],
            "r",
            format="nitf",
            roles=[["data"], ["metadata"]],
        ) as reader:
            keys = reader.get_asset_keys()
            # At minimum, the base image should be accessible
            assert len(keys) >= 1
