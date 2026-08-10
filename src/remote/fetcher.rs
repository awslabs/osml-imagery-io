//! The [`StreamFetcher`] — the single `(offset, len) -> bytes` fetch entry point.
//!
//! A fetcher owns three things: an abstract [`RangeReader`] (the actual byte
//! source), a [`RangeCache`] of already-fetched ranges, and a
//! [`PrefetchPolicy`](crate::remote::PrefetchPolicy) that decides which ranges to
//! pull. [`StreamFetcher::read_range`] serves from cache when possible; on a miss
//! it asks the policy for a plan, coalesces the plan into a minimal set of
//! non-overlapping fetches, pulls the missing pieces through the reader, caches
//! them, and returns the requested bytes.
//!
//! Putting the byte source behind [`RangeReader`] is what lets the unit tests run
//! with a pure in-memory fake — no `Py<PyAny>`, no Python interpreter, no threads.
//! The Python-backed reader (which turns `seek`+`read` on a file-like object into
//! range reads) implements the same trait.

use std::ops::Range;
use std::sync::Mutex;

use crate::error::CodecError;
use crate::remote::cache::RangeCache;
use crate::remote::policy::{HeaderAwarePolicy, PrefetchPolicy};

/// The byte-source seam: fetch `[offset, offset + len)` from the underlying
/// source, returning exactly `len` bytes on success.
///
/// Implementors must return `Err(CodecError)` (rather than short reads) if the
/// range cannot be fully satisfied. Both the Python-backed stream reader and the
/// in-memory test fake implement this trait.
///
/// # Concurrency
///
/// Both methods take `&self` so N threads can fetch through one reader without a
/// lock — this is what lets concurrent `get_block` callers overlap across blocks
/// (see the lock refactor in [`StreamFetcher`]). The trait is therefore
/// `Send + Sync`.
///
/// The **containment invariant** (see the remote range-read design, "the lock
/// refactor"): concurrent fetches must flow through the *stateless*
/// [`read_many`](RangeReader::read_many) path (fsspec `cat_ranges`, no cursor),
/// never through a stateful `seek`+`read` cursor. An implementor whose
/// [`read_at`](RangeReader::read_at) mutates shared cursor state (the fsspec
/// handle path) must guard against concurrent entry (the Python-backed reader
/// does so with a debug re-entry assertion); the in-memory fakes are naturally
/// stateless.
pub trait RangeReader: Send + Sync {
    /// Read exactly `len` bytes starting at absolute `offset`.
    fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, CodecError>;

    /// Fetch multiple ranges, concurrently when the backing supports it.
    ///
    /// Returns one `Vec<u8>` per requested range, in input order, each exactly
    /// its requested `len`. The default implementation loops [`read_at`], which
    /// is the correct serial fallback for backings that cannot fetch
    /// concurrently (e.g. an `io.BytesIO` or plain-file handle). Concurrent
    /// backings (the fsspec-backed reader) override this to fan the ranges out
    /// in one batched, cursor-free request.
    ///
    /// The return contract matches [`read_at`]: a range that cannot be fully
    /// satisfied is an `Err(CodecError)`, never a short read.
    ///
    /// [`read_at`]: RangeReader::read_at
    fn read_many(&self, ranges: &[(u64, usize)]) -> Result<Vec<Vec<u8>>, CodecError> {
        ranges.iter().map(|&(o, l)| self.read_at(o, l)).collect()
    }

    /// Total size of the source in bytes.
    fn total_size(&self) -> u64;
}

/// The cache + policy behind [`StreamFetcher`]'s short-held lock.
///
/// Everything mutated during a fetch lives here so the mutex is held only for
/// cache lookups, planning, and inserts — never across the network wait. The
/// [`RangeReader`] itself is **not** in here: it is lock-free (`&self`) so N
/// threads can fetch through it concurrently.
struct CacheState {
    cache: RangeCache,
    policy: Box<dyn PrefetchPolicy>,
}

impl CacheState {
    /// Plan (and coalesce) the not-yet-cached spans needed to satisfy
    /// `requested`, returning them as `(offset, len)` fetch targets.
    ///
    /// The policy may add speculative ranges (e.g. an eager header); coalescing
    /// merges overlaps/adjacencies, and already-cached spans are filtered out so
    /// each missing byte is fetched at most once.
    fn plan_uncached(&self, requested: Range<u64>, total_size: u64) -> Vec<(u64, usize)> {
        let planned = self.policy.plan(requested, total_size);
        coalesce(planned)
            .into_iter()
            .filter(|span| {
                !self
                    .cache
                    .contains(span.start, (span.end - span.start) as usize)
            })
            .map(|span| (span.start, (span.end - span.start) as usize))
            .collect()
    }
}

/// Fetches and caches byte ranges from a [`RangeReader`] under a
/// [`PrefetchPolicy`].
///
/// This is the type the `Remote` `OwnedBuffer` backing holds. It is intentionally
/// unaware of imagery formats; the header hint that tunes the policy is supplied
/// from the IO dispatch layer.
///
/// # Concurrency (the lock refactor)
///
/// The `reader` is lock-free — its `&self` [`RangeReader`] methods let N threads
/// fetch concurrently, so overlapping `get_block` callers across blocks issue
/// their `cat_ranges` in parallel. Only the mutable cache/policy sits behind a
/// [`Mutex`], and that lock is **released across the network fetch**: each read
/// consults the cache and plans under the lock, drops it, fetches via
/// [`RangeReader::read_many`], then re-locks to insert and serve. Two callers
/// that miss the same span may both fetch it (a benign duplicate-fetch race):
/// [`RangeCache::insert`] merges/dedups identical overlapping bytes, so the cache
/// stays coherent.
///
/// All fetches — including the self-heal fallback when a buggy policy
/// under-plans — go through the cursor-free `read_many` path, upholding the
/// containment invariant (concurrent reads never touch a stateful `seek`+`read`
/// cursor).
pub struct StreamFetcher {
    reader: Box<dyn RangeReader>,
    state: Mutex<CacheState>,
    total_size: u64,
}

impl StreamFetcher {
    /// Create a fetcher over `reader` using the default header-aware policy.
    pub fn new(reader: Box<dyn RangeReader>) -> Self {
        Self::with_policy(reader, Box::new(HeaderAwarePolicy::default()))
    }

    /// Create a fetcher over `reader` with an explicit prefetch policy.
    pub fn with_policy(reader: Box<dyn RangeReader>, policy: Box<dyn PrefetchPolicy>) -> Self {
        let total_size = reader.total_size();
        Self {
            reader,
            state: Mutex::new(CacheState {
                cache: RangeCache::new(),
                policy,
            }),
            total_size,
        }
    }

    /// Total size of the underlying source in bytes.
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Number of bytes currently resident in the cache.
    pub fn resident_bytes(&self) -> usize {
        self.state
            .lock()
            .map(|s| s.cache.resident_bytes())
            .unwrap_or(0)
    }

    /// Validate `[offset, offset + len)` against `total_size`, returning `end`.
    fn check_bounds(&self, offset: u64, len: usize) -> Result<u64, CodecError> {
        let end = offset
            .checked_add(len as u64)
            .ok_or_else(|| CodecError::Decode("range offset+len overflows u64".to_string()))?;
        if end > self.total_size {
            return Err(CodecError::Decode(format!(
                "range out of bounds: end {} > total_size {}",
                end, self.total_size
            )));
        }
        Ok(end)
    }

    /// Lock the cache state, mapping a poisoned mutex to a `CodecError`.
    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, CacheState>, CodecError> {
        self.state
            .lock()
            .map_err(|_| CodecError::Remote("remote cache mutex poisoned".to_string()))
    }

    /// Insert the results of a batched fetch into the cache under a short-held
    /// lock. `to_fetch` and `fetched` are zipped in order.
    fn fill_cache(
        &self,
        to_fetch: &[(u64, usize)],
        fetched: Vec<Vec<u8>>,
    ) -> Result<(), CodecError> {
        let mut s = self.lock_state()?;
        for (&(start, _), data) in to_fetch.iter().zip(fetched.into_iter()) {
            s.cache.insert(start, data);
        }
        Ok(())
    }

    /// Serve `[offset, offset + len)` from the cache, self-healing with a single
    /// cursor-free `read_many` fetch (lock released) if a buggy policy left the
    /// range uncovered.
    fn serve_or_heal(&self, offset: u64, len: usize) -> Result<Vec<u8>, CodecError> {
        // Read under the lock, release it before any network call.
        if let Some(bytes) = self.lock_state()?.cache.get(offset, len) {
            return Ok(bytes);
        }
        // Self-heal: fetch exactly the requested range through the stateless
        // read_many path (never the cursor), then insert and serve. `len == 0`
        // never reaches here (cache.get treats it as covered).
        let data = self
            .reader
            .read_many(&[(offset, len)])?
            .pop()
            .ok_or_else(|| CodecError::Remote("self-heal fetch returned no data".to_string()))?;
        self.lock_state()?.cache.insert(offset, data.clone());
        Ok(data)
    }

    /// Read `[offset, offset + len)`, fetching (and caching, with prefetch) any
    /// bytes not already resident.
    ///
    /// The requested range is validated against `total_size`; an out-of-bounds
    /// request is a `CodecError::Decode`. On a cache miss the policy plans the
    /// ranges to fetch, they are coalesced into non-overlapping spans, and the
    /// spans not already cached are pulled through the reader — **with the cache
    /// lock released** — and inserted, and the requested bytes are then served
    /// from cache. See the type-level concurrency note.
    pub fn read_range(&self, offset: u64, len: usize) -> Result<Vec<u8>, CodecError> {
        let end = self.check_bounds(offset, len)?;

        // Step 1: consult the cache + plan under a short-held lock, then UNLOCK.
        let to_fetch = {
            let s = self.lock_state()?;
            if let Some(bytes) = s.cache.get(offset, len) {
                return Ok(bytes); // fast path: already resident
            }
            s.plan_uncached(offset..end, self.total_size)
        };

        // Step 2: FETCH with the lock released (lock-free, &self reader), then
        // re-lock only to insert.
        if !to_fetch.is_empty() {
            let fetched = self.reader.read_many(&to_fetch)?;
            self.fill_cache(&to_fetch, fetched)?;
        }

        // Step 3: serve from cache (self-healing if under-planned).
        self.serve_or_heal(offset, len)
    }

    /// Read multiple ranges, issuing **one** batched [`RangeReader::read_many`]
    /// call for the bytes not already resident.
    ///
    /// This is the batch counterpart to [`read_range`](Self::read_range): it
    /// validates every requested range, plans + coalesces the union of their
    /// uncached spans, fetches all of them through the reader in a single
    /// `read_many` (which the concurrent fsspec-backed reader fans out as one
    /// `cat_ranges`) **with the cache lock released**, inserts them, and then
    /// serves each requested range from the cache in input order. A range already
    /// covered by the cache issues no fetch.
    ///
    /// Returns one `Vec<u8>` per input range, in order, each exactly its requested
    /// length. An out-of-bounds range is a `CodecError::Decode`; a reader failure
    /// surfaces unchanged.
    pub fn read_ranges(&self, ranges: &[(u64, usize)]) -> Result<Vec<Vec<u8>>, CodecError> {
        if ranges.is_empty() {
            return Ok(Vec::new());
        }

        // Validate every range up front (matches read_range's bounds check).
        for &(offset, len) in ranges {
            self.check_bounds(offset, len)?;
        }

        // Step 1: plan the union of uncached spans across all requested ranges
        // under a short-held lock, coalescing so each byte is fetched at most
        // once, then UNLOCK.
        let to_fetch: Vec<(u64, usize)> = {
            let s = self.lock_state()?;
            let mut planned: Vec<Range<u64>> = Vec::new();
            for &(offset, len) in ranges {
                if len == 0 || s.cache.contains(offset, len) {
                    continue;
                }
                planned.extend(s.policy.plan(offset..offset + len as u64, self.total_size));
            }
            coalesce(planned)
                .into_iter()
                .filter(|span| {
                    !s.cache
                        .contains(span.start, (span.end - span.start) as usize)
                })
                .map(|span| (span.start, (span.end - span.start) as usize))
                .collect()
        };

        // Step 2: fetch every not-already-cached span in a single batched
        // read_many, with the lock released, then re-lock only to insert.
        if !to_fetch.is_empty() {
            let fetched = self.reader.read_many(&to_fetch)?;
            self.fill_cache(&to_fetch, fetched)?;
        }

        // Step 3: serve each requested range from cache, in input order,
        // self-healing any range a buggy policy under-planned.
        ranges
            .iter()
            .map(|&(offset, len)| self.serve_or_heal(offset, len))
            .collect()
    }
}

/// Merge overlapping or adjacent ranges into a minimal set of non-overlapping,
/// sorted spans so each byte is fetched at most once.
fn coalesce(mut ranges: Vec<Range<u64>>) -> Vec<Range<u64>> {
    ranges.retain(|r| r.end > r.start);
    ranges.sort_by_key(|r| r.start);

    let mut merged: Vec<Range<u64>> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => {
                last.end = last.end.max(r.end);
            }
            _ => merged.push(r),
        }
    }
    merged
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Shared log of per-range `(offset, len)` reads, in call order.
    type RangeLog = Arc<Mutex<Vec<(u64, usize)>>>;
    /// Shared log of batched `read_many` invocations (one entry per call).
    type ManyLog = Arc<Mutex<Vec<Vec<(u64, usize)>>>>;

    /// An in-memory [`RangeReader`] backed by a `Vec<u8>` with a call log, so
    /// tests can assert exactly which ranges were fetched and that the whole
    /// buffer is never pulled in one shot.
    pub struct FakeReader {
        data: Vec<u8>,
        /// Log of `(offset, len)` reads, in call order. Shared so a test can hold
        /// a handle after moving the reader into a fetcher. A batched `read_many`
        /// records each of its ranges here too (so byte-accounting still works).
        pub log: RangeLog,
        /// Log of batched `read_many` invocations, one entry per call holding that
        /// call's full range list. Lets a test assert a batch went out as **one**
        /// `read_many` (i.e. one `cat_ranges`), not N serial `read_at`s.
        pub many_log: ManyLog,
    }

    impl FakeReader {
        pub fn new(data: Vec<u8>) -> Self {
            Self {
                data,
                log: Arc::new(Mutex::new(Vec::new())),
                many_log: Arc::new(Mutex::new(Vec::new())),
            }
        }

        /// A shared handle to this reader's per-range call log.
        pub fn log_handle(&self) -> RangeLog {
            Arc::clone(&self.log)
        }

        /// A shared handle to this reader's batched `read_many` invocation log.
        pub fn many_log_handle(&self) -> ManyLog {
            Arc::clone(&self.many_log)
        }
    }

    impl RangeReader for FakeReader {
        fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, CodecError> {
            let start = offset as usize;
            let end = start + len;
            if end > self.data.len() {
                return Err(CodecError::Decode("fake read out of bounds".to_string()));
            }
            self.log.lock().unwrap().push((offset, len));
            Ok(self.data[start..end].to_vec())
        }

        /// Override the serial default so tests can distinguish a single batched
        /// fetch from N `read_at`s. Records the batch in `many_log`, then reads
        /// each range (which also appends to the per-range `log`).
        fn read_many(&self, ranges: &[(u64, usize)]) -> Result<Vec<Vec<u8>>, CodecError> {
            self.many_log.lock().unwrap().push(ranges.to_vec());
            ranges.iter().map(|&(o, l)| self.read_at(o, l)).collect()
        }

        fn total_size(&self) -> u64 {
            self.data.len() as u64
        }
    }

    /// A reader that always fails, to exercise error propagation.
    struct FailingReader {
        size: u64,
    }

    impl RangeReader for FailingReader {
        fn read_at(&self, _offset: u64, _len: usize) -> Result<Vec<u8>, CodecError> {
            Err(CodecError::Io(std::io::Error::other("network down")))
        }
        fn total_size(&self) -> u64 {
            self.size
        }
    }

    fn calls(log: &Arc<Mutex<Vec<(u64, usize)>>>) -> Vec<(u64, usize)> {
        log.lock().unwrap().clone()
    }

    fn total_fetched(log: &Arc<Mutex<Vec<(u64, usize)>>>) -> usize {
        log.lock().unwrap().iter().map(|(_, len)| *len).sum()
    }

    #[test]
    fn test_read_range_returns_correct_bytes() {
        let data: Vec<u8> = (0..=255).collect();
        let reader = FakeReader::new(data.clone());
        let fetcher = StreamFetcher::with_policy(
            Box::new(reader),
            Box::new(HeaderAwarePolicy::new(0)), // no eager header
        );
        let bytes = fetcher.read_range(100, 10).unwrap();
        assert_eq!(bytes, &data[100..110]);
    }

    #[test]
    fn test_fetches_only_requested_range_without_header_prefetch() {
        let data: Vec<u8> = (0..200).map(|i| (i % 256) as u8).collect();
        let reader = FakeReader::new(data);
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        fetcher.read_range(50, 20).unwrap();
        // Exactly one fetch, of exactly the requested range — never the whole file.
        assert_eq!(calls(&log), vec![(50, 20)]);
        assert!(total_fetched(&log) < 200);
    }

    #[test]
    fn test_repeated_access_hits_cache() {
        let data: Vec<u8> = (0..100).collect();
        let reader = FakeReader::new(data);
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let first = fetcher.read_range(10, 5).unwrap();
        let second = fetcher.read_range(10, 5).unwrap();
        assert_eq!(first, second);
        // The second read was served from cache: still exactly one fetch.
        assert_eq!(calls(&log).len(), 1);
    }

    #[test]
    fn test_sub_range_of_cached_hits_cache() {
        let data: Vec<u8> = (0..100).collect();
        let reader = FakeReader::new(data);
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        fetcher.read_range(10, 20).unwrap(); // caches [10, 30)
        fetcher.read_range(15, 5).unwrap(); // sub-range, no new fetch
        assert_eq!(calls(&log).len(), 1);
    }

    #[test]
    fn test_header_prefetch_coalesces_into_one_read() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let reader = FakeReader::new(data);
        let log = reader.log_handle();
        // Header region [0, 100) plus a request at [20, 40) within it.
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(100)));
        fetcher.read_range(20, 20).unwrap();
        // One coalesced fetch covering the header region.
        assert_eq!(calls(&log), vec![(0, 100)]);
        // A subsequent access within the prefetched region is a cache hit.
        fetcher.read_range(50, 10).unwrap();
        assert_eq!(calls(&log).len(), 1);
    }

    #[test]
    fn test_whole_file_never_pulled_across_many_accesses() {
        let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
        let reader = FakeReader::new(data);
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(64)));
        for off in (0..5000).step_by(500) {
            fetcher.read_range(off as u64, 10).unwrap();
        }
        // Sum of fetched bytes is far below the 5000-byte file.
        assert!(
            total_fetched(&log) < 5000,
            "fetched {} bytes of a 5000-byte file",
            total_fetched(&log)
        );
    }

    #[test]
    fn test_error_propagates_as_codec_error() {
        let fetcher = StreamFetcher::with_policy(
            Box::new(FailingReader { size: 1000 }),
            Box::new(HeaderAwarePolicy::new(0)),
        );
        let err = fetcher.read_range(0, 10).unwrap_err();
        assert!(matches!(err, CodecError::Io(_)));
    }

    #[test]
    fn test_out_of_bounds_request_errors() {
        let reader = FakeReader::new(vec![0u8; 100]);
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let err = fetcher.read_range(90, 20).unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn test_zero_length_read_fetches_nothing() {
        let reader = FakeReader::new(vec![0u8; 100]);
        let log = reader.log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let bytes = fetcher.read_range(50, 0).unwrap();
        assert!(bytes.is_empty());
        assert!(calls(&log).is_empty());
    }

    #[test]
    fn test_coalesce_merges_overlapping_and_adjacent() {
        let merged = coalesce(vec![0..10, 10..20, 5..15, 100..110]);
        assert_eq!(merged, vec![0..20, 100..110]);
    }

    #[test]
    fn test_read_many_default_returns_one_buffer_per_range_in_order() {
        let data: Vec<u8> = (0..=255).collect();
        let reader = FakeReader::new(data.clone());
        let log = reader.log_handle();
        let ranges = [(100, 10), (0, 4), (250, 6)];
        let result = reader.read_many(&ranges).unwrap();
        // One buffer per range, in input order, each exactly its requested length.
        assert_eq!(result.len(), ranges.len());
        assert_eq!(result[0], &data[100..110]);
        assert_eq!(result[1], &data[0..4]);
        assert_eq!(result[2], &data[250..256]);
        for (buf, &(_, len)) in result.iter().zip(ranges.iter()) {
            assert_eq!(buf.len(), len);
        }
        // The default impl issues one read_at per range, in order.
        assert_eq!(calls(&log), vec![(100, 10), (0, 4), (250, 6)]);
    }

    #[test]
    fn test_read_many_default_empty_input_returns_empty() {
        let reader = FakeReader::new(vec![0u8; 100]);
        let log = reader.log_handle();
        let result = reader.read_many(&[]).unwrap();
        assert!(result.is_empty());
        assert!(calls(&log).is_empty());
    }

    #[test]
    fn test_read_many_default_propagates_error() {
        let reader = FailingReader { size: 1000 };
        let err = reader.read_many(&[(0, 10)]).unwrap_err();
        assert!(matches!(err, CodecError::Io(_)));
    }

    #[test]
    fn test_total_size_reported() {
        let reader = FakeReader::new(vec![0u8; 4096]);
        let fetcher = StreamFetcher::new(Box::new(reader));
        assert_eq!(fetcher.total_size(), 4096);
    }

    #[test]
    fn test_read_ranges_returns_bytes_in_order() {
        let data: Vec<u8> = (0..=255).collect();
        let reader = FakeReader::new(data.clone());
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let ranges = [(100, 10), (0, 4), (250, 6)];
        let result = fetcher.read_ranges(&ranges).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], &data[100..110]);
        assert_eq!(result[1], &data[0..4]);
        assert_eq!(result[2], &data[250..256]);
    }

    #[test]
    fn test_read_ranges_issues_single_batched_read_many() {
        // The core Option-1 guarantee: scattered parts go out as ONE read_many
        // (one cat_ranges), not N serial reads.
        let data: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        let reader = FakeReader::new(data);
        let many_log = reader.many_log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        // Six scattered parts, à la a J2K tile's tile-parts.
        let ranges = [
            (13, 40),
            (500, 100),
            (900, 20),
            (1200, 64),
            (1600, 8),
            (1900, 50),
        ];
        fetcher.read_ranges(&ranges).unwrap();
        let many = many_log.lock().unwrap();
        assert_eq!(many.len(), 1, "expected exactly one batched read_many call");
        assert_eq!(many[0].len(), 6, "all six parts in the one batch");
    }

    #[test]
    fn test_read_ranges_byte_identical_to_individual_read_range() {
        let data: Vec<u8> = (0..1024).map(|i| (i * 7 % 256) as u8).collect();
        let ranges = [(10, 40), (300, 200), (700, 1), (900, 124)];

        let reader_batch = FakeReader::new(data.clone());
        let batch =
            StreamFetcher::with_policy(Box::new(reader_batch), Box::new(HeaderAwarePolicy::new(0)));
        let batched = batch.read_ranges(&ranges).unwrap();

        let reader_serial = FakeReader::new(data.clone());
        let serial = StreamFetcher::with_policy(
            Box::new(reader_serial),
            Box::new(HeaderAwarePolicy::new(0)),
        );
        for (i, &(o, l)) in ranges.iter().enumerate() {
            assert_eq!(batched[i], serial.read_range(o, l).unwrap());
        }
    }

    #[test]
    fn test_read_ranges_serves_cached_without_refetch() {
        let data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
        let reader = FakeReader::new(data);
        let log = reader.log_handle();
        let many_log = reader.many_log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        // Prime the cache with one range. Post-lock-refactor every fetch (even a
        // single read_range) goes through the cursor-free read_many path, so this
        // records one batched call of just the primed range.
        fetcher.read_range(100, 50).unwrap();
        assert_eq!(calls(&log).len(), 1);
        assert_eq!(many_log.lock().unwrap().as_slice(), &[vec![(100, 50)]]);
        // A batch mixing the cached range with two new ones fetches only the two.
        fetcher
            .read_ranges(&[(100, 50), (0, 10), (400, 20)])
            .unwrap();
        let many = many_log.lock().unwrap();
        // Two read_many calls total: the prime, then the batch of the two misses.
        assert_eq!(many.len(), 2);
        // The cached [100,150) is not re-fetched; only the two misses batch.
        assert_eq!(many[1], vec![(0, 10), (400, 20)]);
    }

    #[test]
    fn test_read_ranges_empty_input_returns_empty_and_fetches_nothing() {
        let reader = FakeReader::new(vec![0u8; 100]);
        let log = reader.log_handle();
        let many_log = reader.many_log_handle();
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let result = fetcher.read_ranges(&[]).unwrap();
        assert!(result.is_empty());
        assert!(calls(&log).is_empty());
        assert!(many_log.lock().unwrap().is_empty());
    }

    #[test]
    fn test_read_ranges_out_of_bounds_errors() {
        let reader = FakeReader::new(vec![0u8; 100]);
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let err = fetcher.read_ranges(&[(50, 10), (90, 20)]).unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn test_read_ranges_zero_length_range_yields_empty() {
        let data: Vec<u8> = (0..100).collect();
        let reader = FakeReader::new(data.clone());
        let fetcher =
            StreamFetcher::with_policy(Box::new(reader), Box::new(HeaderAwarePolicy::new(0)));
        let result = fetcher.read_ranges(&[(10, 0), (20, 5)]).unwrap();
        assert!(result[0].is_empty());
        assert_eq!(result[1], &data[20..25]);
    }
}
