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
