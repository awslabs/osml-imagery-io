//! Python write-stream adapter for `std::io::Write`.
//!
//! This module provides `PyWriteStream`, a Rust struct that wraps a Python
//! writable file-like object (one implementing `.write()` and `.flush()`) and
//! implements `std::io::Write`. It is the bridge between Rust's I/O traits and
//! Python's file-like protocol, allowing format writers to target Python
//! streams (e.g., `io.BytesIO`, fsspec handles) without knowing about Python.
//!
//! Each `write()` and `flush()` call acquires the GIL and dispatches to the
//! corresponding Python method. Because every call crosses the GIL boundary,
//! callers should wrap `PyWriteStream` in `std::io::BufWriter` to batch small
//! writes and reduce the number of GIL crossings.
//!
//! This struct is NOT exposed as a `#[pyclass]` — it is a crate-internal
//! implementation detail used only by the IO dispatch layer in
//! `crate::bindings::io`.

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::CodecError;
use crate::remote::RangeReader;

/// Adapts a Python **seekable** file-like object to the [`RangeReader`] seam so
/// a [`crate::remote::StreamFetcher`] (and, above it, a `Remote`
/// [`OwnedBuffer`](crate::owned_buffer::OwnedBuffer)) can pull arbitrary byte
/// ranges on demand instead of downloading the whole file.
///
/// Each [`read_at`](RangeReader::read_at) acquires the GIL, seeks the Python
/// stream to the absolute offset, and reads exactly `len` bytes. The total size
/// is probed once by the IO dispatch layer and passed to the constructor, then
/// cached here, so [`total_size`](RangeReader::total_size) never crosses the GIL.
///
/// The IO dispatch layer is responsible for verifying the wrapped object is
/// seekable and has a known size before constructing this adapter (see
/// `probe_seekable_size` in `crate::bindings::io`); this struct assumes both.
///
/// When the wrapped object is an fsspec file handle, the IO dispatch layer also
/// recovers its `(filesystem, path)` back-references (via [`probe_fsspec_refs`])
/// and passes them to the constructor. With those present,
/// [`read_many`](RangeReader::read_many) fans the requested ranges out in one
/// concurrent `cat_ranges` call on the filesystem — the stateless, cursor-free
/// path that is parallelized across threads. Without them (an
/// `io.BytesIO` or plain-file handle), `read_many` falls back to the serial
/// `read_at` loop.
pub(crate) struct PyReadStream {
    /// Reference to the wrapped Python readable+seekable object.
    py_obj: Py<PyAny>,
    /// Total size in bytes, probed once at construction.
    total_size: u64,
    /// `Some((fs, path))` for fsspec handles (enables concurrent `cat_ranges`);
    /// `None` for `io.BytesIO` / plain files (serial `read_at` fallback).
    fsspec: Option<(Py<PyAny>, String)>,
    /// Debug-only re-entry guard for the stateful `seek`+`read` cursor path.
    ///
    /// Enforces the **containment invariant**: [`read_at`](RangeReader::read_at)
    /// mutates the wrapped handle's single cursor (`seek` then `read`), so it is
    /// unsafe to enter concurrently. Concurrent fetches are supposed to flow
    /// through the cursor-free [`read_many`](RangeReader::read_many)→`cat_ranges`
    /// path instead; this flag makes a violation fail loudly in debug/test builds
    /// rather than silently corrupt the cursor. Compiled out of release builds.
    #[cfg(debug_assertions)]
    in_cursor_read: std::sync::atomic::AtomicBool,
}

/// RAII guard that clears [`PyReadStream::in_cursor_read`] when a `read_at`
/// returns, so an early `?` exit does not leave the flag stuck.
#[cfg(debug_assertions)]
struct CursorGuard<'a>(&'a std::sync::atomic::AtomicBool);

#[cfg(debug_assertions)]
impl Drop for CursorGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl PyReadStream {
    /// Create a reader over `py_obj`, which the caller has already verified is
    /// seekable, readable, and `total_size` bytes long.
    ///
    /// `fsspec` carries the recovered `(filesystem, path)` back-references when
    /// `py_obj` is an fsspec handle (see [`probe_fsspec_refs`]), or `None` for a
    /// non-fsspec handle. It selects the [`read_many`](RangeReader::read_many)
    /// strategy: concurrent `cat_ranges` when present, serial otherwise.
    pub(crate) fn new(
        py_obj: Py<PyAny>,
        total_size: u64,
        fsspec: Option<(Py<PyAny>, String)>,
    ) -> Self {
        Self {
            py_obj,
            total_size,
            fsspec,
            #[cfg(debug_assertions)]
            in_cursor_read: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// Recovers an fsspec handle's `(filesystem, path)` back-references so range
/// reads can be routed through the filesystem's concurrent `cat_ranges` instead
/// of the handle's stateful `seek`+`read` cursor.
///
/// fsspec file handles expose `.fs` (the owning filesystem) and `.path` (the
/// object key); see the remote range-read design. Returns `None` for any handle
/// missing either attribute (e.g. `io.BytesIO`, a plain file object) or whose
/// `.fs` is `None`, signalling the caller to keep the serial fallback.
pub(crate) fn probe_fsspec_refs(
    py: Python<'_>,
    stream_obj: &Py<PyAny>,
) -> Option<(Py<PyAny>, String)> {
    let bound = stream_obj.bind(py);
    let fs = bound.getattr("fs").ok()?;
    if fs.is_none() {
        return None;
    }
    let path: String = bound.getattr("path").ok()?.extract().ok()?;
    Some((fs.unbind(), path))
}

impl RangeReader for PyReadStream {
    fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, CodecError> {
        if len == 0 {
            return Ok(Vec::new());
        }
        // Containment invariant: the seek+read cursor path must never be entered
        // by more than one thread at a time. Concurrent fetches are routed through
        // read_many→cat_ranges (cursor-free); a debug re-entry assertion catches
        // any regression that lets two threads race this cursor.
        #[cfg(debug_assertions)]
        let _guard = {
            use std::sync::atomic::Ordering;
            let already_in = self.in_cursor_read.swap(true, Ordering::SeqCst);
            debug_assert!(
                !already_in,
                "PyReadStream::read_at (seek+read cursor) entered concurrently; \
                 concurrent reads must flow through read_many/cat_ranges"
            );
            // Reset the flag when this read returns, even on early exit.
            CursorGuard(&self.in_cursor_read)
        };
        Python::attach(|py| {
            let bound = self.py_obj.bind(py);
            // Seek to the absolute offset (whence=0, SEEK_SET is the default).
            bound.call_method1("seek", (offset,)).map_err(|e| {
                CodecError::Remote(format!("stream seek to {} failed: {}", offset, e))
            })?;
            let result = bound.call_method1("read", (len,)).map_err(|e| {
                CodecError::Remote(format!(
                    "stream read of {} bytes at {} failed: {}",
                    len, offset, e
                ))
            })?;
            let py_bytes = result.cast::<PyBytes>().map_err(|_| {
                CodecError::Remote(".read() must return bytes for range reads".to_string())
            })?;
            let data = py_bytes.as_bytes();
            // A range source must return exactly `len` bytes; a short read means
            // the range could not be satisfied.
            if data.len() != len {
                return Err(CodecError::Remote(format!(
                    "short read: requested {} bytes at offset {}, got {}",
                    len,
                    offset,
                    data.len()
                )));
            }
            Ok(data.to_vec())
        })
    }

    /// Fetch multiple ranges in one shot.
    ///
    /// For an fsspec handle (`self.fsspec` is `Some`) this issues a single
    /// `fs.cat_ranges(paths, starts, ends)` call, which fans one concurrent GET
    /// out per range on fsspec's background event loop and blocks the caller on a
    /// `threading.Event` (releasing the GIL while blocked). For a non-fsspec
    /// handle it falls back to the serial `read_at` loop (today's behavior).
    ///
    /// `cat_ranges` returns `list[bytes]` in input order; each element must be
    /// exactly its requested `len`. Its `on_error` stays at the default `"raise"`,
    /// so any failed range surfaces as a Python exception — all-or-nothing,
    /// matching [`read_at`](RangeReader::read_at).
    fn read_many(&self, ranges: &[(u64, usize)]) -> Result<Vec<Vec<u8>>, CodecError> {
        let Some((fs, path)) = &self.fsspec else {
            // Non-fsspec handle: serial fallback (today's behavior).
            return ranges.iter().map(|&(o, l)| self.read_at(o, l)).collect();
        };
        if ranges.is_empty() {
            return Ok(Vec::new());
        }
        Python::attach(|py| {
            let starts: Vec<u64> = ranges.iter().map(|&(o, _)| o).collect();
            let ends: Vec<u64> = ranges.iter().map(|&(o, l)| o + l as u64).collect();
            let paths = vec![path.as_str(); ranges.len()];

            // Blocks on fsspec's background event loop; concurrent GETs; the GIL
            // is released while blocked. on_error defaults to "raise", so any
            // failed range surfaces here as a Python exception.
            let result = fs
                .bind(py)
                .call_method1("cat_ranges", (paths, starts, ends))
                .map_err(|e| CodecError::Remote(format!("cat_ranges failed: {}", e)))?;

            // result is list[bytes] in input order. Validate per-range length so a
            // short range is a hard error, matching read_at's contract.
            let items = result.try_iter().map_err(|e| {
                CodecError::Remote(format!("cat_ranges did not return an iterable: {}", e))
            })?;
            let mut out: Vec<Vec<u8>> = Vec::with_capacity(ranges.len());
            for (i, item) in items.enumerate() {
                let item = item
                    .map_err(|e| CodecError::Remote(format!("cat_ranges iteration failed: {}", e)))?;
                let py_bytes = item.cast::<PyBytes>().map_err(|_| {
                    CodecError::Remote("cat_ranges must return bytes for each range".to_string())
                })?;
                let data = py_bytes.as_bytes();
                let expected = ranges
                    .get(i)
                    .map(|&(_, l)| l)
                    .ok_or_else(|| CodecError::Remote("cat_ranges returned too many ranges".to_string()))?;
                if data.len() != expected {
                    let (offset, _) = ranges[i];
                    return Err(CodecError::Remote(format!(
                        "short range: requested {} bytes at offset {}, got {}",
                        expected,
                        offset,
                        data.len()
                    )));
                }
                out.push(data.to_vec());
            }
            if out.len() != ranges.len() {
                return Err(CodecError::Remote(format!(
                    "cat_ranges returned {} ranges, expected {}",
                    out.len(),
                    ranges.len()
                )));
            }
            Ok(out)
        })
    }

    fn total_size(&self) -> u64 {
        self.total_size
    }
}

// SAFETY: `Py<PyAny>` is `Send` (PyO3 guarantees this — access is gated by the
// GIL, which every method re-acquires via `Python::attach`). The `fsspec`
// back-references are `Option<(Py<PyAny>, String)>` — both components `Send` for
// the same reason — and the debug re-entry guard is an `AtomicBool`, so
// `PyReadStream` has no non-`Send` fields and is safe to move between threads.
// (`PyReadStream` is also `Sync`: `Py<T>` is `Sync` and `AtomicBool` is `Sync`,
// which the `&self` `RangeReader` methods rely on so N threads can fetch through
// one reader. This explicit `Send` impl is retained to document the GIL contract;
// it matches the auto-derived bound.) Same contract `PyWriteStream` relies on.
unsafe impl Send for PyReadStream {}

/// Wraps a Python writable file-like object and implements `std::io::Write`.
///
/// Each `write()` call acquires the GIL and invokes `.write(bytes)` on the
/// wrapped Python object; `flush()` does the same for `.flush()`. Python
/// exceptions raised during either call are converted to `std::io::Error`
/// (via `std::io::Error::other`) so they can flow through the standard Rust
/// `Write` trait without panicking.
///
/// Constructors (the IO dispatch layer) are responsible for verifying that
/// the wrapped object has `.write()` and `.flush()` methods before
/// constructing this adapter — this struct does not validate at construction
/// time.
pub(crate) struct PyWriteStream {
    /// Reference to the wrapped Python writable object.
    py_obj: Py<PyAny>,
}

impl PyWriteStream {
    /// Create a new `PyWriteStream` wrapping the given Python object.
    ///
    /// The caller must verify that `py_obj` has `.write()` and `.flush()`
    /// methods before constructing this adapter.
    pub(crate) fn new(py_obj: Py<PyAny>) -> Self {
        Self { py_obj }
    }
}

impl std::io::Write for PyWriteStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Python::attach(|py| {
            let py_bytes = PyBytes::new(py, buf);
            let result = self
                .py_obj
                .call_method1(py, "write", (py_bytes,))
                .map_err(|e| std::io::Error::other(format!("Python write error: {}", e)))?;
            // Python's write() returns the number of bytes written as an int.
            result.extract::<usize>(py).map_err(|e| {
                std::io::Error::other(format!(
                    "Python write() returned non-integer or negative value: {}",
                    e
                ))
            })
        })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Python::attach(|py| {
            self.py_obj
                .call_method0(py, "flush")
                .map_err(|e| std::io::Error::other(format!("Python flush error: {}", e)))?;
            Ok(())
        })
    }
}

// SAFETY: `Py<PyAny>` is `Send` (PyO3 guarantees this — the reference is
// opaque and access is gated by the GIL). `PyWriteStream` has no other
// fields, so it is also safe to move between threads.
unsafe impl Send for PyWriteStream {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Drive `read_many` through a real fsspec `LocalFileSystem` handle: probe
    /// its `.fs`/`.path`, then fan several ranges out via `cat_ranges` and assert
    /// each range is byte-correct and exactly its requested length.
    #[test]
    fn pyreadstream_read_many_over_fsspec_localfilesystem() {
        Python::attach(|py| {
            // Write a known 256-byte payload to a temp file, then open it through
            // fsspec's LocalFileSystem so the handle carries `.fs`/`.path`.
            let code = "\
import fsspec, tempfile, os
data = bytes(range(256))
d = tempfile.mkdtemp()
p = os.path.join(d, 'payload.bin')
with open(p, 'wb') as f:
    f.write(data)
fs = fsspec.filesystem('file')
handle = fs.open(p, 'rb')
";
            let globals = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
                .unwrap();
            let handle: Py<PyAny> = globals
                .get_item("handle")
                .unwrap()
                .unwrap()
                .unbind();

            let fsspec = probe_fsspec_refs(py, &handle);
            assert!(fsspec.is_some(), "LocalFileSystem handle should expose .fs/.path");

            let stream = PyReadStream::new(handle, 256, fsspec);
            let ranges = [(100u64, 10usize), (0, 4), (250, 6)];
            let result = stream.read_many(&ranges).unwrap();

            assert_eq!(result.len(), 3);
            let expected: Vec<u8> = (0..=255).collect();
            assert_eq!(result[0], &expected[100..110]);
            assert_eq!(result[1], &expected[0..4]);
            assert_eq!(result[2], &expected[250..256]);
            for (buf, &(_, len)) in result.iter().zip(ranges.iter()) {
                assert_eq!(buf.len(), len);
            }

            // Empty input short-circuits to an empty result without touching the fs.
            assert!(stream.read_many(&[]).unwrap().is_empty());
        });
    }

    /// A non-fsspec handle (`io.BytesIO`) has no `.fs`/`.path`, so `read_many`
    /// falls back to the serial `read_at` loop and still returns correct bytes.
    #[test]
    fn pyreadstream_read_many_serial_fallback_for_bytesio() {
        Python::attach(|py| {
            let io_module = py.import("io").unwrap();
            let payload: Vec<u8> = (0..=255).collect();
            let py_bytes = PyBytes::new(py, &payload);
            let bytesio = io_module.call_method1("BytesIO", (py_bytes,)).unwrap();
            let handle: Py<PyAny> = bytesio.unbind();

            // BytesIO exposes no fsspec back-references.
            let fsspec = probe_fsspec_refs(py, &handle);
            assert!(fsspec.is_none());

            let stream = PyReadStream::new(handle, 256, fsspec);
            let ranges = [(100u64, 10usize), (0, 4), (250, 6)];
            let result = stream.read_many(&ranges).unwrap();

            assert_eq!(result.len(), 3);
            assert_eq!(result[0], &payload[100..110]);
            assert_eq!(result[1], &payload[0..4]);
            assert_eq!(result[2], &payload[250..256]);
        });
    }

    /// A short range from the fsspec path is a hard `CodecError::Remote`, not a
    /// silent short read — matching `read_at`'s all-or-nothing contract. A range
    /// running past EOF makes `cat_ranges` return fewer than `len` bytes.
    #[test]
    fn pyreadstream_read_many_short_range_errors() {
        Python::attach(|py| {
            let code = "\
import fsspec, tempfile, os
d = tempfile.mkdtemp()
p = os.path.join(d, 'small.bin')
with open(p, 'wb') as f:
    f.write(bytes(range(16)))
fs = fsspec.filesystem('file')
handle = fs.open(p, 'rb')
";
            let globals = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
                .unwrap();
            let handle: Py<PyAny> = globals.get_item("handle").unwrap().unwrap().unbind();

            let fsspec = probe_fsspec_refs(py, &handle);
            let stream = PyReadStream::new(handle, 16, fsspec);
            // Request 10 bytes starting at 12 — only 4 are available.
            let err = stream.read_many(&[(12, 10)]).unwrap_err();
            assert!(matches!(err, CodecError::Remote(_)), "unexpected error: {:?}", err);
        });
    }

    /// Compile-time check: `PyWriteStream` implements `std::io::Write`.
    #[test]
    fn pywritestream_implements_write() {
        fn assert_write<T: Write>() {}
        assert_write::<PyWriteStream>();
    }

    /// Compile-time check: `PyWriteStream` is `Send`, so it can be stored in
    /// `Box<dyn Write + Send>` and moved across threads.
    #[test]
    fn pywritestream_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PyWriteStream>();
    }

    /// End-to-end smoke test: writing bytes to a `PyWriteStream` wrapping an
    /// `io.BytesIO` buffer produces the expected bytes on the Python side,
    /// and `flush()` succeeds without error.
    #[test]
    fn pywritestream_writes_and_flushes_to_bytesio() {
        Python::attach(|py| {
            let io_module = py.import("io").unwrap();
            let bytesio = io_module.call_method0("BytesIO").unwrap();
            let py_obj: Py<PyAny> = bytesio.clone().unbind();

            let mut stream = PyWriteStream::new(py_obj);
            let payload = b"hello, stream";
            let n = stream.write(payload).unwrap();
            assert_eq!(n, payload.len());
            stream.flush().unwrap();

            // Verify the BytesIO actually received the bytes.
            let written: Vec<u8> = bytesio.call_method0("getvalue").unwrap().extract().unwrap();
            assert_eq!(written, payload);
        });
    }

    /// A Python object that raises on `.write()` should produce an
    /// `std::io::Error` rather than panicking.
    #[test]
    fn pywritestream_write_error_converts_to_io_error() {
        Python::attach(|py| {
            let code = "\
class RaisingWriter:
    def write(self, data):
        raise RuntimeError('boom on write')
    def flush(self):
        pass
";
            let globals = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
                .unwrap();
            let cls = globals.get_item("RaisingWriter").unwrap().unwrap();
            let instance = cls.call0().unwrap();
            let py_obj: Py<PyAny> = instance.unbind();

            let mut stream = PyWriteStream::new(py_obj);
            let err = stream.write(b"data").unwrap_err();
            assert!(
                err.to_string().contains("Python write error"),
                "unexpected error message: {}",
                err
            );
        });
    }

    /// A Python object that raises on `.flush()` should produce an
    /// `std::io::Error` rather than panicking.
    #[test]
    fn pywritestream_flush_error_converts_to_io_error() {
        Python::attach(|py| {
            let code = "\
class RaisingFlusher:
    def write(self, data):
        return len(data)
    def flush(self):
        raise RuntimeError('boom on flush')
";
            let globals = pyo3::types::PyDict::new(py);
            py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
                .unwrap();
            let cls = globals.get_item("RaisingFlusher").unwrap().unwrap();
            let instance = cls.call0().unwrap();
            let py_obj: Py<PyAny> = instance.unbind();

            let mut stream = PyWriteStream::new(py_obj);
            let err = stream.flush().unwrap_err();
            assert!(
                err.to_string().contains("Python flush error"),
                "unexpected error message: {}",
                err
            );
        });
    }
}
