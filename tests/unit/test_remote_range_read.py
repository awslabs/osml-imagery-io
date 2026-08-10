"""Cross-cutting remote range-read tests.

Proves that remoteness is transparent to output across every format that IO.open
routes through the ``Remote`` ``OwnedBuffer`` path, and that the range-read path
never regresses to a silent full download for the block-capable formats.

The tests here open a real unit-data file through a *seekable, sized* Python
file-like object that logs every ``read`` range (``_RangeLoggingStream``). Because
``IO.open(stream, "r", format=...)`` builds a ``Remote`` ``OwnedBuffer`` for the
block-capable formats (TIFF, J2K, NITF/JBP, DTED — see ``remote_header_hint`` in
``src/bindings/io.rs``), these exercise the real fetch path end to end and can
assert exactly how many bytes were pulled.

Format coverage / expectations:

- **TIFF, J2K, NITF/JBP** — block-capable: metadata + a block decode must pull
  strictly fewer bytes than the whole file (range reads, not a full download).
- **DTED** — block-capable header, but its single full-grid block spans every
  record, so a *block decode* legitimately materializes the whole file; only
  *metadata* stays bounded. Asserted accordingly.
- **PNG, standalone JPEG** — monolithic / non-blocking: IO.open deliberately keeps
  them on the full-read fallback (chunking a mandatory whole-file read has no
  benefit). Covered by the fallback tests, where correctness is what matters.

The pixel-identity checks (remote vs. full in-memory read) are the transparency
proof; they run for every format, including the monolithic ones.
"""

import io
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, iminfo, imread, tiles

UNIT_DATA = Path("data/unit")

TIFF_FILE = UNIT_DATA / "tiff-256x256-1band-8bit-tiled-deflate.tif"
J2K_NITF_FILE = UNIT_DATA / "nitf21-64x64-3band-8bit-j2k.ntf"
NC_NITF_FILE = UNIT_DATA / "nitf21-256x256-3band-8bit-nc.ntf"
DTED_FILE = UNIT_DATA / "dted-16x16-1band-int16.dt1"


# ---------------------------------------------------------------------------
# Range-logging / non-seekable stream fakes
# ---------------------------------------------------------------------------


class _RangeLoggingStream:
    """A seekable, sized file-like object that logs every ``read`` range.

    Records ``(offset, length)`` for each read so a test can assert the reader
    pulled byte ranges on demand instead of downloading the whole file.
    ``total_read`` accumulates the bytes returned.
    """

    def __init__(self, data: bytes):
        self._buf = io.BytesIO(data)
        self._size = len(data)
        self.reads: list[tuple[int, int]] = []
        self.total_read = 0

    def seekable(self) -> bool:
        return True

    def seek(self, offset, whence=0):
        return self._buf.seek(offset, whence)

    def tell(self):
        return self._buf.tell()

    def read(self, n=-1):
        pos = self._buf.tell()
        b = self._buf.read(n)
        self.reads.append((pos, len(b)))
        self.total_read += len(b)
        return b


class _NonSeekableStream:
    """A file-like object that reports it is not seekable (full-read fallback)."""

    def __init__(self, data: bytes):
        self._buf = io.BytesIO(data)

    def seekable(self) -> bool:
        return False

    def read(self, n=-1):
        return self._buf.read(n)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _first_image_block(reader):
    """Decode block (0,0,0) of the first image asset of an open reader."""
    keys = reader.get_asset_keys()
    assert keys, "expected at least one asset"
    # Prefer an image asset if the format exposes typed keys.
    key = next((k for k in keys if "image" in k or "elevation" in k), keys[0])
    asset = reader.get_asset(key)
    block = asset.get_block(0, 0, 0)
    return np.array(block.data)


def _decode_via_stream(file_bytes: bytes, fmt: str, stream):
    """Open ``stream`` as ``fmt`` and return the first image block."""
    with IO.open(stream, "r", format=fmt) as reader:
        return _first_image_block(reader)


# ===========================================================================
# Per-format remote decode + bounded-fetch (block-capable formats)
# ===========================================================================

BLOCK_CAPABLE_CASES = [
    pytest.param(TIFF_FILE, "tiff", id="tiff"),
    pytest.param(J2K_NITF_FILE, "nitf", id="nitf-j2k"),
    pytest.param(NC_NITF_FILE, "nitf", id="nitf-nc"),
]


@pytest.mark.parametrize("path,fmt", BLOCK_CAPABLE_CASES)
class TestBlockCapableRemoteDecode:
    """TIFF, J2K-in-NITF, and NC-in-NITF read via byte ranges, not full download."""

    def test_pixels_match_full_read(self, path, fmt):
        if not path.exists():
            pytest.skip(f"{path} unavailable")
        file_bytes = path.read_bytes()

        remote_block = _decode_via_stream(file_bytes, fmt, _RangeLoggingStream(file_bytes))
        full_block = _decode_via_stream(file_bytes, fmt, io.BytesIO(file_bytes))

        np.testing.assert_array_equal(remote_block, full_block)

    def test_metadata_is_bounded(self, path, fmt):
        """Reading metadata alone must not pull the whole file.

        Only meaningful when the file is larger than the eager header prefetch
        (64 KiB — see ``remote_header_hint``): for a file smaller than the
        prefetch region, a single read covering it is the correct, coalesced
        behavior, not a full-download regression.
        """
        if not path.exists():
            pytest.skip(f"{path} unavailable")
        file_bytes = path.read_bytes()
        if len(file_bytes) <= 64 * 1024:
            pytest.skip(
                f"{path.name} ({len(file_bytes)} B) fits within the 64 KiB header "
                "prefetch; bounded-fetch is only observable on larger files"
            )
        stream = _RangeLoggingStream(file_bytes)

        with IO.open(stream, "r", format=fmt) as reader:
            keys = reader.get_asset_keys()
            assert keys
            _ = reader.get_asset(keys[0])  # metadata access

        assert stream.reads, "expected at least one bounded range read"
        assert all(length < len(file_bytes) for _, length in stream.reads), (
            f"a single metadata read covered the whole file: {stream.reads}"
        )
        assert stream.total_read < len(file_bytes), (
            f"metadata read {stream.total_read} of {len(file_bytes)} bytes "
            "— expected a bounded range read, not a full download"
        )


class TestEmbeddedJ2kTilePartFetch:
    """Embedded J2K (JBP/NITF) per-tile decode over a Remote source fetches only
    the touched tile's byte ranges when TLM markers are present.

    A tiled J2K-in-NITF written by our encoder carries TLM markers, so the
    decoder resolves each tile's byte ranges from the TLM table (no full-file
    SOT scan) and fetches only header + that tile's parts. Contrast with the
    TLM-absent conformance case, where OpenJPEG/SOT must walk the codestream —
    the documented TLM-absent floor, asserted only for pixel identity elsewhere.
    """

    def _tiled_j2k_nitf_bytes(self, tmp_path) -> bytes:
        """Write a multi-tile TLM-present C8 NITF and return its bytes.

        Uses random data so tiles do not compress to near-nothing (which would
        fit inside the 64 KiB prefetch and mask the per-tile fetch reduction).
        """
        from aws.osml.io import imsave

        rng = np.random.RandomState(7)
        data = rng.randint(0, 255, (1, 2048, 2048), dtype=np.uint8)
        out = tmp_path / "tiled_c8.ntf"
        imsave(str(out), data, compression="c8", block_size=(256, 256))
        return out.read_bytes()

    def test_center_tile_fetch_is_bounded(self, tmp_path):
        file_bytes = self._tiled_j2k_nitf_bytes(tmp_path)
        file_size = len(file_bytes)
        # Random-data C8 of a 2048x2048 image is comfortably larger than the
        # header prefetch; if a future encoder change shrinks it, skip rather
        # than assert a meaningless bound.
        if file_size <= 2 * 64 * 1024:
            pytest.skip(f"generated C8 NITF too small ({file_size} B) to observe bounding")

        stream = _RangeLoggingStream(file_bytes)
        with IO.open(stream, "r", format="nitf") as reader:
            key = reader.get_asset_keys()[0]
            asset = reader.get_asset(key)
            grid_rows, grid_cols = asset.block_grid_size
            assert grid_rows > 1 and grid_cols > 1, "expected a multi-tile grid"
            cr, cc = grid_rows // 2, grid_cols // 2
            remote_px = np.array(asset.get_block(cr, cc, 0).data)

        # A single center-tile decode must pull materially less than the whole
        # file — header + prefetch + this tile's parts, not ~100% of the file.
        assert stream.total_read < file_size // 2, (
            f"center-tile decode read {stream.total_read} of {file_size} bytes "
            "— TLM-present per-tile fetch should stay well under the whole file"
        )

        # And it must be pixel-identical to a full in-memory decode of the same tile.
        with IO.open(io.BytesIO(file_bytes), "r", format="nitf") as reader:
            key = reader.get_asset_keys()[0]
            full_px = np.array(reader.get_asset(key).get_block(cr, cc, 0).data)
        np.testing.assert_array_equal(remote_px, full_px)


class TestDtedRemoteDecode:
    """DTED: bounded metadata, but a full-grid block spans the whole file."""

    def test_pixels_match_full_read(self):
        if not DTED_FILE.exists():
            pytest.skip(f"{DTED_FILE} unavailable")
        file_bytes = DTED_FILE.read_bytes()

        remote_block = _decode_via_stream(file_bytes, "dted", _RangeLoggingStream(file_bytes))
        full_block = _decode_via_stream(file_bytes, "dted", io.BytesIO(file_bytes))

        np.testing.assert_array_equal(remote_block, full_block)

    def test_metadata_is_bounded(self):
        """iminfo over a remote DTED handle reads only the header, not the grid."""
        if not DTED_FILE.exists():
            pytest.skip(f"{DTED_FILE} unavailable")
        file_bytes = DTED_FILE.read_bytes()
        stream = _RangeLoggingStream(file_bytes)

        info = iminfo(stream, format="dted")
        assert info.width > 0 and info.height > 0
        # The UHL/DSI/ACC header is 3428 bytes; metadata must stay well under the
        # full file (which includes all elevation records).
        assert stream.total_read < len(file_bytes), (
            f"DTED metadata read {stream.total_read} of {len(file_bytes)} bytes"
        )


# ===========================================================================
# Python integration: iminfo / tiles / DatasetReader.get_block over a remote
# range-logging handle (block-capable TIFF as the representative format)
# ===========================================================================


class TestRemoteIntegrationApis:
    """iminfo, tiles, and direct DatasetReader.get_block work over a remote handle
    without a full download."""

    def _tiff_bytes(self):
        if not TIFF_FILE.exists():
            pytest.skip(f"{TIFF_FILE} unavailable")
        return TIFF_FILE.read_bytes()

    def test_iminfo_over_remote(self):
        file_bytes = self._tiff_bytes()
        stream = _RangeLoggingStream(file_bytes)

        info = iminfo(stream, format="tiff")
        assert info.width == 256
        assert info.height == 256
        assert stream.total_read < len(file_bytes), (
            f"iminfo read {stream.total_read} of {len(file_bytes)} bytes"
        )

    def test_tiles_over_remote(self):
        file_bytes = self._tiff_bytes()
        stream = _RangeLoggingStream(file_bytes)

        tile_list = list(tiles(stream, tile_size=(128, 128), format="tiff"))
        # 256x256 image with 128x128 tiles = 4 tiles.
        assert len(tile_list) == 4

        # Cross-check pixels against a full in-memory read.
        full_tiles = list(tiles(io.BytesIO(file_bytes), tile_size=(128, 128), format="tiff"))
        for remote_tile, full_tile in zip(tile_list, full_tiles):
            np.testing.assert_array_equal(
                np.array(remote_tile.data), np.array(full_tile.data)
            )

    def test_dataset_reader_get_block_over_remote(self):
        file_bytes = self._tiff_bytes()
        stream = _RangeLoggingStream(file_bytes)

        with IO.open(stream, "r", format="tiff") as reader:
            key = reader.get_asset_keys()[0]
            block = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        with IO.open(io.BytesIO(file_bytes), "r", format="tiff") as reader:
            key = reader.get_asset_keys()[0]
            full = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        np.testing.assert_array_equal(block, full)
        assert stream.total_read < len(file_bytes), (
            f"get_block read {stream.total_read} of {len(file_bytes)} bytes"
        )


# ===========================================================================
# Python integration over a real fsspec LocalFileSystem handle
# ===========================================================================


class TestRemoteViaFsspecLocalFileSystem:
    """A real fsspec ``LocalFileSystem`` handle is seekable and sized, so IO.open
    routes it through the ``Remote`` path. Skipped if fsspec is unavailable."""

    def test_tiff_via_fsspec_local(self):
        fsspec = pytest.importorskip("fsspec")
        if not TIFF_FILE.exists():
            pytest.skip(f"{TIFF_FILE} unavailable")

        fs = fsspec.filesystem("file")
        with fs.open(str(TIFF_FILE), "rb") as handle:
            with IO.open(handle, "r", format="tiff") as reader:
                key = reader.get_asset_keys()[0]
                remote_block = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        full_block = imread(str(TIFF_FILE))
        # imread returns the full image; compare the top-left block region.
        bh, bw = remote_block.shape[-2], remote_block.shape[-1]
        expected = full_block[..., :bh, :bw]
        np.testing.assert_array_equal(remote_block, expected)


# ===========================================================================
# IO.open remote-source interface: filesystem= param and URL resolution
# (s3://-style URL over an in-memory fsspec FS, no real S3)
# ===========================================================================


class TestRemoteSourceInterface:
    """``IO.open`` gains a ``filesystem=`` parameter and internal ``url_to_fs``
    URL resolution. Both route a remote source through the concurrent
    ``Remote``/``cat_ranges`` path. An in-memory ``MemoryFileSystem`` stands in
    for S3 (same fsspec ``AsyncFileSystem`` contract, no network)."""

    def _memory_fs_with_tiff(self):
        """Write the unit TIFF into a fresh fsspec memory filesystem.

        Returns ``(fs, path, file_bytes)``. Skips if fsspec or the unit data
        file is unavailable.
        """
        fsspec = pytest.importorskip("fsspec")
        if not TIFF_FILE.exists():
            pytest.skip(f"{TIFF_FILE} unavailable")
        file_bytes = TIFF_FILE.read_bytes()
        fs = fsspec.filesystem("memory")
        # Unique-ish path within the shared in-memory store.
        path = "/remote-iface/image.tif"
        fs.pipe_file(path, file_bytes)
        return fs, path, file_bytes

    def test_explicit_filesystem_drives_remote_path(self):
        """Passing ``filesystem=`` opens the path through it and decodes
        pixel-identically to a full in-memory read."""
        fs, path, file_bytes = self._memory_fs_with_tiff()

        with IO.open(path, "r", format="tiff", filesystem=fs) as reader:
            key = reader.get_asset_keys()[0]
            remote_block = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        with IO.open(io.BytesIO(file_bytes), "r", format="tiff") as reader:
            key = reader.get_asset_keys()[0]
            full_block = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        np.testing.assert_array_equal(remote_block, full_block)

    def test_url_string_resolves_via_url_to_fs(self):
        """A ``memory://`` URL (S3-style scheme) resolves internally via
        ``fsspec.core.url_to_fs`` and decodes pixel-identically."""
        _fs, path, file_bytes = self._memory_fs_with_tiff()
        url = f"memory://{path}"

        # No explicit format: detected from the .tif extension in the URL.
        with IO.open(url, "r") as reader:
            key = reader.get_asset_keys()[0]
            remote_block = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        with IO.open(io.BytesIO(file_bytes), "r", format="tiff") as reader:
            key = reader.get_asset_keys()[0]
            full_block = np.array(reader.get_asset(key).get_block(0, 0, 0).data)

        np.testing.assert_array_equal(remote_block, full_block)

    def test_url_string_forwards_through_imread(self):
        """A remote URL flows through the convenience ``imread`` unchanged."""
        _fs, path, file_bytes = self._memory_fs_with_tiff()
        url = f"memory://{path}"

        remote = imread(url)
        full = imread(io.BytesIO(file_bytes), format="tiff")
        np.testing.assert_array_equal(remote, full)

    def test_imread_accepts_filesystem_kwarg(self):
        """``imread`` forwards an explicit ``filesystem=`` to ``IO.open``."""
        fs, path, file_bytes = self._memory_fs_with_tiff()

        remote = imread(path, format="tiff", filesystem=fs)
        full = imread(io.BytesIO(file_bytes), format="tiff")
        np.testing.assert_array_equal(remote, full)

    def test_iminfo_and_tiles_accept_filesystem_kwarg(self):
        """``iminfo`` and ``tiles`` forward ``filesystem=`` to ``IO.open``."""
        fs, path, _file_bytes = self._memory_fs_with_tiff()

        info = iminfo(path, format="tiff", filesystem=fs)
        assert info.width == 256 and info.height == 256

        tile_list = list(tiles(path, tile_size=(128, 128), format="tiff", filesystem=fs))
        assert len(tile_list) == 4

    def test_filesystem_with_bytesio_raises_value_error(self):
        """``filesystem=`` combined with an in-memory stream is contradictory."""
        fs = pytest.importorskip("fsspec").filesystem("memory")
        with pytest.raises(ValueError):
            IO.open(io.BytesIO(b"not an image"), "r", format="tiff", filesystem=fs)

    def test_filesystem_single_path_write_mode_accepted(self):
        """``filesystem=`` with a single path is accepted in write mode."""
        fs = pytest.importorskip("fsspec").filesystem("memory")
        # Single-path write routes through the remote writer; the returned
        # object is a DatasetWriter (no exception up front).
        writer = IO.open("out.tif", "w", format="tiff", filesystem=fs)
        writer.close()

    def test_filesystem_multipath_write_mode_accepted(self):
        """``filesystem=`` with a multi-path R-set list is accepted in write mode.

        Multi-path remote write (an R-set pyramid to multiple remote keys) is the
        symmetric twin of multi-path remote read; the returned object is a
        DatasetWriter (no exception up front). Full commit/round-trip behavior is
        covered in ``test_remote_write.py``.
        """
        fs = pytest.importorskip("fsspec").filesystem("memory")
        writer = IO.open(["out.tif", "out.tif.r1"], "w", format="tiff", filesystem=fs)
        writer.close()

    def test_plain_local_path_unaffected(self):
        """A plain local path still opens (memory-mapped) with no filesystem=."""
        if not TIFF_FILE.exists():
            pytest.skip(f"{TIFF_FILE} unavailable")
        local = imread(str(TIFF_FILE))
        full = imread(io.BytesIO(TIFF_FILE.read_bytes()), format="tiff")
        np.testing.assert_array_equal(local, full)


# ===========================================================================
# VirtualiZarr index construction (skipped without virtualizarr/fsspec)
# ===========================================================================


class TestVirtualiZarrIndexConstruction:
    """Index construction over a remote-capable source. Skipped when the optional
    virtualizarr dependency is unavailable."""

    def test_index_construction_succeeds(self):
        pytest.importorskip("virtualizarr", minversion="2.0")
        pytest.importorskip("fsspec")
        if not NC_NITF_FILE.exists():
            pytest.skip(f"{NC_NITF_FILE} unavailable")

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        # The parser builds its manifest via IO.open over the file path; this
        # exercises the full index-construction bootstrap the design targets.
        # virtualizarr requires absolute posix paths / URIs in the manifest.
        abs_path = str(NC_NITF_FILE.resolve())
        parser = OversightMLParser()
        store = parser(abs_path)
        assert store is not None


# ===========================================================================
# Fallback: monolithic formats and non-seekable / unknown-size streams
# ===========================================================================


class TestFullReadFallback:
    """Non-seekable streams and monolithic formats use the full-read path and
    still decode correctly."""

    def test_non_seekable_tiff_falls_back(self):
        if not TIFF_FILE.exists():
            pytest.skip(f"{TIFF_FILE} unavailable")
        file_bytes = TIFF_FILE.read_bytes()

        remote_ref = _decode_via_stream(file_bytes, "tiff", io.BytesIO(file_bytes))
        fallback = _decode_via_stream(file_bytes, "tiff", _NonSeekableStream(file_bytes))
        np.testing.assert_array_equal(fallback, remote_ref)

    def test_png_uses_full_read_and_decodes(self):
        """PNG is monolithic: a seekable PNG stream must still decode correctly
        (via the full-read fallback, which IO.open selects for PNG)."""
        rng = np.random.default_rng(7)
        data = rng.integers(0, 255, (3, 8, 8), dtype=np.uint8)
        buf = io.BytesIO()
        from aws.osml.io import imsave

        imsave(buf, data, format="png")
        buf.seek(0)

        result = imread(buf, format="png")
        np.testing.assert_array_equal(result, data)

    def test_dted_non_seekable_falls_back(self):
        if not DTED_FILE.exists():
            pytest.skip(f"{DTED_FILE} unavailable")
        file_bytes = DTED_FILE.read_bytes()

        remote_ref = _decode_via_stream(file_bytes, "dted", io.BytesIO(file_bytes))
        fallback = _decode_via_stream(file_bytes, "dted", _NonSeekableStream(file_bytes))
        np.testing.assert_array_equal(fallback, remote_ref)
