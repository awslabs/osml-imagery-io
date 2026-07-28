//! Prefetch policy seam.
//!
//! A [`PrefetchPolicy`] decides, given a single requested byte range, which
//! ranges the fetcher should actually pull — the requested range plus any
//! speculative extras. Because the fetcher self-heals (an under-sized prefetch
//! simply triggers another fetch on the next access), a policy is a pure
//! optimization with no correctness cliff.
//!
//! v1 ships one implementation, [`HeaderAwarePolicy`]: on the first access to the
//! head of the file it eagerly fetches a header region (headers + offset tables
//! tend to cluster there), and it always includes the requested range. A future
//! spatial policy (Z-order / Hilbert / 2D adjacency) is just another
//! implementation of this trait and needs no reader or buffer-API change.

use std::ops::Range;

/// Given a requested range, may return extra ranges to fetch speculatively.
///
/// Implementations return the set of ranges the fetcher should pull to satisfy
/// (at least) `requested`. The returned ranges need not be sorted or disjoint —
/// the fetcher clamps them to `total_size` and coalesces them before fetching.
/// The requested range must always be covered by the union of the result.
pub trait PrefetchPolicy: Send {
    /// Plan the ranges to fetch for a request of `requested` against a source of
    /// `total_size` bytes.
    fn plan(&self, requested: Range<u64>, total_size: u64) -> Vec<Range<u64>>;
}

/// Default policy: fetch an eager header region for accesses near the start of
/// the file, plus the requested range.
///
/// The header region is `[0, header_len)` (clamped to `total_size`). It is only
/// added when the requested range begins within that region, so a random access
/// deep in the file does not drag the header along with it. Header + offset
/// tables in NITF/TIFF/J2K cluster at the front, so this coalesces the bootstrap
/// reads into a single fetch.
pub struct HeaderAwarePolicy {
    header_len: u64,
}

/// Conservative default header-region size (64 KiB). Under-sizing self-heals via
/// an extra fetch; the format-aware hint can override this.
pub const DEFAULT_HEADER_LEN: u64 = 64 * 1024;

impl HeaderAwarePolicy {
    /// Create a policy with the given eager header-region length in bytes.
    pub fn new(header_len: u64) -> Self {
        Self { header_len }
    }
}

impl Default for HeaderAwarePolicy {
    fn default() -> Self {
        Self::new(DEFAULT_HEADER_LEN)
    }
}

impl PrefetchPolicy for HeaderAwarePolicy {
    fn plan(&self, requested: Range<u64>, total_size: u64) -> Vec<Range<u64>> {
        let mut plan = Vec::with_capacity(2);

        // Eager header region, only for accesses that start within it.
        let header_end = self.header_len.min(total_size);
        if header_end > 0 && requested.start < header_end {
            plan.push(0..header_end);
        }

        // Always include the requested range (clamped to the source size).
        let start = requested.start.min(total_size);
        let end = requested.end.min(total_size);
        if end > start {
            plan.push(start..end);
        }

        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_region_included_for_head_access() {
        let policy = HeaderAwarePolicy::new(1000);
        let plan = policy.plan(0..100, 10_000);
        assert!(plan.contains(&(0..1000)));
        // Requested range is within the header region, so it's covered already.
        assert!(plan.iter().any(|r| r.start == 0 && r.end >= 100));
    }

    #[test]
    fn test_deep_access_excludes_header() {
        let policy = HeaderAwarePolicy::new(1000);
        let plan = policy.plan(5000..5100, 10_000);
        assert!(!plan.contains(&(0..1000)));
        assert_eq!(plan, vec![5000..5100]);
    }

    #[test]
    fn test_header_clamped_to_total_size() {
        let policy = HeaderAwarePolicy::new(1000);
        let plan = policy.plan(0..50, 200);
        // Header region clamped to the 200-byte file.
        assert!(plan.contains(&(0..200)));
    }

    #[test]
    fn test_requested_range_clamped_to_total_size() {
        let policy = HeaderAwarePolicy::new(10);
        let plan = policy.plan(150..250, 200);
        // Deep access (past header), clamped to 200.
        assert_eq!(plan, vec![150..200]);
    }

    #[test]
    fn test_empty_request_yields_only_header_when_at_head() {
        let policy = HeaderAwarePolicy::new(100);
        let plan = policy.plan(0..0, 1000);
        // Header region planned; the empty requested range contributes nothing.
        assert_eq!(plan, vec![0..100]);
    }

    #[test]
    fn test_zero_total_size() {
        let policy = HeaderAwarePolicy::default();
        let plan = policy.plan(0..0, 0);
        assert!(plan.is_empty());
    }
}
