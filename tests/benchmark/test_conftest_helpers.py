"""Unit tests for benchmark conftest helper functions."""

from tests.benchmark.conftest import compute_access_patterns


class TestComputeAccessPatterns:
    """Tests for compute_access_patterns()."""

    def test_1x1_grid_produces_single_tile_and_small_roi_only(self):
        """A 1×1 grid should produce single_tile and small_roi, no large_roi."""
        patterns = compute_access_patterns(
            grid_rows=1, grid_cols=1, tile_h=256, tile_w=256,
            total_rows=256, total_cols=256,
        )
        names = [p["name"] for p in patterns]
        assert names == ["single_tile", "small_roi"]

    def test_20x20_grid_produces_all_three_patterns(self):
        """A 20×20 grid should produce single_tile, small_roi, and large_roi."""
        patterns = compute_access_patterns(
            grid_rows=20, grid_cols=20, tile_h=256, tile_w=256,
            total_rows=5120, total_cols=5120,
        )
        names = [p["name"] for p in patterns]
        assert names == ["single_tile", "small_roi", "large_roi"]

    def test_3x3_grid_correct_coordinates(self):
        """A 3×3 grid should produce correct coordinates for single_tile and small_roi."""
        patterns = compute_access_patterns(
            grid_rows=3, grid_cols=3, tile_h=100, tile_w=100,
            total_rows=300, total_cols=300,
        )
        by_name = {p["name"]: p for p in patterns}

        # Center tile: row=1, col=1 → (100, 200, 100, 200)
        assert by_name["single_tile"]["regions"] == [(100, 200, 100, 200)]

        # Small ROI: r0=0, r1=3, c0=0, c1=3 → (0, 300, 0, 300)
        assert by_name["small_roi"]["regions"] == [(0, 300, 0, 300)]

        assert "large_roi" not in by_name

    def test_large_roi_excluded_when_grid_below_10(self):
        """Grids smaller than 10×10 should not produce large_roi."""
        patterns = compute_access_patterns(
            grid_rows=9, grid_cols=9, tile_h=64, tile_w=64,
            total_rows=576, total_cols=576,
        )
        names = [p["name"] for p in patterns]
        assert "large_roi" not in names

    def test_large_roi_included_at_exactly_10x10(self):
        """A 10×10 grid should include large_roi."""
        patterns = compute_access_patterns(
            grid_rows=10, grid_cols=10, tile_h=64, tile_w=64,
            total_rows=640, total_cols=640,
        )
        names = [p["name"] for p in patterns]
        assert "large_roi" in names

    def test_all_regions_satisfy_bounds(self):
        """All region coordinates must satisfy 0 <= start < end <= total."""
        total_rows, total_cols = 2048, 2048
        patterns = compute_access_patterns(
            grid_rows=16, grid_cols=16, tile_h=128, tile_w=128,
            total_rows=total_rows, total_cols=total_cols,
        )
        for pattern in patterns:
            for rs, re, cs, ce in pattern["regions"]:
                assert 0 <= rs < re <= total_rows, f"{pattern['name']}: row bounds violated"
                assert 0 <= cs < ce <= total_cols, f"{pattern['name']}: col bounds violated"

    def test_single_tile_has_exactly_one_region(self):
        """single_tile pattern should always have exactly one region."""
        patterns = compute_access_patterns(
            grid_rows=5, grid_cols=5, tile_h=100, tile_w=100,
            total_rows=500, total_cols=500,
        )
        single = next(p for p in patterns if p["name"] == "single_tile")
        assert len(single["regions"]) == 1

    def test_total_smaller_than_grid_times_tile(self):
        """When total < grid * tile, region end should be capped at total."""
        patterns = compute_access_patterns(
            grid_rows=4, grid_cols=4, tile_h=256, tile_w=256,
            total_rows=900, total_cols=900,
        )
        for pattern in patterns:
            for rs, re, cs, ce in pattern["regions"]:
                assert re <= 900
                assert ce <= 900


from scripts.generate_benchmark_report import (
    _extract_access_pattern,
    generate_table,
)


class TestExtractAccessPattern:
    """Tests for _extract_access_pattern()."""

    def test_parses_single_tile_from_zarr_read_name(self):
        name = "test_bench_zarr_read[WV Pan J2K-single_tile-local]"
        assert _extract_access_pattern(name) == "single tile"

    def test_parses_small_roi(self):
        name = "test_bench_zarr_read[Synth Medium C3-small_roi-s3]"
        assert _extract_access_pattern(name) == "small roi"

    def test_parses_large_roi(self):
        name = "test_bench_zarr_read[Synth Large NC-large_roi-local]"
        assert _extract_access_pattern(name) == "large roi"

    def test_returns_none_for_block_read(self):
        name = "test_bench_block_read[Large NITF-UL]"
        assert _extract_access_pattern(name) is None

    def test_returns_none_for_metadata_read(self):
        name = "test_bench_metadata_read[Large NITF]"
        assert _extract_access_pattern(name) is None

    def test_returns_none_for_index_generation(self):
        name = "test_bench_index_generation[WV Pan J2K]"
        assert _extract_access_pattern(name) is None


def _make_entry(name: str, group: str = "block_read") -> dict:
    """Create a minimal benchmark entry for testing."""
    return {
        "name": name,
        "group": group,
        "stats": {
            "min": 0.001,
            "max": 0.005,
            "mean": 0.003,
            "median": 0.003,
            "stddev": 0.001,
            "rounds": 10,
        },
    }


class TestGenerateTable:
    """Tests for generate_table() with group_name parameter."""

    def test_tile_read_group_includes_access_pattern_column(self):
        entries = [
            _make_entry("test_bench_zarr_read[WV Pan J2K-single_tile-local]",
                        group="tile_read_zarr_local"),
        ]
        table = generate_table(entries, group_name="tile_read_zarr_local")
        lines = table.split("\n")
        assert "Access Pattern" in lines[0]
        assert "single tile" in lines[2]

    def test_tile_read_s3_group_includes_access_pattern_column(self):
        entries = [
            _make_entry("test_bench_zarr_read[Synth Medium C3-small_roi-s3]",
                        group="tile_read_zarr_s3"),
        ]
        table = generate_table(entries, group_name="tile_read_zarr_s3")
        lines = table.split("\n")
        assert "Access Pattern" in lines[0]
        assert "small roi" in lines[2]

    def test_tile_read_strips_dataset_label(self):
        """Dataset column should show only the dataset name, not pattern/backend."""
        entries = [
            _make_entry("test_bench_zarr_read[WV Pan J2K-single_tile-local]",
                        group="tile_read_zarr_local"),
        ]
        table = generate_table(entries, group_name="tile_read_zarr_local")
        data_row = table.split("\n")[2]
        assert "WV Pan J2K" in data_row
        assert "single_tile-local" not in data_row

    def test_non_tile_read_group_no_access_pattern_column(self):
        entries = [_make_entry("test_bench_metadata_read[Large NITF]", group="metadata")]
        table = generate_table(entries, group_name="metadata")
        lines = table.split("\n")
        assert "Access Pattern" not in lines[0]

    def test_block_read_group_preserves_original_format(self):
        entries = [_make_entry("test_bench_block_read[Large NITF-UL]", group="block_read")]
        table = generate_table(entries, group_name="block_read")
        lines = table.split("\n")
        # Original 8-column header
        assert lines[0].count("|") == 9  # 8 columns → 9 pipe chars
        assert "Access Pattern" not in lines[0]

    def test_default_group_name_preserves_original_format(self):
        """Calling without group_name should produce the original format."""
        entries = [_make_entry("test_bench_block_read[Large NITF-UL]")]
        table = generate_table(entries)
        assert "Access Pattern" not in table

    def test_tile_read_table_has_nine_columns(self):
        entries = [
            _make_entry("test_bench_zarr_read[WV Pan J2K-single_tile-local]",
                        group="tile_read_zarr_local"),
        ]
        table = generate_table(entries, group_name="tile_read_zarr_local")
        header = table.split("\n")[0]
        assert header.count("|") == 10  # 9 columns → 10 pipe chars
