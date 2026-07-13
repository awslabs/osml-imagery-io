"""TRE-specific fixtures: path discovery only.

The `registry` fixture is inherited from the parent jbp/conftest.py.
"""

from pathlib import Path

ALL_TRE_PATHS = sorted(Path("data/structures/tre").glob("tre_*.ksy"))
ALL_TRE_IDS = [p.stem for p in ALL_TRE_PATHS]
