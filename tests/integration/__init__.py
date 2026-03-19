"""Integration test infrastructure for manifest-driven validation.

This module provides data classes for managing YAML test manifests that define
expected validation outcomes for integration test files across all formats.
"""

import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import yaml

logger = logging.getLogger(__name__)


@dataclass
class IntegrationEntry:
    """Single entry in the integration manifest.

    Attributes:
        path: Relative file path from the integration data directory
        label: Brief human-readable title for the entry
        tags: Arbitrary strings for filtering and categorization
        description: Human-readable documentation of the test case
        expected_exception: Expected exception type name for negative tests
        expected_message: Expected substring in error message for negative tests
    """

    __test__ = False

    path: str
    label: Optional[str] = None
    tags: list[str] = field(default_factory=list)
    description: Optional[str] = None
    expected_exception: Optional[str] = None
    expected_message: Optional[str] = None

    @property
    def expected_valid(self) -> bool:
        """True when no expected_exception and no expected_message."""
        return self.expected_exception is None and self.expected_message is None


@dataclass
class IntegrationManifest:
    """Collection of IntegrationEntry loaded from YAML.

    Attributes:
        entries: List of IntegrationEntry objects
        base_path: Base directory for resolving relative file paths
    """

    __test__ = False

    entries: list[IntegrationEntry] = field(default_factory=list)
    base_path: Path = field(default_factory=lambda: Path("."))

    @classmethod
    def load(cls, manifest_path: Path, base_path: Path) -> "IntegrationManifest":
        """Load manifest from YAML file.

        Returns empty manifest on missing or unparseable file.

        Args:
            manifest_path: Path to the manifest YAML file
            base_path: Base directory for resolving relative file paths

        Returns:
            IntegrationManifest with loaded entries
        """
        if not manifest_path.exists():
            logger.warning("Manifest file not found: %s", manifest_path)
            return cls(entries=[], base_path=base_path)

        try:
            with open(manifest_path, "r") as f:
                data = yaml.safe_load(f)
        except (yaml.YAMLError, OSError) as e:
            logger.warning("Failed to parse manifest %s: %s", manifest_path, e)
            return cls(entries=[], base_path=base_path)

        if not isinstance(data, dict):
            logger.warning("Manifest is not a YAML mapping: %s", manifest_path)
            return cls(entries=[], base_path=base_path)

        entries = []
        for entry_data in data.get("entries", []):
            if not isinstance(entry_data, dict) or "path" not in entry_data:
                logger.warning("Skipping manifest entry missing 'path': %s", entry_data)
                continue
            tags = entry_data.get("tags", [])
            if not isinstance(tags, list):
                tags = []
            entry = IntegrationEntry(
                path=entry_data["path"],
                label=entry_data.get("label"),
                tags=tags,
                description=entry_data.get("description"),
                expected_exception=entry_data.get("expected_exception"),
                expected_message=entry_data.get("expected_message"),
            )
            entries.append(entry)

        return cls(entries=entries, base_path=base_path)

    def entries_by_tag(self, tag: str) -> list[IntegrationEntry]:
        """Filter entries that contain the given tag.

        Args:
            tag: Tag string to filter by

        Returns:
            List of entries whose tags list contains the tag
        """
        return [entry for entry in self.entries if tag in entry.tags]

    def get_entry(self, path: str) -> Optional[IntegrationEntry]:
        """Look up entry by path.

        Args:
            path: Relative path to look up

        Returns:
            IntegrationEntry if found, None otherwise
        """
        for entry in self.entries:
            if entry.path == path:
                return entry
        return None
