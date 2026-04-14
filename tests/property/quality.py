"""Quality metrics for lossy compression validation.

This module provides PSNR and SSIM calculation functions for comparing
original and decoded images in lossy compression roundtrip tests.

Quality thresholds:
- MIN_PSNR_DB = 28.0 dB (minimum acceptable PSNR for lossy compression)
- MIN_SSIM = 0.95 (minimum acceptable structural similarity)
"""

import numpy as np

# Quality thresholds for lossy compression validation
MIN_PSNR_DB = 28.0
MIN_SSIM = 0.95


def calculate_psnr(original: np.ndarray, decoded: np.ndarray, use_actual_range: bool = False) -> float:
    """Calculate Peak Signal-to-Noise Ratio in dB.

    PSNR measures the ratio between the maximum possible signal power
    and the power of corrupting noise. Higher values indicate better quality.

    Args:
        original: Original image array
        decoded: Decoded image array (must have same shape and dtype as original)
        use_actual_range: If True, use the actual data range (max - min) instead of
                         dtype range. This is more appropriate for images that don't
                         use the full dynamic range of the dtype.

    Returns:
        PSNR value in decibels (dB). Returns float('inf') for identical images.

    Raises:
        ValueError: If arrays have different shapes
    """
    if original.shape != decoded.shape:
        raise ValueError(
            f"Shape mismatch: original {original.shape} vs decoded {decoded.shape}"
        )

    # Calculate Mean Squared Error
    mse = np.mean((original.astype(np.float64) - decoded.astype(np.float64)) ** 2)

    if mse == 0:
        return float('inf')

    # Determine max pixel value for PSNR calculation
    if use_actual_range:
        # Use actual data range (max - min) for images with limited dynamic range
        # This gives a more meaningful PSNR for images that don't use the full dtype range
        data_max = float(np.max(original))
        data_min = float(np.min(original))
        data_range = data_max - data_min

        # Use the data range as the "max signal" value
        # For constant images (range=0), fall back to dtype max
        if data_range > 0:
            max_pixel = data_range
        elif np.issubdtype(original.dtype, np.integer):
            max_pixel = float(np.iinfo(original.dtype).max)
        else:
            max_pixel = 1.0
    elif np.issubdtype(original.dtype, np.integer):
        max_pixel = float(np.iinfo(original.dtype).max)
    elif np.issubdtype(original.dtype, np.floating):
        # For float images, assume [0, 1] range
        max_pixel = 1.0
    else:
        raise ValueError(f"Unsupported dtype: {original.dtype}")

    psnr = 20 * np.log10(max_pixel / np.sqrt(mse))
    return float(psnr)


def calculate_ssim(original: np.ndarray, decoded: np.ndarray) -> float:
    """Calculate Structural Similarity Index Measure (SSIM).

    SSIM measures the perceived quality difference between two images,
    considering luminance, contrast, and structure. Values range from -1 to 1,
    where 1 indicates identical images.

    This implementation uses scikit-image if available, otherwise falls back
    to a simplified calculation.

    Args:
        original: Original image array
        decoded: Decoded image array (must have same shape and dtype as original)

    Returns:
        SSIM value between -1 and 1. Higher values indicate better similarity.

    Raises:
        ValueError: If arrays have different shapes
    """
    if original.shape != decoded.shape:
        raise ValueError(
            f"Shape mismatch: original {original.shape} vs decoded {decoded.shape}"
        )

    # Try to use scikit-image for accurate SSIM
    try:
        from skimage.metrics import structural_similarity

        # Handle multi-band images (bands, rows, cols)
        if original.ndim == 3:
            # Calculate SSIM per band and average
            ssim_values = []
            for band in range(original.shape[0]):
                ssim_val = structural_similarity(
                    original[band],
                    decoded[band],
                    data_range=_get_data_range(original.dtype)
                )
                ssim_values.append(ssim_val)
            return float(np.mean(ssim_values))
        else:
            return float(structural_similarity(
                original,
                decoded,
                data_range=_get_data_range(original.dtype)
            ))
    except ImportError:
        # Fall back to simplified SSIM calculation
        return _simplified_ssim(original, decoded)


def _get_data_range(dtype: np.dtype) -> float:
    """Get the data range for a given dtype."""
    if np.issubdtype(dtype, np.integer):
        info = np.iinfo(dtype)
        return float(info.max - info.min)
    elif np.issubdtype(dtype, np.floating):
        return 1.0
    else:
        raise ValueError(f"Unsupported dtype: {dtype}")


def _simplified_ssim(original: np.ndarray, decoded: np.ndarray) -> float:
    """Simplified SSIM calculation when scikit-image is not available.

    This is a basic implementation that captures the essence of SSIM
    but may not match the full algorithm exactly.
    """
    # Fast path: identical arrays
    if np.array_equal(original, decoded):
        return 1.0

    # Constants for numerical stability
    C1 = (0.01 * _get_data_range(original.dtype)) ** 2
    C2 = (0.03 * _get_data_range(original.dtype)) ** 2

    # Convert to float64 for calculations
    x = original.astype(np.float64).flatten()
    y = decoded.astype(np.float64).flatten()

    # Calculate means
    mu_x = np.mean(x)
    mu_y = np.mean(y)

    # Calculate variances and covariance
    sigma_x_sq = np.var(x)
    sigma_y_sq = np.var(y)
    sigma_xy = np.cov(x, y)[0, 1]

    # SSIM formula
    numerator = (2 * mu_x * mu_y + C1) * (2 * sigma_xy + C2)
    denominator = (mu_x ** 2 + mu_y ** 2 + C1) * (sigma_x_sq + sigma_y_sq + C2)

    return float(numerator / denominator)
