"""TGTIDAdapter — parse and format 17-character NITF TGTID strings."""

from dataclasses import dataclass


@dataclass(frozen=True)
class TGTID:
    """Parsed NITF target identifier."""

    be_number: str  # 10 characters
    osuffix: str  # 5 characters
    country: str  # 2 characters


class TGTIDAdapter:
    """Converts between 17-character NITF TGTID strings and structured TGTID objects."""

    @staticmethod
    def parse(tgtid: str) -> TGTID:
        """Parse a 17-char TGTID string into components.

        Raises ValueError if length is not 17.
        """
        if len(tgtid) != 17:
            raise ValueError(f"TGTID must be 17 characters, got {len(tgtid)}")
        return TGTID(
            be_number=tgtid[0:10],
            osuffix=tgtid[10:15],
            country=tgtid[15:17],
        )

    @staticmethod
    def format(tgtid: TGTID) -> str:
        """Format a TGTID into a 17-char string.

        Concatenates be_number (10), osuffix (5), country (2).
        """
        return tgtid.be_number + tgtid.osuffix + tgtid.country
