"""Spec-fidelity anchor for RSMGGA grid-point coordinate sizing.

RSMGGA's grid-point coordinate fields ``RCOORD``/``CCOORD`` are variable-width:
per STDI-0002 Vol 1, App U (Table 12) their width is the *total number of
coordinate digits* — ``TNUMRD`` for rows, ``TNUMCD`` for columns (range 3-11) —
not the *fractional* digit counts ``FNUMRD``/``FNUMCD`` (range 1-3). They are
encoded BCS-A (may be all spaces when a coordinate is unavailable).

The property round-trip suite now populates ``GRID_POINTS`` (the generator gate
keys on flat-map membership, and the codec resolves the nested ``grid_point_t``
against the flat type map); these tests remain the dedicated *spec-fidelity*
coverage, asserting ``RCOORD``/``CCOORD`` widths equal ``TNUMRD``/``TNUMCD``.

Spec: STDI-0002 Vol 1, Appendix U -- RSMGGA Table 12 (p. U-217).
"""

from pathlib import Path

import pytest
from aws.osml.io import StructureRegistry

STRUCTURES_DIR = Path("data/structures")


@pytest.fixture
def registry():
    reg = StructureRegistry()
    reg.add_search_path(str(STRUCTURES_DIR))
    return reg


def _rsmgga_record(tnumrd: int, tnumcd: int, npln: int = 1) -> dict:
    """An ``npln``-plane, 2-grid-point-per-plane RSMGGA value dict.

    The grid-point coordinate strings are exactly ``tnumrd``/``tnumcd`` wide so
    a faithful round trip requires the field width to track ``TNUMRD``/``TNUMCD``.

    Per STDI-0002 Vol 1, App U (Table 12) the ``DELTA_ORIGIN`` (IXO/IYO) block
    is emitted for grid planes 2..NPLN — i.e. ``npln - 1`` entries — while the
    ``PLANES`` block has one record per plane (``npln`` entries).
    """
    return {
        "IID": "I" * 80,
        "EDITION": "E" * 40,
        "GGRSN": "001",
        "GGCSN": "001",
        "GGRFEP": "0" * 21,
        "GGCFEP": "0" * 21,
        "INTORD": "1",
        "NPLN": str(npln).zfill(3),
        "DELTAZ": "0" * 21,
        "DELTAX": "0" * 21,
        "DELTAY": "0" * 21,
        "ZPLN1": "0" * 21,
        "XIPLN1": "0" * 21,
        "YIPLN1": "0" * 21,
        "REFROW": "0" * 9,
        "REFCOL": "0" * 9,
        "TNUMRD": str(tnumrd).zfill(2),
        "TNUMCD": str(tnumcd).zfill(2),
        "FNUMRD": "1",
        "FNUMCD": "1",
        # NPLN-1 delta-origin entries (grid planes 2..NPLN).
        "DELTA_ORIGIN": [
            {"IXO": "0000", "IYO": "0000"} for _ in range(npln - 1)
        ],
        "PLANES": [
            {
                "NXPTS": "001",
                "NYPTS": "002",
                "GRID_POINTS": [
                    {"RCOORD": "1" * tnumrd, "CCOORD": "2" * tnumcd},
                    {"RCOORD": "3" * tnumrd, "CCOORD": "4" * tnumcd},
                ],
            }
            for _ in range(npln)
        ],
    }


class TestRsmggaGridPointSizing:
    """Grid-point coordinate widths must equal TNUMRD/TNUMCD (not FNUMRD/FNUMCD)."""

    @pytest.mark.parametrize(
        "tnumrd,tnumcd",
        [(3, 3), (5, 6), (11, 11)],  # spec range 3-11, asymmetric widths
    )
    def test_grid_point_widths_track_tnumrd_tnumcd(self, registry, tnumrd, tnumcd):
        """Decoded RCOORD/CCOORD widths equal TNUMRD/TNUMCD across a populated grid."""
        defn = registry.get("tre_rsmgga")
        assert defn is not None, "tre_rsmgga definition not found"

        values = _rsmgga_record(tnumrd, tnumcd)
        raw = defn.encode(values)
        decoded = defn.decode(raw)

        grid_points = decoded["PLANES"][0]["GRID_POINTS"]
        assert len(grid_points) == 2
        for point in grid_points:
            assert len(point["RCOORD"]) == tnumrd
            assert len(point["CCOORD"]) == tnumcd

    def test_populated_grid_round_trips(self, registry):
        """A populated PLANES/GRID_POINTS grid encodes/decodes faithfully."""
        defn = registry.get("tre_rsmgga")
        assert defn is not None

        values = _rsmgga_record(5, 6)
        raw = defn.encode(values)
        decoded = defn.decode(raw)

        # encode(decode(bytes)) == bytes — a faithful round trip.
        assert defn.encode(decoded) == raw

        grid_points = decoded["PLANES"][0]["GRID_POINTS"]
        assert grid_points[0]["RCOORD"] == "1" * 5
        assert grid_points[0]["CCOORD"] == "2" * 6
        assert grid_points[1]["RCOORD"] == "3" * 5
        assert grid_points[1]["CCOORD"] == "4" * 6

    def test_unavailable_coordinate_all_spaces(self, registry):
        """BCS-A coordinates may be all spaces (coordinate unavailable)."""
        defn = registry.get("tre_rsmgga")
        assert defn is not None

        values = _rsmgga_record(5, 6)
        values["PLANES"][0]["GRID_POINTS"][0] = {
            "RCOORD": " " * 5,
            "CCOORD": " " * 6,
        }
        raw = defn.encode(values)
        decoded = defn.decode(raw)

        first = decoded["PLANES"][0]["GRID_POINTS"][0]
        assert first["RCOORD"] == " " * 5
        assert first["CCOORD"] == " " * 6
        assert defn.encode(decoded) == raw


class TestRsmggaDeltaOriginCount:
    """DELTA_ORIGIN must have NPLN-1 entries (grid planes 2..NPLN), not NPLN.

    Per STDI-0002 Vol 1, App U (Table 12, p. U-217) the IXO/IYO grid-plane
    origin offsets are emitted "for grid plane 2 through the total number of
    grid planes" — plane 1 is the reference plane and carries no offset. The
    pre-fix ``repeat-expr: NPLN.to_i`` emitted one extra entry (8 bytes) and
    desynchronized the PLANES section. These assertions fail against the
    pre-fix definition.
    """

    @pytest.mark.parametrize(
        "npln,expected_origins",
        [(1, 0), (2, 1), (5, 4)],
    )
    def test_delta_origin_count_is_npln_minus_one(
        self, registry, npln, expected_origins
    ):
        """Decoded DELTA_ORIGIN count equals NPLN-1 across plane counts."""
        defn = registry.get("tre_rsmgga")
        assert defn is not None

        values = _rsmgga_record(5, 6, npln=npln)
        # Sanity: the fixture itself supplies NPLN-1 origins.
        assert len(values["DELTA_ORIGIN"]) == expected_origins

        raw = defn.encode(values)
        decoded = defn.decode(raw)

        assert len(decoded["DELTA_ORIGIN"]) == expected_origins
        assert len(decoded["PLANES"]) == npln
        # Faithful round trip with the corrected count.
        assert defn.encode(decoded) == raw
