"""Tests for multi-path IO.open() R-set support.

This module tests the multi-path R-set detection in IO.open(), verifying that
when multiple paths are provided with .rN suffixes, the reader correctly
exposes overview assets keyed as image:0:overview:N.

It also covers opening a multi-file pyramid over a *remote* fsspec filesystem:
a list of remote URLs, a list plus a shared ``filesystem=``, explicit roles, and
a mixed local/remote list — each pixel-identical to the equivalent local open.
An in-memory ``MemoryFileSystem`` stands in for S3 (same fsspec contract, no
network), mirroring ``tests/unit/test_remote_range_read.py``.

Requirements: 4.1, 4.2, 4.3
"""

import io
import shutil
import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    AssetType,
    BufferedImageAssetProvider,
)

# Paths to existing unit test NITF files
BASE_NITF = Path("data/unit/nitf21-256x256-3band-8bit-nc.ntf")
SMALL_NITF = Path("data/unit/nitf21-8x8-1band-8bit-nc.ntf")


def _write_nitf(path: Path, num_cols: int, num_rows: int, num_bands: int = 1) -> None:
    """Write a minimal NITF file with the given dimensions."""
    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, 256),
        block_height=min(num_rows, 256),
    )
    data = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
    provider.set_full_image(data)

    writer = IO.open([str(path)], "w", "nitf")
    writer.add_asset("image:0", provider, "Image", "test", ["data"])
    writer.close()


@pytest.fixture()
def tmp_dir():
    """Provide a temporary directory that is cleaned up after the test."""
    d = tempfile.mkdtemp()
    yield Path(d)
    shutil.rmtree(d, ignore_errors=True)


# =============================================================================
# 2.1  Multi-path IO.open() basic test
# =============================================================================


class TestMultiPathIOOpen:
    """Verify IO.open() with two paths produces base + overview assets.

    Requirements: 4.1, 4.2, 4.3
    """

    def test_two_file_rset_produces_base_and_overview(self, tmp_dir):
        """Open base + .r1 file and verify asset keys, dimensions, and byte ranges."""
        base_path = tmp_dir / "image.ntf"
        rset_path = tmp_dir / "image.ntf.r1"

        _write_nitf(base_path, num_cols=512, num_rows=512, num_bands=3)
        _write_nitf(rset_path, num_cols=128, num_rows=128, num_bands=3)

        with IO.open([str(base_path), str(rset_path)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)

            # Must contain both base and overview
            assert "image:0" in keys
            assert "image:0:overview:1" in keys

            # Base asset has the larger dimensions
            base = reader.get_asset("image:0")
            assert base.num_columns == 512
            assert base.num_rows == 512

            # Overview asset has the smaller dimensions
            ovr = reader.get_asset("image:0:overview:1")
            assert ovr.num_columns == 128
            assert ovr.num_rows == 128

            # Both assets have valid tile_byte_ranges
            base_ranges = base.tile_byte_ranges()
            assert base_ranges is not None
            assert len(base_ranges) > 0

            ovr_ranges = ovr.tile_byte_ranges()
            assert ovr_ranges is not None
            assert len(ovr_ranges) > 0

    def test_rset_roles(self, tmp_dir):
        """Verify base has 'data' role and overview has 'overview' role."""
        base_path = tmp_dir / "image.ntf"
        rset_path = tmp_dir / "image.ntf.r1"

        _write_nitf(base_path, num_cols=64, num_rows=64)
        _write_nitf(rset_path, num_cols=32, num_rows=32)

        with IO.open([str(base_path), str(rset_path)], "r") as reader:
            base = reader.get_asset("image:0")
            assert "data" in base.roles

            ovr = reader.get_asset("image:0:overview:1")
            assert "overview" in ovr.roles

    def test_has_asset(self, tmp_dir):
        """Verify has_asset works for both base and overview keys."""
        base_path = tmp_dir / "image.ntf"
        rset_path = tmp_dir / "image.ntf.r1"

        _write_nitf(base_path, num_cols=64, num_rows=64)
        _write_nitf(rset_path, num_cols=32, num_rows=32)

        with IO.open([str(base_path), str(rset_path)], "r") as reader:
            assert reader.has_asset("image:0")
            assert reader.has_asset("image:0:overview:1")
            assert not reader.has_asset("image:0:overview:2")

    def test_tile_byte_ranges_have_valid_entries(self, tmp_dir):
        """Verify tile_byte_ranges entries have (offset, length) tuples."""
        base_path = tmp_dir / "image.ntf"
        rset_path = tmp_dir / "image.ntf.r1"

        _write_nitf(base_path, num_cols=64, num_rows=64)
        _write_nitf(rset_path, num_cols=32, num_rows=32)

        with IO.open([str(base_path), str(rset_path)], "r") as reader:
            for key in ["image:0", "image:0:overview:1"]:
                asset = reader.get_asset(key)
                ranges = asset.tile_byte_ranges()
                assert ranges is not None
                for (row, col), range_list in ranges.items():
                    assert isinstance(row, int)
                    assert isinstance(col, int)
                    assert len(range_list) > 0
                    for offset, length in range_list:
                        assert offset >= 0
                        assert length > 0


# =============================================================================
# 2.2  Out-of-order R-set paths
# =============================================================================


class TestOutOfOrderRSetPaths:
    """Verify overview levels come from filenames, not list position.

    Requirements: 4.1, 4.3
    """

    def test_out_of_order_rset_levels(self, tmp_dir):
        """Pass r3 before r1 in the list; levels should match filenames."""
        base_path = tmp_dir / "img.ntf"
        r1_path = tmp_dir / "img.ntf.r1"
        r3_path = tmp_dir / "img.ntf.r3"

        _write_nitf(base_path, num_cols=256, num_rows=256)
        _write_nitf(r1_path, num_cols=64, num_rows=64)
        _write_nitf(r3_path, num_cols=16, num_rows=16)

        # Deliberately pass r3 before r1
        with IO.open([str(base_path), str(r3_path), str(r1_path)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)

            assert "image:0" in keys
            assert "image:0:overview:1" in keys
            assert "image:0:overview:3" in keys
            # No overview:2 — there's no .r2 file
            assert "image:0:overview:2" not in keys

    def test_out_of_order_dimensions_correct(self, tmp_dir):
        """Verify each overview level has the dimensions from its source file."""
        base_path = tmp_dir / "img.ntf"
        r1_path = tmp_dir / "img.ntf.r1"
        r3_path = tmp_dir / "img.ntf.r3"

        _write_nitf(base_path, num_cols=256, num_rows=256)
        _write_nitf(r1_path, num_cols=64, num_rows=64)
        _write_nitf(r3_path, num_cols=16, num_rows=16)

        with IO.open([str(base_path), str(r3_path), str(r1_path)], "r") as reader:
            assert reader.get_asset("image:0").num_columns == 256
            assert reader.get_asset("image:0:overview:1").num_columns == 64
            assert reader.get_asset("image:0:overview:3").num_columns == 16


# =============================================================================
# 2.3  Single-path backward compatibility
# =============================================================================


class TestSinglePathBackwardCompat:
    """Verify single-path IO.open() behaves identically to current implementation.

    Requirements: 4.1
    """

    def test_single_path_no_overviews(self, tmp_dir):
        """A single NITF file should produce only image:0, no overviews."""
        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=3)

        with IO.open([str(path)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert keys == ["image:0"]
            assert not reader.has_asset("image:0:overview:1")

    def test_single_path_asset_properties(self, tmp_dir):
        """Single-path asset should have correct dimensions and data role."""
        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=3)

        with IO.open([str(path)], "r") as reader:
            asset = reader.get_asset("image:0")
            assert asset.num_columns == 128
            assert asset.num_rows == 128
            assert asset.num_bands == 3
            assert "data" in asset.roles

    def test_single_path_tile_byte_ranges(self, tmp_dir):
        """Single-path tile_byte_ranges should be valid."""
        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128)

        with IO.open([str(path)], "r") as reader:
            asset = reader.get_asset("image:0")
            ranges = asset.tile_byte_ranges()
            assert ranges is not None
            assert len(ranges) > 0

    def test_single_path_matches_existing_data(self):
        """Opening an existing unit test NITF should work as before."""
        if not BASE_NITF.exists():
            pytest.skip("Unit test data not available")

        with IO.open([str(BASE_NITF)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys

            asset = reader.get_asset("image:0")
            assert asset.num_columns == 256
            assert asset.num_rows == 256
            assert asset.num_bands == 3


# =============================================================================
# Edge cases
# =============================================================================


class TestMultiPathEdgeCases:
    """Edge case tests for multi-path IO.open()."""

    def test_non_rset_additional_path_rejected(self, tmp_dir):
        """Additional paths without .rN suffix should raise ValueError."""
        base_path = tmp_dir / "image.ntf"
        other_path = tmp_dir / "other.ntf"

        _write_nitf(base_path, num_cols=64, num_rows=64)
        _write_nitf(other_path, num_cols=32, num_rows=32)

        with pytest.raises(ValueError, match="R-set pattern"):
            IO.open([str(base_path), str(other_path)], "r")


# =============================================================================
# Remote multi-path pyramids over a fsspec MemoryFileSystem (stands in for S3)
# =============================================================================


class TestMultiPathRemotePyramid:
    """Open a multi-file R-set pyramid over a remote fsspec filesystem.

    A ``MemoryFileSystem`` (``memory://`` URLs) stands in for S3 — same fsspec
    ``AsyncFileSystem`` contract, no network. Each remote open is asserted
    pixel-identical to the equivalent local open, proving remoteness is
    transparent to the multi-path read path.

    Requirements: 4.1, 4.2, 4.3
    """

    # Distinct, non-flat pixel data so a mis-routed read would show up as a
    # pixel diff rather than matching zeros by accident.
    @staticmethod
    def _write_nitf_data(path: Path, num_cols: int, num_rows: int, num_bands: int) -> np.ndarray:
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=num_cols,
            num_rows=num_rows,
            num_bands=num_bands,
            block_width=min(num_cols, 256),
            block_height=min(num_rows, 256),
        )
        data = (
            np.arange(num_bands * num_rows * num_cols, dtype=np.uint8)
            .reshape(num_bands, num_rows, num_cols)
        )
        provider.set_full_image(data)
        writer = IO.open([str(path)], "w", "nitf")
        writer.add_asset("image:0", provider, "Image", "test", ["data"])
        writer.close()
        return data

    @pytest.fixture()
    def pyramid(self, tmp_dir):
        """Write a base + .r1 pyramid locally and upload the bytes to a fresh
        in-memory fsspec filesystem.

        Returns ``(fs, base_local, r1_local, base_key, r1_key)`` where the
        ``*_key`` values are the scheme-less keys within the memory store.
        Skips if fsspec is unavailable.
        """
        fsspec = pytest.importorskip("fsspec")

        base_local = tmp_dir / "image.ntf"
        r1_local = tmp_dir / "image.ntf.r1"
        self._write_nitf_data(base_local, num_cols=512, num_rows=512, num_bands=3)
        self._write_nitf_data(r1_local, num_cols=128, num_rows=128, num_bands=3)

        fs = fsspec.filesystem("memory")
        base_key = "/multipath-pyramid/image.ntf"
        r1_key = "/multipath-pyramid/image.ntf.r1"
        fs.pipe_file(base_key, base_local.read_bytes())
        fs.pipe_file(r1_key, r1_local.read_bytes())
        return fs, base_local, r1_local, base_key, r1_key

    @staticmethod
    def _base_and_overview_blocks(reader):
        """Return (base_pixels, overview_pixels) for the first R-set level."""
        base = np.array(reader.get_asset("image:0").get_block(0, 0, 0).data)
        ovr = np.array(reader.get_asset("image:0:overview:1").get_block(0, 0, 0).data)
        return base, ovr

    def _local_reference_blocks(self, base_local, r1_local):
        with IO.open([str(base_local), str(r1_local)], "r") as reader:
            return self._base_and_overview_blocks(reader)

    def test_url_list_no_roles_matches_local(self, pyramid):
        """A list of ``memory://`` URLs (no roles, .rN detection) produces base +
        overview:1, pixel-identical to the local open."""
        fs, base_local, r1_local, base_key, r1_key = pyramid
        urls = [f"memory://{base_key}", f"memory://{r1_key}"]

        with IO.open(urls, "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys
            assert "image:0:overview:1" in keys

            base = reader.get_asset("image:0")
            assert base.num_columns == 512
            assert base.num_rows == 512
            ovr = reader.get_asset("image:0:overview:1")
            assert ovr.num_columns == 128
            assert ovr.num_rows == 128

            remote_base, remote_ovr = self._base_and_overview_blocks(reader)

        local_base, local_ovr = self._local_reference_blocks(base_local, r1_local)
        np.testing.assert_array_equal(remote_base, local_base)
        np.testing.assert_array_equal(remote_ovr, local_ovr)

    def test_explicit_roles_over_memory(self, pyramid):
        """A ``memory://`` URL list with explicit roles opens correctly."""
        fs, base_local, r1_local, base_key, r1_key = pyramid
        urls = [f"memory://{base_key}", f"memory://{r1_key}"]

        with IO.open(urls, "r", roles=[["data"], ["overview:1"]]) as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys
            assert "image:0:overview:1" in keys

            remote_base, remote_ovr = self._base_and_overview_blocks(reader)

        local_base, local_ovr = self._local_reference_blocks(base_local, r1_local)
        np.testing.assert_array_equal(remote_base, local_base)
        np.testing.assert_array_equal(remote_ovr, local_ovr)

    def test_shared_filesystem_with_scheme_less_keys(self, pyramid):
        """``filesystem=fs`` with plain (scheme-less) keys resolves each entry
        through the shared filesystem."""
        fs, base_local, r1_local, base_key, r1_key = pyramid

        with IO.open([base_key, r1_key], "r", filesystem=fs) as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys
            assert "image:0:overview:1" in keys

            remote_base, remote_ovr = self._base_and_overview_blocks(reader)

        local_base, local_ovr = self._local_reference_blocks(base_local, r1_local)
        np.testing.assert_array_equal(remote_base, local_base)
        np.testing.assert_array_equal(remote_ovr, local_ovr)

    def test_mixed_local_base_remote_overview(self, pyramid):
        """A list mixing a local base with a ``memory://`` overview opens
        correctly (per-entry routing) and matches the all-local open."""
        fs, base_local, r1_local, base_key, r1_key = pyramid
        mixed = [str(base_local), f"memory://{r1_key}"]

        with IO.open(mixed, "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys
            assert "image:0:overview:1" in keys

            remote_base, remote_ovr = self._base_and_overview_blocks(reader)

        local_base, local_ovr = self._local_reference_blocks(base_local, r1_local)
        np.testing.assert_array_equal(remote_base, local_base)
        np.testing.assert_array_equal(remote_ovr, local_ovr)

    def test_filesystem_with_list_write_mode_raises(self, pyramid):
        """``filesystem=`` with a list in write mode is rejected (remote write
        is out of scope)."""
        fs, _base_local, _r1_local, base_key, r1_key = pyramid
        with pytest.raises(ValueError):
            IO.open([base_key, r1_key], "w", "nitf", filesystem=fs)

    def test_filesystem_with_stream_list_raises(self):
        """``filesystem=`` combined with a StreamList (list of file-like objects)
        is contradictory and rejected."""
        fs = pytest.importorskip("fsspec").filesystem("memory")
        with pytest.raises(ValueError):
            IO.open(
                [io.BytesIO(b"not an image"), io.BytesIO(b"nor this")],
                "r",
                roles=[["data"], ["overview:1"]],
                filesystem=fs,
            )
