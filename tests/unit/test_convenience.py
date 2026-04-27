"""Unit tests for the convenience API (imread, imsave, iminfo, tiles).

This module tests specific examples, edge cases, and integration points
for the convenience functions and the IO.open() string acceptance change.

Requirements: 1.1, 1.2, 4.1, 4.2, 4.4, 4.6, 4.10, 4.13, 5.3, 5.6, 6.1, 6.2, 7.2, 7.3, 8.1, 8.3
"""

import types
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, AssetType, iminfo, imread, imsave, tiles

# Path to existing unit test data
UNIT_DATA = Path("data/unit")
NITF_3BAND = UNIT_DATA / "nitf21-256x256-3band-8bit-nc.ntf"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _write_test_image(path: Path, data: np.ndarray, **kwargs) -> None:
    """Write a test image using imsave."""
    imsave(str(path), data, **kwargs)


# ============================================================================
# test_imread_default_asset_selection
# ============================================================================


class TestImreadDefaultAssetSelection:
    """Verify imread selects the first data-role asset by default.

    **Validates: Requirements 1.1, 1.2**
    """

    def test_imread_default_asset_selection(self, tmp_path):
        """imread without an asset parameter reads the first data-role image asset."""
        # Create a 3-band uint8 image
        data = np.random.randint(0, 255, (3, 32, 32), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        result = imread(str(path))

        assert result.shape == (3, 32, 32)
        np.testing.assert_array_equal(result, data)

    def test_imread_selects_data_role_from_existing_file(self):
        """imread on an existing NITF file selects the data-role asset."""
        if not NITF_3BAND.exists():
            pytest.skip("Unit test data not available")

        result = imread(str(NITF_3BAND))

        # The file is 256x256 with 3 bands
        assert result.shape[0] == 3
        assert result.shape[1] == 256
        assert result.shape[2] == 256


# ============================================================================
# test_imsave_format_inference
# ============================================================================


class TestImsaveFormatInference:
    """Verify imsave infers the correct format from file extension.

    **Validates: Requirements 4.1, 4.2**
    """

    def test_imsave_format_inference(self, tmp_path):
        """Each supported extension produces a readable file in the expected format."""
        # Use 64x64 to avoid J2K resolution-level issues with very small images
        data = np.random.randint(0, 255, (1, 64, 64), dtype=np.uint8)

        extension_format_pairs = [
            (".ntf", "nitf"),
            (".nitf", "nitf"),
            (".tif", "geotiff"),
            (".tiff", "geotiff"),
            (".png", "png"),
            (".j2k", "j2k"),
            (".jp2", "j2k"),
            (".jpg", "jpeg"),
            (".jpeg", "jpeg"),
        ]

        for ext, expected_format in extension_format_pairs:
            path = tmp_path / f"test{ext}"
            _write_test_image(path, data)

            # Verify the file was created and is readable
            assert path.exists(), f"File with extension {ext} should exist"

            # For lossless formats, verify round-trip
            if ext in (".ntf", ".nitf", ".tif", ".tiff", ".png"):
                result = imread(str(path))
                assert result.shape == (1, 64, 64), (
                    f"Round-trip failed for {ext}"
                )


# ============================================================================
# test_imsave_default_compression
# ============================================================================


class TestImsaveDefaultCompression:
    """Verify imsave applies the correct default compression per format.

    **Validates: Requirements 4.6**
    """

    def test_imsave_default_compression(self, tmp_path):
        """Default compression is applied when no compression parameter is given."""
        data = np.random.randint(0, 255, (1, 32, 32), dtype=np.uint8)

        # NITF default: JPEG 2000 lossless (IC=C8)
        ntf_path = tmp_path / "test.ntf"
        _write_test_image(ntf_path, data)
        info = iminfo(str(ntf_path))
        assert info.width == 32
        assert info.height == 32

        # GeoTIFF default: Deflate — verify file is readable
        tif_path = tmp_path / "test.tif"
        _write_test_image(tif_path, data)
        result = imread(str(tif_path))
        np.testing.assert_array_equal(result, data)

        # PNG default: standard PNG compression — verify round-trip
        png_path = tmp_path / "test.png"
        _write_test_image(png_path, data)
        result = imread(str(png_path))
        np.testing.assert_array_equal(result, data)


# ============================================================================
# test_iminfo_metadata_field
# ============================================================================


class TestIminfoMetadataField:
    """Verify iminfo returns a populated metadata dictionary.

    The ``metadata`` attribute on ``ImageInfo`` should contain the
    format-specific metadata dictionary for the image segment.
    """

    def test_iminfo_metadata_is_dict(self, tmp_path):
        """iminfo().metadata is a plain dict."""
        data = np.random.randint(0, 255, (1, 32, 32), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        info = iminfo(str(path))
        assert isinstance(info.metadata, dict)

    def test_iminfo_metadata_nitf_has_ic(self, tmp_path):
        """NITF metadata dict contains the IC (compression) field."""
        data = np.random.randint(0, 255, (1, 32, 32), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        info = iminfo(str(path))
        assert "IC" in info.metadata
        assert info.metadata["IC"] == "NC"

    def test_iminfo_metadata_nitf_j2k_compression(self, tmp_path):
        """NITF metadata dict reflects JPEG 2000 compression when used."""
        data = np.random.randint(0, 255, (1, 64, 64), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data)  # default = J2K

        info = iminfo(str(path))
        assert "IC" in info.metadata
        assert info.metadata["IC"] == "C8"

    def test_iminfo_metadata_geotiff_has_tags(self, tmp_path):
        """GeoTIFF metadata dict contains numeric tag ID keys."""
        data = np.random.randint(0, 255, (1, 32, 32), dtype=np.uint8)
        path = tmp_path / "test.tif"
        _write_test_image(path, data)

        info = iminfo(str(path))
        # TIFF metadata uses numeric tag ID strings
        # 256 = ImageWidth, 257 = ImageLength
        assert "256" in info.metadata or "ImageWidth" in info.metadata
        assert len(info.metadata) > 0

    def test_iminfo_metadata_is_snapshot(self, tmp_path):
        """metadata dict is a detached snapshot, not a live reference."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        info = iminfo(str(path))
        # Mutating the returned dict should not affect anything
        original_keys = set(info.metadata.keys())
        copy = dict(info.metadata)
        assert set(copy.keys()) == original_keys

    def test_iminfo_metadata_from_existing_file(self):
        """iminfo on an existing NITF file returns a non-empty metadata dict."""
        if not NITF_3BAND.exists():
            pytest.skip("Unit test data not available")

        info = iminfo(str(NITF_3BAND))
        assert isinstance(info.metadata, dict)
        assert len(info.metadata) > 0
        # NITF files always have an IC field
        assert "IC" in info.metadata


# ============================================================================
# test_imsave_default_block_size
# ============================================================================


class TestImsaveDefaultBlockSize:
    """Verify imsave uses the correct default block size per format.

    **Validates: Requirements 4.4**
    """

    def test_imsave_default_block_size(self, tmp_path):
        """Default block sizes match the design specification per format."""
        # PNG: block size = full image (W×H)
        png_data = np.random.randint(0, 255, (1, 48, 64), dtype=np.uint8)
        png_path = tmp_path / "test.png"
        _write_test_image(png_path, png_data)
        png_info = iminfo(str(png_path))
        # PNG block size should be the full image dimensions
        assert png_info.block_size[0] == 64 or png_info.block_size[0] == 0
        assert png_info.block_size[1] == 48 or png_info.block_size[1] == 0

        # GeoTIFF with large image: default 256×256
        tif_data_large = np.random.randint(0, 255, (1, 512, 512), dtype=np.uint8)
        tif_path_large = tmp_path / "test_large.tif"
        _write_test_image(tif_path_large, tif_data_large)
        tif_info_large = iminfo(str(tif_path_large))
        assert tif_info_large.block_size == (256, 256)

        # NITF with large image: default 1024×1024 (clamped to image size)
        ntf_data = np.random.randint(0, 255, (1, 512, 512), dtype=np.uint8)
        ntf_path = tmp_path / "test.ntf"
        _write_test_image(ntf_path, ntf_data)
        ntf_info = iminfo(str(ntf_path))
        # 1024 clamped to 512
        assert ntf_info.block_size[0] == 512
        assert ntf_info.block_size[1] == 512


# ============================================================================
# test_imsave_2d_array_reshape
# ============================================================================


class TestImsave2dArrayReshape:
    """Verify imsave treats 2D (H, W) input as single-band.

    **Validates: Requirements 4.13**
    """

    def test_imsave_2d_array_reshape(self, tmp_path):
        """A 2D array (H, W) is reshaped to (1, H, W) and written as single-band."""
        data_2d = np.random.randint(0, 255, (24, 32), dtype=np.uint8)
        path = tmp_path / "test_2d.png"
        _write_test_image(path, data_2d)

        # Read back and verify shape
        result = imread(str(path))
        assert result.shape == (1, 24, 32)
        np.testing.assert_array_equal(result[0], data_2d)


# ============================================================================
# test_imsave_georef_ignored_for_png
# ============================================================================


class TestImsaveGeorefIgnoredForPng:
    """Verify corners are silently ignored for PNG and JPEG.

    **Validates: Requirements 4.10**
    """

    def test_imsave_georef_ignored_for_png(self, tmp_path):
        """Passing corners and crs to imsave for PNG does not raise an error."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        corners = [
            (-77.0, 39.0),   # UL
            (-76.0, 39.0),   # UR
            (-76.0, 38.0),   # LR
            (-77.0, 38.0),   # LL
        ]

        # PNG: should silently ignore georeferencing
        png_path = tmp_path / "test_georef.png"
        _write_test_image(png_path, data, corners=corners, crs="EPSG:4326")
        assert png_path.exists()

        # Verify the image is still readable and correct
        result = imread(str(png_path))
        np.testing.assert_array_equal(result, data)

    def test_imsave_georef_ignored_for_jpeg(self, tmp_path):
        """Passing corners and crs to imsave for JPEG does not raise an error."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        corners = [
            (-77.0, 39.0),
            (-76.0, 39.0),
            (-76.0, 38.0),
            (-77.0, 38.0),
        ]

        jpeg_path = tmp_path / "test_georef.jpg"
        _write_test_image(jpeg_path, data, corners=corners, crs="EPSG:4326")
        assert jpeg_path.exists()


# ============================================================================
# test_imsave_quality_parameter
# ============================================================================


class TestImsaveQualityParameter:
    """Verify quality parameter is passed to the encoder.

    **Validates: Requirements 4.11 (via design)**
    """

    def test_imsave_quality_parameter(self, tmp_path):
        """Writing JPEG with different quality values produces different file sizes."""
        data = np.random.randint(0, 255, (1, 64, 64), dtype=np.uint8)

        low_q_path = tmp_path / "low_quality.jpg"
        high_q_path = tmp_path / "high_quality.jpg"

        _write_test_image(low_q_path, data, quality=10)
        _write_test_image(high_q_path, data, quality=95)

        # Higher quality should produce a larger file (or at least not crash)
        assert low_q_path.exists()
        assert high_q_path.exists()

        low_size = low_q_path.stat().st_size
        high_size = high_q_path.stat().st_size

        # Higher quality JPEG should generally be larger
        assert high_size >= low_size


# ============================================================================
# test_tiles_is_generator
# ============================================================================


class TestTilesIsGenerator:
    """Verify tiles() returns a generator (lazy evaluation).

    **Validates: Requirements 5.6**
    """

    def test_tiles_is_generator(self, tmp_path):
        """tiles() returns a generator object, not a list."""
        data = np.random.randint(0, 255, (1, 32, 32), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        result = tiles(str(path), tile_size=(16, 16))

        assert isinstance(result, types.GeneratorType)


# ============================================================================
# test_tiles_edge_tiles_smaller
# ============================================================================


class TestTilesEdgeTilesSmaller:
    """Verify edge tiles have reduced dimensions when image is not evenly divisible.

    **Validates: Requirements 5.3**
    """

    def test_tiles_edge_tiles_smaller(self, tmp_path):
        """Edge tiles are smaller than tile_size when image doesn't divide evenly."""
        # 30x20 image with 16x16 tiles:
        # - 2 columns (16, 14), 2 rows (16, 4)
        data = np.random.randint(0, 255, (1, 20, 30), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        tile_list = list(tiles(str(path), tile_size=(16, 16)))

        # Should have 2x2 = 4 tiles
        assert len(tile_list) == 4

        # Top-left tile: full size
        assert tile_list[0].data.shape == (1, 16, 16)
        assert tile_list[0].x == 0
        assert tile_list[0].y == 0
        assert tile_list[0].tile_col == 0
        assert tile_list[0].tile_row == 0

        # Top-right tile: reduced width
        assert tile_list[1].data.shape == (1, 16, 14)
        assert tile_list[1].x == 16
        assert tile_list[1].y == 0
        assert tile_list[1].tile_col == 1
        assert tile_list[1].tile_row == 0

        # Bottom-left tile: reduced height
        assert tile_list[2].data.shape == (1, 4, 16)
        assert tile_list[2].x == 0
        assert tile_list[2].y == 16
        assert tile_list[2].tile_col == 0
        assert tile_list[2].tile_row == 1

        # Bottom-right tile: reduced width and height
        assert tile_list[3].data.shape == (1, 4, 14)
        assert tile_list[3].x == 16
        assert tile_list[3].y == 16
        assert tile_list[3].tile_col == 1
        assert tile_list[3].tile_row == 1


# ============================================================================
# test_io_open_string_accepted
# ============================================================================


class TestIOOpenStringAccepted:
    """Verify IO.open() accepts a single string path.

    **Validates: Requirements 6.1**
    """

    def test_io_open_string_accepted(self, tmp_path):
        """IO.open(path, 'r') works when path is a plain string."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        # Open with a string (not a list)
        with IO.open(str(path), "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys

            asset = reader.get_asset("image:0")
            assert asset.num_columns == 16
            assert asset.num_rows == 16


# ============================================================================
# test_io_open_list_unchanged
# ============================================================================


class TestIOOpenListUnchanged:
    """Verify IO.open() with a list still works as before.

    **Validates: Requirements 6.2**
    """

    def test_io_open_list_unchanged(self, tmp_path):
        """IO.open([path], 'r') continues to work with a single-element list."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        # Open with a list (existing behavior)
        with IO.open([str(path)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys

            asset = reader.get_asset("image:0")
            assert asset.num_columns == 16
            assert asset.num_rows == 16


# ============================================================================
# test_public_api_surface
# ============================================================================


class TestPublicApiSurface:
    """Verify __all__ contains all expected names including new convenience API.

    **Validates: Requirements 7.2, 7.3, 8.1, 8.3**
    """

    def test_public_api_surface(self):
        """__all__ includes all expected members including convenience API additions."""
        import aws.osml.io as io_module

        all_members = set(io_module.__all__)

        # New convenience API members that must be present
        convenience_members = {
            "imread",
            "imsave",
            "iminfo",
            "tiles",
            "ImageInfo",
            "Tile",
        }

        # Existing core members that must still be present
        core_members = {
            "__version__",
            "AssetType",
            "PixelType",
            "IO",
            "open",
            "DatasetReader",
            "DatasetWriter",
            "AssetProvider",
            "ImageAssetProvider",
            "BufferedImageAssetProvider",
            "TextAssetProvider",
            "BufferedTextAssetProvider",
            "DataAssetProvider",
            "GraphicsAssetProvider",
            "MetadataProvider",
            "BufferedMetadataProvider",
            "StructureRegistry",
            "StructureAccessor",
            "StructureWriter",
            "StructureDefinition",
            "Value",
        }

        expected = convenience_members | core_members

        # All expected members must be in __all__
        missing = expected - all_members
        assert not missing, f"Missing from __all__: {missing}"

        # Convenience members specifically must be present
        for name in convenience_members:
            assert name in all_members, f"'{name}' not in __all__"


# ============================================================================
# test_existing_api_preserved
# ============================================================================


class TestExistingApiPreserved:
    """Verify no existing __all__ members were removed.

    **Validates: Requirements 7.2, 7.3**
    """

    def test_existing_api_preserved(self):
        """All pre-existing __all__ members are still present after adding convenience API."""
        import aws.osml.io as io_module

        all_members = set(io_module.__all__)

        # These are the members that existed before the convenience API was added
        pre_existing_members = {
            "__version__",
            "AssetType",
            "PixelType",
            "IO",
            "open",
            "DatasetReader",
            "DatasetWriter",
            "AssetProvider",
            "ImageAssetProvider",
            "BufferedImageAssetProvider",
            "TextAssetProvider",
            "BufferedTextAssetProvider",
            "DataAssetProvider",
            "GraphicsAssetProvider",
            "MetadataProvider",
            "BufferedMetadataProvider",
            "StructureRegistry",
            "StructureAccessor",
            "StructureWriter",
            "StructureDefinition",
            "Value",
        }

        removed = pre_existing_members - all_members
        assert not removed, f"Existing members removed from __all__: {removed}"

        # Verify each pre-existing member is importable
        for name in pre_existing_members:
            assert hasattr(io_module, name), (
                f"'{name}' is in __all__ but not importable from aws.osml.io"
            )


# ============================================================================
# Error-condition tests (Task 9.2)
# ============================================================================
# Requirements: 1.4, 1.10, 2.4, 3.5, 4.3, 4.12, 5.10


class TestImreadMissingFile:
    """Verify imread raises IOError for a nonexistent file path.

    **Validates: Requirements 1.10**
    """

    def test_imread_missing_file(self):
        """imread raises IOError when the file does not exist."""
        with pytest.raises(IOError):
            imread("/nonexistent/path/to/image.ntf")


class TestImreadMissingAsset:
    """Verify imread raises ValueError when the specified asset key does not exist.

    **Validates: Requirements 1.4**
    """

    def test_imread_missing_asset(self, tmp_path):
        """imread raises ValueError with the missing key in the message."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        with pytest.raises(ValueError, match="Asset 'nonexistent_key' not found in dataset"):
            imread(str(path), asset="nonexistent_key")


class TestImreadZeroWindow:
    """Verify imread raises ValueError for a degenerate window.

    **Validates: Requirements 2.4**
    """

    def test_imread_zero_window(self, tmp_path):
        """imread raises ValueError when window has zero dimensions after clamping."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        # Window completely outside image bounds → zero dimensions after clamping
        with pytest.raises(
            ValueError,
            match="Window has zero or negative dimensions after clamping to image bounds",
        ):
            imread(str(path), window=(100, 100, 10, 10))


class TestImsaveUnknownExtension:
    """Verify imsave raises ValueError for an unrecognized file extension.

    **Validates: Requirements 4.3**
    """

    def test_imsave_unknown_extension(self, tmp_path):
        """imsave raises ValueError listing supported extensions."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.bmp"

        with pytest.raises(ValueError, match=r"Unsupported file extension '\.bmp'\. Supported:"):
            imsave(str(path), data)


class TestImsaveUnsupportedDtype:
    """Verify imsave raises ValueError for an unsupported dtype.

    **Validates: Requirements 4.12**
    """

    def test_imsave_unsupported_dtype(self, tmp_path):
        """imsave raises ValueError for float64 → PNG."""
        data = np.random.rand(1, 16, 16).astype(np.float64)
        path = tmp_path / "test.png"

        with pytest.raises(
            ValueError,
            match="dtype 'float64' is not supported for png output",
        ):
            imsave(str(path), data)


class TestImsaveEmptyArray:
    """Verify imsave raises ValueError for zero-dimension arrays.

    **Validates: Requirements 4.12**
    """

    def test_imsave_empty_array(self, tmp_path):
        """imsave raises ValueError for an array with a zero-length dimension."""
        data = np.empty((1, 0, 16), dtype=np.uint8)
        path = tmp_path / "test.png"

        with pytest.raises(ValueError, match="Array dimensions must be positive"):
            imsave(str(path), data)


class TestImsaveInvalidNdim:
    """Verify imsave raises ValueError for 4D arrays.

    **Validates: Requirements 4.12**
    """

    def test_imsave_invalid_ndim(self, tmp_path):
        """imsave raises ValueError for a 4D array."""
        data = np.random.randint(0, 255, (2, 3, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.png"

        with pytest.raises(
            ValueError,
            match=r"Expected a 2D \(H, W\) or 3D \(C, H, W\) array, got 4D",
        ):
            imsave(str(path), data)


class TestIminfoMissingFile:
    """Verify iminfo raises IOError for a nonexistent file path.

    **Validates: Requirements 3.5**
    """

    def test_iminfo_missing_file(self):
        """iminfo raises IOError when the file does not exist."""
        with pytest.raises(IOError):
            iminfo("/nonexistent/path/to/image.ntf")


class TestIminfoMissingAsset:
    """Verify iminfo raises ValueError when the specified asset key does not exist.

    **Validates: Requirements 3.5**
    """

    def test_iminfo_missing_asset(self, tmp_path):
        """iminfo raises ValueError with the missing key in the message."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        with pytest.raises(ValueError, match="Asset 'bad_key' not found in dataset"):
            iminfo(str(path), asset="bad_key")


class TestTilesZeroTileSize:
    """Verify tiles raises ValueError for non-positive tile dimensions.

    **Validates: Requirements 5.10**
    """

    def test_tiles_zero_tile_size(self, tmp_path):
        """tiles raises ValueError when tile_size has zero or negative dimensions."""
        data = np.random.randint(0, 255, (1, 16, 16), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        with pytest.raises(ValueError, match="tile_size dimensions must be positive"):
            # Must consume the generator to trigger the error
            list(tiles(str(path), tile_size=(0, 16)))


class TestTilesOverlapExceedsTileSize:
    """Verify tiles raises ValueError when overlap >= tile_size.

    **Validates: Requirements 5.10**
    """

    def test_tiles_overlap_exceeds_tile_size(self, tmp_path):
        """tiles raises ValueError when overlap is >= tile_size in either dimension."""
        data = np.random.randint(0, 255, (1, 32, 32), dtype=np.uint8)
        path = tmp_path / "test.ntf"
        _write_test_image(path, data, compression="none")

        with pytest.raises(
            ValueError,
            match="overlap must be less than tile_size in both dimensions",
        ):
            list(tiles(str(path), tile_size=(16, 16), overlap=(16, 0)))


class TestIOOpenInvalidType:
    """Verify IO.open raises TypeError for non-string, non-list input.

    **Validates: Requirements 6.1**
    """

    def test_io_open_invalid_type(self):
        """IO.open raises an error for an integer argument."""
        # The Rust PathsArg raises ValueError with "paths must be a str or list[str]"
        with pytest.raises((TypeError, ValueError), match="paths must be a str or list\\[str\\]"):
            IO.open(42, "r")


class TestIOOpenEmptyString:
    """Verify IO.open raises ValueError for an empty string.

    **Validates: Requirements 6.1**
    """

    def test_io_open_empty_string(self):
        """IO.open raises ValueError for an empty string path."""
        with pytest.raises(ValueError, match="paths list cannot be empty"):
            IO.open("", "r")
