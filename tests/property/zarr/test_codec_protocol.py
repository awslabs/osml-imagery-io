"""Protocol and registration guard tests for the dual-protocol zarr codecs.

Where ``test_v3_pipeline.py`` compares *pixels* between the two decode routes,
this suite guards the three pieces of plumbing that decide whether a codec is
reached at all:

1. **The discriminator** (``_is_numcodecs_buffer``).  Every codec in
   ``zarr_codecs.py`` defines a synchronous single-buffer ``decode``, which
   shadows the inherited asynchronous batched ``BytesBytesCodec.decode`` the zarr
   v3 pipeline calls.  ``decode`` therefore routes on this predicate at runtime,
   and a misroute is total breakage in one direction or the other — the v3
   pipeline handing a whole batch to the payload decoder (the failure commit
   ``daa4a0f`` fixed), or a real buffer being deferred to the async path.  The
   unit suite pins seven hand-picked cases; this generalizes them with Hypothesis.

2. **Entry-point discovery.**  Building metadata from ``codec.to_dict()`` with the
   class already imported does not prove URI → entry point → class resolution.
   Only a *cold* interpreter does, so those tests run in a subprocess that
   asserts ``aws.osml.io.zarr_codecs`` is absent from ``sys.modules`` before
   resolution and present after.  This is what validates the
   ``[project.entry-points."zarr.codecs"]`` table in ``pyproject.toml``.

3. **Config serialization.**  A codec reaches a consumer as a dict in
   ``zarr.json`` (``to_dict``/``from_dict``) or ``.zarray`` (``get_config``/
   ``from_config``), never as a live object.  A field dropped or renamed on
   either side yields a codec that is constructible but decodes differently, so
   the round-trips are asserted to reproduce identical *pixels*, not merely
   equal dicts.

Feature: virtualizarr-migration
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io import IO, AssetType
from hypothesis import assume, given
from hypothesis import strategies as st

from ..conftest import pbt_settings

# Require zarr for all tests in this module.
zarr = pytest.importorskip("zarr", minversion="3.0")

import aws.osml.io.zarr_codecs  # noqa: E402, F401
from aws.osml.io.virtualizarr_parsers import _build_codec_instance  # noqa: E402
from aws.osml.io.zarr_codecs import (  # noqa: E402
    DtedTileCodec,
    JbpBlockCodec,
    Jpeg2000Codec,
    JpegCodec,
    TiffTileCodec,
    _is_numcodecs_buffer,
)
from zarr.core.array_spec import ArrayConfig, ArraySpec  # noqa: E402
from zarr.core.buffer import default_buffer_prototype  # noqa: E402
from zarr.core.dtype import parse_dtype  # noqa: E402

CODEC_MODULE = "aws.osml.io.zarr_codecs"
CODEC_URI_PREFIX = "https://awslabs.github.io/osml-imagery-io/codecs/"

# Every registered codec: URI slug → class name, as the entry-point table declares it.
REGISTERED_CODECS = {
    "jpeg2000": "Jpeg2000Codec",
    "jpeg": "JpegCodec",
    "jbp-block": "JbpBlockCodec",
    "tiff-tile": "TiffTileCodec",
    "dted": "DtedTileCodec",
}

DATA_DIR = Path("data/unit")

# One checked-in fixture per codec, so the round-trip and entry-point tests run on
# real artifacts (real codestreams, real configs from ``_build_codec_instance``)
# rather than hand-written configuration dicts.
CODEC_FIXTURES = {
    "jbp-block": DATA_DIR / "nitf21-256x256-3band-8bit-nc.ntf",
    "jpeg2000": DATA_DIR / "nitf21-64x64-3band-8bit-j2k.ntf",
    "jpeg": DATA_DIR / "nitf21-64x64-3band-8bit-jpeg.ntf",
    "tiff-tile": DATA_DIR / "tiff-256x256-1band-8bit-tiled-deflate.tif",
    "dted": DATA_DIR / "dted-16x16-1band-int16.dt1",
}


# ---------------------------------------------------------------------------
# 1. Discriminator robustness (design test 3)
# ---------------------------------------------------------------------------


@st.composite
def numcodecs_buffers(draw):
    """A single buffer, as the numcodecs filter protocol passes to ``decode(buf)``.

    ``ndarray`` comes first because it is what the shipping v2 consumer path
    actually hands over: fsspec ``ReferenceFileSystem`` + ``zarr.open_group``
    reaches ``decode`` with a NumPy array, not ``bytes``.  The byte-like forms are
    included because the codecs are also called directly (tests, and any consumer
    using numcodecs without zarr).
    """
    kind = draw(st.sampled_from(["bytes", "bytearray", "memoryview", "ndarray", "ndarray-memoryview"]))
    dtype = draw(st.sampled_from([np.uint8, np.uint16, np.int16, np.int32, np.float32, np.float64]))
    # 0-d is included deliberately: it has ``__array__`` but no length, which is the
    # kind of shape a length- or iteration-based discriminator would misjudge.
    shape = draw(
        st.one_of(
            st.just(()),
            st.integers(min_value=0, max_value=64).map(lambda n: (n,)),
            st.tuples(
                st.integers(min_value=1, max_value=4),
                st.integers(min_value=1, max_value=8),
                st.integers(min_value=1, max_value=8),
            ),
        )
    )
    arr = np.zeros(shape, dtype=dtype)
    if kind == "ndarray":
        return arr
    if kind == "ndarray-memoryview":
        return memoryview(arr)
    raw = arr.tobytes()
    if kind == "bytes":
        return raw
    if kind == "bytearray":
        return bytearray(raw)
    return memoryview(raw)


@st.composite
def v3_decode_batches(draw):
    """A batch of ``(buffer, ArraySpec)`` pairs, as ``BytesBytesCodec.decode`` receives.

    Pairs are built from real ``zarr`` types — ``prototype.buffer`` and ``ArraySpec``
    — because that is what ``BatchedCodecPipeline.decode_batch`` constructs; a batch
    of ``(bytes, None)`` stand-ins would be an easier target than the real thing.
    The empty batch is included: it is the case ``daa4a0f`` first reproduced the bug
    with, and the one where a misroute raises rather than silently corrupting.
    """
    count = draw(st.integers(min_value=0, max_value=3))
    container = draw(st.sampled_from(["list", "tuple", "generator", "zip"]))
    prototype = default_buffer_prototype()

    pairs = []
    for _ in range(count):
        nbytes = draw(st.integers(min_value=0, max_value=32))
        spec = ArraySpec(
            shape=(1, 2, 2),
            dtype=parse_dtype(np.dtype("uint8"), zarr_format=3),
            fill_value=0,
            config=ArrayConfig.from_dict({}),
            prototype=prototype,
        )
        pairs.append((prototype.buffer.from_bytes(b"\x00" * nbytes), spec))

    if container == "list":
        return pairs
    if container == "tuple":
        return tuple(pairs)
    if container == "generator":
        return (pair for pair in pairs)
    return zip([p[0] for p in pairs], [p[1] for p in pairs])


@pytest.mark.property
class TestIsNumcodecsBufferProperty:
    """``_is_numcodecs_buffer`` separates the two calling conventions.

    ``TestIsNumcodecsBuffer`` in ``tests/unit/test_zarr_codecs.py`` pins four buffer
    types and three batch types by hand.  These properties generalize both sides —
    buffer dtype/shape/container across the forms a numcodecs consumer produces, and
    batch length/container across the forms zarr's pipeline produces — because the
    predicate is a single branch guarding every ``decode`` call in the module.
    """

    @given(numcodecs_buffers())
    @pbt_settings
    def test_single_buffers_are_recognized(self, buf):
        """Any single buffer is the numcodecs protocol, whatever its dtype or shape."""
        assert _is_numcodecs_buffer(buf), (
            f"{type(buf).__name__} of shape "
            f"{getattr(buf, 'shape', len(buf) if hasattr(buf, '__len__') else '?')} "
            f"was not recognized as a single buffer, so a numcodecs decode(buf) call "
            f"would be deferred to the async batched path and never return pixels"
        )

    @given(v3_decode_batches())
    @pbt_settings
    def test_decode_batches_are_not_buffers(self, batch):
        """A batch of ``(buffer, spec)`` pairs is never mistaken for a buffer.

        This is the direction that broke before ``daa4a0f``: the batch reached the
        single-buffer shim, which handed the whole iterable to the Rust decoder.
        """
        assert not _is_numcodecs_buffer(batch), (
            f"a {type(batch).__name__} batch of (buffer, spec) pairs was classified as "
            f"a single buffer; the v3 pipeline's decode() would be routed into the "
            f"numcodecs shim and the batch handed to the payload decoder"
        )

    @pytest.mark.parametrize("codec", [
        Jpeg2000Codec(),
        JpegCodec(bits_per_pixel=8, num_bands=1, block_width=8, block_height=8, imode="B", color_space="MONO"),
        JbpBlockCodec(num_bands=1, block_height=2, block_width=2, nbpp=8, imode="B", pvtype="INT"),
        TiffTileCodec(tile_width=2, tile_height=2),
        DtedTileCodec(),
    ], ids=lambda c: type(c).__name__)
    @given(v3_decode_batches())
    @pbt_settings
    def test_batches_route_to_the_async_path(self, codec, batch):
        """``decode(batch)`` returns an awaitable, not a decoded array.

        The discriminator only matters through its effect on ``decode``, so this
        asserts the routing itself rather than the predicate: a batch must reach the
        inherited coroutine.  Tested per codec because the guard is duplicated in all
        five ``decode`` methods and a copy-paste omission in one would be invisible
        to a test of the predicate alone.
        """
        result = codec.decode(batch)
        assert hasattr(result, "__await__"), (
            f"{type(codec).__name__}.decode(batch) returned "
            f"{type(result).__name__} instead of a coroutine, so the batch was "
            f"decoded by the numcodecs shim rather than the v3 pipeline"
        )
        result.close()  # never awaited; closing avoids an un-awaited-coroutine warning


# ---------------------------------------------------------------------------
# 2. Entry-point discovery from a cold interpreter (design test 5)
# ---------------------------------------------------------------------------

# Runs in a subprocess with nothing from this package imported.  Asserts the codec
# module is absent before resolution (or the test proves nothing), resolves every
# URI through zarr's registry, then reads each store built by the parent and
# compares against pixels the parent read via IO.open().
_COLD_RESOLUTION_CHILD = r"""
import json
import sys

import numpy as np

MODULE = "aws.osml.io.zarr_codecs"
PREFIX = "https://awslabs.github.io/osml-imagery-io/codecs/"

assert MODULE not in sys.modules, (
    "%s was already imported before any codec was resolved; this subprocess "
    "cannot prove entry-point discovery" % MODULE
)

from zarr.registry import get_codec_class

manifest = json.load(open(sys.argv[1]))

for slug, class_name in manifest["registered"].items():
    cls = get_codec_class(PREFIX + slug)
    assert cls.__module__ == MODULE, (slug, "resolved to module", cls.__module__)
    assert cls.__name__ == class_name, (slug, "resolved to class", cls.__name__)

assert MODULE in sys.modules, (
    "resolving a codec URI did not import %s, so the class did not come from the "
    "entry point" % MODULE
)

import zarr

for store in manifest["stores"]:
    got = np.asarray(zarr.open_array(store["path"], mode="r")[:])
    expected = np.load(store["expected"])
    assert got.shape == expected.shape, (store["label"], got.shape, expected.shape)
    np.testing.assert_array_equal(got, expected, err_msg=store["label"])

print("COLD-RESOLUTION-OK")
"""


def _extract_fixture_chunks(src_path):
    """Return ``(codec, shape, chunk_shape, dtype, chunks, expected)`` for a fixture.

    ``chunks`` maps the v3 default chunk-key grid coordinate ``(0, row, col)`` to the
    bytes that chunk's codec receives — taken from ``asset.tile_byte_ranges()``, the
    same source the shipping v2 index uses.  ``expected`` is the full image assembled
    from ``IO.open()`` blocks, an oracle independent of any zarr machinery.

    Only chunky (non-planar) single-chunk-per-tile fixtures are used here, so a tile's
    range list is always fragments of one chunk to concatenate.
    """
    with IO.open([str(src_path)], "r") as reader:
        asset = reader.get_asset(reader.get_asset_keys(asset_type=AssetType.Image)[0])
        codec = _build_codec_instance(asset)
        byte_ranges = asset.tile_byte_ranges()
        num_bands = asset.num_bands
        num_rows = asset.num_rows
        num_cols = asset.num_columns
        block_h = asset.num_pixels_per_block_vertical
        block_w = asset.num_pixels_per_block_horizontal
        dtype = np.dtype(asset.pixel_value_type.to_numpy_dtype())
        grid_rows, grid_cols = asset.block_grid_size
        expected = np.zeros((num_bands, num_rows, num_cols), dtype=dtype)
        for row in range(grid_rows):
            for col in range(grid_cols):
                block = asset.get_block(row, col, 0)
                expected[
                    :,
                    row * block_h:row * block_h + block.shape[1],
                    col * block_w:col * block_w + block.shape[2],
                ] = block

    raw = src_path.read_bytes()
    chunks = {
        (0, row, col): b"".join(raw[offset:offset + length] for offset, length in range_list)
        for (row, col), range_list in byte_ranges.items()
    }
    return codec, (num_bands, num_rows, num_cols), (num_bands, block_h, block_w), dtype, chunks, expected


def _write_v3_store(store_dir, codec, shape, chunk_shape, dtype, chunks):
    """Write a native v3 store whose codec chain names *codec* by its URI.

    The custom codec is serialized via ``to_dict()``, so the store on disk carries
    only a URI plus a configuration dict — nothing that requires the class to be
    importable by the reader ahead of time.  ``BytesCodec`` is told the platform byte
    order because the Rust decoders return native-endian arrays.
    """
    store_dir.mkdir(parents=True, exist_ok=True)
    metadata = {
        "zarr_format": 3,
        "node_type": "array",
        "shape": list(shape),
        "data_type": np.dtype(dtype).name,
        "chunk_grid": {"name": "regular", "configuration": {"chunk_shape": list(chunk_shape)}},
        "chunk_key_encoding": {"name": "default"},
        "fill_value": 0,
        "codecs": [
            {"name": "bytes", "configuration": {"endian": sys.byteorder}},
            codec.to_dict(),
        ],
    }
    (store_dir / "zarr.json").write_text(json.dumps(metadata))
    for (band, row, col), payload in chunks.items():
        chunk_path = store_dir / "c" / str(band) / str(row) / str(col)
        chunk_path.parent.mkdir(parents=True, exist_ok=True)
        chunk_path.write_bytes(payload)


@pytest.mark.property
class TestEntryPointDiscovery:
    """A v3 store opens by URI in an interpreter that never imported the codecs.

    Every other test in the repo has ``aws.osml.io.zarr_codecs`` imported before zarr
    sees a store — including the unit test that builds metadata from
    ``codec.to_dict()`` — so none of them exercise the path a real consumer takes:
    ``zarr.open`` finds an unknown codec name and must locate the class through the
    ``zarr.codecs`` entry-point group.  A broken or missing entry in
    ``pyproject.toml`` is invisible until then.
    """

    def test_uris_resolve_and_stores_read_from_a_cold_interpreter(self, tmp_path):
        """All five URIs resolve, and each codec's store decodes, without a prior import."""
        missing = [slug for slug, path in CODEC_FIXTURES.items() if not path.exists()]
        if missing:
            pytest.skip(f"missing fixtures for {', '.join(sorted(missing))}")

        stores = []
        for slug, fixture in sorted(CODEC_FIXTURES.items()):
            codec, shape, chunk_shape, dtype, chunks, expected = _extract_fixture_chunks(fixture)
            assert codec is not None, f"{fixture} attaches no codec, so it cannot exercise {slug}"
            assert codec.codec_name == CODEC_URI_PREFIX + slug, (
                f"{fixture} maps to {codec.codec_name}, expected the {slug} codec"
            )

            store_dir = tmp_path / slug
            _write_v3_store(store_dir, codec, shape, chunk_shape, dtype, chunks)
            expected_path = tmp_path / f"{slug}.npy"
            np.save(expected_path, expected)
            stores.append({"label": slug, "path": str(store_dir), "expected": str(expected_path)})

        manifest_path = tmp_path / "manifest.json"
        manifest_path.write_text(json.dumps({"registered": REGISTERED_CODECS, "stores": stores}))

        result = subprocess.run(
            [sys.executable, "-c", _COLD_RESOLUTION_CHILD, str(manifest_path)],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, (
            "cold-interpreter codec resolution failed — the zarr.codecs entry points in "
            f"pyproject.toml may be missing or misspelled.\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
        assert "COLD-RESOLUTION-OK" in result.stdout, (
            f"subprocess exited 0 without completing its checks.\nstdout:\n{result.stdout}"
        )

    def test_entry_point_table_covers_every_exported_codec(self):
        """The declared entry points are exactly the module's public codec classes.

        Catches the drift the subprocess test cannot: a codec added to ``__all__`` but
        never declared in ``pyproject.toml`` would simply be absent from
        ``REGISTERED_CODECS`` and silently untested above.
        """
        from importlib.metadata import entry_points

        import aws.osml.io.zarr_codecs as module

        declared = {
            ep.name: ep.value
            for ep in entry_points(group="zarr.codecs")
            if ep.name.startswith(CODEC_URI_PREFIX)
        }
        assert declared == {
            CODEC_URI_PREFIX + slug: f"{CODEC_MODULE}:{class_name}"
            for slug, class_name in REGISTERED_CODECS.items()
        }, "installed zarr.codecs entry points do not match this suite's expectations"

        exported = {name for name in module.__all__ if name.endswith("Codec")}
        assert exported == set(REGISTERED_CODECS.values()), (
            f"codec classes exported from {CODEC_MODULE} but not declared as zarr.codecs "
            f"entry points: {sorted(exported - set(REGISTERED_CODECS.values()))}"
        )


# ---------------------------------------------------------------------------
# 3. Config serialization round-trips (design test 6)
# ---------------------------------------------------------------------------

# Valid (nbpp, pvtype) pairs for decode_jbp_block, with bytes per sample.  ``pvtype``
# "C" is omitted: nbpp=64 complex is accepted but sized and typed as a single
# float32, which is a decoder question rather than a serialization one.
JBP_TYPE_COMBOS = [
    (8, "INT", 1), (16, "INT", 2), (32, "INT", 4),
    (8, "SI", 1), (16, "SI", 2), (32, "SI", 4),
    (32, "R", 4), (64, "R", 8),
]


@st.composite
def jbp_codec_cases(draw):
    """A ``JbpBlockCodec`` plus a raw block payload sized for its configuration."""
    num_bands = draw(st.integers(min_value=1, max_value=3))
    block_height = draw(st.integers(min_value=1, max_value=8))
    block_width = draw(st.integers(min_value=1, max_value=8))
    nbpp, pvtype, bytes_per_sample = draw(st.sampled_from(JBP_TYPE_COMBOS))
    imode = draw(st.sampled_from(["B", "P", "R", "S"]))

    codec = JbpBlockCodec(
        num_bands=num_bands,
        block_height=block_height,
        block_width=block_width,
        nbpp=nbpp,
        imode=imode,
        pvtype=pvtype,
    )
    size = num_bands * block_height * block_width * bytes_per_sample
    payload = draw(st.binary(min_size=size, max_size=size))
    return codec, payload


@st.composite
def tiff_codec_cases(draw):
    """A ``TiffTileCodec`` plus an uncompressed tile payload sized for it.

    Compression is fixed at 1 (none) so the payload can be generated bytes: the
    round-trip under test is the configuration, and a compressed payload would only
    add an encoder to the fixture without exercising more serialization.
    """
    bits_per_sample = draw(st.sampled_from([8, 16, 32]))
    samples_per_pixel = draw(st.integers(min_value=1, max_value=3))
    sample_format = draw(st.sampled_from([1, 2, 3] if bits_per_sample == 32 else [1, 2]))
    tile_width = draw(st.integers(min_value=1, max_value=8))
    tile_height = draw(st.integers(min_value=1, max_value=8))

    codec = TiffTileCodec(
        compression=1,
        bits_per_sample=bits_per_sample,
        samples_per_pixel=samples_per_pixel,
        photometric=1 if samples_per_pixel == 1 else 2,
        planar_config=1,
        predictor=1,
        tile_width=tile_width,
        tile_height=tile_height,
        sample_format=sample_format,
    )
    size = tile_width * tile_height * samples_per_pixel * (bits_per_sample // 8)
    payload = draw(st.binary(min_size=size, max_size=size))
    return codec, payload


@st.composite
def dted_codec_cases(draw):
    """A ``DtedTileCodec`` plus a data-record payload sized for it.

    Record bytes are generated rather than encoded from elevations: the decoder does
    not validate checksums, and what is under test is that a serialized config
    reproduces the *same* decode, not that the elevations are meaningful.
    """
    num_lat_points = draw(st.integers(min_value=2, max_value=16))
    num_lon_lines = draw(st.integers(min_value=2, max_value=16))
    trim_top = draw(st.integers(min_value=0, max_value=2))
    trim_bottom = draw(st.integers(min_value=0, max_value=2))
    trim_left = draw(st.integers(min_value=0, max_value=2))
    trim_right = draw(st.integers(min_value=0, max_value=2))
    # Trimming must leave a non-empty tile.
    assume(trim_top + trim_bottom < num_lat_points)
    assume(trim_left + trim_right < num_lon_lines)

    record_size = 8 + num_lat_points * 2 + 4
    codec = DtedTileCodec(
        num_lat_points=num_lat_points,
        num_lon_lines=num_lon_lines,
        record_size=record_size,
        trim_top=trim_top,
        trim_bottom=trim_bottom,
        trim_left=trim_left,
        trim_right=trim_right,
    )
    size = num_lon_lines * record_size
    payload = draw(st.binary(min_size=size, max_size=size))
    return codec, payload


def _assert_config_round_trips(codec, payload):
    """Both serialization protocols rebuild a codec that decodes *payload* identically.

    Four rebuilds are checked, covering each protocol twice — once through the class
    directly, once through the registry a real consumer goes through:

    - ``from_dict(to_dict())``           — zarr v3, ``zarr.json``
    - ``get_codec_class(uri).from_dict`` — zarr v3, resolved by URI
    - ``from_config(get_config())``      — numcodecs, ``.zarray`` filters
    - ``numcodecs.get_codec(...)``       — numcodecs, resolved by ``id``

    Going through the registries matters because a config that round-trips on the
    class can still fail there: ``get_config`` carries an extra ``id`` key that
    ``numcodecs.get_codec`` strips, and a ``from_config`` that expected to see it
    would break only on that route.
    """
    import numcodecs
    from zarr.registry import get_codec_class

    baseline = codec.decode(payload)

    v3_dict = codec.to_dict()
    numcodecs_config = codec.get_config()
    assert numcodecs_config["id"] == codec.codec_id, (
        f"{type(codec).__name__}.get_config() must carry its codec_id as 'id' or "
        f"numcodecs cannot resolve it from .zarray"
    )

    rebuilds = {
        "from_dict(to_dict())": type(codec).from_dict(v3_dict),
        "registry.from_dict(to_dict())": get_codec_class(codec.codec_name).from_dict(v3_dict),
        "from_config(get_config())": type(codec).from_config(numcodecs_config),
        "numcodecs.get_codec(get_config())": numcodecs.get_codec(dict(numcodecs_config)),
    }

    for label, rebuilt in rebuilds.items():
        assert type(rebuilt) is type(codec), (
            f"{label} produced {type(rebuilt).__name__}, expected {type(codec).__name__}"
        )
        assert rebuilt.to_dict() == v3_dict, (
            f"{label}: v3 configuration changed across the round-trip — "
            f"{rebuilt.to_dict()} != {v3_dict}"
        )
        assert rebuilt.get_config() == numcodecs_config, (
            f"{label}: numcodecs configuration changed across the round-trip — "
            f"{rebuilt.get_config()} != {numcodecs_config}"
        )

        decoded = rebuilt.decode(payload)
        assert decoded.dtype == baseline.dtype, (
            f"{label}: rebuilt codec decoded dtype {decoded.dtype}, expected {baseline.dtype}"
        )
        assert decoded.shape == baseline.shape, (
            f"{label}: rebuilt codec decoded shape {decoded.shape}, expected {baseline.shape}"
        )
        np.testing.assert_array_equal(
            decoded,
            baseline,
            err_msg=(
                f"{label}: a codec rebuilt from serialized configuration decoded the same "
                f"bytes to different pixels, so some field does not survive serialization"
            ),
        )


@pytest.mark.property
class TestCodecConfigRoundTrip:
    """Serialized configuration rebuilds a codec that decodes identically.

    ``TestCodecConfigRoundTrip`` in ``tests/unit/test_zarr_codecs.py`` compares
    ``to_dict()`` output for four hand-written configurations.  Equal dicts are a
    weaker claim than equal *pixels*: a field that round-trips into the dict but is
    ignored when reconstructing the decoder passes that check and fails here.  These
    tests generate configurations and assert on decoded output, through both
    serialization protocols and both registries.
    """

    @given(jbp_codec_cases())
    @pbt_settings
    def test_jbp_block_config_round_trip(self, case):
        """JbpBlockCodec: band count, block size, nbpp, imode, and pvtype all survive."""
        _assert_config_round_trips(*case)

    @given(tiff_codec_cases())
    @pbt_settings
    def test_tiff_tile_config_round_trip(self, case):
        """TiffTileCodec: sample geometry and format survive both protocols."""
        _assert_config_round_trips(*case)

    @given(dted_codec_cases())
    @pbt_settings
    def test_dted_config_round_trip(self, case):
        """DtedTileCodec: post counts, record size, and all four trims survive."""
        _assert_config_round_trips(*case)

    @pytest.mark.parametrize("resolution_level", [0, 1, 2, 3])
    def test_jpeg2000_config_round_trip(self, resolution_level):
        """Jpeg2000Codec: the base64 main header and resolution level survive.

        The header is the field most at risk — it is the only one that is re-decoded
        on construction (base64 → bytes) rather than stored verbatim, and the
        numcodecs decode path reads its SIZ fields to size edge-tile padding, so a
        header lost in serialization changes decoded output rather than raising.
        ``resolution_level`` is swept because it scales that padding.
        """
        fixture = CODEC_FIXTURES["jpeg2000"]
        if not fixture.exists():
            pytest.skip("J2K fixture not available")

        codec, _, _, _, chunks, _ = _extract_fixture_chunks(fixture)
        variant = Jpeg2000Codec(main_header=codec.main_header, resolution_level=resolution_level)
        assert variant._main_header_bytes is not None, "fixture produced no J2K main header"

        for payload in chunks.values():
            _assert_config_round_trips(variant, payload)

    def test_jpeg_config_round_trip(self):
        """JpegCodec: interleave mode and color space survive both protocols."""
        fixture = CODEC_FIXTURES["jpeg"]
        if not fixture.exists():
            pytest.skip("JPEG fixture not available")

        codec, _, _, _, chunks, _ = _extract_fixture_chunks(fixture)
        for payload in chunks.values():
            _assert_config_round_trips(codec, payload)

    @pytest.mark.parametrize("slug", sorted(CODEC_FIXTURES))
    def test_fixture_config_survives_a_v3_store(self, slug, tmp_path):
        """A config written to ``zarr.json`` and read back decodes the fixture's pixels.

        The round-trips above pass dicts in memory; this one goes through the file
        zarr actually parses, so it also covers JSON encoding of the config — the
        base64 header strings and ``None`` values that become ``null``.
        """
        fixture = CODEC_FIXTURES[slug]
        if not fixture.exists():
            pytest.skip(f"{slug} fixture not available")

        codec, shape, chunk_shape, dtype, chunks, expected = _extract_fixture_chunks(fixture)
        with tempfile.TemporaryDirectory() as tmp:
            store_dir = Path(tmp) / slug
            _write_v3_store(store_dir, codec, shape, chunk_shape, dtype, chunks)

            written = json.loads((store_dir / "zarr.json").read_text())
            round_tripped = type(codec).from_dict(written["codecs"][1])
            assert round_tripped.to_dict() == codec.to_dict(), (
                f"{slug}: configuration changed passing through zarr.json"
            )

            actual = np.asarray(zarr.open_array(str(store_dir), mode="r")[:])
            np.testing.assert_array_equal(
                actual,
                expected,
                err_msg=f"{slug}: v3 store read differs from the IO.open() pixels",
            )
