"""Property-based tests for Text Segment roundtrip and API completeness.

This module contains property tests that validate:
- Text segment roundtrip (write and read via NITF files)
- TextAssetProvider Python API completeness
- Media type mapping
- Unknown format code handling
"""

import io
import string
import tempfile
from pathlib import Path

import pytest
from aws.osml.io import (
    IO,
    AssetType,
    BufferedTextAssetProvider,
)
from hypothesis import given
from hypothesis import strategies as st

from ..conftest import pbt_settings

# Strategy for generating valid text content (printable ASCII)
text_content_strategy = st.text(
    alphabet=string.printable.replace('\x0b', '').replace('\x0c', ''),  # Remove vertical tab and form feed
    min_size=1,
    max_size=500,
)

# Strategy for valid encoding names
encoding_strategy = st.sampled_from(["ASCII", "UTF-8", "ECS", "MTF"])

# Strategy for valid asset keys
key_strategy = st.text(
    min_size=1,
    max_size=20,
    alphabet=st.characters(
        whitelist_categories=('L', 'N'),
        min_codepoint=ord('a'),
        max_codepoint=ord('z')
    )
)


@pytest.mark.property
class TestTextSegmentRoundtrip:
    """Property tests for text segment roundtrip (write and read via NITF).

    These tests verify that text segments can be written to NITF files
    and read back with content preserved correctly.
    """

    @given(
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=500,
        ),
    )
    @pbt_settings
    def test_text_segment_roundtrip(self, text_content):
        """For any valid text content, writing a text segment to a NITF
        file and reading it back SHALL produce text equivalent to the original
        (modulo line ending normalization).
        """
        from aws.osml.io import AssetProvider

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            # Normalize line endings to CR/LF for NITF format
            normalized_content = text_content.replace('\r\n', '\n').replace('\r', '\n').replace('\n', '\r\n')
            text_bytes = normalized_content.encode('utf-8')

            # Create a text segment using AssetProvider.from_bytes
            text_asset = AssetProvider.from_bytes(
                key="text_segment_0",
                data=text_bytes,
                asset_type=AssetType.Text,
                title="Test Text",
                description="Property test text segment",
            )

            # Write the NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                "text_segment_0",
                text_asset,
                "Test Text",
                "Property test text segment",
                ["annotation"],
            )
            writer.close()

            # Read back the file
            reader = IO.open([str(path)], "r")

            # Get the text segment
            text_keys = reader.get_asset_keys(asset_type=AssetType.Text)
            assert len(text_keys) == 1, f"Expected 1 text segment, got {len(text_keys)}"

            # Get the asset and verify
            asset = reader.get_asset(text_keys[0])
            assert asset is not None, "Failed to get text asset"

            # Verify asset type
            assert asset.asset_type == AssetType.Text, f"Expected Text, got {asset.asset_type}"

            # Verify raw bytes roundtrip
            raw_data = asset.get_raw_asset().read()
            assert raw_data == text_bytes, (
                f"Raw bytes mismatch: "
                f"original length={len(text_bytes)}, "
                f"read length={len(raw_data)}"
            )

            reader.close()

        finally:
            if path.exists():
                path.unlink()

    @given(
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n\r',
            min_size=1,
            max_size=200,
        ),
    )
    @pbt_settings
    def test_line_delimiter_normalization_roundtrip(self, text_content):
        """For any text content with any combination of line endings (LF, CR, CR/LF),
        the BufferedTextAssetProvider's raw_asset() method SHALL return bytes with
        all line endings converted to CR/LF, and reading back SHALL preserve the
        CR/LF format.
        """
        # Create a BufferedTextAssetProvider to test line ending normalization
        text_asset = BufferedTextAssetProvider.create(
            key="text_segment_0",
            text_content=text_content,
            encoding="UTF-8",
        )

        # Verify raw_asset has CR/LF line endings
        raw_bytes = text_asset.get_raw_asset().read()
        raw_str = raw_bytes.decode('utf-8')

        # Check that all line endings are CR/LF (no standalone LF or CR)
        # First normalize the original to count expected line endings
        temp = text_content.replace('\r\n', '\n').replace('\r', '\n')
        expected_newlines = temp.count('\n')
        actual_crlf = raw_str.count('\r\n')
        # Standalone CR or LF should not exist
        standalone_cr = raw_str.count('\r') - actual_crlf
        standalone_lf = raw_str.count('\n') - actual_crlf

        assert standalone_cr == 0, f"Found {standalone_cr} standalone CR characters"
        assert standalone_lf == 0, f"Found {standalone_lf} standalone LF characters"
        assert actual_crlf == expected_newlines, (
            f"Expected {expected_newlines} CR/LF pairs, got {actual_crlf}"
        )

    @given(
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        title=st.text(
            min_size=1,
            max_size=20,
            alphabet=st.characters(
                whitelist_categories=('L', 'N', 'Zs'),
                min_codepoint=32,
                max_codepoint=126
            )
        ).filter(lambda x: x.strip()),
        description=st.text(
            max_size=50,
            alphabet=st.characters(
                whitelist_categories=('L', 'N', 'Zs'),
                min_codepoint=32,
                max_codepoint=126
            )
        ),
    )
    @pbt_settings
    def test_text_segment_python_api_via_nitf(self, text_content, title, description):
        """For any text segment accessed via Python's DatasetReader.get_asset(),
        the returned TextAssetProvider SHALL expose all required properties
        and methods.
        """
        from aws.osml.io import AssetProvider

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            # Normalize line endings to CR/LF for NITF format
            normalized_content = text_content.replace('\r\n', '\n').replace('\r', '\n').replace('\n', '\r\n')
            text_bytes = normalized_content.encode('utf-8')

            # Create a text segment using AssetProvider.from_bytes
            text_asset = AssetProvider.from_bytes(
                key="text_segment_0",
                data=text_bytes,
                asset_type=AssetType.Text,
                title=title,
                description=description,
            )

            # Write the NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                "text_segment_0",
                text_asset,
                title,
                description,
                ["annotation"],
            )
            writer.close()

            # Read back the file
            reader = IO.open([str(path)], "r")

            # Get the text segment
            text_keys = reader.get_asset_keys(asset_type=AssetType.Text)
            assert len(text_keys) == 1, f"Expected 1 text segment, got {len(text_keys)}"

            # Get the asset
            asset = reader.get_asset(text_keys[0])
            assert asset is not None, "Failed to get text asset"

            # Requirement 8.1: Verify all AssetProvider properties are exposed
            assert hasattr(asset, 'key'), "TextAssetProvider missing 'key' property"
            assert isinstance(asset.key, str), f"key should be str, got {type(asset.key)}"
            assert asset.key == text_keys[0]

            assert hasattr(asset, 'title'), "TextAssetProvider missing 'title' property"
            assert isinstance(asset.title, str)

            assert hasattr(asset, 'description'), "TextAssetProvider missing 'description' property"
            assert isinstance(asset.description, str)

            assert hasattr(asset, 'media_type'), "TextAssetProvider missing 'media_type' property"
            assert isinstance(asset.media_type, str)
            assert 'text/plain' in asset.media_type

            assert hasattr(asset, 'roles'), "TextAssetProvider missing 'roles' property"
            assert isinstance(asset.roles, list)

            assert hasattr(asset, 'asset_type'), "TextAssetProvider missing 'asset_type' property"
            assert asset.asset_type == AssetType.Text

            # Requirement 8.2: Verify get_raw_asset() returns BytesIO
            assert hasattr(asset, 'get_raw_asset'), "TextAssetProvider missing 'get_raw_asset' method"
            raw_asset = asset.get_raw_asset()
            assert isinstance(raw_asset, io.BytesIO)
            raw_bytes = raw_asset.read()
            assert isinstance(raw_bytes, bytes)
            assert len(raw_bytes) > 0

            # Requirement 8.3: Verify get_metadata() returns MetadataProvider
            assert hasattr(asset, 'get_metadata'), "TextAssetProvider missing 'get_metadata' method"
            metadata = asset.get_metadata()
            assert metadata is not None
            assert hasattr(metadata, 'as_dict')
            metadata_dict = metadata.as_dict()
            assert isinstance(metadata_dict, dict)

            # Requirement 8.4: Verify text property
            assert hasattr(asset, 'text'), "TextAssetProvider missing 'text' property"
            assert isinstance(asset.text, str)

            # Requirement 8.5: Verify encoding property
            assert hasattr(asset, 'encoding'), "TextAssetProvider missing 'encoding' property"
            assert isinstance(asset.encoding, str)

            # Requirement 8.6: Verify format property
            assert hasattr(asset, 'format'), "TextAssetProvider missing 'format' property"
            assert isinstance(asset.format, str)

            reader.close()

        finally:
            if path.exists():
                path.unlink()


@pytest.mark.property
class TestTextAssetProviderAPICompleteness:
    """Property tests for TextAssetProvider Python API completeness.

    These tests verify that the TextAssetProvider Python API exposes
    all required properties and methods as specified.
    """

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_buffered_text_asset_provider_exposes_all_asset_properties(
        self, key, text_content, encoding
    ):
        """For any BufferedTextAssetProvider, all AssetProvider properties SHALL be accessible.

        Tests that key, title, description, media_type, roles, asset_type
        properties are all accessible.
        """
        # Skip non-ASCII content for ASCII encoding
        if encoding == "ASCII" and not text_content.isascii():
            return

        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding=encoding,
        )

        # Requirement 8.1: All AssetProvider properties exposed
        assert provider.key == key
        assert isinstance(provider.title, str)
        assert isinstance(provider.description, str)
        assert isinstance(provider.media_type, str)
        assert isinstance(provider.roles, list)
        assert provider.asset_type == AssetType.Text

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_buffered_text_asset_provider_get_raw_asset_returns_bytesio(
        self, key, text_content, encoding
    ):
        """For any BufferedTextAssetProvider, get_raw_asset() SHALL return BytesIO."""
        # Skip non-ASCII content for ASCII encoding
        if encoding == "ASCII" and not text_content.isascii():
            return

        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding=encoding,
        )

        # Requirement 8.2: get_raw_asset() returns BytesIO
        raw_asset = provider.get_raw_asset()
        assert isinstance(raw_asset, io.BytesIO)

        # Verify we can read bytes from it
        raw_bytes = raw_asset.read()
        assert isinstance(raw_bytes, bytes)
        assert len(raw_bytes) > 0

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_buffered_text_asset_provider_get_metadata_returns_metadata_provider(
        self, key, text_content, encoding
    ):
        """For any BufferedTextAssetProvider, get_metadata() SHALL return MetadataProvider."""
        # Skip non-ASCII content for ASCII encoding
        if encoding == "ASCII" and not text_content.isascii():
            return

        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding=encoding,
        )

        # Requirement 8.3: get_metadata() returns MetadataProvider
        metadata = provider.get_metadata()
        assert metadata is not None

        # MetadataProvider should have as_dict method
        metadata_dict = metadata.as_dict()
        assert isinstance(metadata_dict, dict)

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_buffered_text_asset_provider_text_property_returns_string(
        self, key, text_content, encoding
    ):
        """For any BufferedTextAssetProvider, text property SHALL return string."""
        # Skip non-ASCII content for ASCII encoding
        if encoding == "ASCII" and not text_content.isascii():
            return

        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding=encoding,
        )

        # Requirement 8.4: text property returns string
        text = provider.text
        assert isinstance(text, str)
        assert text == text_content

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_buffered_text_asset_provider_encoding_property_returns_string(
        self, key, text_content, encoding
    ):
        """For any BufferedTextAssetProvider, encoding property SHALL return string."""
        # Skip non-ASCII content for ASCII encoding
        if encoding == "ASCII" and not text_content.isascii():
            return

        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding=encoding,
        )

        # Requirement 8.5: encoding property returns string
        enc = provider.encoding
        assert isinstance(enc, str)
        assert enc == encoding

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_buffered_text_asset_provider_format_property_returns_string(
        self, key, text_content, encoding
    ):
        """For any BufferedTextAssetProvider, format property SHALL return string."""
        # Skip non-ASCII content for ASCII encoding
        if encoding == "ASCII" and not text_content.isascii():
            return

        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding=encoding,
        )

        # Requirement 8.6: format property returns string
        fmt = provider.format
        assert isinstance(fmt, str)

        # Verify format code matches encoding
        expected_format = {
            "ASCII": "STA",
            "UTF-8": "U8S",
            "ECS": "UT1",
            "MTF": "MTF",
        }
        assert fmt == expected_format[encoding]


@pytest.mark.property
class TestTextAssetProviderMediaTypeMapping:
    """Property tests for TextAssetProvider media type mapping."""

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' ',
            min_size=1,
            max_size=100,
        ),
    )
    @pbt_settings
    def test_media_type_mapping_ascii(self, key, text_content):
        """For TXTFMT=STA, media_type SHALL return 'text/plain; charset=us-ascii'."""
        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding="ASCII",
        )

        assert provider.media_type == "text/plain; charset=us-ascii"

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' ',
            min_size=1,
            max_size=100,
        ),
    )
    @pbt_settings
    def test_media_type_mapping_utf8(self, key, text_content):
        """For TXTFMT=U8S, media_type SHALL return 'text/plain; charset=utf-8'."""
        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding="UTF-8",
        )

        assert provider.media_type == "text/plain; charset=utf-8"

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' ',
            min_size=1,
            max_size=100,
        ),
    )
    @pbt_settings
    def test_media_type_mapping_ecs(self, key, text_content):
        """For TXTFMT=UT1, media_type SHALL return 'text/plain; charset=iso-8859-1'."""
        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding="ECS",
        )

        assert provider.media_type == "text/plain; charset=iso-8859-1"

    @given(
        key=key_strategy,
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' ',
            min_size=1,
            max_size=100,
        ),
    )
    @pbt_settings
    def test_media_type_mapping_mtf(self, key, text_content):
        """For TXTFMT=MTF, media_type SHALL return 'text/plain'."""
        provider = BufferedTextAssetProvider.create(
            key=key,
            text_content=text_content,
            encoding="MTF",
        )

        assert provider.media_type == "text/plain"


@pytest.mark.property
class TestTextMetadataAccess:
    """Property tests for text segment metadata access.

    These tests verify that text segment metadata is correctly exposed
    through the Python API, including format codes and attachment levels.
    """

    @given(
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
    )
    @pbt_settings
    def test_format_code_accessible_via_metadata(self, text_content):
        """Format code is accessible via metadata.

        For any text segment, the TXTFMT field SHALL be accessible via
        metadata.as_dict()["TXTFMT"].
        """
        from aws.osml.io import AssetProvider

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            # Normalize line endings to CR/LF for NITF format
            normalized_content = text_content.replace('\r\n', '\n').replace('\r', '\n').replace('\n', '\r\n')
            text_bytes = normalized_content.encode('utf-8')

            # Create a text segment
            text_asset = AssetProvider.from_bytes(
                key="text_segment_0",
                data=text_bytes,
                asset_type=AssetType.Text,
                title="Test Text",
                description="Format code test",
            )

            # Write the NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                "text_segment_0",
                text_asset,
                "Test Text",
                "Format code test",
                ["annotation"],
            )
            writer.close()

            # Read back the file
            reader = IO.open([str(path)], "r")
            text_keys = reader.get_asset_keys(asset_type=AssetType.Text)
            asset = reader.get_asset(text_keys[0])

            # Verify format property is accessible
            assert hasattr(asset, 'format'), "TextAssetProvider missing 'format' property"
            fmt = asset.format
            assert isinstance(fmt, str), f"format should be str, got {type(fmt)}"

            # Verify TXTFMT is in metadata
            metadata = asset.get_metadata()
            metadata_dict = metadata.as_dict()
            assert "TXTFMT" in metadata_dict, "TXTFMT not found in metadata"
            assert isinstance(metadata_dict["TXTFMT"], str)

            reader.close()

        finally:
            if path.exists():
                path.unlink()

    @given(
        text_content=st.text(
            alphabet=string.ascii_letters + string.digits + ' \n',
            min_size=1,
            max_size=200,
        ),
    )
    @pbt_settings
    def test_attachment_level_accessible_via_metadata(self, text_content):
        """Attachment level is accessible via metadata.

        For any text segment, the TXTALVL field SHALL be accessible via
        metadata.as_dict()["TXTALVL"], and parsing SHALL succeed regardless
        of whether the referenced display level exists.
        """
        from aws.osml.io import AssetProvider

        with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
            path = Path(f.name)

        try:
            # Normalize line endings to CR/LF for NITF format
            normalized_content = text_content.replace('\r\n', '\n').replace('\r', '\n').replace('\n', '\r\n')
            text_bytes = normalized_content.encode('utf-8')

            # Create a text segment
            text_asset = AssetProvider.from_bytes(
                key="text_segment_0",
                data=text_bytes,
                asset_type=AssetType.Text,
                title="Test Text",
                description="Attachment level test",
            )

            # Write the NITF file
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset(
                "text_segment_0",
                text_asset,
                "Test Text",
                "Attachment level test",
                ["annotation"],
            )
            writer.close()

            # Read back the file
            reader = IO.open([str(path)], "r")
            text_keys = reader.get_asset_keys(asset_type=AssetType.Text)
            asset = reader.get_asset(text_keys[0])

            # Verify TXTALVL is in metadata
            metadata = asset.get_metadata()
            metadata_dict = metadata.as_dict()
            assert "TXTALVL" in metadata_dict, "TXTALVL not found in metadata"

            # TXTALVL should be a string representation of the attachment level
            txtalvl = metadata_dict["TXTALVL"]
            assert isinstance(txtalvl, str), f"TXTALVL should be str, got {type(txtalvl)}"

            # The writer defaults to "000" (unattached), verify it's a valid format
            # TXTALVL is a 3-character numeric string (000-998)
            assert len(txtalvl.strip()) <= 3, f"TXTALVL should be at most 3 chars, got {len(txtalvl)}"

            reader.close()

        finally:
            if path.exists():
                path.unlink()

    @given(
        encoding=encoding_strategy,
    )
    @pbt_settings
    def test_encoding_to_format_mapping(self, encoding):
        """Encoding name maps to correct format code.

        For any BufferedTextAssetProvider with a known encoding, the format()
        method SHALL return the corresponding TXTFMT code.
        """
        provider = BufferedTextAssetProvider.create(
            key="test",
            text_content="test content",
            encoding=encoding,
        )

        expected_format = {
            "ASCII": "STA",
            "UTF-8": "U8S",
            "ECS": "UT1",
            "MTF": "MTF",
        }

        assert provider.format == expected_format[encoding], (
            f"Expected format {expected_format[encoding]} for encoding {encoding}, "
            f"got {provider.format}"
        )
