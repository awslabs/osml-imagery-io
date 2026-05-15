"""Property-based tests for JBP Graphic Segments.

This module contains property tests that validate graphic segment functionality
including CGM data round-trip and bounds validation.
"""

import io
import tempfile
from pathlib import Path

import pytest
from aws.osml.io import (
    IO,
    AssetProvider,
    AssetType,
)
from hypothesis import given
from hypothesis import strategies as st

from ..conftest import pbt_settings


@pytest.mark.property
class TestGraphicSegmentProperties:
    """Property tests for JBP Graphic Segments.

    These tests validate the correctness properties defined in the
    jbp-graphic-segments design document.
    """

    @given(
        cgm_data=st.binary(min_size=1, max_size=10000),
    )
    @pbt_settings
    def test_cgm_data_roundtrip(self, cgm_data):
        """For any NITF file containing a graphic segment with CGM data bytes,
        calling raw_asset() on the JBPGraphicsAssetProvider SHALL return bytes
        identical to the original CGM data portion of the segment.
        """
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            # Create a graphic segment with the generated CGM data
            graphic_asset = AssetProvider.from_bytes(
                key="graphic:0",
                data=cgm_data,
                asset_type=AssetType.Graphics,
                title="Test Graphic",
                description="Property test graphic segment",
            )

            # Write the NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                "graphic:0",
                graphic_asset,
                "Test Graphic",
                "Property test graphic segment",
                ["annotation"],
            )
            writer.close()

            # Read back the file
            reader = IO.open([str(path)], "r")

            # Get the graphic segment
            graphic_keys = reader.get_asset_keys(asset_type=AssetType.Graphics)
            assert len(graphic_keys) == 1, f"Expected 1 graphic segment, got {len(graphic_keys)}"

            # Get the asset and verify raw data
            asset = reader.get_asset(graphic_keys[0])
            assert asset is not None, "Failed to get graphic asset"

            # Verify asset type
            assert asset.asset_type == AssetType.Graphics, f"Expected Graphics, got {asset.asset_type}"

            # Verify media type
            assert asset.media_type == "image/cgm", f"Expected image/cgm, got {asset.media_type}"

            # Get raw asset data and verify round-trip
            raw_data = asset.raw_asset.read()
            assert raw_data == cgm_data, (
                f"CGM data round-trip failed: "
                f"original length={len(cgm_data)}, "
                f"read length={len(raw_data)}"
            )

            reader.close()

        finally:
            if path.exists():
                path.unlink()


    @given(
        cgm_data=st.binary(min_size=1, max_size=1000),
        title=st.text(min_size=1, max_size=20, alphabet=st.characters(
            whitelist_categories=('L', 'N', 'Zs'),
            min_codepoint=32,
            max_codepoint=126
        )).filter(lambda x: x.strip()),
        description=st.text(max_size=50, alphabet=st.characters(
            whitelist_categories=('L', 'N', 'Zs'),
            min_codepoint=32,
            max_codepoint=126
        )),
    )
    @pbt_settings
    def test_python_api_completeness(self, cgm_data, title, description):
        """For any graphic segment accessed via Python's DatasetReader.get_asset(),
        the returned PyGraphicsAssetProvider SHALL expose key, title, description,
        media_type, roles, asset_type properties, raw_asset returning BytesIO,
        and metadata returning PyMetadataProvider.
        """
        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            # Create a graphic segment with the generated CGM data
            graphic_asset = AssetProvider.from_bytes(
                key="graphic:0",
                data=cgm_data,
                asset_type=AssetType.Graphics,
                title=title,
                description=description,
            )

            # Write the NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                "graphic:0",
                graphic_asset,
                title,
                description,
                ["annotation"],
            )
            writer.close()

            # Read back the file
            reader = IO.open([str(path)], "r")

            # Get the graphic segment
            graphic_keys = reader.get_asset_keys(asset_type=AssetType.Graphics)
            assert len(graphic_keys) == 1, f"Expected 1 graphic segment, got {len(graphic_keys)}"

            # Get the asset - this should return a GraphicsAssetProvider
            asset = reader.get_asset(graphic_keys[0])
            assert asset is not None, "Failed to get graphic asset"

            # Requirement 9.1: Verify all AssetProvider properties are exposed
            # key property
            assert hasattr(asset, 'key'), "GraphicsAssetProvider missing 'key' property"
            assert isinstance(asset.key, str), f"key should be str, got {type(asset.key)}"
            assert asset.key == graphic_keys[0], f"key mismatch: {asset.key} != {graphic_keys[0]}"

            # title property
            assert hasattr(asset, 'title'), "GraphicsAssetProvider missing 'title' property"
            assert isinstance(asset.title, str), f"title should be str, got {type(asset.title)}"

            # description property
            assert hasattr(asset, 'description'), "GraphicsAssetProvider missing 'description' property"
            assert isinstance(asset.description, str), f"description should be str, got {type(asset.description)}"

            # media_type property
            assert hasattr(asset, 'media_type'), "GraphicsAssetProvider missing 'media_type' property"
            assert isinstance(asset.media_type, str), f"media_type should be str, got {type(asset.media_type)}"
            assert asset.media_type == "image/cgm", f"Expected media_type 'image/cgm', got '{asset.media_type}'"

            # roles property
            assert hasattr(asset, 'roles'), "GraphicsAssetProvider missing 'roles' property"
            assert isinstance(asset.roles, list), f"roles should be list, got {type(asset.roles)}"

            # asset_type property
            assert hasattr(asset, 'asset_type'), "GraphicsAssetProvider missing 'asset_type' property"
            assert asset.asset_type == AssetType.Graphics, f"Expected AssetType.Graphics, got {asset.asset_type}"

            # Requirement 9.2: Verify raw_asset property returns BytesIO
            assert hasattr(asset, 'raw_asset'), "GraphicsAssetProvider missing 'raw_asset' property"
            raw_asset = asset.raw_asset
            assert isinstance(raw_asset, io.BytesIO), f"raw_asset should return BytesIO, got {type(raw_asset)}"

            # Verify the raw data matches
            raw_data = raw_asset.read()
            assert raw_data == cgm_data, (
                f"CGM data mismatch: original length={len(cgm_data)}, read length={len(raw_data)}"
            )

            # Requirement 9.3: Verify metadata property returns MetadataProvider
            assert hasattr(asset, 'metadata'), "GraphicsAssetProvider missing 'metadata' property"
            metadata = asset.metadata
            assert metadata is not None, "metadata returned None"

            # Verify metadata provider has expected methods
            assert hasattr(metadata, 'entries'), "MetadataProvider missing 'entries' method"
            metadata_dict = metadata.entries()
            assert isinstance(metadata_dict, dict), f"entries() should return dict, got {type(metadata_dict)}"

            reader.close()

        finally:
            if path.exists():
                path.unlink()
