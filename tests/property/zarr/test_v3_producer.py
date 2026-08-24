"""Property-based tests for the native Zarr v3 tile-index producer.

``write_tile_index(..., zarr_format=3)`` emits a **reference-based** native v3
store: a Kerchunk-container reference file whose keys are ``zarr.json`` documents
and ``c.<band>.<row>.<col>`` chunk keys, served through fsspec.  Nothing is
down-converted to Zarr v2 on the way out — the in-memory
:class:`ArrayV3Metadata` that ``_build_manifest_array`` already builds (with
``codecs=[BytesCodec(), <custom codec>]``) is serialized straight through.

Why this suite exists separately from ``test_v3_pipeline.py``: that suite
hand-builds a materialized v3 store to test the *codecs*, which means the store
layout under test is the test's own construction.  This suite reads a **shipped
artifact** produced by library code, so the producer's metadata, chunk-key
encoding, and reference forms are what get exercised.  A producer bug that the
hand-built harness cannot see — a wrong chunk key, a lost codec config, a
multi-range entry serialized in the v2 key form — fails here.

Coverage:
1. Round-trip parity per codec: write an image, produce a v3 reference index,
   read it back through ``zarr.open_group`` / ``xarray.open_zarr`` over fsspec,
   and compare tiles against ``IO.open()``.
2. Producer surface guards: chunk-key encoding, v3 metadata shape, multi-range
   reference preservation, URL relocation, segment filtering, and the Parquet
   rejection.
3. v2 non-regression: the same store serialized both ways describes the same
   pixels, so adding the v3 path did not perturb the v2 one.

Feature: virtualizarr-migration
"""

import json
import shutil
import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import (
    IO,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)
from hypothesis import assume, given

from ..conftest import pbt_settings
from ..quality import MIN_PSNR_DB, MIN_SSIM, calculate_psnr, calculate_ssim
from ..strategies import (
    jpeg_image_for_compression,
    realistic_image_for_compression,
    tiff_writable_image,
)

zarr = pytest.importorskip("zarr", minversion="3.0")
fsspec = pytest.importorskip("fsspec")
pytest.importorskip("virtualizarr")

# Registers the codecs with numcodecs (the v2 comparison path needs it); the v3
# path resolves them by URI through the ``zarr.codecs`` entry points instead.
import aws.osml.io.zarr_codecs  # noqa: E402, F401
from aws.osml.io.multi_reference_fs import MultiReferenceFileSystem  # noqa: E402
from aws.osml.io.virtualizarr_parsers import (  # noqa: E402
    OversightMLParser,
    write_tile_index,
)

# DTED is not writable via IO.open, so its producer coverage uses a checked-in
# fixture (mirrors test_v3_pipeline.py).
DATA_DIR = Path("data/unit")
DTED_FIXTURE = DATA_DIR / "dted-16x16-1band-int16.dt1"

# ---------------------------------------------------------------------------
# Writers (mirrors test_v3_pipeline.py / test_end_to_end.py)
# ---------------------------------------------------------------------------


def _write_nitf(
    array: np.ndarray,
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    metadata_hints: dict,
    block_width: int = 64,
    block_height: int = 64,
) -> Path:
    """Write a NITF file and return the path. Caller must clean up."""
    with tempfile.NamedTemporaryFile(suffix=".ntf", delete=False) as f:
        path = Path(f.name)

    metadata = BufferedMetadataProvider()
    for k, v in metadata_hints.items():
        metadata[k] = v

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, block_width),
        block_height=min(num_rows, block_height),
        pixel_type=pixel_type,
        metadata=metadata,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "nitf")
    writer.add_asset(
        key="image:0",
        provider=provider,
        title="Test Image",
        description="Zarr v3 producer test",
        roles=["data"],
    )
    writer.close()
    return path


def _write_tiff(
    array: np.ndarray,
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
    hints: dict,
) -> Path:
    """Write a TIFF file and return the path. Caller must clean up."""
    with tempfile.NamedTemporaryFile(suffix=".tif", delete=False) as f:
        path = Path(f.name)

    metadata = BufferedMetadataProvider()
    for k, v in hints.items():
        metadata[k] = v

    tile_w = int(hints.get("322", "256"))
    tile_h = int(hints.get("323", "256"))

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, tile_w),
        block_height=min(num_rows, tile_h),
        pixel_type=pixel_type,
        metadata=metadata,
    )
    provider.set_full_image(array)

    writer = IO.open([str(path)], "w", "tiff")
    writer.metadata = metadata
    writer.add_asset(
        key="image:0",
        provider=provider,
        title="Test Image",
        description="Zarr v3 producer test",
        roles=["data"],
    )
    writer.close()
    return path


# ---------------------------------------------------------------------------
# Producer / consumer helpers
# ---------------------------------------------------------------------------


def _produce_index(src_path: Path, output: Path, **kwargs) -> None:
    """Run the real producer: parse *src_path*, serialize an index to *output*.

    Chunk references must carry an absolute path (VirtualiZarr requires a URI or
    absolute posix path), which is what ``generate_tile_index.py`` does too.
    """
    store = OversightMLParser()(str(src_path.resolve()))
    write_tile_index(store, str(output), **kwargs)


def _read_all_tiles_via_io(path: Path) -> dict:
    """Read all tiles from a file via IO.open, returning (row, col) → ndarray."""
    tiles = {}
    with IO.open([str(path)], "r") as reader:
        keys = reader.get_asset_keys(asset_type=AssetType.Image)
        asset = reader.get_asset(keys[0])
        grid_rows, grid_cols = asset.block_grid_size
        for r in range(grid_rows):
            for c in range(grid_cols):
                tiles[(r, c)] = asset.get_block(r, c, 0)
    return tiles


def _read_all_tiles_via_index(index_path: Path, **fs_kwargs) -> dict:
    """Read all tiles from a reference index via fsspec + ``zarr.open_group``.

    ``MultiReferenceFileSystem`` is used rather than plain ``ReferenceFileSystem``
    because it is what the user guide prescribes and what handles the multi-range
    entries interleaved J2K tile-parts produce.  Level 0 (full resolution) lives
    at ``0/data`` under the GeoZarr multiscales layout, the same access path as
    the v2 index.

    Returns (row, col) → ndarray, with edge tiles trimmed to the array boundary.
    """
    fs = MultiReferenceFileSystem(fo=str(index_path), **fs_kwargs)
    root = zarr.open_group(fs.get_mapper(""), mode="r")

    arr = root["0/data"]
    _, tile_h, tile_w = arr.chunks
    _, total_rows, total_cols = arr.shape

    grid_rows = (total_rows + tile_h - 1) // tile_h
    grid_cols = (total_cols + tile_w - 1) // tile_w

    tiles = {}
    for r in range(grid_rows):
        for c in range(grid_cols):
            row_start, col_start = r * tile_h, c * tile_w
            row_end = min(row_start + tile_h, total_rows)
            col_end = min(col_start + tile_w, total_cols)
            tiles[(r, c)] = np.asarray(arr[:, row_start:row_end, col_start:col_end])

    assert len(tiles) > 0, "No tiles read from the index"
    return tiles


def _assert_tiles_match_lossless(tiles_io, tiles_idx, label="v3 index"):
    """Assert all tiles match exactly, trimming to the overlapping region."""
    assert len(tiles_io) > 0, "No tiles from IO path"
    assert len(tiles_idx) > 0, f"No tiles from {label} path"
    assert set(tiles_io.keys()) == set(tiles_idx.keys()), (
        f"Tile coordinate mismatch between IO and {label} paths. "
        f"IO: {sorted(tiles_io.keys())}, {label}: {sorted(tiles_idx.keys())}"
    )
    for coord in sorted(tiles_io.keys()):
        io_tile, idx_tile = tiles_io[coord], tiles_idx[coord]
        assert io_tile.size > 0, f"IO tile {coord} is empty"
        assert idx_tile.size > 0, f"{label} tile {coord} is empty"
        b = min(io_tile.shape[0], idx_tile.shape[0])
        h = min(io_tile.shape[1], idx_tile.shape[1])
        w = min(io_tile.shape[2], idx_tile.shape[2])
        assert b > 0 and h > 0 and w > 0, (
            f"Tile {coord} has zero overlap: IO={io_tile.shape}, {label}={idx_tile.shape}"
        )
        np.testing.assert_array_equal(
            idx_tile[:b, :h, :w],
            io_tile[:b, :h, :w],
            err_msg=f"Tile {coord} differs between IO and {label} paths",
        )


def _assert_tiles_match_lossy(tiles_io, tiles_idx, label="v3 index"):
    """Assert tiles agree, allowing PSNR/SSIM tolerance for lossy codecs.

    Both paths decode the same compressed bytes with the same decoder, so exact
    equality is the norm; the quality thresholds are the fallback.
    """
    assert len(tiles_io) > 0, "No tiles from IO path"
    assert len(tiles_idx) > 0, f"No tiles from {label} path"
    assert set(tiles_io.keys()) == set(tiles_idx.keys()), (
        f"Tile coordinate mismatch between IO and {label} paths. "
        f"IO: {sorted(tiles_io.keys())}, {label}: {sorted(tiles_idx.keys())}"
    )
    for coord in sorted(tiles_io.keys()):
        io_tile, idx_tile = tiles_io[coord], tiles_idx[coord]
        assert io_tile.size > 0, f"IO tile {coord} is empty"
        assert idx_tile.size > 0, f"{label} tile {coord} is empty"
        b = min(io_tile.shape[0], idx_tile.shape[0])
        h = min(io_tile.shape[1], idx_tile.shape[1])
        w = min(io_tile.shape[2], idx_tile.shape[2])
        io_trimmed = io_tile[:b, :h, :w]
        idx_trimmed = idx_tile[:b, :h, :w]
        assert io_trimmed.dtype == idx_trimmed.dtype, (
            f"Tile {coord} dtype mismatch: IO={io_trimmed.dtype}, "
            f"{label}={idx_trimmed.dtype}"
        )
        if not np.array_equal(io_trimmed, idx_trimmed):
            psnr = calculate_psnr(io_trimmed, idx_trimmed, use_actual_range=True)
            ssim = calculate_ssim(io_trimmed, idx_trimmed)
            assert psnr >= MIN_PSNR_DB, (
                f"Tile {coord} PSNR {psnr:.2f} dB below threshold {MIN_PSNR_DB} dB"
            )
            assert ssim >= MIN_SSIM, (
                f"Tile {coord} SSIM {ssim:.4f} below threshold {MIN_SSIM}"
            )


def _run_producer_parity(src_path: Path, *, lossy: bool) -> dict:
    """Produce a v3 index from *src_path* and assert tile parity with ``IO.open()``.

    Returns the index's refs dict so a caller can additionally assert on the
    artifact's own shape without re-running the producer.
    """
    tiles_io = _read_all_tiles_via_io(src_path)
    with tempfile.TemporaryDirectory() as tmp:
        index_path = Path(tmp) / "index.json"
        _produce_index(src_path, index_path, zarr_format=3)
        tiles_idx = _read_all_tiles_via_index(index_path)
        if lossy:
            _assert_tiles_match_lossy(tiles_io, tiles_idx)
        else:
            _assert_tiles_match_lossless(tiles_io, tiles_idx)
        return json.loads(index_path.read_text())["refs"]


def _assert_is_native_v3(refs: dict) -> None:
    """Assert *refs* really is a native v3 store and carries no v2 keys.

    Without this, a producer that silently fell back to the v2 layout would still
    pass the parity tests — the v2 index reads correctly too, just through
    numcodecs instead of the v3 pipeline.  Asserting the *absence* of ``.zarray``
    / ``.zgroup`` / ``.zattrs`` is what pins the test to the path it names.
    """
    v2_keys = [k for k in refs if k.rsplit("/", 1)[-1].startswith(".z")]
    assert not v2_keys, (
        f"v3 index contains Zarr v2 metadata keys {v2_keys} — the producer fell "
        f"back to the v2 layout, which reads through numcodecs, not the v3 pipeline"
    )

    root = json.loads(refs["zarr.json"])
    assert root["zarr_format"] == 3 and root["node_type"] == "group", root

    array_meta_keys = [
        k for k in refs
        if k.endswith("/zarr.json") and json.loads(refs[k]).get("node_type") == "array"
    ]
    assert array_meta_keys, f"no v3 array metadata in the index: {sorted(refs)}"
    for key in array_meta_keys:
        meta = json.loads(refs[key])
        assert meta["zarr_format"] == 3, meta
        codec_names = [c["name"] for c in meta["codecs"]]
        assert codec_names[0] == "bytes", (
            f"{key}: first codec must be the ArrayBytesCodec, got {codec_names}"
        )

    chunk_keys = [k for k in refs if k.rsplit("/", 1)[-1].startswith("c.")]
    assert chunk_keys, f"no v3 chunk keys (c.<band>.<row>.<col>) in {sorted(refs)}"


# ---------------------------------------------------------------------------
# Round-trip parity, per codec
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestV3ProducerRoundTrip:
    """A produced v3 reference index reads back the pixels ``IO.open()`` sees.

    One case per codec the producer can attach.
    """

    @given(realistic_image_for_compression(min_size=48, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_nc_producer_round_trip(self, image_tuple):
        """JbpBlockCodec: uncompressed NITF through a produced v3 index."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=32, block_height=32,
        )
        try:
            refs = _run_producer_parity(path, lossy=False)
            _assert_is_native_v3(refs)
        finally:
            path.unlink(missing_ok=True)

    @given(realistic_image_for_compression(min_size=64, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_c8_producer_round_trip(self, image_tuple):
        """Jpeg2000Codec: NITF IC=C8 through a produced v3 index.

        Dimensions are block-aligned to avoid partial-tile J2K encoding issues;
        edge-tile padding parity between the two decode routes is covered in
        ``test_v3_pipeline.py``.
        """
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        block_size = 32
        num_rows = (num_rows // block_size) * block_size
        num_cols = (num_cols // block_size) * block_size
        assume(num_rows >= block_size and num_cols >= block_size)
        array = np.ascontiguousarray(array[:, :num_rows, :num_cols])

        decomp_levels = min(5, max(1, int(np.floor(np.log2(block_size))) - 1))

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={
                "IC": "C8",
                "COMRAT": "02.0",
                "J2K_DECOMPOSITION_LEVELS": str(decomp_levels),
            },
            block_width=block_size, block_height=block_size,
        )
        try:
            refs = _run_producer_parity(path, lossy=True)
            _assert_is_native_v3(refs)
        finally:
            path.unlink(missing_ok=True)

    @given(jpeg_image_for_compression(min_size=64, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_c3_producer_round_trip(self, image_tuple):
        """JpegCodec: NITF IC=C3 through a produced v3 index."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        block_size = 32
        num_rows = (num_rows // block_size) * block_size
        num_cols = (num_cols // block_size) * block_size
        assume(num_rows >= block_size and num_cols >= block_size)
        array = np.ascontiguousarray(array[:, :num_rows, :num_cols])

        path = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "C3", "COMRAT": "75.0"},
            block_width=block_size, block_height=block_size,
        )
        try:
            refs = _run_producer_parity(path, lossy=True)
            _assert_is_native_v3(refs)
        finally:
            path.unlink(missing_ok=True)

    @given(tiff_writable_image(min_size=48, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_tiff_producer_round_trip(self, image_tuple):
        """TiffTileCodec: TIFF through a produced v3 index.

        Covers the full lossless matrix — ``{1..3} bands × {uncompressed, LZW,
        Deflate} × {chunky, planar}`` — over every writer pixel type.  Only lossy
        JPEG-in-TIFF is excluded; the JPEG codec's parity is covered by the
        NITF-C3 case above, which compares with PSNR/SSIM rather than exact
        equality.
        """
        array, pixel_type, num_bands, num_rows, num_cols, hints = image_tuple
        assume(hints["259"] != 7)  # JPEG TIFF is lossy; NITF-C3 covers the JPEG codec.

        path = _write_tiff(array, pixel_type, num_bands, num_rows, num_cols, hints)
        try:
            refs = _run_producer_parity(path, lossy=False)
            _assert_is_native_v3(refs)
        finally:
            path.unlink(missing_ok=True)

    @pytest.mark.parametrize("zarr_format", [2, 3])
    def test_dted_producer_round_trip(self, zarr_format):
        """DtedTileCodec: DTED through a produced index, in both Zarr formats.

        Uses the checked-in fixture rather than a Hypothesis strategy because
        DTED is not writable via ``IO.open`` — a limitation independent of
        indexability.  The fixture is copied into a temp dir so the produced
        index references a path this test owns.

        Both formats are asserted because the defect that kept DTED out of this
        suite lived in the *parser*, upstream of either serializer: DTED assets
        are keyed ``elevation`` rather than ``image:N``, and the asset classifier
        required the ``image:`` prefix, so no segments were indexed and the
        parser raised ``max() iterable argument is empty`` on both paths.
        """
        if not DTED_FIXTURE.exists():
            pytest.skip("DTED test fixture not available")

        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / DTED_FIXTURE.name
            shutil.copy(DTED_FIXTURE, src)

            tiles_io = _read_all_tiles_via_io(src)
            index_path = Path(tmp) / f"index-v{zarr_format}.json"
            _produce_index(src, index_path, zarr_format=zarr_format)

            tiles_idx = _read_all_tiles_via_index(index_path)
            _assert_tiles_match_lossless(tiles_io, tiles_idx, f"v{zarr_format} index")

            if zarr_format == 3:
                _assert_is_native_v3(json.loads(index_path.read_text())["refs"])


# ---------------------------------------------------------------------------
# Producer artifact surface
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestV3ProducerArtifact:
    """The produced artifact's own structure, not just the pixels it yields.

    Parity tests can pass while the artifact is subtly wrong in ways that only
    surface for another consumer (a different chunk-key separator, a dropped
    codec config, a group node missing its multiscales attributes).  These
    assertions pin the serialized form.
    """

    @pytest.fixture
    def nitf_src(self):
        """A 2×2-tile 3-band uncompressed NITF."""
        rng = np.random.default_rng(0)
        array = rng.integers(0, 255, size=(3, 128, 128), dtype=np.uint8)
        path = _write_nitf(
            array, PixelType.UInt8, 3, 128, 128,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=64, block_height=64,
        )
        yield path
        path.unlink(missing_ok=True)

    def _refs(self, src: Path, tmp_path: Path, **kwargs) -> dict:
        index_path = tmp_path / "index.json"
        _produce_index(src, index_path, zarr_format=3, **kwargs)
        return json.loads(index_path.read_text())["refs"]

    def test_chunk_keys_use_v3_encoding(self, nitf_src, tmp_path):
        """Chunk keys are ``c.<band>.<row>.<col>``, matching the array's encoding.

        The key is derived from the array metadata's own
        ``chunk_key_encoding.encode_chunk_key``, so this asserts that derivation
        produced what the ``default``/``.``-separator encoding declared in the
        same metadata document requires.  A mismatch reads back as ``fill_value``
        — plausible-looking zeros, not an error.
        """
        refs = self._refs(nitf_src, tmp_path)

        meta = json.loads(refs["0/data/zarr.json"])
        assert meta["chunk_key_encoding"]["name"] == "default"
        separator = meta["chunk_key_encoding"].get("configuration", {}).get(
            "separator", "/"
        )
        assert separator == ".", (
            f"expected the '.' separator this producer emits, got {separator!r}"
        )

        chunk_keys = {k for k in refs if k.startswith("0/data/c")}
        expected = {
            f"0/data/c.0.{r}.{c}" for r in range(2) for c in range(2)
        }
        assert chunk_keys == expected, (
            f"chunk keys {sorted(chunk_keys)} do not match the 2×2 grid the "
            f"declared encoding requires: {sorted(expected)}"
        )

    def test_array_metadata_preserves_codec_config(self, nitf_src, tmp_path):
        """The custom codec's full config survives into the array's ``zarr.json``.

        The v3 path does not go through ``dataset_to_kerchunk_refs``, so the codec
        configuration is serialized by this producer's own code.  A dropped field
        would leave a codec that resolves but decodes wrongly.
        """
        refs = self._refs(nitf_src, tmp_path)
        meta = json.loads(refs["0/data/zarr.json"])

        codecs = meta["codecs"]
        assert [c["name"] for c in codecs][0] == "bytes"
        custom = codecs[-1]
        assert custom["name"].startswith("https://"), (
            f"the custom codec must be referenced by its entry-point URI so a cold "
            f"zarr can resolve it, got {custom['name']!r}"
        )
        config = custom["configuration"]
        assert config == {
            "num_bands": 3,
            "block_height": 64,
            "block_width": 64,
            "nbpp": 8,
            "imode": "B",
            "pvtype": "INT",
        }, config

    def test_root_group_carries_multiscales_attributes(self, nitf_src, tmp_path):
        """GeoZarr multiscales metadata lands in the root ``zarr.json`` attributes.

        In a v2 index these live in ``.zattrs``; a v3 group node carries them
        inside its ``zarr.json``.  Losing them in translation would strip the
        pyramid description the layout depends on.
        """
        refs = self._refs(nitf_src, tmp_path)
        attrs = json.loads(refs["zarr.json"])["attributes"]

        assert attrs["multiscales"]["layout"] == [
            {"asset": "0", "transform": {"scale": [1.0, 1.0], "translation": [0.0, 0.0]}}
        ]
        assert [c["name"] for c in attrs["zarr_conventions"]] == ["multiscales"]
        assert attrs["source"] == str(nitf_src.resolve())

    def test_group_nodes_exist_for_every_level(self, nitf_src, tmp_path):
        """Every level has an explicit v3 group node.

        Zarr v3 requires a ``zarr.json`` at each group in the path; a reference
        filesystem serves only the keys the index names, so an implicit
        (unwritten) group would make ``root["0/data"]`` unreachable.
        """
        refs = self._refs(nitf_src, tmp_path)
        assert "zarr.json" in refs
        assert "0/zarr.json" in refs
        assert json.loads(refs["0/zarr.json"])["node_type"] == "group"

    def test_url_relocation_applies_to_v3_keys(self, nitf_src, tmp_path):
        """``template_base`` rewrites v3 chunk refs and the root ``source``.

        Relocation is shared code between the two layouts, but it runs against
        differently-keyed dicts.  This checks the v3 keys are actually visited —
        a filter written for ``.zarray``-style keys would silently skip them and
        emit an index with absolute local paths baked in.
        """
        index_path = tmp_path / "portable.json"
        _produce_index(
            nitf_src, index_path, zarr_format=3, template_base="{{base}}"
        )
        doc = json.loads(index_path.read_text())

        assert doc["templates"] == {"base": ""}
        refs = doc["refs"]
        for key in (k for k in refs if k.startswith("0/data/c")):
            url = refs[key][0]
            assert url == f"{{{{base}}}}{nitf_src.name}", (
                f"chunk ref {key} was not relocated: {url}"
            )
        assert json.loads(refs["zarr.json"])["attributes"]["source"] == (
            f"{{{{base}}}}{nitf_src.name}"
        )

        # And the relocated index still reads, with the base resolved at read time.
        tiles_io = _read_all_tiles_via_io(nitf_src)
        tiles_idx = _read_all_tiles_via_index(
            index_path,
            template_overrides={"base": f"{nitf_src.parent}/"},
        )
        _assert_tiles_match_lossless(tiles_io, tiles_idx, "portable v3 index")

    def test_segment_filtering_drops_other_levels(self, nitf_src, tmp_path):
        """``segments`` restricts which levels the v3 index describes."""
        refs = self._refs(nitf_src, tmp_path, segments=["0"])
        assert {k for k in refs if k.endswith("zarr.json")} == {
            "zarr.json", "0/zarr.json", "0/data/zarr.json"
        }

        with pytest.raises(ValueError, match="Subgroup"):
            self._refs(nitf_src, tmp_path, segments=["nope"])

    def test_parquet_rejected_for_v3(self, nitf_src, tmp_path):
        """Parquet + v3 raises rather than writing an unreadable index.

        The behavior is a deferral, not a structural limit, and the message must say
        so: a v3 ``zarr.json`` does carry the shape and chunk shape that the
        container's positional indexing needs, just not under the v2 names, so what
        blocks it is missing read-side support.  The message is asserted because it
        is the only place a user learns which of those two it is.
        """
        store = OversightMLParser()(str(nitf_src.resolve()))
        with pytest.raises(ValueError, match="Parquet output is not supported"):
            write_tile_index(store, str(tmp_path / "x.parquet"), zarr_format=3)

        with pytest.raises(ValueError, match="not implemented"):
            write_tile_index(store, str(tmp_path / "x.parquet"), zarr_format=3)

    def test_v3_parquet_rejection_does_not_claim_impossibility(
        self, nitf_src, tmp_path
    ):
        """The message must not say a v3 store lacks what positional indexing needs.

        It used to assert that a v3 store "does not have" the ``.zarray``
        ``shape``/``chunks`` the container indexes by.  A v3 store has both, under
        different names, so the claim was false and pointed a reader away from work
        that is actually bounded.
        """
        store = OversightMLParser()(str(nitf_src.resolve()))
        with pytest.raises(ValueError) as excinfo:
            write_tile_index(store, str(tmp_path / "x.parquet"), zarr_format=3)

        message = str(excinfo.value)
        assert "does not have" not in message, message
        assert "zarr.json" in message, (
            "the message should name what a v3 store carries instead of .zarray"
        )

    @pytest.mark.parametrize("bad_format", [0, 1, 4, "3", None])
    def test_invalid_zarr_format_rejected(self, nitf_src, tmp_path, bad_format):
        """Only 2 and 3 are accepted; anything else raises."""
        store = OversightMLParser()(str(nitf_src.resolve()))
        with pytest.raises(ValueError, match="zarr_format must be 2 or 3"):
            write_tile_index(
                store, str(tmp_path / "x.json"), zarr_format=bad_format
            )


# ---------------------------------------------------------------------------
# Multi-range references — the reference form VirtualiZarr does not support
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestV3ProducerMultiRange:
    """Non-contiguous chunks keep their multi-range form in a v3 index.

    Multi-range entries are accumulated at parse time under the *v2-flavored*
    chunk key (``0/data/0.0.0``), so the v3 producer has to translate the key
    while preserving the value.  Getting that wrong is invisible to a
    contiguous-chunk test: the placeholder single-range entry covers only the
    chunk's first fragment, so the chunk decodes from truncated bytes.
    """

    def test_multi_range_entries_translated_to_v3_keys(self, tmp_path):
        """A synthetic multi-range ref lands under the v3 chunk key, intact.

        Whether a given image *produces* interleaved tile-parts depends on the
        J2K encoder's progression order, which the writer does not let a test
        pin.  Injecting the entry directly is what makes the translation itself
        testable rather than dependent on encoder behavior.
        """
        rng = np.random.default_rng(0)
        array = rng.integers(0, 255, size=(1, 64, 64), dtype=np.uint8)
        src = _write_nitf(
            array, PixelType.UInt8, 1, 64, 64,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=32, block_height=32,
        )
        try:
            store = OversightMLParser()(str(src.resolve()))
            arr = store._group.groups["0"].arrays["data"]
            entry = arr.manifest.dict()["0.0.1"]
            # Split one chunk's single range into two adjacent halves.  The bytes
            # are identical, so a correct translation still decodes correctly —
            # the point is that the *form* survives.
            half = entry["length"] // 2
            store.multi_range_refs["0/data/0.0.1"] = [
                entry["path"],
                [
                    [entry["offset"], half],
                    [entry["offset"] + half, entry["length"] - half],
                ],
            ]

            index_path = tmp_path / "multi.json"
            write_tile_index(store, str(index_path), zarr_format=3)
            refs = json.loads(index_path.read_text())["refs"]

            # No v2-keyed leftover, and the v3 key holds the multi-range form.
            assert "0/data/0.0.1" not in refs, (
                "the multi-range entry was written under its v2 key, so the v3 "
                "chunk key still points at the truncated placeholder range"
            )
            v3_entry = refs["0/data/c.0.0.1"]
            assert isinstance(v3_entry[1], list), (
                f"expected the multi-range form [url, [[o, l], ...]], got {v3_entry}"
            )
            assert sum(ln for _, ln in v3_entry[1]) == entry["length"]

            # Chunks that were not injected keep the single-range
            # ``[url, offset, length]`` form — the translation must not convert
            # every entry to the multi-range shape.
            for coord in ("0.0.0", "0.1.0", "0.1.1"):
                other = refs[f"0/data/c.{coord}"]
                assert len(other) == 3 and isinstance(other[1], int), (
                    f"chunk c.{coord} should be single-range, got {other}"
                )

            tiles_io = _read_all_tiles_via_io(src)
            tiles_idx = _read_all_tiles_via_index(index_path)
            _assert_tiles_match_lossless(tiles_io, tiles_idx, "multi-range v3 index")
        finally:
            src.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# xarray consumer — the native-v3 read flow with no Kerchunk-to-v2 layer
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestV3ProducerXarray:
    """``xarray.open_zarr`` reads a produced v3 index.

    This is the flow the user guide's native-v3 example shows, and it exercises
    the store through a second consumer — one that lists keys and consults
    ``dimension_names`` rather than indexing a known path.
    """

    def test_xarray_open_zarr(self, tmp_path):
        """A produced v3 index opens as an xarray Dataset with named dimensions."""
        xr = pytest.importorskip("xarray")

        rng = np.random.default_rng(0)
        array = rng.integers(0, 255, size=(3, 128, 128), dtype=np.uint8)
        src = _write_nitf(
            array, PixelType.UInt8, 3, 128, 128,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=64, block_height=64,
        )
        try:
            index_path = tmp_path / "index.json"
            _produce_index(src, index_path, zarr_format=3)

            fs = MultiReferenceFileSystem(fo=str(index_path))
            # The level-0 subgroup is opened directly: fsspec's FSMap carries its
            # own root, so xarray's ``group=`` argument is not usable with it.
            ds = xr.open_zarr(
                fs.get_mapper("0"), consolidated=False, zarr_format=3
            )

            assert ds["data"].dims == ("bands", "y", "x")
            assert ds["data"].shape == (3, 128, 128)
            np.testing.assert_array_equal(np.asarray(ds["data"]), array)
        finally:
            src.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# v2 non-regression
# ---------------------------------------------------------------------------


@pytest.mark.property
class TestV2Unaffected:
    """The v2 producer still emits the v2 layout and the same pixels.

    Phase 4 refactored the serializer into shared stages (segment filtering, URL
    relocation, the JSON/Parquet sink) plus a per-format refs builder.  These
    tests assert the v2 output is unchanged in both structure and content — the
    existing ``test_end_to_end.py`` suite covers v2 pixels, this pins the two
    layouts against *each other* so a shared-stage regression cannot pass by
    breaking both identically.
    """

    @given(realistic_image_for_compression(min_size=48, max_size=128, min_bands=1, max_bands=3))
    @pbt_settings
    def test_v2_and_v3_indexes_describe_the_same_pixels(self, image_tuple):
        """The same store serialized both ways reads back identical tiles."""
        array, pixel_type, num_bands, num_rows, num_cols = image_tuple

        src = _write_nitf(
            array, pixel_type, num_bands, num_rows, num_cols,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=32, block_height=32,
        )
        try:
            store = OversightMLParser()(str(src.resolve()))
            with tempfile.TemporaryDirectory() as tmp:
                v2_path = Path(tmp) / "v2.json"
                v3_path = Path(tmp) / "v3.json"
                write_tile_index(store, str(v2_path), zarr_format=2)
                write_tile_index(store, str(v3_path), zarr_format=3)

                tiles_v2 = _read_all_tiles_via_index(v2_path)
                tiles_v3 = _read_all_tiles_via_index(v3_path)
                _assert_tiles_match_lossless(tiles_v2, tiles_v3, "v3 index")
        finally:
            src.unlink(missing_ok=True)

    def test_v2_default_still_emits_v2_layout(self, tmp_path):
        """Omitting ``zarr_format`` yields the v2 keys, not the v3 ones.

        The default must stay 2: the shipping consumer path
        (``ReferenceFileSystem`` + numcodecs) reads v2 keys, so flipping the
        default would break every existing index reader.
        """
        rng = np.random.default_rng(0)
        array = rng.integers(0, 255, size=(1, 64, 64), dtype=np.uint8)
        src = _write_nitf(
            array, PixelType.UInt8, 1, 64, 64,
            metadata_hints={"IC": "NC", "IMODE": "B"},
            block_width=32, block_height=32,
        )
        try:
            index_path = tmp_path / "default.json"
            _produce_index(src, index_path)
            refs = json.loads(index_path.read_text())["refs"]

            assert refs[".zgroup"] == json.dumps({"zarr_format": 2})
            assert ".zattrs" in refs
            assert "0/data/.zarray" in refs
            assert "zarr.json" not in refs
            assert not [k for k in refs if k.rsplit("/", 1)[-1].startswith("c.")]
        finally:
            src.unlink(missing_ok=True)
