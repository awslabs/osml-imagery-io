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


def _are_contiguous(ranges: list[tuple[int, int]]) -> bool:
    """Check if a list of (offset, length) ranges are contiguous in the file."""
    for i in range(len(ranges) - 1):
        if ranges[i][0] + ranges[i][1] != ranges[i + 1][0]:
            return False
    return True


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
        multi_range_refs: dict[str, list] = {}

        with IO.open([self.local_path], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)

            for key in keys:
                asset = reader.get_asset(key)
                byte_ranges = asset.tile_byte_ranges()
                if byte_ranges is None:
                    continue

                # Build chunk manifest entries
                entries: dict[str, ChunkEntry] = {}
                for (row, col), range_list in byte_ranges.items():
                    chunk_key = f"0.{row}.{col}"
                    if len(range_list) == 1:
                        offset, length = range_list[0]
                        entries[chunk_key] = ChunkEntry(
                            path=url, offset=offset, length=length
                        )
                    elif _are_contiguous(range_list):
                        offset = range_list[0][0]
                        length = sum(l for _, l in range_list)
                        entries[chunk_key] = ChunkEntry(
                            path=url, offset=offset, length=length
                        )
                    else:
                        # Non-contiguous — placeholder entry + multi-range ref
                        offset, length = range_list[0]
                        entries[chunk_key] = ChunkEntry(
                            path=url, offset=offset, length=length
                        )
                        multi_range_refs[f"{key}/{chunk_key}"] = [
                            url, [[o, l] for o, l in range_list]
                        ]

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
        store = ManifestStore(group=group)
        store.multi_range_refs = multi_range_refs
        return store


# ---------------------------------------------------------------------------
# Tile index serialization with multi-range support
# ---------------------------------------------------------------------------


def _patch_multi_range_refs(refs: dict, multi_range_refs: dict) -> dict:
    """Replace placeholder single-range entries with multi-range entries.

    For each key in *multi_range_refs*, the corresponding entry in *refs*
    is replaced with the multi-range form ``["url", [[offset, length], ...]]``.
    Single-range entries not in *multi_range_refs* are left unchanged.
    """
    if not multi_range_refs:
        return refs
    patched = dict(refs)
    patched.update(multi_range_refs)
    return patched


def write_tile_index(store, output: str, segments: list[str] | None = None) -> None:
    """Write a tile index to JSON or Parquet with multi-range support.

    This is the recommended way to serialize a ``ManifestStore`` produced by
    :class:`OversightMLParser`.  It handles the multi-range reference entries
    that VirtualiZarr's built-in serialization does not support.

    Parameters
    ----------
    store : ManifestStore
        The manifest store returned by ``OversightMLParser()``.
    output : str
        Output file path.  Extension determines format: ``.json`` for
        Kerchunk JSON, ``.parquet`` for Kerchunk Parquet.
    segments : list[str], optional
        Image segment keys to include.  If ``None``, all segments are
        included.  Use this when the file contains segments with different
        image dimensions (which cannot be merged into a single xarray
        Dataset).

    Raises
    ------
    ValueError
        If the output extension is not ``.json`` or ``.parquet``, or if
        a requested segment is not found.

    Examples
    --------
    ::

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        parser = OversightMLParser(local_path="local/image.ntf")
        store = parser(url="s3://my-bucket/imagery/image.ntf")
        write_tile_index(store, "image.tile_index.json")

        # Or index only specific segments
        write_tile_index(store, "image.tile_index.json", segments=["image_segment_0"])
    """
    import json
    from pathlib import Path

    from virtualizarr.accessor import dataset_to_kerchunk_refs
    from virtualizarr.manifests import ManifestGroup, ManifestStore

    ext = Path(output).suffix.lower()
    multi_range_refs = getattr(store, "multi_range_refs", {}) or {}

    # Filter segments if requested
    if segments:
        group = store._group
        available = list(group.arrays.keys())
        missing = [s for s in segments if s not in group.arrays]
        if missing:
            raise ValueError(
                f"Segment(s) not found: {', '.join(missing)}. "
                f"Available: {', '.join(available)}"
            )
        filtered_arrays = {k: v for k, v in group.arrays.items() if k in segments}
        attrs = group.metadata.attributes if group.metadata else None
        store = ManifestStore(
            group=ManifestGroup(arrays=filtered_arrays, attributes=attrs)
        )
        multi_range_refs = {
            k: v for k, v in multi_range_refs.items()
            if any(k.startswith(seg + "/") for seg in segments)
        }

    vds = store.to_virtual_dataset()

    if ext == ".json":
        kerchunk = dataset_to_kerchunk_refs(vds)
        if "refs" in kerchunk:
            kerchunk["refs"] = _patch_multi_range_refs(
                kerchunk["refs"], multi_range_refs
            )
        else:
            kerchunk = _patch_multi_range_refs(kerchunk, multi_range_refs)
        with open(output, "w") as f:
            json.dump(kerchunk, f)

    elif ext == ".parquet":
        import fsspec
        from fsspec.implementations.reference import LazyReferenceMapper

        refs = dataset_to_kerchunk_refs(vds)
        if "refs" in refs:
            refs = refs["refs"]
        if multi_range_refs:
            refs = _patch_multi_range_refs(refs, multi_range_refs)

        fs, _ = fsspec.core.url_to_fs(output)
        out = LazyReferenceMapper.create(
            record_size=100_000,
            root=output,
            fs=fs,
            engine="pyarrow",
        )
        for k in sorted(refs):
            out[k] = refs[k]
        out.flush()

    else:
        raise ValueError(
            f"Unsupported output extension '{ext}'. Use .json or .parquet"
        )
