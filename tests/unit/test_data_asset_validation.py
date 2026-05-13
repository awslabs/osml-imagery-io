"""Tests for DESID/DESVER validation at DES write time."""

import tempfile

import pytest
from aws.osml.io import IO, BufferedDataAssetProvider, BufferedMetadataProvider


def _write_des(desid=None, desver=None):
    """Helper to write a DES segment with given DESID/DESVER metadata."""
    meta = BufferedMetadataProvider()
    if desid is not None:
        meta.set("DESID", desid)
    if desver is not None:
        meta.set("DESVER", desver)

    provider = BufferedDataAssetProvider.create(
        key="des:0",
        data=b"<root/>",
        mime_type="application/xml",
        metadata=meta,
    )

    with tempfile.NamedTemporaryFile(suffix=".ntf", delete=True) as f:
        with IO.open([f.name], "w") as writer:
            writer.add_asset("des:0", provider, "Test", "", ["metadata"])


class TestDesidValidation:
    def test_valid_desid_accepted(self):
        _write_des(desid="XML_DATA_CONTENT", desver="01")

    def test_desid_max_length_accepted(self):
        _write_des(desid="A" * 25, desver="01")

    def test_desid_single_char_accepted(self):
        _write_des(desid="X", desver="01")

    def test_desid_too_long_rejected(self):
        with pytest.raises(Exception, match="DESID"):
            _write_des(desid="A" * 26, desver="01")

    def test_desid_empty_rejected(self):
        with pytest.raises(Exception, match="DESID"):
            _write_des(desid="", desver="01")


class TestDesverValidation:
    def test_valid_desver_accepted(self):
        _write_des(desid="TEST", desver="01")

    def test_desver_two_chars_accepted(self):
        _write_des(desid="TEST", desver="99")

    def test_desver_one_char_rejected(self):
        with pytest.raises(Exception, match="DESVER"):
            _write_des(desid="TEST", desver="1")

    def test_desver_three_chars_rejected(self):
        with pytest.raises(Exception, match="DESVER"):
            _write_des(desid="TEST", desver="001")

    def test_desver_empty_rejected(self):
        with pytest.raises(Exception, match="DESVER"):
            _write_des(desid="TEST", desver="")
