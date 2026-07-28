//! Resident-range cache for the remote fetcher.
//!
//! A [`RangeCache`] stores byte ranges that have already been fetched so a
//! repeated access is served from memory instead of re-invoking the underlying
//! [`RangeReader`](crate::remote::RangeReader). v1 keeps every fetched range
//! resident (no eviction — a documented Non-Goal); the structure is deliberately
//! simple so LRU / tiered eviction can be layered on later without changing the
//! public shape.

/// One contiguous resident byte range: the bytes plus the absolute file offset
/// at which they start.
struct CachedRange {
    start: u64,
    data: Vec<u8>,
}

impl CachedRange {
    /// The absolute end offset (exclusive) of this cached range.
    fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }

    /// True if this cached range fully covers `[start, end)`.
    fn covers(&self, start: u64, end: u64) -> bool {
        self.start <= start && end <= self.end()
    }
}

/// A cache of resident, non-overlapping byte ranges keyed by absolute offset.
///
/// Ranges are kept sorted by start offset and merged on insert so that adjacent
/// or overlapping fetches coalesce into a single contiguous span. This keeps
/// lookups cheap and avoids storing the same bytes twice.
#[derive(Default)]
pub struct RangeCache {
    ranges: Vec<CachedRange>,
}

impl RangeCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// Return the bytes for `[offset, offset + len)` if a single cached range
    /// fully covers the request, otherwise `None`.
    ///
    /// A zero-length request is always considered covered (returns an empty
    /// slice) so callers never fetch nothing.
    pub fn get(&self, offset: u64, len: usize) -> Option<Vec<u8>> {
        let end = offset + len as u64;
        if len == 0 {
            return Some(Vec::new());
        }
        for range in &self.ranges {
            if range.covers(offset, end) {
                let local = (offset - range.start) as usize;
                return Some(range.data[local..local + len].to_vec());
            }
        }
        None
    }

    /// True if `[offset, offset + len)` is fully covered by a single cached range.
    pub fn contains(&self, offset: u64, len: usize) -> bool {
        if len == 0 {
            return true;
        }
        let end = offset + len as u64;
        self.ranges.iter().any(|r| r.covers(offset, end))
    }

    /// Insert a fetched range at `start`, merging with any adjacent or
    /// overlapping cached ranges so the cache stays non-overlapping.
    pub fn insert(&mut self, start: u64, data: Vec<u8>) {
        if data.is_empty() {
            return;
        }
        let mut new = CachedRange { start, data };

        // Merge with every existing range that touches or overlaps the new one,
        // removing them as we absorb their bytes.
        let mut merged = Vec::with_capacity(self.ranges.len() + 1);
        for existing in std::mem::take(&mut self.ranges) {
            if ranges_touch(&existing, &new) {
                new = merge(existing, new);
            } else {
                merged.push(existing);
            }
        }
        merged.push(new);
        merged.sort_by_key(|r| r.start);
        self.ranges = merged;
    }

    /// Total number of bytes resident in the cache (across all ranges).
    pub fn resident_bytes(&self) -> usize {
        self.ranges.iter().map(|r| r.data.len()).sum()
    }
}

/// True if two ranges overlap or are directly adjacent (so they can be merged
/// into one contiguous span).
fn ranges_touch(a: &CachedRange, b: &CachedRange) -> bool {
    a.start <= b.end() && b.start <= a.end()
}

/// Merge two touching/overlapping ranges into one, preferring `a`'s bytes where
/// they overlap `b` (the specific choice is immaterial — overlapping bytes are
/// identical for a consistent source).
fn merge(a: CachedRange, b: CachedRange) -> CachedRange {
    let start = a.start.min(b.start);
    let end = a.end().max(b.end());
    let mut data = vec![0u8; (end - start) as usize];

    // Lay down b first, then a, so a wins on overlap.
    for src in [&b, &a] {
        let dst_off = (src.start - start) as usize;
        data[dst_off..dst_off + src.data.len()].copy_from_slice(&src.data);
    }
    CachedRange { start, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_cache_misses() {
        let cache = RangeCache::new();
        assert!(!cache.contains(0, 10));
        assert_eq!(cache.get(0, 10), None);
    }

    #[test]
    fn test_insert_and_get_exact() {
        let mut cache = RangeCache::new();
        cache.insert(100, vec![1, 2, 3, 4]);
        assert!(cache.contains(100, 4));
        assert_eq!(cache.get(100, 4), Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn test_get_sub_range() {
        let mut cache = RangeCache::new();
        cache.insert(10, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(cache.get(12, 2), Some(vec![2, 3]));
    }

    #[test]
    fn test_partial_coverage_misses() {
        let mut cache = RangeCache::new();
        cache.insert(0, vec![0, 1, 2, 3]);
        // Request extends past the cached range.
        assert!(!cache.contains(2, 5));
        assert_eq!(cache.get(2, 5), None);
    }

    #[test]
    fn test_zero_length_always_covered() {
        let cache = RangeCache::new();
        assert!(cache.contains(50, 0));
        assert_eq!(cache.get(50, 0), Some(Vec::new()));
    }

    #[test]
    fn test_adjacent_ranges_merge() {
        let mut cache = RangeCache::new();
        cache.insert(0, vec![0, 1, 2, 3]);
        cache.insert(4, vec![4, 5, 6, 7]);
        // Now a single span [0, 8) exists and covers a cross-boundary request.
        assert!(cache.contains(2, 4));
        assert_eq!(cache.get(2, 4), Some(vec![2, 3, 4, 5]));
        assert_eq!(cache.resident_bytes(), 8);
    }

    #[test]
    fn test_overlapping_ranges_merge_without_duplication() {
        let mut cache = RangeCache::new();
        cache.insert(0, vec![0, 1, 2, 3, 4]);
        cache.insert(3, vec![3, 4, 5, 6]);
        assert_eq!(cache.get(0, 7), Some(vec![0, 1, 2, 3, 4, 5, 6]));
        assert_eq!(cache.resident_bytes(), 7);
    }

    #[test]
    fn test_disjoint_ranges_stay_separate() {
        let mut cache = RangeCache::new();
        cache.insert(0, vec![0, 1]);
        cache.insert(100, vec![9, 9]);
        assert!(cache.contains(0, 2));
        assert!(cache.contains(100, 2));
        // A request spanning the gap is not covered.
        assert!(!cache.contains(0, 102));
        assert_eq!(cache.resident_bytes(), 4);
    }
}
