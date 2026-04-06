"""Tests for multi-range entry patching in generate_tile_index.py."""

import json
import sys
from pathlib import Path

# Add project root so the script module is importable
sys.path.insert(0, str(Path(__file__).parent.parent))

from scripts.generate_tile_index import _patch_multi_range_refs


class TestPatchMultiRangeRefs:
    """Tests for _patch_multi_range_refs."""

    def test_empty_multi_range_refs_returns_original(self):
        refs = {"key1": ["s3://bucket/file.ntf", 100, 200]}
        result = _patch_multi_range_refs(refs, {})
        assert result is refs  # same object, not a copy

    def test_none_multi_range_refs_returns_original(self):
        refs = {"key1": ["s3://bucket/file.ntf", 100, 200]}
        # Falsy dict
        result = _patch_multi_range_refs(refs, {})
        assert result is refs

    def test_patches_single_entry(self):
        refs = {
            "seg/0.0.0": ["s3://bucket/f.ntf", 100, 50],
            "seg/0.0.1": ["s3://bucket/f.ntf", 200, 50],
        }
        multi = {
            "seg/0.0.0": ["s3://bucket/f.ntf", [[100, 10], [500, 20], [900, 30]]],
        }
        result = _patch_multi_range_refs(refs, multi)
        # Multi-range entry replaced
        assert result["seg/0.0.0"] == ["s3://bucket/f.ntf", [[100, 10], [500, 20], [900, 30]]]
        # Single-range entry unchanged
        assert result["seg/0.0.1"] == ["s3://bucket/f.ntf", 200, 50]

    def test_does_not_mutate_original(self):
        refs = {"seg/0.0.0": ["s3://bucket/f.ntf", 100, 50]}
        multi = {"seg/0.0.0": ["s3://bucket/f.ntf", [[100, 10], [500, 20]]]}
        result = _patch_multi_range_refs(refs, multi)
        # Original unchanged
        assert refs["seg/0.0.0"] == ["s3://bucket/f.ntf", 100, 50]
        # Result patched
        assert result["seg/0.0.0"] == multi["seg/0.0.0"]

    def test_mixed_single_and_multi_range(self):
        refs = {
            "seg/0.0.0": ["s3://b/f", 10, 5],
            "seg/0.1.0": ["s3://b/f", 20, 5],
            "seg/0.0.1": ["s3://b/f", 30, 5],
        }
        multi = {
            "seg/0.0.0": ["s3://b/f", [[10, 2], [50, 3]]],
            "seg/0.0.1": ["s3://b/f", [[30, 1], [80, 4]]],
        }
        result = _patch_multi_range_refs(refs, multi)
        assert result["seg/0.0.0"] == multi["seg/0.0.0"]
        assert result["seg/0.1.0"] == ["s3://b/f", 20, 5]
        assert result["seg/0.0.1"] == multi["seg/0.0.1"]

    def test_json_serialization_format(self):
        """Verify multi-range entries serialize as ["url", [[o,l], ...]] in JSON."""
        refs = {
            "seg/0.0.0": ["s3://b/f", [[100, 10], [500, 20]]],
            "seg/0.0.1": ["s3://b/f", 200, 50],
        }
        serialized = json.dumps(refs)
        loaded = json.loads(serialized)
        # Multi-range: 2-element list, second element is list of lists
        entry = loaded["seg/0.0.0"]
        assert isinstance(entry, list)
        assert len(entry) == 2
        assert isinstance(entry[1], list)
        assert isinstance(entry[1][0], list)
        # Single-range: 3-element list
        entry_single = loaded["seg/0.0.1"]
        assert isinstance(entry_single, list)
        assert len(entry_single) == 3
        assert isinstance(entry_single[1], int)
