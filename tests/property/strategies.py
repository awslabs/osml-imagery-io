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
"""

from typing import Set, Tuple

import numpy as np
from aws.osml.io import PixelType
from hypothesis import assume
from hypothesis import strategies as st
from hypothesis.extra.numpy import arrays

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
    """
    return st.integers(min_value=min_bands, max_value=max_bands)


def block_sizes() -> st.SearchStrategy[Tuple[int, int]]:
    """Strategy for block dimensions.

    Returns a strategy that samples from common block sizes:
    (32, 32), (64, 64), (128, 128), (256, 256)

    Returns:
        Strategy producing (block_height, block_width) tuples
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
    """
    return st.from_regex(r"[A-Z][A-Z0-9]{0,9}", fullmatch=True)


def metadata_values() -> st.SearchStrategy[str]:
    """Strategy for valid metadata values.

    Generates printable ASCII strings suitable for NITF metadata values.
    NITF uses BCS-A (Basic Character Set - Alphanumeric) which is ASCII.

    Returns:
        Strategy producing valid metadata value strings (1-20 chars)
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

# Pixel types suitable for JPEG DCT compression (8-bit only)
# 12-bit JPEG is not supported due to libjpeg-turbo limitations
JPEG_PIXEL_TYPES = [
    PixelType.UInt8,
]

# JPEG IC codes
JPEG_IC_CODES = ["C3", "M3", "I1"]


@st.composite
def mask_patterns(
    draw,
    num_block_rows: int,
    num_block_cols: int,
) -> set:
    """Strategy for generating block mask patterns.

    Generates various mask patterns indicating which blocks are present (not masked).

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        num_block_rows: Number of block rows in the image
        num_block_cols: Number of block columns in the image

    Returns:
        A set of (row, col) tuples indicating which blocks are present.
    """
    pattern_type = draw(st.sampled_from([
        "all_present",      # No blocks masked
        "all_masked",       # All blocks masked (edge case)
        "checkerboard",     # Alternating pattern
        "border_only",      # Only edge blocks present
        "random",           # Random subset of blocks
        "single_block",     # Only one block present
    ]))

    all_blocks = {(r, c) for r in range(num_block_rows) for c in range(num_block_cols)}

    if pattern_type == "all_present":
        return all_blocks
    elif pattern_type == "all_masked":
        return set()
    elif pattern_type == "checkerboard":
        return {(r, c) for r, c in all_blocks if (r + c) % 2 == 0}
    elif pattern_type == "border_only":
        return {(r, c) for r, c in all_blocks
                if r == 0 or r == num_block_rows - 1 or c == 0 or c == num_block_cols - 1}
    elif pattern_type == "single_block":
        if all_blocks:
            return {draw(st.sampled_from(sorted(list(all_blocks))))}
        return set()
    else:  # random
        if all_blocks:
            return set(draw(st.lists(st.sampled_from(sorted(list(all_blocks))), unique=True)))
        return set()


def calculate_safe_j2k_decomposition_levels(
    block_height: int,
    block_width: int,
    num_rows: int = None,
    num_cols: int = None
) -> int:
    """Calculate safe JPEG 2000 decomposition levels for given block dimensions.

    OpenJPEG requires that the tile dimensions are large enough to support
    the requested number of decomposition levels. The requirement is:
    min_dim >= 2^decomposition_levels

    When image dimensions are provided, this also considers partial blocks
    at the edges which may be smaller than the nominal block size.

    Args:
        block_height: Height of the block/tile
        block_width: Width of the block/tile
        num_rows: Optional total image rows (to calculate partial block sizes)
        num_cols: Optional total image columns (to calculate partial block sizes)

    Returns:
        Safe number of decomposition levels (minimum 0)
    """
    min_dim = min(block_height, block_width)

    # If image dimensions provided, consider partial blocks at edges
    if num_rows is not None and num_cols is not None:
        # Calculate the size of the last partial block (if any)
        last_block_height = num_rows % block_height
        last_block_width = num_cols % block_width

        # If there's a partial block, consider its dimensions
        if last_block_height > 0:
            min_dim = min(min_dim, last_block_height)
        if last_block_width > 0:
            min_dim = min(min_dim, last_block_width)

    # Calculate max levels based on OpenJPEG's requirement:
    # min_dim >= 2^decomposition_levels
    # Therefore: decomposition_levels <= floor(log2(min_dim))
    if min_dim <= 1:
        return 0  # 1-pixel blocks can only have 0 decomposition levels

    # floor(log2(min_dim)) gives max safe levels, cap at 5 for reasonable compression
    max_levels = int(np.floor(np.log2(min_dim)))
    return min(5, max_levels)


@st.composite
def masked_image(
    draw,
    min_size: int = 32,
    max_size: int = 128,
    min_bands: int = 1,
    max_bands: int = 3,
) -> Tuple[np.ndarray, PixelType, int, int, int, int, int, Set[Tuple[int, int]], str]:
    """Composite strategy for generating masked images with metadata.

    Generates a random image with a mask pattern indicating which blocks are present.
    This is used for testing masked image roundtrip operations.

    Supports all masked IC codes:
    - NM: uncompressed with mask (any pixel type)
    - M8: JPEG 2000 with mask (UInt8, UInt16, Int16)
    - M3: JPEG DCT with mask (UInt8 only, JPEG-friendly pixel values)

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum dimension size (default 32)
        max_size: Maximum dimension size (default 128)
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 3)

    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols,
                  block_height, block_width, provided_blocks, ic_value)
        - array: numpy array with shape (num_bands, num_rows, num_cols)
        - pixel_type: PixelType enum value
        - num_bands: number of bands
        - num_rows: number of rows
        - num_cols: number of columns
        - block_height: height of each block
        - block_width: width of each block
        - provided_blocks: set of (row, col) tuples for blocks that have data
        - ic_value: IC value (NM, M8, or M3)
    """
    # Choose IC value first, as it affects pixel type and dimension constraints
    # NM = uncompressed with mask, M8 = JPEG 2000 with mask, M3 = JPEG DCT with mask
    ic_value = draw(st.sampled_from(["NM", "M8", "M3"]))

    # M3 (JPEG DCT) only supports 8-bit and standard JPEG band counts (1 or 3).
    # 2-band JPEG is not a valid configuration.
    if ic_value == "M3":
        pixel_type = PixelType.UInt8
        valid_m3_bands = [b for b in [1, 3] if min_bands <= b <= max_bands]
        assume(len(valid_m3_bands) > 0)
        num_bands = draw(st.sampled_from(valid_m3_bands))
    else:
        pixel_type = draw(st.sampled_from([PixelType.UInt8, PixelType.UInt16, PixelType.Int16]))
        num_bands = draw(band_counts(min_bands=min_bands, max_bands=max_bands))
    block_height, block_width = draw(block_sizes())

    # M8 and M3 both need image dimensions as multiples of block size.
    # M8: OpenJPEG has minimum tile size requirements.
    # M3: JPEG DCT works on 8x8 blocks; aligning to block size avoids
    #     partial-block edge artifacts that hurt quality metrics.
    if ic_value in ("M8", "M3"):
        effective_min = max(min_size, 64) if ic_value == "M3" else min_size
        min_blocks = max(2 if ic_value == "M3" else 1, effective_min // block_height)
        max_blocks = max(min_blocks, max_size // block_height)
        num_block_rows = draw(st.integers(min_value=min_blocks, max_value=max_blocks))
        num_rows = num_block_rows * block_height

        min_blocks = max(2 if ic_value == "M3" else 1, effective_min // block_width)
        max_blocks = max(min_blocks, max_size // block_width)
        num_block_cols = draw(st.integers(min_value=min_blocks, max_value=max_blocks))
        num_cols = num_block_cols * block_width
    else:
        # For NM (uncompressed), any dimensions work
        num_rows, num_cols = draw(image_dimensions(min_size=min_size, max_size=max_size))

        # Ensure block size doesn't exceed image dimensions
        block_height = min(block_height, num_rows)
        block_width = min(block_width, num_cols)

        # Calculate block grid
        num_block_rows = (num_rows + block_height - 1) // block_height
        num_block_cols = (num_cols + block_width - 1) // block_width

    # Generate mask pattern
    provided_blocks = draw(mask_patterns(num_block_rows, num_block_cols))

    # Generate image data — M3 needs JPEG-friendly values with guaranteed
    # variance for meaningful PSNR/SSIM calculations.
    if ic_value == "M3":
        value_range = draw(st.integers(min_value=100, max_value=200))
        base_value = draw(st.integers(min_value=20, max_value=55))
        gradient = np.linspace(0, 1, num_cols)
        base_pattern = np.tile(gradient, (num_rows, 1))
        scaled_pattern = base_pattern * value_range + base_value

        bands = []
        for _ in range(num_bands):
            noise = draw(arrays(
                dtype=np.float64,
                shape=(num_rows, num_cols),
                elements=st.floats(min_value=-5, max_value=5,
                                   allow_nan=False, allow_infinity=False),
            ))
            band = np.clip(scaled_pattern + noise, 0, 255)
            bands.append(band.astype(np.uint8))
        array = np.stack(bands, axis=0)
    else:
        dtype = get_numpy_dtype(pixel_type)
        array = draw(arrays(dtype=dtype, shape=(num_bands, num_rows, num_cols)))

    return (array, pixel_type, num_bands, num_rows, num_cols,
            block_height, block_width, provided_blocks, ic_value)


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


# =============================================================================
# JPEG DCT Compression Strategies
# =============================================================================

def jpeg_pixel_types() -> st.SearchStrategy[PixelType]:
    """Strategy for pixel types supported by JPEG DCT compression.

    JPEG DCT only supports 8-bit samples. 12-bit JPEG is not supported
    due to libjpeg-turbo architectural constraints.

    Returns:
        Strategy producing UInt8 pixel type only.
    """
    return st.sampled_from(JPEG_PIXEL_TYPES)


def jpeg_ic_codes() -> st.SearchStrategy[str]:
    """Strategy for JPEG DCT IC codes.

    Returns a strategy that samples from:
    - C3: JPEG DCT compressed imagery
    - M3: JPEG DCT compressed imagery with block mask
    - I1: Downsampled JPEG (single block ≤2048×2048)

    Returns:
        Strategy producing JPEG IC code strings.
    """
    return st.sampled_from(JPEG_IC_CODES)


def jpeg_quality() -> st.SearchStrategy[int]:
    """Strategy for JPEG quality values.

    JPEG quality ranges from 1 (worst) to 100 (best).
    For property testing, we use values that provide good quality
    to ensure PSNR/SSIM thresholds are met.

    Returns:
        Strategy producing quality values 50-95.
    """
    return st.integers(min_value=50, max_value=95)


def jpeg_comrat() -> st.SearchStrategy[str]:
    """Strategy for JPEG COMRAT values.

    COMRAT for JPEG uses format "nn.n" representing quality 00.0 to 99.9.
    Higher values = higher quality.

    Returns:
        Strategy producing valid JPEG COMRAT strings.
    """
    # Generate quality values that map to good compression quality
    return st.integers(min_value=50, max_value=95).map(
        lambda q: f"{q:02d}.0"
    )


def i1_image_dimensions() -> st.SearchStrategy[Tuple[int, int]]:
    """Strategy for IC=I1 (downsampled JPEG) image dimensions.

    I1 images are constrained to ≤2048×2048 pixels and are encoded
    as a single JPEG block.

    Returns:
        Strategy producing (num_rows, num_cols) tuples within I1 constraints.
    """
    return st.tuples(
        st.integers(min_value=32, max_value=512),  # Use smaller sizes for faster tests
        st.integers(min_value=32, max_value=512)
    )


@st.composite
def jpeg_image_for_compression(
    draw,
    min_size: int = 32,
    max_size: int = 128,
    min_bands: int = 1,
    max_bands: int = 3,
) -> Tuple[np.ndarray, PixelType, int, int, int]:
    """Composite strategy for images suitable for JPEG DCT compression testing.

    Generates 8-bit images with realistic value distributions that work well
    with lossy JPEG compression quality metrics. Creates gradient-like images
    with guaranteed variance for meaningful PSNR and SSIM calculations.

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum dimension size (default 32)
        max_size: Maximum dimension size (default 128)
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 3)

    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols)
    """
    # JPEG only supports 8-bit
    pixel_type = PixelType.UInt8
    num_rows, num_cols = draw(image_dimensions(min_size=min_size, max_size=max_size))
    num_bands = draw(band_counts(min_bands=min_bands, max_bands=max_bands))

    dtype = np.uint8

    # Generate a gradient-based image with noise overlay for realistic compression
    # Pick a value range that spans meaningful values for 8-bit
    value_range = draw(st.integers(min_value=100, max_value=200))
    base_value = draw(st.integers(min_value=20, max_value=55))

    # Create gradient pattern
    pattern_type = draw(st.sampled_from(["horizontal", "vertical", "diagonal"]))

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
        noise = draw(arrays(
            dtype=np.float64,
            shape=(num_rows, num_cols),
            elements=st.floats(min_value=-noise_scale, max_value=noise_scale,
                             allow_nan=False, allow_infinity=False)
        ))
        band = np.clip(scaled_pattern + noise, 0, 255)
        bands.append(band.astype(dtype))

    array = np.stack(bands, axis=0)

    return (array, pixel_type, num_bands, num_rows, num_cols)


@st.composite
def jpeg_i1_image(
    draw,
    min_size: int = 32,
    max_size: int = 256,
) -> Tuple[np.ndarray, PixelType, int, int, int]:
    """Composite strategy for IC=I1 (downsampled JPEG) images.

    Generates images suitable for I1 encoding, which is constrained to
    ≤2048×2048 pixels and encoded as a single JPEG block.

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum dimension size (default 32)
        max_size: Maximum dimension size (default 256, kept small for tests)

    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols)
    """
    # I1 supports 1 or 3 bands (grayscale or RGB)
    num_bands = draw(st.sampled_from([1, 3]))

    # Use the JPEG image generator with I1 dimension constraints
    array, pixel_type, _, num_rows, num_cols = draw(
        jpeg_image_for_compression(
            min_size=min_size,
            max_size=max_size,
            min_bands=num_bands,
            max_bands=num_bands,
        )
    )

    return (array, pixel_type, num_bands, num_rows, num_cols)


@st.composite
def masked_jpeg_image(
    draw,
    min_size: int = 64,
    max_size: int = 128,
    min_bands: int = 1,
    max_bands: int = 3,
) -> Tuple[np.ndarray, PixelType, int, int, int, int, int, Set[Tuple[int, int]]]:
    """Composite strategy for generating masked JPEG (IC=M3) images.

    Generates a random 8-bit image with a mask pattern indicating which
    blocks are present. Used for testing masked JPEG roundtrip operations.

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum dimension size (default 64)
        max_size: Maximum dimension size (default 128)
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 3)

    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols,
                  block_height, block_width, provided_blocks)
    """
    # JPEG only supports 8-bit
    pixel_type = PixelType.UInt8
    num_bands = draw(band_counts(min_bands=min_bands, max_bands=max_bands))
    block_height, block_width = draw(block_sizes())

    # Ensure image dimensions are multiples of block size for cleaner testing
    min_blocks = max(2, min_size // block_height)
    max_blocks = max(min_blocks, max_size // block_height)
    num_block_rows = draw(st.integers(min_value=min_blocks, max_value=max_blocks))
    num_rows = num_block_rows * block_height

    min_blocks = max(2, min_size // block_width)
    max_blocks = max(min_blocks, max_size // block_width)
    num_block_cols = draw(st.integers(min_value=min_blocks, max_value=max_blocks))
    num_cols = num_block_cols * block_width

    # Generate mask pattern (exclude all_masked to ensure we have data)
    pattern_type = draw(st.sampled_from([
        "all_present",
        "checkerboard",
        "border_only",
        "random",
        "single_block",
    ]))

    all_blocks = {(r, c) for r in range(num_block_rows) for c in range(num_block_cols)}

    if pattern_type == "all_present":
        provided_blocks = all_blocks
    elif pattern_type == "checkerboard":
        provided_blocks = {(r, c) for r, c in all_blocks if (r + c) % 2 == 0}
    elif pattern_type == "border_only":
        provided_blocks = {(r, c) for r, c in all_blocks
                if r == 0 or r == num_block_rows - 1 or c == 0 or c == num_block_cols - 1}
    elif pattern_type == "single_block":
        provided_blocks = {draw(st.sampled_from(sorted(list(all_blocks))))}
    else:  # random - ensure at least one block
        selected = draw(st.lists(st.sampled_from(sorted(list(all_blocks))),
                                min_size=1, unique=True))
        provided_blocks = set(selected)

    # Generate image data using JPEG-friendly values
    dtype = np.uint8
    value_range = draw(st.integers(min_value=100, max_value=200))
    base_value = draw(st.integers(min_value=20, max_value=55))

    # Create gradient pattern
    gradient = np.linspace(0, 1, num_cols)
    base_pattern = np.tile(gradient, (num_rows, 1))
    scaled_pattern = base_pattern * value_range + base_value

    bands = []
    for _ in range(num_bands):
        noise = draw(arrays(
            dtype=np.float64,
            shape=(num_rows, num_cols),
            elements=st.floats(min_value=-5, max_value=5,
                             allow_nan=False, allow_infinity=False)
        ))
        band = np.clip(scaled_pattern + noise, 0, 255)
        bands.append(band.astype(dtype))

    array = np.stack(bands, axis=0)

    return (array, pixel_type, num_bands, num_rows, num_cols,
            block_height, block_width, provided_blocks)


# =============================================================================
# TIFF Format Strategies
# =============================================================================

# Pixel types that PIL can write to TIFF (used for property test generation).
# PIL writes int16 as int32 and doesn't support int8/uint32/float64,
# so we limit to what PIL can actually produce correctly.
TIFF_PIL_PIXEL_TYPES = [
    PixelType.UInt8,
    PixelType.UInt16,
    PixelType.Int32,
    PixelType.Float32,
]

# Compression names mapped to PIL compression strings
TIFF_COMPRESSION_MAP = {
    "None": "raw",
    "LZW": "tiff_lzw",
    "Deflate": "tiff_adobe_deflate",
    "PackBits": "packbits",
}


def tiff_compression() -> st.SearchStrategy[str]:
    """Strategy for TIFF compression types.

    Returns a strategy that samples from supported lossless compressions.
    """
    return st.sampled_from(["None", "LZW", "Deflate", "PackBits"])


def tiff_planar_config() -> st.SearchStrategy[int]:
    """Strategy for TIFF planar configuration.

    Returns 1 (chunky/RGBRGB) or 2 (planar/RRR...GGG...BBB...).
    Note: PIL only writes chunky (1). Planar tests deferred to Phase 2.
    """
    return st.sampled_from([1, 2])


def tiff_layout() -> st.SearchStrategy[str]:
    """Strategy for TIFF data layout.

    Returns 'tiled' or 'stripped'.
    Note: PIL only writes stripped. Tiled tests use the existing small.tif fixture.
    """
    return st.sampled_from(["tiled", "stripped"])


def tiff_pil_pixel_types() -> st.SearchStrategy[PixelType]:
    """Strategy for pixel types that PIL can write to TIFF.

    Limited to types PIL produces with correct TIFF tags:
    UInt8, UInt16, Int32, Float32.
    """
    return st.sampled_from(TIFF_PIL_PIXEL_TYPES)


def tiff_rows_per_strip(
    image_height: int,
    min_rps: int = 8,
) -> st.SearchStrategy[int]:
    """Strategy for RowsPerStrip values.

    Generates strip heights that divide the image into multiple strips
    when possible, or a single strip for small images.

    Args:
        image_height: Total image height in pixels
        min_rps: Minimum rows per strip (default 8)
    """
    # Pick from powers of 2 and the full height
    candidates = [v for v in [8, 16, 32, 64, 128] if min_rps <= v <= image_height]
    if not candidates or image_height not in candidates:
        candidates.append(image_height)
    return st.sampled_from(sorted(set(candidates)))


@st.composite
def tiff_image_config(
    draw,
    min_size: int = 16,
    max_size: int = 128,
    min_bands: int = 1,
    max_bands: int = 3,
) -> dict:
    """Composite strategy for TIFF image configurations writable by PIL.

    Generates a complete configuration dict for creating a test TIFF:
    pixel_type, width, height, bands, compression, rows_per_strip.

    PIL limitations applied:
    - Always chunky (PlanarConfiguration=1)
    - Always stripped (no tile support)
    - Bands: 1 or 3 (PIL modes L/I;16/I/F for 1-band, RGB for 3-band uint8)
    - Float32 and Int32 are single-band only in PIL

    Args:
        draw: Hypothesis draw function
        min_size: Minimum dimension
        max_size: Maximum dimension
        min_bands: Minimum bands (1)
        max_bands: Maximum bands (3)

    Returns:
        Dict with keys: pixel_type, width, height, bands, compression,
        rows_per_strip, pil_compression
    """
    pixel_type = draw(tiff_pil_pixel_types())
    compression = draw(tiff_compression())
    height = draw(st.integers(min_value=min_size, max_value=max_size))
    width = draw(st.integers(min_value=min_size, max_value=max_size))

    # PIL only supports multi-band (RGB) for uint8
    if pixel_type == PixelType.UInt8:
        bands = draw(st.sampled_from([b for b in [1, 3] if min_bands <= b <= max_bands]))
    else:
        bands = 1

    rows_per_strip = draw(tiff_rows_per_strip(height))

    return {
        "pixel_type": pixel_type,
        "width": width,
        "height": height,
        "bands": bands,
        "compression": compression,
        "rows_per_strip": rows_per_strip,
        "pil_compression": TIFF_COMPRESSION_MAP[compression],
    }


# =============================================================================
# TIFF Writing Strategies (Phase 2 — our own writer)
# =============================================================================

# All pixel types supported by TIFFDatasetWriter
TIFF_WRITER_PIXEL_TYPES = [
    PixelType.UInt8,
    PixelType.UInt16,
    PixelType.UInt32,
    PixelType.Int8,
    PixelType.Int16,
    PixelType.Int32,
    PixelType.Float32,
    PixelType.Float64,
]

# Writer-supported compressions as numeric TIFF tag 259 values.
# 1=None, 5=LZW, 7=JPEG, 8=Deflate.  No PackBits — writer doesn't support it.
TIFF_WRITER_COMPRESSIONS = [1, 5, 7, 8]

# Lossless-only subset (excludes JPEG) for strategies that need exact roundtrip.
TIFF_WRITER_LOSSLESS_COMPRESSIONS = [1, 5, 8]

# Writer-supported predictor values (numeric TIFF tag 317).
# 1=None, 2=Horizontal differencing.
TIFF_WRITER_PREDICTORS = [1, 2]

# Writer-supported planar configurations (numeric TIFF tag 284).
# 1=Chunky, 2=Planar.
TIFF_WRITER_PLANAR_CONFIGS = [1, 2]

# Tile sizes that are multiples of 16 (TIFF spec requirement)
TIFF_TILE_SIZES = [64, 128, 256]


def tiff_writer_pixel_types() -> st.SearchStrategy[PixelType]:
    """Strategy for all pixel types supported by TIFFDatasetWriter.

    Includes the full set: UInt8–UInt32, Int8–Int32, Float32, Float64.
    """
    return st.sampled_from(TIFF_WRITER_PIXEL_TYPES)


def tiff_writer_compression() -> st.SearchStrategy[int]:
    """Strategy for lossless compression types supported by TIFFDatasetWriter.

    Returns one of: 1 (None), 5 (LZW), 8 (Deflate).
    Use ``tiff_writer_all_compression`` to include JPEG (7).
    """
    return st.sampled_from(TIFF_WRITER_LOSSLESS_COMPRESSIONS)


def tiff_writer_all_compression() -> st.SearchStrategy[int]:
    """Strategy for all compression types supported by TIFFDatasetWriter.

    Returns one of: 1 (None), 5 (LZW), 7 (JPEG), 8 (Deflate).
    """
    return st.sampled_from(TIFF_WRITER_COMPRESSIONS)


@st.composite
def tiff_encoding_hints(draw, include_jpeg: bool = False) -> dict:
    """Strategy generating TIFF encoding hint key-value pairs.

    Produces a dict with numeric TIFF tag IDs as keys (matching the TIFF 6.0
    specification) suitable for passing to ``BufferedMetadataProvider``.

    Keys: "322" (TileWidth), "323" (TileLength), "259" (Compression),
    "317" (Predictor), "284" (PlanarConfiguration).
    When JPEG is selected, also includes "65537" (JPEG quality).

    Values for tags 259, 317, and 284 are integers (TIFF-spec numeric codes).
    Tile dimensions remain strings (they are always parsed as strings).

    The predictor is chosen consistently with the compression: Horizontal
    is only meaningful for LZW/Deflate; None is always valid.
    JPEG forces predictor to 1 and planar config to 1.

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        include_jpeg: If True, include JPEG (7) in compression choices.
            Default False for backward compatibility with lossless tests.

    Returns:
        Dict of numeric tag ID string → value (str for tile dims, int for
        compression/predictor/planar/quality).
    """
    tile_w = draw(st.sampled_from(TIFF_TILE_SIZES))
    tile_h = draw(st.sampled_from(TIFF_TILE_SIZES))

    if include_jpeg:
        compression = draw(tiff_writer_all_compression())
    else:
        compression = draw(tiff_writer_compression())

    if compression == 7:
        # JPEG: predictor must be 1, planar must be 1
        predictor = 1
        planar = 1
        quality = draw(st.integers(min_value=50, max_value=95))
        # JPEGCOLORMODE_RGB: tell libtiff to accept RGB input and handle
        # the RGB↔YCbCr conversion internally as part of JPEG encoding.
        jpeg_color_mode = 1
    else:
        planar = draw(st.sampled_from(TIFF_WRITER_PLANAR_CONFIGS))
        if compression == 1:
            predictor = 1
        else:
            predictor = draw(st.sampled_from(TIFF_WRITER_PREDICTORS))
        quality = None
        jpeg_color_mode = None

    hints = {
        "322": str(tile_w),       # TileWidth
        "323": str(tile_h),       # TileLength
        "259": compression,       # Compression (int)
        "317": predictor,         # Predictor (int)
        "284": planar,            # PlanarConfiguration (int)
    }

    if quality is not None:
        hints["65537"] = quality   # JPEG quality (int)

    if jpeg_color_mode is not None:
        hints["65538"] = jpeg_color_mode  # JPEG color mode (int)

    return hints


@st.composite
def tiff_writable_image(
    draw,
    min_size: int = 16,
    max_size: int = 128,
    min_bands: int = 1,
    max_bands: int = 4,
    include_jpeg: bool = False,
) -> Tuple[np.ndarray, PixelType, int, int, int, dict]:
    """Composite strategy for a writable TIFF image with encoding hints.

    Generates a random image array (CHW) together with pixel type metadata
    and a matching set of TIFF encoding hints suitable for
    ``BufferedMetadataProvider``.

    When ``include_jpeg`` is True, JPEG compression (7) may be drawn.
    JPEG constrains pixel type to UInt8 and bands to {1, 3}, and generates
    gradient-based images for meaningful PSNR comparison.

    Args:
        draw: Hypothesis draw function
        min_size: Minimum image dimension (default 16)
        max_size: Maximum image dimension (default 128)
        min_bands: Minimum band count (default 1)
        max_bands: Maximum band count (default 4)
        include_jpeg: If True, include JPEG in compression choices.

    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols, hints)
        where *hints* is a dict of encoding hint values.
    """
    hints = draw(tiff_encoding_hints(include_jpeg=include_jpeg))
    is_jpeg = hints["259"] == 7

    if is_jpeg:
        # JPEG: constrain to UInt8 and {1, 3} bands; use gradient images
        jpeg_bands = draw(st.sampled_from([b for b in [1, 3] if min_bands <= b <= max_bands]))
        array, pixel_type, num_bands, num_rows, num_cols = draw(
            jpeg_image_for_compression(
                min_size=max(min_size, 32),
                max_size=max_size,
                min_bands=jpeg_bands,
                max_bands=jpeg_bands,
            )
        )
    else:
        pixel_type = draw(tiff_writer_pixel_types())
        num_rows, num_cols = draw(image_dimensions(min_size=min_size, max_size=max_size))
        num_bands = draw(band_counts(min_bands=min_bands, max_bands=max_bands))
        array = draw(image_arrays(pixel_type, num_bands, num_rows, num_cols))

    return (array, pixel_type, num_bands, num_rows, num_cols, hints)


# =============================================================================
# GeoTIFF Metadata Strategies
# =============================================================================


def geotiff_model_type() -> st.SearchStrategy[str]:
    """Strategy for GeoTIFF model type values.

    Draws from the two valid GTModelTypeGeoKey labels.
    """
    return st.sampled_from(["Projected", "Geographic"])


def geotiff_raster_type() -> st.SearchStrategy[str]:
    """Strategy for GeoTIFF raster type values.

    Draws from the two valid GTRasterTypeGeoKey labels.
    """
    return st.sampled_from(["PixelIsArea", "PixelIsPoint"])


def epsg_codes() -> st.SearchStrategy[int]:
    """Strategy for valid EPSG codes representable as u16.

    EPSG codes range from 1 to 32767 (positive i16 / valid u16 range).
    """
    return st.integers(min_value=1, max_value=32767)


def pixel_scale() -> st.SearchStrategy[list]:
    """Strategy for GeoPixelScale: 3-element arrays of positive floats.

    Values are kept in a reasonable range to avoid floating-point edge cases
    during roundtrip through libtiff (which stores as DOUBLE).
    """
    return st.lists(
        st.floats(min_value=0.001, max_value=1e6, allow_nan=False, allow_infinity=False),
        min_size=3,
        max_size=3,
    )


def tiepoint_tuples() -> st.SearchStrategy[list]:
    """Strategy for GeoTiepoints: lists of 1-4 tiepoint 6-element float arrays.

    Each tiepoint is [pixel_x, pixel_y, pixel_z, geo_x, geo_y, geo_z].
    """
    single_tiepoint = st.lists(
        st.floats(min_value=-1e8, max_value=1e8, allow_nan=False, allow_infinity=False),
        min_size=6,
        max_size=6,
    )
    return st.lists(single_tiepoint, min_size=1, max_size=4)


def transformation_matrix() -> st.SearchStrategy[list]:
    """Strategy for GeoTransformation: 16-element float arrays (4x4 matrix).
    """
    return st.lists(
        st.floats(min_value=-1e8, max_value=1e8, allow_nan=False, allow_infinity=False),
        min_size=16,
        max_size=16,
    )


@st.composite
def geotiff_metadata(draw) -> dict:
    """Composite strategy for valid GeoTIFF encoding hint dictionaries.

    Generates raw numeric GeoTIFF tags suitable for passing to
    BufferedMetadataProvider via set_json. The dict uses numeric tag ID
    string keys that the writer's build_geokey_directory() expects:

    - "34735" — GeoKeyDirectory (u16 array)
    - "33550" — ModelPixelScale (3 doubles, optional)
    - "33922" — ModelTiepoint (flat array of 6-tuples, optional)
    - "34264" — ModelTransformation (16 doubles, optional)

    Always includes a GeoKeyDirectory with GTModelTypeGeoKey (1024) and
    the matching CRS key. Optionally includes GTRasterTypeGeoKey (1025).
    """
    # GeoKey constants
    MODEL_TYPE_PROJECTED = 1
    MODEL_TYPE_GEOGRAPHIC = 2
    RASTER_PIXEL_IS_AREA = 1
    RASTER_PIXEL_IS_POINT = 2

    # Build GeoKey directory entries: [key_id, tiff_tag_location, count, value]
    geokeys = []

    # Always include model type
    is_projected = draw(st.booleans())
    model_type_val = MODEL_TYPE_PROJECTED if is_projected else MODEL_TYPE_GEOGRAPHIC
    geokeys.append([1024, 0, 1, model_type_val])

    # CRS key depends on model type
    epsg = draw(epsg_codes())
    if is_projected:
        geokeys.append([3072, 0, 1, epsg])  # ProjectedCSTypeGeoKey
    else:
        geokeys.append([2048, 0, 1, epsg])  # GeographicTypeGeoKey

    # Optionally include raster type
    if draw(st.booleans()):
        raster_val = draw(st.sampled_from([RASTER_PIXEL_IS_AREA, RASTER_PIXEL_IS_POINT]))
        geokeys.append([1025, 0, 1, raster_val])

    # Build the directory array: [version, revision, minor_revision, num_keys, ...entries]
    num_keys = len(geokeys)
    directory = [1, 1, 0, num_keys]
    for entry in geokeys:
        directory.extend(entry)

    hints = {"34735": directory}

    # Transformation tags: either PixelScale+Tiepoints or Transformation matrix
    use_transform = draw(st.sampled_from(["scale_tiepoints", "matrix", "none"]))
    if use_transform == "scale_tiepoints":
        hints["33550"] = draw(pixel_scale())
        # Flatten tiepoint tuples into a single array
        tp_tuples = draw(tiepoint_tuples())
        hints["33922"] = [v for tp in tp_tuples for v in tp]
    elif use_transform == "matrix":
        hints["34264"] = draw(transformation_matrix())

    return hints


# =============================================================================
# PNG Format Strategies
# =============================================================================

# Pixel types supported by PNG writer (8-bit and 16-bit unsigned integers)
PNG_PIXEL_TYPES = [
    PixelType.UInt8,
    PixelType.UInt16,
]

# PNG-supported band counts: 1 (Gray), 2 (GrayAlpha), 3 (RGB), 4 (RGBA)
PNG_BAND_COUNTS = [1, 2, 3, 4]


def png_pixel_types() -> st.SearchStrategy[PixelType]:
    """Strategy for PNG-supported pixel types: UInt8 and UInt16."""
    return st.sampled_from(PNG_PIXEL_TYPES)


def png_band_counts() -> st.SearchStrategy[int]:
    """Strategy for PNG-supported band counts: 1 (Gray), 2 (GrayAlpha), 3 (RGB), 4 (RGBA)."""
    return st.sampled_from(PNG_BAND_COUNTS)


@st.composite
def png_writable_image(
    draw,
    min_size: int = 16,
    max_size: int = 64,
) -> Tuple[np.ndarray, PixelType, int, int, int]:
    """Composite strategy for a random image writable as PNG.

    Generates a random image array (BSQ) together with pixel type metadata
    suitable for writing via PNGDatasetWriter.

    Args:
        draw: Hypothesis draw function (injected by @st.composite)
        min_size: Minimum image dimension (default 16)
        max_size: Maximum image dimension (default 64)

    Returns:
        Tuple of (array, pixel_type, num_bands, num_rows, num_cols)
        - array: numpy array with shape (num_bands, num_rows, num_cols)
        - pixel_type: PixelType enum value (UInt8 or UInt16)
        - num_bands: number of bands (1, 2, 3, or 4)
        - num_rows: number of rows
        - num_cols: number of columns
    """
    pixel_type = draw(png_pixel_types())
    num_bands = draw(png_band_counts())
    num_rows, num_cols = draw(image_dimensions(min_size=min_size, max_size=max_size))
    array = draw(image_arrays(pixel_type, num_bands, num_rows, num_cols))

    return (array, pixel_type, num_bands, num_rows, num_cols)
