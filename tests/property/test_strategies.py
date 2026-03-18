"""Property tests for validating hypothesis strategies.

This module contains property tests that verify the strategies themselves
produce valid outputs according to their specifications.
"""

import pytest
from hypothesis import given
from hypothesis import strategies as st

from tests.property.conftest import pbt_settings
from tests.property.strategies import (
    SUPPORTED_PIXEL_TYPES,
    band_counts,
    block_sizes,
    get_numpy_dtype,
    image_arrays,
    image_dimensions,
    metadata_values,
    nitf_field_names,
    pixel_types,
    random_image,
    valid_block_coordinates,
)


@pytest.mark.property
class TestImageStrategyConsistency:
    """For any valid image configuration (pixel type, band count, dimensions),
    the Image_Strategy SHALL produce a NumPy array with shape (bands, rows, cols)
    matching the configuration and dtype matching the pixel type.
    """

    @given(
        pixel_type=pixel_types(),
        dims=image_dimensions(min_size=16, max_size=64),
        num_bands=band_counts(min_bands=1, max_bands=4),
        data=st.data(),
    )
    @pbt_settings
    def test_image_arrays_shape_matches_config(self, pixel_type, dims, num_bands, data):
        """Verify image_arrays produces arrays with correct shape."""
        num_rows, num_cols = dims

        # Generate an array using the strategy via data.draw()
        array = data.draw(image_arrays(pixel_type, num_bands, num_rows, num_cols))

        # Verify shape matches configuration
        assert array.shape == (num_bands, num_rows, num_cols), (
            f"Expected shape ({num_bands}, {num_rows}, {num_cols}), "
            f"got {array.shape}"
        )

    @given(
        pixel_type=pixel_types(),
        dims=image_dimensions(min_size=16, max_size=64),
        num_bands=band_counts(min_bands=1, max_bands=4),
        data=st.data(),
    )
    @pbt_settings
    def test_image_arrays_dtype_matches_pixel_type(self, pixel_type, dims, num_bands, data):
        """Verify image_arrays produces arrays with correct dtype."""
        num_rows, num_cols = dims
        expected_dtype = get_numpy_dtype(pixel_type)

        # Generate an array using the strategy via data.draw()
        array = data.draw(image_arrays(pixel_type, num_bands, num_rows, num_cols))

        # Verify dtype matches pixel type
        assert array.dtype == expected_dtype, (
            f"Expected dtype {expected_dtype}, got {array.dtype}"
        )

    @given(random_image(min_size=16, max_size=64, min_bands=1, max_bands=4))
    @pbt_settings
    def test_random_image_consistency(self, image_data):
        """Verify random_image returns consistent metadata with array."""
        array, pixel_type, num_bands, num_rows, num_cols = image_data

        # Verify shape matches returned metadata
        assert array.shape == (num_bands, num_rows, num_cols), (
            f"Array shape {array.shape} doesn't match metadata "
            f"({num_bands}, {num_rows}, {num_cols})"
        )

        # Verify dtype matches pixel type
        expected_dtype = get_numpy_dtype(pixel_type)
        assert array.dtype == expected_dtype, (
            f"Array dtype {array.dtype} doesn't match pixel type {pixel_type} "
            f"(expected {expected_dtype})"
        )

        # Verify pixel type is one of the supported types
        assert pixel_type in SUPPORTED_PIXEL_TYPES, (
            f"Pixel type {pixel_type} not in supported types"
        )

    @given(pixel_types())
    @pbt_settings
    def test_pixel_types_are_supported(self, pixel_type):
        """Verify pixel_types strategy only produces supported types."""
        assert pixel_type in SUPPORTED_PIXEL_TYPES

    @given(image_dimensions(min_size=16, max_size=256))
    @pbt_settings
    def test_image_dimensions_within_bounds(self, dims):
        """Verify image_dimensions produces values within specified bounds."""
        num_rows, num_cols = dims
        assert 16 <= num_rows <= 256, f"num_rows {num_rows} out of bounds [16, 256]"
        assert 16 <= num_cols <= 256, f"num_cols {num_cols} out of bounds [16, 256]"

    @given(band_counts(min_bands=1, max_bands=8))
    @pbt_settings
    def test_band_counts_within_bounds(self, num_bands):
        """Verify band_counts produces values within specified bounds."""
        assert 1 <= num_bands <= 8, f"num_bands {num_bands} out of bounds [1, 8]"


@pytest.mark.property
class TestBlockStrategyValidity:
    """For any image dimensions and block dimensions, the Block_Strategy SHALL
    produce block coordinates (row, col) that are within the valid range
    [0, num_block_rows) × [0, num_block_cols).
    """

    @given(
        dims=image_dimensions(min_size=32, max_size=128),
        block_size=block_sizes(),
        data=st.data(),
    )
    @pbt_settings
    def test_valid_block_coordinates_within_range(self, dims, block_size, data):
        """Verify valid_block_coordinates produces coordinates within valid range."""
        num_rows, num_cols = dims
        block_height, block_width = block_size

        # Calculate expected number of blocks
        num_block_rows = (num_rows + block_height - 1) // block_height
        num_block_cols = (num_cols + block_width - 1) // block_width

        # Generate coordinates using the strategy via data.draw()
        block_row, block_col = data.draw(valid_block_coordinates(
            num_rows, num_cols, block_height, block_width
        ))

        # Verify coordinates are within valid range
        assert 0 <= block_row < num_block_rows, (
            f"block_row {block_row} out of range [0, {num_block_rows})"
        )
        assert 0 <= block_col < num_block_cols, (
            f"block_col {block_col} out of range [0, {num_block_cols})"
        )

    @given(
        dims=image_dimensions(min_size=32, max_size=128),
        block_size=block_sizes(),
    )
    @pbt_settings
    def test_block_coordinate_calculation_correctness(self, dims, block_size):
        """Verify block count calculation is correct (ceiling division)."""
        num_rows, num_cols = dims
        block_height, block_width = block_size

        # Calculate expected number of blocks using ceiling division
        expected_block_rows = (num_rows + block_height - 1) // block_height
        expected_block_cols = (num_cols + block_width - 1) // block_width

        # Alternative calculation using math.ceil
        import math
        alt_block_rows = math.ceil(num_rows / block_height)
        alt_block_cols = math.ceil(num_cols / block_width)

        # Both calculations should match
        assert expected_block_rows == alt_block_rows
        assert expected_block_cols == alt_block_cols

        # Verify at least one block exists
        assert expected_block_rows >= 1
        assert expected_block_cols >= 1

    @given(block_sizes())
    @pbt_settings
    def test_block_sizes_are_valid(self, block_size):
        """Verify block_sizes produces valid block dimensions."""
        block_height, block_width = block_size

        # Block dimensions should be positive powers of 2
        assert block_height > 0
        assert block_width > 0
        assert block_height == block_width  # Current strategy uses square blocks
        assert block_height in [32, 64, 128, 256]


@pytest.mark.property
class TestMetadataStrategyValidity:
    """For any generated metadata key-value pair, the key SHALL be a valid NITF
    field name (uppercase alphanumeric, 1-10 chars starting with a letter)
    and the value SHALL be a valid printable ASCII string.
    """

    @given(nitf_field_names())
    @pbt_settings
    def test_nitf_field_names_format(self, field_name):
        """Verify nitf_field_names produces valid NITF field names."""
        # Must be 1-10 characters
        assert 1 <= len(field_name) <= 10, (
            f"Field name '{field_name}' length {len(field_name)} not in [1, 10]"
        )

        # Must start with uppercase letter
        assert field_name[0].isupper() and field_name[0].isalpha(), (
            f"Field name '{field_name}' must start with uppercase letter"
        )

        # All characters must be uppercase alphanumeric
        assert field_name.isupper(), (
            f"Field name '{field_name}' must be uppercase"
        )
        assert field_name.isalnum(), (
            f"Field name '{field_name}' must be alphanumeric"
        )

    @given(metadata_values())
    @pbt_settings
    def test_metadata_values_format(self, value):
        """Verify metadata_values produces valid metadata strings."""
        # Must be 1-20 characters
        assert 1 <= len(value) <= 20, (
            f"Metadata value length {len(value)} not in [1, 20]"
        )

        # All characters must be printable ASCII (32-126)
        for char in value:
            assert 32 <= ord(char) <= 126, (
                f"Character '{char}' (ord={ord(char)}) not in printable ASCII range"
            )

    @given(nitf_field_names())
    @pbt_settings
    def test_nitf_field_names_regex_compliance(self, field_name):
        """Verify field names match the NITF regex pattern."""
        import re
        pattern = r"^[A-Z][A-Z0-9]{0,9}$"
        assert re.match(pattern, field_name), (
            f"Field name '{field_name}' doesn't match pattern {pattern}"
        )
