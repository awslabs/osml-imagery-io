"""Unit tests for MultiReferenceFileSystem.

Tests cover:
- Multi-range detection via _is_multi_range()
- Multi-range fetch with single and multiple sub-ranges
- Empty ranges error case
- Mixed reference sets (inline, single-range, multi-range)
- Standard reference compatibility with ReferenceFileSystem
- Async path produces same results as sync path
- Constructor accepts same arguments as ReferenceFileSystem
"""

from __future__ import annotations

import asyncio
import base64
from pathlib import Path

import pytest
from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
from fsspec.implementations.reference import ReferenceFileSystem

# ---------------------------------------------------------------------------
# _is_multi_range detection
# ---------------------------------------------------------------------------


class TestIsMultiRange:
    """Test _is_multi_range() static method for valid and invalid entries."""

    def test_valid_multi_range(self):
        assert MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", [[0, 10], [20, 5]]]
        )

    def test_valid_single_sub_range(self):
        """A single sub-range is still multi-range format."""
        assert MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", [[0, 10]]]
        )

    def test_single_range_entry_not_multi(self):
        """Standard [url, offset, length] is NOT multi-range."""
        assert not MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", 0, 10]
        )

    def test_whole_file_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range(["file:///tmp/f.dat"])

    def test_inline_string_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range("hello")

    def test_inline_bytes_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range(b"hello")

    def test_empty_ranges_not_multi(self):
        """Empty inner list → not multi-range (len check fails)."""
        assert not MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", []]
        )

    def test_second_element_is_flat_list_not_multi(self):
        """[url, [offset, length]] where inner is flat ints → not multi-range."""
        assert not MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", [0, 10]]
        )

    def test_none_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range(None)

    def test_dict_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range({"url": "x"})


# ---------------------------------------------------------------------------
# Helper: create a temp file with known content and build refs
# ---------------------------------------------------------------------------


@pytest.fixture
def data_file(tmp_path):
    """Create a temp file with 100 bytes of known content (0x00..0x63)."""
    content = bytes(range(100))
    p = tmp_path / "data.bin"
    p.write_bytes(content)
    return p, content


def _file_url(path: Path) -> str:
    """Return a file:// URL for a local path."""
    return path.as_uri()


def _make_fs(refs: dict, **kwargs) -> MultiReferenceFileSystem:
    """Build a MultiReferenceFileSystem from a flat refs dict."""
    fo = {"version": 1, "refs": refs}
    return MultiReferenceFileSystem(fo=fo, skip_instance_cache=True, **kwargs)


def _make_ref_fs(refs: dict, **kwargs) -> ReferenceFileSystem:
    """Build a standard ReferenceFileSystem from a flat refs dict."""
    fo = {"version": 1, "refs": refs}
    return ReferenceFileSystem(fo=fo, skip_instance_cache=True, **kwargs)


# ---------------------------------------------------------------------------
# Multi-range fetch tests
# ---------------------------------------------------------------------------


class TestMultiRangeFetch:
    """Test multi-range byte fetching via _cat_common (sync path)."""

    def test_two_non_contiguous_ranges(self, data_file):
        """Fetch two non-contiguous ranges and verify concatenation order."""
        path, content = data_file
        url = _file_url(path)
        # Ranges: bytes [10..15) and [50..55)
        refs = {"chunk/0": [url, [[10, 5], [50, 5]]]}
        fs = _make_fs(refs)
        result = fs.cat("chunk/0")
        expected = content[10:15] + content[50:55]
        assert result == expected

    def test_three_ranges(self, data_file):
        """Fetch three ranges and verify order is preserved."""
        path, content = data_file
        url = _file_url(path)
        refs = {"chunk/0": [url, [[0, 3], [40, 2], [90, 10]]]}
        fs = _make_fs(refs)
        result = fs.cat("chunk/0")
        expected = content[0:3] + content[40:42] + content[90:100]
        assert result == expected

    def test_single_sub_range_degenerates(self, data_file):
        """A multi-range entry with one sub-range works like single-range."""
        path, content = data_file
        url = _file_url(path)
        refs = {"chunk/0": [url, [[20, 10]]]}
        fs = _make_fs(refs)
        result = fs.cat("chunk/0")
        assert result == content[20:30]

    def test_key_not_found_raises(self, data_file):
        path, _ = data_file
        fs = _make_fs({})
        with pytest.raises(FileNotFoundError):
            fs.cat("nonexistent")


# ---------------------------------------------------------------------------
# Mixed reference set
# ---------------------------------------------------------------------------


class TestMixedReferences:
    """Test a reference set mixing inline, single-range, and multi-range."""

    def test_mixed_set(self, data_file):
        path, content = data_file
        url = _file_url(path)
        inline_data = b"inline-payload"
        b64_data = b"base64:" + base64.b64encode(b"secret")

        refs = {
            "inline_str": inline_data.decode(),
            "inline_b64": b64_data.decode(),
            "single_range": [url, 0, 10],
            "multi_range": [url, [[10, 5], [80, 10]]],
        }
        fs = _make_fs(refs)

        assert fs.cat("inline_str") == inline_data
        assert fs.cat("inline_b64") == b"secret"
        assert fs.cat("single_range") == content[0:10]
        assert fs.cat("multi_range") == content[10:15] + content[80:90]


# ---------------------------------------------------------------------------
# Standard reference compatibility with ReferenceFileSystem
# ---------------------------------------------------------------------------


class TestStandardReferenceCompatibility:
    """MultiReferenceFileSystem produces identical bytes to ReferenceFileSystem
    for inline, whole-file, and single-range references."""

    def test_inline_string_identical(self):
        refs = {"key": "hello world"}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")

    def test_inline_base64_identical(self):
        payload = base64.b64encode(b"\x00\x01\x02").decode()
        refs = {"key": f"base64:{payload}"}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")

    def test_single_range_identical(self, data_file):
        path, _ = data_file
        url = _file_url(path)
        refs = {"key": [url, 5, 20]}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")

    def test_whole_file_identical(self, data_file):
        path, content = data_file
        url = _file_url(path)
        refs = {"key": [url]}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")
        assert _make_fs(refs).cat("key") == content


# ---------------------------------------------------------------------------
# Async path
# ---------------------------------------------------------------------------


class TestAsyncPath:
    """Async _cat_file produces same results as sync for identical inputs."""

    def _run_async(self, coro):
        """Run an async coroutine in a fresh event loop."""
        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(coro)
        finally:
            loop.close()

    def test_async_multi_range_matches_sync(self, data_file):
        path, content = data_file
        url = _file_url(path)
        refs = {"chunk/0": [url, [[5, 10], [60, 15]]]}
        fs = _make_fs(refs)

        sync_result = fs.cat("chunk/0")
        async_result = self._run_async(fs._cat_file("chunk/0"))

        expected = content[5:15] + content[60:75]
        assert sync_result == expected
        assert async_result == expected
        assert sync_result == async_result

    def test_async_inline_matches_sync(self):
        refs = {"key": "test-data"}
        fs = _make_fs(refs)

        sync_result = fs.cat("key")
        async_result = self._run_async(fs._cat_file("key"))
        assert sync_result == async_result

    def test_async_single_range_matches_sync(self, data_file):
        path, _ = data_file
        url = _file_url(path)
        refs = {"key": [url, 10, 20]}
        fs = _make_fs(refs)

        sync_result = fs.cat("key")
        async_result = self._run_async(fs._cat_file("key"))
        assert sync_result == async_result

    def test_async_key_not_found(self):
        fs = _make_fs({})
        with pytest.raises(FileNotFoundError):
            self._run_async(fs._cat_file("missing"))


# ---------------------------------------------------------------------------
# Constructor compatibility
# ---------------------------------------------------------------------------


class TestConstructor:
    """MultiReferenceFileSystem accepts same arguments as ReferenceFileSystem."""

    def test_basic_construction(self):
        fo = {"version": 1, "refs": {"k": "v"}}
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        assert isinstance(fs, ReferenceFileSystem)
        assert isinstance(fs, MultiReferenceFileSystem)

    def test_construction_with_remote_options(self, data_file):
        path, _ = data_file
        url = _file_url(path)
        fo = {"version": 1, "refs": {"k": [url, 0, 5]}}
        fs = MultiReferenceFileSystem(
            fo=fo,
            skip_instance_cache=True,
            remote_options={"auto_mkdir": True},
        )
        assert isinstance(fs, MultiReferenceFileSystem)
        # Should still be able to read
        assert len(fs.cat("k")) == 5

    def test_is_subclass(self):
        assert issubclass(MultiReferenceFileSystem, ReferenceFileSystem)


# ---------------------------------------------------------------------------
# Template expansion for multi-range entries
# ---------------------------------------------------------------------------


class TestTemplateExpansion:
    """Verify template expansion works for all reference types including multi-range."""

    def test_single_range_template_expanded(self, data_file):
        """Standard single-range refs with {{base}} are expanded via templates."""
        path, _ = data_file
        url = _file_url(path)
        # Split URL into base + filename
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": base},
            "refs": {"k": ["{{base}}" + filename, 0, 5]},
        }
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        result = fs.cat("k")
        assert len(result) == 5

    def test_multi_range_template_expanded(self, data_file):
        """Multi-range refs with {{base}} are expanded via templates."""
        path, content = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": base},
            "refs": {
                "k": ["{{base}}" + filename, [[0, 5], [10, 3]]],
            },
        }
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        result = fs.cat("k")
        expected = content[0:5] + content[10:13]
        assert result == expected

    def test_template_overrides_applied_to_multi_range(self, data_file):
        """template_overrides replaces template values for multi-range refs."""
        path, content = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": ""},
            "refs": {
                "k": ["{{base}}" + filename, [[0, 5], [10, 3]]],
            },
        }
        fs = MultiReferenceFileSystem(
            fo=fo, skip_instance_cache=True,
            template_overrides={"base": base},
        )
        result = fs.cat("k")
        expected = content[0:5] + content[10:13]
        assert result == expected

    def test_template_overrides_applied_to_single_range(self, data_file):
        """template_overrides replaces template values for single-range refs."""
        path, _ = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": ""},
            "refs": {"k": ["{{base}}" + filename, 0, 5]},
        }
        fs = MultiReferenceFileSystem(
            fo=fo, skip_instance_cache=True,
            template_overrides={"base": base},
        )
        result = fs.cat("k")
        assert len(result) == 5

    def test_no_templates_multi_range_unchanged(self, data_file):
        """Multi-range refs without templates work as before."""
        path, content = data_file
        url = _file_url(path)

        fo = {
            "version": 1,
            "refs": {
                "k": [url, [[0, 5], [10, 3]]],
            },
        }
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        result = fs.cat("k")
        expected = content[0:5] + content[10:13]
        assert result == expected

    def test_mixed_template_refs(self, data_file):
        """Mix of single-range, multi-range, and inline refs all resolve correctly."""
        path, content = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": ""},
            "refs": {
                "inline": "hello",
                "single": ["{{base}}" + filename, 0, 5],
                "multi": ["{{base}}" + filename, [[0, 3], [5, 2]]],
            },
        }
        fs = MultiReferenceFileSystem(
            fo=fo, skip_instance_cache=True,
            template_overrides={"base": base},
        )

        assert fs.cat("inline") == b"hello"
        assert len(fs.cat("single")) == 5
        expected_multi = content[0:3] + content[5:7]
        assert fs.cat("multi") == expected_multi
