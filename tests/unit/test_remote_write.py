"""Integration tests for the remote (fsspec) write path.

Proves that ``IO.open`` and ``imsave`` can write to a remote destination — a
bare ``memory://`` / ``s3://``-style URL or an explicit ``filesystem=`` — with
the library owning and committing (closing) the handle it opened, while a
caller-supplied stream is left open. An in-memory ``fsspec`` ``MemoryFileSystem``
stands in for S3 (same ``AbstractFileSystem`` contract, no network).

Mirrors the fixture patterns in ``test_remote_range_read.py`` (the
``fsspec.filesystem("memory")`` / ``memory://`` stand-in for S3) and the write
round-trip helpers in ``test_stream_io.py``.
"""

import io
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
    imread,
    imsave,
)

UNIT_DATA = Path("data/unit")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_test_image(height: int = 8, width: int = 8, bands: int = 3) -> np.ndarray:
    """Create a small deterministic test image in CHW layout."""
    rng = np.random.default_rng(42)
    return rng.integers(0, 255, (bands, height, width), dtype=np.uint8)


def _write_image_asset(writer, data: np.ndarray) -> None:
    """Add a single full-image asset to an open ``DatasetWriter``.

    Extracted so the several write tests exercise the exact same asset build as
    the ``test_stream_io.py`` BytesIO write cases, differing only in the sink.
    """
    bands, height, width = data.shape
    metadata = BufferedMetadataProvider()
    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=width,
        num_rows=height,
        num_bands=bands,
        block_width=width,
        block_height=height,
        pixel_type=PixelType.UInt8,
        metadata=metadata,
        title="Test",
        description="test image",
    )
    provider.set_full_image(np.ascontiguousarray(data))
    writer.add_asset(
        key="image:0",
        provider=provider,
        title="Test",
        description="test image",
        roles=["data"],
    )


def _local_png_bytes(data: np.ndarray) -> bytes:
    """Encode ``data`` as PNG to a local BytesIO for byte-for-byte comparison."""
    buf = io.BytesIO()
    imsave(buf, data, format="png")
    return buf.getvalue()


# ---------------------------------------------------------------------------
# IO.open remote write via a bare URL
# ---------------------------------------------------------------------------


class TestIoOpenRemoteWriteViaUrl:
    """A bare ``memory://`` URL routes ``IO.open(..., "w", ...)`` through the
    remote writer and commits the object when the block exits."""

    def test_write_png_via_memory_url_byte_identical_to_local(self):
        """Bytes written over a ``memory://`` URL equal a local write of the
        same array."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()

        with IO.open("memory://remote-write/out.png", "w", "png") as writer:
            _write_image_asset(writer, data)

        fs = fsspec.filesystem("memory")
        remote_bytes = fs.cat_file("/remote-write/out.png")

        assert remote_bytes[:4] == b"\x89PNG"
        assert remote_bytes == _local_png_bytes(data)

    def test_commit_on_block_exit(self):
        """After the ``with`` block exits, the object is present and complete in
        the memory FS — proof the library closed the handle it opened."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()
        fs = fsspec.filesystem("memory")
        path = "/remote-write/commit.png"

        with IO.open(f"memory://{path}", "w", "png") as writer:
            _write_image_asset(writer, data)

        # The object exists and its size matches a fully-encoded local write.
        assert fs.exists(path)
        assert fs.info(path)["size"] == len(_local_png_bytes(data))

    def test_roundtrip_via_imread(self):
        """Write over ``memory://`` then read back with ``imread`` →
        pixel-identical."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()

        with IO.open("memory://remote-write/roundtrip.png", "w", "png") as writer:
            _write_image_asset(writer, data)

        fs = fsspec.filesystem("memory")
        recovered = imread(io.BytesIO(fs.cat_file("/remote-write/roundtrip.png")), format="png")
        np.testing.assert_array_equal(recovered, data)

    def test_roundtrip_via_url(self):
        """Write over ``memory://`` then read back through the same URL →
        pixel-identical."""
        pytest.importorskip("fsspec")
        data = _make_test_image()
        url = "memory://remote-write/roundtrip-url.png"

        with IO.open(url, "w", "png") as writer:
            _write_image_asset(writer, data)

        recovered = imread(url)
        np.testing.assert_array_equal(recovered, data)


# ---------------------------------------------------------------------------
# IO.open remote write via explicit filesystem=
# ---------------------------------------------------------------------------


class TestIoOpenRemoteWriteViaFilesystem:
    """A scheme-less key plus ``filesystem=`` resolves through the shared fs and
    commits."""

    def test_write_scheme_less_key_commits_and_reads_back(self):
        """``IO.open("out.png", "w", "png", filesystem=fs)`` commits and reads
        back pixel-identically."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()
        fs = fsspec.filesystem("memory")

        with IO.open("fs-write/out.png", "w", "png", filesystem=fs) as writer:
            _write_image_asset(writer, data)

        # Readable back through the same fs, pixel-identical.
        recovered = imread(io.BytesIO(fs.cat_file("/fs-write/out.png")), format="png")
        np.testing.assert_array_equal(recovered, data)

    def test_write_via_filesystem_byte_identical_to_local(self):
        """Bytes written through ``filesystem=`` equal a local write."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()
        fs = fsspec.filesystem("memory")

        with IO.open("fs-write/identical.png", "w", "png", filesystem=fs) as writer:
            _write_image_asset(writer, data)

        assert fs.cat_file("/fs-write/identical.png") == _local_png_bytes(data)


# ---------------------------------------------------------------------------
# Caller-owned handle is not closed by the writer
# ---------------------------------------------------------------------------


class _CloseRecordingStream:
    """A writable file-like object that records whether ``.close()`` was called.

    Buffers writes so the encoded output stays recoverable, and reports
    ``closed`` so a test can assert the library left a caller-supplied handle
    open (owned-vs-borrowed: the caller closes its own handle, not the library).
    """

    def __init__(self):
        self._buf = bytearray()
        self.closed = False

    def write(self, data) -> int:
        self._buf.extend(data)
        return len(data)

    def flush(self) -> None:
        pass

    def close(self) -> None:
        self.closed = True

    def getvalue(self) -> bytes:
        return bytes(self._buf)


class TestCallerOwnedHandleNotClosed:
    """A caller-supplied stream is left open after the writer's ``close()`` —
    only a library-opened remote handle is closed by the library."""

    def test_caller_stream_not_closed(self):
        data = _make_test_image()
        stream = _CloseRecordingStream()

        with IO.open(stream, "w", "png") as writer:
            _write_image_asset(writer, data)

        # The writer finalized (bytes are present) but did NOT close the
        # caller's handle.
        assert stream.getvalue()[:4] == b"\x89PNG"
        assert not stream.closed, "library must not close a caller-supplied handle"


# ---------------------------------------------------------------------------
# imsave remote write
# ---------------------------------------------------------------------------


class TestImsaveRemoteWrite:
    """``imsave`` commits to a remote destination via ``filesystem=`` and via a
    bare S3-style (``memory://``) URL."""

    def test_imsave_with_filesystem(self):
        """``imsave(path, arr, filesystem=fs)`` commits and reads back."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()
        fs = fsspec.filesystem("memory")

        imsave("imsave-fs/out.tif", data, filesystem=fs)

        recovered = imread(io.BytesIO(fs.cat_file("/imsave-fs/out.tif")), format="tiff")
        np.testing.assert_array_equal(recovered, data)

    def test_imsave_with_memory_url(self):
        """``imsave`` to a bare ``memory://`` (S3-shaped) URL commits without a
        signature change."""
        fsspec = pytest.importorskip("fsspec")
        data = _make_test_image()

        imsave("memory://imsave-url/out.tif", data)

        fs = fsspec.filesystem("memory")
        recovered = imread(io.BytesIO(fs.cat_file("/imsave-url/out.tif")), format="tiff")
        np.testing.assert_array_equal(recovered, data)


# ---------------------------------------------------------------------------
# Negative guards
# ---------------------------------------------------------------------------


class TestRemoteWriteNegativeGuards:
    """``filesystem=`` is rejected with a stream (which carries its own transport).

    A multi-path list in write mode is now *accepted* (multi-path remote write is
    supported — see ``test_multi_path_io.py``), so only the stream guards remain.
    """

    def test_filesystem_with_stream_write_raises(self):
        """``filesystem=`` + a file-like stream in write mode raises ValueError."""
        fs = pytest.importorskip("fsspec").filesystem("memory")
        with pytest.raises(ValueError):
            IO.open(io.BytesIO(), "w", "png", filesystem=fs)

    def test_imsave_filesystem_with_stream_raises(self):
        """``imsave(stream, arr, filesystem=fs)`` raises ValueError (mirrors
        ``imread``'s stream+filesystem guard)."""
        fs = pytest.importorskip("fsspec").filesystem("memory")
        data = _make_test_image()
        with pytest.raises(ValueError):
            imsave(io.BytesIO(), data, format="png", filesystem=fs)
