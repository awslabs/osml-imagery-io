//! Python bindings for DatasetWriter.
//!
//! This module provides the PyDatasetWriter wrapper that exposes the
//! DatasetWriter trait to Python with context manager support.

use std::sync::Arc;

use pyo3::prelude::*;

use crate::bindings::callback_provider::{
    is_duck_typed_image_provider, PyCallbackImageAssetProvider,
};
use crate::bindings::{
    PyAssetProvider, PyBufferedDataAssetProvider, PyBufferedImageAssetProvider,
    PyBufferedTextAssetProvider, PyDataAssetProvider, PyGraphicsAssetProvider,
    PyImageAssetProvider, PyMetadataProvider, PyTextAssetProvider,
};
use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetWriter};

/// Provides write access to geospatial datasets.
///
/// A :class:`DatasetWriter` creates a new geospatial dataset (NITF, GeoTIFF,
/// etc.) and populates it with assets and metadata. Use :meth:`IO.open` with
/// mode ``"w"`` and a format name to obtain an instance. The writer handles
/// format-specific encoding details so you can focus on the content. It
/// supports the Python context manager protocol, so resources are flushed and
/// released automatically when the ``with`` block exits.
///
/// Example:
///
/// ```python
/// from aws.osml.io import IO, BufferedMetadataProvider
///
/// metadata = BufferedMetadataProvider()
/// metadata.set("IC", "NC")
///
/// with IO.open(["output.ntf"], "w", "nitf") as writer:
///     writer.metadata = metadata
///     writer.add_asset(
///         "image:0", image_provider,
///         "Primary Image", "RGB scene", ["data"],
///     )
/// ```
#[pyclass(name = "DatasetWriter")]
pub struct PyDatasetWriter {
    inner: Option<Box<dyn DatasetWriter>>,
    /// Python file handles the *library* opened for remote destinations and is
    /// therefore responsible for closing after `inner.close()` commits the bytes.
    /// Object-store uploads finalize on a handle's `.close()`, not on `.flush()`,
    /// so a library-opened remote destination is only committed once its handle is
    /// closed. There is one entry per library-opened remote key: one for a
    /// single-path write, N for a multi-path R-set pyramid. Empty for local files
    /// and caller-supplied streams — the caller owns and closes those, and closing
    /// another party's handle is not our concern.
    owned_handles: Vec<Py<PyAny>>,
}

impl PyDatasetWriter {
    /// Creates a new PyDatasetWriter wrapping the given trait object.
    ///
    /// The writer owns no Python handle, so `close()` finalizes only the format
    /// writer — the behavior for local files and caller-supplied streams.
    pub fn new(inner: Box<dyn DatasetWriter>) -> Self {
        Self {
            inner: Some(inner),
            owned_handles: Vec::new(),
        }
    }

    /// Creates a new PyDatasetWriter that also owns a single Python file handle.
    ///
    /// Use this when the *library* opened one destination handle (a remote
    /// `s3://`/`memory://` URL or an explicit `filesystem=` + path). `close()`
    /// finalizes `inner` first, then closes `handle` so the object-store upload
    /// commits. See [`PyDatasetWriter::close`] for the ordering rationale.
    pub fn new_owning(inner: Box<dyn DatasetWriter>, handle: Py<PyAny>) -> Self {
        Self {
            inner: Some(inner),
            owned_handles: vec![handle],
        }
    }

    /// Creates a new PyDatasetWriter that owns a set of Python file handles.
    ///
    /// Use this for a multi-path remote R-set write: the library opened one handle
    /// per remote key (base + each overview). `close()` finalizes `inner` first
    /// (the composite writer flushes every sub-writer's bytes into its handle),
    /// then closes each handle so every object-store upload commits. See
    /// [`PyDatasetWriter::close`] for the ordering and error-handling rationale.
    pub fn new_owning_many(inner: Box<dyn DatasetWriter>, handles: Vec<Py<PyAny>>) -> Self {
        Self {
            inner: Some(inner),
            owned_handles: handles,
        }
    }

    /// Returns a mutable reference to the inner DatasetWriter, if available.
    fn get_inner_mut(&mut self) -> PyResult<&mut Box<dyn DatasetWriter>> {
        self.inner.as_mut().ok_or_else(|| {
            CodecError::Io(std::io::Error::other("DatasetWriter has been closed")).into()
        })
    }
}

#[pymethods]
impl PyDatasetWriter {
    /// Add an asset to the dataset.
    ///
    /// Each asset is identified by a unique key and backed by an
    /// :class:`AssetProvider` (or any subtype such as
    /// :class:`BufferedImageAssetProvider`).
    ///
    /// :param key: Unique string identifier for the asset.
    /// :type key: str
    /// :param provider: The asset data to add.
    /// :type provider: AssetProvider | ImageAssetProvider | BufferedImageAssetProvider
    /// :param title: Human-readable title for the asset.
    /// :type title: str
    /// :param description: Detailed description of the asset.
    /// :type description: str
    /// :param roles: Semantic roles (e.g., ``"data"``, ``"thumbnail"``).
    /// :type roles: list[str]
    /// :raises ValueError: If an asset with the given key already exists.
    /// :raises TypeError: If *provider* is not a valid asset provider type.
    ///
    /// Example:
    ///
    /// ```python
    /// writer.add_asset(
    ///     "image:0", image_provider,
    ///     "Primary Image", "RGB scene", ["data"],
    /// )
    /// ```
    #[pyo3(signature = (key, provider, title, description, roles))]
    fn add_asset(
        &mut self,
        _py: Python<'_>,
        key: &str,
        provider: &Bound<'_, PyAny>,
        title: &str,
        description: &str,
        roles: Vec<String>,
    ) -> PyResult<()> {
        let inner = self.get_inner_mut()?;

        // Extract the AssetProvider enum from whichever Python type was passed
        let enum_provider = if let Ok(img) = provider.extract::<PyRef<PyImageAssetProvider>>() {
            AssetProvider::Image(img.inner().clone())
        } else if let Ok(buf) = provider.extract::<PyRef<PyBufferedImageAssetProvider>>() {
            AssetProvider::Image(buf.inner().clone())
        } else if let Ok(txt) = provider.extract::<PyRef<PyTextAssetProvider>>() {
            AssetProvider::Text(txt.inner().clone())
        } else if let Ok(buf_txt) = provider.extract::<PyRef<PyBufferedTextAssetProvider>>() {
            AssetProvider::Text(buf_txt.as_text_provider())
        } else if let Ok(data) = provider.extract::<PyRef<PyDataAssetProvider>>() {
            AssetProvider::Data(data.inner().clone())
        } else if let Ok(buf_data) = provider.extract::<PyRef<PyBufferedDataAssetProvider>>() {
            AssetProvider::Data(buf_data.as_data_provider())
        } else if let Ok(gfx) = provider.extract::<PyRef<PyGraphicsAssetProvider>>() {
            AssetProvider::Graphics(gfx.inner().clone())
        } else if let Ok(asset) = provider.extract::<PyRef<PyAssetProvider>>() {
            // PyAssetProvider wraps an AssetProvider enum internally
            asset.inner().clone()
        } else if is_duck_typed_image_provider(provider) {
            // Duck-typing fallback for Python-defined image providers
            let adapter = PyCallbackImageAssetProvider::new(_py, provider)?;
            AssetProvider::Image(Arc::new(adapter))
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "provider must be an AssetProvider, ImageAssetProvider, \
                 BufferedImageAssetProvider, TextAssetProvider, \
                 BufferedTextAssetProvider, DataAssetProvider, \
                 BufferedDataAssetProvider, GraphicsAssetProvider, \
                 or a duck-typed image provider with required methods \
                 (get_block, num_rows, etc.)",
            ));
        };

        inner.add_asset(key, enum_provider, title, description, &roles)?;
        Ok(())
    }

    /// Set the dataset-level metadata.
    ///
    /// Assign a :class:`MetadataProvider` (or :class:`BufferedMetadataProvider`)
    /// containing file-level fields for the output file. For NITF, this populates
    /// the file header (security markings, originator, etc.). For TIFF, this has
    /// no effect — IFD tags and encoding hints are sourced from each asset
    /// provider's metadata instead.
    ///
    /// :raises IOError: If the metadata cannot be applied to the dataset.
    #[setter]
    fn metadata(&mut self, metadata: &PyMetadataProvider) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        let metadata_provider = Arc::clone(metadata.inner());
        inner.set_metadata(metadata_provider)?;
        Ok(())
    }

    /// Flush pending data and release all resources.
    ///
    /// After calling this method the writer should not be used. When using
    /// the context manager (``with`` statement), ``close`` is called
    /// automatically on exit.
    ///
    /// For a library-opened remote destination this also closes the retained
    /// Python file handle(s), committing the object-store upload(s). The format
    /// writer is finalized **first** (each final ``flush`` must land before the
    /// corresponding handle closes and the upload commits), then every handle is
    /// closed and any error surfaced as a ``PyErr`` — an upload-commit failure is a
    /// real write failure, which is why this lives in the explicit
    /// ``close``/``__exit__`` path rather than a ``Drop`` impl that could not report
    /// it. There may be one handle (single-path write) or several (a multi-path
    /// R-set pyramid, one per remote key). All steps run even if an earlier one
    /// fails, so no partial handle is left dangling; the first error wins.
    /// Idempotent: a second ``close`` is a no-op (the taken ``inner`` and drained
    /// handles leave nothing to do).
    ///
    /// :raises IOError: If flushing data to storage fails.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        let finalize = self
            .inner
            .take()
            .map(|mut inner| inner.close())
            .transpose()
            .map(|_| ());

        // Close every library-owned handle regardless of whether finalize or an
        // earlier handle close failed, so no partial upload is left dangling;
        // report the first error encountered (finalize wins over handle close).
        let mut first_error: PyResult<()> = Ok(());
        for handle in std::mem::take(&mut self.owned_handles) {
            if let Err(e) = handle.bind(py).call_method0("close") {
                if first_error.is_ok() {
                    first_error = Err(e);
                }
            }
        }

        finalize?;
        first_error
    }

    /// Context manager entry point.
    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Enable or disable strict encoding validation for metadata fields.
    ///
    /// When *strict* is ``True``, numeric TRE fields are validated against
    /// their exact declared encoding (e.g. BCS-NPI rejects ``+``, ``-``,
    /// ``.``). When ``False`` (the default), numeric fields accept any
    /// printable ASCII, tolerating real-world deviations from the spec.
    ///
    /// :param strict: Whether to enforce strict encoding validation.
    /// :type strict: bool
    ///
    /// Example:
    ///
    /// ```python
    /// with IO.open("output.ntf", "w", "nitf") as writer:
    ///     writer.strict_encoding = True  # enforce spec-exact validation
    /// ```
    #[setter]
    fn strict_encoding(&mut self, strict: bool) -> PyResult<()> {
        let inner = self.get_inner_mut()?;
        inner.set_strict_encoding(strict);
        Ok(())
    }

    /// Context manager exit point.
    ///
    /// Automatically closes the writer when exiting the context.
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        py: Python<'_>,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        self.close(py)?;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use crate::bindings::stream::PyWriteStream;

    /// A minimal [`DatasetWriter`] stub that mimics the real format writers'
    /// finalize behavior: it buffers nothing and, on [`close`](DatasetWriter::close),
    /// performs one `write_all` + `flush` through the held `Box<dyn Write + Send>`
    /// sink — exactly the shape `PyDatasetWriter::close` orders before the handle
    /// close. Constructed over a `PyWriteStream` in tests so the writes/flush cross
    /// into a recording Python handle.
    struct StubWriter {
        output: std::sync::Mutex<Option<Box<dyn Write + Send>>>,
    }

    impl DatasetWriter for StubWriter {
        fn add_asset(
            &mut self,
            _key: &str,
            _provider: AssetProvider,
            _title: &str,
            _description: &str,
            _roles: &[String],
        ) -> Result<(), CodecError> {
            Ok(())
        }

        fn set_metadata(
            &mut self,
            _metadata: Arc<dyn crate::traits::MetadataProvider>,
        ) -> Result<(), CodecError> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), CodecError> {
            if let Some(mut output) = self.output.lock().unwrap().take() {
                output.write_all(b"payload").map_err(CodecError::Io)?;
                output.flush().map_err(CodecError::Io)?;
            }
            Ok(())
        }
    }

    /// Builds a Python object that records the order of `write`/`flush`/`close`
    /// calls into a `calls` list attribute, extending the recording-stub style in
    /// `stream.rs` tests.
    fn recording_handle(py: Python<'_>) -> Py<PyAny> {
        let code = "\
class RecordingHandle:
    def __init__(self):
        self.calls = []
        self.closed = False
    def write(self, data):
        self.calls.append('write')
        return len(data)
    def flush(self):
        self.calls.append('flush')
    def close(self):
        self.calls.append('close')
        self.closed = True
";
        let globals = pyo3::types::PyDict::new(py);
        py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
            .unwrap();
        globals
            .get_item("RecordingHandle")
            .unwrap()
            .unwrap()
            .call0()
            .unwrap()
            .unbind()
    }

    /// An owned-handle writer finalizes the format writer (write + flush) before
    /// closing the Python handle: the recorded call order is
    /// `write`, `flush`, `close`, and the handle ends up closed.
    #[test]
    fn owned_handle_closed_after_finalize() {
        Python::attach(|py| {
            let handle = recording_handle(py);
            let sink: Box<dyn Write + Send> = Box::new(PyWriteStream::new(handle.clone_ref(py)));
            let inner: Box<dyn DatasetWriter> = Box::new(StubWriter { output: std::sync::Mutex::new(Some(sink)) });

            let mut writer = PyDatasetWriter::new_owning(inner, handle.clone_ref(py));
            writer.close(py).unwrap();

            let bound = handle.bind(py);
            let calls: Vec<String> = bound.getattr("calls").unwrap().extract().unwrap();
            assert_eq!(calls, vec!["write", "flush", "close"]);
            let closed: bool = bound.getattr("closed").unwrap().extract().unwrap();
            assert!(closed, "owned handle should be closed after finalize");

            // Idempotent: a second close is a no-op (nothing left to close).
            writer.close(py).unwrap();
            let calls: Vec<String> = bound.getattr("calls").unwrap().extract().unwrap();
            assert_eq!(calls, vec!["write", "flush", "close"]);
        });
    }

    /// Builds a Python handle whose `close()` raises, recording the attempt in a
    /// module-level `closed_attempts` list so a test can prove every handle was
    /// still closed even after an earlier one failed.
    fn raising_close_handle(py: Python<'_>, tag: &str, log: &Py<PyAny>) -> Py<PyAny> {
        let code = "\
class RaisingCloseHandle:
    def __init__(self, tag, log):
        self.tag = tag
        self.log = log
    def write(self, data):
        return len(data)
    def flush(self):
        pass
    def close(self):
        self.log.append(self.tag)
        raise IOError('commit failed: ' + self.tag)
";
        let globals = pyo3::types::PyDict::new(py);
        py.run(&std::ffi::CString::new(code).unwrap(), Some(&globals), None)
            .unwrap();
        globals
            .get_item("RaisingCloseHandle")
            .unwrap()
            .unwrap()
            .call1((tag, log.bind(py)))
            .unwrap()
            .unbind()
    }

    /// A multi-handle writer (multi-path remote R-set) closes **every** owned
    /// handle after finalize, in order, and each ends up closed.
    #[test]
    fn many_handles_all_closed_after_finalize() {
        Python::attach(|py| {
            let h0 = recording_handle(py);
            let h1 = recording_handle(py);
            // The format writer's sink is the base handle; the overview handle is
            // owned for commit but not written by this stub (mirrors the composite
            // finalizing each sub-writer's own sink in the real path).
            let sink: Box<dyn Write + Send> = Box::new(PyWriteStream::new(h0.clone_ref(py)));
            let inner: Box<dyn DatasetWriter> =
                Box::new(StubWriter { output: std::sync::Mutex::new(Some(sink)) });

            let mut writer = PyDatasetWriter::new_owning_many(
                inner,
                vec![h0.clone_ref(py), h1.clone_ref(py)],
            );
            writer.close(py).unwrap();

            for h in [&h0, &h1] {
                let closed: bool = h.bind(py).getattr("closed").unwrap().extract().unwrap();
                assert!(closed, "every owned handle should be closed after finalize");
            }
        });
    }

    /// When several owned handles fail to close, all are still attempted and the
    /// first error is the one surfaced.
    #[test]
    fn many_handles_all_attempted_first_error_wins() {
        Python::attach(|py| {
            let log: Py<PyAny> = pyo3::types::PyList::empty(py).into_any().unbind();
            let h0 = raising_close_handle(py, "h0", &log);
            let h1 = raising_close_handle(py, "h1", &log);
            let sink: Box<dyn Write + Send> = Box::new(PyWriteStream::new(h0.clone_ref(py)));
            let inner: Box<dyn DatasetWriter> =
                Box::new(StubWriter { output: std::sync::Mutex::new(Some(sink)) });

            let mut writer =
                PyDatasetWriter::new_owning_many(inner, vec![h0.clone_ref(py), h1.clone_ref(py)]);
            let err = writer.close(py).unwrap_err();

            // Both handles were attempted despite the first raising.
            let attempts: Vec<String> = log.bind(py).extract().unwrap();
            assert_eq!(attempts, vec!["h0", "h1"]);
            // The first error (h0) wins.
            assert!(
                err.to_string().contains("h0"),
                "first close error should win, got: {err}"
            );
        });
    }

    /// A `None`-handle writer (local file / caller-supplied stream) finalizes the
    /// format writer but never triggers a handle `close`.
    #[test]
    fn none_handle_is_not_closed() {
        Python::attach(|py| {
            let handle = recording_handle(py);
            let sink: Box<dyn Write + Send> = Box::new(PyWriteStream::new(handle.clone_ref(py)));
            let inner: Box<dyn DatasetWriter> = Box::new(StubWriter { output: std::sync::Mutex::new(Some(sink)) });

            // `new` (not `new_owning`) — the writer owns no handle.
            let mut writer = PyDatasetWriter::new(inner);
            writer.close(py).unwrap();

            let bound = handle.bind(py);
            let calls: Vec<String> = bound.getattr("calls").unwrap().extract().unwrap();
            assert_eq!(calls, vec!["write", "flush"]);
            let closed: bool = bound.getattr("closed").unwrap().extract().unwrap();
            assert!(!closed, "a None-handle writer must not close the stream");
        });
    }
}
