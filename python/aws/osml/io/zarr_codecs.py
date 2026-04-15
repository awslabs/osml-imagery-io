"""Zarr codec plugins for JPEG 2000, JPEG, and uncompressed JBP/NITF imagery.

Codec classes subclass the zarr-python v3 ``BytesBytesCodec`` ABC and are
registered via entry points in ``pyproject.toml``.  The zarr v3 codec pipeline
discovers them automatically when it encounters the corresponding URI-based
codec name in ``.zarray`` metadata.

Usage (automatic via entry points — no import needed):
    import xarray as xr
    ds = xr.open_zarr("reference://", storage_options={"fo": "index.json"})

Usage (explicit import):
    from aws.osml.io.zarr_codecs import Jpeg2000Codec, JpegCodec, JbpBlockCodec
"""

from __future__ import annotations

import asyncio
import base64
from dataclasses import dataclass

from aws.osml.io._io import decode_jbp_block, decode_jpeg, decode_jpeg2000, decode_tiff_tile

__all__ = [
    "Jpeg2000Codec",
    "JpegCodec",
    "JbpBlockCodec",
    "TiffTileCodec",
    "decode_jpeg2000",
    "decode_jpeg",
    "decode_jbp_block",
    "decode_tiff_tile",
]


def _import_zarr():
    """Lazily import zarr, raising ImportError with install instructions if missing."""
    try:
        import zarr

        return zarr
    except ImportError:
        raise ImportError(
            "zarr>=3.0 is required for Zarr codec support. "
            "Install with: pip install osml-imagery-io[zarr]"
        )


def _import_zarr_bytescodec():
    """Return the ``BytesBytesCodec`` ABC, raising ``ImportError`` if zarr is missing."""
    _import_zarr()
    from zarr.abc.codec import BytesBytesCodec

    return BytesBytesCodec


@dataclass(frozen=True)
class Jpeg2000Codec(_import_zarr_bytescodec()):
    """Zarr v3 bytes-to-bytes codec for JPEG 2000 codestreams.

    Registered as: https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000

    Configuration:
        main_header: Optional[str]  — base64-encoded J2K main header bytes
        resolution_level: int       — target resolution level (default 0)
    """

    codec_name = "https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000"
    codec_id = "https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000"
    is_fixed_size = False

    main_header: str | None = None
    resolution_level: int = 0

    def __init__(self, *, main_header: str | None = None, resolution_level: int = 0):
        if main_header is not None:
            try:
                main_header_bytes = base64.b64decode(main_header)
            except Exception as e:
                raise ValueError(f"Invalid base64 in main_header: {e}")
        else:
            main_header_bytes = None

        object.__setattr__(self, "resolution_level", resolution_level)
        object.__setattr__(self, "main_header", main_header)
        object.__setattr__(self, "_main_header_bytes", main_header_bytes)

    def _decode_sync(self, chunk_bytes, chunk_spec):
        """Synchronous decode — delegates to the Rust JPEG 2000 decoder."""
        from zarr.core.buffer.cpu import as_numpy_array_wrapper

        expected_shape = chunk_spec.shape

        def _decode_and_pad(buf):
            arr = decode_jpeg2000(
                bytes(buf),
                main_header=self._main_header_bytes,
                resolution_level=self.resolution_level,
            )
            # Edge tiles may decode to smaller dimensions than the chunk shape.
            # Pad to the expected chunk shape so zarr's reshape succeeds; zarr
            # will trim the padding when the array boundary is reached.
            if arr.shape != expected_shape:
                import numpy as np

                padded = np.zeros(expected_shape, dtype=arr.dtype)
                slices = tuple(slice(0, s) for s in arr.shape)
                padded[slices] = arr
                return padded.tobytes()
            return arr.tobytes()

        return as_numpy_array_wrapper(
            _decode_and_pad,
            chunk_bytes,
            chunk_spec.prototype,
        )

    async def _decode_single(self, chunk_bytes, chunk_spec):
        """Decode JPEG 2000 chunk bytes into pixel data.

        Args:
            chunk_bytes: Buffer containing J2K codestream data.
            chunk_spec: ArraySpec describing the expected output.

        Returns:
            Buffer with decoded pixel data.
        """
        return await asyncio.to_thread(self._decode_sync, chunk_bytes, chunk_spec)

    async def _encode_single(self, chunk_bytes, chunk_spec):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("Jpeg2000Codec: encoding is not supported")

    def compute_encoded_size(self, input_byte_length: int, chunk_spec) -> int:
        """Return *input_byte_length* — compressed size is not predictable."""
        return input_byte_length

    def evolve_from_array_spec(self, array_spec):
        """Codec configuration is fixed at construction time."""
        return self

    def to_dict(self):
        """Serialize codec configuration to a JSON-compatible dictionary.

        Returns:
            dict with 'name' and 'configuration' keys.
        """
        return {
            "name": self.codec_name,
            "configuration": {
                "main_header": self.main_header,
                "resolution_level": self.resolution_level,
            },
        }

    @classmethod
    def from_dict(cls, data):
        """Construct a Jpeg2000Codec from a serialized configuration dictionary.

        Accepts both ``{"name": ..., "configuration": {...}}`` format and a
        flat configuration dictionary.

        Args:
            data: Configuration dictionary.

        Returns:
            Jpeg2000Codec instance.
        """
        config = data.get("configuration", data)
        return cls(
            main_header=config.get("main_header"),
            resolution_level=config.get("resolution_level", 0),
        )

    # -- numcodecs compatibility (consumer path) ---------------------------

    def decode(self, buf, out=None):
        """Synchronous decode for numcodecs filter protocol.

        For edge tiles, the decoded array may be smaller than the nominal tile
        dimensions. Pad to the nominal tile size so zarr v2's reshape succeeds.
        """
        import struct

        import numpy as np

        data = bytes(buf) if not isinstance(buf, bytes) else buf
        arr = decode_jpeg2000(
            data,
            main_header=self._main_header_bytes,
            resolution_level=self.resolution_level,
        )

        # If we have a main_header, compute the nominal tile size and pad if needed.
        if self._main_header_bytes is not None and len(self._main_header_bytes) >= 40:
            hdr = self._main_header_bytes
            # SIZ fields start at offset 2 (after SOC marker)
            xtsiz = struct.unpack(">I", hdr[24:28])[0]
            ytsiz = struct.unpack(">I", hdr[28:32])[0]
            scale = 1 << self.resolution_level
            nominal_tw = -(-xtsiz // scale)  # ceil division
            nominal_th = -(-ytsiz // scale)
            bands = arr.shape[0]
            nominal_shape = (bands, nominal_th, nominal_tw)
            if arr.shape != nominal_shape:
                padded = np.zeros(nominal_shape, dtype=arr.dtype)
                slices = tuple(slice(0, s) for s in arr.shape)
                padded[slices] = arr
                return padded
        return arr

    def encode(self, buf):
        """Encoding is not supported."""
        raise NotImplementedError("Jpeg2000Codec: encoding is not supported")

    def get_config(self):
        """Return numcodecs-compatible configuration dict."""
        return {
            "id": self.codec_id,
            "main_header": self.main_header,
            "resolution_level": self.resolution_level,
        }

    @classmethod
    def from_config(cls, config):
        """Construct from a numcodecs configuration dict."""
        return cls(
            main_header=config.get("main_header"),
            resolution_level=config.get("resolution_level", 0),
        )


@dataclass(frozen=True)
class JpegCodec(_import_zarr_bytescodec()):
    """Zarr v3 bytes-to-bytes codec for JPEG streams.

    Registered as: https://awslabs.github.io/osml-imagery-io/codecs/jpeg

    Configuration:
        bits_per_pixel: int   — 8 or 12
        num_bands: int        — number of bands
        block_width: int      — block width in pixels
        block_height: int     — block height in pixels
        imode: str            — interleave mode ("B", "P", "R", or "S")
        color_space: str      — "MONO", "RGB", or "YCbCr601"
    """

    codec_name = "https://awslabs.github.io/osml-imagery-io/codecs/jpeg"
    codec_id = "https://awslabs.github.io/osml-imagery-io/codecs/jpeg"
    is_fixed_size = False

    bits_per_pixel: int = 8
    num_bands: int = 1
    block_width: int = 256
    block_height: int = 256
    imode: str = "B"
    color_space: str = "MONO"

    def __init__(self, *, bits_per_pixel, num_bands, block_width, block_height, imode, color_space):
        object.__setattr__(self, "bits_per_pixel", bits_per_pixel)
        object.__setattr__(self, "num_bands", num_bands)
        object.__setattr__(self, "block_width", block_width)
        object.__setattr__(self, "block_height", block_height)
        object.__setattr__(self, "imode", imode)
        object.__setattr__(self, "color_space", color_space)

    def _decode_sync(self, chunk_bytes, chunk_spec):
        """Synchronous decode — delegates to the Rust JPEG decoder."""
        from zarr.core.buffer.cpu import as_numpy_array_wrapper

        return as_numpy_array_wrapper(
            lambda buf: decode_jpeg(
                bytes(buf),
                bits_per_pixel=self.bits_per_pixel,
                num_bands=self.num_bands,
                block_width=self.block_width,
                block_height=self.block_height,
                imode=self.imode,
                color_space=self.color_space,
            ).tobytes(),
            chunk_bytes,
            chunk_spec.prototype,
        )

    async def _decode_single(self, chunk_bytes, chunk_spec):
        """Decode JPEG chunk bytes into pixel data.

        Args:
            chunk_bytes: Buffer containing JPEG stream data.
            chunk_spec: ArraySpec describing the expected output.

        Returns:
            Buffer with decoded pixel data.
        """
        return await asyncio.to_thread(self._decode_sync, chunk_bytes, chunk_spec)

    async def _encode_single(self, chunk_bytes, chunk_spec):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("JpegCodec: encoding is not supported")

    def compute_encoded_size(self, input_byte_length: int, chunk_spec) -> int:
        """Return *input_byte_length* — compressed size is not predictable."""
        return input_byte_length

    def evolve_from_array_spec(self, array_spec):
        """Codec configuration is fixed at construction time."""
        return self

    def to_dict(self):
        """Serialize codec configuration to a JSON-compatible dictionary.

        Returns:
            dict with 'name' and 'configuration' keys.
        """
        return {
            "name": self.codec_name,
            "configuration": {
                "bits_per_pixel": self.bits_per_pixel,
                "num_bands": self.num_bands,
                "block_width": self.block_width,
                "block_height": self.block_height,
                "imode": self.imode,
                "color_space": self.color_space,
            },
        }

    @classmethod
    def from_dict(cls, data):
        """Construct a JpegCodec from a serialized configuration dictionary.

        Accepts both ``{"name": ..., "configuration": {...}}`` format and a
        flat configuration dictionary.

        Args:
            data: Configuration dictionary.

        Returns:
            JpegCodec instance.

        Raises:
            ValueError: If required configuration fields are missing.
        """
        config = data.get("configuration", data)
        required = ["bits_per_pixel", "num_bands", "block_width", "block_height", "imode", "color_space"]
        missing = [f for f in required if f not in config]
        if missing:
            raise ValueError(f"Missing required configuration field(s): {', '.join(missing)}")
        return cls(
            bits_per_pixel=config["bits_per_pixel"],
            num_bands=config["num_bands"],
            block_width=config["block_width"],
            block_height=config["block_height"],
            imode=config["imode"],
            color_space=config["color_space"],
        )

    # -- numcodecs compatibility (consumer path) ---------------------------

    def decode(self, buf, out=None):
        """Synchronous decode for numcodecs filter protocol."""
        data = bytes(buf) if not isinstance(buf, bytes) else buf
        return decode_jpeg(
            data,
            bits_per_pixel=self.bits_per_pixel,
            num_bands=self.num_bands,
            block_width=self.block_width,
            block_height=self.block_height,
            imode=self.imode,
            color_space=self.color_space,
        )

    def encode(self, buf):
        """Encoding is not supported."""
        raise NotImplementedError("JpegCodec: encoding is not supported")

    def get_config(self):
        """Return numcodecs-compatible configuration dict."""
        return {
            "id": self.codec_id,
            "bits_per_pixel": self.bits_per_pixel,
            "num_bands": self.num_bands,
            "block_width": self.block_width,
            "block_height": self.block_height,
            "imode": self.imode,
            "color_space": self.color_space,
        }

    @classmethod
    def from_config(cls, config):
        """Construct from a numcodecs configuration dict."""
        return cls(
            bits_per_pixel=config["bits_per_pixel"],
            num_bands=config["num_bands"],
            block_width=config["block_width"],
            block_height=config["block_height"],
            imode=config["imode"],
            color_space=config["color_space"],
        )


@dataclass(frozen=True)
class JbpBlockCodec(_import_zarr_bytescodec()):
    """Zarr v3 bytes-to-bytes codec for uncompressed JBP/NITF/NSIF image blocks.

    Registered as: https://awslabs.github.io/osml-imagery-io/codecs/jbp-block

    Configuration:
        num_bands: int    — number of bands
        block_height: int — block height in pixels
        block_width: int  — block width in pixels
        nbpp: int         — bits per pixel per band
        imode: str        — NITF interleave mode ("B", "P", "R", or "S")
        pvtype: str       — NITF pixel value type ("INT", "SI", "R", or "C")
    """

    codec_name = "https://awslabs.github.io/osml-imagery-io/codecs/jbp-block"
    codec_id = "https://awslabs.github.io/osml-imagery-io/codecs/jbp-block"
    is_fixed_size = False

    num_bands: int = 1
    block_height: int = 256
    block_width: int = 256
    nbpp: int = 8
    imode: str = "B"
    pvtype: str = "INT"

    def __init__(self, *, num_bands, block_height, block_width, nbpp, imode, pvtype):
        object.__setattr__(self, "num_bands", num_bands)
        object.__setattr__(self, "block_height", block_height)
        object.__setattr__(self, "block_width", block_width)
        object.__setattr__(self, "nbpp", nbpp)
        object.__setattr__(self, "imode", imode)
        object.__setattr__(self, "pvtype", pvtype)

    def _decode_sync(self, chunk_bytes, chunk_spec):
        """Synchronous decode — delegates to the Rust JBP block decoder."""
        from zarr.core.buffer.cpu import as_numpy_array_wrapper

        return as_numpy_array_wrapper(
            lambda buf: decode_jbp_block(
                bytes(buf),
                num_bands=self.num_bands,
                block_height=self.block_height,
                block_width=self.block_width,
                nbpp=self.nbpp,
                imode=self.imode,
                pvtype=self.pvtype,
            ).tobytes(),
            chunk_bytes,
            chunk_spec.prototype,
        )

    async def _decode_single(self, chunk_bytes, chunk_spec):
        """Decode JBP/NITF chunk bytes into pixel data.

        Args:
            chunk_bytes: Buffer containing raw pixel data.
            chunk_spec: ArraySpec describing the expected output.

        Returns:
            Buffer with decoded pixel data.
        """
        return await asyncio.to_thread(self._decode_sync, chunk_bytes, chunk_spec)

    async def _encode_single(self, chunk_bytes, chunk_spec):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("JbpBlockCodec: encoding is not supported")

    def compute_encoded_size(self, input_byte_length: int, chunk_spec) -> int:
        """Return *input_byte_length* — compressed size is not predictable."""
        return input_byte_length

    def evolve_from_array_spec(self, array_spec):
        """Codec configuration is fixed at construction time."""
        return self

    def to_dict(self):
        """Serialize codec configuration to a JSON-compatible dictionary.

        Returns:
            dict with 'name' and 'configuration' keys.
        """
        return {
            "name": self.codec_name,
            "configuration": {
                "num_bands": self.num_bands,
                "block_height": self.block_height,
                "block_width": self.block_width,
                "nbpp": self.nbpp,
                "imode": self.imode,
                "pvtype": self.pvtype,
            },
        }

    @classmethod
    def from_dict(cls, data):
        """Construct a JbpBlockCodec from a serialized configuration dictionary.

        Accepts both ``{"name": ..., "configuration": {...}}`` format and a
        flat configuration dictionary.

        Args:
            data: Configuration dictionary.

        Returns:
            JbpBlockCodec instance.

        Raises:
            ValueError: If required configuration fields are missing.
        """
        config = data.get("configuration", data)
        required = ["num_bands", "block_height", "block_width", "nbpp", "imode", "pvtype"]
        missing = [f for f in required if f not in config]
        if missing:
            raise ValueError(f"Missing required configuration field(s): {', '.join(missing)}")
        return cls(
            num_bands=config["num_bands"],
            block_height=config["block_height"],
            block_width=config["block_width"],
            nbpp=config["nbpp"],
            imode=config["imode"],
            pvtype=config["pvtype"],
        )

    # -- numcodecs compatibility (consumer path) ---------------------------

    def decode(self, buf, out=None):
        """Synchronous decode for numcodecs filter protocol."""
        data = bytes(buf) if not isinstance(buf, bytes) else buf
        return decode_jbp_block(
            data,
            num_bands=self.num_bands,
            block_height=self.block_height,
            block_width=self.block_width,
            nbpp=self.nbpp,
            imode=self.imode,
            pvtype=self.pvtype,
        )

    def encode(self, buf):
        """Encoding is not supported."""
        raise NotImplementedError("JbpBlockCodec: encoding is not supported")

    def get_config(self):
        """Return numcodecs-compatible configuration dict."""
        return {
            "id": self.codec_id,
            "num_bands": self.num_bands,
            "block_height": self.block_height,
            "block_width": self.block_width,
            "nbpp": self.nbpp,
            "imode": self.imode,
            "pvtype": self.pvtype,
        }

    @classmethod
    def from_config(cls, config):
        """Construct from a numcodecs configuration dict."""
        return cls(
            num_bands=config["num_bands"],
            block_height=config["block_height"],
            block_width=config["block_width"],
            nbpp=config["nbpp"],
            imode=config["imode"],
            pvtype=config["pvtype"],
        )


@dataclass(frozen=True)
class TiffTileCodec(_import_zarr_bytescodec()):
    """Zarr v3 bytes-to-bytes codec for TIFF tile codestreams.

    Registered as: https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile

    Configuration:
        compression: int          — TIFF compression tag value (default 1)
        bits_per_sample: int      — bits per sample (default 8)
        samples_per_pixel: int    — number of bands (default 1)
        photometric: int          — photometric interpretation (default 1)
        planar_config: int        — planar configuration (default 1)
        predictor: int            — differencing predictor (default 1)
        tile_width: int           — tile width in pixels (default 256)
        tile_height: int          — tile height in pixels (default 256)
        sample_format: int        — sample format (default 1)
        jpeg_tables: str | None   — base64-encoded JPEG tables (default None)
    """

    codec_name = "https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile"
    codec_id = "https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile"
    is_fixed_size = False

    compression: int = 1
    bits_per_sample: int = 8
    samples_per_pixel: int = 1
    photometric: int = 1
    planar_config: int = 1
    predictor: int = 1
    tile_width: int = 256
    tile_height: int = 256
    sample_format: int = 1
    jpeg_tables: str | None = None

    def __init__(
        self,
        *,
        compression: int = 1,
        bits_per_sample: int = 8,
        samples_per_pixel: int = 1,
        photometric: int = 1,
        planar_config: int = 1,
        predictor: int = 1,
        tile_width: int = 256,
        tile_height: int = 256,
        sample_format: int = 1,
        jpeg_tables: str | None = None,
    ):
        if jpeg_tables is not None:
            try:
                jpeg_tables_bytes = base64.b64decode(jpeg_tables)
            except Exception as e:
                raise ValueError(f"Invalid base64 in jpeg_tables: {e}")
        else:
            jpeg_tables_bytes = None

        object.__setattr__(self, "compression", compression)
        object.__setattr__(self, "bits_per_sample", bits_per_sample)
        object.__setattr__(self, "samples_per_pixel", samples_per_pixel)
        object.__setattr__(self, "photometric", photometric)
        object.__setattr__(self, "planar_config", planar_config)
        object.__setattr__(self, "predictor", predictor)
        object.__setattr__(self, "tile_width", tile_width)
        object.__setattr__(self, "tile_height", tile_height)
        object.__setattr__(self, "sample_format", sample_format)
        object.__setattr__(self, "jpeg_tables", jpeg_tables)
        object.__setattr__(self, "_jpeg_tables_bytes", jpeg_tables_bytes)

    def _decode_sync(self, chunk_bytes, chunk_spec):
        """Synchronous decode — delegates to the Rust TIFF tile decoder."""
        from zarr.core.buffer.cpu import as_numpy_array_wrapper

        expected_shape = chunk_spec.shape

        def _decode_and_pad(buf):
            arr = decode_tiff_tile(
                bytes(buf),
                compression=self.compression,
                bits_per_sample=self.bits_per_sample,
                samples_per_pixel=self.samples_per_pixel,
                photometric=self.photometric,
                planar_config=self.planar_config,
                predictor=self.predictor,
                tile_width=self.tile_width,
                tile_height=self.tile_height,
                sample_format=self.sample_format,
                jpeg_tables=self._jpeg_tables_bytes,
            )
            # Edge tiles may decode to smaller dimensions than the chunk shape.
            # Pad to the expected chunk shape so zarr's reshape succeeds; zarr
            # will trim the padding when the array boundary is reached.
            if arr.shape != expected_shape:
                import numpy as np

                padded = np.zeros(expected_shape, dtype=arr.dtype)
                slices = tuple(slice(0, s) for s in arr.shape)
                padded[slices] = arr
                return padded.tobytes()
            return arr.tobytes()

        return as_numpy_array_wrapper(
            _decode_and_pad,
            chunk_bytes,
            chunk_spec.prototype,
        )

    async def _decode_single(self, chunk_bytes, chunk_spec):
        """Decode TIFF tile chunk bytes into pixel data.

        Args:
            chunk_bytes: Buffer containing compressed TIFF tile data.
            chunk_spec: ArraySpec describing the expected output.

        Returns:
            Buffer with decoded pixel data.
        """
        return await asyncio.to_thread(self._decode_sync, chunk_bytes, chunk_spec)

    async def _encode_single(self, chunk_bytes, chunk_spec):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("TiffTileCodec: encoding is not supported")

    def compute_encoded_size(self, input_byte_length: int, chunk_spec) -> int:
        """Return *input_byte_length* — compressed size is not predictable."""
        return input_byte_length

    def evolve_from_array_spec(self, array_spec):
        """Codec configuration is fixed at construction time."""
        return self

    def to_dict(self):
        """Serialize codec configuration to a JSON-compatible dictionary.

        Returns:
            dict with 'name' and 'configuration' keys.
        """
        return {
            "name": self.codec_name,
            "configuration": {
                "compression": self.compression,
                "bits_per_sample": self.bits_per_sample,
                "samples_per_pixel": self.samples_per_pixel,
                "photometric": self.photometric,
                "planar_config": self.planar_config,
                "predictor": self.predictor,
                "tile_width": self.tile_width,
                "tile_height": self.tile_height,
                "sample_format": self.sample_format,
                "jpeg_tables": self.jpeg_tables,
            },
        }

    @classmethod
    def from_dict(cls, data):
        """Construct a TiffTileCodec from a serialized configuration dictionary.

        Accepts both ``{"name": ..., "configuration": {...}}`` format and a
        flat configuration dictionary.

        Args:
            data: Configuration dictionary.

        Returns:
            TiffTileCodec instance.
        """
        config = data.get("configuration", data)
        return cls(
            compression=config.get("compression", 1),
            bits_per_sample=config.get("bits_per_sample", 8),
            samples_per_pixel=config.get("samples_per_pixel", 1),
            photometric=config.get("photometric", 1),
            planar_config=config.get("planar_config", 1),
            predictor=config.get("predictor", 1),
            tile_width=config.get("tile_width", 256),
            tile_height=config.get("tile_height", 256),
            sample_format=config.get("sample_format", 1),
            jpeg_tables=config.get("jpeg_tables"),
        )

    # -- numcodecs compatibility (consumer path) ---------------------------

    def decode(self, buf, out=None):
        """Synchronous decode for numcodecs filter protocol."""
        data = bytes(buf) if not isinstance(buf, bytes) else buf
        return decode_tiff_tile(
            data,
            compression=self.compression,
            bits_per_sample=self.bits_per_sample,
            samples_per_pixel=self.samples_per_pixel,
            photometric=self.photometric,
            planar_config=self.planar_config,
            predictor=self.predictor,
            tile_width=self.tile_width,
            tile_height=self.tile_height,
            sample_format=self.sample_format,
            jpeg_tables=self._jpeg_tables_bytes,
        )

    def encode(self, buf):
        """Encoding is not supported."""
        raise NotImplementedError("TiffTileCodec: encoding is not supported")

    def get_config(self):
        """Return numcodecs-compatible configuration dict."""
        return {
            "id": self.codec_id,
            "compression": self.compression,
            "bits_per_sample": self.bits_per_sample,
            "samples_per_pixel": self.samples_per_pixel,
            "photometric": self.photometric,
            "planar_config": self.planar_config,
            "predictor": self.predictor,
            "tile_width": self.tile_width,
            "tile_height": self.tile_height,
            "sample_format": self.sample_format,
            "jpeg_tables": self.jpeg_tables,
        }

    @classmethod
    def from_config(cls, config):
        """Construct from a numcodecs configuration dict."""
        return cls(
            compression=config.get("compression", 1),
            bits_per_sample=config.get("bits_per_sample", 8),
            samples_per_pixel=config.get("samples_per_pixel", 1),
            photometric=config.get("photometric", 1),
            planar_config=config.get("planar_config", 1),
            predictor=config.get("predictor", 1),
            tile_width=config.get("tile_width", 256),
            tile_height=config.get("tile_height", 256),
            sample_format=config.get("sample_format", 1),
            jpeg_tables=config.get("jpeg_tables"),
        )


# ---------------------------------------------------------------------------
# numcodecs registration (consumer path)
# ---------------------------------------------------------------------------


def _register_numcodecs():
    """Register all codecs with numcodecs for the consumer path.

    The consumer path (``fsspec ReferenceFileSystem`` + ``zarr.open_group``)
    reads zarr v2 metadata from Kerchunk JSON and uses
    ``numcodecs.get_codec(filter_config)`` to resolve codecs by their ``id``
    field.  This function registers our codec classes so that resolution works.
    """
    try:
        import numcodecs
    except ImportError:
        return

    numcodecs.register_codec(Jpeg2000Codec)
    numcodecs.register_codec(JpegCodec)
    numcodecs.register_codec(JbpBlockCodec)
    numcodecs.register_codec(TiffTileCodec)


_register_numcodecs()

