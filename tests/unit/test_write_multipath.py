"""Tests for IO.open() write-mode changes: format auto-detection and multi-path writes.

This module tests:
- Single-path write with format auto-detection from file extension
- Multi-path write creating a CompositeDatasetWriter for R-set pyramids
- Multi-path write rejection when additional paths lack .rN suffix
- Format auto-detection with .rN suffix stripping

Requirements: 4.1, 4.2, 4.3, 6.1, 6.2, 6.4
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


@pytest.fixture()
def tmp_dir():
    """Provide a temporary directory that is cleaned up after the test."""
    d = tempfile.mkdtemp()
    yield Path(d)
    shutil.rmtree(d, ignore_errors=True)


def _create_provider(key: str, num_cols: int, num_rows: int, num_bands: int = 1):
    """Create a BufferedImageAssetProvider with synthetic data."""
    provider = BufferedImageAssetProvider.create(
        key=key,
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, 256),
        block_height=min(num_rows, 256),
    )
    data = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
    provider.set_full_image(data)
    return provider


# =============================================================================
# 13.1  Single-path write with format auto-detection
# =============================================================================


class TestSinglePathFormatAutoDetection:
    """Verify IO.open() auto-detects format from file extension in write mode.

    Requirements: 6.1, 6.4
    """

    def test_ntf_extension_autodetects_nitf(self, tmp_dir):
        """IO.open(["output.ntf"], "w") with format=None creates a NITF writer."""
        path = tmp_dir / "output.ntf"
        writer = IO.open([str(path)], "w")
        provider = _create_provider("image:0", 64, 64)
        writer.add_asset("image:0", provider, "Test", "test", ["data"])
        writer.close()

        # Verify the file was written and is readable as NITF
        assert path.exists()
        with IO.open([str(path)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys

    def test_tif_extension_autodetects_tiff(self, tmp_dir):
        """IO.open(["output.tif"], "w") with format=None creates a TIFF writer."""
        path = tmp_dir / "output.tif"
        writer = IO.open([str(path)], "w")
        provider = _create_provider("image:0", 64, 64)
        writer.add_asset("image:0", provider, "Test", "test", ["data"])
        writer.close()

        # Verify the file was written and is readable as TIFF
        assert path.exists()
        with IO.open([str(path)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys

    def test_unrecognized_extension_raises_valueerror(self, tmp_dir):
        """IO.open(["output.xyz"], "w") with format=None raises ValueError."""
        path = tmp_dir / "output.xyz"
        with pytest.raises(ValueError, match="Cannot determine output format"):
            IO.open([str(path)], "w")


# =============================================================================
# 13.2  Multi-path write
# =============================================================================


class TestMultiPathWrite:
    """Verify IO.open() with multiple write paths creates a CompositeDatasetWriter.

    Requirements: 4.1, 4.2
    """

    def test_multi_path_write_creates_rset_files(self, tmp_dir):
        """Write base + overview assets via multi-path, verify files exist and are readable."""
        base_path = tmp_dir / "out.ntf"
        r1_path = tmp_dir / "out.ntf.r1"
        r2_path = tmp_dir / "out.ntf.r2"

        writer = IO.open(
            [str(base_path), str(r1_path), str(r2_path)], "w", "nitf"
        )

        # Add base asset
        base_provider = _create_provider("image:0", 256, 256, num_bands=3)
        writer.add_asset("image:0", base_provider, "Base", "base image", ["data"])

        # Add overview assets
        ovr1_provider = _create_provider("image:0", 128, 128, num_bands=3)
        writer.add_asset(
            "image:0:overview:1", ovr1_provider, "Overview 1", "overview", ["overview"]
        )

        ovr2_provider = _create_provider("image:0", 64, 64, num_bands=3)
        writer.add_asset(
            "image:0:overview:2", ovr2_provider, "Overview 2", "overview", ["overview"]
        )

        writer.close()

        # Verify all output files exist
        assert base_path.exists(), "Base file should exist"
        assert r1_path.exists(), "R1 file should exist"
        assert r2_path.exists(), "R2 file should exist"

        # Verify files are readable via multi-path read
        with IO.open(
            [str(base_path), str(r1_path), str(r2_path)], "r"
        ) as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys
            assert "image:0:overview:1" in keys
            assert "image:0:overview:2" in keys

            # Verify dimensions
            base_asset = reader.get_asset("image:0")
            assert base_asset.num_columns == 256
            assert base_asset.num_rows == 256

            ovr1_asset = reader.get_asset("image:0:overview:1")
            assert ovr1_asset.num_columns == 128
            assert ovr1_asset.num_rows == 128

            ovr2_asset = reader.get_asset("image:0:overview:2")
            assert ovr2_asset.num_columns == 64
            assert ovr2_asset.num_rows == 64


# =============================================================================
# 13.3  Multi-path write with invalid rset path
# =============================================================================


class TestMultiPathWriteInvalidRset:
    """Verify IO.open() rejects additional paths without .rN suffix in write mode.

    Requirements: 4.3
    """

    def test_non_rset_additional_path_raises_valueerror(self, tmp_dir):
        """IO.open(["out.ntf", "out_extra.ntf"], "w", "nitf") raises ValueError."""
        base_path = tmp_dir / "out.ntf"
        extra_path = tmp_dir / "out_extra.ntf"

        with pytest.raises(ValueError, match="R-set pattern"):
            IO.open([str(base_path), str(extra_path)], "w", "nitf")


# =============================================================================
# 13.4  Format auto-detection with rset suffix
# =============================================================================


class TestFormatAutoDetectionRsetSuffix:
    """Verify format auto-detection strips .rN suffix before detecting format.

    Requirements: 6.2
    """

    def test_rset_suffix_stripped_for_format_detection(self, tmp_dir):
        """IO.open(["out.ntf.r1"], "w") with format=None detects NITF format."""
        path = tmp_dir / "out.ntf.r1"
        writer = IO.open([str(path)], "w")
        provider = _create_provider("image:0", 64, 64)
        writer.add_asset("image:0", provider, "Test", "test", ["data"])
        writer.close()

        # Verify the file was written and is readable as NITF
        # (must specify format on read since .r1 extension is not auto-detected by reader)
        assert path.exists()
        with IO.open([str(path)], "r", "nitf") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys
