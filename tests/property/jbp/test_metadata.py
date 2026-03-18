"""Property-based tests for metadata preservation.

The tests verify that user-settable NITF fields survive encode/decode cycles.
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, assume
from hypothesis import strategies as st

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from ..conftest import pbt_settings
from ..strategies import (
    random_image,
    get_numpy_dtype,
)


# BCS-A alphabet: uppercase letters, digits, and space
BCS_A_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 "

# ECS-A alphabet: printable ASCII (alphanumeric + common punctuation)
ECS_A_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .,;:!?-_"


# User-settable NITF image subheader fields that should be preserved during roundtrip
# Reference: JBP specification Section 5.13 (Image Subheader)
# 
# The writer now checks metadata for these fields before using defaults:
# - IID1: defaults to asset.key if not provided
# - IID2: defaults to asset.title if not provided
# - TGTID: defaults to spaces if not provided
# - ISORCE: defaults to spaces if not provided
# - IDATIM: defaults to spaces if not provided
NITF_IMAGE_USER_FIELDS = {
    "IID1": st.text(alphabet=BCS_A_ALPHABET, min_size=1, max_size=10),     # Image Identifier 1
    "IID2": st.text(alphabet=ECS_A_ALPHABET, min_size=1, max_size=80),     # Image Identifier 2
    "TGTID": st.text(alphabet=BCS_A_ALPHABET, min_size=1, max_size=17),    # Target Identifier
    "ISORCE": st.text(alphabet=ECS_A_ALPHABET, min_size=1, max_size=42),   # Image Source
}


@st.composite
def nitf_user_metadata(draw, min_fields: int = 1, max_fields: int = 4):
    """Strategy for generating valid NITF user-settable image metadata.
    
    Generates metadata using actual NITF image subheader field names that
    users can set and that should be preserved during encode/decode roundtrip.
    
    Args:
        draw: Hypothesis draw function
        min_fields: Minimum number of fields to set
        max_fields: Maximum number of fields to set
    
    Returns:
        Dictionary of NITF field names to values
    """
    available_fields = list(NITF_IMAGE_USER_FIELDS.keys())
    num_fields = draw(st.integers(
        min_value=min_fields, 
        max_value=min(max_fields, len(available_fields))
    ))
    
    # Select random fields
    selected_fields = draw(st.lists(
        st.sampled_from(available_fields),
        min_size=num_fields,
        max_size=num_fields,
        unique=True,
    ))
    
    # Generate values for each field
    result = {}
    for field in selected_fields:
        value = draw(NITF_IMAGE_USER_FIELDS[field])
        # Ensure non-empty after stripping
        if value.strip():
            result[field] = value
    
    return result


@pytest.mark.property
class TestMetadataRoundtrip:
    """Property tests for metadata roundtrip preservation.
    
    For any valid metadata key-value pairs attached to an image, encoding then
    decoding SHALL preserve all metadata key-value pairs.
    
    Note: NITF has a fixed format with predefined fields. This test uses
    user-settable fields (ONAME, OPHONE, FTITLE, IID1, IID2, TGTID, ISORCE)
    that are free-text fields users can populate, as opposed to fields like
    ICAT which are derived from image properties.
    """
    
    @given(
        image_tuple=random_image(min_size=16, max_size=64, min_bands=1, max_bands=3),
        user_metadata=nitf_user_metadata(min_fields=1, max_fields=4),
    )
    @pbt_settings
    def test_metadata_roundtrip_preservation(self, image_tuple, user_metadata):
        """For any valid user-settable NITF fields attached to an image, encoding
        then decoding SHALL preserve all field values.
        
        This test:
        1. Generates a random image with random dimensions, bands, and pixel type
        2. Generates random values for user-settable NITF fields
        3. Attaches metadata to the image and encodes to NITF
        4. Decodes the NITF and retrieves metadata
        5. Verifies all user-provided field values are preserved
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        
        # Skip if no metadata to test
        assume(len(user_metadata) > 0)
        
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)
        
        try:
            # Create metadata provider
            metadata = BufferedMetadataProvider()
            
            # Set required encoding hint (IC=NC for uncompressed)
            metadata.set("IC", "NC")
            
            # Set user-provided NITF fields
            for key, value in user_metadata.items():
                metadata.set(key, value)
            
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
            
            # Set image data (array is in BSQ format: bands, rows, cols)
            provider.set_full_image(array)
            
            # Write to NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test Image",
                description="Property test image for metadata roundtrip",
                roles=["data"],
            )
            writer.close()
            
            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image_segment_0")
            
            # Get metadata from decoded asset
            decoded_metadata = asset.get_metadata()
            decoded_dict = decoded_metadata.as_dict()
            
            reader.close()
            
            # Requirement 5.1: Verify user-provided metadata is preserved
            # Requirement 5.3: Both file-level and image-level metadata
            for key, expected_value in user_metadata.items():
                actual_value = decoded_dict.get(key)
                
                # NITF fields have fixed widths and may be padded with spaces
                # Strip both values for comparison
                if actual_value is not None:
                    actual_value = actual_value.strip()
                expected_value_stripped = expected_value.strip()
                
                assert actual_value == expected_value_stripped, (
                    f"Metadata field '{key}' mismatch: "
                    f"expected '{expected_value_stripped}', got '{actual_value}'"
                )
            
        finally:
            if path.exists():
                path.unlink()
