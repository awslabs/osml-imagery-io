"""Unit tests for TestFileEntry and TestManifest classes."""

import json
import pytest
from pathlib import Path

from tests.conformance import TestFileEntry, TestManifest


class TestTestFileEntry:
    """Tests for TestFileEntry dataclass."""

    def test_required_fields_only(self):
        """Entry can be created with only required fields."""
        entry = TestFileEntry(path="test.ntf", expected_valid=True)
        assert entry.path == "test.ntf"
        assert entry.expected_valid is True
        assert entry.expected_exception is None
        assert entry.expected_message is None
        assert entry.category is None
        assert entry.description is None

    def test_all_fields(self):
        """Entry can be created with all fields."""
        entry = TestFileEntry(
            path="invalid.ntf",
            expected_valid=False,
            expected_exception="ValueError",
            expected_message="Invalid magic",
            category="format",
            description="Test invalid magic number",
        )
        assert entry.path == "invalid.ntf"
        assert entry.expected_valid is False
        assert entry.expected_exception == "ValueError"
        assert entry.expected_message == "Invalid magic"
        assert entry.category == "format"
        assert entry.description == "Test invalid magic number"


class TestTestManifest:
    """Tests for TestManifest class."""

    def test_load_missing_file(self, tmp_path):
        """Empty manifest returned when file doesn't exist."""
        manifest = TestManifest.load(tmp_path / "nonexistent.json", tmp_path)
        assert len(manifest.entries) == 0
        assert manifest.base_path == tmp_path

    def test_load_valid_json(self, tmp_path):
        """Manifest loads entries from valid JSON."""
        manifest_file = tmp_path / "manifest.json"
        manifest_file.write_text(
            json.dumps({
                "entries": [
                    {"path": "test.ntf", "expected_valid": True},
                    {
                        "path": "invalid.ntf",
                        "expected_valid": False,
                        "expected_exception": "ValueError",
                        "expected_message": "bad format",
                        "category": "format",
                        "description": "Invalid file",
                    },
                ]
            })
        )
        manifest = TestManifest.load(manifest_file, tmp_path)
        assert len(manifest.entries) == 2
        assert manifest.entries[0].path == "test.ntf"
        assert manifest.entries[0].expected_valid is True
        assert manifest.entries[1].path == "invalid.ntf"
        assert manifest.entries[1].expected_exception == "ValueError"

    def test_load_invalid_json(self, tmp_path):
        """JSON parse error raised for invalid JSON."""
        manifest_file = tmp_path / "manifest.json"
        manifest_file.write_text("{ invalid json }")
        with pytest.raises(json.JSONDecodeError):
            TestManifest.load(manifest_file, tmp_path)

    def test_load_missing_required_field(self, tmp_path):
        """KeyError raised when required field is missing."""
        manifest_file = tmp_path / "manifest.json"
        manifest_file.write_text(
            json.dumps({"entries": [{"path": "test.ntf"}]})  # missing expected_valid
        )
        with pytest.raises(KeyError):
            TestManifest.load(manifest_file, tmp_path)

    def test_get_entry_found(self):
        """Lookup returns entry when path exists."""
        entry = TestFileEntry(path="test.ntf", expected_valid=True)
        manifest = TestManifest(entries=[entry], base_path=Path("."))
        found = manifest.get_entry("test.ntf")
        assert found is not None
        assert found.path == "test.ntf"
        assert found.expected_valid is True

    def test_get_entry_not_found(self):
        """Lookup returns None when path doesn't exist."""
        manifest = TestManifest(entries=[], base_path=Path("."))
        assert manifest.get_entry("missing.ntf") is None

    def test_entries_by_category(self):
        """Filter returns only entries matching category."""
        entries = [
            TestFileEntry(path="a.ntf", expected_valid=True, category="format"),
            TestFileEntry(path="b.ntf", expected_valid=True, category="security"),
            TestFileEntry(path="c.ntf", expected_valid=False, category="format"),
        ]
        manifest = TestManifest(entries=entries, base_path=Path("."))
        format_entries = manifest.entries_by_category("format")
        assert len(format_entries) == 2
        assert all(e.category == "format" for e in format_entries)

    def test_to_json_and_from_json_roundtrip(self):
        """Manifest survives JSON serialization round-trip."""
        entries = [
            TestFileEntry(
                path="test.ntf",
                expected_valid=True,
                category="format",
                description="Valid file",
            ),
            TestFileEntry(
                path="invalid.ntf",
                expected_valid=False,
                expected_exception="ValueError",
                expected_message="bad",
            ),
        ]
        manifest = TestManifest(entries=entries, base_path=Path("."))
        json_str = manifest.to_json()
        loaded = TestManifest.from_json(json_str, Path("."))
        
        assert len(loaded.entries) == 2
        assert loaded.entries[0].path == "test.ntf"
        assert loaded.entries[0].expected_valid is True
        assert loaded.entries[0].category == "format"
        assert loaded.entries[1].expected_exception == "ValueError"
