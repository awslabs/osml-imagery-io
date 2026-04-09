"""IGEOLOAdapter — parse and format 60-character NITF IGEOLO strings."""

from dataclasses import dataclass
from typing import List, Tuple


@dataclass(frozen=True)
class UTMCoordinate:
    """UTM coordinate with zone, easting, and northing."""

    zone: int
    easting: int
    northing: int


class IGEOLOAdapter:
    """Converts between 60-character NITF IGEOLO strings and structured coordinates."""

    @staticmethod
    def parse(igeolo: str, icords: str) -> list:
        """Parse a 60-char IGEOLO string based on ICORDS value.

        ICORDS="G": Returns List[Tuple[float, float]] — 4 (lat, lon) in decimal degrees
        ICORDS="D": Returns List[Tuple[float, float]] — 4 (lat, lon) in decimal degrees
        ICORDS="N"/"S": Returns List[UTMCoordinate] — 4 UTM coordinates
        ICORDS="U": Returns List[str] — 4 MGRS coordinate strings

        Raises ValueError for invalid IGEOLO length, bad hemisphere indicators,
        or non-numeric characters where digits are expected.
        """
        if len(igeolo) != 60:
            raise ValueError(
                f"IGEOLO must be exactly 60 characters, got {len(igeolo)}"
            )

        if icords == "G":
            return IGEOLOAdapter._parse_geographic(igeolo)
        elif icords == "D":
            return IGEOLOAdapter._parse_decimal(igeolo)
        elif icords in ("N", "S"):
            return IGEOLOAdapter._parse_utm(igeolo)
        elif icords == "U":
            return IGEOLOAdapter._parse_mgrs(igeolo)
        else:
            raise ValueError(f"Unsupported ICORDS value: {icords!r}")

    @staticmethod
    def format(coords: list, icords: str) -> str:
        """Format coordinates into a 60-char IGEOLO string.

        ICORDS="G": coords is List[Tuple[float, float]] (lat, lon) in decimal degrees
        ICORDS="D": coords is List[Tuple[float, float]] (lat, lon) in decimal degrees
        ICORDS="N"/"S": coords is List[UTMCoordinate]
        ICORDS="U": coords is List[str] (MGRS strings, padded to 15 chars)

        Always returns exactly 60 characters.
        """
        if icords == "G":
            return IGEOLOAdapter._format_geographic(coords)
        elif icords == "D":
            return IGEOLOAdapter._format_decimal(coords)
        elif icords in ("N", "S"):
            return IGEOLOAdapter._format_utm(coords)
        elif icords == "U":
            return IGEOLOAdapter._format_mgrs(coords)
        else:
            raise ValueError(f"Unsupported ICORDS value: {icords!r}")

    # ── Geographic (ICORDS=G) ──────────────────────────────────────────

    @staticmethod
    def _parse_geographic(igeolo: str) -> List[Tuple[float, float]]:
        """Parse ddmmssXdddmmssY × 4 corners into (lat, lon) decimal degrees."""
        coords: List[Tuple[float, float]] = []
        for i in range(4):
            corner = igeolo[i * 15 : (i + 1) * 15]

            # Latitude: ddmmssX (7 chars)
            lat_dd_str = corner[0:2]
            lat_mm_str = corner[2:4]
            lat_ss_str = corner[4:6]
            lat_hem = corner[6]

            # Longitude: dddmmssY (8 chars)
            lon_ddd_str = corner[7:10]
            lon_mm_str = corner[10:12]
            lon_ss_str = corner[12:14]
            lon_hem = corner[14]

            # Validate hemisphere indicators
            if lat_hem not in ("N", "S"):
                raise ValueError(
                    f"Invalid latitude hemisphere indicator: {lat_hem!r}"
                )
            if lon_hem not in ("E", "W"):
                raise ValueError(
                    f"Invalid longitude hemisphere indicator: {lon_hem!r}"
                )

            # Validate and parse numeric components
            try:
                lat_dd = int(lat_dd_str)
                lat_mm = int(lat_mm_str)
                lat_ss = int(lat_ss_str)
            except ValueError:
                raise ValueError(
                    f"Non-numeric characters in latitude: {corner[:7]!r}"
                )

            try:
                lon_ddd = int(lon_ddd_str)
                lon_mm = int(lon_mm_str)
                lon_ss = int(lon_ss_str)
            except ValueError:
                raise ValueError(
                    f"Non-numeric characters in longitude: {corner[7:]!r}"
                )

            # Convert to decimal degrees
            lat = lat_dd + lat_mm / 60.0 + lat_ss / 3600.0
            lon = lon_ddd + lon_mm / 60.0 + lon_ss / 3600.0

            # Apply hemisphere sign
            if lat_hem == "S":
                lat = -lat
            if lon_hem == "W":
                lon = -lon

            coords.append((lat, lon))
        return coords

    @staticmethod
    def _format_geographic(coords: List[Tuple[float, float]]) -> str:
        """Format (lat, lon) tuples into ddmmssXdddmmssY × 4 corners."""
        parts: list = []
        for lat, lon in coords:
            lat_hem = "N" if lat >= 0 else "S"
            lon_hem = "E" if lon >= 0 else "W"

            abs_lat = abs(lat)
            abs_lon = abs(lon)

            # Convert to total seconds then decompose to avoid float drift
            lat_total_ss = int(round(abs_lat * 3600))
            lat_dd = lat_total_ss // 3600
            lat_mm = (lat_total_ss % 3600) // 60
            lat_ss = lat_total_ss % 60

            lon_total_ss = int(round(abs_lon * 3600))
            lon_ddd = lon_total_ss // 3600
            lon_mm = (lon_total_ss % 3600) // 60
            lon_ss = lon_total_ss % 60

            part = (
                f"{lat_dd:02d}{lat_mm:02d}{lat_ss:02d}{lat_hem}"
                f"{lon_ddd:03d}{lon_mm:02d}{lon_ss:02d}{lon_hem}"
            )
            parts.append(part)
        return "".join(parts)

    # ── Decimal Degrees (ICORDS=D) ─────────────────────────────────────

    @staticmethod
    def _parse_decimal(igeolo: str) -> List[Tuple[float, float]]:
        """Parse ±dd.ddd±ddd.ddd × 4 corners into (lat, lon) float tuples."""
        coords: List[Tuple[float, float]] = []
        for i in range(4):
            corner = igeolo[i * 15 : (i + 1) * 15]
            # Latitude: ±dd.ddd (7 chars), Longitude: ±ddd.ddd (8 chars)
            lat_str = corner[0:7]
            lon_str = corner[7:15]
            try:
                lat = float(lat_str)
            except ValueError:
                raise ValueError(
                    f"Non-numeric latitude value: {lat_str!r}"
                )
            try:
                lon = float(lon_str)
            except ValueError:
                raise ValueError(
                    f"Non-numeric longitude value: {lon_str!r}"
                )
            coords.append((lat, lon))
        return coords

    @staticmethod
    def _format_decimal(coords: List[Tuple[float, float]]) -> str:
        """Format (lat, lon) tuples into ±dd.ddd±ddd.ddd × 4 corners."""
        parts: list = []
        for lat, lon in coords:
            lat_sign = "+" if lat >= 0 else "-"
            lon_sign = "+" if lon >= 0 else "-"
            abs_lat = abs(lat)
            abs_lon = abs(lon)
            lat_str = f"{lat_sign}{abs_lat:06.3f}"
            lon_str = f"{lon_sign}{abs_lon:07.3f}"
            parts.append(lat_str + lon_str)
        return "".join(parts)

    # ── UTM (ICORDS=N/S) ──────────────────────────────────────────────

    @staticmethod
    def _parse_utm(igeolo: str) -> List[UTMCoordinate]:
        """Parse zzeeeeeennnnnnn × 4 corners into UTMCoordinate objects."""
        coords: List[UTMCoordinate] = []
        for i in range(4):
            corner = igeolo[i * 15 : (i + 1) * 15]
            zone_str = corner[0:2]
            easting_str = corner[2:8]
            northing_str = corner[8:15]
            try:
                zone = int(zone_str)
                easting = int(easting_str)
                northing = int(northing_str)
            except ValueError:
                raise ValueError(
                    f"Non-numeric characters in UTM coordinate: {corner!r}"
                )
            coords.append(UTMCoordinate(zone=zone, easting=easting, northing=northing))
        return coords

    @staticmethod
    def _format_utm(coords: List[UTMCoordinate]) -> str:
        """Format UTMCoordinate objects into zzeeeeeennnnnnn × 4 corners."""
        parts: list = []
        for c in coords:
            part = f"{c.zone:02d}{c.easting:06d}{c.northing:07d}"
            parts.append(part)
        return "".join(parts)

    # ── MGRS (ICORDS=U) ───────────────────────────────────────────────

    @staticmethod
    def _parse_mgrs(igeolo: str) -> List[str]:
        """Parse 4 × 15-character MGRS strings, stripping trailing whitespace."""
        coords: List[str] = []
        for i in range(4):
            mgrs = igeolo[i * 15 : (i + 1) * 15].rstrip()
            coords.append(mgrs)
        return coords

    @staticmethod
    def _format_mgrs(coords: List[str]) -> str:
        """Pad each MGRS string to 15 chars (right-pad with spaces)."""
        parts: list = []
        for mgrs in coords:
            parts.append(mgrs.ljust(15))
        return "".join(parts)
