"""Tests for JPEG DCT compression support in Python bindings.

This module tests the JPEG DCT (IC=C3/M3/I1) compression features exposed through
Python bindings, including:
- Reading and writing JPEG compressed images
- Basic roundtrip with IC=C3
- JPEG encoding hints via BufferedMetadataProvider

Requirements: 1.1, 2.1, 5.1, 5.4, 7.1, 7.2
"""

from pathlib import Path
import tempfile

import numpy as np
import pytest

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)


UNIT_DATA_DIR = Path("data/unit")


class TestJpegEncodingHints:
    """Tests for JPEG encoding hints via BufferedMetadataProvider."""

    def test_set_ic_c3(self):
        """Test setting IC=C3 for JPEG DCT compression."""
        metadata = BufferedMetadataProvider()
        metadata.set("IC", "C3")
        assert metadata.get("IC") == "C3"

    def test_set_ic_m3(self):
        """Test setting IC=M3 for masked JPEG DCT."""
        metadata = BufferedMetadataProvider()
        metadata.set("IC", "M3")
        assert metadata.get("IC") == "M3"

    def test_set_ic_i1(self):
        """Test setting IC=I1 for downsampled JPEG."""
        metadata = BufferedMetadataProvider()
        metadata.set("IC", "I1")
        assert metadata.get("IC") == "I1"

    def test_set_comrat_jpeg_quality(self):
        """Test setting COMRAT for JPEG quality (0-100 mapped to 00.0-99.9)."""
        metadata = BufferedMetadataProvider()
        metadata.set("COMRAT", "75.0")  # Quality 75
        assert metadata.get("COMRAT") == "75.0"

    def test_all_jpeg_hints_together(self):
        """Test setting all JPEG encoding hints together."""
        metadata = BufferedMetadataProvider()
        metadata.set("IC", "C3")
        metadata.set("COMRAT", "85.0")
        metadata.set("NPPBH", "256")
        metadata.set("NPPBV", "256")
        
        assert metadata.get("IC") == "C3"
        assert metadata.get("COMRAT") == "85.0"
        assert metadata.get("NPPBH") == "256"
        assert metadata.get("NPPBV") == "256"


class TestJpegRoundtrip:
    """Tests for JPEG DCT roundtrip encoding/decoding."""

    def test_jpeg_c3_grayscale_roundtrip(self):
        """Test basic roundtrip with IC=C3 for 8-bit grayscale image.
        
        Validates: Requirements 1.1, 1.2, 2.1, 2.2, 7.1, 7.2
        """
        num_rows, num_cols, num_bands = 64, 64, 1
        
        # Create test image with gradient pattern
        original = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
        for r in range(num_rows):
            for c in range(num_cols):
                original[0, r, c] = (r + c) % 256
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for JPEG compression
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            metadata.set("COMRAT", "90.0")  # High quality for better roundtrip
            
            # Create image provider
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=num_cols,
                block_height=num_rows,
                pixel_type=PixelType.UInt8,
                metadata=metadata,
            )
            provider.set_full_image(original)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test JPEG Image",
                description="JPEG C3 roundtrip test",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Verify IC code in metadata
            asset_metadata = asset.get_metadata().as_dict()
            assert asset_metadata.get("IC", "").strip() == "C3"
            
            # Read the block
            decoded = asset.get_block(0, 0, 0)
            reader.close()
            
            # Verify shape preservation
            assert decoded.shape == original.shape, (
                f"Shape mismatch: expected {original.shape}, got {decoded.shape}"
            )
            
            # Verify dtype preservation
            assert decoded.dtype == original.dtype, (
                f"Dtype mismatch: expected {original.dtype}, got {decoded.dtype}"
            )
            
            # Verify lossy quality (PSNR >= 30 dB for high quality JPEG)
            mse = np.mean((original.astype(float) - decoded.astype(float)) ** 2)
            if mse > 0:
                psnr = 10 * np.log10(255.0 ** 2 / mse)
                assert psnr >= 30.0, f"PSNR {psnr:.2f} dB is below threshold"
            
        finally:
            if path.exists():
                path.unlink()

    def test_jpeg_c3_rgb_roundtrip(self):
        """Test roundtrip with IC=C3 for 8-bit RGB image.
        
        Validates: Requirements 1.4, 2.4, 7.1, 7.2
        """
        num_rows, num_cols, num_bands = 64, 64, 3
        
        # Create test RGB image
        original = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
        original[0, :, :] = 200  # Red channel
        original[1, :, :] = 100  # Green channel
        original[2, :, :] = 50   # Blue channel
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            metadata.set("COMRAT", "90.0")
            metadata.set("IREP", "RGB")
            metadata.set("IMODE", "P")  # Pixel interleaved for RGB
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=num_cols,
                block_height=num_rows,
                pixel_type=PixelType.UInt8,
                metadata=metadata,
            )
            provider.set_full_image(original)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test RGB JPEG Image",
                description="JPEG C3 RGB roundtrip test",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = asset.get_block(0, 0, 0)
            reader.close()
            
            # Verify shape and dtype
            assert decoded.shape == original.shape
            assert decoded.dtype == original.dtype
            
            # Verify quality
            mse = np.mean((original.astype(float) - decoded.astype(float)) ** 2)
            if mse > 0:
                psnr = 10 * np.log10(255.0 ** 2 / mse)
                assert psnr >= 30.0, f"PSNR {psnr:.2f} dB is below threshold"
            
        finally:
            if path.exists():
                path.unlink()

    def test_jpeg_multiblock_roundtrip(self):
        """Test roundtrip with IC=C3 for multi-block image.
        
        Validates: Requirements 1.1, 2.1
        """
        num_rows, num_cols, num_bands = 128, 128, 1
        block_size = 64
        
        # Create test image with gradient pattern (more compressible than random)
        original = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
        for r in range(num_rows):
            for c in range(num_cols):
                original[0, r, c] = (r + c) % 256
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            metadata.set("COMRAT", "95.0")  # Very high quality
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=block_size,
                block_height=block_size,
                pixel_type=PixelType.UInt8,
                metadata=metadata,
            )
            provider.set_full_image(original)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Multi-block JPEG",
                description="JPEG C3 multi-block test",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Verify block grid
            grid_rows, grid_cols = asset.block_grid_size
            assert grid_rows == 2
            assert grid_cols == 2
            
            # Read all blocks and reassemble
            decoded = np.zeros_like(original)
            for br in range(grid_rows):
                for bc in range(grid_cols):
                    block = asset.get_block(br, bc, 0)
                    r_start = br * block_size
                    c_start = bc * block_size
                    r_end = min(r_start + block.shape[1], num_rows)
                    c_end = min(c_start + block.shape[2], num_cols)
                    decoded[:, r_start:r_end, c_start:c_end] = block[:, :r_end-r_start, :c_end-c_start]
            
            reader.close()
            
            # Verify shape
            assert decoded.shape == original.shape
            
            # Verify quality - JPEG is lossy so we check PSNR
            mse = np.mean((original.astype(float) - decoded.astype(float)) ** 2)
            if mse > 0:
                psnr = 10 * np.log10(255.0 ** 2 / mse)
                assert psnr >= 25.0, f"PSNR {psnr:.2f} dB is below threshold"
            
        finally:
            if path.exists():
                path.unlink()


class TestJpegComratHandling:
    """Tests for COMRAT handling in JPEG compression."""

    def test_default_comrat_when_not_specified(self):
        """Test that default quality is used when COMRAT not specified.
        
        Validates: Requirement 5.4
        """
        num_rows, num_cols, num_bands = 32, 32, 1
        original = np.full((num_bands, num_rows, num_cols), 128, dtype=np.uint8)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            # No COMRAT set - should use default
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=num_cols,
                block_height=num_rows,
                pixel_type=PixelType.UInt8,
                metadata=metadata,
            )
            provider.set_full_image(original)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Default COMRAT",
                description="JPEG with default quality",
                roles=["data"],
            )
            writer.close()
            
            # Verify file was created and can be read
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = asset.get_block(0, 0, 0)
            reader.close()
            
            assert decoded.shape == original.shape
            
        finally:
            if path.exists():
                path.unlink()

    def test_comrat_quality_affects_compression(self):
        """Test that different COMRAT values affect compression.
        
        Validates: Requirements 5.2, 5.3
        """
        num_rows, num_cols, num_bands = 64, 64, 1
        original = np.random.randint(0, 256, (num_bands, num_rows, num_cols), dtype=np.uint8)
        
        file_sizes = {}
        
        for quality in ["25.0", "75.0", "95.0"]:
            with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
                path = Path(f.name)
            
            try:
                metadata = BufferedMetadataProvider()
                metadata.set("IC", "C3")
                metadata.set("COMRAT", quality)
                
                provider = BufferedImageAssetProvider.create(
                    key="image_segment_0",
                    num_columns=num_cols,
                    num_rows=num_rows,
                    num_bands=num_bands,
                    block_width=num_cols,
                    block_height=num_rows,
                    pixel_type=PixelType.UInt8,
                    metadata=metadata,
                )
                provider.set_full_image(original)
                
                writer = IO.open([str(path)], "w", "nitf")
                writer.add_asset(
                    key="image_segment_0",
                    provider=provider,
                    title=f"Quality {quality}",
                    description="COMRAT test",
                    roles=["data"],
                )
                writer.close()
                
                file_sizes[quality] = path.stat().st_size
                
            finally:
                if path.exists():
                    path.unlink()
        
        # Higher quality should generally produce larger files
        # (though this isn't strictly guaranteed for all images)
        assert file_sizes["95.0"] >= file_sizes["25.0"], (
            f"Expected quality 95 file ({file_sizes['95.0']}) >= quality 25 file ({file_sizes['25.0']})"
        )
