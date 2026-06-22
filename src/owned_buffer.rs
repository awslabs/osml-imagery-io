use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use memmap2::Mmap;

use crate::error::CodecError;

#[derive(Clone)]
enum BackingStore {
    Mapped(Arc<Mmap>),
    Heap(Arc<Vec<u8>>),
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

    /// View the bytes this buffer represents.
    pub fn as_bytes(&self) -> &[u8] {
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

    /// Create a sub-slice (zero-copy). Panics on out-of-bounds.
    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(range.start <= range.end, "invalid range: start > end");
        assert!(range.end <= self.len(), "slice out of bounds");
        self.slice_unchecked(range)
    }

    /// Create a sub-slice (zero-copy). Returns error on out-of-bounds.
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
        Ok(self.slice_unchecked(range))
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
        }
    }
}

impl PartialEq for OwnedBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for OwnedBuffer {}

impl fmt::Debug for OwnedBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
