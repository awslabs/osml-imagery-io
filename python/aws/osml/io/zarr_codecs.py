"""Zarr codec plugins for JPEG 2000, JPEG, and uncompressed JBP/NITF imagery.

Codec classes implement both the zarr-python v3 codec protocol (registered via
entry points) and the numcodecs Codec protocol (registered at import time).
This dual registration allows the codecs to work with:
- zarr v3 native stores (via entry points)
- Kerchunk v1 references consumed through fsspec ReferenceFileSystem (via numcodecs)

Usage (automatic via entry points — no import needed):
    import xarray as xr
    ds = xr.open_zarr("reference://", storage_options={"fo": "index.json"})

Usage (explicit import):
    from aws.osml.io.zarr_codecs import Jpeg2000Codec, JpegCodec, JbpBlockCodec
"""

import base64

from aws.osml.io._io import decode_jbp_block, decode_jpeg, decode_jpeg2000

__all__ = [
    "Jpeg2000Codec",
    "JpegCodec",
    "JbpBlockCodec",
    "decode_jpeg2000",
    "decode_jpeg",
    "decode_jbp_block",
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


class Jpeg2000Codec:
    """Zarr codec for JPEG 2000 codestreams.

    Registered as: https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000

    Configuration:
        main_header: Optional[str]  — base64-encoded J2K main header bytes
        resolution_level: int       — target resolution level (default 0)
    """

    codec_name = "https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000"
    codec_id = "https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000"

    def __init__(self, *, main_header=None, resolution_level=0):
        self.resolution_level = resolution_level
        if main_header is not None:
            try:
                self._main_header_bytes = base64.b64decode(main_header)
            except Exception as e:
                raise ValueError(f"Invalid base64 in main_header: {e}")
            self._main_header_b64 = main_header
        else:
            self._main_header_bytes = None
            self._main_header_b64 = None

    def decode(self, chunk_bytes, chunk_spec=None):
        """Decode JPEG 2000 chunk bytes into pixel data.

        Args:
            chunk_bytes: Bytes-like object or numpy array containing J2K codestream data.
            chunk_spec: Optional ArraySpec (unused, accepted for zarr protocol compatibility).

        Returns:
            Flat byte buffer for numcodecs filter compatibility.
        """
        if hasattr(chunk_bytes, 'tobytes'):
            data = chunk_bytes.tobytes()
        else:
            data = bytes(chunk_bytes)
        result = decode_jpeg2000(
            data,
            main_header=self._main_header_bytes,
            resolution_level=self.resolution_level,
        )
        return result.tobytes()

    def encode(self, chunk_bytes, chunk_spec=None):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("Jpeg2000Codec: encoding is not yet supported")

    def to_dict(self):
        """Serialize codec configuration to a JSON-compatible dictionary.

        Returns:
            dict with 'name' and 'configuration' keys.
        """
        return {
            "name": self.codec_name,
            "configuration": {
                "main_header": self._main_header_b64,
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

    def get_config(self):
        """Return numcodecs-compatible configuration dict."""
        return {
            "id": self.codec_id,
            "main_header": self._main_header_b64,
            "resolution_level": self.resolution_level,
        }

    @classmethod
    def from_config(cls, config):
        """Construct from a numcodecs configuration dict."""
        return cls(
            main_header=config.get("main_header"),
            resolution_level=config.get("resolution_level", 0),
        )


class JpegCodec:
    """Zarr codec for JPEG streams.

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

    def __init__(self, *, bits_per_pixel, num_bands, block_width, block_height, imode, color_space):
        self.bits_per_pixel = bits_per_pixel
        self.num_bands = num_bands
        self.block_width = block_width
        self.block_height = block_height
        self.imode = imode
        self.color_space = color_space

    def decode(self, chunk_bytes, chunk_spec=None):
        """Decode JPEG chunk bytes into pixel data.

        Args:
            chunk_bytes: Bytes-like object or numpy array containing JPEG stream data.
            chunk_spec: Optional ArraySpec (unused, accepted for zarr protocol compatibility).

        Returns:
            Flat byte buffer for numcodecs filter compatibility.
        """
        if hasattr(chunk_bytes, 'tobytes'):
            data = chunk_bytes.tobytes()
        else:
            data = bytes(chunk_bytes)
        result = decode_jpeg(
            data,
            bits_per_pixel=self.bits_per_pixel,
            num_bands=self.num_bands,
            block_width=self.block_width,
            block_height=self.block_height,
            imode=self.imode,
            color_space=self.color_space,
        )
        return result.tobytes()

    def encode(self, chunk_bytes, chunk_spec=None):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("JpegCodec: encoding is not yet supported")

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


class JbpBlockCodec:
    """Zarr codec for uncompressed JBP/NITF/NSIF image blocks.

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

    def __init__(self, *, num_bands, block_height, block_width, nbpp, imode, pvtype):
        self.num_bands = num_bands
        self.block_height = block_height
        self.block_width = block_width
        self.nbpp = nbpp
        self.imode = imode
        self.pvtype = pvtype

    def decode(self, chunk_bytes, chunk_spec=None):
        """Decode uncompressed JBP/NITF chunk bytes into a NumPy ndarray.

        When called as a numcodecs filter (by zarr v2 pipeline), the input
        may be a numpy array of raw bytes. Returns a flat byte buffer that
        zarr will view/reshape into the chunk shape.

        Args:
            chunk_bytes: Bytes-like object or numpy array containing raw pixel data.
            chunk_spec: Optional ArraySpec (unused, accepted for zarr protocol compatibility).

        Returns:
            NumPy ndarray — flat byte buffer when used as filter, shaped array otherwise.
        """
        if hasattr(chunk_bytes, 'tobytes'):
            data = chunk_bytes.tobytes()
        else:
            data = bytes(chunk_bytes)
        result = decode_jbp_block(
            data,
            num_bands=self.num_bands,
            block_height=self.block_height,
            block_width=self.block_width,
            nbpp=self.nbpp,
            imode=self.imode,
            pvtype=self.pvtype,
        )
        # Return flat contiguous bytes for numcodecs filter compatibility.
        # zarr's V2Codec will view() and reshape() the result.
        return result.tobytes()

    def encode(self, chunk_bytes, chunk_spec=None):
        """Encoding is not supported.

        Raises:
            NotImplementedError: Always.
        """
        raise NotImplementedError("JbpBlockCodec: encoding is not yet supported")

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


# ---------------------------------------------------------------------------
# Register codecs with numcodecs so zarr v2 filters can resolve them
# ---------------------------------------------------------------------------

def _register_numcodecs():
    """Register our codecs with the numcodecs registry.

    This allows zarr v2 metadata (used by Kerchunk v1 references) to
    resolve our custom codecs when they appear in the ``filters`` list.
    """
    try:
        from numcodecs.registry import register_codec
        register_codec(Jpeg2000Codec)
        register_codec(JpegCodec)
        register_codec(JbpBlockCodec)
    except ImportError:
        pass  # numcodecs not installed — zarr v2 filter path unavailable


_register_numcodecs()
