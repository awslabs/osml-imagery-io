"""Unit tests for _build_codec_instance() TIFF path.

Validates that TIFF codec configurations (containing a 'compression' key)
are correctly mapped to TiffTileCodec instances, including LE byte
normalization and jpeg_tables base64 encoding.

Requirements: 14.1, 14.2
"""

import base64
import struct
from unittest.mock import MagicMock

import pytest

virtualizarr = pytest.importorskip("virtualizarr", minversion="2.0")

from aws.osml.io._io import PixelType  # noqa: E402
from aws.osml.io.virtualizarr_parsers import (  # noqa: E402
    _build_codec_instance,
    _is_planar_multiband_tiff,
    _validate_chunk_geometry,
)
from aws.osml.io.zarr_codecs import TiffTileCodec  # noqa: E402


def _make_mock_asset(codec_config, num_bands=3, num_bits_per_pixel=8,
                     block_h=256, block_w=256, num_rows=256, num_cols=256,
                     pixel_type=None):
    """Create a mock asset with the given codec_configuration dict."""
    asset = MagicMock()
    asset.codec_configuration.return_value = codec_config
    asset.num_bands = num_bands
    asset.num_bits_per_pixel = num_bits_per_pixel
    asset.num_pixels_per_block_vertical = block_h
    asset.num_pixels_per_block_horizontal = block_w
    asset.num_rows = num_rows
    asset.num_columns = num_cols
    asset.pixel_value_type = pixel_type or PixelType.UInt8
    return asset


class TestBuildCodecInstanceTiffPath:
    """Verify _build_codec_instance() produces TiffTileCodec for TIFF configs.

    Requirements: 14.1, 14.2
    """

    def test_compression_key_produces_tiff_tile_codec(self):
        """A config with 'compression' key returns a TiffTileCodec instance."""
        config = {
            "compression": struct.pack("<H", 5),       # LZW
            "bits_per_sample": struct.pack("<H", 8),
            "samples_per_pixel": struct.pack("<H", 3),
            "photometric": struct.pack("<H", 2),        # RGB
            "planar_config": struct.pack("<H", 1),
            "predictor": struct.pack("<H", 2),           # horizontal
            "tile_width": struct.pack("<I", 256),
            "tile_height": struct.pack("<I", 256),
            "sample_format": struct.pack("<H", 1),
        }
        asset = _make_mock_asset(config)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        assert codec.compression == 5
        assert codec.bits_per_sample == 8
        assert codec.samples_per_pixel == 3
        assert codec.photometric == 2
        assert codec.planar_config == 1
        assert codec.predictor == 2
        assert codec.tile_width == 256
        assert codec.tile_height == 256
        assert codec.sample_format == 1
        assert codec.jpeg_tables is None

    def test_jpeg_tables_raw_bytes_are_base64_encoded(self):
        """Raw jpeg_tables bytes are base64-encoded before passing to TiffTileCodec."""
        # Simulate JPEG tables with SOI/EOI markers
        jpeg_tables_raw = b"\xff\xd8\xff\xdb\x00\x43" + b"\x00" * 64 + b"\xff\xd9"
        config = {
            "compression": struct.pack("<H", 7),       # JPEG
            "bits_per_sample": struct.pack("<H", 8),
            "samples_per_pixel": struct.pack("<H", 3),
            "photometric": struct.pack("<H", 6),        # YCbCr
            "planar_config": struct.pack("<H", 1),
            "predictor": struct.pack("<H", 1),
            "tile_width": struct.pack("<I", 512),
            "tile_height": struct.pack("<I", 512),
            "sample_format": struct.pack("<H", 1),
            "jpeg_tables": jpeg_tables_raw,
        }
        asset = _make_mock_asset(config)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        assert codec.compression == 7
        # jpeg_tables should be base64-encoded string
        expected_b64 = base64.b64encode(jpeg_tables_raw).decode("ascii")
        assert codec.jpeg_tables == expected_b64
        # Verify round-trip: decoding the stored b64 gives back original bytes
        assert base64.b64decode(codec.jpeg_tables) == jpeg_tables_raw

    def test_2byte_le_values_normalized_to_integers(self):
        """2-byte LE values (u16) are normalized to Python ints."""
        config = {
            "compression": struct.pack("<H", 32773),   # PackBits
            "bits_per_sample": struct.pack("<H", 16),
            "samples_per_pixel": struct.pack("<H", 1),
            "photometric": struct.pack("<H", 1),        # MinIsBlack
            "planar_config": struct.pack("<H", 1),
            "tile_width": struct.pack("<I", 128),
            "tile_height": struct.pack("<I", 128),
            "sample_format": struct.pack("<H", 2),      # INT (signed)
        }
        asset = _make_mock_asset(config, num_bands=1)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        # All u16 fields should be ints, not bytes
        assert isinstance(codec.compression, int)
        assert codec.compression == 32773
        assert isinstance(codec.bits_per_sample, int)
        assert codec.bits_per_sample == 16
        assert isinstance(codec.samples_per_pixel, int)
        assert codec.samples_per_pixel == 1
        assert isinstance(codec.sample_format, int)
        assert codec.sample_format == 2

    def test_4byte_le_values_normalized_to_integers(self):
        """4-byte LE values (u32) are normalized to Python ints."""
        config = {
            "compression": struct.pack("<H", 8),       # Deflate
            "bits_per_sample": struct.pack("<H", 8),
            "samples_per_pixel": struct.pack("<H", 4),
            "photometric": struct.pack("<H", 2),
            "planar_config": struct.pack("<H", 1),
            "predictor": struct.pack("<H", 1),
            "tile_width": struct.pack("<I", 512),
            "tile_height": struct.pack("<I", 1024),
            "sample_format": struct.pack("<H", 1),
        }
        asset = _make_mock_asset(config, num_bands=4)
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        # u32 fields should be ints, not bytes
        assert isinstance(codec.tile_width, int)
        assert codec.tile_width == 512
        assert isinstance(codec.tile_height, int)
        assert codec.tile_height == 1024

    def test_defaults_from_asset_when_keys_missing(self):
        """Missing config keys fall back to asset metadata defaults."""
        config = {
            "compression": struct.pack("<H", 5),
        }
        asset = _make_mock_asset(
            config, num_bands=4, num_bits_per_pixel=16,
            block_h=128, block_w=64,
        )
        codec = _build_codec_instance(asset)

        assert isinstance(codec, TiffTileCodec)
        assert codec.compression == 5
        # Defaults from asset
        assert codec.bits_per_sample == 16   # from num_bits_per_pixel
        assert codec.samples_per_pixel == 4  # from num_bands
        assert codec.tile_width == 64        # from block_w
        assert codec.tile_height == 128      # from block_h
        # Defaults from code
        assert codec.photometric == 1
        assert codec.planar_config == 1
        assert codec.predictor == 1
        assert codec.sample_format == 1
        assert codec.jpeg_tables is None


def _planar_config(compression=5, bands=3, predictor=1):
    """A planar (PlanarConfiguration=2) multiband TIFF codec config."""
    return {
        "compression": struct.pack("<H", compression),
        "bits_per_sample": struct.pack("<H", 16),
        "samples_per_pixel": struct.pack("<H", bands),
        "photometric": struct.pack("<H", 2),
        "planar_config": struct.pack("<H", 2),
        "predictor": struct.pack("<H", predictor),
        "tile_width": struct.pack("<I", 64),
        "tile_height": struct.pack("<I", 64),
        "sample_format": struct.pack("<H", 1),
    }


class TestSinglePlaneOverride:
    """``single_plane=True`` presents a planar chunk as a single-band chunky tile.

    Each chunk of a planar array is one plane's tile: it holds one band's
    samples with nothing interleaved.  ``samples_per_pixel=1, planar_config=1``
    is the truthful description of that buffer, and reporting the array's real
    band count would make the decoder expect N planes' bytes in a buffer that
    carries one.
    """

    def test_single_plane_overrides_samples_and_planar_config(self):
        codec = _build_codec_instance(
            _make_mock_asset(_planar_config()), single_plane=True
        )

        assert isinstance(codec, TiffTileCodec)
        assert codec.samples_per_pixel == 1
        assert codec.planar_config == 1

    def test_single_plane_preserves_every_other_parameter(self):
        """Only the two interleaving parameters change; decode still needs the rest."""
        codec = _build_codec_instance(
            _make_mock_asset(_planar_config(predictor=2)), single_plane=True
        )

        assert codec.compression == 5
        assert codec.bits_per_sample == 16
        assert codec.photometric == 2
        assert codec.tile_width == 64
        assert codec.tile_height == 64
        assert codec.sample_format == 1
        # Predictor must survive: per TIFF 6.0 Section 13 (p. 64) differencing
        # for planar data works exactly as it does for grayscale, so a plane
        # presented as single-band still needs its predictor applied.
        assert codec.predictor == 2

    def test_default_reports_true_band_count(self):
        """Without the flag, the planar config passes through unchanged."""
        codec = _build_codec_instance(_make_mock_asset(_planar_config()))

        assert codec.samples_per_pixel == 3
        assert codec.planar_config == 2


class TestIsPlanarMultibandTiff:
    """``_is_planar_multiband_tiff`` gates per-plane chunking.

    It must key off the provider's declared ``planar_config``, never off the
    number of byte ranges: a 3-tile-part J2K chunk and a 3-band planar TIFF tile
    are indistinguishable by length alone.
    """

    def test_planar_multiband_tiff_is_true(self):
        assert _is_planar_multiband_tiff(_make_mock_asset(_planar_config()))

    def test_chunky_multiband_tiff_is_false(self):
        config = _planar_config()
        config["planar_config"] = struct.pack("<H", 1)
        assert not _is_planar_multiband_tiff(_make_mock_asset(config))

    def test_single_band_planar_tiff_is_false(self):
        """PlanarConfiguration is irrelevant at SamplesPerPixel=1 (TIFF 6.0 p. 38)."""
        asset = _make_mock_asset(_planar_config(bands=1), num_bands=1)
        assert not _is_planar_multiband_tiff(asset)

    def test_asset_without_codec_config_is_false(self):
        assert not _is_planar_multiband_tiff(_make_mock_asset(None))

    def test_non_tiff_codec_config_is_false(self):
        """Other formats carry no ``planar_config`` key, so they never chunk per plane."""
        jbp_config = {
            "pvtype": b"INT",
            "nbpp": struct.pack("<H", 16),
            "imode": struct.pack("<H", ord("B")),
        }
        assert not _is_planar_multiband_tiff(_make_mock_asset(jbp_config))


# ---------------------------------------------------------------------------
# Chunk-geometry fail-safe
# ---------------------------------------------------------------------------


def _validate(
    *,
    raw,
    chunk_lengths,
    bands=3,
    rows=128,
    cols=128,
    block=64,
    bands_per_chunk=None,
    grid_shape=None,
    pixel_type=None,
    bits=8,
):
    """Run the geometry fail-safe over a described configuration."""
    bpc = bands if bands_per_chunk is None else bands_per_chunk
    grid_bands = 1 if bpc == bands else bands
    _validate_chunk_geometry(
        _make_mock_asset(
            None,
            num_bands=bands,
            num_bits_per_pixel=bits,
            num_rows=rows,
            num_cols=cols,
            pixel_type=pixel_type,
        ),
        raw,
        chunk_lengths,
        shape=(bands, rows, cols),
        chunk_shape=(bpc, block, block),
        grid_shape=grid_shape or (grid_bands, -(-rows // block), -(-cols // block)),
        bands_per_chunk=bpc,
    )


def _full_grid(length, rows=128, cols=128, block=64, bands=1):
    """A complete chunk-length map with *length* bytes at every grid position."""
    return {
        f"{b}.{r}.{c}": length
        for b in range(bands)
        for r in range(-(-rows // block))
        for c in range(-(-cols // block))
    }


def _tiff_raw(compression=5, planar=1, bands=3, bits=8, tile=64):
    return {
        "compression": compression,
        "planar_config": planar,
        "samples_per_pixel": bands,
        "bits_per_sample": bits,
        "tile_width": tile,
        "tile_height": tile,
        "sample_format": 1,
    }


class TestChunkGeometryFailSafe:
    """The parser must refuse a geometry it cannot satisfy.

    A Zarr store with mismatched chunk geometry does not fail loudly — missing
    chunks read back as ``fill_value`` and short chunks read back zero-padded.
    The result is a plausible-looking array of wrong pixels, which is exactly
    how the uncompressed-multiband TIFF defect stayed hidden.  These cases pin
    each way that can happen to a raised error instead.
    """

    def test_multiband_without_codec_is_refused(self):
        """The original defect: no codec means no de-interleave, silently.

        ``BytesCodec`` would reshape pixel-interleaved ``RGBRGB`` bytes straight
        into ``(bands, y, x)``, scrambling the bands with no error at all.
        """
        with pytest.raises(ValueError, match="no codec configuration"):
            _validate(raw=None, chunk_lengths=_full_grid(12288))

    def test_single_band_without_codec_is_allowed(self):
        """Single-band uncompressed genuinely needs no codec (TIFF 6.0 p. 38).

        The raw bytes *are* the pixel data and PlanarConfiguration is irrelevant
        at ``SamplesPerPixel=1``, so this must stay a zero-copy read rather than
        get swept up by the multiband check.
        """
        _validate(raw=None, bands=1, chunk_lengths=_full_grid(4096))

    def test_uncompressed_chunk_holding_one_plane_is_refused(self):
        """A 3-band chunk fed one plane's bytes reads back 2/3 zero-filled."""
        with pytest.raises(ValueError, match="inconsistent with its geometry"):
            _validate(
                raw=_tiff_raw(compression=1, planar=2),
                chunk_lengths=_full_grid(4096),  # one 64×64 UInt8 plane, not three
            )

    def test_uncompressed_exact_length_is_accepted(self):
        """The same geometry passes once the chunk carries all three planes."""
        _validate(
            raw=_tiff_raw(compression=1),
            chunk_lengths=_full_grid(64 * 64 * 3),
        )

    def test_per_plane_uncompressed_length_is_accepted(self):
        """Per-plane chunking: one plane per chunk, so one plane's bytes is right."""
        _validate(
            raw=_tiff_raw(compression=1, planar=2),
            bands_per_chunk=1,
            chunk_lengths=_full_grid(64 * 64, bands=3),
        )

    def test_uncompressed_16bit_length_accounts_for_itemsize(self):
        with pytest.raises(ValueError, match="requires 24576"):
            _validate(
                raw=_tiff_raw(compression=1, bits=16),
                pixel_type=PixelType.UInt16,
                bits=16,
                chunk_lengths=_full_grid(64 * 64 * 3),  # 8-bit sizing
            )

    def test_missing_chunk_is_refused(self):
        """A hole in the grid would read back as fill_value, not raise."""
        lengths = _full_grid(900)
        del lengths["0.1.1"]
        with pytest.raises(ValueError, match="unreferenced chunks"):
            _validate(raw=_tiff_raw(), chunk_lengths=lengths)

    def test_grid_not_covering_shape_is_refused(self):
        with pytest.raises(ValueError, match="does not cover its shape"):
            _validate(
                raw=_tiff_raw(),
                rows=256,
                chunk_lengths=_full_grid(900, rows=256),
                grid_shape=(1, 2, 2),  # 2 rows of chunks cannot span 256 pixels
            )

    def test_zero_length_chunk_is_refused(self):
        lengths = _full_grid(900)
        lengths["0.1.1"] = 0
        with pytest.raises(ValueError, match="empty chunk references"):
            _validate(raw=_tiff_raw(), chunk_lengths=lengths)

    def test_error_message_names_the_configuration(self):
        """The message must identify *which* config was rejected, not just that."""
        with pytest.raises(ValueError) as excinfo:
            _validate(
                raw=_tiff_raw(compression=1, planar=2),
                chunk_lengths=_full_grid(4096),
            )
        message = str(excinfo.value)
        assert "bands=3" in message
        assert "compression=1" in message
        assert "planar_config=2" in message

    def test_compressed_lengths_are_not_length_checked(self):
        """LZW/Deflate lengths vary with pixel content; only emptiness is checkable."""
        lengths = _full_grid(900)
        lengths["0.0.1"] = 17
        lengths["0.1.0"] = 4096
        _validate(raw=_tiff_raw(compression=5), chunk_lengths=lengths)

    def test_clipped_trailing_block_is_accepted(self):
        """A stripped TIFF's last strip is stored clipped, not padded to nominal.

        Both forms are readable — the decoder zero-pads a short block — so the
        check must accept either rather than reject a spec-conformant file.
        """
        # 100 rows of 64-row strips: the trailing strip covers only 36 rows.
        lengths = {"0.0.0": 64 * 64, "0.1.0": 36 * 64}
        _validate(
            raw=_tiff_raw(compression=1, bands=1),
            bands=1,
            rows=100,
            cols=64,
            chunk_lengths=lengths,
        )


class TestFailSafeAcceptsUnaffectedCodecs:
    """The four codecs this bug never touched must keep building stores.

    A geometry invariant tuned to TIFF is only useful if it does not reject
    NITF-NC, NITF-C3, NITF-C8, or DTED — whose chunk lengths follow entirely
    different rules (bit-packed planes, entropy-coded scans, record framing).
    """

    def test_nitf_uncompressed_bit_packed_block(self):
        """NBPP-packed NITF planes: ceil(rows·cols·nbpp/8) per band."""
        raw = {"pvtype": "INT", "nbpp": 8, "abpp": 8, "imode": ord("B"), "nbands": 3}
        _validate(raw=raw, block=64, chunk_lengths=_full_grid(64 * 64 * 3))

    def test_nitf_sub_byte_bit_packed_block(self):
        """1-bit NITF data packs 8 samples per byte, so the plane is 1/8 the size."""
        raw = {"pvtype": "INT", "nbpp": 1, "abpp": 1, "imode": ord("B"), "nbands": 3}
        _validate(raw=raw, bits=1, chunk_lengths=_full_grid((64 * 64 // 8) * 3))

    def test_nitf_c3_jpeg_variable_length(self):
        """An entropy-coded JPEG scan has no geometry-derived length."""
        raw = {"color_space": 1, "num_bands": 3, "bits_per_pixel": 8}
        _validate(raw=raw, chunk_lengths=_full_grid(3076))

    def test_nitf_c8_j2k_variable_length(self):
        """A JPEG 2000 codestream can legitimately exceed the raw pixel size."""
        raw = {"main_header": "AAAA"}
        _validate(raw=raw, chunk_lengths=_full_grid(13669))

    def test_dted_record_framing(self):
        """DTED chunks are whole data records: sentinel + header + posts + checksum."""
        raw = {"dted_codec": "", "num_lat_points": 16, "num_lon_lines": 16,
               "record_size": 44}
        _validate(
            raw=raw,
            bands=1,
            rows=16,
            cols=16,
            block=16,
            chunk_lengths={"0.0.0": 16 * 44},
        )
