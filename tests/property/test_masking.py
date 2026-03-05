"""Property-based tests for masked image operations.

This module tests the correctness properties for masked image operations:
- Property 3: Valid Block Decoding in Masked Images
- Property 6: Pad Pixel Value Preservation
- Property 7: Masked Block Pattern Preservation

The tests verify that masked images correctly handle sparse block data,
preserve mask patterns through roundtrip operations, and maintain pad
pixel values.

Requirements: 2.4, 3.1, 3.3, 8.1, 8.2, 8.3
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
    masked_image,
    get_numpy_dtype,
    calculate_safe_j2k_decomposition_levels,
)


# Default hypothesis settings for I/O-bound property tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


@pytest.mark.property
class TestValidBlockDecoding:
    """Property tests for valid block decoding in masked images.
    
    These tests verify Property 3: Valid Block Decoding in Masked Images.
    
    For any masked image and for any block where has_block() returns true,
    get_block() SHALL return block data that matches the original data
    provided during writing.
    
    **Feature: image-masking, Property 3: Valid Block Decoding in Masked Images**
    **Validates: Requirements 2.4, 8.1**
    """
    
    @given(masked_image(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_valid_block_decoding_in_masked_images(self, image_tuple):
        """Property 3: Valid Block Decoding in Masked Images
        
        For any masked image and for any block where has_block() returns true,
        get_block() SHALL return block data that matches the original data
        provided during writing.
        
        **Feature: image-masking, Property 3: Valid Block Decoding in Masked Images**
        **Validates: Requirements 2.4, 8.1**
        """
        (array, pixel_type, num_bands, num_rows, num_cols, 
         block_height, block_width, provided_blocks, ic_value) = image_tuple
        
        # Skip if no blocks are provided (all_masked pattern)
        assume(len(provided_blocks) > 0)
        
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for masked image
            metadata = BufferedMetadataProvider()
            metadata.set("IC", ic_value)
            
            # For M8 (J2K), set lossless compression with safe decomposition levels
            if ic_value == "M8":
                metadata.set("COMRAT", "N1.0")
                decomp_levels = calculate_safe_j2k_decomposition_levels(
                    block_height, block_width, num_rows, num_cols
                )
                metadata.set("J2K_DECOMPOSITION_LEVELS", str(decomp_levels))
            
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
            self._set_sparse_blocks(provider, array, provided_blocks, 
                                   block_height, block_width, num_bands, pixel_type)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Masked Image",
                description="Property test masked image",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Verify each provided block can be decoded and matches original
            for block_row, block_col in provided_blocks:
                # Verify has_block returns true for provided blocks
                assert asset.has_block(block_row, block_col, 0), (
                    f"has_block({block_row}, {block_col}) should return True for provided block"
                )
                
                # Get the decoded block
                decoded_block = asset.get_block(block_row, block_col, 0)
                
                # Get the original block data
                original_block = self._extract_block(
                    array, block_row, block_col, 
                    block_height, block_width, num_rows, num_cols
                )
                
                # Verify shape matches
                assert decoded_block.shape == original_block.shape, (
                    f"Block ({block_row}, {block_col}) shape mismatch: "
                    f"expected {original_block.shape}, got {decoded_block.shape}"
                )
                
                # Verify data matches (for lossless compression)
                np.testing.assert_array_equal(
                    decoded_block, original_block,
                    err_msg=f"Block ({block_row}, {block_col}) data mismatch"
                )
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()
    
    def _set_sparse_blocks(self, provider, array, provided_blocks, 
                          block_height, block_width, num_bands, pixel_type):
        """Set only the provided blocks in the provider."""
        num_rows, num_cols = array.shape[1], array.shape[2]
        
        for block_row, block_col in provided_blocks:
            # Extract block from array
            block = self._extract_block(
                array, block_row, block_col,
                block_height, block_width, num_rows, num_cols
            )
            
            # Pass numpy array directly (not bytes)
            provider.set_block(block_row, block_col, block)
    
    def _extract_block(self, array, block_row, block_col, 
                      block_height, block_width, num_rows, num_cols):
        """Extract a block from the full image array."""
        start_row = block_row * block_height
        start_col = block_col * block_width
        end_row = min(start_row + block_height, num_rows)
        end_col = min(start_col + block_width, num_cols)
        
        return array[:, start_row:end_row, start_col:end_col].copy()


@pytest.mark.property
class TestMaskedBlockPatternPreservation:
    """Property tests for masked block pattern preservation.
    
    These tests verify Property 7: Masked Block Pattern Preservation.
    
    For any masked image, the set of block coordinates where has_block()
    returns false SHALL be identical before writing and after reading.
    
    **Feature: image-masking, Property 7: Masked Block Pattern Preservation**
    **Validates: Requirements 8.2, 8.4**
    """
    
    @given(masked_image(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_masked_block_pattern_preservation(self, image_tuple):
        """Property 7: Masked Block Pattern Preservation
        
        For any masked image, the set of block coordinates where has_block()
        returns false SHALL be identical before writing and after reading.
        
        **Feature: image-masking, Property 7: Masked Block Pattern Preservation**
        **Validates: Requirements 8.2, 8.4**
        """
        (array, pixel_type, num_bands, num_rows, num_cols, 
         block_height, block_width, provided_blocks, ic_value) = image_tuple
        
        
        # Skip if no blocks are provided (all_masked pattern) - can't write empty image
        assume(len(provided_blocks) > 0)
        
        # Calculate block grid dimensions
        num_block_rows = (num_rows + block_height - 1) // block_height
        num_block_cols = (num_cols + block_width - 1) // block_width
        
        # Calculate expected masked blocks (blocks NOT in provided_blocks)
        all_blocks = {(r, c) for r in range(num_block_rows) for c in range(num_block_cols)}
        expected_masked_blocks = all_blocks - provided_blocks
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for masked image
            metadata = BufferedMetadataProvider()
            metadata.set("IC", ic_value)
            
            # For M8 (J2K), set lossless compression with safe decomposition levels
            if ic_value == "M8":
                metadata.set("COMRAT", "N1.0")
                decomp_levels = calculate_safe_j2k_decomposition_levels(
                    block_height, block_width, num_rows, num_cols
                )
                metadata.set("J2K_DECOMPOSITION_LEVELS", str(decomp_levels))
            
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
            self._set_sparse_blocks(provider, array, provided_blocks, 
                                   block_height, block_width, pixel_type)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Masked Image",
                description="Property test masked image",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Collect actual masked blocks from the read asset
            actual_masked_blocks = set()
            for block_row in range(num_block_rows):
                for block_col in range(num_block_cols):
                    if not asset.has_block(block_row, block_col, 0):
                        actual_masked_blocks.add((block_row, block_col))
            
            # Verify masked block pattern is preserved
            assert actual_masked_blocks == expected_masked_blocks, (
                f"Masked block pattern mismatch.\n"
                f"Expected masked: {sorted(expected_masked_blocks)}\n"
                f"Actual masked: {sorted(actual_masked_blocks)}\n"
                f"Missing from actual: {sorted(expected_masked_blocks - actual_masked_blocks)}\n"
                f"Extra in actual: {sorted(actual_masked_blocks - expected_masked_blocks)}"
            )
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()
    
    def _set_sparse_blocks(self, provider, array, provided_blocks, 
                          block_height, block_width, pixel_type):
        """Set only the provided blocks in the provider."""
        num_rows, num_cols = array.shape[1], array.shape[2]
        
        for block_row, block_col in provided_blocks:
            # Extract block from array
            start_row = block_row * block_height
            start_col = block_col * block_width
            end_row = min(start_row + block_height, num_rows)
            end_col = min(start_col + block_width, num_cols)
            
            block = array[:, start_row:end_row, start_col:end_col].copy()
            # Pass numpy array directly (not bytes)
            provider.set_block(block_row, block_col, block)


@pytest.mark.property
class TestPadPixelValuePreservation:
    """Property tests for pad pixel value preservation.
    
    These tests verify Property 6: Pad Pixel Value Preservation.
    
    For any masked image with a defined pad pixel code (TPXCDLNTH > 0),
    writing then reading SHALL preserve the pad pixel value exactly.
    
    Note: This test is currently a placeholder as pad pixel support
    requires additional implementation in the writer. The test verifies
    that the pad_pixel_value property is accessible on read assets.
    
    **Feature: image-masking, Property 6: Pad Pixel Value Preservation**
    **Validates: Requirements 3.1, 3.3, 8.3**
    """
    
    @given(masked_image(min_size=32, max_size=64, min_bands=1, max_bands=3))
    @pbt_settings
    def test_pad_pixel_value_accessible(self, image_tuple):
        """Property 6: Pad Pixel Value Preservation (Basic Access Test)
        
        For any masked image, the pad_pixel_value property SHALL be accessible
        on the read asset without error.
        
        Note: Full pad pixel value preservation testing requires additional
        writer support for setting TPXCD values.
        
        **Feature: image-masking, Property 6: Pad Pixel Value Preservation**
        **Validates: Requirements 3.1, 3.3, 8.3**
        """
        (array, pixel_type, num_bands, num_rows, num_cols, 
         block_height, block_width, provided_blocks, ic_value) = image_tuple
        
        # Skip if no blocks are provided (all_masked pattern)
        assume(len(provided_blocks) > 0)
        
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for masked image
            metadata = BufferedMetadataProvider()
            metadata.set("IC", ic_value)
            
            # For M8 (J2K), set lossless compression with safe decomposition levels
            if ic_value == "M8":
                metadata.set("COMRAT", "N1.0")
                decomp_levels = calculate_safe_j2k_decomposition_levels(
                    block_height, block_width, num_rows, num_cols
                )
                metadata.set("J2K_DECOMPOSITION_LEVELS", str(decomp_levels))
            
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
            self._set_sparse_blocks(provider, array, provided_blocks, 
                                   block_height, block_width, pixel_type)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Masked Image",
                description="Property test masked image",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Verify pad_pixel_value is accessible (should not raise)
            # The value may be 0.0 if no pad pixel code was set
            pad_value = asset.pad_pixel_value
            
            # Verify it's a numeric value
            assert isinstance(pad_value, (int, float)), (
                f"pad_pixel_value should be numeric, got {type(pad_value)}"
            )
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()
    
    def _set_sparse_blocks(self, provider, array, provided_blocks, 
                          block_height, block_width, pixel_type):
        """Set only the provided blocks in the provider."""
        num_rows, num_cols = array.shape[1], array.shape[2]
        
        for block_row, block_col in provided_blocks:
            # Extract block from array
            start_row = block_row * block_height
            start_col = block_col * block_width
            end_row = min(start_row + block_height, num_rows)
            end_col = min(start_col + block_width, num_cols)
            
            block = array[:, start_row:end_row, start_col:end_col].copy()
            # Pass numpy array directly (not bytes)
            provider.set_block(block_row, block_col, block)
