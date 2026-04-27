"""SecurityClassificationAdapter — extract and produce NITF security classification blocks."""

from dataclasses import dataclass

# The 16 security fields in order (suffix after prefix).
SECURITY_FIELDS = [
    "CLAS",
    "CLSY",
    "CODE",
    "CTLH",
    "REL",
    "DCTP",
    "DCDT",
    "DCXM",
    "DG",
    "DGDT",
    "CLTX",
    "CATP",
    "CAUT",
    "CRSN",
    "SRDT",
    "CTLN",
]

# Mapping from logical prefix to the actual field name prefix used in NITF headers.
PREFIX_MAP = {
    "FS": "FS",
    "IS": "IS",
    "TS": "TS",
    "SS": "SS",
    "DES": "DES",
}


@dataclass
class SecurityClassification:
    """Prefix-agnostic NITF security classification block."""

    clas: str = "U"
    clsy: str = ""
    code: str = ""
    ctlh: str = ""
    rel: str = ""
    dctp: str = ""
    dcdt: str = ""
    dcxm: str = ""
    dg: str = ""
    dgdt: str = ""
    cltx: str = ""
    catp: str = ""
    caut: str = ""
    crsn: str = ""
    srdt: str = ""
    ctln: str = ""


class SecurityClassificationAdapter:
    """Converts between prefixed NITF security metadata dicts and SecurityClassification."""

    @staticmethod
    def extract(metadata: dict, prefix: str) -> SecurityClassification:
        """Extract security fields from a metadata dict using the given prefix.

        Supports prefixes: "FS", "IS", "TS", "SS", "DES".
        Missing fields use NITF defaults ("U" for CLAS, "" for others).
        """
        actual_prefix = PREFIX_MAP[prefix]
        kwargs = {}
        for suffix in SECURITY_FIELDS:
            key = actual_prefix + suffix
            attr = suffix.lower()
            default = "U" if suffix == "CLAS" else ""
            kwargs[attr] = metadata.get(key, default)
        return SecurityClassification(**kwargs)

    @staticmethod
    def to_dict(sec: SecurityClassification, prefix: str) -> dict:
        """Convert a SecurityClassification to a prefixed metadata dict.

        Returns a dict suitable for setting on BufferedMetadataProvider.
        """
        actual_prefix = PREFIX_MAP[prefix]
        result = {}
        for suffix in SECURITY_FIELDS:
            key = actual_prefix + suffix
            attr = suffix.lower()
            result[key] = getattr(sec, attr)
        return result
