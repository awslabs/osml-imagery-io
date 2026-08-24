"""MultiReferenceFileSystem — scatter-gather I/O for multi-range Kerchunk references.

Extends fsspec's ``ReferenceFileSystem`` with a fourth reference form::

    ["url", [[offset, length], [offset, length], ...]]

This allows a single Zarr chunk key to map to multiple non-contiguous byte
ranges in the same file — required for JPEG 2000 codestreams with interleaved
tile-parts (RLCP / RPCL progression order).

All existing reference forms (inline, whole-file, single-range) are handled
by the parent class unchanged.

Template expansion (Kerchunk v1 ``"templates"`` dict) is supported for all
reference forms including multi-range entries.  Use ``template_overrides``
at construction time to resolve portable ``{{base}}`` placeholders::

    fs = MultiReferenceFileSystem(
        fo="image.tile_index.json",
        template_overrides={"base": "s3://bucket/path/"},
    )

Kerchunk **Parquet** indexes are also readable here, which they are not through
the stock ``ReferenceFileSystem`` — see :class:`ArrayOnlyReferenceMapper` for the
two container-level defects that requires working around.  Parquet indexes need
``pyarrow``, supplied by the ``osml-imagery-io[zarr]`` extra.
"""

from __future__ import annotations

import asyncio
import base64
import logging
from functools import lru_cache

from fsspec.core import split_protocol
from fsspec.implementations.reference import LazyReferenceMapper, ReferenceFileSystem

logger = logging.getLogger(__name__)

_PYARROW_HINT = (
    "Reading a Kerchunk Parquet tile index requires the 'pyarrow' package, which "
    "is not installed. Install it with 'pip install osml-imagery-io[zarr]' (the "
    "zarr extra supplies pyarrow), or use a '.json' tile index instead. Note "
    "writing a Parquet index needs pyarrow too — the dependency is not read-only."
)


def _is_null(value) -> bool:
    """Is *value* a Parquet null, as either engine may have decoded it?

    ``pyarrow`` and ``fastparquet`` disagree on how a null in an object column
    round-trips: ``pyarrow`` yields ``None`` for ``raw`` but ``float('nan')`` for
    ``path``, while ``fastparquet`` yields ``nan`` for both.  ``nan`` is the only
    float that is not equal to itself, and ``numpy.float64`` subclasses ``float``,
    so this catches every form without importing pandas or numpy.
    """
    return value is None or (isinstance(value, float) and value != value)


def _null_normalized(array):
    """Return *array* with Parquet nulls replaced by ``None``.

    Only float and object columns can carry a null, so integer columns
    (``offset``, ``size``) are returned untouched.  The result is a fresh
    ``dtype=object`` array: the divergence between engines is per column dtype,
    not per engine, so testing ``dtype.kind`` for ``"f"`` alone would miss the
    object-dtype ``path`` column that ``pyarrow`` fills with ``nan``.
    """
    if array.dtype.kind not in "fO":
        return array

    import numpy as np

    out = np.empty(len(array), dtype=object)
    for i, value in enumerate(array):
        out[i] = None if _is_null(value) else value
    return out


def is_multi_range_ref(part) -> bool:
    """Is *part* a multi-range reference ``["url", [[offset, length], ...]]``?

    A reference is multi-range when it is a 2-element list whose second element is
    a non-empty list of lists.  The single source of truth for the form, shared
    with the writer in ``virtualizarr_parsers`` so the two ends of the contract
    cannot drift: a writer that emitted a shape the reader did not recognize would
    produce an index that reads as missing chunks rather than as a failure.
    """
    return (
        isinstance(part, list)
        and len(part) == 2
        and isinstance(part[1], list)
        and len(part[1]) > 0
        and isinstance(part[1][0], list)
    )


def _expand_templates(url, templates):
    """Substitute ``{{name}}`` placeholders in *url* from the *templates* map.

    Only the simple-replace form is handled, which is what this library's own
    writer emits (``template_base="{{base}}"`` produces ``{{base}}<filename>``)
    and what the Kerchunk spec calls a "simple" template.  Non-string and
    placeholder-free values pass through untouched.
    """
    if not isinstance(url, str) or "{{" not in url:
        return url
    for name, value in templates.items():
        if callable(value):
            value = value()
        url = url.replace("{{" + name + "}}", value)
    return url


class ArrayOnlyReferenceMapper(LazyReferenceMapper):
    """``LazyReferenceMapper`` that can open a *hierarchical* Parquet index.

    fsspec's own lazy Parquet mapper was designed against a flat, single-array
    store and cannot read the hierarchical (GeoZarr multiscales) stores this
    library writes.  Two independent container-level defects stack:

    1. **The record loader goes through pandas, and nulls decode inconsistently.**
       Upstream's loader is ``pandas.read_parquet(..., engine=self.engine)``
       followed immediately by ``to_numpy()`` per column, so pandas decodes via
       pyarrow and is then thrown away — making ``pandas`` a runtime requirement of
       merely *reading* a tile index, in exchange for nothing.  Worse, the writer's
       engine is *not recorded in the store* (``.zmetadata`` holds only
       ``{"metadata": ..., "record_size": ...}``) while ``LazyReferenceMapper``
       defaults to ``fastparquet`` on read, and the engines disagree on nulls:
       upstream's ``raw is not None`` test takes the inline-data branch for ``nan``,
       surfacing as ``TypeError: object of type 'numpy.float64' has no len()``.
       Fixed here by loading with ``pyarrow.parquet.read_table`` directly *and*
       normalizing nulls, so this library never imports pandas and an index written
       by some other tool with ``fastparquet`` still reads correctly.
    2. **Group prefixes are mistaken for array fields.**  Upstream ``listdir()``
       derives its field list by stripping the last segment off every metadata
       key, excluding only *root-level* ``.z*`` keys.  A nested ``0/.zgroup``
       therefore strips to the prefix ``"0"``, which every consumer treats as an
       array — ``_get_chunk_sizes("0")`` then raises ``KeyError: '0/.zarray'``
       while the store is still being constructed.  Removing the nested group
       keys is not an option: zarr requires them.  Fixed here by reporting only
       prefixes that actually have a ``.zarray``.

    A third gap is closed here too, in *templates*.  The Kerchunk JSON container
    carries a ``"templates"`` dict alongside its refs, but the Parquet container
    has nowhere to put one, so a portable index written with
    ``template_base="{{base}}"`` stores literal ``{{base}}<filename>`` URLs and
    upstream never expands them — reads fail with ``ReferenceNotReachable`` on the
    unsubstituted URL.  Pass *templates* (from ``template_overrides``) to expand
    them as refs are loaded.

    Finally, this class decodes the **multi-range** rows this library's writer
    emits.  Upstream's record schema has only scalar ``offset`` / ``size`` columns,
    so the writer appends ``range_path`` / ``offsets`` / ``sizes`` (the latter two
    ``list<int64>``) and leaves ``path`` null on those rows;
    :meth:`_load_one_key` recognizes them and returns the multi-range reference
    form ``[url, [[offset, length], ...]]``, which
    :meth:`MultiReferenceFileSystem._cat_common` then fans out.  Because ``path``
    is null, a reader without this override gets a broken reference rather than one
    fragment mistaken for a whole chunk — see :meth:`_load_one_key`.

    One consequence of that null is worth knowing: upstream's ``ls()`` builds its
    chunk entries from ``_generate_all_records`` and drops any row whose first
    column is falsy, so **multi-range chunks do not appear in a listing** of their
    array field.  Reads are unaffected — Zarr fetches chunks by key, and both the
    ``zarr_format=2`` mapper and the ``FsspecStore`` path decode multi-range chunks
    correctly — so this is cosmetic and left alone rather than fixed by
    re-implementing upstream's ``ls``.  It is asserted in the tests so it cannot
    drift into something load-bearing unnoticed.  Note the JSON container does list
    them, so the two containers differ here.

    These fixes are confined to four overrides — ``listdir()``, ``setup()``,
    ``_load_one_key()`` and ``__init__`` — and only ``setup()`` restates any upstream
    logic: the four lines that read ``.zmetadata`` into ``_items`` / ``record_size``
    / ``zmetadata``, which come along with replacing the loader rather than wrapping
    it.  Everything that consumes those arrays — ``_key_to_record``,
    ``_get_chunk_sizes``, ``ls``, ``items()`` — is upstream's and untouched.

    The *write* path does not route through ``LazyReferenceMapper`` at all:
    ``virtualizarr_parsers._write_parquet_index`` emits the store directly, because
    ``LazyReferenceMapper.write()`` builds a hardcoded four-column DataFrame with no
    seam to widen for the range columns.  What the two ends share is the on-disk
    layout, not any code.
    """

    def __init__(self, root, fs=None, templates=None, **kwargs):
        # Set before super().__init__: it may trigger setup() via __getattr__.
        self.templates = dict(templates or {})
        # ``engine`` no longer selects anything — :meth:`setup` reads with pyarrow
        # unconditionally.  It is still set to "pyarrow" because upstream's
        # ``__init__`` uses that value to run ``find_spec("pyarrow")``, which is the
        # check that lets a missing install surface as ``_PYARROW_HINT`` rather than
        # as a ModuleNotFoundError from inside the first chunk read.
        kwargs.setdefault("engine", "pyarrow")
        try:
            super().__init__(root, fs=fs, **kwargs)
        except ImportError as exc:
            # Upstream raises a bare "engine choice `pyarrow` is not installed."
            raise ImportError(_PYARROW_HINT) from exc

        # ``LazyReferenceMapper.__init__`` applies ``lru_cache`` to ``self.listdir``
        # per instance, which already picks up this subclass's override.  Wrap it
        # here only if that upstream detail ever goes away — ``listdir()`` is hot
        # (every key lookup reaches it) and must not rescan ``.zmetadata``.
        if not hasattr(self.listdir, "cache_clear"):
            self.listdir = lru_cache()(self.listdir)

    def listdir(self):
        """List the array fields — prefixes owning a ``.zarray`` document.

        Narrower than upstream, which also reports group prefixes.  See defect 2
        in the class docstring.
        """
        suffix = "/.zarray"
        return {
            key[: -len(suffix)] for key in self.zmetadata if key.endswith(suffix)
        }

    def setup(self):
        """Read ``.zmetadata`` and install a ``pyarrow``-only record loader.

        Replaces ``super().setup()`` rather than wrapping it.  Upstream's loader is
        ``pandas.read_parquet(...)`` followed immediately by ``to_numpy()`` per
        column — pandas decodes through pyarrow and is then discarded — so calling it
        would make ``pandas`` a runtime requirement of merely *reading* a tile index,
        for no capability.  ``pyarrow.parquet.read_table`` produces the same values
        directly; see defect 1 in the class docstring.

        The three column fix-ups this class exists for all happen here, which is why
        the loader rather than ``_load_one_key`` is the seam:

        * **Nulls** are normalized to ``None``.  pyarrow already yields ``None``, so
          this is now insurance for stores written by other tools — fastparquet types
          an all-null column ``float64``, whose ``nan`` would sail through upstream's
          ``raw is not None`` test as if it were chunk data.
        * **Templates** are expanded in *both* URL columns.  Missing ``range_path``
          would leave ``{{base}}`` literal in exactly the entries a portable index
          most needs — the multi-range ones — and the failure would look like an
          unreachable reference rather than an unexpanded template.
        * Everything downstream (``_load_one_key``, ``_key_to_record``, ``ls``) is
          upstream's and reads these arrays unchanged.
        """
        import io
        import json

        import pyarrow.parquet as pq

        self._items = {}
        self._items[".zmetadata"] = self.fs.cat_file(
            "/".join([self.root, ".zmetadata"])
        )
        met = json.loads(self._items[".zmetadata"])
        self.record_size = met["record_size"]
        self.zmetadata = met["metadata"]

        templates = self.templates

        @lru_cache(maxsize=self.cache_size)
        def open_refs(field, record):
            path = self.url.format(field=field, record=record)
            try:
                table = pq.read_table(io.BytesIO(self.fs.cat_file(path)))
            except OSError:
                # Upstream returns None here and ``_load_one_key`` relies on that
                # to raise KeyError; preserve the short-circuit.
                return None
            refs = {
                name: _null_normalized(
                    table.column(name).to_numpy(zero_copy_only=False)
                )
                for name in table.schema.names
            }
            if templates:
                import numpy as np

                for column in ("path", "range_path"):
                    if column in refs:
                        refs[column] = np.array(
                            [
                                _expand_templates(url, templates)
                                for url in refs[column]
                            ],
                            dtype=object,
                        )
            return refs

        self.open_refs = open_refs

    def _load_one_key(self, key):
        """Return the reference for *key*, decoding multi-range rows.

        A multi-range row is one whose ``offsets`` cell is populated; it decodes to
        ``[range_path, [[offset, length], ...]]``, the fourth reference form
        :class:`MultiReferenceFileSystem` fans out.  Everything else — metadata,
        inline ``raw``, whole-file and single-range references — delegates to
        upstream unchanged.

        Note what a reader *without* this override sees for such a row: ``path`` is
        null, so upstream raises
        ``KeyError("This reference does not exist or has been deleted")`` once nulls
        are normalized, and returns a malformed single-element reference if they are
        not.  Either way it fails; neither hands back pixels.  That is the point of
        leaving ``path`` null — seeding it with the first fragment would make an
        unaware reader decode one sixth of a chunk as if it were the whole thing.
        """
        ranges = self._multi_range_for(key)
        if ranges is not None:
            return ranges
        return super()._load_one_key(key)

    def _multi_range_for(self, key):
        """Decode *key* as a multi-range reference, or ``None`` if it is not one.

        Returns ``None`` — rather than raising — for every key that is not a chunk
        in the widened schema, so :meth:`_load_one_key` can fall through to
        upstream and keep a single place that decides what a missing key means.
        """
        if key in self._items or key in self.zmetadata:
            return None
        if "/" not in key or self._is_meta(key):
            return None

        field, _ = key.rsplit("/", 1)
        try:
            record, index, _ = self._key_to_record(key)
            refs = self.open_refs(field, record)
        except (KeyError, ValueError, TypeError, FileNotFoundError):
            return None
        if not refs or "offsets" not in refs:
            return None

        offsets = refs["offsets"][index]
        if offsets is None or len(offsets) == 0:
            return None
        sizes = refs["sizes"][index]
        url = refs["range_path"][index]
        return [
            url,
            [[int(offset), int(length)] for offset, length in zip(offsets, sizes)],
        ]


class MultiReferenceFileSystem(ReferenceFileSystem):
    """ReferenceFileSystem with multi-range chunk support.

    Extends the Kerchunk reference spec to support a fourth reference form::

        ["url", [[offset, length], [offset, length], ...]]

    for chunks whose data spans multiple non-contiguous byte ranges.

    All existing reference forms (inline, whole-file, single-range) are
    handled by the parent class unchanged.

    Kerchunk Parquet reference directories are opened through
    :class:`ArrayOnlyReferenceMapper` rather than the parent's default mapper, so
    hierarchical (multiscales) Parquet indexes are readable — including portable
    ones, whose ``{{base}}`` placeholders are resolved from
    ``template_overrides``.  JSON and dict inputs take the parent path unchanged.
    """

    def __init__(self, fo, *args, **kwargs):
        """Open *fo*, routing Parquet reference directories to our own mapper.

        The parent hardcodes ``LazyReferenceMapper(..., cache_size=...)`` with no
        ``engine``, which cannot read a hierarchical Parquet index — see
        :class:`ArrayOnlyReferenceMapper`.  Since the mapper is built inline in
        ``ReferenceFileSystem.__init__`` there is no seam to override, so the
        Parquet case is detected here and the parent is entered with an empty
        ``fo`` before the real mapper is assigned.

        Note the mapper is installed on the *instance*.  Patching the module
        global around ``super().__init__`` is not sufficient: a second mapper is
        built lazily during the first chunk read, and would be built unpatched.
        """
        parquet_root = self._parquet_root(fo, kwargs)
        if parquet_root is None:
            super().__init__(fo, *args, **kwargs)
            return

        ref_fs, root = parquet_root
        cache_size = kwargs.pop("cache_size", 128)
        # An empty dict, not the mapper: the parent routes any non-str ``fo``
        # through ``_process_references``, which fails on a mapper with
        # "TypeError: Object of type int64 is not JSON serializable".
        super().__init__({}, *args, **kwargs)
        # A Parquet store cannot carry a Kerchunk "templates" dict, so the
        # overrides are the only source of substitutions — there is no
        # store-side default to merge them into, unlike the JSON path.
        self.references = ArrayOnlyReferenceMapper(
            root,
            fs=ref_fs,
            cache_size=cache_size,
            templates=self.template_overrides,
        )

    @staticmethod
    def _parquet_root(fo, kwargs):
        """Return ``(ref_fs, root)`` if *fo* names a Parquet reference directory.

        Mirrors the parent's own discrimination: a ``str`` that is not a ``.json``
        path and either has a Parquet-ish suffix or resolves to a directory.
        """
        if not isinstance(fo, str):
            return None

        import fsspec

        dic = dict(
            **(kwargs.get("ref_storage_args") or kwargs.get("target_options") or {}),
            protocol=kwargs.get("target_protocol"),
        )
        ref_fs, root = fsspec.core.url_to_fs(fo, **dic)
        if ".json" in root:
            return None
        if fo.endswith(("parq", "parquet", "/")) or ref_fs.isdir(root):
            return ref_fs, root
        return None

    def ls(self, path, detail=True, **kwargs):
        """List *path*, falling back to the directory cache for group prefixes.

        The parent short-circuits to ``LazyReferenceMapper.ls`` for any lazy
        mapper and never consults ``dircache``.  Because
        :class:`ArrayOnlyReferenceMapper` reports only array fields, that lookup
        raises for a group prefix (and for the root) — so fall back to the
        ``dircache`` built by :meth:`_dircache_from_items`, which does carry the
        group levels.
        """
        try:
            return super().ls(path, detail=detail, **kwargs)
        except (FileNotFoundError, KeyError):
            if not isinstance(self.references, LazyReferenceMapper):
                raise
        stripped = self._strip_protocol(path)
        if not self.dircache:
            self._dircache_from_items()
        out = self._ls_from_cache(stripped)
        if out is None:
            raise FileNotFoundError(path)
        return out if detail else [entry["name"] for entry in out]

    def isdir(self, path):
        """Is *path* a directory (group prefix or array field)?

        The parent asks ``LazyReferenceMapper.listdir()``, which for
        :class:`ArrayOnlyReferenceMapper` deliberately omits group prefixes, so
        consult the directory cache as well.
        """
        if super().isdir(path):
            return True
        if not isinstance(self.references, LazyReferenceMapper):
            return False
        if not self.dircache:
            self._dircache_from_items()
        return self._strip_protocol(path) in self.dircache

    def _dircache_from_items(self):
        """Build directory cache, handling multi-range entries.

        Overrides parent because ``ReferenceFileSystem._dircache_from_items``
        unpacks every list reference as ``(url, offset, size)`` which fails
        for multi-range entries ``["url", [[offset, length], ...]]``.
        """
        self.dircache = {"": []}
        for path, part in self.references.items():
            if isinstance(part, (bytes, str)) or hasattr(part, "to_bytes"):
                size = len(part)
            elif len(part) == 1:
                size = None
            elif self._is_multi_range(part):
                # Sum of all sub-range lengths
                size = sum(length for _, length in part[1])
            else:
                _, _, size = part

            par = path.rsplit("/", 1)[0] if "/" in path else ""
            par0 = par
            subdirs = [par0]
            while par0 and par0 not in self.dircache:
                par0 = self._parent(par0)
                subdirs.append(par0)

            subdirs.reverse()
            for parent, child in zip(subdirs, subdirs[1:]):
                if child not in self.dircache:
                    if parent in self.dircache:
                        self.dircache[parent].append(
                            {"name": child, "type": "directory", "size": 0}
                        )
                    self.dircache[child] = []

            self.dircache[par].append({"name": path, "type": "file", "size": size})

    def _process_references1(self, references, template_overrides=None):
        """Extend parent to handle template expansion for multi-range entries.

        The parent ``_process_references1`` expands ``{{template}}``
        placeholders in URL strings for standard reference forms (1-element
        and 3-element lists).  Multi-range entries ``["url", [[o, l], ...]]``
        are 2-element lists whose second element is a list of lists — the
        parent crashes on these because it only handles ``len(v) == 1`` or
        ``len(v) == 3``.

        This override extracts multi-range entries before calling the parent,
        then adds them back with templates expanded.
        """
        # Extract multi-range entries from refs before parent processes them
        raw_refs = references.get("refs", {})
        multi_range_entries = {}
        if isinstance(raw_refs, dict):
            for k, v in list(raw_refs.items()):
                if isinstance(v, list) and self._is_multi_range(v):
                    multi_range_entries[k] = v

            # Remove multi-range entries so parent doesn't choke on them
            if multi_range_entries:
                filtered_refs = {
                    k: v for k, v in raw_refs.items()
                    if k not in multi_range_entries
                }
                references = dict(references)
                references["refs"] = filtered_refs

        # Let parent handle standard refs + templates
        super()._process_references1(references, template_overrides)

        # Now add multi-range entries back, expanding templates if active
        for k, v in multi_range_entries.items():
            u = v[0]
            if self.templates and "{{" in u:
                if self.simple_templates:
                    u = (
                        u.replace("{{", "{")
                        .replace("}}", "}")
                        .format(**self.templates)
                    )
                else:
                    import jinja2
                    u = jinja2.Template(u).render(**self.templates)
            self.references[k] = [u, v[1]]

    @staticmethod
    def _is_multi_range(part) -> bool:
        """Detect multi-range reference entries.

        Delegates to :func:`is_multi_range_ref`, which the writer shares.
        """
        return is_multi_range_ref(part)

    def _cat_common(self, path, start=None, end=None):
        """Resolve a reference key to bytes.

        Overrides parent to detect multi-range entries and fetch+concatenate
        multiple byte ranges synchronously.  All other reference types
        delegate to the parent implementation.
        """
        path = self._strip_protocol(path)
        try:
            part = self.references[path]
        except KeyError as exc:
            raise FileNotFoundError(path) from exc

        # Inline string → encode to bytes
        if isinstance(part, str):
            part = part.encode()

        # Inline bytes (including base64-encoded)
        if isinstance(part, bytes):
            if part.startswith(b"base64:"):
                part = base64.b64decode(part[7:])
            return part, None, None

        # Multi-range: fetch all ranges and concatenate
        if self._is_multi_range(part):
            logger.debug("Reference: %s, multi-range (%d ranges)", path, len(part[1]))
            return self._fetch_multi_range_sync(part), None, None

        # Everything else (whole-file, single-range) → parent
        return super()._cat_common(path, start=start, end=end)

    def _fetch_multi_range_sync(self, part: list) -> bytes:
        """Fetch multiple byte ranges sequentially and concatenate."""
        url = part[0]
        ranges = part[1]
        protocol, _ = split_protocol(url)
        fs = self.fss[protocol]
        parts: list[bytes] = []
        for offset, length in ranges:
            parts.append(fs.cat_file(url, start=offset, end=offset + length))
        return b"".join(parts)

    async def _cat_file(self, path, start=None, end=None, **kwargs):
        """Async variant — issues concurrent fetches for multi-range entries."""
        path = self._strip_protocol(path)
        try:
            part = self.references[path]
        except KeyError as exc:
            raise FileNotFoundError(path) from exc

        # Inline string → encode to bytes
        if isinstance(part, str):
            part = part.encode()

        # Inline bytes (including base64-encoded)
        if isinstance(part, bytes):
            if part.startswith(b"base64:"):
                part = base64.b64decode(part[7:])
            return part

        # Multi-range: concurrent async fetches
        if self._is_multi_range(part):
            logger.debug("Reference: %s, async multi-range (%d ranges)", path, len(part[1]))
            return await self._fetch_multi_range_async(part)

        # Everything else → parent
        return await super()._cat_file(path, start=start, end=end, **kwargs)

    async def _fetch_multi_range_async(self, part: list) -> bytes:
        """Fetch multiple byte ranges concurrently and concatenate in order."""
        url = part[0]
        ranges = part[1]
        protocol, _ = split_protocol(url)
        fs = self.fss[protocol]

        async def _fetch_one(offset: int, length: int) -> bytes:
            return await fs._cat_file(url, start=offset, end=offset + length)

        results = await asyncio.gather(*[_fetch_one(o, n) for o, n in ranges])
        return b"".join(results)
