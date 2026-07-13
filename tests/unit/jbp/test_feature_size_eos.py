"""Structural-feature test: ``size-eos: true`` fields.

Exercises the ``size-eos`` construct through the public dict API using a
synthetic, unit-only ``.ksy`` written to ``tmp_path``. A ``size-eos`` field
consumes all remaining bytes, so ``decode`` must return the entire (per-field)
input rather than an empty string.

Recast from the removed ``tests/unit/tre/test_comnta_parsing.py``, which drove
the real COMNTA TRE through ``StructureAccessor``/``StructureWriter``. Because
the construct is structure-agnostic, this file uses a synthetic single-field
``size-eos`` structure instead of the shared COMNTA definition, and drives reads
through ``decode`` and writes through ``encode``. The COMNTA edge cases the prior
file covered are preserved: empty input, single byte, multi-byte UTF-8, CRLF line
endings, and a long (10k) payload simulating a large comment.
"""

import pytest
from aws.osml.io import StructureRegistry

# A single UTF-8 field that consumes the whole input, mirroring COMNTA's COMMENT.
_EOS_KSY = """\
meta:
  id: eos_field
  title: Size-Eos Test Structure
  endian: be
seq:
  - id: BODY
    type: str
    size-eos: true
    encoding: UTF-8
"""


@pytest.fixture
def eos_definition(tmp_path):
    """Write the size-eos KSY to a fresh temp search path and load it."""
    ksy_path = tmp_path / "eos_field.ksy"
    ksy_path.write_text(_EOS_KSY)

    registry = StructureRegistry()
    registry.add_search_path(str(tmp_path))
    defn = registry.get("eos_field")
    assert defn is not None, "eos_field definition not found"
    return defn


class TestSizeEosDecode:
    """A size-eos field returns the entire remaining input."""

    def test_returns_full_input(self, eos_definition):
        decoded = eos_definition.decode(b"This is a test comment")
        assert decoded["BODY"] == "This is a test comment"

    def test_empty_data(self, eos_definition):
        decoded = eos_definition.decode(b"")
        assert decoded["BODY"] == ""

    def test_single_byte(self, eos_definition):
        decoded = eos_definition.decode(b"X")
        assert decoded["BODY"] == "X"

    def test_multibyte_utf8(self, eos_definition):
        decoded = eos_definition.decode("Hello 世界!".encode("utf-8"))
        assert decoded["BODY"] == "Hello 世界!"

    def test_multiline_crlf(self, eos_definition):
        decoded = eos_definition.decode(b"Line 1\r\nLine 2\r\nLine 3")
        assert decoded["BODY"] == "Line 1\r\nLine 2\r\nLine 3"

    def test_long_text(self, eos_definition):
        decoded = eos_definition.decode(b"A" * 10000)
        assert decoded["BODY"] == "A" * 10000
        assert len(decoded["BODY"]) == 10000


class TestSizeEosRoundTrip:
    """Writing a size-eos field and reading it back preserves the value."""

    def test_round_trip(self, eos_definition):
        raw = eos_definition.encode({"BODY": "Hello, world!"})
        assert raw == b"Hello, world!"
        assert eos_definition.decode(raw)["BODY"] == "Hello, world!"

    def test_empty(self, eos_definition):
        raw = eos_definition.encode({"BODY": ""})
        assert raw == b""
        assert eos_definition.decode(raw)["BODY"] == ""

    def test_multibyte_utf8(self, eos_definition):
        raw = eos_definition.encode({"BODY": "Hello 世界!"})
        assert raw == "Hello 世界!".encode("utf-8")
        assert eos_definition.decode(raw)["BODY"] == "Hello 世界!"
