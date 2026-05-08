"""Convenience functions for common image read/write/inspect operations.

This module provides a thin layer over the low-level Rust-backed API,
making common tasks achievable in one or two lines of Python. The
low-level API (``DatasetReader``, ``DatasetWriter``, ``get_block``, etc.)
remains unchanged and available for advanced use cases.

Public API
----------
- :func:`imread` — read an image (or windowed region) as a NumPy array
- :func:`imsave` — save a NumPy array to an image file
- :func:`iminfo` — get image metadata without reading pixels
- :func:`tiles`  — iterate over fixed-size tiles of a large image
- :class:`ImageInfo` — read-only metadata returned by :func:`iminfo`
- :class:`Tile` — a tile of pixel data yielded by :func:`tiles`
"""

from __future__ import annotations

import math
import os
from collections.abc import Iterator
from dataclasses import dataclass
from typing import TYPE_CHECKING, BinaryIO

import numpy as np

if TYPE_CHECKING:
    from numpy.typing import NDArray

# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ImageInfo:
    """Read-only image metadata returned by :func:`iminfo`.

    Attributes
    ----------
    width : int
        Image width in pixels (number of columns).
    height : int
        Image height in pixels (number of rows).
    bands : int
        Number of spectral bands.
    dtype : str
        NumPy dtype string (e.g. ``"uint8"``, ``"float32"``).
    block_size : tuple[int, int]
        Block dimensions as ``(block_width, block_height)``.
    num_resolution_levels : int
        Number of resolution levels available for decoding.
    asset_key : str
        The resolved asset key that was used.
    metadata : dict
        Format-specific metadata dictionary for the image segment.
        For NITF files this contains subheader fields (``IC``,
        ``IGEOLO``, etc.) and parsed TREs as nested dicts. For TIFF
        files this contains IFD tags keyed by numeric tag ID strings.
        The dictionary is a snapshot taken when :func:`iminfo` is
        called — it is not a live reference.
    """

    width: int
    height: int
    bands: int
    dtype: str
    block_size: tuple[int, int]
    num_resolution_levels: int
    asset_key: str
    metadata: dict


@dataclass
class Tile:
    """A rectangular tile of pixel data from the :func:`tiles` iterator.

    Attributes
    ----------
    data : numpy.ndarray
        Pixel data in CHW layout ``(bands, height, width)``.
    x : int
        Column offset of the tile's top-left corner in image coordinates.
    y : int
        Row offset of the tile's top-left corner in image coordinates.
    tile_col : int
        Tile grid column index (0-based).
    tile_row : int
        Tile grid row index (0-based).
    """

    data: NDArray
    x: int
    y: int
    tile_col: int
    tile_row: int


# ---------------------------------------------------------------------------
# Internal constants
# ---------------------------------------------------------------------------

_EXTENSION_TO_FORMAT: dict[str, str] = {
    ".ntf": "nitf",
    ".nitf": "nitf",
    ".tif": "geotiff",
    ".tiff": "geotiff",
    ".png": "png",
    ".j2k": "j2k",
    ".jp2": "j2k",
    ".jpg": "jpeg",
    ".jpeg": "jpeg",
    ".dt0": "dted",
    ".dt1": "dted",
    ".dt2": "dted",
    ".dt3": "dted",
    ".dt4": "dted",
    ".dt5": "dted",
    ".avg": "dted",
    ".min": "dted",
    ".max": "dted",
}

_SUPPORTED_DTYPES: dict[str, set[str]] = {
    "nitf": {"uint8", "uint16", "int16", "float32"},
    "geotiff": {"uint8", "uint16", "int16", "float32"},
    "png": {"uint8", "uint16"},
    "j2k": {"uint8", "uint16", "int16"},
    "jpeg": {"uint8"},
    "dted": {"int16"},
}


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _is_file_like(obj: object) -> bool:
    """Return True if *obj* is a file-like object (not a str or list).

    A file-like object is anything that is not a ``str`` or ``list`` and
    has a ``.read()`` or ``.write()`` attribute.
    """
    return not isinstance(obj, (str, list)) and (
        hasattr(obj, "read") or hasattr(obj, "write")
    )


def _resolve_asset_key(dataset, asset: str | None) -> str:
    """Resolve the image asset key to use for reading.

    Parameters
    ----------
    dataset : DatasetReader
        An open dataset reader.
    asset : str or None
        An explicit asset key, or ``None`` to auto-select.

    Returns
    -------
    str
        The resolved asset key.

    Raises
    ------
    ValueError
        If the requested asset key does not exist, or if the dataset
        contains no image assets at all.
    """
    from aws.osml.io import AssetType

    if asset is not None:
        # Verify the explicit key exists
        all_keys = dataset.get_asset_keys()
        if asset not in all_keys:
            raise ValueError(f"Asset '{asset}' not found in dataset")
        return asset

    # Try to find the first image asset with role "data"
    data_keys = dataset.get_asset_keys(asset_type=AssetType.Image, roles=["data"])
    if data_keys:
        return data_keys[0]

    # Fall back to the first image asset (any role)
    image_keys = dataset.get_asset_keys(asset_type=AssetType.Image)
    if image_keys:
        return image_keys[0]

    raise ValueError("No image assets found in dataset")


def _assemble_blocks(
    image_asset,
    window: tuple[int, int, int, int],
    bands: list[int] | None,
    resolution_level: int,
    fill_value: int | float,
) -> NDArray:
    """Read overlapping blocks and assemble into a CHW ndarray.

    Parameters
    ----------
    image_asset : ImageAssetProvider
        The image asset to read blocks from.
    window : tuple[int, int, int, int]
        The pixel region to read as ``(x, y, width, height)`` where
        *x* and *y* are column and row offsets of the top-left corner.
    bands : list[int] or None
        Zero-based band indices to decode, or ``None`` for all bands.
    resolution_level : int
        Resolution level for block decoding (0 = full resolution).
    fill_value : int or float
        Value used to fill regions where ``has_block()`` returns False.

    Returns
    -------
    numpy.ndarray
        Assembled pixel data in CHW layout ``(num_bands, height, width)``.
    """
    x, y, w, h = window

    # Image and block dimensions
    block_width = image_asset.num_pixels_per_block_horizontal
    block_height = image_asset.num_pixels_per_block_vertical
    img_width = image_asset.num_columns
    img_height = image_asset.num_rows

    # Handle non-blocked images (block size 0 means single block = full image)
    if block_width == 0:
        block_width = img_width
    if block_height == 0:
        block_height = img_height

    # Determine number of output bands
    num_bands = len(bands) if bands is not None else image_asset.num_bands

    # Determine output dtype from the image's pixel type
    dtype = np.dtype(image_asset.pixel_value_type.to_numpy_dtype())

    # Allocate output array, filled with fill_value
    output = np.full((num_bands, h, w), fill_value, dtype=dtype)

    # Compute which blocks overlap the requested window
    block_col_start = x // block_width
    block_col_end = (x + w - 1) // block_width + 1
    block_row_start = y // block_height
    block_row_end = (y + h - 1) // block_height + 1

    # Clamp to the actual block grid
    grid_rows, grid_cols = image_asset.block_grid_size
    block_col_end = min(block_col_end, grid_cols)
    block_row_end = min(block_row_end, grid_rows)

    for block_row in range(block_row_start, block_row_end):
        for block_col in range(block_col_start, block_col_end):
            # Skip masked (sparse) blocks — fill_value already in output
            if not image_asset.has_block(block_row, block_col, resolution_level):
                continue

            # Read the block
            block = image_asset.get_block(
                block_row, block_col, resolution_level, bands=bands
            )

            # Block's pixel coordinates in image space
            block_x_start = block_col * block_width
            block_y_start = block_row * block_height

            # Source region within the block (handles edge blocks that may
            # be smaller than the standard block size)
            src_x_start = max(0, x - block_x_start)
            src_y_start = max(0, y - block_y_start)
            src_x_end = min(block.shape[2], x + w - block_x_start)
            src_y_end = min(block.shape[1], y + h - block_y_start)

            # Destination region within the output array
            dst_x_start = max(0, block_x_start - x)
            dst_y_start = max(0, block_y_start - y)
            dst_x_end = dst_x_start + (src_x_end - src_x_start)
            dst_y_end = dst_y_start + (src_y_end - src_y_start)

            # Copy the overlapping region
            output[:, dst_y_start:dst_y_end, dst_x_start:dst_x_end] = (
                block[:, src_y_start:src_y_end, src_x_start:src_x_end]
            )

    return output


# ---------------------------------------------------------------------------
# Public API — imread
# ---------------------------------------------------------------------------


def imread(
    path: str | BinaryIO,
    *,
    window: tuple[int, int, int, int] | None = None,
    asset: str | None = None,
    bands: list[int] | None = None,
    resolution_level: int = 0,
    fill_value: int | float = 0,
    format: str | None = None,
) -> NDArray:
    """Read an image file (or a windowed region) as a NumPy array.

    Returns a CHW array with shape ``(bands, height, width)``.

    .. note::

       When reading from a stream, the entire content is loaded into memory
       via ``.read()``. For large files (multi-GB NITF), this is significantly
       more expensive than the memory-mapped file path. Consider downloading
       large files to the local filesystem, or using the VirtualiZarr-based
       tile index for cloud-native range-read access.

    Parameters
    ----------
    path : str or BinaryIO
        Path to the image file, or a file-like object containing image
        bytes.
    window : tuple[int, int, int, int] or None
        Optional pixel region to read as ``(x, y, width, height)`` where
        *x* and *y* are column and row offsets of the top-left corner.
        If ``None``, the entire image is read.
    asset : str or None
        Explicit asset key to read. If ``None``, the first image asset
        with role ``"data"`` is used (falling back to the first image
        asset of any role).
    bands : list[int] or None
        Zero-based band indices to decode. If ``None``, all bands are
        returned.
    resolution_level : int
        Resolution level for block decoding. ``0`` is full resolution;
        higher levels produce progressively smaller arrays (each level
        halves dimensions).
    fill_value : int or float
        Value used to fill regions where ``has_block()`` returns False.
        Defaults to ``0``.
    format : str or None
        Explicit format string (e.g. ``"png"``, ``"nitf"``). Required
        when reading from a stream. If ``None`` and *path* is a string,
        the format is inferred from the file extension.

    Returns
    -------
    numpy.ndarray
        Pixel data in CHW layout ``(bands, height, width)``.

    Raises
    ------
    IOError
        If the file does not exist or cannot be opened.
    ValueError
        If the specified asset key does not exist, if the dataset
        contains no image assets, if the window has zero or negative
        dimensions after clamping, or if *path* is a stream and
        *format* is not provided.
    """
    # Deferred import to avoid circular imports
    from aws.osml.io import IO

    if _is_file_like(path) and format is None:
        raise ValueError(
            "format is required when reading from a stream "
            "(e.g., format='png')"
        )

    open_args = (path, "r", format) if format is not None else (path, "r")

    with IO.open(*open_args) as dataset:
        # Resolve which asset to read
        asset_key = _resolve_asset_key(dataset, asset)
        image_asset = dataset.get_asset(asset_key)

        # Image dimensions at full resolution
        img_width = image_asset.num_columns
        img_height = image_asset.num_rows

        if window is None:
            # Full-image read
            read_window = (0, 0, img_width, img_height)
        else:
            # Windowed read — clamp to image bounds
            wx, wy, ww, wh = window
            x0 = max(0, wx)
            y0 = max(0, wy)
            x1 = min(img_width, wx + ww)
            y1 = min(img_height, wy + wh)

            clamped_w = x1 - x0
            clamped_h = y1 - y0

            if clamped_w <= 0 or clamped_h <= 0:
                raise ValueError(
                    "Window has zero or negative dimensions after clamping "
                    "to image bounds"
                )

            read_window = (x0, y0, clamped_w, clamped_h)

        # When both window and resolution_level > 0 are specified, scale
        # window coordinates to match the reduced resolution. Each level
        # halves dimensions, so we divide by 2^resolution_level.
        if resolution_level > 0:
            scale = 1 << resolution_level  # 2^resolution_level
            rx, ry, rw, rh = read_window
            read_window = (
                rx // scale,
                ry // scale,
                max(1, rw // scale),
                max(1, rh // scale),
            )

        return _assemble_blocks(
            image_asset,
            read_window,
            bands,
            resolution_level,
            fill_value,
        )


# ---------------------------------------------------------------------------
# Internal helpers — imsave
# ---------------------------------------------------------------------------

# Mapping from numpy dtype name to PixelType enum member name.
# Used by imsave() to convert the array's dtype to the Rust PixelType.
_DTYPE_TO_PIXEL_TYPE: dict[str, str] = {
    "uint8": "UInt8",
    "uint16": "UInt16",
    "int16": "Int16",
    "float32": "Float32",
}


def _resolve_format(path: str) -> str:
    """Infer the output format from the file extension.

    Parameters
    ----------
    path : str
        Output file path.

    Returns
    -------
    str
        Format string (e.g. ``"nitf"``, ``"geotiff"``).

    Raises
    ------
    ValueError
        If the extension is not recognized.
    """
    ext = os.path.splitext(path)[1].lower()
    fmt = _EXTENSION_TO_FORMAT.get(ext)
    if fmt is None:
        supported = ", ".join(sorted(_EXTENSION_TO_FORMAT.keys()))
        raise ValueError(
            f"Unsupported file extension '{ext}'. Supported: {supported}"
        )
    return fmt


def _validate_array(data: np.ndarray, fmt: str) -> np.ndarray:
    """Validate and normalise the input array for writing.

    - Rejects 0-D, 1-D, and >3-D arrays.
    - Rejects arrays with any zero-length dimension.
    - Reshapes 2-D ``(H, W)`` arrays to ``(1, H, W)``.
    - Validates the dtype against the target format.

    Parameters
    ----------
    data : numpy.ndarray
        Input pixel data.
    fmt : str
        Target format string (e.g. ``"nitf"``).

    Returns
    -------
    numpy.ndarray
        Array in CHW layout ``(bands, height, width)``.

    Raises
    ------
    ValueError
        If the array has invalid dimensions or an unsupported dtype.
    """
    if data.ndim < 2 or data.ndim > 3:
        raise ValueError(
            f"Expected a 2D (H, W) or 3D (C, H, W) array, got {data.ndim}D"
        )

    if any(s == 0 for s in data.shape):
        raise ValueError("Array dimensions must be positive")

    # Reshape 2D to 3D single-band
    if data.ndim == 2:
        data = data[np.newaxis, :, :]

    # Validate dtype
    dtype_name = data.dtype.name
    supported = _SUPPORTED_DTYPES.get(fmt, set())
    if dtype_name not in supported:
        supported_list = ", ".join(sorted(supported))
        raise ValueError(
            f"dtype '{dtype_name}' is not supported for {fmt} output. "
            f"Supported: {supported_list}"
        )

    return data


def _apply_format_defaults(
    metadata,
    fmt: str,
    bands: int,
    height: int,
    width: int,
    compression: str | None,
    block_size: tuple[int, int] | None,
    quality: float | None,
) -> tuple[int, int]:
    """Apply format-specific default compression and block size.

    Sets encoding hints on the ``BufferedMetadataProvider`` and returns
    the effective ``(block_width, block_height)`` to use when creating
    the image provider.

    Parameters
    ----------
    metadata : BufferedMetadataProvider
        Metadata provider to populate with encoding hints.
    fmt : str
        Target format string.
    bands : int
        Number of bands in the image.
    height : int
        Image height in pixels.
    width : int
        Image width in pixels.
    compression : str or None
        User-specified compression override, or ``None`` for format default.
    block_size : tuple[int, int] or None
        User-specified ``(block_width, block_height)`` override, or ``None``
        for format default.
    quality : float or None
        User-specified quality for lossy formats.

    Returns
    -------
    tuple[int, int]
        Effective ``(block_width, block_height)``.
    """
    if fmt == "nitf":
        return _apply_nitf_defaults(
            metadata, bands, height, width, compression, block_size, quality
        )
    elif fmt == "geotiff":
        return _apply_geotiff_defaults(
            metadata, height, width, compression, block_size
        )
    elif fmt == "png":
        return _apply_png_defaults(metadata, height, width, block_size)
    elif fmt == "j2k":
        return _apply_j2k_defaults(metadata, height, width, compression, block_size, quality)
    elif fmt == "jpeg":
        return _apply_jpeg_defaults(metadata, height, width, block_size, quality)
    else:
        # Fallback — should not happen given _resolve_format validation
        bw = width if block_size is None else block_size[0]
        bh = height if block_size is None else block_size[1]
        return (bw, bh)


def _apply_nitf_defaults(
    metadata,
    bands: int,
    height: int,
    width: int,
    compression: str | None,
    block_size: tuple[int, int] | None,
    quality: float | None,
) -> tuple[int, int]:
    """Apply NITF-specific encoding defaults."""
    # Default: JPEG 2000 lossless (IC=C8, IMODE=B)
    # Note: COMRAT is intentionally omitted for the default case — the
    # writer applies lossless J2K encoding when COMRAT is absent.
    if compression is None:
        metadata.set("IC", "C8")
    else:
        comp_lower = compression.lower()
        if comp_lower in ("jpeg2000", "j2k", "c8"):
            metadata.set("IC", "C8")
            if quality is not None:
                metadata.set("COMRAT", f"N{quality:05.1f}")
        elif comp_lower in ("jpeg", "c3"):
            metadata.set("IC", "C3")
            if quality is not None:
                metadata.set("COMRAT", f"{quality:04.1f}")
        elif comp_lower in ("none", "nc"):
            metadata.set("IC", "NC")
        else:
            metadata.set("IC", compression)

    metadata.set("IMODE", "B")

    # Block size: default 1024×1024 for NITF
    if block_size is not None:
        bw, bh = block_size
    else:
        bw, bh = (1024, 1024)

    return (bw, bh)


def _apply_geotiff_defaults(
    metadata,
    height: int,
    width: int,
    compression: str | None,
    block_size: tuple[int, int] | None,
) -> tuple[int, int]:
    """Apply GeoTIFF-specific encoding defaults."""
    from aws.osml.io.tiff.utils import TagNameResolver

    tag_dict = metadata.as_dict()
    resolver = TagNameResolver(tag_dict)

    # Block size: default 256×256 for GeoTIFF
    if block_size is not None:
        bw, bh = block_size
    else:
        bw, bh = (256, 256)

    resolver["TileWidth"] = bw
    resolver["TileLength"] = bh

    # Compression: default Deflate with horizontal predictor
    if compression is None:
        resolver["Compression"] = "Deflate"
        resolver["Predictor"] = 2  # Horizontal differencing
    else:
        comp_lower = compression.lower()
        if comp_lower == "deflate":
            resolver["Compression"] = "Deflate"
            resolver["Predictor"] = 2
        elif comp_lower == "lzw":
            resolver["Compression"] = "Lzw"
            resolver["Predictor"] = 2
        elif comp_lower == "none":
            resolver["Compression"] = "None"
        else:
            resolver["Compression"] = compression.capitalize()

    # Write resolved numeric keys back into the metadata provider
    for key, value in tag_dict.items():
        if isinstance(value, str):
            metadata.set(key, value)
        else:
            metadata.set_json(key, value)

    return (bw, bh)


def _apply_png_defaults(
    metadata,
    height: int,
    width: int,
    block_size: tuple[int, int] | None,
) -> tuple[int, int]:
    """Apply PNG-specific encoding defaults."""
    # PNG: full image as a single block (standard PNG Deflate)
    if block_size is not None:
        bw, bh = block_size
    else:
        bw, bh = (width, height)

    return (bw, bh)


def _apply_j2k_defaults(
    metadata,
    height: int,
    width: int,
    compression: str | None,
    block_size: tuple[int, int] | None,
    quality: float | None,
) -> tuple[int, int]:
    """Apply JPEG 2000 codestream encoding defaults."""
    # Default: lossless
    if compression is None or compression.lower() in ("lossless", "none"):
        metadata.set("J2K_LOSSLESS", "true")
    else:
        metadata.set("J2K_LOSSLESS", "false")

    if quality is not None:
        metadata.set("J2K_QUALITY", str(quality))

    # Block size: default 1024×1024
    if block_size is not None:
        bw, bh = block_size
    else:
        bw, bh = (1024, 1024)

    return (bw, bh)


def _apply_jpeg_defaults(
    metadata,
    height: int,
    width: int,
    block_size: tuple[int, int] | None,
    quality: float | None,
) -> tuple[int, int]:
    """Apply JPEG encoding defaults."""
    # Default quality: 75
    jpeg_quality = quality if quality is not None else 75
    metadata.set("JPEG_QUALITY", str(int(jpeg_quality)))

    # JPEG: full image as a single block
    if block_size is not None:
        bw, bh = block_size
    else:
        bw, bh = (width, height)

    return (bw, bh)


# ---------------------------------------------------------------------------
# Internal helpers — georeferencing
# ---------------------------------------------------------------------------

# Tolerance for axis-aligned detection (in coordinate units).
# Corners are considered axis-aligned if the top edge is horizontal
# (UL.lat == UR.lat) and the left edge is vertical (UL.lon == LL.lon)
# within this tolerance.
_AXIS_ALIGNED_TOL = 1e-10


def _is_axis_aligned(
    corners: list[tuple[float, float]],
) -> bool:
    """Check whether the four corners form an axis-aligned rectangle.

    An image is axis-aligned if the top edge is horizontal
    (UL.lat == UR.lat) and the left edge is vertical
    (UL.lon == LL.lon), within a small tolerance.

    Parameters
    ----------
    corners : list[tuple[float, float]]
        Four ``(lon, lat)`` pairs in order UL, UR, LR, LL.

    Returns
    -------
    bool
        ``True`` if the corners are axis-aligned.
    """
    ul_lon, ul_lat = corners[0]
    ur_lon, ur_lat = corners[1]
    _lr_lon, _lr_lat = corners[2]
    ll_lon, ll_lat = corners[3]

    top_horizontal = abs(ul_lat - ur_lat) < _AXIS_ALIGNED_TOL
    left_vertical = abs(ul_lon - ll_lon) < _AXIS_ALIGNED_TOL
    return top_horizontal and left_vertical


def _apply_nitf_georef(
    metadata,
    corners: list[tuple[float, float]],
    crs: str,
) -> None:
    """Apply NITF georeferencing metadata.

    Sets ``ICORDS`` based on the CRS and formats the corner coordinates
    as a 60-character ``IGEOLO`` string.

    Parameters
    ----------
    metadata : BufferedMetadataProvider
        Metadata provider to populate.
    corners : list[tuple[float, float]]
        Four ``(lon, lat)`` pairs in order UL, UR, LR, LL.
    crs : str
        CRS identifier (e.g. ``"EPSG:4326"``, ``"EPSG:32618"``).
    """
    from aws.osml.io.jbp.utils import IGEOLOAdapter

    icords = _crs_to_icords(crs)
    metadata.set("ICORDS", icords)

    if icords == "G":
        # Geographic: corners are (lon, lat), IGEOLO expects (lat, lon)
        latlon_corners = [(lat, lon) for lon, lat in corners]
        igeolo = IGEOLOAdapter.format(latlon_corners, "G")
        metadata.set("IGEOLO", igeolo)
    elif icords == "D":
        # Decimal degrees: corners are (lon, lat), IGEOLO expects (lat, lon)
        latlon_corners = [(lat, lon) for lon, lat in corners]
        igeolo = IGEOLOAdapter.format(latlon_corners, "D")
        metadata.set("IGEOLO", igeolo)


def _crs_to_icords(crs: str) -> str:
    """Map a CRS identifier to the NITF ICORDS value.

    Parameters
    ----------
    crs : str
        CRS identifier (e.g. ``"EPSG:4326"``, ``"EPSG:32618"``).

    Returns
    -------
    str
        ICORDS value: ``"G"`` for geographic, ``"N"`` for UTM North,
        ``"S"`` for UTM South.
    """
    crs_upper = crs.upper()

    # Parse EPSG code
    if crs_upper.startswith("EPSG:"):
        try:
            epsg = int(crs_upper.split(":")[1])
        except (ValueError, IndexError):
            return "G"

        # EPSG:4326 = WGS 84 geographic
        if epsg == 4326:
            return "G"

        # UTM zones: EPSG 326xx = North, EPSG 327xx = South
        if 32601 <= epsg <= 32660:
            return "N"
        if 32701 <= epsg <= 32760:
            return "S"

    # Default to geographic
    return "G"


def _apply_geotiff_georef(
    metadata,
    corners: list[tuple[float, float]],
    crs: str,
    width: int,
    height: int,
) -> None:
    """Apply GeoTIFF georeferencing metadata.

    For axis-aligned images, sets ``ModelPixelScale`` (tag 33550) and
    ``ModelTiepoint`` (tag 33922). For rotated/skewed images, sets
    ``ModelTransformation`` (tag 34264). Also builds the GeoKey
    directory (tag 34735) with model type, raster type, and CRS.

    Parameters
    ----------
    metadata : BufferedMetadataProvider
        Metadata provider to populate.
    corners : list[tuple[float, float]]
        Four ``(lon, lat)`` pairs in order UL, UR, LR, LL.
    crs : str
        CRS identifier (e.g. ``"EPSG:4326"``, ``"EPSG:32618"``).
    width : int
        Image width in pixels.
    height : int
        Image height in pixels.
    """
    ul_lon, ul_lat = corners[0]
    ur_lon, ur_lat = corners[1]
    lr_lon, lr_lat = corners[2]
    ll_lon, ll_lat = corners[3]

    if _is_axis_aligned(corners):
        # Axis-aligned: use ModelPixelScale + ModelTiepoint
        pixel_width = (ur_lon - ul_lon) / width
        pixel_height = (ul_lat - ll_lat) / height

        # ModelPixelScaleTag (33550): [scale_x, scale_y, scale_z]
        metadata.set_json("33550", [abs(pixel_width), abs(pixel_height), 0.0])

        # ModelTiepointTag (33922): [pixel_x, pixel_y, pixel_z, geo_x, geo_y, geo_z]
        # Pixel (0, 0) maps to the UL corner
        metadata.set_json("33922", [0.0, 0.0, 0.0, ul_lon, ul_lat, 0.0])
    else:
        # Rotated/skewed: use ModelTransformation (4×4 affine matrix)
        # The affine maps pixel (col, row) to geographic (x, y):
        #   x = a * col + b * row + tx
        #   y = d * col + e * row + ty
        #
        # Using the UL, UR, LL corners to compute the affine:
        #   UL = (0, 0) → (ul_lon, ul_lat)
        #   UR = (width, 0) → (ur_lon, ur_lat)
        #   LL = (0, height) → (ll_lon, ll_lat)
        a = (ur_lon - ul_lon) / width
        b = (ll_lon - ul_lon) / height
        tx = ul_lon
        d = (ur_lat - ul_lat) / width
        e = (ll_lat - ul_lat) / height
        ty = ul_lat

        # 4×4 row-major transformation matrix
        # [a,  b,  0, tx]
        # [d,  e,  0, ty]
        # [0,  0,  0,  0]
        # [0,  0,  0,  1]
        transform = [
            a, b, 0.0, tx,
            d, e, 0.0, ty,
            0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]
        metadata.set_json("34264", transform)

    # Build GeoKey directory
    _build_geokey_directory(metadata, crs)


def _build_geokey_directory(metadata, crs: str) -> None:
    """Build and set the GeoKeyDirectory (tag 34735) for the given CRS.

    Sets ``GeoModelType`` (key 1024), ``GeoRasterType`` (key 1025),
    and the appropriate CRS key (``ProjectedCSTypeGeoKey`` 3072 or
    ``GeographicTypeGeoKey`` 2048).

    Parameters
    ----------
    metadata : BufferedMetadataProvider
        Metadata provider to populate.
    crs : str
        CRS identifier (e.g. ``"EPSG:4326"``, ``"EPSG:32618"``).
    """
    crs_upper = crs.upper()
    epsg = 4326  # default
    is_projected = False

    if crs_upper.startswith("EPSG:"):
        try:
            epsg = int(crs_upper.split(":")[1])
        except (ValueError, IndexError):
            # Malformed EPSG code; fall through to default geographic
            pass

    # Determine if projected or geographic
    # UTM zones (326xx, 327xx) and other projected CRS codes > 32600
    if 32601 <= epsg <= 32760:
        is_projected = True
    elif epsg == 4326:
        is_projected = False
    elif epsg > 2000:
        # Heuristic: most EPSG codes > 2000 that aren't 4326 are projected
        # This is a simplification; a full CRS database would be more accurate
        is_projected = True

    # GeoModelType: 1 = Projected, 2 = Geographic
    model_type = 1 if is_projected else 2

    # GeoRasterType: 1 = RasterPixelIsArea
    raster_type = 1

    # Build the GeoKey directory as a flat array of u16 values
    # Header: [version=1, revision=1, minor_revision=1, num_keys=N]
    # Each key: [key_id, tiff_tag_location, count, value_offset]
    # tiff_tag_location=0 means the value is inline in value_offset
    keys = []

    # GTModelTypeGeoKey (1024) = model_type
    keys.extend([1024, 0, 1, model_type])

    # GTRasterTypeGeoKey (1025) = raster_type
    keys.extend([1025, 0, 1, raster_type])

    if is_projected:
        # ProjectedCSTypeGeoKey (3072) = EPSG code
        keys.extend([3072, 0, 1, epsg])
    else:
        # GeographicTypeGeoKey (2048) = EPSG code
        keys.extend([2048, 0, 1, epsg])

    num_keys = len(keys) // 4
    header = [1, 1, 1, num_keys]
    directory = header + keys

    metadata.set_json("34735", directory)


# ---------------------------------------------------------------------------
# Public API — imsave
# ---------------------------------------------------------------------------


def imsave(
    path: str | BinaryIO,
    data: np.ndarray,
    *,
    compression: str | None = None,
    block_size: tuple[int, int] | None = None,
    corners: list[tuple[float, float]] | None = None,
    crs: str | None = None,
    quality: float | None = None,
    format: str | None = None,
) -> None:
    """Save a NumPy array to an image file.

    The array should be in CHW layout ``(bands, height, width)``.
    2-D arrays ``(height, width)`` are treated as single-band images
    by reshaping to ``(1, height, width)`` before writing.

    The output format is inferred from the file extension when *path*
    is a string and *format* is not provided. When *path* is a
    file-like object, *format* must be specified explicitly.

    Parameters
    ----------
    path : str or BinaryIO
        Output file path or a writable file-like object. When a string,
        the extension determines the format (e.g. ``.ntf`` → NITF,
        ``.tif`` → GeoTIFF, ``.png`` → PNG, ``.j2k`` → JPEG 2000,
        ``.jpg`` → JPEG).
    data : numpy.ndarray
        Pixel data in CHW layout ``(bands, height, width)`` or a 2-D
        array ``(height, width)`` for single-band images.
    compression : str or None
        Compression algorithm override. If ``None``, a sensible default
        is chosen for the format (e.g. Deflate for GeoTIFF, JPEG 2000
        lossless for NITF).
    block_size : tuple[int, int] or None
        Block dimensions as ``(width, height)`` override. If ``None``,
        a format-appropriate default is used (e.g. 256×256 for GeoTIFF,
        1024×1024 for NITF, full image for PNG/JPEG).
    corners : list[tuple[float, float]] or None
        Four geographic corner coordinates as
        ``[(lon, lat), ...]`` in order UL, UR, LR, LL. Used for
        georeferencing (NITF IGEOLO, GeoTIFF tiepoints). Silently
        ignored for formats that do not support georeferencing.
    crs : str or None
        Coordinate reference system identifier (e.g. ``"EPSG:4326"``).
        Used together with *corners* for georeferencing.
    quality : float or None
        Quality parameter for lossy formats (e.g. JPEG quality 0–100,
        JPEG 2000 compression ratio).
    format : str or None
        Explicit format string (e.g. ``"png"``, ``"nitf"``). Required
        when writing to a stream. If ``None`` and *path* is a string,
        the format is inferred from the file extension.

    Raises
    ------
    ValueError
        If the file extension is not recognized, the array dtype is
        unsupported for the target format, the array has invalid
        dimensions (0-D, 1-D, or >3-D), the array is empty, or
        *path* is a stream and *format* is not provided.
    """
    from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType

    # 1. Determine format
    is_stream = _is_file_like(path)
    if is_stream:
        if format is None:
            raise ValueError(
                "format is required when writing to a stream "
                "(e.g., format='png')"
            )
        fmt = format
    else:
        fmt = _resolve_format(path) if format is None else format

    # The IO library uses "tiff" as the format string for GeoTIFF
    io_format = "tiff" if fmt == "geotiff" else fmt

    # 2. Validate and normalise the array
    data = _validate_array(data, fmt)

    bands, height, width = data.shape
    dtype_name = data.dtype.name

    # 3. Map numpy dtype to PixelType enum
    pixel_type_name = _DTYPE_TO_PIXEL_TYPE[dtype_name]
    pixel_type = getattr(PixelType, pixel_type_name)

    # 4. Create metadata provider and apply format defaults
    metadata = BufferedMetadataProvider()
    bw, bh = _apply_format_defaults(
        metadata, fmt, bands, height, width, compression, block_size, quality
    )

    # Clamp block size to image dimensions — some format writers
    # (e.g. NITF) require blocks not to exceed the image size.
    bw = min(bw, width)
    bh = min(bh, height)

    # 5. Apply georeferencing when corners and crs are provided
    if corners is not None and crs is not None:
        if fmt == "nitf":
            _apply_nitf_georef(metadata, corners, crs)
        elif fmt == "geotiff":
            _apply_geotiff_georef(metadata, corners, crs, width, height)
        # PNG, JPEG, J2K: silently ignore georeferencing

    # 6. Create BufferedImageAssetProvider with pixel data
    image_provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=width,
        num_rows=height,
        num_bands=bands,
        block_width=bw,
        block_height=bh,
        pixel_type=pixel_type,
        metadata=metadata,
        title="Image",
        description=f"{width}x{height} {bands}-band {dtype_name}",
    )

    # Ensure the array is contiguous in memory before passing to Rust
    data = np.ascontiguousarray(data)
    image_provider.set_full_image(data)

    # 7. Write via IO.open
    with IO.open(path, "w", io_format) as writer:
        # Set dataset-level metadata for formats that need it
        if io_format in ("tiff", "j2k", "jpeg"):
            writer.metadata = metadata

        writer.add_asset(
            key="image:0",
            provider=image_provider,
            title="Image",
            description=f"{width}x{height} {bands}-band {dtype_name}",
            roles=["data"],
        )


# ---------------------------------------------------------------------------
# Public API — iminfo
# ---------------------------------------------------------------------------


def iminfo(
    path: str | BinaryIO,
    *,
    asset: str | None = None,
    format: str | None = None,
) -> ImageInfo:
    """Get image metadata without reading pixel data.

    Returns an :class:`ImageInfo` object with image dimensions, band
    count, pixel type, block layout, resolution level count, and the
    full format-specific metadata dictionary for the image segment.

    Parameters
    ----------
    path : str or BinaryIO
        Path to the image file, or a file-like object containing image
        bytes.
    asset : str or None
        Explicit asset key to inspect. If ``None``, the first image
        asset with role ``"data"`` is used (falling back to the first
        image asset of any role).
    format : str or None
        Explicit format string (e.g. ``"png"``, ``"nitf"``). Required
        when reading from a stream. If ``None`` and *path* is a string,
        the format is inferred from the file extension.

    Returns
    -------
    ImageInfo
        Read-only metadata for the resolved image asset. The
        :attr:`~ImageInfo.metadata` attribute contains the full
        format-specific metadata dictionary (NITF subheader fields
        and TREs, or TIFF IFD tags).

    Raises
    ------
    IOError
        If the file does not exist or cannot be opened.
    ValueError
        If the specified asset key does not exist, if the dataset
        contains no image assets, or if *path* is a stream and
        *format* is not provided.
    """
    from aws.osml.io import IO

    if _is_file_like(path) and format is None:
        raise ValueError(
            "format is required when reading from a stream "
            "(e.g., format='png')"
        )

    open_args = (path, "r", format) if format is not None else (path, "r")

    with IO.open(*open_args) as dataset:
        asset_key = _resolve_asset_key(dataset, asset)
        image_asset = dataset.get_asset(asset_key)

        return ImageInfo(
            width=image_asset.num_columns,
            height=image_asset.num_rows,
            bands=image_asset.num_bands,
            dtype=image_asset.pixel_value_type.to_numpy_dtype(),
            block_size=(
                image_asset.num_pixels_per_block_horizontal,
                image_asset.num_pixels_per_block_vertical,
            ),
            num_resolution_levels=image_asset.num_resolution_levels,
            asset_key=asset_key,
            metadata=dict(image_asset.get_metadata().as_dict()),
        )


# ---------------------------------------------------------------------------
# Public API — tiles
# ---------------------------------------------------------------------------


def tiles(
    path: str | BinaryIO,
    tile_size: tuple[int, int],
    *,
    overlap: tuple[int, int] = (0, 0),
    asset: str | None = None,
    bands: list[int] | None = None,
    resolution_level: int = 0,
    fill_value: int | float = 0,
    format: str | None = None,
) -> Iterator[Tile]:
    """Iterate over fixed-size tiles of an image.

    Yields :class:`Tile` objects in row-major order (left-to-right,
    top-to-bottom). Edge tiles may be smaller than *tile_size*.

    The dataset is kept open for the lifetime of the generator. The
    caller should consume or close the generator to release the
    underlying file handle.

    Parameters
    ----------
    path : str or BinaryIO
        Path to the image file, or a file-like object containing image
        bytes.
    tile_size : tuple[int, int]
        Tile dimensions as ``(width, height)``.
    overlap : tuple[int, int]
        Horizontal and vertical overlap in pixels as
        ``(overlap_width, overlap_height)``. Defaults to ``(0, 0)``.
    asset : str or None
        Explicit asset key to read. If ``None``, the first image asset
        with role ``"data"`` is used (falling back to the first image
        asset of any role).
    bands : list[int] or None
        Zero-based band indices to decode. If ``None``, all bands are
        returned.
    resolution_level : int
        Resolution level for block decoding. ``0`` is full resolution.
    fill_value : int or float
        Value used to fill regions where ``has_block()`` returns False.
        Defaults to ``0``.
    format : str or None
        Explicit format string (e.g. ``"png"``, ``"nitf"``). Required
        when reading from a stream. If ``None`` and *path* is a string,
        the format is inferred from the file extension.

    Yields
    ------
    Tile
        A tile of pixel data with position and grid coordinates.

    Raises
    ------
    IOError
        If the file does not exist or cannot be opened.
    ValueError
        If *tile_size* has non-positive dimensions, if *overlap* is
        greater than or equal to *tile_size* in either dimension, if
        the specified asset key does not exist, or if *path* is a
        stream and *format* is not provided.
    """
    from aws.osml.io import IO

    if _is_file_like(path) and format is None:
        raise ValueError(
            "format is required when reading from a stream "
            "(e.g., format='png')"
        )

    tile_w, tile_h = tile_size
    overlap_w, overlap_h = overlap

    # Validate tile_size: both dimensions must be positive
    if tile_w <= 0 or tile_h <= 0:
        raise ValueError("tile_size dimensions must be positive")

    # Validate overlap: must be less than tile_size in both dimensions
    if overlap_w >= tile_w or overlap_h >= tile_h:
        raise ValueError(
            "overlap must be less than tile_size in both dimensions"
        )

    # Compute stride
    stride_w = tile_w - overlap_w
    stride_h = tile_h - overlap_h

    open_args = (path, "r", format) if format is not None else (path, "r")

    with IO.open(*open_args) as dataset:
        # Resolve which asset to read
        asset_key = _resolve_asset_key(dataset, asset)
        image_asset = dataset.get_asset(asset_key)

        # Image dimensions
        img_width = image_asset.num_columns
        img_height = image_asset.num_rows

        # Compute tile grid dimensions using ceiling division
        num_tile_cols = math.ceil((img_width - overlap_w) / stride_w)
        num_tile_rows = math.ceil((img_height - overlap_h) / stride_h)

        # Iterate in row-major order
        for tile_row in range(num_tile_rows):
            for tile_col in range(num_tile_cols):
                # Compute the tile's pixel window origin
                x = tile_col * stride_w
                y = tile_row * stride_h

                # Clamp tile dimensions to image bounds for edge tiles
                w = min(tile_w, img_width - x)
                h = min(tile_h, img_height - y)

                # Read the tile region using the shared block-assembly helper
                data = _assemble_blocks(
                    image_asset,
                    (x, y, w, h),
                    bands,
                    resolution_level,
                    fill_value,
                )

                yield Tile(
                    data=data,
                    x=x,
                    y=y,
                    tile_col=tile_col,
                    tile_row=tile_row,
                )
