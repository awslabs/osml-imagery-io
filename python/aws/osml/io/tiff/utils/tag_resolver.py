"""Tag name resolver for TIFF / GeoTIFF metadata dictionaries.

Provides convenient name-based access to TIFF tag values stored under
numeric string keys in a Tag_Dictionary.
"""

from __future__ import annotations

from typing import Any, Dict, Iterator, Optional, Tuple


class TagNameResolver:
    """Resolve TIFF tag names to numeric IDs for convenient metadata access.

    Wraps a Tag_Dictionary (from MetadataProvider.entries()) and provides
    lookup by human-readable tag name via a configurable name-to-number mapping.

    Keys that are not present in the mapping are passed through unchanged,
    mirroring the behaviour of :meth:`__iter__` which exposes unmapped keys
    directly.

    Example::

        meta = reader.metadata.entries()
        resolver = TagNameResolver(meta)
        width = resolver["ImageWidth"]       # looks up key "256"
        crs = resolver.by_number(34735)      # direct numeric access
        comp = resolver.get("Compression")   # returns None if absent
    """

    # Enumerated tag values: maps (tag_number, value_name) → numeric value.
    # Used by __setitem__ to resolve human-readable value names to the
    # integers the Rust writer expects.
    VALUE_MAPPING: Dict[int, Dict[str, int]] = {
        # Tag 259 – Compression
        259: {
            "none": 1,
            "ccittrle": 2,
            "ccittfax3": 3,
            "ccittfax4": 4,
            "lzw": 5,
            "ojpeg": 6,
            "jpeg": 7,
            "deflate": 8,
            "packbits": 32773,
        },
        # Tag 262 – PhotometricInterpretation
        262: {
            "miniswhite": 0,
            "minisblack": 1,
            "rgb": 2,
            "palette": 3,
            "mask": 4,
            "ycbcr": 6,
        },
        # Tag 274 – Orientation
        274: {
            "topleft": 1,
            "topright": 2,
            "bottomright": 3,
            "bottomleft": 4,
            "lefttop": 5,
            "righttop": 6,
            "rightbottom": 7,
            "leftbottom": 8,
        },
        # Tag 284 – PlanarConfiguration
        284: {
            "chunky": 1,
            "planar": 2,
        },
        # Tag 317 – Predictor
        317: {
            "none": 1,
            "horizontal": 2,
            "floatingpoint": 3,
        },
        # Tag 339 – SampleFormat
        339: {
            "uint": 1,
            "int": 2,
            "float": 3,
            "void": 4,
        },
    }

    DEFAULT_MAPPING: Dict[str, int] = {
        # Baseline TIFF 6.0 tags
        "NewSubfileType": 254,
        "SubfileType": 255,
        "ImageWidth": 256,
        "ImageLength": 257,
        "BitsPerSample": 258,
        "Compression": 259,
        "PhotometricInterpretation": 262,
        "Threshholding": 263,
        "CellWidth": 264,
        "CellLength": 265,
        "FillOrder": 266,
        "DocumentName": 269,
        "ImageDescription": 270,
        "Make": 271,
        "Model": 272,
        "StripOffsets": 273,
        "Orientation": 274,
        "SamplesPerPixel": 277,
        "RowsPerStrip": 278,
        "StripByteCounts": 279,
        "MinSampleValue": 280,
        "MaxSampleValue": 281,
        "XResolution": 282,
        "YResolution": 283,
        "PlanarConfiguration": 284,
        "PageName": 285,
        "FreeOffsets": 288,
        "FreeByteCounts": 289,
        "GrayResponseUnit": 290,
        "GrayResponseCurve": 291,
        "ResolutionUnit": 296,
        "PageNumber": 297,
        "Software": 305,
        "DateTime": 306,
        "Artist": 315,
        "HostComputer": 316,
        "Predictor": 317,
        "WhitePoint": 318,
        "PrimaryChromaticities": 319,
        "ColorMap": 320,
        "HalftoneHints": 321,
        "TileWidth": 322,
        "TileLength": 323,
        "TileOffsets": 324,
        "TileByteCounts": 325,
        "SubIFDs": 330,
        "InkSet": 332,
        "InkNames": 333,
        "NumberOfInks": 334,
        "DotRange": 336,
        "TargetPrinter": 337,
        "ExtraSamples": 338,
        "SampleFormat": 339,
        "SMinSampleValue": 340,
        "SMaxSampleValue": 341,
        "JPEGTables": 347,
        "Copyright": 33432,
        # GeoTIFF tags
        "ModelPixelScale": 33550,
        "ModelTiepoint": 33922,
        "ModelTransformation": 34264,
        "GeoKeyDirectory": 34735,
        "GeoDoubleParams": 34736,
        "GeoAsciiParams": 34737,
        # GDAL tags
        "GDALMetadata": 42112,
        "GDALNoData": 42113,
    }

    def __init__(
        self,
        tag_dict: Dict[str, Any],
        custom_mapping: Optional[Dict[str, int]] = None,
    ) -> None:
        self._tag_dict = tag_dict
        self._mapping: Dict[str, int] = {**self.DEFAULT_MAPPING}
        if custom_mapping:
            self._mapping.update(custom_mapping)

    def _resolve_key(self, name: str) -> str:
        """Return the numeric string key for *name*.

        If *name* is in the mapping it is resolved to ``str(tag_number)``.
        Otherwise *name* is returned unchanged so that unmapped keys pass
        through transparently.
        """
        if name in self._mapping:
            return str(self._mapping[name])
        return name

    def __getitem__(self, name: str) -> Any:
        """Look up a tag value by human-readable name.

        If *name* is in the mapping it is resolved to the corresponding
        numeric key.  Otherwise *name* is used directly as the dictionary
        key, allowing unmapped keys to pass through.

        Raises:
            KeyError: If the resolved key is not present in the underlying
                dictionary.
        """
        key = self._resolve_key(name)
        if key not in self._tag_dict:
            if name in self._mapping:
                raise KeyError(f"Tag {name!r} ({self._mapping[name]}) not present in metadata")
            raise KeyError(name)
        return self._tag_dict[key]

    def get(self, name: str, default: Any = None) -> Any:
        """Look up a tag value by name, returning *default* if not found."""
        try:
            return self[name]
        except KeyError:
            return default

    def by_number(self, tag_number: int) -> Any:
        """Retrieve a tag by its numeric key directly.

        Raises:
            KeyError: If the tag number is not present in the dictionary.
        """
        key = str(tag_number)
        if key not in self._tag_dict:
            raise KeyError(f"Tag {tag_number} not present in metadata")
        return self._tag_dict[key]

    def __iter__(self) -> Iterator[Tuple[str, Any]]:
        """Iterate over all (resolved_name, value) pairs.

        Keys are resolved to human-readable tag names when a mapping exists.
        Tags without a known name are yielded with their numeric string key.
        """
        reverse = {str(v): k for k, v in self._mapping.items()}
        for key, value in self._tag_dict.items():
            yield (reverse.get(key, key), value)

    def __len__(self) -> int:
        """Return the number of entries in the underlying Tag_Dictionary."""
        return len(self._tag_dict)

    def __contains__(self, name: str) -> bool:
        """Check if a tag name is present in the metadata.

        Returns ``True`` when the resolved key exists in the underlying
        dictionary.  For mapped names this checks the numeric key; for
        unmapped names the raw key is checked directly.
        """
        key = self._resolve_key(name)
        return key in self._tag_dict

    def _resolve_value(self, tag_number: int, value: Any) -> Any:
        """Resolve a human-readable value name to its numeric equivalent.

        For tags with well-known enumerated values (e.g. Compression,
        PhotometricInterpretation), string values are looked up
        case-insensitively in :attr:`VALUE_MAPPING` and replaced with the
        corresponding integer.  Non-string values and strings that don't
        match any known name are returned unchanged.
        """
        if not isinstance(value, str) or tag_number not in self.VALUE_MAPPING:
            return value
        lookup = value.lower()
        enum_map = self.VALUE_MAPPING[tag_number]
        if lookup in enum_map:
            return enum_map[lookup]
        return value

    def __setitem__(self, name: str, value: Any) -> None:
        """Set a tag value by human-readable name.

        If *name* is in the mapping it is resolved to the corresponding
        numeric key.  If the tag has well-known enumerated values (see
        :attr:`VALUE_MAPPING`), string values are resolved to their numeric
        equivalents automatically.  Otherwise *name* and *value* are used
        as-is, allowing unmapped keys to pass through.

        Examples::

            resolver["Compression"] = "LZW"       # stored as 5
            resolver["Compression"] = 5            # stored as 5
            resolver["Compression"] = "Deflate"    # stored as 8
            resolver["TileWidth"] = 512            # stored as 512
        """
        key = self._resolve_key(name)
        # Resolve enumerated string values when the tag number is known
        if name in self._mapping:
            value = self._resolve_value(self._mapping[name], value)
        self._tag_dict[key] = value

    def set(self, name: str, value: Any) -> None:
        """Set a tag value by name.

        Convenience wrapper around ``__setitem__``.
        """
        self[name] = value
