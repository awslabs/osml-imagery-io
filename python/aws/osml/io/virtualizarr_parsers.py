"""VirtualiZarr parser for imagery formats supported by IO.open().

This module provides :class:`OversightMLParser`, a single VirtualiZarr parser
that works for any imagery format the library supports: NITF (2.0, 2.1,
NSIF 1.0, SICD, SIDD), standalone JPEG 2000 (.j2k, .jp2), TIFF, and
GeoTIFF.  Format detection is handled by ``IO.open()`` — the parser itself
is format-agnostic.

VirtualiZarr is a lazy dependency — importing this module when ``virtualizarr``
is not installed raises :class:`ImportError` with install instructions.

Usage::

    from aws.osml.io.virtualizarr_parsers import OversightMLParser

    parser = OversightMLParser(local_path="/data/image.ntf")
    manifest_store = parser(url="s3://bucket/image.ntf")
"""

from __future__ import annotations

import base64
from typing import TYPE_CHECKING

from aws.osml.io._io import IO, AssetType

if TYPE_CHECKING:
    pass


# ---------------------------------------------------------------------------
# Lazy import helpers
# ---------------------------------------------------------------------------


def _import_virtualizarr():
    """Lazily import virtualizarr, raising ImportError with install instructions."""
    try:
        import virtualizarr  # noqa: F401

        return virtualizarr
    except ImportError:
        raise ImportError(
            "virtualizarr>=2.0 is required for parser support. "
            "Install with: pip install osml-imagery-io[virtualizarr]"
        )


# ---------------------------------------------------------------------------
# PixelType → zarr v3 ZDType mapping
# ---------------------------------------------------------------------------


def _pixel_type_to_zdtype(pixel_type):
    """Convert a PixelType to the corresponding zarr v3 ZDType instance.

    Uses the numpy dtype string as an intermediary since the PyO3 PixelType
    enum is not hashable.
    """
    import numpy as np
    from zarr.core.dtype import data_type_registry

    np_dtype = np.dtype(pixel_type.to_numpy_dtype())
    return data_type_registry.match_dtype(dtype=np_dtype)


# ---------------------------------------------------------------------------
# Codec configuration → codec instance mapping
# ---------------------------------------------------------------------------


def _build_codec_instance(asset):
    """Map an ImageAssetProvider's codec_configuration() to a codec instance.

    Returns ``None`` when the asset has no codec configuration (e.g. TIFF
    segments where ``codec_configuration()`` returns ``None``).

    The mapping logic mirrors ``tile_index.py:_build_zarray()`` but produces
    zarr v3 codec class instances instead of zarr v2 filter dicts.
    """
    from aws.osml.io.zarr_codecs import JbpBlockCodec, Jpeg2000Codec, JpegCodec

    codec_config = asset.codec_configuration()
    if codec_config is None:
        return None

    # Normalize raw values from the Rust side
    raw: dict = {}
    for key, value in codec_config.items():
        if key == "main_header":
            raw[key] = base64.b64encode(value).decode("ascii")
        elif isinstance(value, (bytes, bytearray)) and len(value) == 1:
            raw[key] = value[0]
        elif isinstance(value, (bytes, bytearray)):
            try:
                raw[key] = value.decode("ascii")
            except UnicodeDecodeError:
                raw[key] = base64.b64encode(value).decode("ascii")
        else:
            raw[key] = value

    num_bands = asset.num_bands
    block_h = asset.num_pixels_per_block_vertical
    block_w = asset.num_pixels_per_block_horizontal

    if "main_header" in raw:
        # JPEG 2000
        return Jpeg2000Codec(
            main_header=raw["main_header"],
            resolution_level=0,
        )
    elif "color_space" in raw:
        # JPEG
        imode_raw = raw.get("imode", 66)
        imode = chr(imode_raw) if isinstance(imode_raw, int) else str(imode_raw)
        cs_raw = raw.get("color_space", 0)
        cs_map = {0: "MONO", 1: "YCbCr601", 2: "RGB"}
        color_space = cs_map.get(cs_raw, "MONO") if isinstance(cs_raw, int) else str(cs_raw)
        return JpegCodec(
            bits_per_pixel=raw.get("bits_per_pixel", asset.num_bits_per_pixel),
            num_bands=num_bands,
            block_width=block_w,
            block_height=block_h,
            imode=imode,
            color_space=color_space,
        )
    elif "pvtype" in raw:
        # Uncompressed JBP block
        imode_raw = raw.get("imode", 66)
        imode = chr(imode_raw) if isinstance(imode_raw, int) else str(imode_raw)
        nbpp = raw.get("nbpp", raw.get("abpp", asset.num_bits_per_pixel))
        if isinstance(nbpp, str):
            nbpp = int(nbpp)
        return JbpBlockCodec(
            num_bands=num_bands,
            block_height=block_h,
            block_width=block_w,
            nbpp=nbpp,
            imode=imode,
            pvtype=raw["pvtype"],
        )

    return None


# ---------------------------------------------------------------------------
# OversightMLParser
# ---------------------------------------------------------------------------


class OversightMLParser:
    """VirtualiZarr parser for any imagery format supported by IO.open().

    Supports NITF (2.0, 2.1, NSIF 1.0, SICD, SIDD), standalone JPEG 2000
    (.j2k, .jp2), TIFF, and GeoTIFF.  Format detection is handled by
    ``IO.open()`` — the parser itself is format-agnostic.

    Parameters
    ----------
    local_path : str
        Path to the local imagery file to scan.

    Examples
    --------
    ::

        parser = OversightMLParser(local_path="/data/image.ntf")
        manifest_store = parser(url="s3://bucket/image.ntf")

        parser = OversightMLParser(local_path="/data/image.tif")
        manifest_store = parser(url="s3://bucket/image.tif")
    """

    def __init__(self, local_path: str):
        self.local_path = local_path

    def __call__(self, url: str, registry=None, **kwargs):
        """Scan the local file and build a ManifestStore.

        Parameters
        ----------
        url : str
            Cloud URI written into all chunk references.
        registry : optional
            Object store registry (accepted for protocol conformance, ignored).

        Returns
        -------
        ManifestStore
            Virtual Zarr store with chunk references into *url*.

        Raises
        ------
        ValueError
            If the file contains no indexable image segments.
        """
        _import_virtualizarr()

        from virtualizarr.manifests import (
            ChunkEntry,
            ChunkManifest,
            ManifestArray,
            ManifestGroup,
            ManifestStore,
        )
        from zarr.codecs import BytesCodec
        from zarr.core.chunk_grids import RegularChunkGrid
        from zarr.core.metadata.v3 import ArrayV3Metadata

        arrays: dict[str, ManifestArray] = {}

        with IO.open([self.local_path], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)

            for key in keys:
                asset = reader.get_asset(key)
                byte_ranges = asset.tile_byte_ranges()
                if byte_ranges is None:
                    continue

                # Build chunk manifest entries
                entries: dict[str, ChunkEntry] = {}
                for (row, col), (offset, length) in byte_ranges.items():
                    chunk_key = f"0.{row}.{col}"
                    entries[chunk_key] = ChunkEntry(
                        path=url, offset=offset, length=length
                    )

                if not entries:
                    continue

                # Compute grid shape
                max_row = max(r for (r, _) in byte_ranges.keys()) + 1
                max_col = max(c for (_, c) in byte_ranges.keys()) + 1
                grid_shape = (1, max_row, max_col)

                chunk_manifest = ChunkManifest(entries=entries, shape=grid_shape)

                # Build metadata
                zdtype = _pixel_type_to_zdtype(asset.pixel_value_type)

                num_bands = asset.num_bands
                block_h = asset.num_pixels_per_block_vertical
                block_w = asset.num_pixels_per_block_horizontal

                # Build codecs list — BytesCodec is required as ArrayBytesCodec
                codecs = [BytesCodec()]
                custom_codec = _build_codec_instance(asset)
                if custom_codec is not None:
                    codecs.append(custom_codec)

                metadata = ArrayV3Metadata(
                    shape=(num_bands, asset.num_rows, asset.num_columns),
                    data_type=zdtype,
                    chunk_grid=RegularChunkGrid(
                        chunk_shape=(num_bands, block_h, block_w)
                    ),
                    chunk_key_encoding={"name": "default", "separator": "."},
                    fill_value=0,
                    codecs=codecs,
                    attributes={},
                    dimension_names=["bands", "y", "x"],
                )

                arrays[key] = ManifestArray(
                    metadata=metadata, chunkmanifest=chunk_manifest
                )

        if not arrays:
            raise ValueError(
                f"No indexable image segments found in {self.local_path}"
            )

        group = ManifestGroup(
            arrays=arrays, attributes={"source": url}
        )
        return ManifestStore(group=group)

