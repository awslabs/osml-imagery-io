use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use memmap2::Mmap;

use crate::error::CodecError;
use crate::remote::StreamFetcher;

#[derive(Clone)]
enum BackingStore {
    Mapped(Arc<Mmap>),
    Heap(Arc<Vec<u8>>),
    /// A remote byte source: bytes are fetched (and cached) on demand by
    /// `try_slice`, never held whole-file resident. See
    /// [`crate::remote`] and the remote range-read design.
    Remote(Arc<RemoteBacking>),
}

/// The shared state behind a `Remote` [`OwnedBuffer`].
///
/// Holds the [`StreamFetcher`] **lock-free**: the fetcher's `&self` methods are
/// internally synchronized (a lock-free `&self` reader plus a short-held cache
/// mutex, released across the network fetch — see [`StreamFetcher`]), so N
/// threads can fetch through one backing concurrently. This is what lets
/// overlapping `get_block` callers across blocks issue their `cat_ranges` in
/// parallel. `try_slice` pulls the requested range and returns a `Heap`-backed
/// sub-buffer. The total source size lives on the buffer's `range` (set at
/// construction from the fetcher), so it is not duplicated here.
struct RemoteBacking {
    fetcher: StreamFetcher,
}

/// A reference-counted, zero-copy-sliceable view into a byte buffer.
///
/// Clone is O(1) (refcount increment). Slicing is O(1) (refcount increment +
/// range adjustment). The backing store stays alive as long as any clone or
/// slice exists.
#[derive(Clone)]
pub struct OwnedBuffer {
    store: BackingStore,
    range: Range<usize>,
}

impl OwnedBuffer {
    /// Wrap a memory-mapped file.
    pub fn from_mmap(mmap: Mmap) -> Self {
        let len = mmap.len();
        Self {
            store: BackingStore::Mapped(Arc::new(mmap)),
            range: 0..len,
        }
    }

    /// Wrap a heap-allocated byte vector. Reuses the Vec's allocation (no copy).
    pub fn from_vec(data: Vec<u8>) -> Self {
        let len = data.len();
        Self {
            store: BackingStore::Heap(Arc::new(data)),
            range: 0..len,
        }
    }

    /// Wrap a remote byte source.
    ///
    /// The returned buffer logically spans the whole source (`len()` reports the
    /// source's total size) but holds **no bytes resident**. Bytes are obtained
    /// by [`try_slice`](Self::try_slice) / [`slice`](Self::slice), which fetch
    /// (and cache) the requested range and return a `Heap`-backed sub-buffer with
    /// a stable contiguous pointer. A bare full-buffer
    /// [`as_bytes`](Self::as_bytes) on a remote buffer is a hard error (it would
    /// force a whole-file download) — see the slice-before-view rule.
    pub fn from_remote(fetcher: StreamFetcher) -> Self {
        let total_size = fetcher.total_size() as usize;
        Self {
            store: BackingStore::Remote(Arc::new(RemoteBacking { fetcher })),
            range: 0..total_size,
        }
    }

    /// View the bytes this buffer represents.
    ///
    /// For `Mapped`/`Heap` backings this is a zero-copy view of the buffer's
    /// range. For a **`Remote`** backing a bare full-buffer view panics: remote
    /// bytes must be obtained through [`try_slice`](Self::try_slice) /
    /// [`slice`](Self::slice) (bounded range) or [`materialize`](Self::materialize)
    /// (whole range, fallibly) first, all of which return a resident `Heap`-backed
    /// buffer whose `as_bytes()` works normally.
    ///
    /// This panic is a **backstop guard**, not a normal control-flow path: no
    /// real call site reaches it (monolithic readers `materialize()?`,
    /// block-capable readers slice bounded ranges). It
    /// exists so that if a future change reintroduces a bare full-buffer view on a
    /// possibly-`Remote` buffer, the fetch-log Remote tests fail loudly instead of
    /// the code silently downloading (or worse, hiding a network error behind an
    /// infallible signature).
    pub fn as_bytes(&self) -> &[u8] {
        if matches!(self.store, BackingStore::Remote(_)) {
            panic!(
                "as_bytes() called on a Remote OwnedBuffer: fetch bytes with \
                 try_slice(range) first (slice-before-view). A bare full-buffer \
                 view would force a whole-file download."
            );
        }
        &self.backing_bytes()[self.range.clone()]
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.range.len()
    }

    /// True if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Create a sub-slice. Panics on out-of-bounds (or on a remote fetch error).
    ///
    /// Zero-copy for `Mapped`/`Heap`. For a `Remote` backing this is the fetch
    /// point: it pulls (and caches) `range` and returns a resident `Heap`-backed
    /// sub-buffer; prefer [`try_slice`](Self::try_slice) when a fetch failure
    /// should be handled rather than panic.
    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(range.start <= range.end, "invalid range: start > end");
        assert!(range.end <= self.len(), "slice out of bounds");
        match &self.store {
            BackingStore::Remote(backing) => self
                .fetch_remote_slice(backing, range)
                .expect("remote slice fetch failed"),
            _ => self.slice_unchecked(range),
        }
    }

    /// Create a sub-slice. Returns error on out-of-bounds or remote fetch failure.
    ///
    /// **This is the fetch point for a `Remote` backing:** it fetches (and
    /// caches) `range` from the underlying source and returns a resident
    /// `Heap`-backed sub-buffer whose `as_bytes()`/`as_ptr()` yield a stable
    /// contiguous pointer. For `Mapped`/`Heap` it stays zero-copy. A network
    /// error surfaces as `Err(CodecError::Remote(..))`.
    pub fn try_slice(&self, range: Range<usize>) -> Result<Self, CodecError> {
        if range.start > range.end {
            return Err(CodecError::Decode(
                "invalid slice range: start > end".to_string(),
            ));
        }
        if range.end > self.len() {
            return Err(CodecError::Decode(format!(
                "slice out of bounds: end {} > len {}",
                range.end,
                self.len()
            )));
        }
        match &self.store {
            BackingStore::Remote(backing) => self.fetch_remote_slice(backing, range),
            _ => Ok(self.slice_unchecked(range)),
        }
    }

    /// Fetch `range` (relative to this buffer) from a remote backing and wrap the
    /// resulting bytes in a `Heap`-backed [`OwnedBuffer`].
    ///
    /// `range` is offset by this buffer's own `range.start` so that slicing a
    /// sub-view of a remote buffer addresses absolute source offsets correctly.
    fn fetch_remote_slice(
        &self,
        backing: &Arc<RemoteBacking>,
        range: Range<usize>,
    ) -> Result<Self, CodecError> {
        if range.is_empty() {
            return Ok(Self::from_vec(Vec::new()));
        }
        let abs_start = (self.range.start + range.start) as u64;
        let len = range.end - range.start;
        // The fetcher is lock-free (&self, internally synchronized), so concurrent
        // slices across threads overlap rather than serialize.
        let bytes = backing.fetcher.read_range(abs_start, len).map_err(|e| {
            CodecError::Remote(format!(
                "failed to fetch range [{}, {}): {}",
                abs_start,
                abs_start + len as u64,
                e
            ))
        })?;
        Ok(Self::from_vec(bytes))
    }

    /// Obtain the whole logical range as a resident buffer.
    ///
    /// Zero-copy for `Mapped`/`Heap` (returns a clone sharing the same backing).
    /// For a `Remote` backing this is a single bounded fetch of the entire logical
    /// range, returning a `Heap`-backed buffer whose `as_bytes()` then works
    /// normally. A network error surfaces as `Err(CodecError::Remote(..))`.
    ///
    /// This is the seam for **monolithic / non-blocking** formats (PNG, standalone
    /// JPEG, DTED's full-grid decode) that legitimately need every byte: they call
    /// `materialize()?` and view the result, so a fetch failure propagates through
    /// `?` rather than becoming an infallible-`as_bytes()` panic. Block-capable
    /// formats should slice bounded ranges instead of materializing the whole file.
    pub(crate) fn materialize(&self) -> Result<Self, CodecError> {
        match &self.store {
            BackingStore::Remote(_) => self.try_slice(0..self.len()),
            _ => Ok(self.clone()),
        }
    }

    /// Narrow this buffer to `range` **without fetching**, for every backing.
    ///
    /// Unlike [`slice`](Self::slice)/[`try_slice`](Self::try_slice) — which for a
    /// `Remote` backing fetch the range and return a resident `Heap` sub-buffer —
    /// this only adjusts the logical range. A `Remote` buffer stays `Remote`
    /// (bytes are still fetched on demand by a later `try_slice`/callback), a
    /// `Mapped`/`Heap` buffer stays zero-copy. This is how a reader isolates a
    /// codestream/segment region of a remote source while keeping it remote.
    ///
    /// Panics on out-of-bounds, matching [`slice`](Self::slice)'s contract.
    pub(crate) fn subview(&self, range: Range<usize>) -> Self {
        assert!(range.start <= range.end, "invalid range: start > end");
        assert!(range.end <= self.len(), "subview out of bounds");
        self.slice_unchecked(range)
    }

    /// The resident bytes of this buffer, or `None` for a `Remote` backing.
    ///
    /// This is the non-panicking counterpart to [`as_bytes`](Self::as_bytes): it
    /// lets a parser take a zero-copy fast path when the bytes are already
    /// resident (`Mapped`/`Heap`) and fall back to range fetches
    /// ([`read_range`](Self::read_range)) when they are not (`Remote`).
    pub(crate) fn resident_bytes(&self) -> Option<&[u8]> {
        match self.store {
            BackingStore::Remote(_) => None,
            _ => Some(&self.backing_bytes()[self.range.clone()]),
        }
    }

    /// Read `[offset, offset + len)` (relative to this buffer) into owned bytes.
    ///
    /// Works for every backing: a copy out of the resident bytes for
    /// `Mapped`/`Heap`, a fetch-and-cache for `Remote`. This is the single seam
    /// the C-library I/O callbacks (libtiff, OpenJPEG) and the pure-Rust
    /// header/IFD parsers use so they never depend on the whole file being
    /// resident. A remote fetch failure surfaces as `Err(CodecError::Remote(..))`.
    pub(crate) fn read_range(&self, offset: usize, len: usize) -> Result<Vec<u8>, CodecError> {
        Ok(self.try_slice(offset..offset + len)?.as_bytes().to_vec())
    }

    /// Read multiple `[offset, offset + len)` ranges (each relative to this
    /// buffer) into owned byte vectors, one per input range in order.
    ///
    /// This is the batch counterpart to [`read_range`](Self::read_range) and the
    /// buffer-level seam for the concurrent fetch path. For a `Remote` backing it
    /// funnels all ranges through the fetcher's
    /// [`read_ranges`](StreamFetcher::read_ranges) as a **single batched call**
    /// (which the fsspec-backed reader fans out as one `cat_ranges`), so a
    /// decoder's scattered tile-parts fetch concurrently rather than one at a
    /// time. Each range is offset-adjusted by this buffer's own `range.start`
    /// (like [`fetch_remote_slice`](Self::fetch_remote_slice)) so a `subview`'d
    /// remote codestream addresses absolute source offsets correctly. For
    /// resident (`Mapped`/`Heap`) backings it returns N cheap copies of the
    /// already-resident slices.
    ///
    /// Keeping the batch behind `OwnedBuffer` preserves the "buffer is the fetch
    /// seam" abstraction: readers never touch `RemoteBacking` directly.
    pub(crate) fn read_ranges(
        &self,
        ranges: &[(usize, usize)],
    ) -> Result<Vec<Vec<u8>>, CodecError> {
        match &self.store {
            BackingStore::Remote(backing) => {
                // Validate + offset-adjust every range, then issue one batched
                // fetch. An empty individual range contributes an empty result
                // without touching the network.
                let mut abs_ranges: Vec<(u64, usize)> = Vec::with_capacity(ranges.len());
                for &(offset, len) in ranges {
                    let end = offset.checked_add(len).ok_or_else(|| {
                        CodecError::Decode("range offset+len overflows usize".to_string())
                    })?;
                    if end > self.len() {
                        return Err(CodecError::Decode(format!(
                            "read_ranges out of bounds: end {} > len {}",
                            end,
                            self.len()
                        )));
                    }
                    abs_ranges.push(((self.range.start + offset) as u64, len));
                }
                // Lock-free fetcher: one batched read_ranges → one cat_ranges,
                // concurrent with any other thread's fetch on this backing.
                backing.fetcher.read_ranges(&abs_ranges).map_err(|e| {
                    CodecError::Remote(format!("failed to fetch ranges: {}", e))
                })
            }
            // Resident backing: N cheap copies of the mapped/heap slices.
            _ => ranges
                .iter()
                .map(|&(offset, len)| self.read_range(offset, len))
                .collect(),
        }
    }

    fn slice_unchecked(&self, range: Range<usize>) -> Self {
        if range.is_empty() {
            return Self {
                store: self.store.clone(),
                range: 0..0,
            };
        }
        Self {
            store: self.store.clone(),
            range: (self.range.start + range.start)..(self.range.start + range.end),
        }
    }

    fn backing_bytes(&self) -> &[u8] {
        match &self.store {
            BackingStore::Mapped(m) => m.as_ref(),
            BackingStore::Heap(h) => h.as_slice(),
            // Unreachable: `as_bytes` guards against Remote before calling this,
            // and no other caller reaches a Remote backing.
            BackingStore::Remote(_) => unreachable!(
                "backing_bytes() on a Remote OwnedBuffer; use try_slice to fetch first"
            ),
        }
    }
}

impl OwnedBuffer {
    /// True if this buffer is backed by a remote (fetch-on-slice) source.
    fn is_remote(&self) -> bool {
        matches!(self.store, BackingStore::Remote(_))
    }
}

impl PartialEq for OwnedBuffer {
    /// Compares resident bytes. Two `Remote` buffers are equal iff they are the
    /// same underlying backing (comparing contents would force a full download,
    /// which the `as_bytes()` guard forbids); a `Remote` buffer never equals a
    /// resident one.
    fn eq(&self, other: &Self) -> bool {
        match (&self.store, &other.store) {
            (BackingStore::Remote(a), BackingStore::Remote(b)) => {
                Arc::ptr_eq(a, b) && self.range == other.range
            }
            (BackingStore::Remote(_), _) | (_, BackingStore::Remote(_)) => false,
            _ => self.as_bytes() == other.as_bytes(),
        }
    }
}

impl Eq for OwnedBuffer {}

impl fmt::Debug for OwnedBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_remote() {
            return f
                .debug_struct("OwnedBuffer")
                .field("backing", &"Remote")
                .field("len", &self.len())
                .finish();
        }
        let bytes = self.as_bytes();
        let preview_len = bytes.len().min(32);
        let hex: String = bytes[..preview_len]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        f.debug_struct("OwnedBuffer")
            .field("len", &self.len())
            .field("bytes", &hex)
            .finish()
    }
}

const _: () = {
    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}
    #[allow(dead_code)]
    fn check() {
        assert_send_sync::<OwnedBuffer>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    use std::io::Write;

    fn make_heap_buffer(data: &[u8]) -> OwnedBuffer {
        OwnedBuffer::from_vec(data.to_vec())
    }

    fn make_mmap_buffer(data: &[u8]) -> OwnedBuffer {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(data).unwrap();
        file.flush().unwrap();
        let mmap = unsafe { Mmap::map(file.as_file()).unwrap() };
        OwnedBuffer::from_mmap(mmap)
    }

    #[test]
    fn test_from_vec_as_bytes() {
        let data = b"hello world";
        let buf = make_heap_buffer(data);
        assert_eq!(buf.as_bytes(), data);
    }

    #[test]
    fn test_from_mmap_as_bytes() {
        let data = b"memory mapped content";
        let buf = make_mmap_buffer(data);
        assert_eq!(buf.as_bytes(), data);
    }

    #[test]
    fn test_len_and_is_empty() {
        let buf = make_heap_buffer(b"abc");
        assert_eq!(buf.len(), 3);
        assert!(!buf.is_empty());

        let empty = make_heap_buffer(b"");
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_slice_returns_correct_sub_range() {
        let data: Vec<u8> = (0..100).collect();
        let buf = make_heap_buffer(&data);
        let sub = buf.slice(10..20);
        assert_eq!(sub.as_bytes(), &data[10..20]);
        assert_eq!(sub.len(), 10);
    }

    #[test]
    fn test_nested_slicing() {
        let data: Vec<u8> = (0..100).collect();
        let buf = make_heap_buffer(&data);
        let outer = buf.slice(10..50);
        let inner = outer.slice(5..15);
        assert_eq!(inner.as_bytes(), &data[15..25]);
    }

    #[test]
    fn test_try_slice_success() {
        let data: Vec<u8> = (0..50).collect();
        let buf = make_heap_buffer(&data);
        let sub = buf.try_slice(5..10).unwrap();
        assert_eq!(sub.as_bytes(), &data[5..10]);
    }

    #[test]
    fn test_try_slice_out_of_bounds() {
        let buf = make_heap_buffer(b"short");
        let result = buf.try_slice(0..100);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("out of bounds"));
    }

    #[test]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_try_slice_start_greater_than_end() {
        let buf = make_heap_buffer(b"data");
        let result = buf.try_slice(3..1);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("start > end"));
    }

    #[test]
    #[should_panic(expected = "slice out of bounds")]
    fn test_slice_panics_on_out_of_bounds() {
        let buf = make_heap_buffer(b"short");
        buf.slice(0..100);
    }

    #[test]
    #[should_panic(expected = "invalid range: start > end")]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_slice_panics_on_start_greater_than_end() {
        let buf = make_heap_buffer(b"data");
        buf.slice(3..1);
    }

    #[test]
    fn test_empty_slice_handling() {
        let buf = make_heap_buffer(b"non-empty");
        let empty = buf.slice(5..5);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        let expected: &[u8] = &[];
        assert_eq!(empty.as_bytes(), expected);
    }

    #[test]
    fn test_clone_is_zero_copy() {
        let data = vec![1u8, 2, 3, 4, 5];
        let buf = OwnedBuffer::from_vec(data);
        let ptr_before = buf.as_bytes().as_ptr();
        let cloned = buf.clone();
        let ptr_after = cloned.as_bytes().as_ptr();
        assert_eq!(ptr_before, ptr_after);

        // Verify refcount increased
        match &buf.store {
            BackingStore::Heap(arc) => assert_eq!(Arc::strong_count(arc), 2),
            _ => panic!("expected Heap variant"),
        }
    }

    #[test]
    fn test_partial_eq_same_content_different_backing() {
        let data = b"identical content";
        let heap_buf = make_heap_buffer(data);
        let mmap_buf = make_mmap_buffer(data);
        assert_eq!(heap_buf, mmap_buf);
    }

    #[test]
    fn test_partial_eq_different_content() {
        let buf1 = make_heap_buffer(b"one");
        let buf2 = make_heap_buffer(b"two");
        assert_ne!(buf1, buf2);
    }

    #[test]
    fn test_partial_eq_slice_equals_original_sub_range() {
        let data: Vec<u8> = (0..100).collect();
        let full = make_heap_buffer(&data);
        let sliced = full.slice(10..20);
        let direct = make_heap_buffer(&data[10..20]);
        assert_eq!(sliced, direct);
    }

    #[test]
    fn test_debug_output() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let buf = OwnedBuffer::from_vec(data);
        let debug = format!("{:?}", buf);
        assert!(debug.contains("OwnedBuffer"));
        assert!(debug.contains("len: 4"));
        assert!(debug.contains("deadbeef"));
    }

    #[test]
    fn test_debug_truncates_long_content() {
        let data: Vec<u8> = (0..100).collect();
        let buf = make_heap_buffer(&data);
        let debug = format!("{:?}", buf);
        assert!(debug.contains("len: 100"));
        // First 32 bytes as hex = 64 hex chars
        let expected_hex: String = data[..32].iter().map(|b| format!("{:02x}", b)).collect();
        assert!(debug.contains(&expected_hex));
    }

    #[test]
    fn test_from_vec_empty() {
        let buf = OwnedBuffer::from_vec(vec![]);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        let empty: &[u8] = &[];
        assert_eq!(buf.as_bytes(), empty);
    }

    #[test]
    fn test_slice_full_range() {
        let data = b"full range test";
        let buf = make_heap_buffer(data);
        let sliced = buf.slice(0..buf.len());
        assert_eq!(sliced.as_bytes(), data.as_slice());
    }

    // =========================================================================
    // Remote backing tests
    // =========================================================================

    use crate::remote::{FakeReader, HeaderAwarePolicy, StreamFetcher};
    use std::sync::Mutex as StdMutex;

    /// Shared `(offset, len)` fetch log produced by the fake reader.
    type FetchLog = Arc<StdMutex<Vec<(u64, usize)>>>;

    /// Build a `Remote` `OwnedBuffer` over `data` with no eager header prefetch,
    /// returning the buffer plus a handle to the reader's fetch log.
    fn make_remote_buffer(data: &[u8]) -> (OwnedBuffer, FetchLog) {
        let reader = FakeReader::new(data.to_vec());
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        (OwnedBuffer::from_remote(fetcher), log)
    }

    #[test]
    fn test_remote_len_is_total_size() {
        let data: Vec<u8> = (0..250).map(|i| (i % 256) as u8).collect();
        let (buf, log) = make_remote_buffer(&data);
        assert_eq!(buf.len(), 250);
        assert!(!buf.is_empty());
        // Reporting len must not fetch anything.
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn test_remote_try_slice_fetches_and_returns_correct_bytes() {
        let data: Vec<u8> = (0..=255).collect();
        let (buf, log) = make_remote_buffer(&data);
        let chunk = buf.try_slice(100..110).unwrap();
        assert_eq!(chunk.as_bytes(), &data[100..110]);
        // Exactly the requested range was fetched, not the whole file.
        assert_eq!(&*log.lock().unwrap(), &[(100, 10)]);
    }

    #[test]
    fn test_remote_sub_buffer_is_heap_backed_with_stable_pointer() {
        let data: Vec<u8> = (0..=255).collect();
        let (buf, _log) = make_remote_buffer(&data);
        let chunk = buf.try_slice(10..40).unwrap();
        // A Heap-backed sub-buffer: as_bytes() works and the pointer is stable
        // across clones (zero-copy clone of the fetched Vec).
        let p1 = chunk.as_bytes().as_ptr();
        let p2 = chunk.clone();
        assert_eq!(p1, p2.as_bytes().as_ptr());
        assert!(!chunk.is_remote());
    }

    #[test]
    fn test_remote_repeated_slice_hits_cache() {
        let data: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
        let (buf, log) = make_remote_buffer(&data);
        let a = buf.try_slice(20..60).unwrap();
        let b = buf.try_slice(30..50).unwrap(); // sub-range, served from cache
        assert_eq!(a.as_bytes()[10..30], *b.as_bytes());
        assert_eq!(log.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_remote_never_fetches_whole_file() {
        let data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
        let (buf, log) = make_remote_buffer(&data);
        for off in (0..4096).step_by(512) {
            buf.try_slice(off..off + 16).unwrap();
        }
        let fetched: usize = log.lock().unwrap().iter().map(|(_, l)| *l).sum();
        assert!(fetched < 4096, "fetched {} of 4096 bytes", fetched);
    }

    #[test]
    fn test_remote_out_of_bounds_slice_errors() {
        let (buf, _log) = make_remote_buffer(&[0u8; 100]);
        let err = buf.try_slice(90..120).unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn test_remote_fetch_error_is_remote_variant() {
        // A reader whose reads always fail, wrapped so the fetch surfaces
        // CodecError::Remote from try_slice.
        struct FailReader;
        impl crate::remote::RangeReader for FailReader {
            fn read_at(&self, _o: u64, _l: usize) -> Result<Vec<u8>, CodecError> {
                Err(CodecError::Io(std::io::Error::other("boom")))
            }
            fn total_size(&self) -> u64 {
                1000
            }
        }
        let fetcher =
            StreamFetcher::with_policy(Box::new(FailReader), Box::new(HeaderAwarePolicy::new(0)));
        let buf = OwnedBuffer::from_remote(fetcher);
        let err = buf.try_slice(0..10).unwrap_err();
        assert!(matches!(err, CodecError::Remote(_)));
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    #[should_panic(expected = "as_bytes() called on a Remote OwnedBuffer")]
    fn test_remote_bare_as_bytes_guard_fires() {
        let (buf, _log) = make_remote_buffer(&[1u8, 2, 3, 4]);
        // Missed slice-first conversion: a bare full-buffer view must fail loudly.
        let _ = buf.as_bytes();
    }

    #[test]
    fn test_remote_empty_slice_is_resident_and_empty() {
        let (buf, log) = make_remote_buffer(&[0u8; 100]);
        let empty = buf.try_slice(50..50).unwrap();
        assert!(empty.is_empty());
        let e: &[u8] = &[];
        assert_eq!(empty.as_bytes(), e);
        // No fetch for an empty slice.
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn test_remote_debug_does_not_fetch() {
        let (buf, log) = make_remote_buffer(&[0u8; 100]);
        let debug = format!("{:?}", buf);
        assert!(debug.contains("Remote"));
        assert!(debug.contains("len: 100"));
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn test_mapped_and_heap_as_bytes_not_affected_by_guard() {
        // Guard must have no false positives on the resident backings.
        let heap = make_heap_buffer(b"heap bytes");
        assert_eq!(heap.as_bytes(), b"heap bytes");
        let mmap = make_mmap_buffer(b"mmap bytes");
        assert_eq!(mmap.as_bytes(), b"mmap bytes");
    }

    #[test]
    fn test_remote_slice_via_panicking_slice_method() {
        let data: Vec<u8> = (0..=255).collect();
        let (buf, _log) = make_remote_buffer(&data);
        // The panicking slice() also fetches for Remote.
        let chunk = buf.slice(5..15);
        assert_eq!(chunk.as_bytes(), &data[5..15]);
    }

    #[test]
    fn test_read_ranges_heap_byte_identical_to_read_range() {
        let data: Vec<u8> = (0..256).map(|i| (i % 256) as u8).collect();
        let buf = make_heap_buffer(&data);
        let ranges = [(10usize, 20usize), (100, 50), (200, 1), (0, 5)];
        let batched = buf.read_ranges(&ranges).unwrap();
        for (i, &(o, l)) in ranges.iter().enumerate() {
            assert_eq!(batched[i], buf.read_range(o, l).unwrap());
            assert_eq!(batched[i], &data[o..o + l]);
        }
    }

    #[test]
    fn test_read_ranges_mapped_byte_identical_to_read_range() {
        let data: Vec<u8> = (0..256).map(|i| (i % 256) as u8).collect();
        let buf = make_mmap_buffer(&data);
        let ranges = [(0usize, 32usize), (128, 64), (255, 1)];
        let batched = buf.read_ranges(&ranges).unwrap();
        for (i, &(o, l)) in ranges.iter().enumerate() {
            assert_eq!(batched[i], &data[o..o + l]);
        }
    }

    #[test]
    fn test_read_ranges_remote_byte_identical_to_read_range() {
        let data: Vec<u8> = (0..1024).map(|i| (i * 3 % 256) as u8).collect();
        let (buf, _log) = make_remote_buffer(&data);
        let ranges = [(13usize, 40usize), (500, 100), (900, 20), (0, 8)];
        let batched = buf.read_ranges(&ranges).unwrap();
        for (i, &(o, l)) in ranges.iter().enumerate() {
            assert_eq!(batched[i], &data[o..o + l]);
        }
    }

    #[test]
    fn test_read_ranges_remote_issues_single_batched_read_many() {
        // The Option-1 win at the buffer seam: scattered ranges over a Remote
        // buffer go out as ONE read_many (cat_ranges), not N serial reads.
        let data: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        let reader = FakeReader::new(data);
        let many_log = reader.many_log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let buf = OwnedBuffer::from_remote(fetcher);
        let ranges = [(13usize, 40usize), (500, 100), (900, 20), (1200, 64), (1600, 8), (1900, 50)];
        buf.read_ranges(&ranges).unwrap();
        let many = many_log.lock().unwrap();
        assert_eq!(many.len(), 1, "expected exactly one batched read_many call");
        assert_eq!(many[0].len(), 6, "all six ranges in the one batch");
    }

    #[test]
    fn test_read_ranges_remote_subview_addresses_absolute_offsets() {
        // A subview'd remote buffer must offset each range by range.start so it
        // addresses absolute source offsets, matching fetch_remote_slice.
        let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        let (buf, _log) = make_remote_buffer(&data);
        // Narrow to [200, 700) without fetching.
        let sub = buf.subview(200..700);
        let ranges = [(0usize, 10usize), (50, 20), (300, 100)];
        let batched = sub.read_ranges(&ranges).unwrap();
        for (i, &(o, l)) in ranges.iter().enumerate() {
            // Sub-buffer offset o maps to absolute source offset 200 + o.
            assert_eq!(batched[i], &data[200 + o..200 + o + l]);
            // And matches the single-range read_range on the same subview.
            assert_eq!(batched[i], sub.read_range(o, l).unwrap());
        }
    }

    #[test]
    fn test_read_ranges_remote_out_of_bounds_errors() {
        let (buf, _log) = make_remote_buffer(&[0u8; 100]);
        let err = buf.read_ranges(&[(50, 10), (90, 20)]).unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn test_read_ranges_empty_input_returns_empty() {
        let (buf, log) = make_remote_buffer(&[0u8; 100]);
        let result = buf.read_ranges(&[]).unwrap();
        assert!(result.is_empty());
        assert!(log.lock().unwrap().is_empty());
    }

    // =========================================================================
    // Concurrency test — the concurrent-fetch payoff
    // =========================================================================

    #[test]
    fn test_concurrent_reads_are_byte_identical_and_bounded() {
        // N threads hammer ONE Remote OwnedBuffer concurrently through both the
        // single-range (read_range) and batched (read_ranges) seams. Because the
        // fetcher is lock-free (&self reader) with only a short-held cache lock,
        // this must not panic, must return byte-identical results, must keep the
        // cache coherent, and must never fetch the whole file (guarded via the
        // FakeReader log — no atomic counter, per Design Decision 5).
        use std::thread;

        const FILE_LEN: usize = 16_384;
        const N_THREADS: usize = 8;
        const ITERS: usize = 40;

        let data: Vec<u8> = (0..FILE_LEN).map(|i| (i * 31 % 256) as u8).collect();

        // Six scattered ranges à la a J2K tile's tile-parts. Their union is a
        // small fraction of the file, so even if every thread races and fetches
        // all six (the benign duplicate-fetch race), total fetched bytes stay far
        // below FILE_LEN — the whole-file-never-fetched guarantee.
        let ranges: [(usize, usize); 6] =
            [(37, 40), (2_048, 100), (5_000, 64), (8_192, 128), (12_000, 32), (15_900, 50)];
        let unique_bytes: usize = ranges.iter().map(|(_, l)| *l).sum();
        assert!(unique_bytes * N_THREADS < FILE_LEN, "test ranges too large to prove the bound");

        let reader = FakeReader::new(data.clone());
        let log = reader.log_handle();
        let many_log = reader.many_log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        // Clone is O(1) and shares the same Arc<RemoteBacking>, so every thread
        // fetches through the one backing concurrently.
        let buf = OwnedBuffer::from_remote(fetcher);

        for _ in 0..ITERS {
            thread::scope(|scope| {
                for t in 0..N_THREADS {
                    let buf = buf.clone();
                    let data = &data;
                    scope.spawn(move || {
                        if t % 2 == 0 {
                            // Batched path.
                            let out = buf.read_ranges(&ranges).unwrap();
                            for (i, &(o, l)) in ranges.iter().enumerate() {
                                assert_eq!(out[i], &data[o..o + l]);
                            }
                        } else {
                            // Single-range path.
                            for &(o, l) in &ranges {
                                let chunk = buf.try_slice(o..o + l).unwrap();
                                assert_eq!(chunk.as_bytes(), &data[o..o + l]);
                            }
                        }
                    });
                }
            });
        }

        // Whole file was never fetched: no single fetch covered the whole file,
        // and the total fetched bytes stayed bounded (the union is fetched at most
        // a small number of times despite the duplicate-fetch race).
        let log = log.lock().unwrap();
        assert!(!log.is_empty(), "expected at least one fetch");
        for &(_, len) in log.iter() {
            assert!(len < FILE_LEN, "a single fetch of {len} bytes covered the whole file");
        }
        let total_fetched: usize = log.iter().map(|(_, l)| *l).sum();
        assert!(
            total_fetched < FILE_LEN,
            "fetched {total_fetched} bytes of a {FILE_LEN}-byte file — whole-file fetch regression"
        );
        // The concurrent fetches flowed through the batched read_many path (the
        // containment invariant), not a stateful cursor.
        assert!(
            !many_log.lock().unwrap().is_empty(),
            "concurrent reads must route through read_many/cat_ranges"
        );
    }

    #[test]
    fn test_remote_decode_byte_identical_to_heap() {
        // Remoteness is transparent to output: slicing a Remote buffer yields the
        // same bytes as slicing a Heap buffer over the same data.
        let data: Vec<u8> = (0..1024).map(|i| (i * 7 % 256) as u8).collect();
        let heap = OwnedBuffer::from_vec(data.clone());
        let (remote, _log) = make_remote_buffer(&data);
        for (start, end) in [(0usize, 100usize), (200, 400), (500, 501), (900, 1024)] {
            let h = heap.slice(start..end);
            let r = remote.try_slice(start..end).unwrap();
            assert_eq!(h.as_bytes(), r.as_bytes());
        }
    }

    // Property tests using proptest
    proptest! {
        #[test]
        fn prop_slice_content_matches(
            data in proptest::collection::vec(any::<u8>(), 0..256),
            start in 0usize..256,
            end in 0usize..256,
        ) {
            if start <= end && end <= data.len() {
                let buf = OwnedBuffer::from_vec(data.clone());
                let sliced = buf.slice(start..end);
                prop_assert_eq!(sliced.as_bytes(), &data[start..end]);
            }
        }

        #[test]
        fn prop_nested_slice_correct(
            data in proptest::collection::vec(any::<u8>(), 1..256),
            a in 0usize..256,
            b in 0usize..256,
            c in 0usize..256,
            d in 0usize..256,
        ) {
            if a <= b && b <= data.len() {
                let outer_len = b - a;
                if c <= d && d <= outer_len {
                    let buf = OwnedBuffer::from_vec(data.clone());
                    let outer = buf.slice(a..b);
                    let inner = outer.slice(c..d);
                    prop_assert_eq!(inner.as_bytes(), &data[a + c..a + d]);
                }
            }
        }

        #[test]
        fn prop_try_slice_agrees_with_slice(
            data in proptest::collection::vec(any::<u8>(), 0..128),
            start in 0usize..128,
            end in 0usize..128,
        ) {
            let buf = OwnedBuffer::from_vec(data.clone());
            if start <= end && end <= data.len() {
                let result = buf.try_slice(start..end).unwrap();
                prop_assert_eq!(result.as_bytes(), &data[start..end]);
            } else if start > end {
                let result = buf.try_slice(start..end);
                prop_assert!(result.is_err());
            } else {
                let result = buf.try_slice(start..end);
                prop_assert!(result.is_err());
            }
        }
    }
}
