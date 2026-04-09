"""DateTimeAdapter — parse and format 14-character NITF FDT strings."""

from dataclasses import dataclass
from datetime import datetime
from typing import Optional, Union


@dataclass(frozen=True)
class NitfDateTime:
    """Parsed NITF datetime with optional components for unknown values."""

    year: int
    month: Optional[int] = None
    day: Optional[int] = None
    hour: Optional[int] = None
    minute: Optional[int] = None
    second: Optional[int] = None


class DateTimeAdapter:
    """Converts between 14-character NITF FDT strings and Python datetime objects."""

    # Component definitions: (name, start, end, min_val, max_val)
    _COMPONENTS = [
        ("year", 0, 4, 0, 9999),
        ("month", 4, 6, 1, 12),
        ("day", 6, 8, 1, 31),
        ("hour", 8, 10, 0, 23),
        ("minute", 10, 12, 0, 59),
        ("second", 12, 14, 0, 59),
    ]

    @staticmethod
    def parse(fdt: str) -> Union[datetime, NitfDateTime]:
        """Parse a 14-char FDT string into datetime or NitfDateTime.

        Returns datetime.datetime when all components are known,
        NitfDateTime when any component is "--".

        Raises ValueError for invalid length or out-of-range components.
        """
        if len(fdt) != 14:
            raise ValueError(
                f"FDT string must be 14 characters, got {len(fdt)}"
            )

        values: dict[str, Optional[int]] = {}
        has_unknown = False

        for name, start, end, lo, hi in DateTimeAdapter._COMPONENTS:
            raw = fdt[start:end]
            if raw == "--":
                if name == "year":
                    raise ValueError(
                        f"Year component cannot be unknown ('--')"
                    )
                values[name] = None
                has_unknown = True
            else:
                if not raw.isdigit():
                    raise ValueError(
                        f"Invalid characters in {name} component: '{raw}'"
                    )
                val = int(raw)
                if val < lo or val > hi:
                    raise ValueError(
                        f"{name} value {val} out of range {lo}–{hi}"
                    )
                values[name] = val

        if has_unknown:
            return NitfDateTime(
                year=values["year"],  # type: ignore[arg-type]
                month=values["month"],
                day=values["day"],
                hour=values["hour"],
                minute=values["minute"],
                second=values["second"],
            )

        return datetime(
            year=values["year"],  # type: ignore[arg-type]
            month=values["month"],  # type: ignore[arg-type]
            day=values["day"],  # type: ignore[arg-type]
            hour=values["hour"],  # type: ignore[arg-type]
            minute=values["minute"],  # type: ignore[arg-type]
            second=values["second"],  # type: ignore[arg-type]
        )

    @staticmethod
    def format(dt: Union[datetime, NitfDateTime]) -> str:
        """Format a datetime or NitfDateTime into a 14-char FDT string.

        NitfDateTime with None components produces "--" in those positions.
        Always returns exactly 14 characters.
        """
        if isinstance(dt, datetime):
            return (
                f"{dt.year:04d}{dt.month:02d}{dt.day:02d}"
                f"{dt.hour:02d}{dt.minute:02d}{dt.second:02d}"
            )

        # NitfDateTime
        parts = [f"{dt.year:04d}"]
        for field in ("month", "day", "hour", "minute", "second"):
            val = getattr(dt, field)
            parts.append("--" if val is None else f"{val:02d}")
        return "".join(parts)
