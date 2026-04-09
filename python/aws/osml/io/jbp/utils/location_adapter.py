"""LocationAdapter — parse and format 10-character NITF location fields."""

from typing import Tuple


class LocationAdapter:
    """Converts between 10-character NITF RRRRRCCCCC location strings and (row, col) tuples."""

    @staticmethod
    def parse(loc: str) -> Tuple[int, int]:
        """Parse a 10-char RRRRRCCCCC string into (row, column).

        Raises ValueError if length is not 10 or contains non-numeric chars.
        """
        if len(loc) != 10:
            raise ValueError(f"Location string must be 10 characters, got {len(loc)}")
        if not loc.isdigit():
            raise ValueError(f"Location string must contain only digits, got {loc!r}")
        row = int(loc[:5])
        col = int(loc[5:])
        return (row, col)

    @staticmethod
    def format(row: int, col: int) -> str:
        """Format (row, column) into a 10-char RRRRRCCCCC string.

        Zero-pads both values to 5 digits.
        """
        return f"{row:05d}{col:05d}"
