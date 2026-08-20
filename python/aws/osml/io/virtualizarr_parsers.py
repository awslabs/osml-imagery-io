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


def _normalize_codec_config(codec_config: dict) -> dict:
    """Decode a Rust ``codec_configuration()`` map into Python scalars.

    The Rust side sends every value as little-endian bytes: 1-, 2- and 4-byte
    values are integers, longer opaque blobs (``main_header``, ``jpeg_tables``)
    are base64-encoded, and anything else that is ASCII-decodable becomes a
    string.
    """
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
    return raw


def _is_planar_multiband_tiff(asset) -> bool:
    """Whether *asset* is a TIFF with ``PlanarConfiguration = 2`` and >1 band.

    Such an asset stores each band as its own tile/strip (TIFF 6.0 tag 284,
    p. 38), so the Zarr view gives each plane its own chunk — see
    :func:`_build_manifest_array`.  Assets from other formats, and single-band
    TIFFs (where PlanarConfiguration is irrelevant per the same page), return
    ``False``.
    """
    if asset.num_bands <= 1:
        return False
    codec_config = asset.codec_configuration()
    if codec_config is None:
        return False
    raw = _normalize_codec_config(codec_config)
    # ``planar_config`` is TIFF-only, so its presence also identifies the format.
    return raw.get("planar_config") == 2


def _build_codec_instance(asset, *, single_plane: bool = False):
    """Map an ImageAssetProvider's codec_configuration() to a codec instance.

    Returns ``None`` when the asset has no codec configuration (e.g. TIFF
    segments where ``codec_configuration()`` returns ``None``).

    The mapping logic mirrors ``tile_index.py:_build_zarray()`` but produces
    zarr v3 codec class instances instead of zarr v2 filter dicts.

    Parameters
    ----------
    asset : ImageAssetProvider
        The asset whose codec configuration to translate.
    single_plane : bool
        TIFF only.  When set, the returned :class:`TiffTileCodec` is overridden
        to ``samples_per_pixel=1, planar_config=1`` because each chunk it will
        be handed is one plane of a planar image — i.e. a standalone
        single-band chunky tile.  Ignored by the other codecs.
    """
    from aws.osml.io.zarr_codecs import DtedTileCodec, JbpBlockCodec, Jpeg2000Codec, JpegCodec, TiffTileCodec

    codec_config = asset.codec_configuration()
    if codec_config is None:
        return None

    raw = _normalize_codec_config(codec_config)

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

        # A per-plane chunk is a standalone single-band chunky tile: it holds one
        # plane's samples with nothing interleaved, which is exactly what
        # ``samples_per_pixel=1, planar_config=1`` describes.  Reporting the true
        # band count here would make the decoder expect N planes' worth of bytes
        # in a buffer that carries one.
        samples_per_pixel = 1 if single_plane else raw.get("samples_per_pixel", num_bands)
        planar_config = 1 if single_plane else raw.get("planar_config", 1)

        return TiffTileCodec(
            compression=raw["compression"],
            bits_per_sample=raw.get("bits_per_sample", asset.num_bits_per_pixel),
            samples_per_pixel=samples_per_pixel,
            photometric=raw.get("photometric", 1),
            planar_config=planar_config,
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


# ---------------------------------------------------------------------------
# Chunk-geometry fail-safe
# ---------------------------------------------------------------------------
#
# A Zarr store whose chunk geometry does not match its declared shape does not
# fail loudly — it reads.  Missing chunks come back as ``fill_value``, and a
# chunk holding fewer bytes than the codec expects comes back zero-padded.  The
# result is a plausible-looking array of wrong pixels, which is the worst
# possible failure mode for an imagery index: nothing downstream can tell it
# from real data.
#
# The checks below run once per array at index-build time, not per chunk read.
# They are pure geometry — no bytes are fetched and no chunk is decoded.


def _pixel_itemsize(asset) -> int:
    """Bytes per decoded sample for *asset*'s pixel type."""
    import numpy as np

    return np.dtype(asset.pixel_value_type.to_numpy_dtype()).itemsize


def _uncompressed_bytes_fn(raw, asset, bands_per_chunk):
    """Return ``f(rows, cols) -> bytes`` for a fixed-geometry chunk, else ``None``.

    ``None`` means "cannot be predicted from geometry", and the caller falls back
    to requiring only a non-empty range.  That is the honest answer for every
    entropy-coded layout — LZW / Deflate / PackBits TIFF, TIFF-in-JPEG, JPEG
    2000, NITF C3/C8 — where the encoded length depends on pixel content, not on
    the block's dimensions.

    For the uncompressed layouts the length *is* determined by geometry, so a
    sizing function is returned and the caller checks lengths against it.  It
    takes a row/column extent rather than returning a single number because a
    trailing block may legitimately hold only its clipped extent instead of a
    full padded block — see :func:`_validate_chunk_geometry`.
    """
    if raw is None:
        # No codec in the chain: Zarr reshapes the raw bytes into the chunk.
        itemsize = _pixel_itemsize(asset)
        return lambda rows, cols: rows * cols * bands_per_chunk * itemsize

    if "main_header" in raw or "color_space" in raw:
        # JPEG 2000 codestream / JPEG scan.
        return None

    if "dted_codec" in raw:
        # One DTED chunk is a run of whole data records, each carrying its own
        # sentinel, header and checksum alongside the elevation posts.
        record_size = int(raw.get("record_size", 0))
        if record_size <= 0:
            return None
        return lambda rows, cols: cols * record_size

    if "pvtype" in raw:
        # Uncompressed NITF block: samples are bit-packed at NBPP bits and each
        # band's plane is padded out to a byte boundary.
        nbpp = int(raw.get("nbpp") or asset.num_bits_per_pixel)
        return lambda rows, cols: (-(-(rows * cols * nbpp) // 8)) * bands_per_chunk

    if "compression" in raw:
        if int(raw["compression"]) != 1:
            return None
        bits = int(raw.get("bits_per_sample", asset.num_bits_per_pixel))
        if bits < 8:
            # Sub-byte samples are packed MSB-first with every row padded out to
            # a byte boundary (TIFF 6.0, BitsPerSample, p. 29).
            return lambda rows, cols: (
                -(-(cols * bands_per_chunk * bits) // 8) * rows
            )
        return lambda rows, cols: rows * cols * bands_per_chunk * (bits // 8)

    return None


def _describe_geometry(asset, raw, shape, chunk_shape) -> str:
    """A one-line config summary for fail-safe error messages.

    Names the axes of the configuration matrix that actually determine chunk
    layout, so the message identifies *which* image config was rejected rather
    than only that something was wrong.
    """
    parts = [f"bands={asset.num_bands}"]
    if raw is not None and "compression" in raw:
        parts.append(f"compression={int(raw['compression'])}")
        parts.append(f"planar_config={int(raw.get('planar_config', 1))}")
        parts.append(
            f"bits_per_sample={int(raw.get('bits_per_sample', asset.num_bits_per_pixel))}"
        )
    elif raw is None:
        parts.append("codec=none")
    parts.append(f"shape={tuple(shape)}")
    parts.append(f"chunk_shape={tuple(chunk_shape)}")
    return ", ".join(parts)


def _validate_chunk_geometry(
    asset, raw, chunk_lengths, *, shape, chunk_shape, grid_shape, bands_per_chunk
):
    """Refuse to emit an array whose chunk geometry cannot be satisfied.

    Four invariants, cheapest first:

    1. **A multiband chunk must have a codec.**  With no codec in the chain
       ``BytesCodec`` reshapes the chunk's bytes straight into
       ``(bands, h, w)``.  That is only meaningful for a single band — any
       multiband layout is either pixel-interleaved or per-plane, and both need
       a codec to reorder samples.  This is the shape of the original
       uncompressed-chunky-TIFF defect: bands silently scrambled, no error.
    2. **The chunk grid covers the declared shape.**  Every grid position the
       shape requires must carry an entry; a hole reads back as ``fill_value``.
    3. **No chunk references zero bytes.**
    4. **Each chunk's byte length matches the codec's expected input.**
       Enforced only where geometry determines the length (see
       :func:`_uncompressed_bytes_fn`); compressed chunks, whose lengths
       legitimately vary per tile, stop at invariant 3.

    Raises
    ------
    ValueError
        Naming the offending configuration.  Hard-failing is deliberate: a
        rejected store is recoverable, a silently-wrong one is not.
    """
    detail = _describe_geometry(asset, raw, shape, chunk_shape)

    if raw is None and asset.num_bands > 1:
        raise ValueError(
            "Refusing to build a Zarr array for a multiband asset with no codec "
            f"configuration ({detail}): the raw chunk bytes would be reshaped into "
            "(bands, y, x) with no de-interleaving, silently scrambling the bands."
        )

    required = tuple(-(-s // c) for s, c in zip(shape, chunk_shape))
    for axis, (need, have) in enumerate(zip(required, grid_shape)):
        if have < need:
            raise ValueError(
                f"Refusing to build a Zarr array whose chunk grid does not cover its "
                f"shape ({detail}): axis {axis} needs {need} chunks of "
                f"{chunk_shape[axis]} to span {shape[axis]}, but the manifest grid "
                f"has {have}."
            )

    missing = [
        f"{band}.{row}.{col}"
        for band in range(required[0])
        for row in range(required[1])
        for col in range(required[2])
        if f"{band}.{row}.{col}" not in chunk_lengths
    ]
    if missing:
        shown = ", ".join(missing[:5])
        suffix = f" (and {len(missing) - 5} more)" if len(missing) > 5 else ""
        raise ValueError(
            f"Refusing to build a Zarr array with unreferenced chunks ({detail}): "
            f"{len(missing)} of {required[0] * required[1] * required[2]} grid "
            f"positions have no byte range and would read back as fill_value. "
            f"Missing: {shown}{suffix}"
        )

    empty = [key for key, length in chunk_lengths.items() if length <= 0]
    if empty:
        raise ValueError(
            f"Refusing to build a Zarr array with empty chunk references "
            f"({detail}): {len(empty)} chunk(s) reference zero bytes, e.g. "
            f"{empty[0]}."
        )

    size_of = _uncompressed_bytes_fn(raw, asset, bands_per_chunk)
    if size_of is None:
        # Entropy-coded: length varies with pixel content, so non-empty is all
        # that can be asserted without decoding.
        return

    _, block_h, block_w = chunk_shape
    _, rows, cols = shape
    for key, length in sorted(chunk_lengths.items()):
        _, row, col = (int(part) for part in key.split("."))
        # A trailing block may be stored either padded out to the full block —
        # what tiled TIFF does — or clipped to the extent it actually covers,
        # which is what a stripped TIFF's last strip does.  Both are readable:
        # the decoder zero-pads a short block up to nominal size.  Accept either
        # and reject anything else.
        clipped = size_of(
            min(block_h, max(0, rows - row * block_h)),
            min(block_w, max(0, cols - col * block_w)),
        )
        padded = size_of(block_h, block_w)
        if length in (clipped, padded):
            continue
        raise ValueError(
            f"Refusing to build a Zarr array whose chunk byte lengths are "
            f"inconsistent with its geometry ({detail}): chunk {key} references "
            f"{length} bytes, but this uncompressed geometry requires "
            f"{padded}"
            + (f" (or {clipped} if clipped to the image edge)" if clipped != padded else "")
            + "."
        )


OVERVIEW_PATTERN = re.compile(r"^(image:\d+):overview:(\d+)$")
# Transparency-mask assets carry a ``:mask`` suffix (``image:0:mask``,
# ``image:0:overview:1:mask``).  ``OVERVIEW_PATTERN`` is anchored with ``$`` so
# mask-overview keys never match it; without an explicit pattern they would
# otherwise fall through to the ``image:`` branch and be misclassified as
# parents.  Matching them here lets ``_classify_assets`` skip masks
# intentionally rather than silently mis-grouping them.
MASK_PATTERN = re.compile(r":mask$")


def _no_indexable_segments(url):
    """Build the error raised when a URL yields nothing indexable.

    Two distinct conditions produce it — no parent image assets at all, and
    parents whose arrays could not be built — and both mean the same thing to a
    caller, so the wording lives here rather than being duplicated at each
    raise site.
    """
    return ValueError(f"No indexable image segments found in {url}")


def _classify_assets(all_assets):
    """Classify assets into parent images and their overviews.

    The classification rule is **"not an overview, not a mask"**: every
    remaining key is a parent.  Callers pass keys already filtered by
    ``get_asset_keys(asset_type=AssetType.Image)``, so each key is by
    definition an image asset — re-testing the key *spelling* would discard
    that guarantee for a weaker one.  Do not reintroduce a ``key.startswith
    ("image:")`` test here: ``image:N`` is a NITF/TIFF convention, not a
    library-wide one, and requiring it made every other format unindexable.
    DTED, whose sole image asset is keyed ``elevation``, was silently dropped
    until this was relaxed.

    Transparency-mask assets (keys ending in ``:mask``) are recognized via
    :data:`MASK_PATTERN` and deliberately excluded from both ``parents`` and
    ``overviews``: they are not part of the resolution pyramid the Zarr view
    exposes.  Consuming a mask as nodata is a deferred follow-on (see the
    design doc's Non-Goals), so masks are skipped here rather than surfaced as
    extra Zarr arrays.  The mask check must stay *first* in the loop — it is
    what keeps masks out of the catch-all parent branch.

    Parameters
    ----------
    all_assets : list of (key, asset) tuples

    Returns
    -------
    parents : dict
        Mapping parent key (e.g. "image:0", or "elevation" for DTED) to asset.
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
        else:
            # Any non-overview, non-mask image asset is a parent, whatever its
            # key spelling.  Every overview-key producer in the library emits
            # ``image:N:overview:M`` (matched above), so nothing lands here by
            # accident.
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

    Chunk granularity depends on how the source interleaves bands.  Normally one
    chunk spans every band — chunk shape ``(num_bands, block_h, block_w)``, keys
    ``0.{row}.{col}``.  For planar TIFF (``PlanarConfiguration = 2``) each band
    is a separate tile in the file, so each band gets its own chunk instead —
    chunk shape ``(1, block_h, block_w)``, keys ``{band}.{row}.{col}`` — and
    zarr stacks the bands.

    Before returning, the resulting geometry is checked by
    :func:`_validate_chunk_geometry`, which raises rather than hand back a store
    that would read as plausible wrong pixels.

    Returns
    -------
    ManifestArray or None
        ``None`` when ``asset.tile_byte_ranges()`` returns ``None``.

    Raises
    ------
    ValueError
        If the chunk geometry cannot be satisfied — see
        :func:`_validate_chunk_geometry`.
    """
    from virtualizarr.manifests import ChunkEntry, ChunkManifest, ManifestArray
    from zarr.codecs import BytesCodec
    from zarr.core.chunk_grids import RegularChunkGrid
    from zarr.core.metadata.v3 import ArrayV3Metadata

    byte_ranges = asset.tile_byte_ranges()
    if byte_ranges is None:
        return None

    num_bands = asset.num_bands
    block_h = asset.num_pixels_per_block_vertical
    block_w = asset.num_pixels_per_block_horizontal

    # A range list of length N carries two different meanings, and they are
    # indistinguishable by length alone — a 3-tile-part J2K chunk and a 3-band
    # planar TIFF tile both arrive as three ranges.  Disambiguate on the
    # provider's declared planar configuration, never on ``len(range_list)``:
    #
    #   planar TIFF  → N independently-decodable per-band chunks
    #   otherwise    → N fragments of one chunk, concatenated in order
    per_plane_chunks = _is_planar_multiband_tiff(asset)

    # Build chunk manifest entries.  ``chunk_lengths`` records the number of
    # bytes each chunk *effectively* carries, which is not always the manifest
    # entry's own length: a non-contiguous chunk stores a placeholder entry
    # covering only its first fragment, while the bytes the decoder receives are
    # every fragment concatenated.  The geometry check below needs the effective
    # figure, so it is tracked separately rather than read back off ``entries``.
    entries: dict[str, ChunkEntry] = {}
    chunk_lengths: dict[str, int] = {}
    for (row, col), range_list in byte_ranges.items():
        if per_plane_chunks:
            # One chunk per plane.  Each range is a complete single-plane tile,
            # decoded on its own, so no concatenation or multi-range ref is ever
            # needed here.
            for band, (offset, length) in enumerate(range_list):
                entries[f"{band}.{row}.{col}"] = ChunkEntry(
                    path=url, offset=offset, length=length
                )
                chunk_lengths[f"{band}.{row}.{col}"] = length
            continue

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
        chunk_lengths[chunk_key] = sum(ln for _, ln in range_list)

    if not entries:
        return None

    # Compute grid shape.  The band axis of the chunk grid is one entry per band
    # for per-plane chunking and a single entry otherwise.
    max_row = max(r for (r, _) in byte_ranges.keys()) + 1
    max_col = max(c for (_, c) in byte_ranges.keys()) + 1
    bands_per_chunk = 1 if per_plane_chunks else num_bands
    grid_shape = (num_bands if per_plane_chunks else 1, max_row, max_col)

    shape = (num_bands, asset.num_rows, asset.num_columns)
    chunk_shape = (bands_per_chunk, block_h, block_w)

    codec_config = asset.codec_configuration()
    raw_config = _normalize_codec_config(codec_config) if codec_config else None

    # Fail-safe: refuse a geometry that would read back as plausible wrong
    # pixels rather than raise.  Pure geometry — no chunk is fetched or decoded.
    _validate_chunk_geometry(
        asset,
        raw_config,
        chunk_lengths,
        shape=shape,
        chunk_shape=chunk_shape,
        grid_shape=grid_shape,
        bands_per_chunk=bands_per_chunk,
    )

    chunk_manifest = ChunkManifest(entries=entries, shape=grid_shape)

    # Build metadata
    zdtype = _pixel_type_to_zdtype(asset.pixel_value_type)

    # Build codecs list — BytesCodec is required as ArrayBytesCodec
    codecs = [BytesCodec()]
    custom_codec = _build_codec_instance(asset, single_plane=per_plane_chunks)
    if custom_codec is not None:
        codecs.append(custom_codec)

    metadata = ArrayV3Metadata(
        shape=shape,
        data_type=zdtype,
        chunk_grid=RegularChunkGrid(chunk_shape=chunk_shape),
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
            # Nothing to index.  Report it in the caller's terms — ``max()`` on
            # an empty dict would otherwise raise "max() iterable argument is
            # empty", which names neither the file nor the problem.
            if not parents:
                raise _no_indexable_segments(url)

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
                raise _no_indexable_segments(url)

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


def _filter_subgroups(group, segments, multi_range_refs):
    """Restrict *group* to the named *segments*, dropping their multi-range refs.

    Returns the (possibly unchanged) group and multi-range mapping.  Filtering is
    format-agnostic — it operates on the in-memory ``ManifestGroup`` tree before
    either serializer runs — so the v2 and v3 writers share it.

    Raises
    ------
    ValueError
        If any requested segment is not a subgroup of *group*.
    """
    from virtualizarr.manifests import ManifestGroup

    if not segments:
        return group, multi_range_refs

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
    return group, multi_range_refs


def _relocate_ref_urls(refs, root_attrs, template_base, url_overrides):
    """Apply URL relocation to *refs* and the root ``source`` attribute.

    ``template_base`` produces a portable ``{{base}}<filename>`` index;
    ``url_overrides`` remaps concrete URLs.  Shared by both serializers: the
    reference *values* have the same shape in a v2 and a v3 index, only the keys
    differ.

    Returns the rewritten ``(refs, root_attrs)`` pair.
    """
    source = root_attrs.get("source") if isinstance(root_attrs, dict) else None
    rewrites = _build_url_rewrites(
        _collect_ref_urls(refs, source), template_base, url_overrides
    )
    if not rewrites:
        return refs, root_attrs
    if source in rewrites:
        root_attrs = dict(root_attrs)
        root_attrs["source"] = rewrites[source]
    return _rewrite_refs_urls(refs, rewrites), root_attrs


def _emit_refs(refs, output, ext, *, use_templates, zarr_format=2):
    """Write a flat Kerchunk reference mapping to ``.json`` or ``.parquet``.

    The Kerchunk **JSON** container is agnostic about the Zarr version of the keys
    it carries, so it serves both the v2 (``.zgroup`` / ``.zarray``) and the v3
    (``zarr.json`` / ``c.<coords>``) layouts.

    The Kerchunk **Parquet** container is not: ``LazyReferenceMapper`` stores chunk
    references positionally, deriving each chunk's record index from the array's
    ``.zarray`` ``shape``/``chunks`` and parsing the key's last path segment as
    dot-separated grid coordinates.  A v3 store has no ``.zarray``, and its
    ``c.<band>.<row>.<col>`` key would parse the leading ``c`` as a coordinate.
    Parquet output is therefore rejected for ``zarr_format=3`` rather than written
    in a form nothing can read back.

    Raises
    ------
    ValueError
        If *ext* is neither ``.json`` nor ``.parquet``, or if Parquet output is
        requested for a v3 index.
    """
    import json

    if ext == ".json":
        kerchunk = {"version": 1, "refs": refs}
        if use_templates:
            kerchunk["templates"] = {"base": ""}
        with open(output, "w") as f:
            json.dump(kerchunk, f)

    elif ext == ".parquet":
        if zarr_format == 3:
            # Raised before the pyarrow-backed imports so the reason surfaces even
            # in an environment without that optional dependency installed.
            raise ValueError(
                "Parquet output is not supported for zarr_format=3: the Kerchunk "
                "Parquet container (fsspec's LazyReferenceMapper) indexes chunk "
                "references by position using the v2 '.zarray' shape/chunks, which "
                "a native v3 store does not have. Use a .json output path for v3 "
                "indexes, or zarr_format=2 for Parquet."
            )

        import fsspec
        from fsspec.implementations.reference import LazyReferenceMapper

        fs, _ = fsspec.core.url_to_fs(output)
        # ``engine="pyarrow"`` is required, not a preference, on both sides:
        #
        # Writing — fastparquet under pandas 3.x + numpy 2.x fails outright
        # ("Error converting column 'path' to bytes using encoding UTF8 ...
        # Unable to avoid copy while creating an array as requested").
        #
        # Reading — the engine is *not recorded in the store* (``.zmetadata``
        # holds only ``metadata`` and ``record_size``), and fsspec's
        # ``LazyReferenceMapper`` defaults to fastparquet.  The two engines
        # disagree on how a null in an object column round-trips, so a reader
        # using the other engine misreads every chunk reference.  The writer and
        # reader must therefore agree out of band:
        # ``MultiReferenceFileSystem`` pins the same engine on the read side,
        # which is why a Parquet index must be opened with it rather than a
        # stock ``ReferenceFileSystem``.
        try:
            out = LazyReferenceMapper.create(
                record_size=100_000,
                root=output,
                fs=fs,
                engine="pyarrow",
            )
        except ImportError as exc:
            raise ImportError(
                "Writing a Kerchunk Parquet tile index requires the 'pyarrow' "
                "package, which is not installed. Install it with "
                "'pip install osml-imagery-io[zarr]' (the zarr extra supplies "
                "pyarrow), or use a '.json' output path instead."
            ) from exc
        for k in sorted(refs):
            out[k] = refs[k]
        out.flush()

    else:
        raise ValueError(
            f"Unsupported output extension '{ext}'. Use .json or .parquet"
        )


def _build_v2_refs(group):
    """Build the Zarr v2 / Kerchunk reference keys for *group*'s subtree.

    Each subgroup's arrays are serialized via ``dataset_to_kerchunk_refs`` — the
    path that down-converts the in-memory ``ArrayV3Metadata`` to a v2 ``.zarray``
    — and their keys are prefixed with the subgroup path (e.g. ``0/data/0.0.0``).

    The root ``.zattrs`` is *not* written here: it is added after URL relocation,
    which may rewrite the ``source`` attribute it carries.
    """
    import json

    from virtualizarr.accessor import dataset_to_kerchunk_refs
    from virtualizarr.manifests import ManifestStore

    refs = {".zgroup": json.dumps({"zarr_format": 2})}

    for sg_name, sg in group.groups.items():
        temp_store = ManifestStore(group=sg)
        temp_vds = temp_store.to_virtual_dataset()
        temp_refs = dataset_to_kerchunk_refs(temp_vds)
        if "refs" in temp_refs:
            temp_refs = temp_refs["refs"]

        # Prefix all keys with the subgroup path
        for k, v in temp_refs.items():
            refs[f"{sg_name}/{k}"] = v

    return refs


def _v3_group_metadata(attributes) -> str:
    """Serialize a Zarr v3 group node's ``zarr.json`` document."""
    import json

    return json.dumps(
        {
            "zarr_format": 3,
            "node_type": "group",
            "attributes": dict(attributes) if attributes else {},
        }
    )


def _build_v3_refs(group, multi_range_refs):
    """Build the native Zarr v3 reference keys for *group*'s subtree.

    Unlike :func:`_build_v2_refs` this does **not** down-convert to a v2
    ``.zarray``: each array's in-memory :class:`ArrayV3Metadata` — already built
    with ``codecs=[BytesCodec(), <custom codec>]`` by
    :func:`_build_manifest_array` — is serialized straight to a ``zarr.json``
    document, and chunk keys use the array's own v3 chunk-key encoding
    (``c.<band>.<row>.<col>`` for the ``default`` encoding this library emits).

    Multi-range entries are written in place rather than patched in afterwards:
    they are accumulated at parse time under the v2-flavored chunk key
    (``0/data/0.0.0``), so the translation to the v3 key has to happen while both
    forms are in hand.

    The root ``zarr.json`` is *not* written here — it carries the ``source``
    attribute that URL relocation may rewrite, so the caller adds it last.
    """
    import json

    from zarr.core.buffer import default_buffer_prototype

    prototype = default_buffer_prototype()
    refs = {}

    for sg_name, sg in group.groups.items():
        refs[f"{sg_name}/zarr.json"] = _v3_group_metadata(
            sg.metadata.attributes if sg.metadata else {}
        )

        for arr_name, arr in sg.arrays.items():
            prefix = f"{sg_name}/{arr_name}/"
            metadata = arr.metadata
            # Let zarr serialize its own metadata rather than reconstructing the
            # document by hand — that keeps the codec chain, chunk grid and dtype
            # spelling exactly as zarr will expect to read them back.  The
            # decode/re-encode round-trip only strips zarr's indentation, which
            # would otherwise bloat every array's entry in the shipped index.
            refs[f"{prefix}zarr.json"] = json.dumps(
                json.loads(
                    metadata.to_buffer_dict(prototype)["zarr.json"].to_bytes()
                )
            )

            encode = metadata.chunk_key_encoding.encode_chunk_key
            for coord_key, entry in arr.manifest.dict().items():
                # ``ChunkManifest`` keys are dot-joined grid coordinates; the v3
                # store key comes from the array's own encoding so the two can
                # never drift.
                coords = tuple(int(part) for part in coord_key.split("."))
                v3_key = f"{prefix}{encode(coords)}"
                multi_range = multi_range_refs.get(f"{prefix}{coord_key}")
                if multi_range is not None:
                    refs[v3_key] = multi_range
                else:
                    refs[v3_key] = [
                        entry["path"], entry["offset"], entry["length"]
                    ]

    return refs


def _write_hierarchical_tile_index(
    store, output, ext, multi_range_refs, segments, template_base, url_overrides,
    *, zarr_format=2,
):
    """Serialize a hierarchical ManifestStore with GeoZarr multiscales metadata.

    Walks the ManifestGroup tree and builds a flat refs dict with path-prefixed
    keys.  The root node's attributes carry the GeoZarr ``zarr_conventions``
    array and ``multiscales`` object produced by
    :func:`_build_multiscale_group` — in a v2 index those live in ``.zattrs``,
    in a v3 index inside the root ``zarr.json``.

    *zarr_format* selects the on-disk key layout (2 → Kerchunk/``.zarray``,
    3 → native ``zarr.json``).  Everything either layout shares — segment
    filtering, URL relocation, and the JSON/Parquet sink — is common code; only
    the refs-building step differs.

    URL relocation (``template_base`` for portable ``{{base}}`` indexes, or an
    explicit ``url_overrides`` mapping) is applied here at serialization time.
    """
    import json

    group = store._group
    group, multi_range_refs = _filter_subgroups(group, segments, multi_range_refs)

    root_attrs = group.metadata.attributes if group.metadata else {}

    if zarr_format == 3:
        refs = _build_v3_refs(group, multi_range_refs)
    else:
        refs = _patch_multi_range_refs(_build_v2_refs(group), multi_range_refs)

    refs, root_attrs = _relocate_ref_urls(
        refs, root_attrs, template_base, url_overrides
    )

    # The root node's metadata is written last because relocation may have
    # rewritten the ``source`` attribute it carries.
    if zarr_format == 3:
        refs["zarr.json"] = _v3_group_metadata(root_attrs)
    else:
        refs[".zattrs"] = json.dumps(root_attrs)

    _emit_refs(
        refs, output, ext,
        use_templates=template_base is not None,
        zarr_format=zarr_format,
    )


def write_tile_index(
    store,
    output: str,
    segments: list[str] | None = None,
    *,
    template_base: str | None = None,
    url_overrides: dict[str, str] | None = None,
    zarr_format: int = 2,
) -> None:
    """Write a tile index to JSON or Parquet with multi-range support.

    This is the recommended way to serialize a ``ManifestStore`` produced by
    :class:`OversightMLParser`.  It handles the multi-range reference entries
    that VirtualiZarr's built-in serialization does not support.

    ``zarr_format`` selects which Zarr version the index describes.  Both are
    Kerchunk reference files served through fsspec — the difference is the store
    keys inside, and therefore which consumer path reads them:

    - ``2`` (default) — a ``.zgroup`` / ``.zarray`` / ``.zattrs`` layout.  Codecs
      resolve through the **numcodecs** registry by ``id`` and are called
      synchronously with a single buffer.
    - ``3`` — a native ``zarr.json`` layout with ``c.<band>.<row>.<col>`` chunk
      keys.  Codecs resolve by URI through the ``zarr.codecs`` entry points and
      are called by zarr's asynchronous batched codec pipeline.

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
        Kerchunk JSON, ``.parquet`` for a Kerchunk Parquet directory.  Parquet
        requires ``zarr_format=2`` — see :func:`_emit_refs` — needs ``pyarrow``
        (the ``osml-imagery-io[zarr]`` extra), and must be read back with
        ``MultiReferenceFileSystem``, which a stock fsspec
        ``ReferenceFileSystem`` cannot do.  Multi-resolution pyramids are
        supported in both formats.
    segments : list[str], optional
        Subgroup keys to include (e.g. ``["0", "2"]``).  If ``None``, all
        subgroups are included.
    template_base : str, optional
        When given (typically ``"{{base}}"``), produces a portable index whose
        chunk-reference URLs are rewritten to ``template_base + basename``.
    url_overrides : dict[str, str], optional
        Explicit ``{concrete_url: replacement_url}`` rewrite applied to chunk
        references and the root ``source`` attribute.
    zarr_format : int, default 2
        ``2`` for the Kerchunk / Zarr v2 layout read through numcodecs, ``3``
        for the native Zarr v3 layout read through the ``zarr.codecs``
        entry-point pipeline.

    Raises
    ------
    ValueError
        If the output extension is not ``.json`` or ``.parquet``, if a
        requested segment is not found, if both ``template_base`` and
        ``url_overrides`` are given, or if *zarr_format* is not 2 or 3.
    ImportError
        If ``.parquet`` output is requested without ``pyarrow`` installed.

    Examples
    --------
    Absolute URL index (references point at the parsed URL)::

        parser = OversightMLParser()
        store = parser("s3://my-bucket/imagery/image.ntf")
        write_tile_index(store, "image.tile_index.json")

    Native Zarr v3 index (read with no Kerchunk-to-v2 translation)::

        parser = OversightMLParser()
        store = parser("s3://my-bucket/imagery/image.ntf")
        write_tile_index(store, "image.v3.json", zarr_format=3)

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
    if zarr_format not in (2, 3):
        raise ValueError(
            f"zarr_format must be 2 or 3, got {zarr_format!r}"
        )

    ext = Path(output).suffix.lower()
    multi_range_refs = getattr(store, "multi_range_refs", {}) or {}

    _write_hierarchical_tile_index(
        store, output, ext, multi_range_refs, segments,
        template_base, url_overrides,
        zarr_format=zarr_format,
    )
