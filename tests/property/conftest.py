"""Shared fixtures and pytest configuration for property-based tests.

This module provides:
- temp_nitf_path fixture for temporary file handling with cleanup
- hypothesis_settings fixture with appropriate settings for I/O-bound tests
- pytest marker registration for 'property' tests
"""

import tempfile
from pathlib import Path

import pytest
from hypothesis import settings, Phase


def pytest_configure(config):
    """Register the 'property' marker for property-based tests."""
    config.addinivalue_line(
        "markers",
        "property: mark test as a property-based test (run with pytest -m property)"
    )


# Default hypothesis settings for I/O-bound property tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,  # Disable deadline for I/O operations
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)


@pytest.fixture
def temp_nitf_path():
    """Fixture providing a temporary NITF file path with cleanup.
    
    Yields a Path object pointing to a temporary .ntf file.
    The file is automatically deleted after the test completes.
    """
    with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
        path = Path(f.name)
    yield path
    if path.exists():
        path.unlink()


@pytest.fixture
def hypothesis_settings():
    """Default hypothesis settings for I/O-bound tests.
    
    Returns a dict with:
    - max_examples: 100 (sufficient coverage without excessive runtime)
    - deadline: None (I/O operations can be slow)
    """
    return {"max_examples": 100, "deadline": None}
