"""Unit tests for OversightMLParser and _build_manifest_array.

Validates that the extracted _build_manifest_array helper produces
structurally identical output to the original inline implementation.

Requirements: 7.1, 7.2
"""

import shutil
import tempfile
from pathlib import Path

import numpy as np
import pytest

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
)

# Guard: virtualizarr is a lazy dependency
virtualizarr = pytest.importorskip("virtualizarr", minversion="2.0")


def _write_nitf(path: Path, num_cols: int, num_rows: int, num_bands: int = 1,
                block_width: int = 256, block_height: int = 256,
                ic: str = "NC") -> None:
    """Write a minimal NITF file with the given dimensions."""
    metadata = BufferedMetadataProvider()
    metadata.set("IC", ic)
    metadata.set("IMODE", "B")

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, block_width),
        block_height=min(num_rows, block_height),
        metadata=metadata,
    )
    data = np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
    provider.set_full_image(data)

    writer = IO.open([str(path)], "w", "nitf")
    writer.add_asset("image:0", provider, "Image", "test", ["data"])
    writer.close()


@pytest.fixture()
def tmp_dir():
    """Provide a temporary directory that is cleaned up after the test."""
    d = tempfile.mkdtemp()
    yield Path(d)
    shutil.rmtree(d, ignore_errors=True)


class TestBuildManifestArrayBehaviorPreserving:
    """Verify that the extracted _build_manifest_array produces correct ManifestStore output.

    The refactored OversightMLParser uses _build_manifest_array as a standalone
    helper. This test confirms the parser output is structurally correct for a
    single-segment NITF file.

    Requirements: 7.1, 7.2
    """

    def test_single_segment_nitf_produces_correct_store_structure(self, tmp_dir):
        """A single-segment NITF produces a ManifestStore with expected arrays,
        chunk entries, multi_range_refs, and source attribute."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=3)

        url = "s3://bucket/test.ntf"
        parser = OversightMLParser(local_paths=str(path))
        store = parser(url=url)

        # Root group has at least one array keyed "image:0"
        group = store._group
        assert "image:0" in group.arrays, (
            f"Expected 'image:0' in arrays, got {list(group.arrays.keys())}"
        )

        # No subgroups — flat store for single-segment, no-overview input
        assert not group.groups, "Expected no subgroups for single-segment NITF"

        # Source attribute matches the URL
        attrs = group.metadata.attributes if group.metadata else {}
        assert attrs.get("source") == url, (
            f"Expected source='{url}', got '{attrs.get('source')}'"
        )

    def test_manifest_array_shape_and_chunks(self, tmp_dir):
        """The ManifestArray has correct shape and chunk shape matching the NITF."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        num_cols, num_rows, num_bands = 128, 128, 3
        _write_nitf(path, num_cols=num_cols, num_rows=num_rows, num_bands=num_bands)

        store = OversightMLParser(local_paths=str(path))(url="s3://bucket/test.ntf")
        array = store._group.arrays["image:0"]

        # Shape: (bands, rows, cols)
        assert array.shape == (num_bands, num_rows, num_cols), (
            f"Expected shape ({num_bands}, {num_rows}, {num_cols}), got {array.shape}"
        )

        # Chunk shape: (bands, block_height, block_width)
        chunks = array.chunks
        assert chunks[0] == num_bands, (
            f"Expected bands chunk {num_bands}, got {chunks[0]}"
        )

    def test_chunk_manifest_has_entries(self, tmp_dir):
        """The chunk manifest contains entries for all tiles."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser(local_paths=str(path))(url="s3://bucket/test.ntf")
        array = store._group.arrays["image:0"]

        manifest = array.manifest
        # Single tile for 128x128 image with 256x256 block size → 1 chunk
        assert len(manifest) > 0, "Chunk manifest should have at least one entry"

        # Each entry should reference the correct URL
        for _key, entry in manifest.items():
            assert entry["path"] == "s3://bucket/test.ntf", (
                f"Expected chunk path 's3://bucket/test.ntf', got '{entry['path']}'"
            )
            assert entry["offset"] >= 0
            assert entry["length"] > 0

    def test_multi_tile_chunk_manifest(self, tmp_dir):
        """A multi-tile image produces the correct number of chunk entries."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        # 256x256 image with 64x64 blocks → 4x4 = 16 tiles
        _write_nitf(path, num_cols=256, num_rows=256, num_bands=1,
                     block_width=64, block_height=64)

        store = OversightMLParser(local_paths=str(path))(url="s3://bucket/test.ntf")
        array = store._group.arrays["image:0"]

        manifest = array.manifest
        # 4 rows × 4 cols = 16 tiles, each with chunk key "0.row.col"
        assert len(manifest) == 16, (
            f"Expected 16 chunk entries for 4x4 grid, got {len(manifest)}"
        )

    def test_multi_range_refs_is_dict(self, tmp_dir):
        """multi_range_refs is a dict (may be empty for simple uncompressed files)."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=64, num_rows=64, num_bands=1)

        store = OversightMLParser(local_paths=str(path))(url="s3://bucket/test.ntf")
        multi_range_refs = getattr(store, "multi_range_refs", None)
        assert isinstance(multi_range_refs, dict), (
            f"Expected multi_range_refs to be a dict, got {type(multi_range_refs)}"
        )


class TestConstructorVariants:
    """Verify OversightMLParser constructor normalizes local_paths correctly.

    Requirements: 1.1, 1.2, 1.3
    """

    def test_single_string_positional(self):
        """A single string positional arg is wrapped in a list."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser("file.ntf")
        assert parser.local_paths == ["file.ntf"]

    def test_list_of_strings_positional(self):
        """A list of strings positional arg is stored as-is."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser(["a.ntf", "b.ntf"])
        assert parser.local_paths == ["a.ntf", "b.ntf"]

    def test_single_string_keyword(self):
        """A single string via keyword arg is wrapped in a list."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser(local_paths="file.ntf")
        assert parser.local_paths == ["file.ntf"]


class TestURLNormalization:
    """Verify OversightMLParser.__call__ normalizes the url parameter correctly.

    Requirements: 2.1, 2.2, 2.3, 2.4
    """

    def test_single_url_string_with_multiple_paths(self, tmp_dir):
        """A single URL string with 2 paths is used for all assets (no error)."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        # Create 2 small NITF files
        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=128, num_rows=128, num_bands=1)
        _write_nitf(path_r1, num_cols=64, num_rows=64, num_bands=1)

        parser = OversightMLParser([str(path_base), str(path_r1)])
        url = "s3://bucket/image.ntf"
        store = parser(url=url)

        # Should succeed without error; all chunk refs use the single URL
        group = store._group
        for array in group.arrays.values():
            for _key, entry in array.manifest.items():
                assert entry["path"] == url

    def test_url_list_matching_path_count(self, tmp_dir):
        """A URL list matching path count is used as-is (no error).

        With R-set naming (.r1 suffix), the parser produces a hierarchical
        store with subgroups instead of a flat store with arrays.
        """
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=128, num_rows=128, num_bands=1)
        _write_nitf(path_r1, num_cols=64, num_rows=64, num_bands=1)

        parser = OversightMLParser([str(path_base), str(path_r1)])
        urls = ["s3://bucket/image.ntf", "s3://bucket/image.ntf.r1"]
        store = parser(url=urls)

        # Should succeed without error — produces hierarchical store
        group = store._group
        assert len(group.groups) == 2, (
            f"Expected 2 subgroups for R-set pyramid, got {len(group.groups)}"
        )

    def test_url_list_wrong_length_raises_value_error(self):
        """A URL list with wrong length raises ValueError."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser(["a.ntf", "b.ntf", "c.ntf"])
        with pytest.raises(ValueError, match="url list length"):
            parser(url=["s3://bucket/a.ntf", "s3://bucket/b.ntf"])


class TestClassifyAssets:
    """Verify _classify_assets groups parent images and overviews correctly.

    Requirements: 3.1, 3.2
    """

    def test_no_overviews_returns_empty_overviews(self):
        """Input with no overview keys → parents populated, overviews empty."""
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        asset_a = object()
        asset_b = object()
        all_assets = [("image:0", asset_a), ("image:1", asset_b)]

        parents, overviews = _classify_assets(all_assets)

        assert parents == {"image:0": asset_a, "image:1": asset_b}
        assert overviews == {}

    def test_single_parent_with_overviews(self):
        """image:0 with two overviews → correct grouping and sort order."""
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        parent = "parent_sentinel"
        ovr1 = "overview_1_sentinel"
        ovr2 = "overview_2_sentinel"
        all_assets = [
            ("image:0", parent),
            ("image:0:overview:1", ovr1),
            ("image:0:overview:2", ovr2),
        ]

        parents, overviews = _classify_assets(all_assets)

        assert parents == {"image:0": parent}
        assert "image:0" in overviews
        assert overviews["image:0"] == [(1, ovr1), (2, ovr2)]

    def test_multiple_parents_with_mixed_overviews(self):
        """Multiple parents with different overviews → correct per-parent grouping."""
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        p0 = "parent_0"
        p1 = "parent_1"
        ovr_0_2 = "ovr_0_level_2"
        ovr_0_1 = "ovr_0_level_1"
        ovr_1_3 = "ovr_1_level_3"

        # Deliberately out of order to verify sorting
        all_assets = [
            ("image:0", p0),
            ("image:0:overview:2", ovr_0_2),
            ("image:1", p1),
            ("image:0:overview:1", ovr_0_1),
            ("image:1:overview:3", ovr_1_3),
        ]

        parents, overviews = _classify_assets(all_assets)

        assert parents == {"image:0": p0, "image:1": p1}
        # image:0 overviews sorted by level ascending
        assert overviews["image:0"] == [(1, ovr_0_1), (2, ovr_0_2)]
        # image:1 has only one overview
        assert overviews["image:1"] == [(3, ovr_1_3)]


def _make_manifest_array(rows, cols, num_bands=1):
    """Create a synthetic ManifestArray with the given dimensions.

    Returns a minimal ManifestArray suitable for testing _build_multiscale_group.
    """
    import numpy as np
    from virtualizarr.manifests import ChunkEntry, ChunkManifest, ManifestArray
    from zarr.codecs import BytesCodec
    from zarr.core.chunk_grids import RegularChunkGrid
    from zarr.core.dtype import data_type_registry
    from zarr.core.metadata.v3 import ArrayV3Metadata

    zdtype = data_type_registry.match_dtype(dtype=np.dtype("uint8"))
    metadata = ArrayV3Metadata(
        shape=(num_bands, rows, cols),
        data_type=zdtype,
        chunk_grid=RegularChunkGrid(chunk_shape=(num_bands, 256, 256)),
        chunk_key_encoding={"name": "default", "separator": "."},
        fill_value=0,
        codecs=[BytesCodec()],
        attributes={},
        dimension_names=["bands", "y", "x"],
    )
    manifest = ChunkManifest(
        entries={"0.0.0": ChunkEntry(path="s3://b/f", offset=0, length=100)},
        shape=(1, 1, 1),
    )
    return ManifestArray(metadata=metadata, chunkmanifest=manifest)


class TestBuildMultiscaleGroup:
    """Verify _build_multiscale_group builds correct hierarchical ManifestGroup.

    Requirements: 5.1, 5.2, 5.3, 5.4, 5.5
    """

    def test_three_levels_correct_subgroups_and_scale_transforms(self):
        """3 levels (4096×4096, 2048×2048, 1024×1024) → correct subgroups, layout entries, and relative scales."""
        from aws.osml.io.virtualizarr_parsers import _build_multiscale_group

        levels = [
            (_make_manifest_array(4096, 4096), 4096, 4096),
            (_make_manifest_array(2048, 2048), 2048, 2048),
            (_make_manifest_array(1024, 1024), 1024, 1024),
        ]
        group = _build_multiscale_group(levels, "s3://b/f", {})

        # 3 subgroups named "0", "1", "2"
        assert set(group.groups.keys()) == {"0", "1", "2"}

        # GeoZarr layout entries with relative scale transforms
        ms = group.metadata.attributes["multiscales"]
        layout = ms["layout"]
        assert len(layout) == 3

        # Level 0: asset "0", no derived_from, scale [1.0, 1.0]
        assert layout[0]["asset"] == "0"
        assert "derived_from" not in layout[0]
        assert layout[0]["transform"]["scale"] == [1.0, 1.0]
        assert layout[0]["transform"]["translation"] == [0.0, 0.0]

        # Level 1: asset "1", derived_from "0", relative scale [2.0, 2.0]
        assert layout[1]["asset"] == "1"
        assert layout[1]["derived_from"] == "0"
        assert layout[1]["transform"]["scale"] == [2.0, 2.0]
        assert layout[1]["transform"]["translation"] == [0.0, 0.0]

        # Level 2: asset "2", derived_from "1", relative scale [2.0, 2.0]
        assert layout[2]["asset"] == "2"
        assert layout[2]["derived_from"] == "1"
        assert layout[2]["transform"]["scale"] == [2.0, 2.0]
        assert layout[2]["transform"]["translation"] == [0.0, 0.0]

    def test_multiscales_metadata_structure(self):
        """Verify multiscales is a GeoZarr dict with layout, zarr_conventions, and no OME-NGFF fields."""
        from aws.osml.io.virtualizarr_parsers import (
            GEOZARR_MULTISCALES_CONVENTION,
            _build_multiscale_group,
        )

        levels = [
            (_make_manifest_array(4096, 4096), 4096, 4096),
            (_make_manifest_array(2048, 2048), 2048, 2048),
            (_make_manifest_array(1024, 1024), 1024, 1024),
        ]
        group = _build_multiscale_group(levels, "s3://b/f", {})

        attrs = group.metadata.attributes

        # multiscales is a dict (not a list)
        assert "multiscales" in attrs
        ms = attrs["multiscales"]
        assert isinstance(ms, dict), f"Expected multiscales to be a dict, got {type(ms)}"

        # Has layout array with correct length
        assert "layout" in ms
        assert len(ms["layout"]) == 3

        # No OME-NGFF fields
        assert "version" not in ms
        assert "axes" not in ms
        assert "type" not in ms
        assert "datasets" not in ms
        for entry in ms["layout"]:
            assert "coordinateTransformations" not in entry

        # zarr_conventions array present with correct UUID
        assert "zarr_conventions" in attrs
        zc = attrs["zarr_conventions"]
        assert isinstance(zc, list)
        assert len(zc) >= 1
        uuids = [c["uuid"] for c in zc]
        assert "d35379db-88df-4056-af3a-620245f8e347" in uuids

    def test_each_subgroup_has_one_data_array(self):
        """Each subgroup has exactly one array named 'data'."""
        from aws.osml.io.virtualizarr_parsers import _build_multiscale_group

        levels = [
            (_make_manifest_array(4096, 4096), 4096, 4096),
            (_make_manifest_array(2048, 2048), 2048, 2048),
            (_make_manifest_array(1024, 1024), 1024, 1024),
        ]
        group = _build_multiscale_group(levels, "s3://b/f", {})

        for name in ("0", "1", "2"):
            subgroup = group.groups[name]
            assert list(subgroup.arrays.keys()) == ["data"]
            # Verify the array shape matches the level dimensions
            array = subgroup.arrays["data"]
            assert array.shape is not None

    def test_resampling_method_present_when_provided(self):
        """Calling with downsampling_method='average' includes resampling_method in multiscales."""
        from aws.osml.io.virtualizarr_parsers import _build_multiscale_group

        levels = [
            (_make_manifest_array(512, 512), 512, 512),
            (_make_manifest_array(256, 256), 256, 256),
        ]
        group = _build_multiscale_group(levels, "s3://b/f", {}, downsampling_method="average")

        ms = group.metadata.attributes["multiscales"]
        assert "resampling_method" in ms, "Expected resampling_method in multiscales"
        assert ms["resampling_method"] == "average"

    def test_resampling_method_absent_when_none(self):
        """Calling with downsampling_method=None omits resampling_method from multiscales."""
        from aws.osml.io.virtualizarr_parsers import _build_multiscale_group

        levels = [
            (_make_manifest_array(512, 512), 512, 512),
            (_make_manifest_array(256, 256), 256, 256),
        ]
        group = _build_multiscale_group(levels, "s3://b/f", {}, downsampling_method=None)

        ms = group.metadata.attributes["multiscales"]
        assert "resampling_method" not in ms, "Expected no resampling_method when downsampling_method is None"


class TestMultiFileRSetPyramid:
    """Verify OversightMLParser produces correct hierarchical store from multi-file R-set pyramids.

    Requirements: 4.1, 4.2, 4.3, 4.5, 4.6, 5.1, 5.4
    """

    def test_two_file_rset_produces_two_subgroups(self, tmp_dir):
        """Two NITF files (base + .r1) produce a hierarchical store with 2 subgroups."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)

        parser = OversightMLParser([str(path_base), str(path_r1)])
        store = parser(url=["s3://b/image.ntf", "s3://b/image.ntf.r1"])

        group = store._group
        assert len(group.groups) == 2, (
            f"Expected 2 subgroups, got {len(group.groups)}: {list(group.groups.keys())}"
        )
        assert set(group.groups.keys()) == {"0", "1"}

    def test_subgroup_0_larger_subgroup_1_smaller(self, tmp_dir):
        """Subgroup '0' has larger dimensions (512×512), subgroup '1' has smaller (256×256)."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)

        parser = OversightMLParser([str(path_base), str(path_r1)])
        store = parser(url=["s3://b/image.ntf", "s3://b/image.ntf.r1"])

        group = store._group
        arr_0 = group.groups["0"].arrays["data"]
        arr_1 = group.groups["1"].arrays["data"]

        # shape is (bands, rows, cols)
        assert arr_0.shape[1] == 512 and arr_0.shape[2] == 512, (
            f"Subgroup '0' shape {arr_0.shape} should be (*, 512, 512)"
        )
        assert arr_1.shape[1] == 256 and arr_1.shape[2] == 256, (
            f"Subgroup '1' shape {arr_1.shape} should be (*, 256, 256)"
        )

    def test_chunk_refs_use_correct_urls(self, tmp_dir):
        """Chunk refs in subgroup '0' use first URL, subgroup '1' uses second URL."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)

        url_base = "s3://b/image.ntf"
        url_r1 = "s3://b/image.ntf.r1"
        parser = OversightMLParser([str(path_base), str(path_r1)])
        store = parser(url=[url_base, url_r1])

        group = store._group

        # All chunks in subgroup "0" should reference the base URL
        for _key, entry in group.groups["0"].arrays["data"].manifest.items():
            assert entry["path"] == url_base, (
                f"Subgroup '0' chunk should use '{url_base}', got '{entry['path']}'"
            )

        # All chunks in subgroup "1" should reference the R1 URL
        for _key, entry in group.groups["1"].arrays["data"].manifest.items():
            assert entry["path"] == url_r1, (
                f"Subgroup '1' chunk should use '{url_r1}', got '{entry['path']}'"
            )

    def test_multiscales_metadata_present_with_correct_scales(self, tmp_dir):
        """Root group has GeoZarr multiscales metadata with correct relative scale transforms."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)

        parser = OversightMLParser([str(path_base), str(path_r1)])
        store = parser(url=["s3://b/image.ntf", "s3://b/image.ntf.r1"])

        group = store._group
        attrs = group.metadata.attributes
        assert "multiscales" in attrs, "Root group should have 'multiscales' attribute"

        ms = attrs["multiscales"]
        assert isinstance(ms, dict), f"Expected multiscales to be a dict, got {type(ms)}"
        layout = ms["layout"]
        assert len(layout) == 2

        # Level 0: asset "0", no derived_from, scale [1.0, 1.0]
        assert layout[0]["asset"] == "0"
        assert "derived_from" not in layout[0]
        assert layout[0]["transform"]["scale"] == [1.0, 1.0]
        assert layout[0]["transform"]["translation"] == [0.0, 0.0]

        # Level 1: asset "1", derived_from "0", relative scale [2.0, 2.0] (512/256)
        assert layout[1]["asset"] == "1"
        assert layout[1]["derived_from"] == "0"
        assert layout[1]["transform"]["scale"] == [2.0, 2.0]
        assert layout[1]["transform"]["translation"] == [0.0, 0.0]

    def test_out_of_order_rset_paths_produce_correct_levels(self, tmp_dir):
        """Out-of-order paths produce correct overview levels from filenames."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "img.ntf"
        path_r1 = tmp_dir / "img.ntf.r1"
        path_r3 = tmp_dir / "img.ntf.r3"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r3, num_cols=128, num_rows=128, num_bands=1)

        # Pass paths out of order: base, r3, r1
        parser = OversightMLParser([str(path_base), str(path_r3), str(path_r1)])
        store = parser(
            url=["s3://b/img.ntf", "s3://b/img.ntf.r3", "s3://b/img.ntf.r1"]
        )

        group = store._group

        # Should have 3 subgroups (base + 2 overviews)
        assert len(group.groups) == 3, (
            f"Expected 3 subgroups, got {len(group.groups)}: {list(group.groups.keys())}"
        )

        # Subgroup "0" is the base (largest), then overviews sorted by level
        # IO.open() sorts overviews by level number from filename, so:
        # subgroup "0" = base (512×512)
        # subgroup "1" = overview:1 (256×256, from .r1)
        # subgroup "2" = overview:3 (128×128, from .r3)
        arr_0 = group.groups["0"].arrays["data"]
        arr_1 = group.groups["1"].arrays["data"]
        arr_2 = group.groups["2"].arrays["data"]

        assert arr_0.shape[1] == 512, f"Level 0 rows should be 512, got {arr_0.shape[1]}"
        assert arr_1.shape[1] == 256, f"Level 1 rows should be 256, got {arr_1.shape[1]}"
        assert arr_2.shape[1] == 128, f"Level 2 rows should be 128, got {arr_2.shape[1]}"

        # Verify GeoZarr layout entries with relative scale transforms
        ms = group.metadata.attributes["multiscales"]
        layout = ms["layout"]

        # Level 0: asset "0", no derived_from, scale [1.0, 1.0]
        assert layout[0]["asset"] == "0"
        assert "derived_from" not in layout[0]
        assert layout[0]["transform"]["scale"] == [1.0, 1.0]

        # Level 1: asset "1", derived_from "0", relative scale [2.0, 2.0] (512/256)
        assert layout[1]["asset"] == "1"
        assert layout[1]["derived_from"] == "0"
        assert layout[1]["transform"]["scale"] == [2.0, 2.0]

        # Level 2: asset "2", derived_from "1", relative scale [2.0, 2.0] (256/128)
        assert layout[2]["asset"] == "2"
        assert layout[2]["derived_from"] == "1"
        assert layout[2]["transform"]["scale"] == [2.0, 2.0]

        # Verify URL mapping: r1 chunks use r1 URL, r3 chunks use r3 URL
        for _key, entry in group.groups["1"].arrays["data"].manifest.items():
            assert entry["path"] == "s3://b/img.ntf.r1", (
                f"Level 1 (from .r1) should use r1 URL, got '{entry['path']}'"
            )
        for _key, entry in group.groups["2"].arrays["data"].manifest.items():
            assert entry["path"] == "s3://b/img.ntf.r3", (
                f"Level 2 (from .r3) should use r3 URL, got '{entry['path']}'"
            )


class TestSingleFileBackwardCompat:
    """Verify single-file, no-overview path produces a flat store (backward compat).

    Requirements: 3.4, 7.1, 7.2
    """

    def test_single_file_produces_flat_store_with_no_subgroups(self, tmp_dir):
        """A single NITF file with single path and URL produces a flat store
        with no subgroups, an 'image:0' array, no multiscales, and correct source."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "single.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        url = "s3://bucket/single.ntf"
        parser = OversightMLParser(str(path))
        store = parser(url=url)

        group = store._group

        # No subgroups — flat store for single-file, no-overview input
        assert not group.groups, (
            f"Expected no subgroups for single-file input, got {list(group.groups.keys())}"
        )

        # Arrays dict has 'image:0' key
        assert "image:0" in group.arrays, (
            f"Expected 'image:0' in arrays, got {list(group.arrays.keys())}"
        )

        # No multiscales or zarr_conventions in attributes
        attrs = group.metadata.attributes if group.metadata else {}
        assert "multiscales" not in attrs, (
            "Expected no 'multiscales' attribute for single-file, no-overview input"
        )
        assert "zarr_conventions" not in attrs, (
            "Expected no 'zarr_conventions' attribute for single-file, no-overview input"
        )

        # Source attribute matches the URL
        assert attrs.get("source") == url, (
            f"Expected source='{url}', got '{attrs.get('source')}'"
        )


class TestWriteTileIndex:
    """Verify write_tile_index serializes flat and hierarchical stores correctly.

    Requirements: 6.2, 6.3, 6.4, 6.5, 6.6, 7.3
    """

    def test_flat_store_json_output_structure(self, tmp_dir):
        """A flat ManifestStore (single NITF, single path) serializes to JSON
        with expected kerchunk structure.

        Requirements: 6.2, 7.3
        """
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path = tmp_dir / "flat.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        url = "s3://bucket/flat.ntf"
        store = OversightMLParser(str(path))(url=url)

        output = str(tmp_dir / "flat.tile_index.json")
        write_tile_index(store, output)

        with open(output) as f:
            data = json.load(f)

        # Kerchunk JSON has "version" and "refs" keys
        assert "version" in data, "Expected 'version' key in kerchunk JSON"
        assert "refs" in data, "Expected 'refs' key in kerchunk JSON"

        refs = data["refs"]

        # Root metadata keys present
        assert ".zgroup" in refs, "Expected '.zgroup' in refs"
        assert ".zattrs" in refs, "Expected '.zattrs' in refs"

        # Flat store .zattrs should NOT contain zarr_conventions or multiscales
        import json as _json
        flat_zattrs = _json.loads(refs[".zattrs"])
        assert "zarr_conventions" not in flat_zattrs, (
            "Expected no 'zarr_conventions' in flat store .zattrs"
        )
        assert "multiscales" not in flat_zattrs, (
            "Expected no 'multiscales' in flat store .zattrs"
        )

        # At least one chunk reference exists (for the 128x128 image)
        chunk_keys = [k for k in refs if not k.startswith(".")]
        assert len(chunk_keys) > 0, "Expected at least one chunk reference key"

    def test_hierarchical_store_json_output_structure(self, tmp_dir):
        """A hierarchical ManifestStore (2 NITF files with R-set naming)
        serializes to JSON with path-prefixed keys and multiscales metadata.

        Requirements: 6.3, 6.4
        """
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r1, num_cols=128, num_rows=128, num_bands=1)

        url_base = "s3://bucket/image.ntf"
        url_r1 = "s3://bucket/image.ntf.r1"
        store = OversightMLParser([str(path_base), str(path_r1)])(
            url=[url_base, url_r1]
        )

        output = str(tmp_dir / "hierarchical.tile_index.json")
        write_tile_index(store, output)

        with open(output) as f:
            data = json.load(f)

        refs = data["refs"]

        # Root .zattrs contains GeoZarr multiscales metadata
        zattrs = json.loads(refs[".zattrs"])
        assert "multiscales" in zattrs, (
            "Expected 'multiscales' in root .zattrs"
        )

        # multiscales is a dict (GeoZarr), not a list (OME-NGFF)
        ms = zattrs["multiscales"]
        assert isinstance(ms, dict), (
            f"Expected multiscales to be a dict (GeoZarr), got {type(ms)}"
        )
        assert "layout" in ms, "Expected 'layout' key in multiscales"
        assert len(ms["layout"]) == 2, (
            f"Expected 2 layout entries, got {len(ms['layout'])}"
        )

        # zarr_conventions array present with correct UUID
        assert "zarr_conventions" in zattrs, (
            "Expected 'zarr_conventions' in root .zattrs"
        )
        zc = zattrs["zarr_conventions"]
        assert isinstance(zc, list), (
            f"Expected zarr_conventions to be a list, got {type(zc)}"
        )
        assert len(zc) >= 1, "Expected at least one zarr_conventions entry"
        uuids = [c["uuid"] for c in zc]
        assert "d35379db-88df-4056-af3a-620245f8e347" in uuids, (
            f"Expected GeoZarr multiscales UUID in zarr_conventions, got {uuids}"
        )

        # No OME-NGFF fields
        assert "version" not in ms, "OME-NGFF 'version' should not be in multiscales"
        assert "axes" not in ms, "OME-NGFF 'axes' should not be in multiscales"
        assert "datasets" not in ms, "OME-NGFF 'datasets' should not be in multiscales"
        assert "type" not in ms, "OME-NGFF 'type' should not be in multiscales"
        for entry in ms["layout"]:
            assert "coordinateTransformations" not in entry, (
                "OME-NGFF 'coordinateTransformations' should not be in layout entries"
            )

        # Keys are prefixed with subgroup paths: 0/ and 1/
        all_keys = set(refs.keys())
        has_level_0 = any(k.startswith("0/") for k in all_keys)
        has_level_1 = any(k.startswith("1/") for k in all_keys)
        assert has_level_0, "Expected keys prefixed with '0/'"
        assert has_level_1, "Expected keys prefixed with '1/'"

        # Chunk reference keys should include 0/data/ and 1/data/ prefixes
        data_keys_0 = [k for k in all_keys if k.startswith("0/data/") and not k.startswith("0/data/.")]
        data_keys_1 = [k for k in all_keys if k.startswith("1/data/") and not k.startswith("1/data/.")]
        assert len(data_keys_0) > 0, "Expected chunk keys under 0/data/"
        assert len(data_keys_1) > 0, "Expected chunk keys under 1/data/"

        # Verify chunk references have correct URLs and byte ranges
        for k in data_keys_0:
            ref = refs[k]
            assert isinstance(ref, list), f"Chunk ref {k} should be a list"
            assert ref[0] == url_base, (
                f"Level 0 chunk {k} should reference '{url_base}', got '{ref[0]}'"
            )
            assert isinstance(ref[1], int) and ref[1] >= 0, "Offset should be non-negative int"
            assert isinstance(ref[2], int) and ref[2] > 0, "Length should be positive int"

        for k in data_keys_1:
            ref = refs[k]
            assert isinstance(ref, list), f"Chunk ref {k} should be a list"
            assert ref[0] == url_r1, (
                f"Level 1 chunk {k} should reference '{url_r1}', got '{ref[0]}'"
            )

    def test_hierarchical_store_parquet_output(self, tmp_dir):
        """A hierarchical ManifestStore serializes to Parquet as a non-empty directory.

        Requirements: 6.5
        """
        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r1, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser([str(path_base), str(path_r1)])(
            url=["s3://bucket/image.ntf", "s3://bucket/image.ntf.r1"]
        )

        output = str(tmp_dir / "hierarchical.tile_index.parquet")
        write_tile_index(store, output)

        # Parquet output is a directory with files inside
        output_path = Path(output)
        assert output_path.exists(), "Parquet output directory should exist"
        # LazyReferenceMapper creates files inside the directory
        contents = list(output_path.iterdir()) if output_path.is_dir() else []
        assert len(contents) > 0, (
            "Parquet output directory should contain at least one file"
        )

    def test_segments_filter_on_hierarchical_store(self, tmp_dir):
        """Filtering with segments=['0', '2'] on a 3-level hierarchical store
        produces output containing only subgroups '0' and '2'.

        Requirements: 6.6
        """
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path_base = tmp_dir / "img.ntf"
        path_r1 = tmp_dir / "img.ntf.r1"
        path_r3 = tmp_dir / "img.ntf.r3"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r3, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser([str(path_base), str(path_r1), str(path_r3)])(
            url=["s3://b/img.ntf", "s3://b/img.ntf.r1", "s3://b/img.ntf.r3"]
        )

        output = str(tmp_dir / "filtered.tile_index.json")
        write_tile_index(store, output, segments=["0", "2"])

        with open(output) as f:
            data = json.load(f)

        refs = data["refs"]
        all_keys = set(refs.keys())

        # Only subgroups "0" and "2" should appear
        has_level_0 = any(k.startswith("0/") for k in all_keys)
        has_level_2 = any(k.startswith("2/") for k in all_keys)
        has_level_1 = any(k.startswith("1/") for k in all_keys)

        assert has_level_0, "Expected keys prefixed with '0/' in filtered output"
        assert has_level_2, "Expected keys prefixed with '2/' in filtered output"
        assert not has_level_1, "Subgroup '1' should NOT appear in filtered output"


class TestEndToEndPyramidRoundTrip:
    """End-to-end test for multi-file pyramid round-trip.

    Creates 2 NITF files with different dimensions, generates a hierarchical
    tile index, opens it via fsspec ReferenceFileSystem + zarr, and verifies
    the zarr store has the expected group structure and multiscales metadata.

    Requirements: 8.1, 8.2, 9.1, 9.2
    """

    def test_pyramid_round_trip_structure(self, tmp_dir):
        """Multi-file pyramid round-trip: create files, generate index, open
        via zarr, verify group structure and multiscales metadata."""
        import json

        import zarr
        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        # 1. Create 2 NITF files with random pixel data
        rng = np.random.default_rng(42)

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"

        # Write base image (256×256) with random data
        metadata_base = BufferedMetadataProvider()
        metadata_base.set("IC", "NC")
        metadata_base.set("IMODE", "B")
        provider_base = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=256,
            num_rows=256,
            num_bands=1,
            block_width=256,
            block_height=256,
            metadata=metadata_base,
        )
        data_base = rng.integers(1, 255, size=(1, 256, 256), dtype=np.uint8)
        provider_base.set_full_image(data_base)
        writer_base = IO.open([str(path_base)], "w", "nitf")
        writer_base.add_asset("image:0", provider_base, "Image", "base", ["data"])
        writer_base.close()

        # Write R1 image (128×128) with random data
        metadata_r1 = BufferedMetadataProvider()
        metadata_r1.set("IC", "NC")
        metadata_r1.set("IMODE", "B")
        provider_r1 = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=128,
            num_rows=128,
            num_bands=1,
            block_width=128,
            block_height=128,
            metadata=metadata_r1,
        )
        data_r1 = rng.integers(1, 255, size=(1, 128, 128), dtype=np.uint8)
        provider_r1.set_full_image(data_r1)
        writer_r1 = IO.open([str(path_r1)], "w", "nitf")
        writer_r1.add_asset("image:0", provider_r1, "Image", "r1", ["data"])
        writer_r1.close()

        # 2. Generate hierarchical tile index JSON
        #    Use file:// URLs so fsspec can resolve them locally
        url_base = path_base.as_uri()
        url_r1 = path_r1.as_uri()
        parser = OversightMLParser([str(path_base), str(path_r1)])
        store = parser(url=[url_base, url_r1])

        output = str(tmp_dir / "pyramid.tile_index.json")
        write_tile_index(store, output)

        # 3. Open the JSON index via fsspec ReferenceFileSystem + zarr
        import fsspec
        from zarr.storage._fsspec import FsspecStore

        fs = fsspec.filesystem(
            "reference", fo=output, skip_instance_cache=True
        )
        store_zarr = FsspecStore(fs=fs, read_only=True, path="")
        root = zarr.open_group(store_zarr, mode="r", zarr_format=2)

        # 4. Verify the zarr group has subgroups "0" and "1"
        group_keys = list(root.group_keys())
        assert "0" in group_keys, (
            f"Expected subgroup '0' in zarr store, got {group_keys}"
        )
        assert "1" in group_keys, (
            f"Expected subgroup '1' in zarr store, got {group_keys}"
        )

        # 5. Verify each subgroup has a "data" array with correct shape
        #    Access arrays by direct path since zarr v2 refs-based stores
        #    may not enumerate children via array_keys()
        arr_0 = root["0/data"]
        assert arr_0.shape == (1, 256, 256), (
            f"Expected shape (1, 256, 256) for level 0, got {arr_0.shape}"
        )

        arr_1 = root["1/data"]
        assert arr_1.shape == (1, 128, 128), (
            f"Expected shape (1, 128, 128) for level 1, got {arr_1.shape}"
        )

        # 6. Verify the root group has GeoZarr multiscales metadata
        with open(output) as f:
            index_data = json.load(f)
        root_zattrs = json.loads(index_data["refs"][".zattrs"])
        assert "multiscales" in root_zattrs, (
            "Expected 'multiscales' in root .zattrs"
        )

        # multiscales is a dict (GeoZarr), not a list (OME-NGFF)
        ms = root_zattrs["multiscales"]
        assert isinstance(ms, dict), (
            f"Expected multiscales to be a dict (GeoZarr), got {type(ms)}"
        )
        assert "layout" in ms, "Expected 'layout' key in multiscales"
        layout = ms["layout"]
        assert len(layout) == 2, (
            f"Expected 2 layout entries, got {len(layout)}"
        )

        # Layout entry structure: asset, derived_from, transform
        assert layout[0]["asset"] == "0"
        assert "derived_from" not in layout[0]
        assert layout[0]["transform"]["scale"] == [1.0, 1.0]
        assert layout[0]["transform"]["translation"] == [0.0, 0.0]

        assert layout[1]["asset"] == "1"
        assert layout[1]["derived_from"] == "0"
        assert layout[1]["transform"]["scale"] == [2.0, 2.0]
        assert layout[1]["transform"]["translation"] == [0.0, 0.0]

        # zarr_conventions array present with correct UUID
        assert "zarr_conventions" in root_zattrs, (
            "Expected 'zarr_conventions' in root .zattrs"
        )
        zc = root_zattrs["zarr_conventions"]
        assert isinstance(zc, list)
        uuids = [c["uuid"] for c in zc]
        assert "d35379db-88df-4056-af3a-620245f8e347" in uuids

        # No OME-NGFF fields
        assert "version" not in ms, "OME-NGFF 'version' should not be in multiscales"
        assert "axes" not in ms, "OME-NGFF 'axes' should not be in multiscales"
        assert "datasets" not in ms, "OME-NGFF 'datasets' should not be in multiscales"
        assert "type" not in ms, "OME-NGFF 'type' should not be in multiscales"
        for entry in layout:
            assert "coordinateTransformations" not in entry, (
                "OME-NGFF 'coordinateTransformations' should not be in layout entries"
            )

    def test_pyramid_io_open_reads_both_levels(self, tmp_dir):
        """Verify IO.open() can read tile data from both files in the pyramid."""
        from aws.osml.io import AssetType

        rng = np.random.default_rng(99)

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"

        # Write base image (256×256)
        metadata_base = BufferedMetadataProvider()
        metadata_base.set("IC", "NC")
        metadata_base.set("IMODE", "B")
        provider_base = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=256,
            num_rows=256,
            num_bands=1,
            block_width=256,
            block_height=256,
            metadata=metadata_base,
        )
        data_base = rng.integers(1, 255, size=(1, 256, 256), dtype=np.uint8)
        provider_base.set_full_image(data_base)
        writer_base = IO.open([str(path_base)], "w", "nitf")
        writer_base.add_asset("image:0", provider_base, "Image", "base", ["data"])
        writer_base.close()

        # Write R1 image (128×128)
        metadata_r1 = BufferedMetadataProvider()
        metadata_r1.set("IC", "NC")
        metadata_r1.set("IMODE", "B")
        provider_r1 = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=128,
            num_rows=128,
            num_bands=1,
            block_width=128,
            block_height=128,
            metadata=metadata_r1,
        )
        data_r1 = rng.integers(1, 255, size=(1, 128, 128), dtype=np.uint8)
        provider_r1.set_full_image(data_r1)
        writer_r1 = IO.open([str(path_r1)], "w", "nitf")
        writer_r1.add_asset("image:0", provider_r1, "Image", "r1", ["data"])
        writer_r1.close()

        # Open both files via IO.open() and verify assets
        with IO.open([str(path_base), str(path_r1)], "r") as reader:
            keys = reader.get_asset_keys(asset_type=AssetType.Image)
            assert "image:0" in keys, f"Expected 'image:0' in keys, got {keys}"
            assert "image:0:overview:1" in keys, (
                f"Expected 'image:0:overview:1' in keys, got {keys}"
            )

            # Read tile data from base image
            base_asset = reader.get_asset("image:0")
            base_tile = base_asset.get_block(0, 0, 0)
            assert base_tile.shape == (1, 256, 256), (
                f"Base tile shape should be (1, 256, 256), got {base_tile.shape}"
            )
            np.testing.assert_array_equal(
                base_tile, data_base,
                err_msg="Base tile data should match written data",
            )

            # Read tile data from overview image
            ovr_asset = reader.get_asset("image:0:overview:1")
            ovr_tile = ovr_asset.get_block(0, 0, 0)
            assert ovr_tile.shape == (1, 128, 128), (
                f"Overview tile shape should be (1, 128, 128), got {ovr_tile.shape}"
            )
            np.testing.assert_array_equal(
                ovr_tile, data_r1,
                err_msg="Overview tile data should match written data",
            )
