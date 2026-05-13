"""Property-based tests for Data Extension Segment (DES) roundtrip.

This module validates:
- Arbitrary bytes survive a write/read cycle through DES segments
- DESID/DESVER metadata roundtrips correctly
- Various MIME types are preserved
"""

import tempfile
from pathlib import Path

import pytest
from aws.osml.io import (
    IO,
    AssetType,
    BufferedDataAssetProvider,
    BufferedMetadataProvider,
)
from hypothesis import given
from hypothesis import strategies as st

from ..conftest import pbt_settings

# Strategy for arbitrary binary payloads
binary_payload_strategy = st.binary(min_size=1, max_size=2000)

# Strategy for valid XML strings (well-formed minimal XML)
xml_content_strategy = st.text(
    alphabet=st.characters(
        whitelist_categories=("L", "N", "Zs"),
        min_codepoint=32,
        max_codepoint=126,
    ),
    min_size=1,
    max_size=200,
).map(lambda s: f"<root>{s}</root>".encode("utf-8"))

# Strategy for MIME types
mime_type_strategy = st.sampled_from([
    "application/octet-stream",
    "application/xml",
    "application/json",
    "text/plain",
    "application/x-custom",
])

# Strategy for valid DESID values (1-25 ASCII chars, no spaces for easier assertion)
desid_strategy = st.text(
    alphabet=st.characters(
        whitelist_categories=("Lu",),
        min_codepoint=ord("A"),
        max_codepoint=ord("Z"),
    ),
    min_size=1,
    max_size=25,
)

# Strategy for valid DESVER values (exactly 2 digit characters)
desver_strategy = st.from_regex(r"[0-9]{2}", fullmatch=True)


@pytest.mark.property
class TestDataSegmentRoundtrip:
    """Property tests for DES roundtrip through NITF write/read cycle."""

    @given(payload=binary_payload_strategy)
    @pbt_settings
    def test_binary_payload_roundtrip(self, payload):
        """For any binary payload, writing to a DES and reading back SHALL
        produce identical bytes.
        """
        meta = BufferedMetadataProvider()
        meta.set("DESID", "TESTDATA")
        meta.set("DESVER", "01")

        provider = BufferedDataAssetProvider.create(
            key="des:0",
            data=payload,
            mime_type="application/octet-stream",
            metadata=meta,
        )

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset("des:0", provider, "Test", "", ["data"])
            writer.close()

            reader = IO.open([str(path)], "r")
            data_keys = reader.get_asset_keys(asset_type=AssetType.Data)
            assert len(data_keys) == 1, f"Expected 1 DES, got {len(data_keys)}"

            asset = reader.get_asset(data_keys[0])
            read_bytes = asset.get_raw_asset().read()
            assert read_bytes == payload, (
                f"Payload mismatch: wrote {len(payload)} bytes, "
                f"read {len(read_bytes)} bytes"
            )
            reader.close()
        finally:
            if path.exists():
                path.unlink()

    @given(xml_data=xml_content_strategy)
    @pbt_settings
    def test_xml_payload_roundtrip(self, xml_data):
        """For any valid XML content, writing to a DES and reading back
        SHALL produce identical bytes and parse_as_xml() SHALL succeed.
        """
        meta = BufferedMetadataProvider()
        meta.set("DESID", "XML_DATA_CONTENT")
        meta.set("DESVER", "01")

        provider = BufferedDataAssetProvider.create(
            key="des:0",
            data=xml_data,
            mime_type="application/xml",
            metadata=meta,
        )

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset("des:0", provider, "XML", "", ["metadata"])
            writer.close()

            reader = IO.open([str(path)], "r")
            data_keys = reader.get_asset_keys(asset_type=AssetType.Data)
            asset = reader.get_asset(data_keys[0])

            read_bytes = asset.get_raw_asset().read()
            assert read_bytes == xml_data

            elem = asset.parse_as_xml()
            assert elem.tag == "root"
            reader.close()
        finally:
            if path.exists():
                path.unlink()

    @given(
        desid=desid_strategy,
        desver=desver_strategy,
    )
    @pbt_settings
    def test_desid_desver_metadata_roundtrip(self, desid, desver):
        """For any valid DESID/DESVER pair, writing and reading back SHALL
        preserve both values in the asset metadata.
        """
        meta = BufferedMetadataProvider()
        meta.set("DESID", desid)
        meta.set("DESVER", desver)

        provider = BufferedDataAssetProvider.create(
            key="des:0",
            data=b"payload",
            mime_type="application/octet-stream",
            metadata=meta,
        )

        with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
            path = Path(f.name)

        try:
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset("des:0", provider, "Test", "", ["data"])
            writer.close()

            reader = IO.open([str(path)], "r")
            data_keys = reader.get_asset_keys(asset_type=AssetType.Data)
            asset = reader.get_asset(data_keys[0])

            asset_meta = asset.get_metadata().as_dict()
            read_desid = asset_meta.get("DESID", "").strip()
            read_desver = asset_meta.get("DESVER", "").strip()

            assert read_desid == desid, (
                f"DESID mismatch: wrote '{desid}', read '{read_desid}'"
            )
            assert read_desver == desver, (
                f"DESVER mismatch: wrote '{desver}', read '{read_desver}'"
            )
            reader.close()
        finally:
            if path.exists():
                path.unlink()

    @given(mime_type=mime_type_strategy, payload=binary_payload_strategy)
    @pbt_settings
    def test_mime_type_preserved_in_provider(self, mime_type, payload):
        """For any MIME type, the BufferedDataAssetProvider SHALL preserve
        the mime_type property in memory (NITF does not store MIME types
        in the DES subheader, so this is a provider-level guarantee).
        """
        provider = BufferedDataAssetProvider.create(
            key="des:0",
            data=payload,
            mime_type=mime_type,
        )

        assert provider.mime_type == mime_type
        assert provider.media_type == mime_type
