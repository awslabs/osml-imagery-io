//! Remote range-read machinery.
//!
//! This module provides the byte-range fetch foundation for reading remote-backed
//! sources (e.g. fsspec/s3fs file-like objects) without downloading the whole file.
//! It is deliberately independent of [`crate::owned_buffer::OwnedBuffer`]: the
//! `Remote` backing wraps a [`StreamFetcher`], but the fetcher itself only knows
//! how to pull `(offset, len)` byte ranges from an abstract [`RangeReader`] and
//! cache them.
//!
//! # Components
//!
//! - [`RangeReader`] — the `(offset, len) -> bytes` seam. Both the Python-backed
//!   stream reader (added later) and the in-memory test fake implement it, so the
//!   fetcher can be exercised with no `Py<PyAny>`.
//! - [`StreamFetcher`] — owns a [`RangeReader`], a [`RangeCache`], and a
//!   [`PrefetchPolicy`]. Its [`StreamFetcher::read_range`] is the single fetch
//!   entry point: it serves from cache when possible, otherwise plans (and
//!   coalesces) ranges via the policy, fetches the misses, and caches them.
//! - [`RangeCache`] — resident, non-overlapping byte ranges; extensible to
//!   eviction later (a v1 Non-Goal).
//! - [`PrefetchPolicy`] — a pluggable strategy that, given a requested range, may
//!   return extra ranges to fetch speculatively. The default
//!   [`HeaderAwarePolicy`] coalesces an eager header region with the requested
//!   range.
//!
//! Nothing here understands imagery formats; the format-aware header hint is
//! supplied by the IO dispatch layer.

mod cache;
mod fetcher;
mod policy;

pub use cache::RangeCache;
pub use fetcher::{RangeReader, StreamFetcher};
pub use policy::{HeaderAwarePolicy, PrefetchPolicy};

/// In-memory fake [`RangeReader`] with a call log, re-exported for use by other
/// modules' `#[cfg(test)]` code (e.g. `owned_buffer` `Remote`-backing tests).
#[cfg(test)]
pub(crate) use fetcher::tests::FakeReader;
