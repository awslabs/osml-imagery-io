"""Property-based tests for JPEG DCT compression roundtrip operations.

This module tests the correctness properties for JPEG DCT compression:
- Property 1: JPEG DCT Lossy Roundtrip Quality
- Property 2: Masked JPEG Roundtrip
- Property 3: Downsampled JPEG (I1) Roundtrip

The tests verify that JPEG encoding then decoding produces acceptable
quality results with preserved shape and pixel type.

Requirements: 1.1-1.6, 2.1-2.6, 3.1-3.5, 4.1-4.3, 7.1-7.4
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase, assume

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from .strategies import (
    jpeg_image_for_compression,
    jpeg_i1_image,
    masked_jpeg_image,
    jpeg_comrat,
    get_numpy_dtype,
    band_counts,
)
from .quality import (
    calculate_psnr,
    calculate_ssim,
    MIN_PSNR_DB,
    MIN_SSIM,
)


# Default hypothesis settings for I/O-bound property tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


def _read_full_image(asset, num_bands: int, num_rows: int, num_cols: int) -> np.ndarray:
    """Read all blocks from an asset and reassemble into full image.
    
    Args:
        asset: ImageAssetProvider to read from
        num_bands: Expected number of bands
        num_rows: Expected number of rows
        num_cols: Expected number of columns
        
    Returns:
        Reassembled image array in BSQ format (bands, rows, cols)
    """
    block_grid_rows, block_grid_cols = asset.block_grid_size
    block_bands, block_rows, block_cols = asset.block_shape
    
    dtype = get_numpy_dtype(asset.pixel_value_type)
    result = np.zeros((num_bands, num_rows, num_cols), dtype=dtype)
    
    for block_row in range(block_grid_rows):
        for block_col in range(block_grid_cols):
            block = asset.get_block(block_row, block_col, 0)
            
            start_row = block_row * block_rows
            start_col = block_col * block_cols
            end_row = min(start_row + block.shape[1], num_rows)
            end_col = min(start_col + block.shape[2], num_cols)
            
            actual_rows = end_row - start_row
            actual_cols = end_col - start_col
            
            result[:, start_row:end_row, start_col:end_col] = block[:, :actual_rows, :actual_cols]
    
    return result


@pytest.mark.property
class TestJpegLossyRoundtrip:
    """Property tests for JPEG DCT lossy roundtrip quality.
    
    These tests verify Property 1: JPEG DCT Lossy Roundtrip Quality.
    
    For any valid image with supported pixel type (UInt8 8-bit) and band
    configuration (mono, RGB, YCbCr, multiband), encoding with IC=C3 then
    decoding SHALL produce an image with:
    - PSNR >= 30 dB
    - SSIM >= 0.95
    - Identical shape (bands, rows, cols)
    - Identical pixel type (dtype)
    
    **Feature: jpeg-dct-compression, Property 1: JPEG DCT Lossy Roundtrip Quality**
    **Validates: Requirements 1.1-1.6, 2.1-2.6, 7.1-7.4**
    """
    
    @given(jpeg_image_for_compression(min_size=32, max_size=64, min_bands=1, max_bands=1))
    @pbt_settings
    def test_jpeg_8bit_mono_roundtrip(self, image_tuple):
        """Property 1: JPEG DCT Lossy Roundtrip - 8-bit Mono
        
        For any 8-bit monochrome image, JPEG encoding then decoding SHALL
        produce an image with PSNR >= 30 dB and SSIM >= 0.95.
        
        **Feature: jpeg-dct-compression, Property 1: JPEG DCT Lossy Roundtrip Quality**
        **Validates: Requirements 1.1, 1.2, 2.1, 2.2, 7.1, 7.2, 7.3, 7.4**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for JPEG DCT (IC=C3)
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            metadata.set("COMRAT", "75.0")  # Quality 75
            
            # Create image provider
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, 64),
                block_height=min(num_rows, 64),
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="JPEG DCT property test - 8-bit mono",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = _read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()
            
            # Verify shape preservation
            assert decoded.shape == array.shape, (
                f"Shape mismatch: expected {array.shape}, got {decoded.shape}"
            )
            
            # Verify dtype preservation
            assert decoded.dtype == array.dtype, (
                f"Dtype mismatch: expected {array.dtype}, got {decoded.dtype}"
            )
            
            # Calculate quality metrics
            psnr = calculate_psnr(array, decoded, use_actual_range=True)
            ssim = calculate_ssim(array, decoded)
            
            # Verify quality bounds
            assert psnr >= MIN_PSNR_DB, (
                f"PSNR {psnr:.2f} dB is below minimum threshold {MIN_PSNR_DB} dB"
            )
            assert ssim >= MIN_SSIM, (
                f"SSIM {ssim:.4f} is below minimum threshold {MIN_SSIM}"
            )
            
        finally:
            if path.exists():
                path.unlink()
    
    @given(jpeg_image_for_compression(min_size=32, max_size=64, min_bands=3, max_bands=3))
    @pbt_settings
    def test_jpeg_8bit_rgb_roundtrip(self, image_tuple):
        """Property 1: JPEG DCT Lossy Roundtrip - 8-bit RGB
        
        For any 8-bit RGB image, JPEG encoding then decoding SHALL
        produce an image with PSNR >= 30 dB and SSIM >= 0.95.
        
        **Feature: jpeg-dct-compression, Property 1: JPEG DCT Lossy Roundtrip Quality**
        **Validates: Requirements 1.4, 2.4, 7.1, 7.2, 7.3, 7.4**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            metadata.set("COMRAT", "75.0")
            metadata.set("IMODE", "P")  # Pixel interleaved for RGB
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, 64),
                block_height=min(num_rows, 64),
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            provider.set_full_image(array)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="JPEG DCT property test - 8-bit RGB",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = _read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()
            
            assert decoded.shape == array.shape
            assert decoded.dtype == array.dtype
            
            psnr = calculate_psnr(array, decoded, use_actual_range=True)
            ssim = calculate_ssim(array, decoded)
            
            assert psnr >= MIN_PSNR_DB, f"PSNR {psnr:.2f} dB below threshold"
            assert ssim >= MIN_SSIM, f"SSIM {ssim:.4f} below threshold"
            
        finally:
            if path.exists():
                path.unlink()
    
    @given(jpeg_image_for_compression(min_size=32, max_size=64, min_bands=2, max_bands=4))
    @pbt_settings
    def test_jpeg_8bit_multiband_roundtrip(self, image_tuple):
        """Property 1: JPEG DCT Lossy Roundtrip - 8-bit Multiband
        
        For any 8-bit multiband image (2-4 bands), JPEG encoding then decoding
        SHALL produce an image with PSNR >= 30 dB and SSIM >= 0.95.
        
        **Feature: jpeg-dct-compression, Property 1: JPEG DCT Lossy Roundtrip Quality**
        **Validates: Requirements 1.6, 2.6, 7.1, 7.2, 7.3, 7.4**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        # Skip 3-band images (tested separately as RGB)
        assume(num_bands != 3)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C3")
            metadata.set("COMRAT", "75.0")
            metadata.set("IMODE", "B")  # Block interleaved for multiband
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, 64),
                block_height=min(num_rows, 64),
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            provider.set_full_image(array)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="JPEG DCT property test - 8-bit multiband",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = _read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()
            
            assert decoded.shape == array.shape
            assert decoded.dtype == array.dtype
            
            psnr = calculate_psnr(array, decoded, use_actual_range=True)
            ssim = calculate_ssim(array, decoded)
            
            assert psnr >= MIN_PSNR_DB, f"PSNR {psnr:.2f} dB below threshold"
            assert ssim >= MIN_SSIM, f"SSIM {ssim:.4f} below threshold"
            
        finally:
            if path.exists():
                path.unlink()



@pytest.mark.property
class TestMaskedJpegRoundtrip:
    """Property tests for masked JPEG (IC=M3) roundtrip operations.
    
    These tests verify Property 2: Masked JPEG Roundtrip.
    
    For any masked image with IC=M3 and any mask pattern (checkerboard,
    border, random), writing then reading SHALL:
    - Preserve the exact mask pattern (has_block() returns same values)
    - Return valid block data for all provided blocks
    - Return false from has_block() for all omitted blocks
    
    **Feature: jpeg-dct-compression, Property 2: Masked JPEG Roundtrip**
    **Validates: Requirements 3.1-3.5**
    """
    
    @given(masked_jpeg_image(min_size=64, max_size=128, min_bands=1, max_bands=1))
    @pbt_settings
    def test_masked_jpeg_roundtrip(self, image_tuple):
        """Property 2: Masked JPEG Roundtrip
        
        For any masked JPEG image (IC=M3), writing then reading SHALL
        preserve the mask pattern and return valid block data for all
        provided blocks.
        
        Note: This test focuses on monochrome images as the most common
        use case for masked JPEG. RGB masked JPEG requires IMODE=P which
        has different encoding behavior.
        
        **Feature: jpeg-dct-compression, Property 2: Masked JPEG Roundtrip**
        **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**
        """
        (array, pixel_type, num_bands, num_rows, num_cols, 
         block_height, block_width, provided_blocks) = image_tuple
        
        # Skip if no blocks are provided
        assume(len(provided_blocks) > 0)
        
        # Calculate block grid dimensions
        num_block_rows = (num_rows + block_height - 1) // block_height
        num_block_cols = (num_cols + block_width - 1) // block_width
        
        # Calculate expected masked blocks
        all_blocks = {(r, c) for r in range(num_block_rows) for c in range(num_block_cols)}
        expected_masked_blocks = all_blocks - provided_blocks
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for masked JPEG (IC=M3)
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "M3")
            metadata.set("COMRAT", "85.0")  # Higher quality for reliable PSNR
            
            # Create image provider
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=block_width,
                block_height=block_height,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            # Set only the provided blocks (sparse data)
            for block_row, block_col in provided_blocks:
                start_row = block_row * block_height
                start_col = block_col * block_width
                end_row = min(start_row + block_height, num_rows)
                end_col = min(start_col + block_width, num_cols)
                
                block = array[:, start_row:end_row, start_col:end_col].copy()
                provider.set_block(block_row, block_col, block)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Masked JPEG Image",
                description="JPEG DCT property test - masked (M3)",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Verify mask pattern preservation
            actual_masked_blocks = set()
            for block_row in range(num_block_rows):
                for block_col in range(num_block_cols):
                    if not asset.has_block(block_row, block_col, 0):
                        actual_masked_blocks.add((block_row, block_col))
            
            assert actual_masked_blocks == expected_masked_blocks, (
                f"Masked block pattern mismatch.\n"
                f"Expected masked: {sorted(expected_masked_blocks)}\n"
                f"Actual masked: {sorted(actual_masked_blocks)}"
            )
            
            # Verify provided blocks have valid data with acceptable quality
            for block_row, block_col in provided_blocks:
                assert asset.has_block(block_row, block_col, 0), (
                    f"has_block({block_row}, {block_col}) should return True"
                )
                
                decoded_block = asset.get_block(block_row, block_col, 0)
                
                start_row = block_row * block_height
                start_col = block_col * block_width
                end_row = min(start_row + block_height, num_rows)
                end_col = min(start_col + block_width, num_cols)
                original_block = array[:, start_row:end_row, start_col:end_col]
                
                # Verify shape matches
                assert decoded_block.shape == original_block.shape, (
                    f"Block ({block_row}, {block_col}) shape mismatch"
                )
                
                # For lossy JPEG, verify quality bounds instead of exact match
                psnr = calculate_psnr(original_block, decoded_block, use_actual_range=True)
                assert psnr >= MIN_PSNR_DB, (
                    f"Block ({block_row}, {block_col}) PSNR {psnr:.2f} dB below threshold"
                )
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()
    
    @given(masked_jpeg_image(min_size=64, max_size=128, min_bands=1, max_bands=1))
    @pbt_settings
    def test_masked_jpeg_mono_roundtrip(self, image_tuple):
        """Property 2: Masked JPEG Roundtrip - Monochrome
        
        For any masked monochrome JPEG image (IC=M3), writing then reading
        SHALL preserve the mask pattern.
        
        **Feature: jpeg-dct-compression, Property 2: Masked JPEG Roundtrip**
        **Validates: Requirements 3.1, 3.2, 3.3**
        """
        (array, pixel_type, num_bands, num_rows, num_cols, 
         block_height, block_width, provided_blocks) = image_tuple
        
        assume(len(provided_blocks) > 0)
        
        num_block_rows = (num_rows + block_height - 1) // block_height
        num_block_cols = (num_cols + block_width - 1) // block_width
        all_blocks = {(r, c) for r in range(num_block_rows) for c in range(num_block_cols)}
        expected_masked_blocks = all_blocks - provided_blocks
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "M3")
            metadata.set("COMRAT", "75.0")
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=block_width,
                block_height=block_height,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            for block_row, block_col in provided_blocks:
                start_row = block_row * block_height
                start_col = block_col * block_width
                end_row = min(start_row + block_height, num_rows)
                end_col = min(start_col + block_width, num_cols)
                block = array[:, start_row:end_row, start_col:end_col].copy()
                provider.set_block(block_row, block_col, block)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Masked JPEG Image",
                description="JPEG DCT property test - masked mono",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            actual_masked_blocks = set()
            for block_row in range(num_block_rows):
                for block_col in range(num_block_cols):
                    if not asset.has_block(block_row, block_col, 0):
                        actual_masked_blocks.add((block_row, block_col))
            
            assert actual_masked_blocks == expected_masked_blocks
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()



@pytest.mark.property
class TestDownsampledJpegRoundtrip:
    """Property tests for downsampled JPEG (IC=I1) roundtrip operations.
    
    These tests verify Property 3: Downsampled JPEG (I1) Roundtrip.
    
    For any valid image with dimensions ≤2048×2048, encoding with IC=I1
    then decoding SHALL produce an image with acceptable quality
    (PSNR >= 30 dB, SSIM >= 0.95) and preserved dimensions.
    
    **Feature: jpeg-dct-compression, Property 3: Downsampled JPEG (I1) Roundtrip**
    **Validates: Requirements 4.1-4.3**
    """
    
    @given(jpeg_i1_image(min_size=32, max_size=256))
    @pbt_settings
    def test_i1_jpeg_roundtrip(self, image_tuple):
        """Property 3: Downsampled JPEG (I1) Roundtrip
        
        For any valid image with dimensions ≤2048×2048, encoding with IC=I1
        then decoding SHALL produce an image with PSNR >= 30 dB and SSIM >= 0.95.
        
        **Feature: jpeg-dct-compression, Property 3: Downsampled JPEG (I1) Roundtrip**
        **Validates: Requirements 4.1, 4.2, 4.3**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for downsampled JPEG (IC=I1)
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "I1")
            metadata.set("COMRAT", "75.0")
            
            # I1 is encoded as a single block
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=num_cols,  # Single block
                block_height=num_rows,  # Single block
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test I1 Image",
                description="JPEG DCT property test - downsampled (I1)",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = _read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()
            
            # Verify dimension preservation
            assert decoded.shape == array.shape, (
                f"Shape mismatch: expected {array.shape}, got {decoded.shape}"
            )
            
            # Verify dtype preservation
            assert decoded.dtype == array.dtype, (
                f"Dtype mismatch: expected {array.dtype}, got {decoded.dtype}"
            )
            
            # Calculate quality metrics
            psnr = calculate_psnr(array, decoded, use_actual_range=True)
            ssim = calculate_ssim(array, decoded)
            
            # Verify quality bounds
            assert psnr >= MIN_PSNR_DB, (
                f"PSNR {psnr:.2f} dB is below minimum threshold {MIN_PSNR_DB} dB"
            )
            assert ssim >= MIN_SSIM, (
                f"SSIM {ssim:.4f} is below minimum threshold {MIN_SSIM}"
            )
            
        finally:
            if path.exists():
                path.unlink()
    
    @given(jpeg_i1_image(min_size=32, max_size=128))
    @pbt_settings
    def test_i1_jpeg_mono_roundtrip(self, image_tuple):
        """Property 3: Downsampled JPEG (I1) Roundtrip - Monochrome
        
        For any monochrome image with dimensions ≤2048×2048, encoding with
        IC=I1 then decoding SHALL preserve dimensions and quality.
        
        **Feature: jpeg-dct-compression, Property 3: Downsampled JPEG (I1) Roundtrip**
        **Validates: Requirements 4.1, 4.2**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        # Only test monochrome
        assume(num_bands == 1)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "I1")
            metadata.set("COMRAT", "75.0")
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=num_cols,
                block_height=num_rows,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            provider.set_full_image(array)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test I1 Image",
                description="JPEG DCT property test - I1 mono",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = _read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()
            
            assert decoded.shape == array.shape
            assert decoded.dtype == array.dtype
            
            psnr = calculate_psnr(array, decoded, use_actual_range=True)
            ssim = calculate_ssim(array, decoded)
            
            assert psnr >= MIN_PSNR_DB, f"PSNR {psnr:.2f} dB below threshold"
            assert ssim >= MIN_SSIM, f"SSIM {ssim:.4f} below threshold"
            
        finally:
            if path.exists():
                path.unlink()
    
    @given(jpeg_i1_image(min_size=32, max_size=128))
    @pbt_settings
    def test_i1_jpeg_rgb_roundtrip(self, image_tuple):
        """Property 3: Downsampled JPEG (I1) Roundtrip - RGB
        
        For any RGB image with dimensions ≤2048×2048, encoding with
        IC=I1 then decoding SHALL preserve dimensions and quality.
        
        **Feature: jpeg-dct-compression, Property 3: Downsampled JPEG (I1) Roundtrip**
        **Validates: Requirements 4.1, 4.3**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        # Only test RGB
        assume(num_bands == 3)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "I1")
            metadata.set("COMRAT", "75.0")
            metadata.set("IMODE", "P")  # Pixel interleaved for RGB
            
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=num_cols,
                block_height=num_rows,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            provider.set_full_image(array)
            
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test I1 Image",
                description="JPEG DCT property test - I1 RGB",
                roles=["data"],
            )
            writer.close()
            
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            decoded = _read_full_image(asset, num_bands, num_rows, num_cols)
            reader.close()
            
            assert decoded.shape == array.shape
            assert decoded.dtype == array.dtype
            
            psnr = calculate_psnr(array, decoded, use_actual_range=True)
            ssim = calculate_ssim(array, decoded)
            
            assert psnr >= MIN_PSNR_DB, f"PSNR {psnr:.2f} dB below threshold"
            assert ssim >= MIN_SSIM, f"SSIM {ssim:.4f} below threshold"
            
        finally:
            if path.exists():
                path.unlink()
