"""MultiReferenceFileSystem — scatter-gather I/O for multi-range Kerchunk references.

Extends fsspec's ``ReferenceFileSystem`` with a fourth reference form::

    ["url", [[offset, length], [offset, length], ...]]

This allows a single Zarr chunk key to map to multiple non-contiguous byte
ranges in the same file — required for JPEG 2000 codestreams with interleaved
tile-parts (RLCP / RPCL progression order).

All existing reference forms (inline, whole-file, single-range) are handled
by the parent class unchanged.
"""

from __future__ import annotations

import asyncio
import base64
import logging

from fsspec.core import split_protocol
from fsspec.implementations.reference import ReferenceFileSystem

logger = logging.getLogger(__name__)


class MultiReferenceFileSystem(ReferenceFileSystem):
    """ReferenceFileSystem with multi-range chunk support.

    Extends the Kerchunk reference spec to support a fourth reference form::

        ["url", [[offset, length], [offset, length], ...]]

    for chunks whose data spans multiple non-contiguous byte ranges.

    All existing reference forms (inline, whole-file, single-range) are
    handled by the parent class unchanged.
    """

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

    @staticmethod
    def _is_multi_range(part) -> bool:
        """Detect multi-range reference entries.

        A reference is multi-range when it is a 2-element list whose second
        element is a non-empty list of 2-element lists (each ``[offset, length]``).
        """
        return (
            isinstance(part, list)
            and len(part) == 2
            and isinstance(part[1], list)
            and len(part[1]) > 0
            and isinstance(part[1][0], list)
        )

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
