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

    parser = OversightMLParser()
    manifest_store = parser("s3://bucket/image.ntf")  # range reads, no full download

The URL is the single source of truth: it is both read (via fsspec) and written
into the chunk references.  Relocatable / portable indexes are produced at
serialization time — see :func:`write_tile_index` (``template_base`` /
``url_overrides``)::

    parser = OversightMLParser()
    manifest_store = parser("/data/image.ntf")
    write_tile_index(manifest_store, "image.json", template_base="{{base}}")
"""

from __future__ import annotations

import base64
import os
import posixpath
import re
from typing import TYPE_CHECKING
from urllib.parse import urlsplit

from aws.osml.io._io import IO, AssetType

if TYPE_CHECKING:
    pass


# ---------------------------------------------------------------------------
# URL → format derivation
# ---------------------------------------------------------------------------

# Map a file extension to the format string ``IO.open`` expects when reading
# from a stream (a file-like object has no filename to auto-detect from).  This
# mirrors ``convenience._EXTENSION_TO_FORMAT`` but is kept local so the parser
# module has no import-time dependency on the convenience layer.
_URL_EXTENSION_TO_FORMAT: dict[str, str] = {
    ".ntf": "nitf",
    ".nitf": "nitf",
    ".nsif": "nitf",
    ".tif": "geotiff",
    ".tiff": "geotiff",
    ".gtif": "geotiff",
    ".gtiff": "geotiff",
    ".j2k": "j2k",
    ".jp2": "j2k",
    ".jpx": "j2k",
    ".png": "png",
    ".jpg": "jpeg",
    ".jpeg": "jpeg",
    ".dt0": "dted",
    ".dt1": "dted",
    ".dt2": "dted",
    ".dt3": "dted",
    ".dt4": "dted",
    ".dt5": "dted",
}

# R-set companion suffix, e.g. ``image.ntf.r1`` is overview level 1 of
# ``image.ntf``.  Matches the ``.rN`` convention IO.open uses for local R-set
# pyramids.
_RSET_SUFFIX = re.compile(r"\.r(\d+)$", re.IGNORECASE)


def _url_path(url: str) -> str:
    """Return the path component of *url* (scheme-agnostic).

    For a plain local path the whole string is the path; for a URL such as
    ``s3://bucket/key.ntf`` only ``/bucket/key.ntf`` is returned.  Used purely
    for extension / ``.rN`` inspection, never for opening bytes.
    """
    split = urlsplit(url)
    # No scheme (plain local path) → urlsplit puts everything in ``path``.
    if not split.scheme:
        return url
    return split.path


def _format_from_url(url: str) -> str:
    """Derive the ``IO.open`` format string from a URL's file extension.

    The base file's extension determines the format; any ``.rN`` R-set suffix
    is stripped first so ``image.ntf.r1`` is recognized as NITF.

    Raises
    ------
    ValueError
        If the extension is not recognized.
    """
    path = _RSET_SUFFIX.sub("", _url_path(url))
    ext = posixpath.splitext(path)[1].lower()
    fmt = _URL_EXTENSION_TO_FORMAT.get(ext)
    if fmt is None:
        supported = ", ".join(sorted(_URL_EXTENSION_TO_FORMAT))
        raise ValueError(
            f"Cannot determine imagery format from URL '{url}'. "
            f"Recognized extensions: {supported}"
        )
    return fmt


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
    from aws.osml.io.zarr_codecs import DtedTileCodec, JbpBlockCodec, Jpeg2000Codec, JpegCodec, TiffTileCodec

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
        elif isinstance(value, (bytes, bytearray)) and len(value) == 2:
            raw[key] = int.from_bytes(value, "little")
        elif isinstance(value, (bytes, bytearray)) and len(value) == 4:
            raw[key] = int.from_bytes(value, "little")
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
        pvtype_raw = raw["pvtype"]
        if isinstance(pvtype_raw, int):
            # Rust sends pvtype as raw ASCII bytes (e.g. b"SI", b"R").  The
            # byte-length normalizer above converts 1-byte and 2-byte values
            # to ints.  Reverse that to recover the original ASCII string.
            if pvtype_raw < 256:
                pvtype = chr(pvtype_raw)
            else:
                pvtype = pvtype_raw.to_bytes(2, "little").decode("ascii").rstrip("\x00")
        else:
            pvtype = str(pvtype_raw)
        nbpp = raw.get("nbpp", raw.get("abpp", asset.num_bits_per_pixel))
        if isinstance(nbpp, str):
            nbpp = int(nbpp)
        return JbpBlockCodec(
            num_bands=num_bands,
            block_height=block_h,
            block_width=block_w,
            nbpp=nbpp,
            imode=imode,
            pvtype=pvtype,
        )
    elif "compression" in raw:
        # TIFF tile codec
        jpeg_tables_raw = raw.get("jpeg_tables")
        jpeg_tables = None
        if jpeg_tables_raw is not None:
            if isinstance(jpeg_tables_raw, (bytes, bytearray)):
                jpeg_tables = base64.b64encode(jpeg_tables_raw).decode("ascii")
            else:
                jpeg_tables = str(jpeg_tables_raw)

        return TiffTileCodec(
            compression=raw["compression"],
            bits_per_sample=raw.get("bits_per_sample", asset.num_bits_per_pixel),
            samples_per_pixel=raw.get("samples_per_pixel", num_bands),
            photometric=raw.get("photometric", 1),
            planar_config=raw.get("planar_config", 1),
            predictor=raw.get("predictor", 1),
            tile_width=raw.get("tile_width", block_w),
            tile_height=raw.get("tile_height", block_h),
            sample_format=raw.get("sample_format", 1),
            jpeg_tables=jpeg_tables,
        )
    elif "dted_codec" in raw:
        # DTED tile codec
        return DtedTileCodec(
            num_lat_points=raw.get("num_lat_points", block_h),
            num_lon_lines=raw.get("num_lon_lines", block_w),
            record_size=raw.get("record_size", 8 + block_h * 2 + 4),
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
# Transparency-mask assets carry a ``:mask`` suffix (``image:0:mask``,
# ``image:0:overview:1:mask``).  ``OVERVIEW_PATTERN`` is anchored with ``$`` so
# mask-overview keys never match it; without an explicit pattern they would
# otherwise fall through to the ``image:`` branch and be misclassified as
# parents.  Matching them here lets ``_classify_assets`` skip masks
# intentionally rather than silently mis-grouping them.
MASK_PATTERN = re.compile(r":mask$")


def _classify_assets(all_assets):
    """Classify assets into parent images and their overviews.

    Transparency-mask assets (keys ending in ``:mask``) are recognized via
    :data:`MASK_PATTERN` and deliberately excluded from both ``parents`` and
    ``overviews``: they are not part of the resolution pyramid the Zarr view
    exposes.  Consuming a mask as nodata is a deferred follow-on (see the
    design doc's Non-Goals), so masks are skipped here rather than surfaced as
    extra Zarr arrays.

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
        if MASK_PATTERN.search(key):
            # Transparency masks are not part of the Zarr pyramid — skip them.
            continue
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


def _discover_rset_companions(url: str) -> dict[int, str]:
    """Discover ``.rN`` R-set companion URLs for a base *url*.

    Multi-file NITF pyramids store each overview level in a sibling file named
    ``<base>.r1``, ``<base>.r2``, …  Given the base *url*, this globs the
    containing filesystem (via fsspec, so it works for local paths, ``file://``,
    and ``s3://`` alike) for companions and returns ``{level: companion_url}``
    including ``{0: url}`` for the base.  Companion URLs are reconstructed by
    appending the ``.rN`` suffix to *url*, preserving its scheme and directory.

    Levels may be sparse (e.g. ``.r1`` and ``.r3`` with no ``.r2``); each
    discovered level is keyed by its own number.  A filesystem that cannot glob
    degrades gracefully to the base URL only.
    """
    import fsspec

    companions: dict[int, str] = {0: url}
    try:
        fs, path = fsspec.core.url_to_fs(url)
        matches = fs.glob(path + ".r*")
    except Exception:
        return companions
    for match in matches:
        m = _RSET_SUFFIX.search(str(match))
        if m:
            level = int(m.group(1))
            # Reconstruct the companion URL from the base URL so the scheme and
            # directory match exactly (glob returns bare filesystem paths).
            companions[level] = f"{url}.r{level}"
    return companions


class OversightMLParser:
    """VirtualiZarr parser for any imagery format supported by IO.open().

    Supports NITF (2.0, 2.1, NSIF 1.0, SICD, SIDD), standalone JPEG 2000
    (.j2k, .jp2), TIFF, and GeoTIFF.  Format detection is derived from the URL
    extension — the parser itself is format-agnostic.

    Conforms to the VirtualiZarr parser protocol: an instance is a callable
    ``(url: str, registry) -> ManifestStore``.  Bytes are read by opening *url*
    with fsspec and handing the seekable handle to :func:`IO.open`, which issues
    on-demand byte-range reads for the block-capable formats (TIFF, JPEG 2000,
    NITF, DTED) — so a local path and an ``s3://`` URL follow the same code path
    and neither downloads the whole file to build the index.

    Multi-file R-set pyramids are reconstructed automatically: given a base
    *url*, sibling ``<base>.r1`` / ``.r2`` / … files are discovered on the same
    filesystem and mapped to overview levels.

    Examples
    --------
    Index a local file (chunk refs point at the same local path)::

        parser = OversightMLParser()
        manifest_store = parser("/data/image.ntf")

    Index a remote file directly (range reads, no full download)::

        parser = OversightMLParser()
        manifest_store = parser("s3://bucket/image.ntf")

    Multi-file pyramid (``image.ntf`` + ``image.ntf.r1`` auto-discovered)::

        parser = OversightMLParser()
        manifest_store = parser("s3://bucket/image.ntf")

    See :func:`write_tile_index` for portable (``{{base}}``-template) and
    URL-rewritten output — relocating chunk references is a serialization-time
    concern, not a parse-time one.
    """

    def __init__(self):
        # No parse-time configuration: the URL passed to __call__ is the single
        # source of truth for both reading and chunk-reference targets.
        pass

    def __call__(self, url: str, registry=None, **kwargs):
        """Scan the imagery at *url* and build a ManifestStore.

        Parameters
        ----------
        url : str
            URL or path of the imagery file to index.  Opened via fsspec, so
            local paths, ``file://`` URIs, and ``s3://`` URIs all work.  Chunk
            references in the returned store point at this URL.  R-set overview
            companions (``<url>.r1``, …) are discovered automatically.
        registry : ObjectStoreRegistry, optional
            Passed through to the returned ``ManifestStore`` for VirtualiZarr
            protocol conformance (used to resolve chunk-data object stores at
            read time).  The parser reads its own bytes via fsspec.

        Returns
        -------
        ManifestStore
            Virtual Zarr store with chunk references into *url*.

        Raises
        ------
        ValueError
            If the file contains no indexable image segments, or the format
            cannot be derived from the URL extension.
        """
        import contextlib

        _import_virtualizarr()

        import fsspec
        from virtualizarr.manifests import ManifestStore

        if not isinstance(url, str):
            raise TypeError(
                f"url must be a string (path or URI), got {type(url).__name__}. "
                "Multi-file R-set pyramids are discovered automatically from the "
                "base URL."
            )

        fmt = _format_from_url(url)

        # --- Discover R-set companions and map each level → URL ---
        url_by_overview_level = _discover_rset_companions(url)
        # Sorted levels: 0 (base) first, then overviews ascending.
        sorted_levels = sorted(url_by_overview_level)

        multi_range_refs: dict[str, list] = {}

        with contextlib.ExitStack() as stack:
            # Open one fsspec handle per level. IO.open routes each seekable,
            # sized handle through the Remote OwnedBuffer (range reads).
            handles = [
                stack.enter_context(fsspec.open(url_by_overview_level[level], "rb"))
                for level in sorted_levels
            ]

            if len(handles) == 1:
                reader = stack.enter_context(IO.open(handles[0], "r", format=fmt))
            else:
                # Multi-source R-set: level 0 is the base ("data"), the rest are
                # overviews. Explicit roles are required for stream lists (no
                # filenames to derive .rN from).
                roles = [
                    ["data"] if level == 0 else [f"overview:{level}"]
                    for level in sorted_levels
                ]
                reader = stack.enter_context(
                    IO.open(handles, "r", format=fmt, roles=roles)
                )
            keys = reader.get_asset_keys(asset_type=AssetType.Image)

            # Collect all (key, asset) tuples
            all_assets = [(key, reader.get_asset(key)) for key in keys]

            # Classify into parents + overviews
            parents, overviews = _classify_assets(all_assets)

            # Always produce a hierarchical store with GeoZarr multiscales
            # metadata.  Single-resolution images become a one-level pyramid
            # so the access path (``root["0/data"]``) is the same regardless
            # of whether overviews are present.
            #
            # When multiple independent parent segments exist (e.g. a main
            # image + an embedded thumbnail), select the largest by pixel
            # count.  The previous loop overwrote ``group`` on each
            # iteration, silently dropping all parents except the last.
            primary_key = max(
                parents,
                key=lambda k: parents[k].num_rows * parents[k].num_columns,
            )
            primary_asset = parents[primary_key]

            levels = []

            # Level 0: primary parent asset
            parent_url = url_by_overview_level.get(0, url)
            parent_array = _build_manifest_array(
                primary_asset, parent_url, multi_range_refs,
                key_prefix="0/data/"
            )
            if parent_array is not None:
                levels.append((
                    parent_array,
                    primary_asset.num_rows,
                    primary_asset.num_columns,
                ))

            # Levels 1+: overviews (if any)
            if primary_key in overviews:
                for level_num, ovr_asset in overviews[primary_key]:
                    ovr_url = url_by_overview_level.get(
                        level_num, url
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

            group = None
            if levels:
                group = _build_multiscale_group(
                    levels, url, multi_range_refs,
                    downsampling_method=kwargs.get(
                        "downsampling_method"
                    ),
                )

            if group is None:
                raise ValueError(
                    f"No indexable image segments found in {url}"
                )

            store = ManifestStore(group=group, registry=registry)

        # Attach the multi-range references so ``write_tile_index`` can patch
        # non-contiguous chunk entries into their multi-range form.  Relocating
        # references (portable ``{{base}}`` templating, URL overrides) is a
        # serialization-time concern handled entirely by ``write_tile_index``.
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


def _rewrite_refs_urls(refs: dict, rewrites: dict) -> dict:
    """Rewrite URLs in chunk references using a mapping dict.

    For each reference value that is a list (single-range or multi-range),
    if the URL (first element) matches a key in *rewrites*, it is replaced
    with the corresponding value.  Metadata keys (strings, dicts) are left
    unchanged.

    Parameters
    ----------
    refs : dict
        The refs dict to rewrite.
    rewrites : dict
        Mapping from original URL to replacement URL.

    Returns
    -------
    dict
        A new refs dict with URLs rewritten.
    """
    if not rewrites:
        return refs
    patched = {}
    for k, v in refs.items():
        if isinstance(v, list) and len(v) >= 1 and isinstance(v[0], str):
            url = v[0]
            if url in rewrites:
                patched[k] = [rewrites[url]] + v[1:]
            else:
                patched[k] = v
        else:
            patched[k] = v
    return patched


def _collect_ref_urls(refs: dict, source: str | None) -> set[str]:
    """Collect the distinct chunk-reference URLs present in *refs*.

    Each list-valued entry (single-range ``[url, offset, length]`` or
    multi-range ``[url, [[o, l], ...]]``) contributes its URL (first element).
    *source*, when given, is included so the root ``source`` attribute is
    rewritten alongside the chunk refs.
    """
    urls: set[str] = set()
    if source:
        urls.add(source)
    for v in refs.values():
        if isinstance(v, list) and v and isinstance(v[0], str):
            urls.add(v[0])
    return urls


def _build_url_rewrites(urls, template_base, url_overrides):
    """Build a ``{concrete_url: replacement_url}`` rewrite mapping.

    *template_base* (e.g. ``"{{base}}"``) maps each URL to
    ``template_base + basename`` for relocatable/portable indexes.
    *url_overrides* is an explicit ``{old: new}`` mapping (e.g. rewrite a local
    read path to the ``s3://`` URL the data will live at).  The two are
    mutually exclusive.
    """
    if template_base is not None:
        return {
            u: f"{template_base}{posixpath.basename(_url_path(u))}"
            for u in urls
        }
    if not url_overrides:
        return {}
    # Make matching tolerant of the ``file://`` normalization VirtualiZarr
    # applies to bare local paths: an override keyed on a plain local path also
    # matches the ``file://<abspath>`` form that appears in the stored refs.
    rewrites: dict[str, str] = {}
    for old, new in url_overrides.items():
        rewrites[old] = new
        if "://" not in old:
            rewrites[f"file://{os.path.abspath(old)}"] = new
    return rewrites


def _write_hierarchical_tile_index(store, output, ext, multi_range_refs, segments, template_base, url_overrides):
    """Serialize a hierarchical ManifestStore with GeoZarr multiscales metadata.

    Walks the ManifestGroup tree and builds a flat refs dict with
    path-prefixed keys.  The root ``.zattrs`` contains the GeoZarr
    ``zarr_conventions`` array and ``multiscales`` object produced by
    :func:`_build_multiscale_group`.  Each subgroup's arrays are serialized
    individually via ``dataset_to_kerchunk_refs`` and their keys are prefixed
    with the subgroup path (e.g. ``0/data/0.0.0``).

    URL relocation (``template_base`` for portable ``{{base}}`` indexes, or an
    explicit ``url_overrides`` mapping) is applied here at serialization time.
    """
    import json

    from virtualizarr.accessor import dataset_to_kerchunk_refs
    from virtualizarr.manifests import ManifestGroup, ManifestStore

    use_templates = template_base is not None

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

    # Patch multi-range refs into the flat refs dict.
    refs = _patch_multi_range_refs(refs, multi_range_refs)

    # Compute and apply URL relocation (portable template or explicit overrides)
    # over every concrete URL that appears in the refs, plus the root source.
    source = root_attrs.get("source") if isinstance(root_attrs, dict) else None
    rewrites = _build_url_rewrites(
        _collect_ref_urls(refs, source), template_base, url_overrides
    )
    if rewrites:
        if source in rewrites:
            root_attrs = dict(root_attrs)
            root_attrs["source"] = rewrites[source]
        refs = _rewrite_refs_urls(refs, rewrites)

    refs[".zattrs"] = json.dumps(root_attrs)

    if ext == ".json":
        kerchunk = {"version": 1, "refs": refs}
        if use_templates:
            kerchunk["templates"] = {"base": ""}
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


def write_tile_index(
    store,
    output: str,
    segments: list[str] | None = None,
    *,
    template_base: str | None = None,
    url_overrides: dict[str, str] | None = None,
) -> None:
    """Write a tile index to JSON or Parquet with multi-range support.

    This is the recommended way to serialize a ``ManifestStore`` produced by
    :class:`OversightMLParser`.  It handles the multi-range reference entries
    that VirtualiZarr's built-in serialization does not support.

    By default chunk references point at the URL the store was parsed from.
    Relocating those references is a serialization-time concern controlled here:

    - **Portable / relocatable index** — pass ``template_base="{{base}}"`` to
      rewrite every chunk-reference URL to ``{{base}}<filename>`` and emit a
      Kerchunk v1 ``"templates": {"base": ""}`` dict.  At read time the base is
      supplied via ``template_overrides={"base": "s3://bucket/path/"}`` to
      ``MultiReferenceFileSystem`` / ``ReferenceFileSystem``.
    - **Explicit URL rewrite** — pass ``url_overrides={old_url: new_url}`` to
      remap concrete URLs (e.g. index by reading a local copy, then point the
      references at the ``s3://`` location the data will be served from).

    ``template_base`` and ``url_overrides`` are mutually exclusive.

    Parameters
    ----------
    store : ManifestStore
        The manifest store returned by ``OversightMLParser()``.
    output : str
        Output file path.  Extension determines format: ``.json`` for
        Kerchunk JSON, ``.parquet`` for Kerchunk Parquet.
    segments : list[str], optional
        Subgroup keys to include (e.g. ``["0", "2"]``).  If ``None``, all
        subgroups are included.
    template_base : str, optional
        When given (typically ``"{{base}}"``), produces a portable index whose
        chunk-reference URLs are rewritten to ``template_base + basename``.
    url_overrides : dict[str, str], optional
        Explicit ``{concrete_url: replacement_url}`` rewrite applied to chunk
        references and the root ``source`` attribute.

    Raises
    ------
    ValueError
        If the output extension is not ``.json`` or ``.parquet``, if a
        requested segment is not found, or if both ``template_base`` and
        ``url_overrides`` are given.

    Examples
    --------
    Absolute URL index (references point at the parsed URL)::

        parser = OversightMLParser()
        store = parser("s3://my-bucket/imagery/image.ntf")
        write_tile_index(store, "image.tile_index.json")

    Portable index (resolve base URL at read time)::

        parser = OversightMLParser()
        store = parser("local/image.ntf")
        write_tile_index(store, "image.tile_index.json", template_base="{{base}}")

    Index a local copy, reference the remote location::

        parser = OversightMLParser()
        store = parser("local/image.ntf")
        write_tile_index(
            store, "image.tile_index.json",
            url_overrides={os.path.abspath("local/image.ntf"): "s3://bucket/image.ntf"},
        )
    """
    from pathlib import Path

    if template_base is not None and url_overrides:
        raise ValueError(
            "template_base and url_overrides are mutually exclusive"
        )

    ext = Path(output).suffix.lower()
    multi_range_refs = getattr(store, "multi_range_refs", {}) or {}

    _write_hierarchical_tile_index(
        store, output, ext, multi_range_refs, segments,
        template_base, url_overrides,
    )
