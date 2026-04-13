"""Tests for multi-path IO.open() R-set support.

This module tests the multi-path R-set detection in IO.open(), verifying that
when multiple paths are provided with .rN suffixes, the reader correctly
exposes overview assets keyed as image:0:overview:N.

Requirements: 4.1, 4.2, 4.3
"""

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
