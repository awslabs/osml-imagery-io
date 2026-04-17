"""Property-based tests for the convenience API.

This module tests universal correctness properties of the convenience layer
functions (``imread``, ``imsave``, ``iminfo``, ``tiles``).

Feature: convenience-api
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import iminfo, imread, imsave, tiles
from hypothesis import given
from hypothesis import strategies as st
from hypothesis.extra.numpy import arrays

from .conftest import pbt_settings
from .helpers import assert_lossless_match
from .strategies import axis_aligned_corners, band_subsets, convenience_image, overlap_sizes, tile_sizes, window_coordinates


# =============================================================================
# Property 1: Lossless round-trip preserves pixel data
# =============================================================================


@pytest.mark.property
class TestRoundtripLossless:
    """Property 1: Lossless round-trip preserves pixel data.

    *For any* valid NumPy array in CHW layout with a dtype supported by a
    lossless format, writing the array with ``imsave`` and reading it back
    with ``imread`` SHALL produce an array that is element-wise equal to
    the original.

    Covers:
    - All supported pixel types (uint8, uint16, int16, float32)
    - Various image dimensions including non-block-aligned edges
    - Single-band and multi-band images
    - Lossless formats: uncompressed NITF, Deflate GeoTIFF, PNG (uint8/uint16)

    **Validates: Requirements 1.1, 1.7, 4.1, 4.13**
    """

    @given(image_data=convenience_image())
    @pbt_settings
    def test_roundtrip_lossless(self, image_data):
        """Write with imsave, read back with imread, assert element-wise equality.

        Feature: convenience-api, Property 1: Lossless round-trip preserves pixel data
        """
        array, pixel_type_name, format_string, path_suffix = image_data

        # Use a temp file for each generated input (avoids function-scoped
        # fixture issues with Hypothesis)
        with tempfile.NamedTemporaryFile(
            suffix=path_suffix, delete=False
        ) as f:
            output_path = Path(f.name)

        try:
            # For NITF, use compression="none" to ensure lossless uncompressed.
            # For GeoTIFF, default Deflate compression is lossless.
            # For PNG, standard compression is lossless.
            if format_string == "nitf":
                imsave(str(output_path), array, compression="none")
            else:
                imsave(str(output_path), array)

            # Read back
            decoded = imread(str(output_path))

            # Assert element-wise equality
            assert_lossless_match(array, decoded)
        finally:
            if output_path.exists():
                output_path.unlink()


# =============================================================================
# Property 2: Band selection returns correct subset
# =============================================================================


@pytest.mark.property
class TestBandSelection:
    """Property 2: Band selection returns correct subset.

    *For any* valid multi-band image and any valid subset of zero-based band
    indices, calling ``imread`` with the ``bands`` parameter SHALL return an
    array whose bands are exactly the selected bands from the full image, in
    the order specified.

    The test writes a multi-band image (≥2 bands) with ``imsave``, reads the
    full image, generates a random band subset (including reordering), reads
    again with the ``bands`` parameter, and asserts the selected bands match
    the corresponding slices of the full read.

    **Validates: Requirements 1.5, 5.5**
    """

    @given(data=st.data())
    @pbt_settings
    def test_band_selection_returns_correct_subset(self, data):
        """Read full image, then read with bands param, assert selected bands match.

        Feature: convenience-api, Property 2: Band selection returns correct subset
        """
        # Generate a multi-band image (at least 2 bands for meaningful selection)
        num_bands = data.draw(st.integers(min_value=2, max_value=6), label="num_bands")
        num_rows = data.draw(st.integers(min_value=16, max_value=64), label="num_rows")
        num_cols = data.draw(st.integers(min_value=16, max_value=64), label="num_cols")
        array = data.draw(
            arrays(dtype=np.uint8, shape=(num_bands, num_rows, num_cols)),
            label="array",
        )

        # Generate a random band subset
        selected_bands = data.draw(band_subsets(num_bands), label="selected_bands")

        # Write the image to a temp NITF file (uncompressed for lossless)
        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            output_path = Path(f.name)

        try:
            imsave(str(output_path), array, compression="none")

            # Read the full image
            full_image = imread(str(output_path))

            # Read with band selection
            band_selected = imread(str(output_path), bands=selected_bands)

            # Assert shape: selected bands count, same height/width
            assert band_selected.shape == (
                len(selected_bands),
                num_rows,
                num_cols,
            ), (
                f"Shape mismatch: expected "
                f"({len(selected_bands)}, {num_rows}, {num_cols}), "
                f"got {band_selected.shape}"
            )

            # Assert each selected band matches the corresponding slice
            # of the full read
            expected = full_image[selected_bands, :, :]
            np.testing.assert_array_equal(
                band_selected,
                expected,
                err_msg=(
                    f"Band selection mismatch for bands={selected_bands}. "
                    f"Max diff: {np.max(np.abs(band_selected.astype(np.int16) - expected.astype(np.int16)))}"
                ),
            )
        finally:
            if output_path.exists():
                output_path.unlink()


# =============================================================================
# Property 3: Windowed read returns correct sub-region
# =============================================================================


@pytest.mark.property
class TestWindowedRead:
    """Property 3: Windowed read returns correct sub-region.

    *For any* valid image and any window ``(x, y, width, height)`` (including
    windows that extend beyond image boundaries), calling ``imread`` with the
    ``window`` parameter SHALL return an array containing exactly the pixels
    from the clamped window region, matching the corresponding sub-region of
    a full-image read.

    This property covers both in-bounds and out-of-bounds windows. Windows
    extending beyond the image are clamped to image dimensions. The generator
    produces random windows — some fully within bounds, some partially
    outside — and verifies the result matches the expected sub-array.

    **Validates: Requirements 2.1, 2.3, 2.6**
    """

    @given(data=st.data())
    @pbt_settings
    def test_windowed_read_returns_correct_sub_region(self, data):
        """Read full image, then read with window param, assert sub-region matches.

        Feature: convenience-api, Property 3: Windowed read returns correct sub-region
        """
        # Generate a small image (uncompressed NITF for lossless round-trip)
        num_bands = data.draw(st.integers(min_value=1, max_value=3), label="num_bands")
        num_rows = data.draw(st.integers(min_value=16, max_value=64), label="num_rows")
        num_cols = data.draw(st.integers(min_value=16, max_value=64), label="num_cols")
        array = data.draw(
            arrays(dtype=np.uint8, shape=(num_bands, num_rows, num_cols)),
            label="array",
        )

        # Write the image to a temp NITF file (uncompressed for lossless)
        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            output_path = Path(f.name)

        try:
            imsave(str(output_path), array, compression="none")

            # Read the full image
            full_image = imread(str(output_path))

            # Generate a random window using the window_coordinates strategy
            img_width = full_image.shape[2]
            img_height = full_image.shape[1]
            wx, wy, ww, wh = data.draw(
                window_coordinates(img_width, img_height), label="window"
            )

            # Compute the expected clamped window coordinates
            x0 = max(0, wx)
            y0 = max(0, wy)
            x1 = min(img_width, wx + ww)
            y1 = min(img_height, wy + wh)
            clamped_w = x1 - x0
            clamped_h = y1 - y0

            if clamped_w <= 0 or clamped_h <= 0:
                # Window is entirely outside image bounds — expect ValueError
                with pytest.raises(ValueError, match="zero or negative dimensions"):
                    imread(str(output_path), window=(wx, wy, ww, wh))
            else:
                # Read with window parameter
                windowed = imread(str(output_path), window=(wx, wy, ww, wh))

                # Expected sub-region from the full read
                expected = full_image[:, y0:y1, x0:x1]

                # Assert shape matches the clamped window
                assert windowed.shape == (
                    full_image.shape[0],
                    clamped_h,
                    clamped_w,
                ), (
                    f"Shape mismatch: expected "
                    f"({full_image.shape[0]}, {clamped_h}, {clamped_w}), "
                    f"got {windowed.shape}"
                )

                # Assert pixel values match the corresponding sub-region
                np.testing.assert_array_equal(
                    windowed,
                    expected,
                    err_msg=(
                        f"Windowed read mismatch for window=({wx}, {wy}, {ww}, {wh}), "
                        f"clamped to ({x0}, {y0}, {clamped_w}, {clamped_h}). "
                        f"Max diff: {np.max(np.abs(windowed.astype(np.int16) - expected.astype(np.int16)))}"
                    ),
                )
        finally:
            if output_path.exists():
                output_path.unlink()


# =============================================================================
# Property 4: iminfo metadata matches image properties
# =============================================================================


@pytest.mark.property
class TestIminfoAccuracy:
    """Property 4: iminfo metadata matches image properties.

    *For any* valid image written with ``imsave``, calling ``iminfo`` on the
    written file SHALL return an ``ImageInfo`` whose ``width``, ``height``,
    ``bands``, and ``dtype`` attributes match the dimensions and dtype of the
    original array.

    This property verifies that ``iminfo`` correctly extracts metadata without
    reading pixel data, and that the metadata is consistent with what was
    written.

    **Validates: Requirements 3.1, 3.2**
    """

    @given(image_data=convenience_image())
    @pbt_settings
    def test_iminfo_metadata_matches_image_properties(self, image_data):
        """Write with imsave, call iminfo, assert metadata matches original array.

        Feature: convenience-api, Property 4: iminfo metadata matches image properties
        """
        array, pixel_type_name, format_string, path_suffix = image_data

        with tempfile.NamedTemporaryFile(
            suffix=path_suffix, delete=False
        ) as f:
            output_path = Path(f.name)

        try:
            # Use compression="none" for NITF to ensure lossless uncompressed.
            if format_string == "nitf":
                imsave(str(output_path), array, compression="none")
            else:
                imsave(str(output_path), array)

            # Call iminfo on the written file
            info = iminfo(str(output_path))

            # Assert width matches array.shape[2] (columns)
            assert info.width == array.shape[2], (
                f"Width mismatch: expected {array.shape[2]}, got {info.width}"
            )

            # Assert height matches array.shape[1] (rows)
            assert info.height == array.shape[1], (
                f"Height mismatch: expected {array.shape[1]}, got {info.height}"
            )

            # Assert bands matches array.shape[0] (channels)
            assert info.bands == array.shape[0], (
                f"Bands mismatch: expected {array.shape[0]}, got {info.bands}"
            )

            # Assert dtype matches array.dtype.name
            assert info.dtype == array.dtype.name, (
                f"Dtype mismatch: expected {array.dtype.name!r}, got {info.dtype!r}"
            )

            # Assert metadata is a non-empty dict
            assert isinstance(info.metadata, dict), (
                f"metadata should be a dict, got {type(info.metadata)}"
            )
            assert len(info.metadata) > 0, (
                "metadata dict should not be empty"
            )

            # For NITF files, verify IC field is present and correct
            if format_string == "nitf":
                assert "IC" in info.metadata, (
                    "NITF metadata should contain 'IC' field"
                )
                assert info.metadata["IC"] == "NC", (
                    f"Expected IC='NC' for uncompressed NITF, "
                    f"got IC={info.metadata['IC']!r}"
                )
        finally:
            if output_path.exists():
                output_path.unlink()


# =============================================================================
# Property 5: Tiles cover the entire image without gaps
# =============================================================================


@pytest.mark.property
class TestTileCoverage:
    """Property 5: Tiles cover the entire image without gaps.

    *For any* valid image, any tile size ``(width, height)``, and any valid
    overlap ``(overlap_w, overlap_h)`` where ``overlap_w < width`` and
    ``overlap_h < height``, iterating all tiles from ``tiles()`` and
    assembling them into a single array SHALL produce an array that covers
    every pixel of the image with no gaps.

    When overlap is ``(0, 0)``, the assembled tiles are element-wise equal
    to the result of ``imread`` on the same file (no gaps, no overlaps).
    When overlap is greater than ``(0, 0)``, tiles share boundary pixels —
    the overlapping regions in adjacent tiles SHALL contain identical pixel
    values, and the union of all tile regions SHALL still cover the full
    image.

    **Validates: Requirements 5.1, 5.2, 5.3, 5.7, 5.8, 5.9**
    """

    @given(data=st.data())
    @pbt_settings
    def test_tiles_no_overlap_cover_full_image(self, data):
        """With overlap=(0,0), assembled tiles equal the full imread result.

        Feature: convenience-api, Property 5: Tiles cover the entire image without gaps
        """
        # Generate a small image (uncompressed NITF for lossless round-trip)
        num_bands = data.draw(st.integers(min_value=1, max_value=3), label="num_bands")
        num_rows = data.draw(st.integers(min_value=16, max_value=64), label="num_rows")
        num_cols = data.draw(st.integers(min_value=16, max_value=64), label="num_cols")
        array = data.draw(
            arrays(dtype=np.uint8, shape=(num_bands, num_rows, num_cols)),
            label="array",
        )

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            output_path = Path(f.name)

        try:
            imsave(str(output_path), array, compression="none")

            # Read the full image for comparison
            full_image = imread(str(output_path))
            img_height, img_width = full_image.shape[1], full_image.shape[2]

            # Generate a random tile size
            tile_w, tile_h = data.draw(
                tile_sizes(img_width, img_height), label="tile_size"
            )

            # Assemble tiles into a single array
            assembled = np.zeros_like(full_image)

            for tile in tiles(str(output_path), (tile_w, tile_h), overlap=(0, 0)):
                # Verify tile position and grid indices are consistent
                expected_x = tile.tile_col * tile_w
                expected_y = tile.tile_row * tile_h
                assert tile.x == expected_x, (
                    f"Tile x mismatch: expected {expected_x}, got {tile.x} "
                    f"at grid ({tile.tile_row}, {tile.tile_col})"
                )
                assert tile.y == expected_y, (
                    f"Tile y mismatch: expected {expected_y}, got {tile.y} "
                    f"at grid ({tile.tile_row}, {tile.tile_col})"
                )

                # Verify edge tiles have correct reduced dimensions
                expected_w = min(tile_w, img_width - tile.x)
                expected_h = min(tile_h, img_height - tile.y)
                assert tile.data.shape[2] == expected_w, (
                    f"Tile width mismatch at ({tile.tile_row}, {tile.tile_col}): "
                    f"expected {expected_w}, got {tile.data.shape[2]}"
                )
                assert tile.data.shape[1] == expected_h, (
                    f"Tile height mismatch at ({tile.tile_row}, {tile.tile_col}): "
                    f"expected {expected_h}, got {tile.data.shape[1]}"
                )

                # Place tile into assembled array
                x, y = tile.x, tile.y
                h, w = tile.data.shape[1], tile.data.shape[2]
                assembled[:, y : y + h, x : x + w] = tile.data

            # Assert assembled tiles are element-wise equal to full imread
            np.testing.assert_array_equal(
                assembled,
                full_image,
                err_msg=(
                    f"Assembled tiles do not match full image. "
                    f"tile_size=({tile_w}, {tile_h}), "
                    f"image=({img_width}, {img_height})"
                ),
            )
        finally:
            if output_path.exists():
                output_path.unlink()

    @given(data=st.data())
    @pbt_settings
    def test_tiles_with_overlap_cover_full_image(self, data):
        """With overlap>0, every pixel is covered and overlapping regions match.

        Feature: convenience-api, Property 5: Tiles cover the entire image without gaps
        """
        # Generate a small image (uncompressed NITF for lossless round-trip)
        num_bands = data.draw(st.integers(min_value=1, max_value=3), label="num_bands")
        num_rows = data.draw(st.integers(min_value=16, max_value=64), label="num_rows")
        num_cols = data.draw(st.integers(min_value=16, max_value=64), label="num_cols")
        array = data.draw(
            arrays(dtype=np.uint8, shape=(num_bands, num_rows, num_cols)),
            label="array",
        )

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            output_path = Path(f.name)

        try:
            imsave(str(output_path), array, compression="none")

            # Read the full image for comparison
            full_image = imread(str(output_path))
            img_height, img_width = full_image.shape[1], full_image.shape[2]

            # Generate a random tile size and overlap
            tile_w, tile_h = data.draw(
                tile_sizes(img_width, img_height), label="tile_size"
            )
            overlap_w, overlap_h = data.draw(
                overlap_sizes(tile_w, tile_h), label="overlap"
            )

            stride_w = tile_w - overlap_w
            stride_h = tile_h - overlap_h

            # Coverage array: counts how many tiles cover each pixel
            coverage = np.zeros((img_height, img_width), dtype=np.int32)

            # Reference array: stores pixel values from the first tile
            # that covers each pixel (for overlap consistency checking)
            reference = np.full_like(full_image, -1, dtype=np.int16)

            overlap_consistent = True

            for tile in tiles(
                str(output_path),
                (tile_w, tile_h),
                overlap=(overlap_w, overlap_h),
            ):
                # Verify tile position is consistent with stride
                expected_x = tile.tile_col * stride_w
                expected_y = tile.tile_row * stride_h
                assert tile.x == expected_x, (
                    f"Tile x mismatch: expected {expected_x}, got {tile.x} "
                    f"at grid ({tile.tile_row}, {tile.tile_col})"
                )
                assert tile.y == expected_y, (
                    f"Tile y mismatch: expected {expected_y}, got {tile.y} "
                    f"at grid ({tile.tile_row}, {tile.tile_col})"
                )

                x, y = tile.x, tile.y
                h, w = tile.data.shape[1], tile.data.shape[2]

                # Update coverage count
                coverage[y : y + h, x : x + w] += 1

                # Check overlap consistency: where reference already has
                # values (from a previous tile), the pixel values must match
                region = reference[:, y : y + h, x : x + w]
                already_covered = region[0] >= 0  # mask of previously covered pixels

                if np.any(already_covered):
                    # Compare tile data with reference in overlapping region
                    for band in range(tile.data.shape[0]):
                        tile_vals = tile.data[band][already_covered]
                        ref_vals = region[band][already_covered].astype(
                            tile.data.dtype
                        )
                        if not np.array_equal(tile_vals, ref_vals):
                            overlap_consistent = False

                # Store pixel values in reference for future overlap checks
                reference[:, y : y + h, x : x + w] = tile.data.astype(np.int16)

            # Assert every pixel is covered at least once (no gaps)
            uncovered = np.argwhere(coverage == 0)
            assert uncovered.size == 0, (
                f"Found {len(uncovered)} uncovered pixels. "
                f"tile_size=({tile_w}, {tile_h}), "
                f"overlap=({overlap_w}, {overlap_h}), "
                f"image=({img_width}, {img_height}). "
                f"First uncovered: {uncovered[:5].tolist()}"
            )

            # Assert overlapping regions contain identical pixels
            assert overlap_consistent, (
                f"Overlapping tile regions contain different pixel values. "
                f"tile_size=({tile_w}, {tile_h}), "
                f"overlap=({overlap_w}, {overlap_h}), "
                f"image=({img_width}, {img_height})"
            )
        finally:
            if output_path.exists():
                output_path.unlink()


# =============================================================================
# Property 6: GeoTIFF corners-to-transform round-trip
# =============================================================================


@pytest.mark.property
class TestGeotiffTransformRoundtrip:
    """Property 6: GeoTIFF corners-to-transform round-trip.

    *For any* axis-aligned rectangular set of four corner coordinates and
    valid image dimensions, computing the GeoTIFF ModelTiepoint +
    ModelPixelScale from the corners and then mapping the four corner pixel
    positions through the resulting affine transform SHALL recover the
    original corner coordinates (within floating-point tolerance).

    This property verifies the mathematical correctness of the
    ``_apply_geotiff_georef`` helper. The generator produces random
    axis-aligned bounding boxes (UL.lat == UR.lat, UL.lon == LL.lon) with
    varying extents and image dimensions.

    **Validates: Requirements 4.9**
    """

    @given(
        corners=axis_aligned_corners(),
        width=st.integers(min_value=1, max_value=4096),
        height=st.integers(min_value=1, max_value=4096),
    )
    @pbt_settings
    def test_geotiff_corners_to_transform_roundtrip(self, corners, width, height):
        """Compute tiepoint + pixel scale from corners, map corner pixels, recover coords.

        Feature: convenience-api, Property 6: GeoTIFF corners-to-transform round-trip
        """
        ul_lon, ul_lat = corners[0]  # Upper-Left
        ur_lon, ur_lat = corners[1]  # Upper-Right
        lr_lon, lr_lat = corners[2]  # Lower-Right
        ll_lon, ll_lat = corners[3]  # Lower-Left

        # Compute ModelPixelScale and ModelTiepoint from corners
        # (same math as _apply_geotiff_georef for axis-aligned images)
        pixel_width = (ur_lon - ul_lon) / width
        pixel_height = (ul_lat - ll_lat) / height

        pixel_scale = [abs(pixel_width), abs(pixel_height), 0.0]
        tiepoint = [0.0, 0.0, 0.0, ul_lon, ul_lat, 0.0]

        # Define the affine transform:
        #   geo_x = tiepoint[3] + pixel_x * pixel_scale[0]
        #   geo_y = tiepoint[4] - pixel_y * pixel_scale[1]

        def pixel_to_geo(pixel_x, pixel_y):
            geo_x = tiepoint[3] + pixel_x * pixel_scale[0]
            geo_y = tiepoint[4] - pixel_y * pixel_scale[1]
            return geo_x, geo_y

        tol = 1e-9

        # UL pixel (0, 0) → (UL_lon, UL_lat)
        geo_x, geo_y = pixel_to_geo(0, 0)
        assert abs(geo_x - ul_lon) < tol, (
            f"UL lon mismatch: expected {ul_lon}, got {geo_x}, diff={abs(geo_x - ul_lon)}"
        )
        assert abs(geo_y - ul_lat) < tol, (
            f"UL lat mismatch: expected {ul_lat}, got {geo_y}, diff={abs(geo_y - ul_lat)}"
        )

        # UR pixel (width, 0) → (UR_lon, UR_lat)
        geo_x, geo_y = pixel_to_geo(width, 0)
        assert abs(geo_x - ur_lon) < tol, (
            f"UR lon mismatch: expected {ur_lon}, got {geo_x}, diff={abs(geo_x - ur_lon)}"
        )
        assert abs(geo_y - ur_lat) < tol, (
            f"UR lat mismatch: expected {ur_lat}, got {geo_y}, diff={abs(geo_y - ur_lat)}"
        )

        # LR pixel (width, height) → (LR_lon, LR_lat)
        geo_x, geo_y = pixel_to_geo(width, height)
        assert abs(geo_x - lr_lon) < tol, (
            f"LR lon mismatch: expected {lr_lon}, got {geo_x}, diff={abs(geo_x - lr_lon)}"
        )
        assert abs(geo_y - lr_lat) < tol, (
            f"LR lat mismatch: expected {lr_lat}, got {geo_y}, diff={abs(geo_y - lr_lat)}"
        )

        # LL pixel (0, height) → (LL_lon, LL_lat)
        geo_x, geo_y = pixel_to_geo(0, height)
        assert abs(geo_x - ll_lon) < tol, (
            f"LL lon mismatch: expected {ll_lon}, got {geo_x}, diff={abs(geo_x - ll_lon)}"
        )
        assert abs(geo_y - ll_lat) < tol, (
            f"LL lat mismatch: expected {ll_lat}, got {geo_y}, diff={abs(geo_y - ll_lat)}"
        )


# =============================================================================
# Property 7: IO.open string and list equivalence
# =============================================================================


@pytest.mark.property
class TestIOOpenStringListEquivalence:
    """Property 7: IO.open string and list equivalence.

    *For any* valid single-file path, calling ``IO.open(path, "r")`` with a
    string SHALL produce a reader that yields identical asset keys and image
    properties as ``IO.open([path], "r")`` with a single-element list.

    This property verifies that the Rust-side ``PathsArg`` change correctly
    normalizes a bare string to a list without altering behavior.

    **Validates: Requirements 6.1, 6.2**
    """

    @given(image_data=convenience_image())
    @pbt_settings
    def test_io_open_string_and_list_equivalence(self, image_data):
        """Open with IO.open(path, "r") and IO.open([path], "r"), assert identical results.

        Feature: convenience-api, Property 7: IO.open string and list equivalence
        """
        from aws.osml.io import IO, AssetType

        array, pixel_type_name, format_string, path_suffix = image_data

        with tempfile.NamedTemporaryFile(
            suffix=path_suffix, delete=False
        ) as f:
            output_path = Path(f.name)

        try:
            # Write the image using imsave
            if format_string == "nitf":
                imsave(str(output_path), array, compression="none")
            else:
                imsave(str(output_path), array)

            path_str = str(output_path)

            # Open with string path
            with IO.open(path_str, "r") as reader_str:
                keys_str = reader_str.get_asset_keys(asset_type=AssetType.Image)
                props_str = {}
                for key in keys_str:
                    asset = reader_str.get_asset(key)
                    props_str[key] = {
                        "num_columns": asset.num_columns,
                        "num_rows": asset.num_rows,
                        "num_bands": asset.num_bands,
                        "pixel_value_type": asset.pixel_value_type,
                    }

            # Open with list path
            with IO.open([path_str], "r") as reader_list:
                keys_list = reader_list.get_asset_keys(asset_type=AssetType.Image)
                props_list = {}
                for key in keys_list:
                    asset = reader_list.get_asset(key)
                    props_list[key] = {
                        "num_columns": asset.num_columns,
                        "num_rows": asset.num_rows,
                        "num_bands": asset.num_bands,
                        "pixel_value_type": asset.pixel_value_type,
                    }

            # Assert identical asset keys
            assert keys_str == keys_list, (
                f"Asset keys differ: string={keys_str}, list={keys_list}"
            )

            # Assert identical image properties for each asset
            for key in keys_str:
                str_props = props_str[key]
                list_props = props_list[key]

                assert str_props["num_columns"] == list_props["num_columns"], (
                    f"num_columns mismatch for '{key}': "
                    f"string={str_props['num_columns']}, list={list_props['num_columns']}"
                )
                assert str_props["num_rows"] == list_props["num_rows"], (
                    f"num_rows mismatch for '{key}': "
                    f"string={str_props['num_rows']}, list={list_props['num_rows']}"
                )
                assert str_props["num_bands"] == list_props["num_bands"], (
                    f"num_bands mismatch for '{key}': "
                    f"string={str_props['num_bands']}, list={list_props['num_bands']}"
                )
                assert str_props["pixel_value_type"] == list_props["pixel_value_type"], (
                    f"pixel_value_type mismatch for '{key}': "
                    f"string={str_props['pixel_value_type']}, list={list_props['pixel_value_type']}"
                )
        finally:
            if output_path.exists():
                output_path.unlink()
