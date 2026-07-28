# Remote Range-Read Design

This document describes how osml-imagery-io reads from remote object stores
(e.g. fsspec/s3fs) without downloading the entire file first. It covers the
internal machinery a maintainer needs to reason about the feature: the `Remote`
`OwnedBuffer` backing, the slice-before-view rule, the per-codec I/O-hook
requirement, the fetch/prefetch seam, the concurrent `cat_ranges` batch path,
and which layers know a source is remote.

## The Problem

Reading imagery from cloud storage should not require pulling the whole file into
RAM. Metadata reads (`iminfo`), tile access (`tiles`, `DatasetReader.get_block`),
and VirtualiZarr index construction all need only a small fraction of a multi-GB
file — headers, offset tables, and the specific tiles requested. The library
already routes remote imagery through fsspec, which turns `seek`+`read` into HTTP
range requests. The goal is to preserve that `(offset, len) -> bytes` shape all
the way down to the decoders rather than collapsing it into a single whole-file
`.read()`.

The abstraction boundary is `(filesystem, path)` — an fsspec `AbstractFileSystem`
plus a path string. This is the canonical remote source: `IO.open` accepts an
explicit `filesystem=` (and the convenience functions forward it), and a bare
remote URL string (`s3://bucket/key`) is resolved internally via
`fsspec.core.url_to_fs` to the same `(filesystem, path)` pair. A raw fsspec
**file-like handle still works** and is probed for its `.fs`/`.path`
back-references as a fallback — kept deliberately so a caller who passes a handle
instead of `filesystem=`+path still gets the concurrent path where possible,
rather than silently degrading to serial reads. The library never learns what
backend it is reading from beyond the filesystem instance; choosing a
protocol/backend is entirely an fsspec concern.

`(filesystem, path)` is what makes the concurrent batch path (below) possible:
fsspec exposes multi-range concurrency on the *filesystem* (`cat_ranges`), not on
the file handle, so recovering the filesystem is what lets one logical tile's
scattered byte ranges be fetched in parallel.

## `OwnedBuffer` Gains a `Remote` Backing

`OwnedBuffer` wraps every byte source the readers consume. Its private
`BackingStore` enum has three variants:

| Variant | Bytes | `try_slice(a..b)` | `as_bytes()` |
|---------|-------|-------------------|--------------|
| `Mapped(Arc<Mmap>)` | OS demand-paged from disk | zero-copy (refcount + range adjust) | zero-copy view of the range |
| `Heap(Arc<Vec<u8>>)` | resident in RAM | zero-copy (refcount + range adjust) | zero-copy view of the range |
| `Remote(Arc<RemoteBacking>)` | **none resident** — fetched on demand | **fetch + cache `[a..b]`**, return a resident `Heap`-backed sub-buffer | **hard error** (see below) |

A `Remote` buffer logically spans the whole source — `len()` reports the total
size — but holds no bytes. It is constructed with `OwnedBuffer::from_remote(fetcher)`.

### `try_slice` is the fetch

The decisive contract: **`try_slice(a..b)` is the fetch point.** For `Mapped`/`Heap`
it stays zero-copy; for `Remote` it locks the fetcher, pulls (and caches) exactly
`[a..b]`, and returns a resident `Heap`-backed sub-buffer whose `as_bytes()` /
`as_ptr()` yield a stable, contiguous pointer. A network error surfaces as
`Err(CodecError::Remote(..))`.

Because `try_slice` already returned `Result<OwnedBuffer, CodecError>` before this
feature (from the zero-copy buffer refactor), remote fetch failures ride the
*existing* fallible seam. There is no out-of-band error channel, no poison state,
no `check_health()` — a fetch is just another `Result` threaded through code that
already returns `Result`.

Three sibling seams round out the buffer API:

- **`materialize()`** — obtain the whole logical range as a resident buffer. Thin
  wrapper over `try_slice(0..len)`: zero-copy for resident backings, one bounded
  whole-file fetch for `Remote`. This is the seam for **monolithic** formats (PNG,
  standalone JPEG, DTED's full-grid decode) that legitimately need every byte.
  Crucially, a fetch failure propagates through `?` rather than becoming an
  infallible-`as_bytes()` panic.
- **`subview(range)`** — narrow the logical range **without fetching**, for every
  backing. A `Remote` buffer stays `Remote` (bytes still fetched on demand by a
  later `try_slice`/callback); `Mapped`/`Heap` stay zero-copy. This is how a reader
  isolates a codestream/segment window of a remote source while keeping it remote
  (used by the embedded-J2K path so a `get_block` fetches only the target tile).
- **`read_range(offset, len)` / `resident_bytes()`** — the universal fetch seam and
  its zero-copy fast path. `resident_bytes()` returns `Some(&[u8])` for resident
  backings and `None` for `Remote`, so a parser can borrow when possible and fall
  back to `read_range` (a copy for resident, a fetch-and-cache for `Remote`)
  otherwise. The C-library I/O callbacks and the pure-Rust header/IFD parsers use
  these so they never depend on the whole file being resident.

## The Slice-Before-View Rule and the Full-Buffer Guard

A bare full-buffer `as_bytes()` on a `Remote` backing **panics**. This is a
deliberate correctness guard, not a normal control-flow path: a full-buffer view
of a remote source would silently force a whole-file download (or, worse, hide a
network error behind an infallible signature).

The rule for readers is therefore **slice before you view**:

```rust
// Wrong for a Remote backing — would materialize the whole file:
let image_data = &self.source_data.as_bytes()[data_start..data_end];

// Right — try_slice is the fetch; a fetch error returns via `?`:
let chunk = self.source_data.try_slice(data_start..data_end)?;
let image_data = chunk.as_bytes();   // as_bytes() on the resident sub-buffer is fine
```

### Monolithic vs block-capable

A bare full-buffer view is *not always wrong* — it depends on the format's access
pattern:

- **Block-capable formats** (NITF/JBP, TIFF, J2K) must read only bounded ranges. A
  full-buffer view here would defeat range reads, so it is a bug the guard catches.
  These readers parse headers/offset tables from bounded `try_slice`/`read_range`
  calls and materialize per-block on demand.
- **Monolithic formats** (PNG, standalone JPEG) have no sub-file structure to
  exploit — a whole-file read is the correct and only decode strategy. These readers
  call `materialize()?` and view the result. That is still a single bounded fetch on
  a `Remote` backing, and the failure is fallible.

The guard is thus an **unreachable-in-practice backstop**: after the reader
conversions, no real call site hits it. It is not `#[deprecated]` (that would fire
on the hundreds of legitimate resident-buffer call sites the type cannot distinguish
at compile time). If a future change reintroduces a bare full-buffer view on a
possibly-`Remote` buffer, the fetch-log remote tests — which assert "no single fetch
covered the whole file" — fail loudly.

## Per-Codec I/O Hook Requirement

The feature works only because **no codec assumes it holds a single addressable
buffer covering the entire image file.** Every codec either navigates file structure
through an I/O callback the library owns, or is handed pre-isolated per-block bytes.
The codecs split into two categories that must be wired differently:

### Category A — codec navigates via our seek/read callbacks

**Standalone TIFF/GeoTIFF (libtiff).** Opened with `mode="rm"` (memory-mapping
disabled) and `mapproc=None`, so libtiff cannot bypass the callbacks. Each tile is
read via `TIFFReadEncodedTile`, which makes libtiff seek to that tile's offset and
read through `tiff_read_proc` / `tiff_seek_proc`. `MemoryReadStreamData` holds the
source `OwnedBuffer`; the read callback takes the `resident_bytes()` fast path or
calls `read_range` on a `Remote` source. libtiff drives the ranges; the file is
never resident.

**Standalone J2K (OpenJPEG).** OpenJPEG is driven through
`memory_read_callback` / `memory_skip_callback` / `memory_seek_callback` (via
`OjpStream::from_owned_buffer` + `BufferReadStreamData`). This is the **always-works
hook point**: OpenJPEG's own seeks/reads materialize on demand for any codestream,
with no reliance on parsed tile-part metadata. When TLM markers are present OpenJPEG
seeks straight to the target tile; when they are absent it forward-scans (the
documented floor — see below).

The callback path is the **sole** J2K remote strategy. A tile-part slice
optimization (using a `scan_sot_markers` offset table to fetch only a tile's byte
range) was measured and **rejected**: it yields no fetch reduction over the callback
path. When TLM is present OpenJPEG already seeks to the tile; when TLM is absent,
building the tile-part table requires the same full-file scan the callback path
already performs. The amortized win for random access on non-TLM files lives in the
persisted VirtualiZarr index (scan once, persist offsets, then direct per-chunk range
GETs), not in per-read slicing.

### Category B — codec handed a contiguous buffer

**Standalone JPEG (libjpeg-turbo).** A single frame decoded in one shot. It
`materialize()`s its codestream — a bounded whole-file fetch on a `Remote` backing.
A standalone `.jpg` is not the multi-GB tiled case, so this is acceptable.

**NITF-embedded J2K / JPEG / NC (JBP).** Already per-block isolated: each block
decoder receives an `OwnedBuffer` sliced (via `subview`, keeping it `Remote`) to one
block's codestream/segment. This is the target shape the Zarr codec path already uses
in production. For embedded J2K, the decoder resolves the target tile's `(offset,
length)` parts from the TLM table and fetches them as one batched `read_ranges`
(concurrent `cat_ranges` for an fsspec source — see the batch path below), so a
`get_block` over a `Remote` source fetches only header + the target tile's parts,
and those parts are fetched in parallel rather than serially.

## Fetch and Prefetch Seam

The range machinery lives in `src/remote/`, deliberately independent of
`OwnedBuffer`:

```
RangeReader (trait)   — the (offset, len) -> Result<Vec<u8>> byte-source seam,
      │                  plus a batched read_many(&[(offset, len)]) -> Vec<Vec<u8>>;
      │                  both are &self so N threads read concurrently with no lock.
      │                  implemented by the Python-backed stream reader AND an
      │                  in-memory test fake (no Py<PyAny> needed for tests)
      ▼
StreamFetcher         — owns a RangeReader + RangeCache + PrefetchPolicy;
      │                  read_range()/read_ranges() are the fetch entry points:
      │                  serve-from-cache → plan → coalesce → fetch misses → cache
      ├── RangeCache      resident, non-overlapping ranges (extensible to eviction)
      └── PrefetchPolicy  given a requested range, may return extra ranges
```

`RemoteBacking` holds the `StreamFetcher` directly (no outer `Mutex`); the
fetcher is `Sync` via its own internal split: a **lock-free** `reader`
(`&self` methods) plus a short-held `Mutex<CacheState>` guarding only the cache
and policy. The reader is deliberately outside the lock so the network fetch
happens with the mutex released — concurrent `get_block` callers overlap their
S3 round-trips instead of serializing on the cache lock. See the concurrent
batch path below.

### The concurrent `cat_ranges` batch path

One logical J2K tile's tile-parts can live at widely separated file offsets. The
`RangeReader::read_many` seam fetches them **concurrently** rather than one at a
time. For an fsspec-backed source, `PyReadStream::read_many` recovers the
filesystem's `cat_ranges(paths, starts, ends)` — fsspec's synchronous
multi-range fetch — which fans the ranges out as concurrent GETs on its
background event loop and blocks the caller on a `threading.Event` (releasing the
GIL while blocked). The caller manages no event loop. `OwnedBuffer::read_ranges`
funnels a batch to this path for a `Remote` backing (cheap resident slices
otherwise), and the J2K decoder issues one `read_ranges` for a tile's parts
instead of a serial per-part loop.

Two levels of concurrency compose over the same shared s3fs connection pool:
**within a block** (a tile's parts via one `cat_ranges`) and **across blocks**
(multiple `get_block` callers on their own threads, overlapping because the
reader is lock-free). Non-fsspec handles (`io.BytesIO`, plain files) fall back to
a serial `read_at` loop — correct, just not concurrent, which is fine for
sources that are not latency-bound.

The containment invariant that makes concurrent `&self` reads safe: **all**
concurrent fetches route through the stateless `cat_ranges` path (no cursor),
while the stateful `seek`+`read` cursor path stays single-threaded (a debug
re-entry guard on `PyReadStream::read_at` fires if two threads ever race the
cursor). The duplicate-fetch race — two callers missing the same span and both
fetching — is harmless: `RangeCache::insert` merges/dedups overlapping bytes, so
the cache stays coherent and only a little bandwidth is wasted.

### PrefetchPolicy

`PrefetchPolicy::plan(requested, total_size) -> Vec<Range<u64>>` decides which ranges
the fetcher actually pulls — the requested range plus any speculative extras. Because
the fetcher self-heals (an under-sized prefetch simply triggers another fetch on the
next access), a policy is a pure optimization with no correctness cliff.

v1 ships `HeaderAwarePolicy`: on an access that starts within the file's head it
eagerly fetches a header region (`[0, header_len)`), coalescing the header +
offset-table bootstrap reads into a single fetch; a random access deep in the file
does not drag the header along. The header-region size is supplied per format by the
IO dispatch layer (which knows the format); the fetcher stays format-agnostic. A
future spatial policy (Z-order / Hilbert / 2D adjacency) is just another
implementation of the trait and needs no reader or buffer-API change.

## Layer Awareness

Only Layer 2 (the PyO3 IO binding) knows a source is remote. Everything above and
below sees the ordinary abstractions.

| Layer | Component | Knows about remote fetch? |
|-------|-----------|---------------------------|
| 1 | Python API (`IO.open`) | No |
| 2 | PyO3 binding (`bindings/io.rs`) | **Yes** — probes the stream, constructs the `Remote` buffer, supplies the header hint |
| 3 | `PyDatasetReader` wrapper | No — thin delegation |
| 4 | `DatasetReader` trait | No |
| 5 | Format readers | Only that byte access is via `try_slice`/`materialize`/callbacks and can return `Err` |
| 6 | Asset providers | No |
| 7 | C FFI (libtiff, OpenJPEG) | No — callbacks call the fetcher |

### Layer 2 wiring

`create_reader_from_stream` (and `open_multi_stream_with_roles`, per source) probes
the stream and either builds a `Remote` buffer or falls back to the full-read path:

1. **`remote_header_hint(format)`** — returns a header-prefetch size for the
   block-capable formats whose readers are proven remote-safe (TIFF, J2K, NITF/JBP,
   DTED) and `None` otherwise. This is the single gate. PNG and standalone JPEG stay
   on the full-read fallback by routing choice, not necessity: their readers *are*
   remote-safe via `materialize()`, but chunking a mandatory whole-file read through
   range GETs has no benefit.
2. **`probe_seekable_size(stream)`** — returns `Some(total_size)` iff the stream has
   `read`, `seekable()` is true, and a size is known (prefers a `.size` attribute —
   fsspec exposes it — else `seek(0, SEEK_END)`); it restores the read position to 0
   and treats size 0 as unknown.
3. If both succeed, wrap a `PyReadStream` (the `RangeReader` adapter that turns
   `seek`+`read` into range reads under `Python::attach`) in a `StreamFetcher` with a
   `HeaderAwarePolicy(hint)` and build the `Remote` `OwnedBuffer`. Otherwise fall back
   to `read_stream_bytes` → `from_vec`, unchanged.

Non-seekable or unknown-size streams therefore behave exactly as before (full read).
The multi-source path probes each source independently, so a mixed
seekable/non-seekable list degrades per source.

## Testing Approach

The whole point of the byte-range foundation (over the rejected page-fault approach)
is testability with no OS machinery, no signal handlers, and no threads. An in-memory
fake `RangeReader` — a `Vec<u8>` plus a call log — backs the fetcher, so tests assert
*which ranges* were fetched and that the whole file is never pulled. Coverage:

- **Unit** — `try_slice` on `Remote` fetches exactly the requested range (plus policy
  prefetch); repeated access hits cache; a fetch error propagates as `CodecError`; the
  full-buffer guard fires on `Remote`.
- **Per-format remote decode** — for TIFF, J2K, JBP (NC/embedded), PNG, DTED: metadata
  read + `get_block` over a `Remote` buffer yield correct pixels. "Never fetch the
  whole file" is asserted for the block-capable formats; the monolithic formats (PNG,
  standalone JPEG) and DTED's full-grid block assert byte-identity only.
- **Property** — decoding via a `Remote` buffer is byte-identical to decoding the same
  file via `Heap`, under randomized block-visit order.
- **Python integration** — `IO.open` with a range-logging fake file-like object (and a
  real fsspec `LocalFileSystem` handle) drives `iminfo`, `tiles`, `get_block`, and
  VirtualiZarr index construction without a full download.
- **Fallback** — non-seekable / unknown-size streams still work via the full-read path.

## See Also

- [Synthetic Codestream Codec Pattern](zarr-codec-design.md) — the already
  range-based VirtualiZarr consumer path.
- [Native Library FFI Design](native-library-ffi.md) — the libtiff/OpenJPEG FFI
  callback patterns this feature reuses.
- [API Design](api-design.md) — the public `DatasetReader` / `IO.open` surface, which
  this feature leaves unchanged.
- [Datasets and the IO Interface](../user-guide/datasets-and-io.md) — user-facing
  guidance on reading from remote handles.
