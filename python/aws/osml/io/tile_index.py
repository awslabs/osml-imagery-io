"""Kerchunk v1 tile index generation, loading, and saving.

This module provides the :class:`TileIndex` class for generating Kerchunk v1
reference files that map image tile coordinates to byte ranges in source files.
It consumes the existing Rust-backed ``tile_byte_ranges()`` and
``codec_configuration()`` APIs on :class:`ImageAssetProvider` and serializes
the result to JSON or Parquet.
"""

from __future__ import annotations

import base64
import json
import os
from typing import TYPE_CHECKING

from aws.osml.io._io import IO, AssetType

if TYPE_CHECKING:
    from aws.osml.io._io import ImageAssetProvider, PixelType


# ---------------------------------------------------------------------------
# Format detection mapping
# ---------------------------------------------------------------------------

_EXTENSION_TO_FORMAT: dict[str, str] = {
    ".ntf": "nitf",
    ".nitf": "nitf",
    ".nsif": "nitf",
    ".nsf": "nitf",
    ".tif": "tiff",
    ".tiff": "tiff",
    ".gtif": "tiff",
    ".gtiff": "tiff",
    ".j2k": "jpeg2000",
    ".jp2": "jpeg2000",
    ".jpg": "jpeg",
    ".jpeg": "jpeg",
    ".png": "png",
}


# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------


def _pixel_type_to_zarr_dtype(pixel_type: PixelType) -> str:
    """Map a :class:`PixelType` enum to the corresponding Zarr dtype string.

    Delegates to ``PixelType.to_numpy_dtype()`` which already returns the
    correct string (e.g. ``"uint8"``, ``"float32"``).
    """
    return pixel_type.to_numpy_dtype()


def _detect_format(path: str) -> str | None:
    """Infer the source imagery format from a file extension.

    Returns one of ``"nitf"``, ``"tiff"``, ``"jpeg2000"``, ``"jpeg"``,
    ``"png"``, or ``None`` if the extension is not recognised.
    """
    _, ext = os.path.splitext(path)
    return _EXTENSION_TO_FORMAT.get(ext.lower())


def _import_pyarrow():
    """Lazily import *pyarrow*, raising a helpful error if it is missing."""
    try:
        import pyarrow as pa

        return pa
    except ImportError:
        raise ImportError(
            "pyarrow is required for Parquet output. "
            "Install with: pip install pyarrow"
        )


def _build_zarray(asset: ImageAssetProvider) -> str:
    """Build the ``.zarray`` JSON string for an image segment.

    Parameters
    ----------
    asset:
        The :class:`ImageAssetProvider` for the segment.

    Returns
    -------
    str
        A JSON-encoded string suitable for inclusion in the Kerchunk v1
        ``refs`` dictionary.
    """
    zarray: dict = {
        "shape": [asset.num_bands, asset.num_rows, asset.num_columns],
        "chunks": [
            asset.num_bands,
            asset.num_pixels_per_block_vertical,
            asset.num_pixels_per_block_horizontal,
        ],
        "data_type": _pixel_type_to_zarr_dtype(asset.pixel_value_type),
    }

    codec_config = asset.codec_configuration()
    if codec_config is not None:
        # Build a single codec entry from the configuration map.
        configuration: dict = {}
        codec_name: str | None = None
        for key, value in codec_config.items():
            if key == "main_header":
                configuration[key] = base64.b64encode(value).decode("ascii")
            elif isinstance(value, (bytes, bytearray)) and len(value) == 1:
                configuration[key] = value[0]
            elif isinstance(value, (bytes, bytearray)):
                # Try to decode as ASCII text; fall back to base64
                try:
                    configuration[key] = value.decode("ascii")
                except UnicodeDecodeError:
                    configuration[key] = base64.b64encode(value).decode("ascii")
            else:
                configuration[key] = value
            if codec_name is None:
                codec_name = key  # use first key as a fallback name
        # Use the first key as the codec name if no explicit name is provided.
        zarray["codecs"] = [
            {"name": codec_name or "unknown", "configuration": configuration}
        ]

    return json.dumps(zarray)


def _build_tile_refs(
    asset: ImageAssetProvider,
    segment_key: str,
    source_uri: str,
) -> dict[str, list]:
    """Build chunk-key → ``[uri, offset, length]`` entries for one segment.

    Parameters
    ----------
    asset:
        The :class:`ImageAssetProvider` for the segment.
    segment_key:
        The asset key string (e.g. ``"image_segment_0"``).
    source_uri:
        The cloud URI to embed in every tile reference.

    Returns
    -------
    dict
        Mapping of ``"{segment_key}/{row}.{col}"`` to
        ``[source_uri, byte_offset, byte_length]``.
    """
    byte_ranges = asset.tile_byte_ranges()
    if byte_ranges is None:
        return {}

    refs: dict[str, list] = {}
    for (row, col), (offset, length) in byte_ranges.items():
        chunk_key = f"{segment_key}/{row}.{col}"
        refs[chunk_key] = [source_uri, offset, length]
    return refs


# ---------------------------------------------------------------------------
# TileIndex class
# ---------------------------------------------------------------------------


class TileIndex:
    """Generates, loads, and saves Kerchunk v1 tile index references."""

    def __init__(self, refs: dict) -> None:
        """Initialise with a pre-built Kerchunk v1 reference dict.

        Parameters
        ----------
        refs:
            A dictionary conforming to the Kerchunk v1 specification,
            containing ``"version"`` and ``"refs"`` keys.
        """
        self._refs = refs

    # -- Properties ---------------------------------------------------------

    @property
    def refs(self) -> dict:
        """The Kerchunk v1 reference dictionary."""
        return self._refs

    @property
    def num_segments(self) -> int:
        """Count of image segments in the index.

        Determined by counting keys in ``refs["refs"]`` that end with
        ``.zarray``.
        """
        inner = self._refs.get("refs", {})
        return sum(1 for k in inner if k.endswith(".zarray"))

    @property
    def num_tiles(self) -> int:
        """Total count of tile references across all segments.

        Determined by counting values in ``refs["refs"]`` that are
        three-element lists (``[uri, offset, length]``).
        """
        inner = self._refs.get("refs", {})
        return sum(1 for v in inner.values() if isinstance(v, list) and len(v) == 3)

    # -- Class methods ------------------------------------------------------

    @classmethod
    def generate(
        cls,
        path: str,
        *,
        source_uri: str,
        segments: list[str] | None = None,
    ) -> "TileIndex":
        """Generate a tile index from a local imagery file.

        Parameters
        ----------
        path:
            Local file path to the source imagery.
        source_uri:
            Cloud URI written into tile references (e.g.
            ``"s3://bucket/image.ntf"``).
        segments:
            Optional list of asset key strings to process. If ``None``,
            all image segments are indexed.

        Returns
        -------
        TileIndex
            A new instance containing the assembled Kerchunk v1 references.

        Raises
        ------
        FileNotFoundError
            If *path* does not exist on the local filesystem.
        KeyError
            If *segments* contains a key not present in the dataset.
        ValueError
            If no image segments provide tile byte ranges.
        """
        if not os.path.exists(path):
            raise FileNotFoundError(f"File not found: {path}")

        with IO.open([path], "r") as reader:
            all_keys = reader.get_asset_keys(asset_type=AssetType.Image)

            if segments is not None:
                for key in segments:
                    if key not in all_keys:
                        raise KeyError(
                            f"Segment key '{key}' not found in dataset. "
                            f"Available: {all_keys}"
                        )
                selected_keys = segments
            else:
                selected_keys = all_keys

            # Build dataset-level metadata
            zattrs: dict = {"source": source_uri}
            detected = _detect_format(path)
            if detected is not None:
                zattrs["format"] = detected

            inner_refs: dict = {
                ".zgroup": json.dumps({"zarr_format": 3}),
                ".zattrs": json.dumps(zattrs),
            }

            indexed_count = 0
            for key in selected_keys:
                asset = reader.get_asset(key)
                byte_ranges = asset.tile_byte_ranges()
                if byte_ranges is None:
                    continue

                indexed_count += 1
                inner_refs[f"{key}/.zarray"] = _build_zarray(asset)
                tile_refs = _build_tile_refs(asset, key, source_uri)
                inner_refs.update(tile_refs)

            if indexed_count == 0:
                raise ValueError(
                    f"No indexable image segments found in {path}"
                )

        return cls({"version": 1, "refs": inner_refs})

    @classmethod
    def load(cls, path: str) -> "TileIndex":
        """Load a tile index from a JSON or Parquet file.

        Parameters
        ----------
        path:
            Path to the index file (``.json`` or ``.parquet``).

        Returns
        -------
        TileIndex
            A new instance containing the loaded Kerchunk v1 references.

        Raises
        ------
        FileNotFoundError
            If *path* does not exist on the local filesystem.
        ValueError
            If the file extension is unsupported or the loaded data does
            not contain a valid Kerchunk v1 ``version`` field equal to 1.
        ImportError
            If ``.parquet`` is requested but *pyarrow* is not installed.
        """
        if not os.path.exists(path):
            raise FileNotFoundError(f"File not found: {path}")

        _, ext = os.path.splitext(path)
        ext = ext.lower()

        if ext == ".json":
            return cls._load_json(path)
        elif ext == ".parquet":
            return cls._load_parquet(path)
        else:
            raise ValueError(
                f"Unsupported file extension '{ext}'. "
                "Supported: .json, .parquet"
            )

    @classmethod
    def _load_json(cls, path: str) -> "TileIndex":
        """Deserialize a Kerchunk v1 reference dict from a JSON file."""
        with open(path, "r") as f:
            data = json.load(f)

        v = data.get("version")
        if v != 1:
            raise ValueError(
                f"Invalid Kerchunk reference: expected version 1, got {v}"
            )

        return cls(data)

    @classmethod
    def _load_parquet(cls, path: str) -> "TileIndex":
        """Deserialize a Kerchunk v1 reference dict from a Parquet file."""
        _import_pyarrow()
        import pyarrow.parquet as pq

        table = pq.read_table(path)
        file_metadata = table.schema.metadata or {}

        # Validate version from file metadata
        version_bytes = file_metadata.get(b"version")
        v = int(version_bytes.decode("utf-8")) if version_bytes else None
        if v != 1:
            raise ValueError(
                f"Invalid Kerchunk reference: expected version 1, got {v}"
            )

        # Reconstruct inline metadata entries from file metadata
        inner_refs: dict = {}
        for key_bytes, value_bytes in file_metadata.items():
            key = key_bytes.decode("utf-8")
            # Skip the version key and any pyarrow internal metadata keys
            if key == "version" or key == "pandas" or key.startswith("ARROW:"):
                continue
            inner_refs[key] = value_bytes.decode("utf-8")

        # Reconstruct tile references from table rows
        paths = table.column("path").to_pylist()
        urls = table.column("url").to_pylist()
        offsets = table.column("offset").to_pylist()
        sizes = table.column("size").to_pylist()

        for i in range(table.num_rows):
            inner_refs[paths[i]] = [urls[i], offsets[i], sizes[i]]

        return cls({"version": 1, "refs": inner_refs})

    # -- Instance methods ---------------------------------------------------

    def save(self, path: str) -> None:
        """Save the tile index to a JSON or Parquet file.

        Parameters
        ----------
        path:
            Output file path. The extension determines the format:
            ``.json`` for JSON, ``.parquet`` for Parquet.

        Raises
        ------
        ValueError
            If the file extension is not ``.json`` or ``.parquet``.
        ImportError
            If ``.parquet`` is requested but *pyarrow* is not installed.
        """
        _, ext = os.path.splitext(path)
        ext = ext.lower()

        if ext == ".json":
            self._save_json(path)
        elif ext == ".parquet":
            self._save_parquet(path)
        else:
            raise ValueError(
                f"Unsupported file extension '{ext}'. "
                "Supported: .json, .parquet"
            )

    def _save_json(self, path: str) -> None:
        """Serialize the refs dict to a JSON file."""
        with open(path, "w") as f:
            json.dump(self._refs, f)

    def _save_parquet(self, path: str) -> None:
        """Serialize the tile index to a Parquet file.

        Inline metadata entries (``.zgroup``, ``.zattrs``, ``.zarray``)
        are stored in the Parquet file metadata (key-value footer).
        Tile references are stored as a table with columns:
        ``path``, ``url``, ``offset``, ``size``.
        """
        pa = _import_pyarrow()
        import pyarrow.parquet as pq

        inner = self._refs.get("refs", {})

        # Separate inline metadata from tile references
        metadata: dict[bytes, bytes] = {}
        paths: list[str] = []
        urls: list[str] = []
        offsets: list[int] = []
        sizes: list[int] = []

        for key, value in inner.items():
            if isinstance(value, list) and len(value) == 3:
                # Tile reference: [url, offset, size]
                paths.append(key)
                urls.append(value[0])
                offsets.append(value[1])
                sizes.append(value[2])
            else:
                # Inline metadata (JSON string)
                encoded_val = value.encode("utf-8") if isinstance(value, str) else json.dumps(value).encode("utf-8")
                metadata[key.encode("utf-8")] = encoded_val

        # Store version in file metadata as well
        metadata[b"version"] = str(self._refs.get("version", 1)).encode("utf-8")

        # Build the table
        schema = pa.schema(
            [
                pa.field("path", pa.string()),
                pa.field("url", pa.string()),
                pa.field("offset", pa.int64()),
                pa.field("size", pa.int64()),
            ],
            metadata=metadata,
        )

        table = pa.table(
            {
                "path": paths,
                "url": urls,
                "offset": offsets,
                "size": sizes,
            },
            schema=schema,
        )

        pq.write_table(table, path)
