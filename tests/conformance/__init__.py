"""Conformance test infrastructure for JBP validation.

This module provides data classes for managing test manifests that define
expected validation outcomes for NITF conformance test files.
"""

import json
import logging
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Optional

logger = logging.getLogger(__name__)


@dataclass
class ConformanceEntry:
    """Single test file entry from manifest.

    Attributes:
        path: Relative path to test file from the test data directory
        expected_valid: True if validation should pass without error
        expected_exception: Expected exception type name (e.g., "ValueError")
        expected_message: Substring expected in error message
        category: Optional category for filtering tests (e.g., "format", "security")
        description: Human-readable description of the test case
    """
    __test__ = False  # Prevent pytest from collecting this as a test class

    path: str
    expected_valid: bool
    expected_exception: Optional[str] = None
    expected_message: Optional[str] = None
    category: Optional[str] = None
    description: Optional[str] = None


# Alias for backward compatibility with design doc naming
TestFileEntry = ConformanceEntry


@dataclass
class ConformanceManifest:
    """Collection of test file entries loaded from a JSON manifest.

    Attributes:
        entries: List of ConformanceEntry objects
        base_path: Base directory for resolving relative file paths
    """
    __test__ = False  # Prevent pytest from collecting this as a test class

    entries: list[ConformanceEntry] = field(default_factory=list)
    base_path: Path = field(default_factory=lambda: Path("."))

    @classmethod
    def load(cls, manifest_path: Path, base_path: Path) -> "ConformanceManifest":
        """Load manifest from JSON file.

        Args:
            manifest_path: Path to the manifest JSON file
            base_path: Base directory for resolving relative file paths

        Returns:
            ConformanceManifest with loaded entries, or empty manifest if file not found

        Raises:
            json.JSONDecodeError: If the file contains invalid JSON
            KeyError: If required fields are missing from entries
        """
        if not manifest_path.exists():
            logger.warning(f"Manifest file not found: {manifest_path}")
            return cls(entries=[], base_path=base_path)

        with open(manifest_path, "r") as f:
            data = json.load(f)

        entries = []
        for entry_data in data.get("entries", []):
            entry = ConformanceEntry(
                path=entry_data["path"],
                expected_valid=entry_data["expected_valid"],
                expected_exception=entry_data.get("expected_exception"),
                expected_message=entry_data.get("expected_message"),
                category=entry_data.get("category"),
                description=entry_data.get("description"),
            )
            entries.append(entry)

        return cls(entries=entries, base_path=base_path)

    def to_json(self) -> str:
        """Serialize manifest to JSON string.

        Returns:
            JSON string representation of the manifest
        """
        data = {
            "entries": [asdict(entry) for entry in self.entries]
        }
        return json.dumps(data, indent=2)

    @classmethod
    def from_json(cls, json_str: str, base_path: Path) -> "ConformanceManifest":
        """Deserialize manifest from JSON string.

        Args:
            json_str: JSON string containing manifest data
            base_path: Base directory for resolving relative file paths

        Returns:
            ConformanceManifest with loaded entries

        Raises:
            json.JSONDecodeError: If the string contains invalid JSON
            KeyError: If required fields are missing from entries
        """
        data = json.loads(json_str)

        entries = []
        for entry_data in data.get("entries", []):
            entry = ConformanceEntry(
                path=entry_data["path"],
                expected_valid=entry_data["expected_valid"],
                expected_exception=entry_data.get("expected_exception"),
                expected_message=entry_data.get("expected_message"),
                category=entry_data.get("category"),
                description=entry_data.get("description"),
            )
            entries.append(entry)

        return cls(entries=entries, base_path=base_path)

    def get_entry(self, path: str) -> Optional[ConformanceEntry]:
        """Look up entry by path.

        Args:
            path: Relative path to look up

        Returns:
            ConformanceEntry if found, None otherwise
        """
        for entry in self.entries:
            if entry.path == path:
                return entry
        return None

    def entries_by_category(self, category: str) -> list[ConformanceEntry]:
        """Filter entries by category.

        Args:
            category: Category string to filter by

        Returns:
            List of entries matching the category
        """
        return [entry for entry in self.entries if entry.category == category]



# Alias for backward compatibility with design doc naming
TestManifest = ConformanceManifest
