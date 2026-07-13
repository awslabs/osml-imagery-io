"""Shared fixtures for JBP property tests.

The `registry` fixture is scoped at this level so all JBP property tests
(including the TRE suite) can use it without redeclaring it.
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

_STRUCTURES_DIR = Path("data/structures")


@pytest.fixture(scope="session")
def registry() -> StructureRegistry:
    """Return a StructureRegistry pre-loaded with all TRE definitions."""
    reg = StructureRegistry()
    reg.add_search_path(str(_STRUCTURES_DIR))
    return reg
