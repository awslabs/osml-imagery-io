"""Property-based tests for metadata preservation.

The tests verify that user-settable NITF fields survive encode/decode cycles.
"""

import tempfile
from pathlib import Path

import pytest
from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
)
from hypothesis import assume, given
from hypothesis import strategies as st

from ..conftest import pbt_settings
from ..strategies import (
    random_image,
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
            metadata["IC"] = "NC"

            # Set user-provided NITF fields
            for key, value in user_metadata.items():
                metadata[key] = value

            # Create image provider
            provider = BufferedImageAssetProvider.create(
                key="image:0",
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
                key="image:0",
                provider=provider,
                title="Test Image",
                description="Property test image for metadata roundtrip",
                roles=["data"],
            )
            writer.close()

            # Read back
            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            # Get metadata from decoded asset
            decoded_metadata = asset.metadata
            decoded_dict = decoded_metadata.entries()

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


@pytest.mark.property
class TestMetadataStructure:
    """Property tests for metadata output structure after restructuring.

    Feature: metadata-restructure, Property 8: Metadata Round-Trip Preservation

    For any valid NITF image, the decoded metadata dictionary SHALL contain:
    - Scalar subheader fields as top-level string values
    - Repeated fields (e.g. BAND_INFO) as lists
    - User-settable fields preserved after encode/decode

    Validates: Requirements 14.1, 14.2
    """

    @given(
        image_tuple=random_image(min_size=16, max_size=64, min_bands=1, max_bands=3),
        user_metadata=nitf_user_metadata(min_fields=1, max_fields=4),
    )
    @pbt_settings
    def test_metadata_structure_after_roundtrip(self, image_tuple, user_metadata):
        """Decoded metadata SHALL have nested structure: repeated fields as lists,
        scalar fields as strings, and user-provided values preserved."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple
        assume(len(user_metadata) > 0)

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            metadata = BufferedMetadataProvider()
            metadata["IC"] = "NC"
            for key, value in user_metadata.items():
                metadata[key] = value

            provider = BufferedImageAssetProvider.create(
                key="image:0",
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
                key="image:0",
                provider=provider,
                title="Test Image",
                description="Property test for metadata structure",
                roles=["data"],
            )
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")
            decoded_dict = asset.metadata.entries()
            reader.close()

            # Verify BAND_INFO is a list (repeated field → array)
            if "BAND_INFO" in decoded_dict:
                assert isinstance(decoded_dict["BAND_INFO"], list), (
                    f"BAND_INFO should be a list, got {type(decoded_dict['BAND_INFO'])}"
                )
                assert len(decoded_dict["BAND_INFO"]) == num_bands, (
                    f"BAND_INFO length should match band count: "
                    f"expected {num_bands}, got {len(decoded_dict['BAND_INFO'])}"
                )
                # Each element should be a dict with band fields
                for i, band in enumerate(decoded_dict["BAND_INFO"]):
                    assert isinstance(band, dict), (
                        f"BAND_INFO[{i}] should be a dict, got {type(band)}"
                    )

            # Verify user-settable fields are preserved as top-level strings
            for key, expected_value in user_metadata.items():
                actual = decoded_dict.get(key)
                if actual is not None:
                    assert isinstance(actual, str), (
                        f"Field '{key}' should be a string, got {type(actual)}"
                    )
                    assert actual.strip() == expected_value.strip(), (
                        f"Field '{key}': expected '{expected_value.strip()}', "
                        f"got '{actual.strip()}'"
                    )

            # Verify no dot-separated TRE keys exist (old format)
            for key in decoded_dict:
                assert "." not in key, (
                    f"Found dot-separated key '{key}' — old flat format detected"
                )

        finally:
            if path.exists():
                path.unlink()


@pytest.mark.property
class TestMetadataRawBytes:
    """Property tests for metadata raw byte access.

    For any NITF image segment, metadata.raw SHALL return bytes whose first
    two characters are the segment identifier "IM" (the NITF image subheader
    marker).
    """

    @given(
        image_tuple=random_image(min_size=16, max_size=64, min_bands=1, max_bands=3),
    )
    @pbt_settings
    def test_metadata_raw_starts_with_im(self, image_tuple):
        """For any NITF image segment, metadata.raw SHALL return bytes starting
        with b"IM" (the image subheader identifier).
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            metadata = BufferedMetadataProvider()
            metadata["IC"] = "NC"

            provider = BufferedImageAssetProvider.create(
                key="image:0",
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
                key="image:0",
                provider=provider,
                title="Test Image",
                description="Property test image for metadata raw bytes",
                roles=["data"],
            )
            writer.close()

            reader = IO.open([str(path)], "r")
            asset = reader.get_asset("image:0")

            raw_metadata = asset.metadata
            raw_io = raw_metadata.raw
            raw_bytes = raw_io.read()

            assert isinstance(raw_bytes, bytes), (
                f"Expected bytes, got {type(raw_bytes)}"
            )
            assert len(raw_bytes) > 2, (
                f"Raw metadata too short: {len(raw_bytes)} bytes"
            )
            assert raw_bytes[:2] == b"IM", (
                f"Expected raw metadata to start with b'IM', "
                f"got {raw_bytes[:2]!r}"
            )

            reader.close()
        finally:
            if path.exists():
                path.unlink()
