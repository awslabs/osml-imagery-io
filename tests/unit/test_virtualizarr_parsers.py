"""Unit tests for OversightMLParser and _build_manifest_array.

Validates that the extracted _build_manifest_array helper produces
structurally identical output to the original inline implementation.

Requirements: 7.1, 7.2
"""

import os
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
    metadata["IC"] = ic
    metadata["IMODE"] = "B"

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


def _chunk_url(path: Path) -> str:
    """Return the chunk-reference URL a local parse of *path* produces.

    ``OversightMLParser`` reads via fsspec, which normalizes a bare local path
    to a ``file://<abspath>`` URI in the stored chunk references.
    """
    return f"file://{os.path.abspath(str(path))}"


class TestBuildManifestArrayBehaviorPreserving:
    """Verify that the extracted _build_manifest_array produces correct ManifestStore output.

    The refactored OversightMLParser uses _build_manifest_array as a standalone
    helper. This test confirms the parser output is structurally correct for a
    single-segment NITF file.

    Requirements: 7.1, 7.2
    """

    def test_single_segment_nitf_produces_correct_store_structure(self, tmp_dir):
        """A single-segment NITF produces a ManifestStore with a hierarchical
        structure: subgroup '0' containing a 'data' array."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=3)

        # The root ``source`` attribute records the URL exactly as passed.
        url = str(path)
        parser = OversightMLParser()
        store = parser(str(path))

        # Root group has subgroup "0" with a "data" array
        group = store._group
        assert "0" in group.groups, (
            f"Expected subgroup '0' in groups, got {list(group.groups.keys())}"
        )
        assert "data" in group.groups["0"].arrays, (
            f"Expected 'data' in subgroup '0' arrays, got {list(group.groups['0'].arrays.keys())}"
        )

        # Source attribute matches the URL
        attrs = group.metadata.attributes if group.metadata else {}
        assert attrs.get("source") == url, (
            f"Expected source='{url}', got '{attrs.get('source')}'"
        )

        # GeoZarr multiscales metadata present with single layout entry
        assert "multiscales" in attrs, "Expected 'multiscales' in root attributes"
        layout = attrs["multiscales"]["layout"]
        assert len(layout) == 1, f"Expected 1 layout entry, got {len(layout)}"
        assert layout[0]["asset"] == "0"

    def test_manifest_array_shape_and_chunks(self, tmp_dir):
        """The ManifestArray has correct shape and chunk shape matching the NITF."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        num_cols, num_rows, num_bands = 128, 128, 3
        _write_nitf(path, num_cols=num_cols, num_rows=num_rows, num_bands=num_bands)

        store = OversightMLParser()(str(path))
        array = store._group.groups["0"].arrays["data"]

        # Shape: (bands, rows, cols)
        assert array.shape == (num_bands, num_rows, num_cols), (
            f"Expected shape ({num_bands}, {num_rows}, {num_cols}), got {array.shape}"
        )

        # Chunk shape: (bands, block_height, block_width)
        chunks = array.metadata.chunks
        assert chunks[0] == num_bands, (
            f"Expected bands chunk {num_bands}, got {chunks[0]}"
        )

    def test_chunk_manifest_has_entries(self, tmp_dir):
        """The chunk manifest contains entries for all tiles."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path))
        array = store._group.groups["0"].arrays["data"]

        manifest = array.manifest
        # Single tile for 128x128 image with 256x256 block size → 1 chunk
        assert len(manifest) > 0, "Chunk manifest should have at least one entry"

        # Each entry should reference the parsed URL
        expected_url = _chunk_url(path)
        for _key, entry in manifest.items():
            assert entry["path"] == expected_url, (
                f"Expected chunk path '{expected_url}', got '{entry['path']}'"
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

        store = OversightMLParser()(str(path))
        array = store._group.groups["0"].arrays["data"]

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

        store = OversightMLParser()(str(path))
        multi_range_refs = getattr(store, "multi_range_refs", None)
        assert isinstance(multi_range_refs, dict), (
            f"Expected multi_range_refs to be a dict, got {type(multi_range_refs)}"
        )


def _write_tiff(path: Path, num_cols: int, num_rows: int, num_bands: int,
                planar_config: int, compression: int = 1,
                tile_size: int = 64) -> None:
    """Write a TIFF with the given band interleaving and compression."""
    metadata = BufferedMetadataProvider()
    metadata["322"] = str(tile_size)   # TileWidth
    metadata["323"] = str(tile_size)   # TileLength
    metadata["259"] = compression      # Compression
    metadata["317"] = 1                # Predictor
    metadata["284"] = planar_config    # PlanarConfiguration

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, tile_size),
        block_height=min(num_rows, tile_size),
        metadata=metadata,
    )
    provider.set_full_image(
        np.zeros((num_bands, num_rows, num_cols), dtype=np.uint8)
    )

    writer = IO.open([str(path)], "w", "tiff")
    writer.metadata = metadata
    writer.add_asset("image:0", provider, "Image", "test", ["data"])
    writer.close()


class TestPlanarTiffChunkGeometry:
    """Planar TIFF (``PlanarConfiguration=2``) is chunked one band per chunk.

    Each band of a planar image is a separate tile in the file (TIFF 6.0 tag 284,
    p. 38), and for compressed data each plane has its own length, so the plane
    boundaries cannot be recovered from a single concatenated buffer.  Giving
    each plane its own chunk sidesteps that entirely: every chunk is one
    self-contained single-plane tile and zarr stacks the bands.
    """

    def test_planar_chunk_shape_is_one_band(self, tmp_dir):
        path = tmp_dir / "planar.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=3, planar_config=2)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]

        # The array still presents all three bands…
        assert array.shape == (3, 128, 128)
        # …but a chunk holds only one of them.
        assert array.metadata.chunks == (1, 64, 64)

    def test_planar_chunk_keys_are_band_granular(self, tmp_dir):
        path = tmp_dir / "planar.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=3, planar_config=2)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]
        keys = set(array.manifest.dict().keys())

        # 3 bands × 2×2 tile grid = 12 chunks, keyed {band}.{row}.{col}.
        assert len(keys) == 12
        assert keys == {
            f"{band}.{row}.{col}"
            for band in range(3)
            for row in range(2)
            for col in range(2)
        }

    def test_planar_codec_presents_chunks_as_single_band(self, tmp_dir):
        path = tmp_dir / "planar.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=3, planar_config=2)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]
        tiff_codec = [
            c for c in array.metadata.codecs
            if getattr(c, "codec_name", "").endswith("/tiff-tile")
        ]
        assert len(tiff_codec) == 1, "planar TIFF must carry a TiffTileCodec"
        # Each chunk is one plane's tile: one band, nothing interleaved.
        assert tiff_codec[0].samples_per_pixel == 1
        assert tiff_codec[0].planar_config == 1

    def test_planar_chunks_have_distinct_byte_ranges(self, tmp_dir):
        """Every plane gets its own range — the bug was referencing band 0 only."""
        path = tmp_dir / "planar.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=3, planar_config=2)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]
        offsets = [entry["offset"] for _key, entry in array.manifest.items()]
        assert len(set(offsets)) == 12, "each plane chunk must reference distinct bytes"

    def test_chunky_multiband_tiff_geometry_unchanged(self, tmp_dir):
        """Chunky arrays keep the all-bands-in-one-chunk grid and 0.{row}.{col} keys."""
        path = tmp_dir / "chunky.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=3, planar_config=1)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]

        assert array.shape == (3, 128, 128)
        assert array.metadata.chunks == (3, 64, 64)
        keys = set(array.manifest.dict().keys())
        assert keys == {f"0.{row}.{col}" for row in range(2) for col in range(2)}

    def test_single_band_planar_tiff_geometry_unchanged(self, tmp_dir):
        """PlanarConfiguration is irrelevant at SamplesPerPixel=1 (TIFF 6.0 p. 38)."""
        path = tmp_dir / "planar1.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=1, planar_config=2)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]

        assert array.metadata.chunks == (1, 64, 64)
        keys = set(array.manifest.dict().keys())
        assert keys == {f"0.{row}.{col}" for row in range(2) for col in range(2)}

    @pytest.mark.parametrize("compression", [1, 5, 8], ids=["none", "lzw", "deflate"])
    def test_planar_geometry_holds_for_every_compression(self, tmp_dir, compression):
        """Per-plane chunking is uniform — compressed and uncompressed alike.

        This is what collapses the two planar sub-cases into one fix: the plane
        boundaries come from the tag, never from geometry, so varying compressed
        lengths stop mattering.
        """
        path = tmp_dir / f"planar-{compression}.tif"
        _write_tiff(path, num_cols=128, num_rows=128, num_bands=3,
                    planar_config=2, compression=compression)

        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        array = OversightMLParser()(str(path))._group.groups["0"].arrays["data"]

        assert array.metadata.chunks == (1, 64, 64)
        assert len(array.manifest.dict()) == 12


class TestParserProtocolSignature:
    """Verify OversightMLParser conforms to the VirtualiZarr (url, registry)
    callable protocol.
    """

    def test_no_constructor_arguments(self):
        """The parser takes no parse-time configuration."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser()
        assert callable(parser)

    def test_url_must_be_a_string(self, tmp_dir):
        """A non-string url (e.g. the old list form) raises TypeError."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser()
        with pytest.raises(TypeError, match="url must be a string"):
            parser(["a.ntf", "b.ntf"])

    def test_unrecognized_extension_raises_value_error(self, tmp_dir):
        """A URL whose extension maps to no format raises ValueError."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        parser = OversightMLParser()
        with pytest.raises(ValueError, match="Cannot determine imagery format"):
            parser(str(tmp_dir / "mystery.xyz"))

    def test_registry_is_threaded_into_store(self, tmp_dir):
        """The registry argument is passed through to the ManifestStore."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        parser = OversightMLParser()
        # A None registry is valid (ManifestStore default); the call must accept
        # the keyword and succeed.
        store = parser(str(path), registry=None)
        assert store is not None


class TestURLReadSource:
    """Verify the parsed URL is the single source of truth for reading and refs."""

    def test_local_url_becomes_chunk_ref(self, tmp_dir):
        """Chunk references point at the (fsspec-normalized) parsed URL."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path))
        expected = _chunk_url(path)
        for sg in store._group.groups.values():
            for array in sg.arrays.values():
                for _key, entry in array.manifest.items():
                    assert entry["path"] == expected

    def test_rset_companions_auto_discovered(self, tmp_dir):
        """Sibling ``.rN`` files are discovered from the base URL (no list)."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=128, num_rows=128, num_bands=1)
        _write_nitf(path_r1, num_cols=64, num_rows=64, num_bands=1)

        store = OversightMLParser()(str(path_base))
        group = store._group
        assert len(group.groups) == 2, (
            f"Expected 2 subgroups for R-set pyramid, got {len(group.groups)}"
        )


class TestParseUsesRangeReads:
    """Verify index construction reads byte ranges on demand, not a full download.

    This is the bootstrap the remote-IO design targets: parsing a block-capable
    source builds the tile index from bounded header/offset-table reads rather
    than pulling the whole file into memory.
    """

    def test_parse_does_not_download_whole_file(self, tmp_dir, monkeypatch):
        """Parsing a large tiled TIFF pulls fewer bytes than its size.

        TIFF keeps its tile offsets in the header, so index construction reads
        only the header + offset table — not the pixel data.  A ~1 MB tiled
        image makes the range-read bound observable (the tiny checked-in unit
        files fit entirely within the 64 KiB header prefetch).
        """
        import fsspec
        from aws.osml.io import imsave
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "big.tif"
        data = np.random.RandomState(0).randint(0, 255, (1, 1024, 1024), dtype=np.uint8)
        imsave(str(path), data, compression="none", block_size=(256, 256))
        file_size = path.stat().st_size
        assert file_size > 64 * 1024

        # Count bytes at BOTH fetch layers: the handle's stateful `.read()` (the
        # serial fallback) and the filesystem's concurrent `cat_ranges` (the
        # cursor-free path the lock refactor routes concurrent reads through).
        # After the concurrent-read work, an fsspec-backed source fetches ranges
        # via `cat_ranges` on the filesystem, bypassing the handle's `.read()`, so
        # counting only `.read()` would observe zero bytes and miss a real
        # download regression. Summing both layers keeps the bound observable
        # regardless of which path the reader takes.
        counter = {"total": 0}
        real_open = fsspec.open

        class _CountingHandle:
            def __init__(self, inner):
                self._inner = inner

            def read(self, n=-1):
                b = self._inner.read(n)
                counter["total"] += len(b)
                return b

            def __getattr__(self, name):
                return getattr(self._inner, name)

        class _CountingOpener:
            def __init__(self, of):
                self._of = of

            def __enter__(self):
                return _CountingHandle(self._of.__enter__())

            def __exit__(self, *exc):
                return self._of.__exit__(*exc)

        def counting_open(url, *args, **kwargs):
            return _CountingOpener(real_open(url, *args, **kwargs))

        monkeypatch.setattr(fsspec, "open", counting_open)

        # Wrap the LocalFileSystem's cat_ranges so concurrent range fetches are
        # tallied too (the class method covers every instance the parser opens).
        local_fs_cls = type(fsspec.filesystem("file"))
        real_cat_ranges = local_fs_cls.cat_ranges

        def counting_cat_ranges(self, paths, starts, ends, *args, **kwargs):
            result = real_cat_ranges(self, paths, starts, ends, *args, **kwargs)
            for chunk in result:
                counter["total"] += len(chunk)
            return result

        monkeypatch.setattr(local_fs_cls, "cat_ranges", counting_cat_ranges)

        store = OversightMLParser()(str(path))
        assert store is not None

        assert 0 < counter["total"] < file_size, (
            f"parse read {counter['total']} of {file_size} bytes — expected a "
            "bounded range read, not a full download"
        )


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

    def test_mask_assets_are_skipped(self):
        """Mask assets (keys ending in :mask) are excluded from parents and
        overviews — they are not part of the Zarr pyramid.

        Resolves design open question 1: mask overviews are classified via
        MASK_PATTERN and deliberately skipped from the pyramid for now.
        """
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        parent = "parent_sentinel"
        ovr1 = "overview_1_sentinel"
        full_mask = "full_mask_sentinel"
        ovr_mask = "overview_mask_sentinel"

        # A full COG-style layout: image, its mask, an overview, and the
        # overview's mask.
        all_assets = [
            ("image:0", parent),
            ("image:0:mask", full_mask),
            ("image:0:overview:1", ovr1),
            ("image:0:overview:1:mask", ovr_mask),
        ]

        parents, overviews = _classify_assets(all_assets)

        # Only the non-mask image is a parent; only the non-mask overview is
        # grouped. Both mask keys are dropped.
        assert parents == {"image:0": parent}
        assert overviews == {"image:0": [(1, ovr1)]}

    def test_standalone_mask_is_not_a_parent(self):
        """A standalone mask key (fallback for non-COG ordering) is skipped
        rather than misclassified as a parent image."""
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        image = "image_sentinel"
        standalone_mask = "standalone_mask_sentinel"
        all_assets = [
            ("image:0:mask", standalone_mask),
            ("image:1", image),
        ]

        parents, overviews = _classify_assets(all_assets)

        assert parents == {"image:1": image}
        assert overviews == {}

    def test_non_image_prefixed_key_is_a_parent(self):
        """A key that is not spelled ``image:N`` is still a parent image.

        ``image:N`` is a NITF/TIFF convention, not a library-wide one. DTED's
        sole image asset is keyed ``elevation`` (``src/dted/image.rs``), and
        while the classifier required an ``image:`` prefix that asset was
        silently dropped — leaving ``parents`` empty so the caller raised
        ``max() iterable argument is empty``. Keys reaching this function have
        already passed ``get_asset_keys(asset_type=AssetType.Image)``, so the
        key spelling carries no additional information worth filtering on.
        """
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        elevation = "elevation_sentinel"
        all_assets = [("elevation", elevation)]

        parents, overviews = _classify_assets(all_assets)

        assert parents == {"elevation": elevation}
        assert overviews == {}

    def test_non_image_prefixed_key_with_masks_and_overviews(self):
        """The relaxed parent rule still excludes masks and groups overviews.

        Guards the catch-all ``else`` branch: broadening what counts as a parent
        must not let a mask or an overview fall through into ``parents``.
        """
        from aws.osml.io.virtualizarr_parsers import _classify_assets

        elevation = "elevation_sentinel"
        ovr1 = "overview_1_sentinel"
        mask = "mask_sentinel"
        all_assets = [
            ("elevation", elevation),
            ("image:0", "image_0_sentinel"),
            ("image:0:overview:1", ovr1),
            ("image:0:mask", mask),
        ]

        parents, overviews = _classify_assets(all_assets)

        assert parents == {"elevation": elevation, "image:0": "image_0_sentinel"}
        assert overviews == {"image:0": [(1, ovr1)]}


class TestNoIndexableSegments:
    """A URL yielding no indexable image assets reports that, in those terms."""

    def test_empty_parents_raises_intended_message(self, tmp_dir, monkeypatch):
        """No parent assets → the "no indexable image segments" ValueError.

        Regression guard: this condition used to flow into a bare ``max()`` over
        an empty dict, so the failure surfaced as ``max() iterable argument is
        empty`` — naming neither the file nor the problem — and the library's
        own message for the case was unreachable.

        Every fixture in ``data/unit/`` has at least one image asset, so the
        state is induced by stubbing the classifier rather than with a crafted
        file.
        """
        import aws.osml.io.virtualizarr_parsers as vp

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=64, num_rows=64, num_bands=1)

        monkeypatch.setattr(vp, "_classify_assets", lambda _assets: ({}, {}))

        with pytest.raises(ValueError, match="No indexable image segments"):
            vp.OversightMLParser()(str(path))

    def test_empty_parents_error_names_the_url(self, tmp_dir, monkeypatch):
        """The error identifies which URL had nothing to index."""
        import aws.osml.io.virtualizarr_parsers as vp

        path = tmp_dir / "test.ntf"
        _write_nitf(path, num_cols=64, num_rows=64, num_bands=1)

        monkeypatch.setattr(vp, "_classify_assets", lambda _assets: ({}, {}))

        with pytest.raises(ValueError) as excinfo:
            vp.OversightMLParser()(str(path))

        assert "test.ntf" in str(excinfo.value)


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

        parser = OversightMLParser()
        store = parser(str(path_base))

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

        parser = OversightMLParser()
        store = parser(str(path_base))

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

        url_base = _chunk_url(path_base)
        url_r1 = _chunk_url(path_r1)
        parser = OversightMLParser()
        store = parser(str(path_base))

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

        parser = OversightMLParser()
        store = parser(str(path_base))

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

    def test_sparse_rset_levels_produce_correct_levels(self, tmp_dir):
        """Sparse R-set levels (.r1 and .r3, no .r2) map to correct levels.

        Companions are auto-discovered from the base URL, so the on-disk level
        numbers (1, 3) — not their discovery order — determine the overview
        levels.
        """
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path_base = tmp_dir / "img.ntf"
        path_r1 = tmp_dir / "img.ntf.r1"
        path_r3 = tmp_dir / "img.ntf.r3"
        _write_nitf(path_base, num_cols=512, num_rows=512, num_bands=1)
        _write_nitf(path_r1, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r3, num_cols=128, num_rows=128, num_bands=1)

        parser = OversightMLParser()
        store = parser(str(path_base))

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
        url_r1 = _chunk_url(path_r1)
        url_r3 = _chunk_url(path_r3)
        for _key, entry in group.groups["1"].arrays["data"].manifest.items():
            assert entry["path"] == url_r1, (
                f"Level 1 (from .r1) should use r1 URL, got '{entry['path']}'"
            )
        for _key, entry in group.groups["2"].arrays["data"].manifest.items():
            assert entry["path"] == url_r3, (
                f"Level 2 (from .r3) should use r3 URL, got '{entry['path']}'"
            )


class TestSingleFileBackwardCompat:
    """Verify single-file, no-overview path produces a hierarchical store with one level.

    Requirements: 3.4, 7.1, 7.2
    """

    def test_single_file_produces_hierarchical_store_with_one_level(self, tmp_dir):
        """A single NITF file with single path and URL produces a hierarchical
        store with subgroup '0', GeoZarr multiscales metadata with one layout
        entry, and correct source."""
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        path = tmp_dir / "single.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        url = str(path)
        parser = OversightMLParser()
        store = parser(str(path))

        group = store._group

        # Has subgroup "0" with a "data" array
        assert "0" in group.groups, (
            f"Expected subgroup '0', got {list(group.groups.keys())}"
        )
        assert "data" in group.groups["0"].arrays, (
            f"Expected 'data' in subgroup '0' arrays, got {list(group.groups['0'].arrays.keys())}"
        )

        # GeoZarr multiscales metadata present with single layout entry
        attrs = group.metadata.attributes if group.metadata else {}
        assert "multiscales" in attrs, (
            "Expected 'multiscales' attribute for single-file input"
        )
        assert "zarr_conventions" in attrs, (
            "Expected 'zarr_conventions' attribute for single-file input"
        )
        layout = attrs["multiscales"]["layout"]
        assert len(layout) == 1, (
            f"Expected 1 layout entry for single-file input, got {len(layout)}"
        )
        assert layout[0]["asset"] == "0"
        assert layout[0]["transform"]["scale"] == [1.0, 1.0]

        # Source attribute matches the URL
        assert attrs.get("source") == url, (
            f"Expected source='{url}', got '{attrs.get('source')}'"
        )


INTERLEAVED_J2K = Path("data/unit/j2k-128x128-1band-8bit-rpcl-interleaved.j2k")
"""Fixture whose tile-parts interleave across tiles, so every tile is multi-range.

Generated by ``scripts/generate_test_data.py``
(:func:`generate_j2k_rpcl_interleaved`), which encodes with ``opj_compress -p RPCL
-TP R`` and then permutes the tile-parts into resolution-major order.  Encoder
flags alone cannot produce this layout: ``opj_compress`` groups each tile's
tile-parts contiguously, and a contiguous run coalesces back to one range.
"""

INTERLEAVED_J2K_SEED = 1234
"""Seed ``generate_j2k_rpcl_interleaved`` draws the fixture's pixels from.

The encode is lossless, so the fixture's decoded pixels equal this array exactly.
Duplicated from the generator rather than imported because ``scripts/`` is not an
importable package.
"""


def _interleaved_expected_pixels() -> np.ndarray:
    """The exact pixels ``INTERLEAVED_J2K`` decodes to, from its generator seed."""
    rng = np.random.RandomState(INTERLEAVED_J2K_SEED)
    return rng.randint(0, 256, (128, 128)).astype(np.uint8).reshape(1, 128, 128)


class TestInterleavedTilePartFixture:
    """The interleaved fixture really is multi-range, and stays that way.

    Nothing else in the repo reaches the multi-range reference path end to end.
    The near neighbour ``j2k-128x128-1band-8bit-rpcl-3tileparts.j2k`` reads like it
    does — three tile-parts per tile — but ``opj_compress`` writes those parts
    *contiguously*, so ``_are_contiguous`` collapses each tile back to a single
    range and ``multi_range_refs`` comes out empty.

    These assertions are deliberately exact.  A regeneration that coalesced back to
    contiguous would silently disarm every multi-range test downstream, which is
    how the gap arose in the first place, so the shape of the reference set is
    pinned rather than merely checked for non-emptiness.
    """

    def test_fixture_exists(self):
        assert INTERLEAVED_J2K.exists(), (
            f"{INTERLEAVED_J2K} is missing; regenerate it with "
            "'python scripts/generate_test_data.py' (needs opj_compress)"
        )

    def test_four_chunks_of_three_non_contiguous_ranges(self):
        """4 multi-range refs, 3 ranges each, none of them contiguous."""
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            _are_contiguous,
        )

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        refs = store.multi_range_refs

        assert sorted(refs) == [
            "0/data/0.0.0", "0/data/0.0.1", "0/data/0.1.0", "0/data/0.1.1",
        ], f"expected 4 multi-range chunk refs, got {sorted(refs)}"

        for key, (url, ranges) in sorted(refs.items()):
            assert url.endswith(INTERLEAVED_J2K.name)
            assert len(ranges) == 3, (
                f"{key}: expected 3 ranges, got {len(ranges)}: {ranges}"
            )
            assert not _are_contiguous([tuple(r) for r in ranges]), (
                f"{key}: ranges are contiguous, so the fixture no longer "
                f"exercises the multi-range path: {ranges}"
            )

    def test_contiguous_neighbour_yields_no_multi_range_refs(self):
        """The control: the 3tileparts fixture produces *no* multi-range refs.

        Pins the distinction the two filenames make, so neither can be
        substituted for the other by mistake.
        """
        from aws.osml.io.virtualizarr_parsers import OversightMLParser

        neighbour = Path("data/unit/j2k-128x128-1band-8bit-rpcl-3tileparts.j2k")
        if not neighbour.exists():
            pytest.skip(f"fixture missing: {neighbour}")

        store = OversightMLParser()(str(neighbour.resolve()))
        assert store.multi_range_refs == {}, (
            "3tileparts fixture unexpectedly produced multi-range refs; it is the "
            "contiguous control and the interleaved fixture is the multi-range one"
        )

    def test_decodes_losslessly_to_its_generated_pixels(self):
        """``imread`` returns the generator's array exactly.

        Permuting tile-parts is a byte-level rewrite, so this is what establishes
        that the permutation produced a *legal* codestream rather than one that
        merely parses.  Lossless because the generator's ``opj_compress`` invocation
        passes no rate or quality target.
        """
        from aws.osml.io import imread

        actual = imread(str(INTERLEAVED_J2K))
        assert actual.shape == (1, 128, 128), f"got {actual.shape}"
        np.testing.assert_array_equal(
            actual, _interleaved_expected_pixels(),
            err_msg="interleaved fixture no longer decodes to its generated pixels",
        )


class TestParquetEmitter:
    """This library writes the Kerchunk Parquet container itself.

    ``LazyReferenceMapper.write()`` builds a hardcoded four-column DataFrame with
    no extension seam, so the emitter had to become ours in order to widen the
    schema.  What stays upstream's is the *layout* — ``.zmetadata`` plus one
    ``{field}/refs.{record}.parq`` per record, chunks placed positionally — and
    these tests pin that agreement, since a placement disagreement does not fail
    loudly: it misfiles references and the store reads back as other chunks'
    pixels.
    """

    def _refs_and_index(self, tmp_dir, source, name):
        """Write both sinks from one parse; return ``(refs, parquet_root)``.

        The JSON container carries the reference mapping verbatim, so it is the
        reference against which the positional Parquet store is checked.
        """
        import json

        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        store = OversightMLParser()(str(source))
        json_path = tmp_dir / f"{name}.tile_index.json"
        parquet_path = tmp_dir / f"{name}.tile_index.parquet"
        write_tile_index(store, str(json_path))
        write_tile_index(store, str(parquet_path))
        with open(json_path) as handle:
            refs = json.load(handle)["refs"]
        return refs, parquet_path

    @staticmethod
    def _chunk_keys(refs):
        from aws.osml.io.virtualizarr_parsers import _is_parquet_meta_key

        return sorted(k for k in refs if not _is_parquet_meta_key(k))

    def test_row_placement_agrees_with_upstream_key_to_record(self, tmp_dir):
        """Our ``(record, row)`` equals ``LazyReferenceMapper._key_to_record``.

        The invariant the whole container rests on: a chunk key *is* its row
        number, and the reader resolves keys with upstream's version of this
        arithmetic.  Checked over a multi-dimensional grid (a 256x256 image in
        128x128 tiles, so ``(1, 2, 2)``) because a 1x1 grid agrees trivially.
        """
        import fsspec
        from aws.osml.io.virtualizarr_parsers import (
            _parquet_grid_shapes,
            _parquet_record_placement,
        )
        from fsspec.implementations.reference import LazyReferenceMapper

        source = Path("data/unit/tiff-256x256-1band-8bit-tiled-deflate.tif")
        if not source.exists():
            pytest.skip(f"fixture missing: {source}")

        refs, parquet_path = self._refs_and_index(
            tmp_dir, source.resolve(), "tiled"
        )
        chunk_keys = self._chunk_keys(refs)
        assert len(chunk_keys) == 4, f"expected a 2x2 tile grid, got {chunk_keys}"

        ref_fs, root = fsspec.core.url_to_fs(str(parquet_path))
        upstream = LazyReferenceMapper(root, fs=ref_fs, engine="pyarrow")
        grid_shapes = _parquet_grid_shapes(upstream.zmetadata)
        assert grid_shapes == {"0/data": [1, 2, 2]}, grid_shapes

        for key in chunk_keys:
            _, record, row = _parquet_record_placement(key, grid_shapes)
            want_record, want_row, _ = upstream._key_to_record(key)
            assert (record, row) == (want_record, want_row), (
                f"{key}: placed at record {record} row {row}, but the reader "
                f"looks it up at record {want_record} row {want_row}"
            )

    def test_upstream_reader_finds_every_reference_we_wrote(self, tmp_dir):
        """A stock ``LazyReferenceMapper`` resolves each key to the JSON value.

        The operational form of the placement invariant: not "the arithmetic
        matches" but "upstream's reader, pointed at our store, returns exactly the
        reference the JSON sink recorded for the same key".
        """
        import fsspec
        from fsspec.implementations.reference import LazyReferenceMapper

        source = Path("data/unit/tiff-256x256-1band-8bit-tiled-deflate.tif")
        if not source.exists():
            pytest.skip(f"fixture missing: {source}")

        refs, parquet_path = self._refs_and_index(
            tmp_dir, source.resolve(), "tiled"
        )
        ref_fs, root = fsspec.core.url_to_fs(str(parquet_path))
        upstream = LazyReferenceMapper(root, fs=ref_fs, engine="pyarrow")

        for key in self._chunk_keys(refs):
            url, offset, size = refs[key]
            assert upstream[key] == [url, offset, size], (
                f"{key}: store holds {upstream[key]!r}, JSON says "
                f"{[url, offset, size]!r}"
            )

    def test_multi_field_pyramid_places_each_level_independently(self, tmp_dir):
        """Two pyramid levels are two fields, each with its own row space."""
        import fsspec
        from fsspec.implementations.reference import LazyReferenceMapper

        for suffix, size in ((".r1", 64), ("", 256)):
            path = tmp_dir / f"image.ntf{suffix}"
            _write_nitf(
                path, num_cols=size, num_rows=size, num_bands=1,
                block_width=size // 2, block_height=size // 2,
            )

        refs, parquet_path = self._refs_and_index(
            tmp_dir, (tmp_dir / "image.ntf").resolve(), "pyramid"
        )
        ref_fs, root = fsspec.core.url_to_fs(str(parquet_path))
        upstream = LazyReferenceMapper(root, fs=ref_fs, engine="pyarrow")
        assert upstream.listdir() >= {"0/data", "1/data"}

        for key in self._chunk_keys(refs):
            assert upstream[key] == refs[key], f"{key} misplaced"

    def test_does_not_route_through_lazy_reference_mapper(self, tmp_dir, monkeypatch):
        """The write path no longer touches ``create`` / ``__setitem__`` / ``flush``.

        Asserted by breaking all three: if the emitter still drove the upstream
        mapper, the write would raise instead of producing a store.
        """
        from fsspec.implementations.reference import LazyReferenceMapper

        def forbidden(*args, **kwargs):
            raise AssertionError(
                "the Parquet emitter still drives LazyReferenceMapper"
            )

        monkeypatch.setattr(LazyReferenceMapper, "create", staticmethod(forbidden))
        monkeypatch.setattr(LazyReferenceMapper, "__setitem__", forbidden)
        monkeypatch.setattr(LazyReferenceMapper, "flush", forbidden)
        monkeypatch.setattr(LazyReferenceMapper, "write", forbidden)

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)
        _, parquet_path = self._refs_and_index(tmp_dir, path, "own")
        assert (parquet_path / ".zmetadata").is_file()
        assert (parquet_path / "0" / "data" / "refs.0.parq").is_file()

    def test_record_table_carries_every_reference_form(self):
        """One record holds single-range, inline and multi-range rows together.

        Asserts the emitter's output shape at the Arrow level, which is where the
        schema contract lives: the four upstream columns in order, then the three
        range columns, and ``path`` null on the multi-range row.
        """
        import pyarrow as pa
        from aws.osml.io.virtualizarr_parsers import _parquet_record_table

        table = _parquet_record_table(
            {
                0: ["file:///img.ntf", 5, 9],
                1: b"\xaa",
                2: ["file:///img.ntf", [[10, 1], [200, 1]]],
            },
            record_size=8,
        )
        assert table.schema.names == [
            "path", "offset", "size", "raw", "range_path", "offsets", "sizes",
        ]
        for name in ("offsets", "sizes"):
            # Checked semantically: in memory Arrow names the element field "item",
            # and a Parquet round-trip renames it "element", so the string form of
            # the type is not stable across the write.
            field_type = table.schema.field(name).type
            assert pa.types.is_list(field_type), field_type
            assert field_type.value_type == pa.int64(), field_type
        assert table.column("path")[0].as_py() == "file:///img.ntf"
        assert table.column("raw")[1].as_py() == b"\xaa"
        assert table.column("path")[2].as_py() is None, (
            "path must be null on a multi-range row"
        )
        assert table.column("offsets")[2].as_py() == [10, 200]
        assert table.column("sizes")[2].as_py() == [1, 1]

    def test_inline_chunks_round_trip(self, tmp_dir):
        """Inline ``raw`` chunk data encodes and reads back.

        Upstream encodes it with ``kerchunk.df._proc_raw``, which was the sole
        reason kerchunk was a runtime dependency of this library; ``_proc_raw`` here
        replaces it.  Both inline spellings are covered: literal bytes, and the
        ``base64:`` prefix the Kerchunk spec uses for binary payloads — which must be
        *decoded* into the column, since the reader hands what it finds straight to
        the codec.
        """
        import json

        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import _emit_refs

        refs = {
            ".zgroup": json.dumps({"zarr_format": 2}),
            ".zattrs": json.dumps({}),
            "0/.zgroup": json.dumps({"zarr_format": 2}),
            "0/data/.zarray": json.dumps({
                "zarr_format": 2, "shape": [4], "chunks": [1], "dtype": "|u1",
                "compressor": None, "filters": None, "fill_value": 0, "order": "C",
            }),
            "0/data/.zattrs": json.dumps({"_ARRAY_DIMENSIONS": ["x"]}),
            "0/data/0": b"\x01",
            "0/data/1": "base64:Ag==",
            "0/data/2": b"\x03",
            "0/data/3": b"base64:BA==",
        }
        output = str(tmp_dir / "inline.parquet")
        _emit_refs(refs, output, ".parquet", use_templates=False)

        fs = MultiReferenceFileSystem(fo=output, skip_instance_cache=True)
        assert [fs.cat(f"0/data/{i}") for i in range(4)] == [
            b"\x01", b"\x02", b"\x03", b"\x04"
        ]
        group = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)
        np.testing.assert_array_equal(
            np.asarray(group["0/data"][:]), np.array([1, 2, 3, 4], dtype=np.uint8)
        )

    def test_record_size_is_recorded_in_the_store(self, tmp_dir):
        """``.zmetadata`` carries the partition size a reader must divide by."""
        import json

        from aws.osml.io.virtualizarr_parsers import _PARQUET_RECORD_SIZE

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)
        _, parquet_path = self._refs_and_index(tmp_dir, path, "recsize")

        metadata = json.loads((parquet_path / ".zmetadata").read_text())
        assert metadata["record_size"] == _PARQUET_RECORD_SIZE == 100_000

    def test_existing_index_at_the_path_is_replaced(self, tmp_dir):
        """Rewriting a path leaves the new store, not a merge of both.

        Matches ``LazyReferenceMapper.create()``: the target directory is removed
        first, so a stale record file from a previous, differently-shaped index
        cannot survive alongside the new one.
        """
        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)
        _, parquet_path = self._refs_and_index(tmp_dir, path, "replaced")

        stale = parquet_path / "0" / "data" / "refs.7.parq"
        stale.write_bytes(b"not a parquet file")
        self._refs_and_index(tmp_dir, path, "replaced")
        assert not stale.exists(), "a stale record file survived the rewrite"

    def test_multi_range_refs_are_written_as_range_columns(self, tmp_dir):
        """Multi-range refs land in the same record file, as list columns.

        This is the write that used to be impossible: it raised
        ``TypeError: int() argument must be ... not 'list'`` from inside fsspec,
        because ``LazyReferenceMapper.write()`` assigned the range list into an
        ``int64`` slot.  The columns land in one file, not a companion — a reader
        must never have to probe for a second file per record.
        """
        import pyarrow as pa
        import pyarrow.parquet as pq
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        if not INTERLEAVED_J2K.exists():
            pytest.skip(f"fixture missing: {INTERLEAVED_J2K}")

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        output = tmp_dir / "mr.tile_index.parquet"
        write_tile_index(store, str(output))

        record = output / "0" / "data" / "refs.0.parq"
        assert sorted(p.name for p in record.parent.iterdir()) == ["refs.0.parq"], (
            "the range columns must live in the record file, not a companion"
        )

        table = pq.read_table(record)
        assert table.schema.names == [
            "path", "offset", "size", "raw", "range_path", "offsets", "sizes",
        ], table.schema.names
        for name in ("offsets", "sizes"):
            field_type = table.schema.field(name).type
            assert pa.types.is_list(field_type), field_type
            assert field_type.value_type == pa.int64(), field_type

        assert table.column("path").null_count == table.num_rows, (
            "path must be null on a multi-range row, so an unaware reader breaks "
            "rather than decoding one fragment as a whole chunk"
        )
        for row in range(4):
            offsets = table.column("offsets")[row].as_py()
            sizes = table.column("sizes")[row].as_py()
            assert table.column("range_path")[row].as_py() is not None
            assert len(offsets) == len(sizes) == 3, (offsets, sizes)

    def test_store_without_multi_range_keeps_the_four_column_schema(self, tmp_dir):
        """The widened schema is opt-in per record, not a format bump.

        An index with no multi-range reference must carry exactly upstream's four
        columns, so adopting the range columns cannot break a reader that only ever
        sees ordinary imagery.
        """
        import pyarrow.parquet as pq

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=256, num_rows=256, num_bands=1,
                    block_width=128, block_height=128)
        _, parquet_path = self._refs_and_index(tmp_dir, path, "plain")

        schema = pq.read_schema(parquet_path / "0" / "data" / "refs.0.parq")
        assert schema.names == ["path", "offset", "size", "raw"], schema.names

    def test_multi_range_json_sink_is_unaffected(self, tmp_dir):
        """The JSON container represents the form natively and still does."""
        import json

        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        if not INTERLEAVED_J2K.exists():
            pytest.skip(f"fixture missing: {INTERLEAVED_J2K}")

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        output = tmp_dir / "mr.tile_index.json"
        write_tile_index(store, str(output))

        with open(output) as handle:
            refs = json.load(handle)["refs"]
        multi = {
            k: v for k, v in refs.items()
            if isinstance(v, list) and len(v) == 2 and isinstance(v[1], list)
        }
        assert len(multi) == 4, f"expected 4 multi-range refs, got {sorted(multi)}"


class TestWriteTileIndex:
    """Verify write_tile_index serializes flat and hierarchical stores correctly.

    Requirements: 6.2, 6.3, 6.4, 6.5, 6.6, 7.3
    """

    def test_flat_store_json_output_structure(self, tmp_dir):
        """A single-file ManifestStore serializes to JSON with hierarchical
        structure (subgroup '0' with 'data' array) and GeoZarr metadata.

        Requirements: 6.2, 7.3
        """
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path = tmp_dir / "flat.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path))

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

        # Single-file .zattrs should contain zarr_conventions and multiscales
        import json as _json
        zattrs = _json.loads(refs[".zattrs"])
        assert "zarr_conventions" in zattrs, (
            "Expected 'zarr_conventions' in single-file store .zattrs"
        )
        assert "multiscales" in zattrs, (
            "Expected 'multiscales' in single-file store .zattrs"
        )
        layout = zattrs["multiscales"]["layout"]
        assert len(layout) == 1, (
            f"Expected 1 layout entry for single-file, got {len(layout)}"
        )

        # Chunk keys should be under 0/data/ prefix
        chunk_keys = [k for k in refs if k.startswith("0/data/") and not k.startswith("0/data/.")]
        assert len(chunk_keys) > 0, "Expected at least one chunk reference key under 0/data/"

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

        # Serialized JSON refs record the URL as passed (base) and the
        # auto-discovered companion (base + ".r1").
        url_base = str(path_base)
        url_r1 = f"{str(path_base)}.r1"
        store = OversightMLParser()(str(path_base))

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
        """A hierarchical ManifestStore serializes to Parquet and reads back.

        Reads the store back rather than asserting only that the output
        directory is non-empty: both of those structural assertions pass on a
        completely unreadable store, which is how the Parquet sink shipped
        write-only.

        Requirements: 6.5
        """
        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r1, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path_base))

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

        # ...and the directory is actually readable, with both pyramid levels
        # present and correctly shaped.
        fs = MultiReferenceFileSystem(fo=output, skip_instance_cache=True)
        root = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)
        assert sorted(root.group_keys()) == ["0", "1"]
        assert root["0/data"].shape == (1, 256, 256)
        assert root["1/data"].shape == (1, 128, 128)

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

        store = OversightMLParser()(str(path_base))

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
        metadata_base["IC"] = "NC"
        metadata_base["IMODE"] = "B"
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
        metadata_r1["IC"] = "NC"
        metadata_r1["IMODE"] = "B"
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

        # 2. Generate hierarchical tile index JSON.  Parsing the local base
        #    path auto-discovers the .r1 companion and produces file:// chunk
        #    refs that fsspec can resolve locally.
        parser = OversightMLParser()
        store = parser(str(path_base))

        output = str(tmp_dir / "pyramid.tile_index.json")
        write_tile_index(store, output)

        # 3. Open the JSON index via fsspec ReferenceFileSystem + zarr
        import fsspec
        from zarr.storage._fsspec import FsspecStore

        fs = fsspec.filesystem(
            "reference", fo=output, skip_instance_cache=True, asynchronous=True
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
        metadata_base["IC"] = "NC"
        metadata_base["IMODE"] = "B"
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
        metadata_r1["IC"] = "NC"
        metadata_r1["IMODE"] = "B"
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


class TestPortableIndex:
    """Verify portable index creation via ``write_tile_index(template_base=…)``.

    Relocating chunk references is a serialization-time concern: passing
    ``template_base="{{base}}"`` rewrites each chunk-reference URL to
    ``{{base}}filename`` and emits a Kerchunk v1 "templates" dict with
    ``{"base": ""}``.  At read time, ``template_overrides`` resolves the
    placeholders.
    """

    def test_template_base_uses_basename_only(self, tmp_dir):
        """template_base rewrites chunk refs to {{base}}filename (basename only)."""
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        subdir = tmp_dir / "deep" / "nested"
        subdir.mkdir(parents=True)
        path = subdir / "myfile.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path))
        output = str(tmp_dir / "portable.tile_index.json")
        write_tile_index(store, output, template_base="{{base}}")

        with open(output) as f:
            data = json.load(f)
        refs = data["refs"]
        chunk_keys = [
            k for k in refs
            if k.startswith("0/data/") and not k.split("/")[-1].startswith(".")
        ]
        assert chunk_keys
        for k in chunk_keys:
            assert refs[k][0] == "{{base}}myfile.ntf", (
                f"Expected '{{{{base}}}}myfile.ntf', got '{refs[k][0]}'"
            )

    def test_flat_json_includes_templates_dict(self, tmp_dir):
        """Flat store JSON output includes "templates": {"base": ""}."""
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path))
        output = str(tmp_dir / "portable.tile_index.json")
        write_tile_index(store, output, template_base="{{base}}")

        with open(output) as f:
            data = json.load(f)

        assert "templates" in data, "Expected 'templates' key in portable JSON"
        assert data["templates"] == {"base": ""}, (
            f"Expected templates={{'base': ''}}, got {data['templates']}"
        )

        # Chunk refs should contain {{base}} prefix
        refs = data.get("refs", data)
        chunk_keys = [k for k in refs if not k.startswith(".")]
        assert len(chunk_keys) > 0
        for k in chunk_keys:
            ref = refs[k]
            if isinstance(ref, list):
                assert "{{base}}" in ref[0], (
                    f"Chunk ref URL should contain '{{{{base}}}}', got '{ref[0]}'"
                )

    def test_hierarchical_json_includes_templates_dict(self, tmp_dir):
        """Hierarchical store JSON output includes "templates": {"base": ""}."""
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path_base = tmp_dir / "image.ntf"
        path_r1 = tmp_dir / "image.ntf.r1"
        _write_nitf(path_base, num_cols=256, num_rows=256, num_bands=1)
        _write_nitf(path_r1, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path_base))
        output = str(tmp_dir / "portable_hier.tile_index.json")
        write_tile_index(store, output, template_base="{{base}}")

        with open(output) as f:
            data = json.load(f)

        assert "templates" in data, "Expected 'templates' key in portable JSON"
        assert data["templates"] == {"base": ""}, (
            f"Expected templates={{'base': ''}}, got {data['templates']}"
        )

        # Level 0 refs should use {{base}}image.ntf
        # Level 1 refs should use {{base}}image.ntf.r1
        refs = data["refs"]
        data_keys_0 = [
            k for k in refs
            if k.startswith("0/data/") and not k.startswith("0/data/.")
        ]
        data_keys_1 = [
            k for k in refs
            if k.startswith("1/data/") and not k.startswith("1/data/.")
        ]
        assert len(data_keys_0) > 0
        assert len(data_keys_1) > 0

        for k in data_keys_0:
            ref = refs[k]
            assert ref[0] == "{{base}}image.ntf", (
                f"Level 0 ref should be '{{{{base}}}}image.ntf', got '{ref[0]}'"
            )
        for k in data_keys_1:
            ref = refs[k]
            assert ref[0] == "{{base}}image.ntf.r1", (
                f"Level 1 ref should be '{{{{base}}}}image.ntf.r1', got '{ref[0]}'"
            )

    def test_absolute_url_does_not_include_templates(self, tmp_dir):
        """Without template_base, no templates dict is emitted."""
        import json

        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        path = tmp_dir / "image.ntf"
        _write_nitf(path, num_cols=128, num_rows=128, num_bands=1)

        store = OversightMLParser()(str(path))
        output = str(tmp_dir / "absolute.tile_index.json")
        write_tile_index(store, output)

        with open(output) as f:
            data = json.load(f)

        assert "templates" not in data, (
            "Non-portable index should not contain 'templates'"
        )

    def test_parquet_index_reads_back_with_pixel_parity(self, tmp_dir):
        """A Parquet index of a compressed fixture reads back matching ``IO.open``.

        Complements the parametrized round-trip above, which covers raw NITF-NC
        written in-test: this one runs the real ``data/unit/`` fixtures through
        the JPEG 2000, JPEG, and deflate-TIFF codecs, so a Parquet-specific
        chunk-reference defect cannot hide behind an uncompressed layout.
        """
        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        from tests.property.zarr.test_v3_producer import (
            _assert_tiles_match_lossy,
            _read_all_tiles_via_index,
            _read_all_tiles_via_io,
        )

        fixtures = [
            "nitf21-256x256-3band-8bit-nc.ntf",
            "nitf21-64x64-3band-8bit-j2k.ntf",
            "nitf21-64x64-3band-8bit-jpeg.ntf",
            "tiff-256x256-1band-8bit-tiled-deflate.tif",
        ]
        for name in fixtures:
            src = Path("data/unit") / name
            if not src.exists():
                pytest.skip(f"fixture missing: {src}")

            # The parser requires an absolute path.
            store = OversightMLParser()(str(src.resolve()))
            index_path = tmp_dir / f"{name}.tile_index.parquet"
            write_tile_index(store, str(index_path))

            _assert_tiles_match_lossy(
                _read_all_tiles_via_io(src),
                _read_all_tiles_via_index(index_path, skip_instance_cache=True),
                label=f"parquet index ({name})",
            )

    def test_parquet_multi_resolution_pyramid_pixel_parity(self, tmp_dir):
        """Both pyramid levels of a Parquet index read back with pixel parity.

        This is the coverage that justifies preserving the hierarchy in Parquet
        rather than flattening to a single resolution: it asserts the pyramid
        survives the round-trip, level by level, against the pixels written.
        """
        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

        rng = np.random.default_rng(7)
        levels = {}
        for suffix, size in (("", 256), (".r1", 64)):
            path = tmp_dir / f"image.ntf{suffix}"
            metadata = BufferedMetadataProvider()
            metadata["IC"] = "NC"
            metadata["IMODE"] = "B"
            provider = BufferedImageAssetProvider.create(
                key="image:0",
                num_columns=size,
                num_rows=size,
                num_bands=3,
                block_width=size,
                block_height=size,
                metadata=metadata,
            )
            data = rng.integers(1, 255, size=(3, size, size), dtype=np.uint8)
            provider.set_full_image(data)
            writer = IO.open([str(path)], "w", "nitf")
            writer.add_asset("image:0", provider, "Image", "test", ["data"])
            writer.close()
            levels[suffix] = data

        store = OversightMLParser()(str((tmp_dir / "image.ntf").resolve()))
        index_path = str(tmp_dir / "pyramid.tile_index.parquet")
        write_tile_index(store, index_path)

        fs = MultiReferenceFileSystem(fo=index_path, skip_instance_cache=True)
        root = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)

        assert "multiscales" in dict(root.attrs), (
            "multiscales metadata missing from Parquet index root attrs"
        )
        assert sorted(root.group_keys()) == ["0", "1"]

        for level, suffix in (("0", ""), ("1", ".r1")):
            expected = levels[suffix]
            actual = np.asarray(root[f"{level}/data"][:])
            assert actual.shape == expected.shape, (
                f"level {level}: expected {expected.shape}, got {actual.shape}"
            )
            np.testing.assert_array_equal(
                actual, expected,
                err_msg=f"level {level} pixels differ in the Parquet index",
            )

    @pytest.mark.parametrize("sink", ["json", "parquet"])
    def test_portable_index_round_trip_with_template_overrides(self, tmp_dir, sink):
        """End-to-end: create portable index, open with template_overrides,
        verify chunk **pixels** match what was written.

        Parametrized over both sinks so they cannot drift in coverage: JSON and
        Parquet share every upstream stage (parsing, refs building, URL
        relocation) and diverge only here, which is exactly the stage a
        JSON-only test cannot reach.

        Asserts values, not just ``tile.shape`` — a shape assertion passes on a
        store full of fill_value zeros.
        """
        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        # Write a NITF with known data
        rng = np.random.default_rng(42)
        path = tmp_dir / "image.ntf"
        metadata = BufferedMetadataProvider()
        metadata["IC"] = "NC"
        metadata["IMODE"] = "B"
        provider = BufferedImageAssetProvider.create(
            key="image:0",
            num_columns=128,
            num_rows=128,
            num_bands=1,
            block_width=128,
            block_height=128,
            metadata=metadata,
        )
        data = rng.integers(1, 255, size=(1, 128, 128), dtype=np.uint8)
        provider.set_full_image(data)
        writer = IO.open([str(path)], "w", "nitf")
        writer.add_asset("image:0", provider, "Image", "test", ["data"])
        writer.close()

        # Create portable index (template_base rewrites refs to {{base}}filename)
        store = OversightMLParser()(str(path))
        index_path = str(tmp_dir / f"image.tile_index.{sink}")
        write_tile_index(store, index_path, template_base="{{base}}")

        if sink == "json":
            # Only the JSON container can carry the templates dict; a Parquet
            # store has nowhere to put one, so its {{base}} placeholders are
            # resolved from template_overrides alone.
            import json

            with open(index_path) as f:
                assert json.load(f)["templates"] == {"base": ""}

        # Open with template_overrides pointing to the local directory
        fs = MultiReferenceFileSystem(
            fo=index_path,
            template_overrides={"base": str(tmp_dir) + "/"},
            skip_instance_cache=True,
        )
        root = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)

        tile = np.asarray(root["0/data"][0:1, 0:128, 0:128])
        assert tile.shape == (1, 128, 128), (
            f"Expected (1, 128, 128), got {tile.shape}"
        )
        np.testing.assert_array_equal(
            tile, data,
            err_msg=f"{sink} index pixels differ from what was written",
        )


class TestInterleavedMultiRangeSinkParity:
    """Both containers deliver the same pixels for a multi-range index.

    This is the coverage that was missing when Parquet multi-range writes were
    impossible.  Sink-parametrized round-trips already existed, but every fixture
    behind them was single-range — including a J2K one named ``3tileparts``, whose
    tile-parts ``opj_compress`` writes contiguously — so JSON coverage stood in for
    Parquet coverage over the one reference form that distinguishes the two.

    Parity is asserted losslessly: both routes decode the same compressed bytes
    with the same decoder, so a tolerance would mask real divergence.
    """

    @pytest.fixture(autouse=True)
    def _require_fixture(self):
        if not INTERLEAVED_J2K.exists():
            pytest.fail(
                f"{INTERLEAVED_J2K} is missing; regenerate it with "
                "'python scripts/generate_test_data.py' (needs opj_compress). "
                "Skipping here would silently drop the only multi-range coverage."
            )

    @pytest.mark.parametrize("sink", ["json", "parquet"])
    def test_index_reads_back_with_pixel_parity(self, tmp_dir, sink):
        """Every tile matches ``IO.open`` exactly, through either container."""
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        from tests.property.zarr.test_v3_producer import (
            _assert_tiles_match_lossless,
            _read_all_tiles_via_index,
            _read_all_tiles_via_io,
        )

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        assert len(store.multi_range_refs) == 4, (
            "fixture stopped producing multi-range refs; this test would then "
            "pass without exercising the path it exists for"
        )

        index_path = tmp_dir / f"interleaved.tile_index.{sink}"
        write_tile_index(store, str(index_path))

        _assert_tiles_match_lossless(
            _read_all_tiles_via_io(INTERLEAVED_J2K),
            _read_all_tiles_via_index(index_path, skip_instance_cache=True),
            label=f"{sink} multi-range index",
        )

    @pytest.mark.parametrize("sink", ["json", "parquet"])
    def test_pixels_match_the_generated_array(self, tmp_dir, sink):
        """Absolute check, independent of ``IO.open``.

        The fixture's encode is lossless, so the index must reproduce the exact
        array the generator drew from its seed.  Comparing only against
        ``IO.open`` would pass if both routes shared a decode defect.
        """
        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        index_path = str(tmp_dir / f"absolute.tile_index.{sink}")
        write_tile_index(store, index_path)

        fs = MultiReferenceFileSystem(fo=index_path, skip_instance_cache=True)
        root = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)
        np.testing.assert_array_equal(
            np.asarray(root["0/data"][:]),
            _interleaved_expected_pixels(),
            err_msg=f"{sink} multi-range index does not reproduce the fixture pixels",
        )

    @pytest.mark.parametrize("sink", ["json", "parquet"])
    def test_portable_index_resolves_multi_range_refs(self, tmp_dir, sink):
        """``template_base`` + ``template_overrides`` works for multi-range chunks.

        The case that catches an unexpanded ``range_path``: in Parquet the URL of a
        multi-range chunk lives in its own column, so expanding only ``path`` would
        leave ``{{base}}`` literal in exactly the entries a portable index of RPCL
        imagery consists of.
        """
        import shutil as _shutil

        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        # Copy the fixture so {{base}} has to resolve somewhere other than the
        # directory the index was built from.
        served = tmp_dir / "served"
        served.mkdir()
        _shutil.copy(INTERLEAVED_J2K, served / INTERLEAVED_J2K.name)

        store = OversightMLParser()(str((served / INTERLEAVED_J2K.name).resolve()))
        index_path = str(tmp_dir / f"portable.tile_index.{sink}")
        write_tile_index(store, index_path, template_base="{{base}}")

        fs = MultiReferenceFileSystem(
            fo=index_path,
            template_overrides={"base": str(served) + "/"},
            skip_instance_cache=True,
        )
        root = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)
        np.testing.assert_array_equal(
            np.asarray(root["0/data"][:]),
            _interleaved_expected_pixels(),
            err_msg=f"portable {sink} index mis-resolved a multi-range chunk URL",
        )

    @pytest.mark.parametrize("sink", ["json", "parquet"])
    def test_async_fetch_matches_sync(self, tmp_dir, sink):
        """The async multi-range fetch returns the same bytes as the sync one.

        ``_fetch_multi_range_async`` and ``_fetch_multi_range_sync`` are separate
        implementations — one gathers concurrently, the other loops — and ``fs.cat``
        reaches only the sync one, so the async entry point is called directly.
        Concurrent fetches complete out of order, which is exactly how a
        concatenation-order bug would show up here and not in the sync path.
        """
        import asyncio

        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        index_path = str(tmp_dir / f"async.tile_index.{sink}")
        write_tile_index(store, index_path)

        fs = MultiReferenceFileSystem(fo=index_path, skip_instance_cache=True)
        keys = ["0/data/0.0.0", "0/data/0.0.1", "0/data/0.1.0", "0/data/0.1.1"]
        for key in keys:
            assert fs._is_multi_range(fs.references[key]), (
                f"{key} is not multi-range in the {sink} index"
            )
            assert asyncio.run(fs._cat_file(key)) == fs.cat(key), (
                f"async and sync fetches disagree for {key} in the {sink} index"
            )

    @pytest.mark.parametrize("sink", ["json", "parquet"])
    def test_zarr_async_store_reads_multi_range(self, tmp_dir, sink):
        """zarr's ``FsspecStore`` — the notebook's path — decodes the array.

        Goes through zarr's async batched pipeline rather than the synchronous
        ``get_mapper`` route the other tests use, so both consumer paths in the
        user guide are covered for multi-range chunks.
        """
        import zarr
        from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem
        from aws.osml.io.virtualizarr_parsers import (
            OversightMLParser,
            write_tile_index,
        )

        store = OversightMLParser()(str(INTERLEAVED_J2K.resolve()))
        index_path = str(tmp_dir / f"fsspecstore.tile_index.{sink}")
        write_tile_index(store, index_path)

        # ``asynchronous=True`` is what ``FsspecStore`` expects; without it zarr
        # warns, and the warning is the only thing separating this from the
        # synchronous route.
        fs = MultiReferenceFileSystem(
            fo=index_path, skip_instance_cache=True, asynchronous=True
        )
        root = zarr.open_group(
            zarr.storage.FsspecStore(fs, path=""), mode="r", zarr_format=2
        )
        np.testing.assert_array_equal(
            np.asarray(root["0/data"][:]),
            _interleaved_expected_pixels(),
            err_msg=f"FsspecStore read of the {sink} index differs",
        )
