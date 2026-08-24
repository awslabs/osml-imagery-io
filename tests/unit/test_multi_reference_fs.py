"""Unit tests for MultiReferenceFileSystem.

Tests cover:
- Multi-range detection via _is_multi_range()
- Multi-range fetch with single and multiple sub-ranges
- Empty ranges error case
- Mixed reference sets (inline, single-range, multi-range)
- Standard reference compatibility with ReferenceFileSystem
- Async path produces same results as sync path
- Constructor accepts same arguments as ReferenceFileSystem
- Kerchunk Parquet reference directories, which the stock ReferenceFileSystem
  cannot open at all for a hierarchical store
"""

from __future__ import annotations

import asyncio
import base64
import json
from pathlib import Path

import numpy as np
import pytest
from aws.osml.io.multi_reference_fs import (
    ArrayOnlyReferenceMapper,
    MultiReferenceFileSystem,
    _is_null,
    _null_normalized,
)
from fsspec.implementations.reference import LazyReferenceMapper, ReferenceFileSystem

# ---------------------------------------------------------------------------
# _is_multi_range detection
# ---------------------------------------------------------------------------


class TestIsMultiRange:
    """Test _is_multi_range() static method for valid and invalid entries."""

    def test_valid_multi_range(self):
        assert MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", [[0, 10], [20, 5]]]
        )

    def test_valid_single_sub_range(self):
        """A single sub-range is still multi-range format."""
        assert MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", [[0, 10]]]
        )

    def test_single_range_entry_not_multi(self):
        """Standard [url, offset, length] is NOT multi-range."""
        assert not MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", 0, 10]
        )

    def test_whole_file_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range(["file:///tmp/f.dat"])

    def test_inline_string_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range("hello")

    def test_inline_bytes_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range(b"hello")

    def test_empty_ranges_not_multi(self):
        """Empty inner list → not multi-range (len check fails)."""
        assert not MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", []]
        )

    def test_second_element_is_flat_list_not_multi(self):
        """[url, [offset, length]] where inner is flat ints → not multi-range."""
        assert not MultiReferenceFileSystem._is_multi_range(
            ["file:///tmp/f.dat", [0, 10]]
        )

    def test_none_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range(None)

    def test_dict_not_multi(self):
        assert not MultiReferenceFileSystem._is_multi_range({"url": "x"})


# ---------------------------------------------------------------------------
# Helper: create a temp file with known content and build refs
# ---------------------------------------------------------------------------


@pytest.fixture
def data_file(tmp_path):
    """Create a temp file with 100 bytes of known content (0x00..0x63)."""
    content = bytes(range(100))
    p = tmp_path / "data.bin"
    p.write_bytes(content)
    return p, content


def _file_url(path: Path) -> str:
    """Return a file:// URL for a local path."""
    return path.as_uri()


def _make_fs(refs: dict, **kwargs) -> MultiReferenceFileSystem:
    """Build a MultiReferenceFileSystem from a flat refs dict."""
    fo = {"version": 1, "refs": refs}
    return MultiReferenceFileSystem(fo=fo, skip_instance_cache=True, **kwargs)


def _make_ref_fs(refs: dict, **kwargs) -> ReferenceFileSystem:
    """Build a standard ReferenceFileSystem from a flat refs dict."""
    fo = {"version": 1, "refs": refs}
    return ReferenceFileSystem(fo=fo, skip_instance_cache=True, **kwargs)


# ---------------------------------------------------------------------------
# Multi-range fetch tests
# ---------------------------------------------------------------------------


class TestMultiRangeFetch:
    """Test multi-range byte fetching via _cat_common (sync path)."""

    def test_two_non_contiguous_ranges(self, data_file):
        """Fetch two non-contiguous ranges and verify concatenation order."""
        path, content = data_file
        url = _file_url(path)
        # Ranges: bytes [10..15) and [50..55)
        refs = {"chunk/0": [url, [[10, 5], [50, 5]]]}
        fs = _make_fs(refs)
        result = fs.cat("chunk/0")
        expected = content[10:15] + content[50:55]
        assert result == expected

    def test_three_ranges(self, data_file):
        """Fetch three ranges and verify order is preserved."""
        path, content = data_file
        url = _file_url(path)
        refs = {"chunk/0": [url, [[0, 3], [40, 2], [90, 10]]]}
        fs = _make_fs(refs)
        result = fs.cat("chunk/0")
        expected = content[0:3] + content[40:42] + content[90:100]
        assert result == expected

    def test_single_sub_range_degenerates(self, data_file):
        """A multi-range entry with one sub-range works like single-range."""
        path, content = data_file
        url = _file_url(path)
        refs = {"chunk/0": [url, [[20, 10]]]}
        fs = _make_fs(refs)
        result = fs.cat("chunk/0")
        assert result == content[20:30]

    def test_key_not_found_raises(self, data_file):
        path, _ = data_file
        fs = _make_fs({})
        with pytest.raises(FileNotFoundError):
            fs.cat("nonexistent")


# ---------------------------------------------------------------------------
# Mixed reference set
# ---------------------------------------------------------------------------


class TestMixedReferences:
    """Test a reference set mixing inline, single-range, and multi-range."""

    def test_mixed_set(self, data_file):
        path, content = data_file
        url = _file_url(path)
        inline_data = b"inline-payload"
        b64_data = b"base64:" + base64.b64encode(b"secret")

        refs = {
            "inline_str": inline_data.decode(),
            "inline_b64": b64_data.decode(),
            "single_range": [url, 0, 10],
            "multi_range": [url, [[10, 5], [80, 10]]],
        }
        fs = _make_fs(refs)

        assert fs.cat("inline_str") == inline_data
        assert fs.cat("inline_b64") == b"secret"
        assert fs.cat("single_range") == content[0:10]
        assert fs.cat("multi_range") == content[10:15] + content[80:90]


# ---------------------------------------------------------------------------
# Standard reference compatibility with ReferenceFileSystem
# ---------------------------------------------------------------------------


class TestStandardReferenceCompatibility:
    """MultiReferenceFileSystem produces identical bytes to ReferenceFileSystem
    for inline, whole-file, and single-range references."""

    def test_inline_string_identical(self):
        refs = {"key": "hello world"}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")

    def test_inline_base64_identical(self):
        payload = base64.b64encode(b"\x00\x01\x02").decode()
        refs = {"key": f"base64:{payload}"}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")

    def test_single_range_identical(self, data_file):
        path, _ = data_file
        url = _file_url(path)
        refs = {"key": [url, 5, 20]}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")

    def test_whole_file_identical(self, data_file):
        path, content = data_file
        url = _file_url(path)
        refs = {"key": [url]}
        assert _make_fs(refs).cat("key") == _make_ref_fs(refs).cat("key")
        assert _make_fs(refs).cat("key") == content


# ---------------------------------------------------------------------------
# Async path
# ---------------------------------------------------------------------------


class TestAsyncPath:
    """Async _cat_file produces same results as sync for identical inputs."""

    def _run_async(self, coro):
        """Run an async coroutine in a fresh event loop."""
        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(coro)
        finally:
            loop.close()

    def test_async_multi_range_matches_sync(self, data_file):
        path, content = data_file
        url = _file_url(path)
        refs = {"chunk/0": [url, [[5, 10], [60, 15]]]}
        fs = _make_fs(refs)

        sync_result = fs.cat("chunk/0")
        async_result = self._run_async(fs._cat_file("chunk/0"))

        expected = content[5:15] + content[60:75]
        assert sync_result == expected
        assert async_result == expected
        assert sync_result == async_result

    def test_async_inline_matches_sync(self):
        refs = {"key": "test-data"}
        fs = _make_fs(refs)

        sync_result = fs.cat("key")
        async_result = self._run_async(fs._cat_file("key"))
        assert sync_result == async_result

    def test_async_single_range_matches_sync(self, data_file):
        path, _ = data_file
        url = _file_url(path)
        refs = {"key": [url, 10, 20]}
        fs = _make_fs(refs)

        sync_result = fs.cat("key")
        async_result = self._run_async(fs._cat_file("key"))
        assert sync_result == async_result

    def test_async_key_not_found(self):
        fs = _make_fs({})
        with pytest.raises(FileNotFoundError):
            self._run_async(fs._cat_file("missing"))


# ---------------------------------------------------------------------------
# Constructor compatibility
# ---------------------------------------------------------------------------


class TestConstructor:
    """MultiReferenceFileSystem accepts same arguments as ReferenceFileSystem."""

    def test_basic_construction(self):
        fo = {"version": 1, "refs": {"k": "v"}}
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        assert isinstance(fs, ReferenceFileSystem)
        assert isinstance(fs, MultiReferenceFileSystem)

    def test_construction_with_remote_options(self, data_file):
        path, _ = data_file
        url = _file_url(path)
        fo = {"version": 1, "refs": {"k": [url, 0, 5]}}
        fs = MultiReferenceFileSystem(
            fo=fo,
            skip_instance_cache=True,
            remote_options={"auto_mkdir": True},
        )
        assert isinstance(fs, MultiReferenceFileSystem)
        # Should still be able to read
        assert len(fs.cat("k")) == 5

    def test_is_subclass(self):
        assert issubclass(MultiReferenceFileSystem, ReferenceFileSystem)


# ---------------------------------------------------------------------------
# Template expansion for multi-range entries
# ---------------------------------------------------------------------------


class TestTemplateExpansion:
    """Verify template expansion works for all reference types including multi-range."""

    def test_single_range_template_expanded(self, data_file):
        """Standard single-range refs with {{base}} are expanded via templates."""
        path, _ = data_file
        url = _file_url(path)
        # Split URL into base + filename
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": base},
            "refs": {"k": ["{{base}}" + filename, 0, 5]},
        }
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        result = fs.cat("k")
        assert len(result) == 5

    def test_multi_range_template_expanded(self, data_file):
        """Multi-range refs with {{base}} are expanded via templates."""
        path, content = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": base},
            "refs": {
                "k": ["{{base}}" + filename, [[0, 5], [10, 3]]],
            },
        }
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        result = fs.cat("k")
        expected = content[0:5] + content[10:13]
        assert result == expected

    def test_template_overrides_applied_to_multi_range(self, data_file):
        """template_overrides replaces template values for multi-range refs."""
        path, content = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": ""},
            "refs": {
                "k": ["{{base}}" + filename, [[0, 5], [10, 3]]],
            },
        }
        fs = MultiReferenceFileSystem(
            fo=fo, skip_instance_cache=True,
            template_overrides={"base": base},
        )
        result = fs.cat("k")
        expected = content[0:5] + content[10:13]
        assert result == expected

    def test_template_overrides_applied_to_single_range(self, data_file):
        """template_overrides replaces template values for single-range refs."""
        path, _ = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": ""},
            "refs": {"k": ["{{base}}" + filename, 0, 5]},
        }
        fs = MultiReferenceFileSystem(
            fo=fo, skip_instance_cache=True,
            template_overrides={"base": base},
        )
        result = fs.cat("k")
        assert len(result) == 5

    def test_no_templates_multi_range_unchanged(self, data_file):
        """Multi-range refs without templates work as before."""
        path, content = data_file
        url = _file_url(path)

        fo = {
            "version": 1,
            "refs": {
                "k": [url, [[0, 5], [10, 3]]],
            },
        }
        fs = MultiReferenceFileSystem(fo=fo, skip_instance_cache=True)
        result = fs.cat("k")
        expected = content[0:5] + content[10:13]
        assert result == expected

    def test_mixed_template_refs(self, data_file):
        """Mix of single-range, multi-range, and inline refs all resolve correctly."""
        path, content = data_file
        url = _file_url(path)
        base = url.rsplit("/", 1)[0] + "/"
        filename = url.rsplit("/", 1)[1]

        fo = {
            "version": 1,
            "templates": {"base": ""},
            "refs": {
                "inline": "hello",
                "single": ["{{base}}" + filename, 0, 5],
                "multi": ["{{base}}" + filename, [[0, 3], [5, 2]]],
            },
        }
        fs = MultiReferenceFileSystem(
            fo=fo, skip_instance_cache=True,
            template_overrides={"base": base},
        )

        assert fs.cat("inline") == b"hello"
        assert len(fs.cat("single")) == 5
        expected_multi = content[0:3] + content[5:7]
        assert fs.cat("multi") == expected_multi


# ---------------------------------------------------------------------------
# Parquet null normalization predicates
# ---------------------------------------------------------------------------


class TestNullPredicate:
    """_is_null classifies every value either Parquet engine can produce.

    The predicate must be pandas-free (this library declares no pandas
    dependency) yet catch the ``numpy.float64('nan')`` fastparquet produces.
    """

    @pytest.mark.parametrize(
        "value",
        [None, float("nan"), np.float64("nan")],
        ids=["none", "float-nan", "numpy-float64-nan"],
    )
    def test_null_values(self, value):
        assert _is_null(value)

    @pytest.mark.parametrize(
        "value",
        [
            b"",
            b"abc",
            b"base64:AAEC",
            "path/to/file",
            np.bytes_(b"abc"),
            bytearray(b"ab"),
            0,
            0.0,
            np.float64(0.0),
            1.5,
        ],
        ids=[
            "empty-bytes", "bytes", "base64-bytes", "str", "numpy-bytes",
            "bytearray", "int-zero", "float-zero", "numpy-float-zero", "float",
        ],
    )
    def test_non_null_values(self, value):
        """An empty inline chunk (b"") must not be mistaken for a null."""
        assert not _is_null(value)

    def test_object_dtype_nan_normalized(self):
        """pyarrow returns ``path`` as object dtype holding nan, not float dtype.

        A ``dtype.kind == "f"`` check alone would silently miss this, which is
        why the normalizer tests ``"fO"``.
        """
        arr = np.array(["a/b", float("nan"), "c/d"], dtype=object)
        out = _null_normalized(arr)
        assert out[0] == "a/b"
        assert out[1] is None
        assert out[2] == "c/d"

    def test_float_dtype_nan_normalized(self):
        """fastparquet returns an all-null ``raw`` column as float64."""
        out = _null_normalized(np.array([np.nan, np.nan], dtype="float64"))
        assert out.dtype == object
        assert out[0] is None and out[1] is None

    def test_integer_columns_untouched(self):
        """``offset``/``size`` cannot carry nulls and are returned as-is."""
        arr = np.array([0, 869, 12], dtype="int64")
        assert _null_normalized(arr) is arr


# ---------------------------------------------------------------------------
# Kerchunk Parquet reference directories
# ---------------------------------------------------------------------------


_PARQUET_STORE_RECORD_SIZE = 10


def _write_parquet_record(path: Path, references: dict, record_size: int) -> None:
    """Write one ``refs.{record}.parq`` from ``{row: reference}``, by hand.

    Deliberately does *not* call the library's own emitter: these are tests of the
    reader, and validating a reader with the writer under test would make the pair
    agree on a wrong encoding without anything noticing.  What is encoded here is
    the container contract as documented — upstream's four columns
    (``path``, ``offset``, ``size``, ``raw``) plus ``range_path`` / ``offsets`` /
    ``sizes`` when a multi-range reference is present, with ``path`` null on those
    rows.

    Built with ``pyarrow`` alone, like the library's writer: this project declares no
    ``pandas`` dependency and imports it nowhere, so a test helper must not either.
    """
    import pyarrow as pa
    import pyarrow.parquet as pq

    paths = np.full(record_size, None, dtype="O")
    offsets = np.zeros(record_size, dtype="int64")
    sizes = np.zeros(record_size, dtype="int64")
    raws = np.full(record_size, None, dtype="O")
    range_paths = np.full(record_size, None, dtype="O")
    range_offsets = np.full(record_size, None, dtype="O")
    range_sizes = np.full(record_size, None, dtype="O")
    any_multi_range = False

    for row, reference in references.items():
        if isinstance(reference, bytes):
            raws[row] = reference
        elif isinstance(reference[1], list):
            any_multi_range = True
            range_paths[row] = reference[0]
            range_offsets[row] = [o for o, _ in reference[1]]
            range_sizes[row] = [n for _, n in reference[1]]
        else:
            paths[row], offsets[row], sizes[row] = reference

    def typed(values, value_type):
        """All-null columns take Arrow's ``null`` type, as a conforming writer does."""
        if all(v is None for v in values):
            return pa.nulls(len(values))
        return pa.array(values, type=value_type)

    int_list = pa.list_(pa.int64())
    columns = {
        "path": typed(paths, pa.string()),
        "offset": pa.array(offsets, type=pa.int64()),
        "size": pa.array(sizes, type=pa.int64()),
        "raw": typed(raws, pa.binary()),
    }
    if any_multi_range:
        columns["range_path"] = pa.array(range_paths, type=pa.string())
        columns["offsets"] = pa.array(range_offsets, type=int_list)
        columns["sizes"] = pa.array(range_sizes, type=int_list)

    path.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(
        pa.table(columns), path, compression="zstd", write_statistics=False,
    )


def _write_parquet_store(
    root: Path, src: Path, *, levels=("0",), template_base=None, multi_range=False
):
    """Hand-build a hierarchical Kerchunk Parquet store.

    Mirrors the shape ``write_tile_index(..., "x.parquet")`` produces — a root
    group, a group per resolution level, and a ``data`` array under each — but
    without depending on the parser or on the library's emitter, so this stays a
    unit test of the reader.

    Each level's array has 4 chunks: chunks 0 and 2 are URL references into *src*,
    chunks 1 and 3 are inline raw bytes.  That mix within a single record is what
    forces ``path`` to object dtype containing ``nan`` under pyarrow,
    distinguishing a correct null normalization from a float-only one.

    With ``multi_range=True`` chunk 2 becomes a multi-range reference of two
    non-adjacent fragments, so one record carries all three reference forms at
    once — and ``path`` keeps a non-null entry, so the column stays
    ``large_string`` with nulls rather than collapsing to Arrow's ``null`` type.
    Chunks widen from 1 byte to 2 in that mode, because two fragments cannot fit a
    one-element chunk and a store whose chunk lengths disagree with its ``.zarray``
    decodes to garbage rather than failing.
    """
    content = src.read_bytes()
    # 2-byte chunks in multi-range mode so the multi-range chunk's two 1-byte
    # fragments add up to exactly one chunk's worth of data.
    width = 2 if multi_range else 1
    zmetadata = {
        ".zgroup": {"zarr_format": 2},
        ".zattrs": {
            "multiscales": [
                {
                    "version": "0.4",
                    "datasets": [{"path": lvl} for lvl in levels],
                }
            ]
        },
    }
    expected: dict[str, bytes] = {}
    records: dict[str, dict] = {}

    for i, level in enumerate(levels):
        zmetadata[f"{level}/.zgroup"] = {"zarr_format": 2}
        zmetadata[f"{level}/.zattrs"] = {}
        zmetadata[f"{level}/data/.zarray"] = {
            "zarr_format": 2,
            "shape": [4 * width],
            "chunks": [width],
            "dtype": "|u1",
            "compressor": None,
            "filters": None,
            "fill_value": 0,
            "order": "C",
        }
        zmetadata[f"{level}/data/.zattrs"] = {"_ARRAY_DIMENSIONS": ["x"]}

        url = f"{template_base}{src.name}" if template_base else str(src)
        references: dict[int, object] = {}
        for chunk in range(4):
            key = f"{level}/data/{chunk}"
            if chunk % 2:
                inline = bytes(0xA0 + 10 * i + chunk + b for b in range(width))
                references[chunk] = inline
                expected[key] = inline
            elif multi_range and chunk == 2:
                # Two non-adjacent single bytes well inside the 100-byte source, so
                # a reader that fetched only the first fragment, or concatenated
                # them out of order, is caught — ``content[n] == n``, so the two
                # fragments always differ.
                first, second = 10 * i + 2, 10 * i + 60
                references[chunk] = [url, [[first, 1], [second, 1]]]
                expected[key] = (
                    content[first : first + 1] + content[second : second + 1]
                )
            else:
                offset = 10 * i + chunk
                references[chunk] = [url, offset, width]
                expected[key] = content[offset : offset + width]
        # A 1-D 4-chunk array ravels to row == chunk index, and 4 < record_size, so
        # everything lands in record 0.
        records[f"{level}/data"] = references

    root.mkdir(parents=True, exist_ok=True)
    (root / ".zmetadata").write_text(
        json.dumps(
            {"metadata": zmetadata, "record_size": _PARQUET_STORE_RECORD_SIZE}
        )
    )
    for field, references in records.items():
        _write_parquet_record(
            root / field / "refs.0.parq", references, _PARQUET_STORE_RECORD_SIZE
        )
    return expected


@pytest.fixture
def parquet_store(tmp_path, data_file):
    """A single-level hierarchical Parquet store plus its expected chunk bytes."""
    src, _ = data_file
    root = tmp_path / "index.parquet"
    expected = _write_parquet_store(root, src)
    return root, expected


class TestParquetReferenceDirectory:
    """A hierarchical Parquet index opens and reads back correctly.

    Every test here fails against the stock ``ReferenceFileSystem``: nested
    ``.zgroup`` keys make upstream ``listdir()`` report the group prefix ``"0"``
    as an array field, and ``_get_chunk_sizes("0")`` then raises
    ``KeyError: '0/.zarray'`` while the store is still being constructed.
    """

    def test_construction_succeeds(self, parquet_store):
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert isinstance(fs.references, ArrayOnlyReferenceMapper)

    def test_stock_mapper_misreports_group_prefix_as_a_field(self, parquet_store):
        """Pins the upstream defect this class exists to work around.

        Asserted against ``listdir()`` rather than by expecting the
        ``KeyError: '0/.zarray'`` that ``ReferenceFileSystem(...)`` raises,
        because that construction failure is *order-dependent*: ``listdir()``
        returns a ``set``, and ``__init__`` stops iterating ``references.values()``
        as soon as it resolves a remote protocol.  Whether it reaches the bogus
        ``'0'`` field before breaking varies with set iteration order, i.e. with
        ``PYTHONHASHSEED``.  The field set itself is deterministic.

        If a future fsspec fixes ``listdir()``, this test fails and the override
        can be reconsidered — it is the signal, not a guard against regression.
        """
        import fsspec

        root, _ = parquet_store
        ref_fs, root_str = fsspec.core.url_to_fs(str(root))
        stock = LazyReferenceMapper(root_str, fs=ref_fs, engine="pyarrow")

        # The group prefix '0' has a .zgroup but no .zarray, yet upstream reports
        # it alongside the real array field — and every consumer of listdir()
        # assumes an array, so _get_chunk_sizes('0') raises.
        assert stock.listdir() == {"0", "0/data"}
        with pytest.raises(KeyError, match=r"0/\.zarray"):
            stock._get_chunk_sizes("0")

        # Ours reports only the array field, so that call site is never reached.
        assert ArrayOnlyReferenceMapper(root_str, fs=ref_fs).listdir() == {"0/data"}

    def test_listdir_reports_only_array_fields(self, parquet_store):
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.references.listdir() == {"0/data"}

    def test_engine_attribute_is_pyarrow(self, parquet_store):
        """``engine`` is vestigial for reads, but must stay "pyarrow".

        :meth:`ArrayOnlyReferenceMapper.setup` reads with ``pyarrow.parquet``
        unconditionally, so this attribute no longer selects anything.  It is still
        asserted because upstream's ``__init__`` uses it to run
        ``find_spec("pyarrow")`` — the check that turns a missing install into an
        actionable message instead of a ModuleNotFoundError mid-read.
        """
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.references.engine == "pyarrow"

    def test_chunk_reads_after_construction(self, parquet_store):
        """Reads must work post-construction, not just at construction.

        A second mapper is built lazily during the first read, so a
        construction-only assertion can pass while every read still fails.
        """
        root, expected = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        for key, want in expected.items():
            assert fs.cat(key) == want, f"chunk {key} resolved incorrectly"

    def test_mixed_inline_and_url_refs_in_one_record(self, parquet_store):
        """Inline-raw and URL chunks in a single record all resolve.

        This is the case that distinguishes a correct null normalization from one
        checking float dtype only: with both kinds present, pyarrow returns
        ``path`` as *object* dtype containing ``nan``.
        """
        root, expected = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        url_keys = [k for k in expected if k.endswith(("/0", "/2"))]
        inline_keys = [k for k in expected if k.endswith(("/1", "/3"))]
        assert url_keys and inline_keys, "fixture must mix both reference forms"
        for key in url_keys + inline_keys:
            assert fs.cat(key) == expected[key]

    def test_reads_a_store_whose_null_columns_are_float64(self, tmp_path, data_file):
        """A store written by another tool, with ``nan`` for nulls, still reads.

        ``fastparquet`` types an all-null column ``float64``, so its nulls arrive as
        ``nan`` rather than ``None``.  Upstream's ``raw is not None`` test passes for
        ``nan`` and hands the float back as if it were chunk data —
        ``TypeError: object of type 'numpy.float64' has no len()``.  Normalizing
        nulls in the loader is what makes the reader independent of who wrote the
        store, and this is the property that used to be checked by forcing
        ``engine="fastparquet"``; that no longer proves anything now the loader is
        pyarrow-only, so the *column type* is reproduced directly instead — which
        also drops the fastparquet dependency from the test.
        """
        import pyarrow as pa
        import pyarrow.parquet as pq

        src, content = data_file
        root = tmp_path / "float_nulls.parquet"
        root.mkdir()
        (root / ".zmetadata").write_text(
            json.dumps({
                "metadata": {
                    ".zgroup": {"zarr_format": 2},
                    "0/.zgroup": {"zarr_format": 2},
                    "0/data/.zarray": {
                        "zarr_format": 2, "shape": [2], "chunks": [1],
                        "dtype": "|u1", "compressor": None, "filters": None,
                        "fill_value": 0, "order": "C",
                    },
                },
                "record_size": _PARQUET_STORE_RECORD_SIZE,
            })
        )
        n = _PARQUET_STORE_RECORD_SIZE
        (root / "0" / "data").mkdir(parents=True)
        pq.write_table(
            pa.table({
                "path": pa.array([str(src), str(src)] + [None] * (n - 2),
                                 type=pa.string()),
                "offset": pa.array([3, 9] + [0] * (n - 2), type=pa.int64()),
                "size": pa.array([1, 1] + [0] * (n - 2), type=pa.int64()),
                # The fastparquet spelling: an all-null column as float64 nan.
                "raw": pa.array([float("nan")] * n, type=pa.float64()),
            }),
            root / "0" / "data" / "refs.0.parq",
            compression="zstd",
        )

        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.cat("0/data/0") == content[3:4]
        assert fs.cat("0/data/1") == content[9:10]

    def test_reads_a_pandas_written_store(self, tmp_path, data_file):
        """Interop: a store written through pandas reads identically.

        This library writes with pyarrow directly and imports no pandas, which
        produces Arrow ``string`` rather than ``large_string`` and omits pandas'
        schema-metadata block.  Neither should matter, in either direction — kerchunk
        and other Kerchunk-ecosystem tools write via pandas, and their indexes must
        remain readable here.  Guarded rather than assumed available, since pandas is
        not a dependency of this project.
        """
        pd = pytest.importorskip("pandas")

        src, content = data_file
        root = tmp_path / "pandas_written.parquet"
        root.mkdir()
        (root / ".zmetadata").write_text(
            json.dumps({
                "metadata": {
                    ".zgroup": {"zarr_format": 2},
                    "0/.zgroup": {"zarr_format": 2},
                    "0/data/.zarray": {
                        "zarr_format": 2, "shape": [2], "chunks": [1],
                        "dtype": "|u1", "compressor": None, "filters": None,
                        "fill_value": 0, "order": "C",
                    },
                },
                "record_size": _PARQUET_STORE_RECORD_SIZE,
            })
        )
        n = _PARQUET_STORE_RECORD_SIZE
        paths = np.full(n, np.nan, dtype="O")
        offsets = np.zeros(n, dtype="int64")
        sizes = np.zeros(n, dtype="int64")
        raws = np.full(n, np.nan, dtype="O")
        paths[0], offsets[0], sizes[0] = str(src), 3, 1
        raws[1] = b"\x5a"
        (root / "0" / "data").mkdir(parents=True)
        pd.DataFrame(
            {"path": paths, "offset": offsets, "size": sizes, "raw": raws},
            copy=False,
        ).to_parquet(
            root / "0" / "data" / "refs.0.parq",
            engine="pyarrow", compression="zstd", index=False,
        )

        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.cat("0/data/0") == content[3:4]
        assert fs.cat("0/data/1") == b"\x5a"

    def test_ls_root_succeeds(self, parquet_store):
        """``ls("")`` works via the dircache fallback.

        Narrowing ``listdir()`` to array fields makes the parent's short-circuit
        to ``LazyReferenceMapper.ls`` raise for the root, since the parent never
        consults ``dircache``.
        """
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        names = fs.ls("", detail=False)
        assert "0" in names, f"group level missing from ls(''): {names}"

    def test_ls_detail_returns_dicts(self, parquet_store):
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        entries = fs.ls("", detail=True)
        assert all(isinstance(e, dict) and "name" in e for e in entries)

    def test_isdir_on_group_level(self, parquet_store):
        """``isdir("0")`` is True even though ``listdir()`` omits group prefixes."""
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.isdir("0")

    def test_isdir_false_for_missing_path(self, parquet_store):
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert not fs.isdir("nope")

    def test_ls_missing_path_raises(self, parquet_store):
        root, _ = parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        with pytest.raises(FileNotFoundError):
            fs.ls("nope")

    def test_multi_level_store(self, tmp_path, data_file):
        """A two-level pyramid keeps both levels readable."""
        src, _ = data_file
        root = tmp_path / "pyramid.parquet"
        expected = _write_parquet_store(root, src, levels=("0", "1"))
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.references.listdir() == {"0/data", "1/data"}
        for key, want in expected.items():
            assert fs.cat(key) == want

    def test_zarr_group_reads_both_levels(self, tmp_path, data_file):
        """zarr can open the store and both pyramid levels decode."""
        zarr = pytest.importorskip("zarr", minversion="3.0")
        src, _ = data_file
        root = tmp_path / "zarr_pyramid.parquet"
        expected = _write_parquet_store(root, src, levels=("0", "1"))
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        group = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)
        assert "multiscales" in dict(group.attrs)
        for level in ("0", "1"):
            values = np.asarray(group[f"{level}/data"][:])
            want = np.frombuffer(
                b"".join(expected[f"{level}/data/{c}"] for c in range(4)),
                dtype=np.uint8,
            )
            np.testing.assert_array_equal(values, want)


@pytest.fixture
def multi_range_parquet_store(tmp_path, data_file):
    """A Parquet store mixing inline, single-range and multi-range chunks."""
    src, _ = data_file
    root = tmp_path / "multirange.parquet"
    expected = _write_parquet_store(root, src, multi_range=True)
    return root, expected


class TestParquetMultiRangeReferences:
    """The Parquet container carries multi-range chunk references.

    Upstream's record schema has only scalar ``offset`` / ``size`` columns, so a
    variable-length range list has nowhere to go — writing one raised
    ``TypeError: int() argument must be ... not 'list'`` from inside fsspec, and no
    column existed that a reader could have returned it from.  The container is
    widened with ``range_path`` / ``offsets`` / ``sizes``, and these tests cover the
    read half of that contract against a hand-built store.
    """

    def test_schema_carries_the_range_columns(self, multi_range_parquet_store):
        """The three appended columns are present, and the list ones are lists."""
        pa = pytest.importorskip("pyarrow")
        import pyarrow.parquet as pq

        root, _ = multi_range_parquet_store
        schema = pq.read_schema(root / "0" / "data" / "refs.0.parq")
        assert schema.names == [
            "path", "offset", "size", "raw", "range_path", "offsets", "sizes",
        ], schema.names
        for name in ("offsets", "sizes"):
            # Checked semantically rather than by the type's string form, which
            # Parquet renames from "item" to "element" across the write.
            field_type = schema.field(name).type
            assert pa.types.is_list(field_type), field_type
            assert field_type.value_type == pa.int64(), field_type

    def test_load_one_key_returns_the_multi_range_form(
        self, multi_range_parquet_store
    ):
        """A multi-range row decodes to ``[url, [[offset, length], ...]]``."""
        import fsspec

        root, _ = multi_range_parquet_store
        ref_fs, root_str = fsspec.core.url_to_fs(str(root))
        mapper = ArrayOnlyReferenceMapper(root_str, fs=ref_fs)

        reference = mapper["0/data/2"]
        assert MultiReferenceFileSystem._is_multi_range(reference), reference
        assert reference[1] == [[2, 1], [60, 1]], reference[1]

    def test_multi_range_chunk_concatenates_in_order(
        self, multi_range_parquet_store
    ):
        """``fs.cat`` returns the fragments joined in the stored order.

        The fixture's two fragments are far apart and hold different bytes, so a
        reader that fetched only the first, or reversed them, fails here.
        """
        root, expected = multi_range_parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert len(expected["0/data/2"]) == 2, "fixture should have two fragments"
        assert fs.cat("0/data/2") == expected["0/data/2"]

    def test_all_reference_forms_coexist_in_one_record(
        self, multi_range_parquet_store
    ):
        """Inline, single-range and multi-range rows all resolve side by side.

        This is the case where ``path`` keeps a non-null entry, so the column stays
        Arrow ``large_string`` with nulls rather than collapsing to the ``null``
        type an all-multi-range record produces.  The two decode through different
        pyarrow paths, so both are covered.
        """
        root, expected = multi_range_parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        for key, want in expected.items():
            assert fs.cat(key) == want, f"chunk {key} resolved incorrectly"

    def test_zarr_reads_the_store(self, multi_range_parquet_store):
        """zarr decodes the array with a multi-range chunk in it."""
        zarr = pytest.importorskip("zarr", minversion="3.0")
        root, expected = multi_range_parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        group = zarr.open_group(fs.get_mapper(""), mode="r", zarr_format=2)
        want = np.frombuffer(
            b"".join(expected[f"0/data/{c}"] for c in range(4)), dtype=np.uint8
        )
        np.testing.assert_array_equal(np.asarray(group["0/data"][:]), want)

    def test_async_path_matches_the_sync_path(self, multi_range_parquet_store):
        """``_cat_file`` fans the fragments out concurrently, same bytes.

        ``_fetch_multi_range_async`` and ``_fetch_multi_range_sync`` are separate
        implementations and ``fs.cat`` reaches only the sync one, so the async
        entry point is exercised directly.
        """
        root, expected = multi_range_parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        got = asyncio.run(fs._cat_file("0/data/2"))
        assert got == expected["0/data/2"] == fs.cat("0/data/2")

    def test_multi_range_chunks_are_absent_from_listings(
        self, multi_range_parquet_store
    ):
        """Pins the one cosmetic consequence of leaving ``path`` null.

        Upstream's ``ls()`` drops rows whose first column is falsy, so multi-range
        chunks do not appear in a listing of their field.  Reads are unaffected —
        Zarr fetches by key — so this is documented and asserted rather than fixed
        by re-implementing upstream's ``ls``.  If a future change makes listings
        load-bearing, this test is the place that says so.
        """
        root, _ = multi_range_parquet_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        names = {entry["name"] for entry in fs.ls("0/data", detail=True)}
        assert "0/data/0" in names, f"single-range chunk should be listed: {names}"
        assert "0/data/2" not in names, (
            f"multi-range chunk unexpectedly listed — if this now works, drop the "
            f"caveat from ArrayOnlyReferenceMapper's docstring: {names}"
        )
        # Reading it still works, which is the property that actually matters.
        assert fs.cat("0/data/2")


class TestMultiRangeParquetFailsLoudForUnawareReaders:
    """A reader that ignores the range columns fails; it never returns pixels.

    This is a deliberate design property and otherwise invisible, so it is pinned
    here.  ``path`` is left null on a multi-range row precisely so that an unaware
    reader breaks: seeding it with the first fragment would hand such a reader one
    fragment of a chunk as though it were the whole thing, and populating it with
    ``offset=size=0`` would make upstream read the *entire* source file as one
    chunk.  Both would be silent wrong pixels.
    """

    def test_stock_mapper_raises_on_an_all_multi_range_record(
        self, tmp_path, data_file
    ):
        """Every row multi-range → ``path`` is Arrow ``null`` → clean ``KeyError``.

        With no non-null entry, pyarrow types the column ``null`` and returns
        ``None``, so upstream's own ``selection[0] is None`` test fires.
        """
        import fsspec

        src, _ = data_file
        root = tmp_path / "allmulti.parquet"
        root.mkdir()
        (root / ".zmetadata").write_text(
            json.dumps({
                "metadata": {
                    ".zgroup": {"zarr_format": 2},
                    "0/.zgroup": {"zarr_format": 2},
                    "0/data/.zarray": {
                        "zarr_format": 2, "shape": [2], "chunks": [1],
                        "dtype": "|u1", "compressor": None, "filters": None,
                        "fill_value": 0, "order": "C",
                    },
                },
                "record_size": _PARQUET_STORE_RECORD_SIZE,
            })
        )
        _write_parquet_record(
            root / "0" / "data" / "refs.0.parq",
            {
                0: [str(src), [[0, 1], [40, 1]]],
                1: [str(src), [[1, 1], [50, 1]]],
            },
            _PARQUET_STORE_RECORD_SIZE,
        )

        ref_fs, root_str = fsspec.core.url_to_fs(str(root))
        stock = LazyReferenceMapper(root_str, fs=ref_fs, engine="pyarrow")
        with pytest.raises(KeyError):
            stock["0/data/0"]

        # Ours reads the same store correctly.
        ours = ArrayOnlyReferenceMapper(root_str, fs=ref_fs)
        assert ours["0/data/0"] == [str(src), [[0, 1], [40, 1]]]

    def test_stock_mapper_returns_a_malformed_ref_on_a_mixed_record(
        self, multi_range_parquet_store
    ):
        """Mixed record → ``path`` stays ``large_string`` and the null survives.

        A stock reader does not normalize nulls, so pyarrow's ``nan`` reaches
        upstream's ``selection[0] is None`` test, passes it, and falls through to
        the whole-file branch returning ``[nan]``.  Still a failure — a
        single-element reference naming a non-string URL — but not the clean
        ``KeyError`` of the all-null case, so it is pinned as what it is rather than
        advertised as clean.
        """
        import fsspec

        root, expected = multi_range_parquet_store
        ref_fs, root_str = fsspec.core.url_to_fs(str(root))
        stock = LazyReferenceMapper(root_str, fs=ref_fs, engine="pyarrow")

        reference = stock["0/data/2"]
        assert isinstance(reference, list) and len(reference) == 1, reference
        assert not isinstance(reference[0], (str, bytes)), (
            f"a stock reader must not resolve a multi-range row to a usable "
            f"reference, got {reference!r}"
        )
        # Single-range and inline rows are unaffected for a stock reader.
        assert stock["0/data/1"] == expected["0/data/1"]


class TestParquetTemplateExpansion:
    """Portable Parquet indexes resolve ``{{base}}`` from ``template_overrides``.

    The Parquet container has nowhere to store a Kerchunk ``"templates"`` dict, so
    the overrides are the only source of substitutions.  Without expansion these
    reads fail with ``ReferenceNotReachable`` on the literal ``{{base}}...`` URL.
    """

    @pytest.fixture
    def portable_store(self, tmp_path, data_file):
        src, _ = data_file
        root = tmp_path / "portable.parquet"
        expected = _write_parquet_store(root, src, template_base="{{base}}")
        return root, expected, str(src.parent) + "/"

    def test_templates_expanded_from_overrides(self, portable_store):
        root, expected, base = portable_store
        fs = MultiReferenceFileSystem(
            fo=str(root),
            skip_instance_cache=True,
            template_overrides={"base": base},
        )
        for key, want in expected.items():
            assert fs.cat(key) == want

    def test_mapper_receives_the_overrides(self, portable_store):
        root, _, base = portable_store
        fs = MultiReferenceFileSystem(
            fo=str(root),
            skip_instance_cache=True,
            template_overrides={"base": base},
        )
        assert fs.references.templates == {"base": base}

    def test_unexpanded_placeholder_is_unreachable(self, portable_store):
        """Without overrides the URL stays literal, and the fetch fails loudly."""
        from fsspec.implementations.reference import ReferenceNotReachable

        root, expected, _ = portable_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        url_key = next(k for k in expected if k.endswith("/0"))
        with pytest.raises(ReferenceNotReachable):
            fs.cat(url_key)

    def test_inline_chunks_unaffected_by_templates(self, portable_store):
        """Inline raw chunks carry no URL, so expansion must leave them alone."""
        root, expected, base = portable_store
        fs = MultiReferenceFileSystem(
            fo=str(root),
            skip_instance_cache=True,
            template_overrides={"base": base},
        )
        for key in (k for k in expected if k.endswith(("/1", "/3"))):
            assert fs.cat(key) == expected[key]

    def test_absolute_urls_unaffected_by_overrides(self, parquet_store):
        """An index with no placeholders ignores overrides rather than corrupting."""
        root, expected = parquet_store
        fs = MultiReferenceFileSystem(
            fo=str(root),
            skip_instance_cache=True,
            template_overrides={"base": "s3://nope/"},
        )
        for key, want in expected.items():
            assert fs.cat(key) == want

    @pytest.fixture
    def portable_multi_range_store(self, tmp_path, data_file):
        src, _ = data_file
        root = tmp_path / "portable_mr.parquet"
        expected = _write_parquet_store(
            root, src, template_base="{{base}}", multi_range=True
        )
        return root, expected, str(src.parent) + "/"

    def test_range_path_is_expanded_too(self, portable_multi_range_store):
        """``{{base}}`` must be substituted in ``range_path``, not only ``path``.

        Expanding one column and not the other leaves the placeholder literal in
        exactly the entries a portable index most needs — for RPCL J2K imagery
        every chunk reference is multi-range — and the failure surfaces as an
        unreachable URL rather than as an unexpanded template.
        """
        root, expected, base = portable_multi_range_store
        fs = MultiReferenceFileSystem(
            fo=str(root),
            skip_instance_cache=True,
            template_overrides={"base": base},
        )
        url = fs.references["0/data/2"][0]
        assert url.startswith(base), f"range_path not expanded: {url!r}"
        assert fs.cat("0/data/2") == expected["0/data/2"]

    def test_unexpanded_range_path_fails_loudly(self, portable_multi_range_store):
        """Without overrides a multi-range read fails rather than reading garbage."""
        root, _, _ = portable_multi_range_store
        fs = MultiReferenceFileSystem(fo=str(root), skip_instance_cache=True)
        assert fs.references["0/data/2"][0].startswith("{{base}}")
        with pytest.raises((FileNotFoundError, OSError)):
            fs.cat("0/data/2")


class TestMissingPyarrowIsActionable:
    """Absent pyarrow, the error names the package and the extra supplying it.

    Upstream raises a bare ``ImportError("engine choice `pyarrow` is not
    installed.")``, which does not tell a user how to fix their install.
    """

    @pytest.fixture
    def pyarrow_absent(self, monkeypatch):
        """Hide pyarrow the way fsspec detects it — via ``find_spec``."""
        import importlib.util

        real = importlib.util.find_spec

        def fake(name, *args, **kwargs):
            return None if name == "pyarrow" else real(name, *args, **kwargs)

        monkeypatch.setattr(importlib.util, "find_spec", fake)

    def test_read_error_names_package_and_extra(self, parquet_store, pyarrow_absent):
        import fsspec

        root, _ = parquet_store
        ref_fs, root_str = fsspec.core.url_to_fs(str(root))
        with pytest.raises(ImportError) as excinfo:
            ArrayOnlyReferenceMapper(root_str, fs=ref_fs)
        message = str(excinfo.value)
        assert "pyarrow" in message
        assert "osml-imagery-io[zarr]" in message

    def test_read_error_does_not_imply_read_only(self, parquet_store, pyarrow_absent):
        """Writing needs pyarrow too; the message must not suggest otherwise."""
        import fsspec

        root, _ = parquet_store
        ref_fs, root_str = fsspec.core.url_to_fs(str(root))
        with pytest.raises(ImportError, match="not read-only"):
            ArrayOnlyReferenceMapper(root_str, fs=ref_fs)

    def test_write_error_names_package_and_extra(self, tmp_path, pyarrow_absent):
        from aws.osml.io.virtualizarr_parsers import _emit_refs

        with pytest.raises(ImportError) as excinfo:
            _emit_refs(
                {".zgroup": '{"zarr_format": 2}'},
                str(tmp_path / "out.parquet"),
                ".parquet",
                use_templates=False,
            )
        message = str(excinfo.value)
        assert "pyarrow" in message
        assert "osml-imagery-io[zarr]" in message


class TestNonParquetInputsUnchanged:
    """JSON and dict inputs must take the parent path, untouched by the override."""

    def test_dict_input_uses_plain_dict_references(self, data_file):
        path, _ = data_file
        fs = _make_fs({"k": [_file_url(path), 0, 5]})
        assert not isinstance(fs.references, LazyReferenceMapper)

    def test_json_input_uses_plain_dict_references(self, tmp_path, data_file):
        path, content = data_file
        index = tmp_path / "index.json"
        index.write_text(
            json.dumps({"version": 1, "refs": {"k": [_file_url(path), 0, 5]}})
        )
        fs = MultiReferenceFileSystem(fo=str(index), skip_instance_cache=True)
        assert not isinstance(fs.references, LazyReferenceMapper)
        assert fs.cat("k") == content[0:5]

    def test_json_path_inside_a_directory_still_json(self, tmp_path, data_file):
        """A ``.json`` path is never mistaken for a Parquet directory."""
        path, _ = data_file
        sub = tmp_path / "nested"
        sub.mkdir()
        index = sub / "index.json"
        index.write_text(
            json.dumps({"version": 1, "refs": {"k": [_file_url(path), 0, 5]}})
        )
        fs = MultiReferenceFileSystem(fo=str(index), skip_instance_cache=True)
        assert not isinstance(fs.references, LazyReferenceMapper)
