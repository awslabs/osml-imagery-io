"""Property-based tests for API Design Alignment spec.

These tests validate the correctness properties defined in the design document
for the API alignment changes.
"""

import os
import tempfile

import numpy as np
import pytest
from hypothesis import given, settings, strategies as st

from aws.osml.io import (
    IO,
    AssetProvider,
    AssetType,
    BufferedImageAssetProvider,
    PixelType,
)


class TestAddAssetTypeHierarchy:
    """Property tests for add_asset accepting all AssetProvider subtypes.
    
    **Property 3: add_asset accepts all AssetProvider subtypes**
    **Validates: Requirements 3.2**
    """

    @given(
        key=st.text(min_size=1, max_size=20, alphabet=st.characters(whitelist_categories=('L', 'N'), min_codepoint=ord('a'), max_codepoint=ord('z'))),
        title=st.text(min_size=1, max_size=50),
        description=st.text(max_size=100),
        asset_type=st.sampled_from([AssetType.Image, AssetType.Text, AssetType.Data]),
    )
    @settings(max_examples=100, deadline=None)
    def test_add_asset_accepts_bytes_asset_provider(self, key, title, description, asset_type):
        """For any AssetProvider created via from_bytes, add_asset SHALL succeed.
        
        **Validates: Requirements 3.2**
        """
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = f.name
        
        try:
            # Create an AssetProvider using from_bytes (BytesAssetProvider)
            data = bytes([i % 256 for i in range(100)])
            provider = AssetProvider.from_bytes(
                key=key,
                data=data,
                asset_type=asset_type,
                title=title,
            )
            
            writer = IO.open([path], "w", "nitf")
            
            # This should succeed without error
            writer.add_asset(
                key=key,
                provider=provider,
                title=title,
                description=description,
                roles=["data"],
            )
            
            writer.close()
            
            # Verify file was created
            assert os.path.exists(path)
            
        finally:
            if os.path.exists(path):
                os.unlink(path)

    @given(
        key=st.text(min_size=1, max_size=20, alphabet=st.characters(whitelist_categories=('L', 'N'), min_codepoint=ord('a'), max_codepoint=ord('z'))),
        title=st.text(min_size=1, max_size=50),
        description=st.text(max_size=100),
        num_cols=st.integers(min_value=16, max_value=128),
        num_rows=st.integers(min_value=16, max_value=128),
        num_bands=st.integers(min_value=1, max_value=4),
    )
    @settings(max_examples=100, deadline=None)
    def test_add_asset_accepts_buffered_image_asset_provider(self, key, title, description, num_cols, num_rows, num_bands):
        """For any BufferedImageAssetProvider, add_asset SHALL succeed.
        
        **Validates: Requirements 3.2**
        """
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = f.name
        
        try:
            # Create a BufferedImageAssetProvider
            provider = BufferedImageAssetProvider.create(
                key=key,
                num_columns=num_cols,
                num_rows=num_rows,
                num_bands=num_bands,
                block_width=min(num_cols, 64),
                block_height=min(num_rows, 64),
                pixel_type=PixelType.UInt8,
            )
            
            # Set image data
            image_data = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
            provider.set_full_image(image_data)
            
            writer = IO.open([path], "w", "nitf")
            
            # This should succeed without error - using add_asset, not add_image_asset
            writer.add_asset(
                key=key,
                provider=provider,
                title=title,
                description=description,
                roles=["data"],
            )
            
            writer.close()
            
            # Verify file was created
            assert os.path.exists(path)
            
        finally:
            if os.path.exists(path):
                os.unlink(path)
