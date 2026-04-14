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

    parser = OversightMLParser(local_paths="/data/image.ntf")
    manifest_store = parser(url="s3://bucket/image.ntf")
"""

from __future__ import annotations

import base64
import re
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


OVERVIEW_PATTERN = re.compile(r"^(image:\d+):overview:(\d+)$")


def _classify_assets(all_assets):
    """Classify assets into parent images and their overviews.

    Parameters
    ----------
    all_assets : list of (key, asset) tuples

    Returns
    -------
    parents : dict
        Mapping parent key (e.g. "image:0") to asset.
    overviews : dict
        Mapping parent key to list of (level, asset) tuples
        sorted by level number ascending.
    """
    parents = {}
    overviews = {}
    for key, asset in all_assets:
        m = OVERVIEW_PATTERN.match(key)
        if m:
            parent_key = m.group(1)
            level = int(m.group(2))
            overviews.setdefault(parent_key, []).append((level, asset))
        elif key.startswith("image:"):
            parents[key] = asset

    # Sort overviews by level number
    for parent_key in overviews:
        overviews[parent_key].sort(key=lambda x: x[0])

    return parents, overviews


def _build_manifest_array(asset, url, multi_range_refs, key_prefix=""):
    """Build a ManifestArray from an ImageAssetProvider.

    Extracts chunk manifest entries (single-range, contiguous, multi-range),
    computes grid shape, constructs metadata, and builds codec instances.

    Parameters
    ----------
    asset : ImageAssetProvider
        The image asset to build an array for.
    url : str
        Cloud URI for chunk references.
    multi_range_refs : dict
        Accumulator for multi-range entries.  Non-contiguous tile byte
        ranges are added here with keys prefixed by *key_prefix*.
    key_prefix : str
        Prefix for ``multi_range_refs`` keys.  Use ``""`` for the flat
        (current) path and ``"0/data/"`` for hierarchical subgroups.

    Returns
    -------
    ManifestArray or None
        ``None`` when ``asset.tile_byte_ranges()`` returns ``None``.
    """
    from virtualizarr.manifests import ChunkEntry, ChunkManifest, ManifestArray
    from zarr.codecs import BytesCodec
    from zarr.core.chunk_grids import RegularChunkGrid
    from zarr.core.metadata.v3 import ArrayV3Metadata

    byte_ranges = asset.tile_byte_ranges()
    if byte_ranges is None:
        return None

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
            length = sum(ln for _, ln in range_list)
            entries[chunk_key] = ChunkEntry(
                path=url, offset=offset, length=length
            )
        else:
            # Non-contiguous — placeholder entry + multi-range ref
            offset, length = range_list[0]
            entries[chunk_key] = ChunkEntry(
                path=url, offset=offset, length=length
            )
            multi_range_refs[f"{key_prefix}{chunk_key}"] = [
                url, [[o, ln] for o, ln in range_list]
            ]

    if not entries:
        return None

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

    return ManifestArray(metadata=metadata, chunkmanifest=chunk_manifest)


GEOZARR_MULTISCALES_CONVENTION = {
    "uuid": "d35379db-88df-4056-af3a-620245f8e347",
    "schema_url": "https://raw.githubusercontent.com/zarr-conventions/multiscales/refs/tags/v1/schema.json",
    "spec_url": "https://github.com/zarr-conventions/multiscales/blob/v1/README.md",
    "name": "multiscales",
    "description": "Multiscale layout of zarr datasets",
}


def _build_multiscale_group(levels, source_url, multi_range_refs, downsampling_method=None):
    """Build a hierarchical ManifestGroup with GeoZarr multiscales metadata.

    Produces root group attributes conforming to the GeoZarr multiscales
    convention (UUID ``d35379db-88df-4056-af3a-620245f8e347``).  The
    ``multiscales`` attribute is a dict containing a ``layout`` array that
    describes each resolution level with ``asset``, ``derived_from``, and
    ``transform`` fields.  A ``zarr_conventions`` array declares convention
    identity.

    Parameters
    ----------
    levels : list of (ManifestArray, int, int)
        One per resolution level: (array, num_rows, num_columns).
        Ordered from highest to lowest resolution.
    source_url : str
        Source URL for the root group attributes.
    multi_range_refs : dict
        Accumulated multi-range references.
    downsampling_method : str or None
        Recorded as ``resampling_method`` in multiscales metadata.
        Only included when not ``None``.

    Returns
    -------
    ManifestGroup
    """
    from virtualizarr.manifests import ManifestGroup

    subgroups = {}
    layout = []

    for i, (array, rows, cols) in enumerate(levels):
        subgroups[str(i)] = ManifestGroup(arrays={"data": array})
        entry = {"asset": str(i)}
        if i == 0:
            entry["transform"] = {
                "scale": [1.0, 1.0],
                "translation": [0.0, 0.0],
            }
        else:
            prev_rows = levels[i - 1][1]
            prev_cols = levels[i - 1][2]
            scale_y = prev_rows / rows if rows > 0 else 1.0
            scale_x = prev_cols / cols if cols > 0 else 1.0
            entry["derived_from"] = str(i - 1)
            entry["transform"] = {
                "scale": [scale_y, scale_x],
                "translation": [0.0, 0.0],
            }
        layout.append(entry)

    multiscales = {"layout": layout}
    if downsampling_method is not None:
        multiscales["resampling_method"] = downsampling_method

    attributes = {
        "source": source_url,
        "zarr_conventions": [GEOZARR_MULTISCALES_CONVENTION],
        "multiscales": multiscales,
    }

    return ManifestGroup(
        arrays={},
        groups=subgroups,
        attributes=attributes,
    )


class OversightMLParser:
    """VirtualiZarr parser for any imagery format supported by IO.open().

    Supports NITF (2.0, 2.1, NSIF 1.0, SICD, SIDD), standalone JPEG 2000
    (.j2k, .jp2), TIFF, and GeoTIFF.  Format detection is handled by
    ``IO.open()`` — the parser itself is format-agnostic.

    Parameters
    ----------
    local_paths : str or list[str]
        Path(s) to the local imagery file(s) to scan.  A single string is
        wrapped in a list automatically.  For multi-file pyramids, pass one
        path per resolution level.

    Examples
    --------
    ::

        parser = OversightMLParser(local_paths="/data/image.ntf")
        manifest_store = parser(url="s3://bucket/image.ntf")

        parser = OversightMLParser(local_paths="/data/image.tif")
        manifest_store = parser(url="s3://bucket/image.tif")

        parser = OversightMLParser(local_paths=["/data/image.ntf", "/data/image.ntf.r1"])
        manifest_store = parser(url=["s3://bucket/image.ntf", "s3://bucket/image.ntf.r1"])
    """

    def __init__(self, local_paths: str | list[str]):
        if isinstance(local_paths, str):
            local_paths = [local_paths]
        self.local_paths = local_paths

    def __call__(self, url: str | list[str], registry=None, **kwargs):
        """Scan the local file(s) and build a ManifestStore.

        Parameters
        ----------
        url : str or list[str]
            Cloud URI(s) written into chunk references.  A single string is
            used for all assets (e.g. a COG with embedded overviews).  A list
            must have the same length as ``local_paths`` — each URL
            corresponds to the local path at the same index.
        registry : optional
            Object store registry (accepted for protocol conformance, ignored).

        Returns
        -------
        ManifestStore
            Virtual Zarr store with chunk references into *url*.

        Raises
        ------
        ValueError
            If the file contains no indexable image segments, or if *url* is
            a list whose length does not match ``local_paths``.
        """
        _import_virtualizarr()

        from virtualizarr.manifests import ManifestGroup, ManifestStore

        # --- URL normalization (Task 6.1) ---
        if isinstance(url, str):
            urls: list[str] = [url]
        else:
            urls = list(url)
            if len(urls) != len(self.local_paths):
                raise ValueError(
                    f"url list length ({len(urls)}) must match "
                    f"local_paths length ({len(self.local_paths)})"
                )

        # --- Build URL lookup from paths (Task 6.2) ---
        # Map overview level N → urls[i] by parsing .rN suffix from each path.
        # The base path (no .rN or .r0) maps to urls[0].
        # When a single URL is provided for multiple paths, all levels use urls[0].
        _rset_pattern = re.compile(r"\.r(\d+)$")
        url_by_overview_level: dict[int, str] = {0: urls[0]}
        for i, p in enumerate(self.local_paths):
            # Use urls[i] when available, otherwise fall back to urls[0]
            # (single URL string → replicated for all paths)
            u = urls[i] if i < len(urls) else urls[0]
            m = _rset_pattern.search(p)
            if m:
                level = int(m.group(1))
                url_by_overview_level[level] = u
            else:
                # Base file (no .rN suffix) — always level 0
                url_by_overview_level[0] = u

        arrays = {}
        multi_range_refs: dict[str, list] = {}

        with IO.open(self.local_paths, "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)

            # Collect all (key, asset) tuples
            all_assets = [(key, reader.get_asset(key)) for key in keys]

            # Classify into parents + overviews
            parents, overviews = _classify_assets(all_assets)

            has_overviews = bool(overviews)

            if has_overviews:
                # Hierarchical path — build one multiscale group per parent
                # with overviews
                group = None
                for parent_key, parent_asset in parents.items():
                    if parent_key in overviews:
                        # Build levels list: parent as level 0, overviews
                        # as levels 1+
                        levels = []

                        # Level 0: parent asset
                        parent_url = url_by_overview_level.get(0, urls[0])
                        parent_array = _build_manifest_array(
                            parent_asset, parent_url, multi_range_refs,
                            key_prefix="0/data/"
                        )
                        if parent_array is not None:
                            levels.append((
                                parent_array,
                                parent_asset.num_rows,
                                parent_asset.num_columns,
                            ))

                        # Levels 1+: overviews
                        for level_num, ovr_asset in overviews[parent_key]:
                            ovr_url = url_by_overview_level.get(
                                level_num, urls[0]
                            )
                            ovr_array = _build_manifest_array(
                                ovr_asset, ovr_url, multi_range_refs,
                                key_prefix=f"{len(levels)}/data/"
                            )
                            if ovr_array is not None:
                                levels.append((
                                    ovr_array,
                                    ovr_asset.num_rows,
                                    ovr_asset.num_columns,
                                ))

                        if levels:
                            group = _build_multiscale_group(
                                levels, urls[0], multi_range_refs,
                                downsampling_method=kwargs.get(
                                    "downsampling_method"
                                ),
                            )
                    else:
                        # Parent without overviews — add as flat array
                        asset_url = urls[0]
                        array = _build_manifest_array(
                            parent_asset, asset_url, multi_range_refs,
                            key_prefix=f"{parent_key}/"
                        )
                        if array is not None:
                            arrays[parent_key] = array

                if group is None and not arrays:
                    raise ValueError(
                        f"No indexable image segments found in "
                        f"{self.local_paths}"
                    )

                # If we built a hierarchical group, use it
                if group is not None:
                    store = ManifestStore(group=group)
                else:
                    store = ManifestStore(
                        group=ManifestGroup(
                            arrays=arrays,
                            attributes={"source": urls[0]},
                        )
                    )
            else:
                # Flat path — identical to current implementation
                for key, asset in all_assets:
                    asset_url = urls[0]
                    array = _build_manifest_array(
                        asset, asset_url, multi_range_refs,
                        key_prefix=f"{key}/"
                    )
                    if array is not None:
                        arrays[key] = array

                if not arrays:
                    raise ValueError(
                        f"No indexable image segments found in "
                        f"{self.local_paths}"
                    )

                group = ManifestGroup(
                    arrays=arrays, attributes={"source": urls[0]}
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


def _write_flat_tile_index(store, output, ext, multi_range_refs, segments):
    """Serialize a flat ManifestStore (no subgroups) to JSON or Parquet.

    This is the original serialization path, extracted as a helper so that
    ``write_tile_index()`` can dispatch between flat and hierarchical stores.
    """
    import json

    from virtualizarr.accessor import dataset_to_kerchunk_refs
    from virtualizarr.manifests import ManifestGroup, ManifestStore

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


def _write_hierarchical_tile_index(store, output, ext, multi_range_refs, segments):
    """Serialize a hierarchical ManifestStore with GeoZarr multiscales metadata.

    Walks the ManifestGroup tree and builds a flat refs dict with
    path-prefixed keys.  The root ``.zattrs`` contains the GeoZarr
    ``zarr_conventions`` array and ``multiscales`` object produced by
    :func:`_build_multiscale_group`.  Each subgroup's arrays are serialized
    individually via ``dataset_to_kerchunk_refs`` and their keys are prefixed
    with the subgroup path (e.g. ``0/data/0.0.0``).
    """
    import json

    from virtualizarr.accessor import dataset_to_kerchunk_refs
    from virtualizarr.manifests import ManifestGroup, ManifestStore

    group = store._group

    # Filter subgroups if segments specified
    if segments:
        available = list(group.groups.keys())
        missing = [s for s in segments if s not in group.groups]
        if missing:
            raise ValueError(
                f"Subgroup(s) not found: {', '.join(missing)}. "
                f"Available: {', '.join(available)}"
            )
        filtered_groups = {k: v for k, v in group.groups.items() if k in segments}
        group = ManifestGroup(
            arrays=group.arrays,
            groups=filtered_groups,
            attributes=group.metadata.attributes if group.metadata else None,
        )
        multi_range_refs = {
            k: v for k, v in multi_range_refs.items()
            if any(k.startswith(seg + "/") for seg in segments)
        }

    # Build refs dict by walking the tree
    refs = {}

    # Root group metadata
    root_attrs = group.metadata.attributes if group.metadata else {}
    refs[".zgroup"] = json.dumps({"zarr_format": 2})
    refs[".zattrs"] = json.dumps(root_attrs)

    # Each subgroup — serialize via dataset_to_kerchunk_refs and prefix keys
    for sg_name, sg in group.groups.items():
        temp_store = ManifestStore(group=sg)
        temp_vds = temp_store.to_virtual_dataset()
        temp_refs = dataset_to_kerchunk_refs(temp_vds)
        if "refs" in temp_refs:
            temp_refs = temp_refs["refs"]

        # Prefix all keys with the subgroup path
        for k, v in temp_refs.items():
            refs[f"{sg_name}/{k}"] = v

    # Patch multi-range refs
    refs = _patch_multi_range_refs(refs, multi_range_refs)

    if ext == ".json":
        kerchunk = {"version": 1, "refs": refs}
        with open(output, "w") as f:
            json.dump(kerchunk, f)

    elif ext == ".parquet":
        import fsspec
        from fsspec.implementations.reference import LazyReferenceMapper

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
        included.  For flat stores, these are array keys.  For hierarchical
        stores, these are subgroup keys (e.g. ``["0", "2"]``).

    Raises
    ------
    ValueError
        If the output extension is not ``.json`` or ``.parquet``, or if
        a requested segment is not found.

    Examples
    --------
    ::

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        parser = OversightMLParser(local_paths="local/image.ntf")
        store = parser(url="s3://my-bucket/imagery/image.ntf")
        write_tile_index(store, "image.tile_index.json")

        # Or index only specific segments
        write_tile_index(store, "image.tile_index.json", segments=["image_segment_0"])
    """
    from pathlib import Path

    ext = Path(output).suffix.lower()
    multi_range_refs = getattr(store, "multi_range_refs", {}) or {}

    # Detect hierarchical vs flat store
    is_hierarchical = bool(store._group.groups)

    if is_hierarchical:
        _write_hierarchical_tile_index(store, output, ext, multi_range_refs, segments)
    else:
        _write_flat_tile_index(store, output, ext, multi_range_refs, segments)
