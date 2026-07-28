"""Property test: remoteness is transparent to decoded output.

Decoding a block-capable format over a ``Remote`` ``OwnedBuffer`` (a seekable,
sized Python stream → range reads) must be byte-identical to decoding the same
bytes resident. We use a tiled TIFF as the representative block-capable format:
its libtiff callbacks pull byte ranges on demand, so it exercises the range path
that the ``Remote`` backing feeds.

The randomization is over image geometry (via the ``tiff_image_config``
strategy) and over the *order* in which the block grid is visited — the design
calls for randomized access order and block coordinates, since a range source
caches fetched ranges and a correct implementation must be order-independent.
"""

import io

import numpy as np
import pytest
from aws.osml.io import IO
from hypothesis import assume, given
from hypothesis import strategies as st

from .conftest import pbt_settings
from .helpers import write_tiff_native_bytes
from .strategies import get_numpy_dtype, tiff_image_config


class _RangeLoggingStream:
    """A seekable, sized stream that logs read ranges (drives the Remote path)."""

    def __init__(self, data: bytes):
        self._buf = io.BytesIO(data)
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


def _make_array(config):
    width, height, bands = config["width"], config["height"], config["bands"]
    dtype = get_numpy_dtype(config["pixel_type"])
    rng = np.random.RandomState(13)
    if np.issubdtype(dtype, np.floating):
        return rng.rand(bands, height, width).astype(dtype)
    if np.issubdtype(dtype, np.signedinteger):
        info = np.iinfo(dtype)
        return rng.randint(info.min, info.max + 1, (bands, height, width), dtype=dtype)
    info = np.iinfo(dtype)
    return rng.randint(0, info.max + 1, (bands, height, width), dtype=dtype)


@pytest.mark.property
class TestRemoteDecodeTransparency:
    """A tiled TIFF decoded over a Remote stream matches the resident decode,
    block-for-block, regardless of visit order."""

    @given(config=tiff_image_config(min_size=16, max_size=96), seed=st.integers(0, 2**16))
    @pbt_settings
    def test_remote_blocks_match_resident(self, config, seed):
        dtype = get_numpy_dtype(config["pixel_type"])
        rps = config["rows_per_strip"]
        # Mirror the guard the other TIFF block tests use for multi-byte strips.
        assume(not (rps >= config["height"] and dtype.itemsize > 1))

        array_chw = _make_array(config)
        tiff_bytes = write_tiff_native_bytes(config, array_chw)

        # Resident reference: decode every block from an in-memory (Heap) read.
        resident_blocks = {}
        with IO.open(io.BytesIO(tiff_bytes), "r", format="tiff") as reader:
            asset = reader.get_asset(reader.get_asset_keys()[0])
            grid_rows, grid_cols = asset.block_grid_size
            for br in range(grid_rows):
                for bc in range(grid_cols):
                    resident_blocks[(br, bc)] = np.array(asset.get_block(br, bc, 0).data)

        # Randomize the block visit order for the Remote decode.
        coords = list(resident_blocks.keys())
        rng = np.random.RandomState(seed)
        rng.shuffle(coords)

        stream = _RangeLoggingStream(tiff_bytes)
        with IO.open(stream, "r", format="tiff") as reader:
            asset = reader.get_asset(reader.get_asset_keys()[0])
            for br, bc in coords:
                remote_block = np.array(asset.get_block(br, bc, 0).data)
                np.testing.assert_array_equal(
                    remote_block,
                    resident_blocks[(br, bc)],
                    err_msg=f"remote block ({br},{bc}) differs from resident",
                )
