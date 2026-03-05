"""Hypothesis strategies for property-based testing of image codecs.

This module provides reusable strategies for generating:
- Pixel types (UInt8, UInt16, Int16, Float32)
- Image dimensions (width, height)
- Band counts (1-8)
- Block sizes (32x32 to 256x256)
- Image arrays in BSQ format (bands, rows, cols)
- Edge case images (single-pixel, gradients, max values, etc.)
- Valid block coordinates
- NITF metadata key-value pairs

Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 5.2
"""

from typing import Tuple

import numpy as np
from hypothesis import strategies as st
from hypothesis.extra.numpy import arrays

from aws.osml.io import PixelType


# Supported pixel types for property testing
# These are the types most commonly used in NITF imagery
SUPPORTED_PIXEL_TYPES = [
    PixelType.UInt8,
    PixelType.UInt16,
    PixelType.Int16,
    PixelType.Float32,
]


def get_numpy_dtype(pixel_type: PixelType) -> np.dtype:
    """Get the numpy dtype for a PixelType.
    
    Uses the PixelType.to_numpy_dtype() method provided by the library.
    
    Args:
        pixel_type: The PixelType enum value
        
    Returns:
        The corresponding numpy dtype
    """
    return np.dtype(pixel_type.to_numpy_dtype())


def pixel_types() -> st.SearchStrategy[PixelType]:
    """Strategy for supported pixel types.
    
    Returns a strategy that samples from UInt8, UInt16, Int16, and Float32.
    
    Requirements: 1.2
    """
    return st.sampled_from(SUPPORTED_PIXEL_TYPES)


def image_dimensions(
    min_size: int = 16,
    max_size: int = 256
) -> st.SearchStrategy[Tuple[int, int]]:
    """Strategy for image dimensions (rows, cols).
    
    Args:
        min_size: Minimum dimension size (default 16)
        max_size: Maximum dimension size (default 256)
    
    Returns:
        Strategy producing (num_rows, num_cols) tuples
    
    Requirements: 1.4
    """
    return st.tuples(
        st.integers(min_value=min_size, max_value=max_size),
        st.integers(min_value=min_size, max_value=max_size)
    )


def band_counts(
    min_bands: int = 1,
    max_bands: int = 8
) -> st.SearchStrategy[int]:
    """Strategy for number of bands.
    
    Args:
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 8)
    
    Returns:
        Strategy producing integer band counts
    
    Requirements: 1.3
    """
    return st.integers(min_value=min_bands, max_value=max_bands)


def block_sizes() -> st.SearchStrategy[Tuple[int, int]]:
    """Strategy for block dimensions.
    
    Returns a strategy that samples from common block sizes:
    (32, 32), (64, 64), (128, 128), (256, 256)
    
    Returns:
        Strategy producing (block_height, block_width) tuples
    
    Requirements: 1.4
    """
    return st.sampled_from([
        (32, 32),
        (64, 64),
        (128, 128),
        (256, 256),
    ])



def image_arrays(
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
) -> st.SearchStrategy[np.ndarray]:
    """Strategy for generating image data arrays in BSQ format (bands, rows, cols).
    
    Args:
        pixel_type: The pixel type determining the numpy dtype
        num_bands: Number of bands in the image
        num_rows: Number of rows (height) in the image
        num_cols: Number of columns (width) in the image
    
    Returns:
        Strategy producing numpy arrays with shape (num_bands, num_rows, num_cols)
    
    Requirements: 1.1
    """
    dtype = get_numpy_dtype(pixel_type)
    return arrays(dtype=dtype, shape=(num_bands, num_rows, num_cols))


@st.composite
def random_image(
    draw,
    min_size: int = 16,
    max_size: int = 256,
    min_bands: int = 1,
    max_bands: int = 8,
) -> Tuple[np.ndarray, PixelType, int, int, int]:
    """Composite strategy for random images with metadata.
    
    Generates a random image with random pixel type, dimensions, and band count.
    
    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum dimension size (default 16)
        max_size: Maximum dimension size (default 256)
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 8)
    
    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols)
        - array: numpy array with shape (num_bands, num_rows, num_cols)
        - pixel_type: PixelType enum value
        - num_bands: number of bands
        - num_rows: number of rows
        - num_cols: number of columns
    
    Requirements: 1.1
    """
    pixel_type = draw(pixel_types())
    num_rows, num_cols = draw(image_dimensions(min_size=min_size, max_size=max_size))
    num_bands = draw(band_counts(min_bands=min_bands, max_bands=max_bands))
    
    array = draw(image_arrays(pixel_type, num_bands, num_rows, num_cols))
    
    return (array, pixel_type, num_bands, num_rows, num_cols)



@st.composite
def edge_case_images(
    draw,
    pixel_type: PixelType = None,
) -> Tuple[np.ndarray, PixelType, str]:
    """Strategy for edge case images: single-pixel, gradients, max values, etc.
    
    Generates images that test boundary conditions and special cases.
    
    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        pixel_type: Optional fixed pixel type. If None, randomly selected.
    
    Returns:
        Tuple of (array, pixel_type, edge_case_name)
        - array: numpy array with shape (num_bands, num_rows, num_cols)
        - pixel_type: PixelType enum value
        - edge_case_name: string describing the edge case type
    
    Requirements: 1.5
    """
    if pixel_type is None:
        pixel_type = draw(pixel_types())
    
    dtype = get_numpy_dtype(pixel_type)
    
    # Choose an edge case type
    edge_case_type = draw(st.sampled_from([
        "single_pixel",
        "single_band",
        "max_value",
        "min_value",
        "gradient_horizontal",
        "gradient_vertical",
        "random_noise",
    ]))
    
    if edge_case_type == "single_pixel":
        # Single pixel image (1x1)
        num_bands = draw(band_counts(min_bands=1, max_bands=3))
        array = draw(arrays(dtype=dtype, shape=(num_bands, 1, 1)))
        return (array, pixel_type, edge_case_type)
    
    elif edge_case_type == "single_band":
        # Single band image with random dimensions
        num_rows, num_cols = draw(image_dimensions(min_size=16, max_size=64))
        array = draw(arrays(dtype=dtype, shape=(1, num_rows, num_cols)))
        return (array, pixel_type, edge_case_type)
    
    elif edge_case_type == "max_value":
        # Image filled with maximum values for the dtype
        num_bands = draw(band_counts(min_bands=1, max_bands=3))
        num_rows, num_cols = draw(image_dimensions(min_size=16, max_size=64))
        
        if np.issubdtype(dtype, np.integer):
            max_val = np.iinfo(dtype).max
        else:
            max_val = 1.0  # For float types, use 1.0 as max
        
        array = np.full((num_bands, num_rows, num_cols), max_val, dtype=dtype)
        return (array, pixel_type, edge_case_type)
    
    elif edge_case_type == "min_value":
        # Image filled with minimum values for the dtype
        num_bands = draw(band_counts(min_bands=1, max_bands=3))
        num_rows, num_cols = draw(image_dimensions(min_size=16, max_size=64))
        
        if np.issubdtype(dtype, np.integer):
            min_val = np.iinfo(dtype).min
        else:
            min_val = 0.0  # For float types, use 0.0 as min
        
        array = np.full((num_bands, num_rows, num_cols), min_val, dtype=dtype)
        return (array, pixel_type, edge_case_type)
    
    elif edge_case_type == "gradient_horizontal":
        # Horizontal gradient (values increase left to right)
        num_bands = draw(band_counts(min_bands=1, max_bands=3))
        num_rows, num_cols = draw(image_dimensions(min_size=16, max_size=64))
        
        if np.issubdtype(dtype, np.integer):
            max_val = np.iinfo(dtype).max
        else:
            max_val = 1.0
        
        # Create gradient for one row, then tile
        gradient = np.linspace(0, max_val, num_cols, dtype=dtype)
        single_band = np.tile(gradient, (num_rows, 1))
        array = np.stack([single_band] * num_bands, axis=0)
        return (array, pixel_type, edge_case_type)
    
    elif edge_case_type == "gradient_vertical":
        # Vertical gradient (values increase top to bottom)
        num_bands = draw(band_counts(min_bands=1, max_bands=3))
        num_rows, num_cols = draw(image_dimensions(min_size=16, max_size=64))
        
        if np.issubdtype(dtype, np.integer):
            max_val = np.iinfo(dtype).max
        else:
            max_val = 1.0
        
        # Create gradient for one column, then tile
        gradient = np.linspace(0, max_val, num_rows, dtype=dtype)
        single_band = np.tile(gradient.reshape(-1, 1), (1, num_cols))
        array = np.stack([single_band] * num_bands, axis=0)
        return (array, pixel_type, edge_case_type)
    
    else:  # random_noise
        # Random noise image
        num_bands = draw(band_counts(min_bands=1, max_bands=3))
        num_rows, num_cols = draw(image_dimensions(min_size=16, max_size=64))
        array = draw(arrays(dtype=dtype, shape=(num_bands, num_rows, num_cols)))
        return (array, pixel_type, edge_case_type)



def valid_block_coordinates(
    num_rows: int,
    num_cols: int,
    block_height: int,
    block_width: int,
) -> st.SearchStrategy[Tuple[int, int]]:
    """Strategy for valid block (row, col) coordinates.
    
    Calculates the number of block rows and columns based on image and block
    dimensions, then returns a strategy that generates valid coordinate pairs.
    
    Args:
        num_rows: Number of rows in the image
        num_cols: Number of columns in the image
        block_height: Height of each block
        block_width: Width of each block
    
    Returns:
        Strategy producing (block_row, block_col) tuples within valid range
        [0, num_block_rows) × [0, num_block_cols)
    
    Requirements: 1.6
    """
    # Calculate number of blocks (ceiling division)
    num_block_rows = (num_rows + block_height - 1) // block_height
    num_block_cols = (num_cols + block_width - 1) // block_width
    
    return st.tuples(
        st.integers(min_value=0, max_value=max(0, num_block_rows - 1)),
        st.integers(min_value=0, max_value=max(0, num_block_cols - 1))
    )


def invalid_block_coordinates(
    num_rows: int,
    num_cols: int,
    block_height: int,
    block_width: int,
) -> st.SearchStrategy[Tuple[int, int]]:
    """Strategy for invalid block coordinates (outside valid range).
    
    Generates block coordinates that are outside the valid range, useful for
    testing error handling.
    
    Args:
        num_rows: Number of rows in the image
        num_cols: Number of columns in the image
        block_height: Height of each block
        block_width: Width of each block
    
    Returns:
        Strategy producing (block_row, block_col) tuples outside valid range
    
    Requirements: 4.4
    """
    num_block_rows = (num_rows + block_height - 1) // block_height
    num_block_cols = (num_cols + block_width - 1) // block_width
    
    # Generate coordinates that are either:
    # - negative row or col
    # - row >= num_block_rows
    # - col >= num_block_cols
    return st.one_of(
        # Negative row
        st.tuples(
            st.integers(min_value=-100, max_value=-1),
            st.integers(min_value=0, max_value=num_block_cols + 10)
        ),
        # Negative col
        st.tuples(
            st.integers(min_value=0, max_value=num_block_rows + 10),
            st.integers(min_value=-100, max_value=-1)
        ),
        # Row too large
        st.tuples(
            st.integers(min_value=num_block_rows, max_value=num_block_rows + 100),
            st.integers(min_value=0, max_value=max(0, num_block_cols - 1))
        ),
        # Col too large
        st.tuples(
            st.integers(min_value=0, max_value=max(0, num_block_rows - 1)),
            st.integers(min_value=num_block_cols, max_value=num_block_cols + 100)
        ),
    )



def nitf_field_names() -> st.SearchStrategy[str]:
    """Strategy for valid NITF field names.
    
    NITF field names are uppercase alphanumeric strings, 1-10 characters,
    starting with a letter.
    
    Returns:
        Strategy producing valid NITF field name strings
    
    Requirements: 5.2
    """
    return st.from_regex(r"[A-Z][A-Z0-9]{0,9}", fullmatch=True)


def metadata_values() -> st.SearchStrategy[str]:
    """Strategy for valid metadata values.
    
    Generates printable ASCII strings suitable for NITF metadata values.
    NITF uses BCS-A (Basic Character Set - Alphanumeric) which is ASCII.
    
    Returns:
        Strategy producing valid metadata value strings (1-20 chars)
    
    Requirements: 5.2
    """
    # Use printable ASCII characters only (codes 32-126)
    # This matches NITF BCS-A character set
    printable_ascii = "".join(chr(c) for c in range(32, 127))
    return st.text(
        alphabet=printable_ascii,
        min_size=1,
        max_size=20
    )


@st.composite
def metadata_pairs(
    draw,
    min_pairs: int = 1,
    max_pairs: int = 5,
) -> dict:
    """Strategy for generating metadata key-value dictionaries.
    
    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_pairs: Minimum number of key-value pairs
        max_pairs: Maximum number of key-value pairs
    
    Returns:
        Dictionary of NITF field names to metadata values
    
    Requirements: 5.2
    """
    num_pairs = draw(st.integers(min_value=min_pairs, max_value=max_pairs))
    
    # Generate unique keys
    keys = draw(st.lists(
        nitf_field_names(),
        min_size=num_pairs,
        max_size=num_pairs,
        unique=True
    ))
    
    # Generate values for each key
    result = {}
    for key in keys:
        result[key] = draw(metadata_values())
    
    return result


# Pixel types suitable for lossy J2K compression (excludes Float32)
J2K_PIXEL_TYPES = [
    PixelType.UInt8,
    PixelType.UInt16,
    PixelType.Int16,
]


@st.composite
def realistic_image_for_compression(
    draw,
    min_size: int = 32,
    max_size: int = 64,
    min_bands: int = 1,
    max_bands: int = 3,
) -> Tuple[np.ndarray, PixelType, int, int, int]:
    """Composite strategy for images suitable for lossy compression testing.
    
    Generates images with realistic value distributions that work well with
    lossy compression quality metrics. This strategy creates gradient-like
    images that have guaranteed variance across the image, making them
    suitable for meaningful PSNR and SSIM calculations.
    
    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum dimension size (default 32)
        max_size: Maximum dimension size (default 64)
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 3)
    
    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols)
    """
    # Only use pixel types that work with J2K (no Float32)
    pixel_type = draw(st.sampled_from(J2K_PIXEL_TYPES))
    num_rows, num_cols = draw(image_dimensions(min_size=min_size, max_size=max_size))
    num_bands = draw(band_counts(min_bands=min_bands, max_bands=max_bands))
    
    dtype = get_numpy_dtype(pixel_type)
    
    # Get dtype range
    dtype_info = np.iinfo(dtype)
    dtype_min = int(dtype_info.min)
    dtype_max = int(dtype_info.max)
    
    # For signed types (Int16), use positive range only for simpler quality metrics
    if dtype_min < 0:
        effective_min = 0
        effective_max = dtype_max
    else:
        effective_min = dtype_min
        effective_max = dtype_max
    
    # Generate a gradient-based image with noise overlay
    # This ensures we have actual variance in the image
    
    # Pick a value range that spans at least 1000 values for meaningful compression
    min_range = 1000
    max_range = (effective_max - effective_min) // 2
    if max_range < min_range:
        max_range = min_range
    
    value_range = draw(st.integers(min_value=min_range, max_value=max_range))
    
    # Pick a base value that allows the full range
    max_base = effective_max - value_range
    base_value = draw(st.integers(min_value=effective_min, max_value=max(effective_min, max_base)))
    
    # Create gradient pattern (horizontal, vertical, or diagonal)
    pattern_type = draw(st.sampled_from(["horizontal", "vertical", "diagonal"]))
    
    # Create base gradient
    if pattern_type == "horizontal":
        gradient = np.linspace(0, 1, num_cols)
        base_pattern = np.tile(gradient, (num_rows, 1))
    elif pattern_type == "vertical":
        gradient = np.linspace(0, 1, num_rows)
        base_pattern = np.tile(gradient.reshape(-1, 1), (1, num_cols))
    else:  # diagonal
        x = np.linspace(0, 1, num_cols)
        y = np.linspace(0, 1, num_rows)
        xx, yy = np.meshgrid(x, y)
        base_pattern = (xx + yy) / 2
    
    # Scale to value range and add base
    scaled_pattern = base_pattern * value_range + base_value
    
    # Add small random noise (up to 5% of range) for more realistic texture
    noise_scale = value_range * 0.05
    
    # Create the multi-band image
    bands = []
    for _ in range(num_bands):
        # Add per-band noise
        noise = draw(arrays(
            dtype=np.float64,
            shape=(num_rows, num_cols),
            elements=st.floats(min_value=-noise_scale, max_value=noise_scale, allow_nan=False, allow_infinity=False)
        ))
        band = np.clip(scaled_pattern + noise, effective_min, effective_max)
        bands.append(band.astype(dtype))
    
    array = np.stack(bands, axis=0)
    
    return (array, pixel_type, num_bands, num_rows, num_cols)
