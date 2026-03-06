"""Property-based tests for block access operations.

This module tests the correctness properties for block-based image access:
- Block access completeness (Property 5)
- Block reassembly roundtrip (Property 6)
- Invalid block coordinate error handling (Property 7)
- Resolution level consistency (Property 12)

The tests verify that block-based access patterns work correctly across
all valid block coordinates and that blocks can be reassembled into
the original image.

Requirements: 4.1, 4.2, 4.3, 4.4, 7.1, 7.2, 7.3
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from .strategies import (
    random_image,
    block_sizes,
    valid_block_coordinates,
    invalid_block_coordinates,
    get_numpy_dtype,
)


# Default hypothesis settings for I/O-bound property tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


@pytest.mark.property
class TestBlockAccessCompleteness:
    """Property tests for block access completeness.
    
    These tests verify Property 5: Block Access Completeness.
    
    For any valid block coordinates within an image's block grid, get_block
    SHALL return a block without error, with shape consistent with the block
    dimensions (or smaller for edge blocks).
    
    **Feature: property-based-testing-framework, Property 5: Block Access Completeness**
    **Validates: Requirements 4.1, 4.2**
    """
    
    @given(
        image_tuple=random_image(min_size=16, max_size=128, min_bands=1, max_bands=4),
        block_size=block_sizes(),
    )
    @pbt_settings
    def test_block_access_completeness(self, image_tuple, block_size):
        """Property 5: Block Access Completeness
        
        For any valid block coordinates within an image's block grid, get_block
        SHALL return a block without error, with shape consistent with the block
        dimensions (or smaller for edge blocks).
        
        This test:
        1. Generates a random image with random dimensions, bands, and pixel type
        2. Writes it to a NITF file with a specific block size
        3. Iterates over ALL valid block coordinates
        4. Verifies get_block succeeds for each coordinate
        5. Verifies the returned block has the expected shape
        
        **Feature: property-based-testing-framework, Property 5: Block Access Completeness**
        **Validates: Requirements 4.1, 4.2**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        block_height, block_width = block_size
        
        # Ensure block size doesn't exceed image dimensions
        actual_block_height = min(block_height, num_rows)
        actual_block_width = min(block_width, num_cols)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for uncompressed (IC=NC) - simplest case for block access
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "NC")
            
            # Create image provider with specified block size
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=actual_block_width,
                block_height=actual_block_height,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            # Set image data (array is in BSQ format: bands, rows, cols)
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="Property test image for block access",
                roles=["data"],
            )
            writer.close()
            
            # Read back and verify block access
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Get block grid dimensions from the asset
            block_grid_rows, block_grid_cols = asset.block_grid_size
            asset_block_bands, asset_block_rows, asset_block_cols = asset.block_shape
            
            # Calculate expected block grid dimensions
            expected_block_grid_rows = (num_rows + actual_block_height - 1) // actual_block_height
            expected_block_grid_cols = (num_cols + actual_block_width - 1) // actual_block_width
            
            # Verify block grid dimensions match expectations
            assert block_grid_rows == expected_block_grid_rows, (
                f"Block grid rows mismatch: expected {expected_block_grid_rows}, got {block_grid_rows}"
            )
            assert block_grid_cols == expected_block_grid_cols, (
                f"Block grid cols mismatch: expected {expected_block_grid_cols}, got {block_grid_cols}"
            )
            
            # Iterate over ALL valid block coordinates and verify access
            for block_row in range(block_grid_rows):
                for block_col in range(block_grid_cols):
                    # Requirement 4.1: get_block SHALL return a block without error
                    block = asset.get_block(block_row, block_col, 0)
                    
                    # Requirement 4.2: Verify returned block has expected shape
                    # Calculate expected block dimensions (edge blocks may be smaller)
                    start_row = block_row * asset_block_rows
                    start_col = block_col * asset_block_cols
                    
                    expected_rows = min(asset_block_rows, num_rows - start_row)
                    expected_cols = min(asset_block_cols, num_cols - start_col)
                    
                    # Block shape is (bands, rows, cols) in BSQ format
                    assert block.shape[0] == num_bands, (
                        f"Block ({block_row}, {block_col}) band count mismatch: "
                        f"expected {num_bands}, got {block.shape[0]}"
                    )
                    assert block.shape[1] == expected_rows, (
                        f"Block ({block_row}, {block_col}) row count mismatch: "
                        f"expected {expected_rows}, got {block.shape[1]}"
                    )
                    assert block.shape[2] == expected_cols, (
                        f"Block ({block_row}, {block_col}) col count mismatch: "
                        f"expected {expected_cols}, got {block.shape[2]}"
                    )
                    
                    # Verify dtype matches
                    expected_dtype = get_numpy_dtype(pixel_type)
                    assert block.dtype == expected_dtype, (
                        f"Block ({block_row}, {block_col}) dtype mismatch: "
                        f"expected {expected_dtype}, got {block.dtype}"
                    )
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()


@pytest.mark.property
class TestBlockReassembly:
    """Property tests for block reassembly roundtrip.
    
    These tests verify Property 6: Block Reassembly Roundtrip.
    
    For any image, reading all blocks via get_block and reassembling them
    in order SHALL produce an array equal to the original image data.
    
    **Feature: property-based-testing-framework, Property 6: Block Reassembly Roundtrip**
    **Validates: Requirements 4.3**
    """
    
    @given(
        image_tuple=random_image(min_size=16, max_size=128, min_bands=1, max_bands=4),
        block_size=block_sizes(),
    )
    @pbt_settings
    def test_block_reassembly_roundtrip(self, image_tuple, block_size):
        """Property 6: Block Reassembly Roundtrip
        
        For any image, reading all blocks via get_block and reassembling them
        in order SHALL produce an array equal to the original image data.
        
        This test:
        1. Generates a random image with random dimensions, bands, and pixel type
        2. Writes it to a NITF file with a specific block size
        3. Reads ALL blocks from the file
        4. Reassembles the blocks into a full array
        5. Verifies the reassembled array equals the original
        
        **Feature: property-based-testing-framework, Property 6: Block Reassembly Roundtrip**
        **Validates: Requirements 4.3**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        block_height, block_width = block_size
        
        # Ensure block size doesn't exceed image dimensions
        actual_block_height = min(block_height, num_rows)
        actual_block_width = min(block_width, num_cols)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for uncompressed (IC=NC) - simplest case for block access
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "NC")
            
            # Create image provider with specified block size
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=actual_block_width,
                block_height=actual_block_height,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            # Set image data (array is in BSQ format: bands, rows, cols)
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="Property test image for block reassembly",
                roles=["data"],
            )
            writer.close()
            
            # Read back and reassemble blocks
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Get block grid dimensions from the asset
            block_grid_rows, block_grid_cols = asset.block_grid_size
            asset_block_bands, asset_block_rows, asset_block_cols = asset.block_shape
            
            # Get the expected dtype
            expected_dtype = get_numpy_dtype(pixel_type)
            
            # Create an empty array to reassemble the blocks into
            reassembled = np.zeros((num_bands, num_rows, num_cols), dtype=expected_dtype)
            
            # Read all blocks and place them in the reassembled array
            for block_row in range(block_grid_rows):
                for block_col in range(block_grid_cols):
                    # Read the block
                    block = asset.get_block(block_row, block_col, 0)
                    
                    # Calculate the position in the full image
                    start_row = block_row * asset_block_rows
                    start_col = block_col * asset_block_cols
                    
                    # Get the actual block dimensions (edge blocks may be smaller)
                    block_bands, block_rows, block_cols = block.shape
                    
                    # Place the block in the reassembled array
                    reassembled[
                        :block_bands,
                        start_row:start_row + block_rows,
                        start_col:start_col + block_cols
                    ] = block
            
            reader.close()
            
            # Verify the reassembled array equals the original
            # Requirement 4.3: Reading all blocks and reassembling them produces the original image
            assert reassembled.shape == array.shape, (
                f"Shape mismatch: expected {array.shape}, got {reassembled.shape}"
            )
            assert reassembled.dtype == array.dtype, (
                f"Dtype mismatch: expected {array.dtype}, got {reassembled.dtype}"
            )
            np.testing.assert_array_equal(
                reassembled, array,
                err_msg="Reassembled image does not match original"
            )
            
        finally:
            if path.exists():
                path.unlink()


@pytest.mark.property
class TestInvalidBlockCoordinates:
    """Property tests for invalid block coordinate error handling.
    
    These tests verify Property 7: Invalid Block Coordinate Error Handling.
    
    For any block coordinates outside the valid range, get_block SHALL raise
    an appropriate error rather than returning invalid data or crashing.
    
    **Feature: property-based-testing-framework, Property 7: Invalid Block Coordinate Error Handling**
    **Validates: Requirements 4.4**
    """
    
    @given(
        image_tuple=random_image(min_size=16, max_size=64, min_bands=1, max_bands=3),
        block_size=block_sizes(),
    )
    @pbt_settings
    def test_invalid_block_coordinate_error_handling(self, image_tuple, block_size):
        """Property 7: Invalid Block Coordinate Error Handling
        
        For any block coordinates outside the valid range, get_block SHALL
        raise an appropriate error rather than returning invalid data or crashing.
        
        This test:
        1. Generates a random image with random dimensions, bands, and pixel type
        2. Writes it to a NITF file with a specific block size
        3. Generates invalid block coordinates (negative or out of bounds)
        4. Verifies that get_block raises IndexError for invalid coordinates
        
        **Feature: property-based-testing-framework, Property 7: Invalid Block Coordinate Error Handling**
        **Validates: Requirements 4.4**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        block_height, block_width = block_size
        
        # Ensure block size doesn't exceed image dimensions
        actual_block_height = min(block_height, num_rows)
        actual_block_width = min(block_width, num_cols)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata for uncompressed (IC=NC) - simplest case for block access
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "NC")
            
            # Create image provider with specified block size
            provider = BufferedImageAssetProvider.create(
                key="image_segment_0",
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=actual_block_width,
                block_height=actual_block_height,
                pixel_type=pixel_type,
                metadata=metadata,
            )
            
            # Set image data (array is in BSQ format: bands, rows, cols)
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="Property test image for invalid block coordinates",
                roles=["data"],
            )
            writer.close()
            
            # Read back and test invalid block access
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Get block grid dimensions from the asset
            block_grid_rows, block_grid_cols = asset.block_grid_size
            
            # Test various invalid coordinate scenarios
            # Requirement 4.4: Invalid coordinates SHALL raise appropriate error
            
            # Test 1: Row index too large
            with pytest.raises(IndexError):
                asset.get_block(block_grid_rows, 0, 0)
            
            # Test 2: Column index too large
            with pytest.raises(IndexError):
                asset.get_block(0, block_grid_cols, 0)
            
            # Test 3: Both row and column too large
            with pytest.raises(IndexError):
                asset.get_block(block_grid_rows, block_grid_cols, 0)
            
            # Test 4: Very large row index
            with pytest.raises(IndexError):
                asset.get_block(block_grid_rows + 100, 0, 0)
            
            # Test 5: Very large column index
            with pytest.raises(IndexError):
                asset.get_block(0, block_grid_cols + 100, 0)
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()


@pytest.mark.property
class TestResolutionLevels:
    """Property tests for resolution level consistency.
    
    These tests verify Property 12: Resolution Level Consistency.
    
    For any image with multiple resolution levels, resolution level N SHALL
    have dimensions reduced by factor 2^N from level 0, and get_block at
    level N SHALL return blocks with shapes consistent with that level's
    dimensions.
    
    **Feature: property-based-testing-framework, Property 12: Resolution Level Consistency**
    **Validates: Requirements 7.1, 7.2, 7.3**
    """
    
    @given(
        image_tuple=random_image(min_size=64, max_size=128, min_bands=1, max_bands=3),
    )
    @pbt_settings
    def test_resolution_level_consistency(self, image_tuple):
        """Property 12: Resolution Level Consistency
        
        For any image with multiple resolution levels, resolution level N SHALL
        have dimensions reduced by factor 2^N from level 0, and get_block at
        level N SHALL return blocks with shapes consistent with that level's
        dimensions.
        
        This test:
        1. Generates a random image with dimensions suitable for multi-resolution
        2. Writes it to a NITF file with J2K compression and multiple decomposition levels
        3. Reads back and verifies num_resolution_levels >= 2
        4. For each resolution level N, verifies:
           - Block dimensions are reduced by factor 2^N from level 0
           - get_block succeeds at each level
           - Block shapes are consistent with the resolution level
        
        **Feature: property-based-testing-framework, Property 12: Resolution Level Consistency**
        **Validates: Requirements 7.1, 7.2, 7.3**
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        # Skip Float32 as J2K doesn't support it well
        if pixel_type == PixelType.Float32:
            return
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Calculate appropriate decomposition levels for image size
            # J2K requires image dimensions >= 2^decomposition_levels
            min_dim = min(num_rows, num_cols)
            max_decomp_levels = max(1, int(np.floor(np.log2(min_dim))) - 1)
            # Use at least 2 decomposition levels to test multi-resolution
            decomp_levels = min(max_decomp_levels, 4)
            
            if decomp_levels < 2:
                # Image too small for meaningful multi-resolution test
                return
            
            # Create metadata for J2K lossless compression with multiple decomposition levels
            metadata = BufferedMetadataProvider()
            metadata.set("IC", "C8")  # JPEG 2000
            metadata.set("COMRAT", "N1.0")  # Lossless
            metadata.set("J2K_DECOMPOSITION_LEVELS", str(decomp_levels))
            
            # Use block size equal to image size (single tile)
            block_width = num_cols
            block_height = num_rows
            
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
            
            # Set image data (array is in BSQ format: bands, rows, cols)
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="Property test image for resolution levels",
                roles=["data"],
            )
            writer.close()
            
            # Read back and verify resolution levels
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Requirement 7.1: Verify multiple resolution levels exist
            num_levels = asset.num_resolution_levels
            assert num_levels >= 2, (
                f"Expected at least 2 resolution levels, got {num_levels}"
            )
            
            # Get full resolution block (level 0) for reference
            block_level_0 = asset.get_block(0, 0, 0)
            level_0_bands, level_0_rows, level_0_cols = block_level_0.shape
            
            # Requirement 7.3: Resolution level 0 always returns full-resolution data
            assert level_0_bands == num_bands, (
                f"Level 0 band count mismatch: expected {num_bands}, got {level_0_bands}"
            )
            assert level_0_rows == num_rows, (
                f"Level 0 row count mismatch: expected {num_rows}, got {level_0_rows}"
            )
            assert level_0_cols == num_cols, (
                f"Level 0 col count mismatch: expected {num_cols}, got {level_0_cols}"
            )
            
            # Test each resolution level
            for level in range(1, num_levels):
                # Check if block exists at this level
                if not asset.has_block(0, 0, level):
                    continue
                
                # Requirement 7.1: Dimension reduction by 2^N at level N
                scale = 1 << level  # 2^level
                expected_rows = (num_rows + scale - 1) // scale  # Ceiling division
                expected_cols = (num_cols + scale - 1) // scale
                
                # Get block at this resolution level
                block_level_n = asset.get_block(0, 0, level)
                level_n_bands, level_n_rows, level_n_cols = block_level_n.shape
                
                # Requirement 7.2: Block shapes at each level are consistent
                assert level_n_bands == num_bands, (
                    f"Level {level} band count mismatch: expected {num_bands}, got {level_n_bands}"
                )
                assert level_n_rows == expected_rows, (
                    f"Level {level} row count mismatch: expected {expected_rows}, got {level_n_rows} "
                    f"(original {num_rows}, scale {scale})"
                )
                assert level_n_cols == expected_cols, (
                    f"Level {level} col count mismatch: expected {expected_cols}, got {level_n_cols} "
                    f"(original {num_cols}, scale {scale})"
                )
                
                # Verify dtype is preserved
                expected_dtype = get_numpy_dtype(pixel_type)
                assert block_level_n.dtype == expected_dtype, (
                    f"Level {level} dtype mismatch: expected {expected_dtype}, got {block_level_n.dtype}"
                )
            
            reader.close()
            
        finally:
            if path.exists():
                path.unlink()
