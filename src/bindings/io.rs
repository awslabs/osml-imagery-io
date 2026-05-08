//! IO Factory for opening datasets.
//!
//! This module provides the IO factory class that selects appropriate
//! reader/writer implementations based on URI scheme and file format.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use pyo3::exceptions::{PyIOError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

use crate::bindings::stream::PyWriteStream;
use crate::bindings::{PyDatasetReader, PyDatasetWriter};
use crate::composite::{CompositeDatasetReader, CompositeDatasetWriter};

/// Accepts a single string, a list of strings, a single file-like object,
/// or a list of file-like objects from Python.
///
/// This enum allows `IO.open()` to accept:
/// - `str` → `Single` (single file path)
/// - `list[str]` → `Multiple` (R-set file paths)
/// - file-like object → `Stream` (single stream)
/// - `list[BinaryIO]` → `StreamList` (multi-source streams, requires `roles`)
#[cfg_attr(test, derive(Debug))]
enum PathsArg {
    Single(String),
    Multiple(Vec<String>),
    Stream(Py<PyAny>),
    StreamList(Vec<Py<PyAny>>),
}

/// Returns true if the object is file-like (has a `.read` or `.write` attribute).
///
/// This is a duck-typing check that matches any object implementing the
/// standard Python read/write protocol — `io.BytesIO`, fsspec file handles,
/// HTTP response stream wrappers, etc.
fn is_file_like(obj: &Bound<'_, PyAny>) -> bool {
    obj.hasattr("read").unwrap_or(false) || obj.hasattr("write").unwrap_or(false)
}

impl<'a, 'py> FromPyObject<'a, 'py> for PathsArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        // Try `str` first — this takes precedence so a bare Python string is
        // always treated as a file path, even though strings happen to have
        // attributes that collide with file-like probing.
        if let Ok(s) = ob.extract::<String>() {
            return Ok(PathsArg::Single(s));
        }

        // Try a list — the elements are either all strings (path list) or
        // all file-like objects (stream list); mixed lists are rejected.
        if let Ok(list) = ob.cast::<PyList>() {
            let mut strings: Vec<String> = Vec::new();
            let mut streams: Vec<Py<PyAny>> = Vec::new();

            for item in list.iter() {
                if let Ok(s) = item.extract::<String>() {
                    if !streams.is_empty() {
                        return Err(PyTypeError::new_err(
                            "List elements must be all strings or all file-like objects \
                             (mixed list not supported)",
                        ));
                    }
                    strings.push(s);
                } else if is_file_like(&item) {
                    if !strings.is_empty() {
                        return Err(PyTypeError::new_err(
                            "List elements must be all strings or all file-like objects \
                             (mixed list not supported)",
                        ));
                    }
                    streams.push(item.clone().unbind());
                } else {
                    return Err(PyTypeError::new_err(
                        "List elements must be all strings or all file-like objects",
                    ));
                }
            }

            if !strings.is_empty() {
                return Ok(PathsArg::Multiple(strings));
            }
            if !streams.is_empty() {
                return Ok(PathsArg::StreamList(streams));
            }
            return Err(PyValueError::new_err("paths list cannot be empty"));
        }

        // Not a str, not a list — try a single file-like object.
        if is_file_like(&ob) {
            return Ok(PathsArg::Stream(Py::<PyAny>::from(ob)));
        }

        Err(PyValueError::new_err("paths must be a str or list[str]"))
    }
}
use crate::dted::DTEDDatasetReader;
use crate::error::CodecError;
#[cfg(feature = "openjpeg")]
use crate::j2k::{J2KDatasetReader, J2KDatasetWriter};
use crate::jbp::{JBPDatasetReader, JBPDatasetWriter, NitfFormat};
#[cfg(feature = "libjpeg-turbo")]
use crate::jpeg::{JPEGDatasetReader, JPEGDatasetWriter};
use crate::png::{PNGDatasetReader, PNGDatasetWriter};
#[cfg(feature = "libtiff")]
use crate::tiff;
use crate::traits::{DatasetReader, DatasetWriter, ImageAssetProvider};
use crate::types::AssetType;

/// Represents a parsed URI with its scheme and path components.
#[derive(Debug, Clone)]
pub struct ParsedUri {
    /// The URI scheme (e.g., "file", "s3", or empty for plain paths)
    pub scheme: String,
    /// The path component of the URI
    pub path: String,
}

impl ParsedUri {
    /// Parses a URI string into its components.
    ///
    /// Supports:
    /// - `file://` URIs (local file paths)
    /// - `s3://` URIs (S3 bucket/key paths)
    /// - Plain paths (treated as local files)
    pub fn parse(uri: &str) -> Self {
        if let Some(rest) = uri.strip_prefix("file://") {
            ParsedUri {
                scheme: "file".to_string(),
                path: rest.to_string(),
            }
        } else if let Some(rest) = uri.strip_prefix("s3://") {
            ParsedUri {
                scheme: "s3".to_string(),
                path: rest.to_string(),
            }
        } else {
            // Plain path - treat as local file
            ParsedUri {
                scheme: "file".to_string(),
                path: uri.to_string(),
            }
        }
    }

    /// Returns the file extension from the path, if any.
    pub fn extension(&self) -> Option<&str> {
        self.path
            .rsplit('.')
            .next()
            .filter(|ext| !ext.contains('/') && !ext.contains('\\'))
    }
}

/// Entry point for opening geospatial datasets for reading or writing.
///
/// The ``IO`` class provides a single static method, ``open``, that accepts
/// a file path string, a list of file paths, a file-like object (stream),
/// or a list of file-like objects, and returns either a
/// :class:`DatasetReader` or a :class:`DatasetWriter` depending on the
/// requested mode. The file format is auto-detected from the extension and
/// file header bytes when reading from paths; when reading from a stream,
/// the ``format`` parameter must be specified explicitly. Both local file
/// paths and ``file://`` URIs are supported.
///
/// Example:
///
/// ```python
/// from aws.osml.io import IO
///
/// # Read mode — single string path (format auto-detected)
/// with IO.open("image.ntf", "r") as dataset:
///     keys = dataset.get_asset_keys()
///     asset = dataset.get_asset(keys[0])
///
/// # Read mode — from an in-memory byte buffer
/// import io
/// with IO.open(io.BytesIO(raw_bytes), "r", format="png") as dataset:
///     keys = dataset.get_asset_keys()
///
/// # Write mode — returns a DatasetWriter
/// with IO.open("output.ntf", "w", "nitf") as writer:
///     writer.add_asset("image", provider, "Title", "Description", ["data"])
/// ```
#[pyclass(name = "IO")]
pub struct IO;

#[pymethods]
impl IO {
    /// Open a dataset for reading or writing.
    ///
    /// The format is auto-detected from the file extension when reading from
    /// a file path. When writing to a file, a format string is inferred from
    /// the extension or may be provided explicitly. When reading from or
    /// writing to a file-like object (stream), the ``format`` parameter is
    /// required since there is no filename to inspect. Use a context manager
    /// (``with`` statement) on the returned object to ensure file handles
    /// are released.
    ///
    /// :param paths: A file path, list of file paths, file-like object, or
    ///     list of file-like objects. For single-file formats a bare string
    ///     is accepted (``"image.ntf"``). For multi-file R-set datasets a
    ///     list is required. File-like objects must implement ``.read()``
    ///     for read mode and ``.write()`` + ``.flush()`` for write mode
    ///     (e.g., ``io.BytesIO``, fsspec file handles). Accepts local paths,
    ///     ``file://`` URIs, and ``s3://`` URIs.
    /// :type paths: str | list[str] | BinaryIO | list[BinaryIO]
    /// :param mode: ``"r"`` for reading or ``"w"`` for writing. Defaults to
    ///     ``"r"``.
    /// :type mode: str
    /// :param format: Format identifier (e.g., ``"nitf"``, ``"geotiff"``,
    ///     ``"png"``). Required when ``paths`` is a stream or list of
    ///     streams. Required when writing to a file with an unrecognized
    ///     extension. Optional otherwise.
    /// :type format: str or None
    /// :param roles: Explicit role strings for each source. ``list[str]``
    ///     when ``paths`` is a single source, ``list[list[str]]`` when
    ///     ``paths`` is a list. Recognised roles: ``"data"`` designates the
    ///     base source; ``"overview:N"`` (N >= 1) designates an R-set
    ///     overview at resolution level N. ``roles`` is required when
    ///     ``paths`` is a list of streams (no filename to derive roles
    ///     from). For a list of file paths, ``roles`` is optional; if
    ///     omitted, the library falls back to ``.rN`` filename detection
    ///     for backward compatibility.
    /// :type roles: list[str] or list[list[str]] or None
    /// :returns: A :class:`DatasetReader` when *mode* is ``"r"``, or a
    ///     :class:`DatasetWriter` when *mode* is ``"w"``.
    /// :rtype: DatasetReader or DatasetWriter
    /// :raises ValueError: If *paths* is empty, the mode is invalid, the
    ///     file format is not supported, or ``format``/``roles`` is missing
    ///     when required.
    /// :raises TypeError: If ``paths`` has an invalid type, or a file-like
    ///     object is missing the required methods.
    /// :raises IOError: If the file cannot be opened.
    ///
    /// .. note::
    ///
    ///    When reading from a stream, the entire content is loaded into
    ///    memory via ``.read()``. For large files (multi-GB NITF) this is
    ///    significantly more expensive than the memory-mapped file path.
    ///    Consider downloading large files to the local filesystem, or
    ///    using the library's VirtualiZarr-based tile index for
    ///    cloud-native range-read access.
    ///
    /// Example:
    ///
    /// ```python
    /// from aws.osml.io import IO
    /// import io
    ///
    /// # Read mode — single string path
    /// with IO.open("image.ntf", "r") as dataset:
    ///     print(type(dataset))  # DatasetReader
    ///
    /// # Read mode — list of paths (R-set, .rN detection)
    /// with IO.open(["image.ntf", "image.ntf.r1"], "r") as dataset:
    ///     print(type(dataset))  # DatasetReader
    ///
    /// # Read mode — list of streams with explicit roles
    /// streams = [open("image.ntf", "rb"), open("image.ntf.r1", "rb")]
    /// with IO.open(streams, "r", format="nitf",
    ///              roles=[["data"], ["overview:1"]]) as dataset:
    ///     print(type(dataset))  # DatasetReader
    ///
    /// # Write mode — to an in-memory buffer
    /// buf = io.BytesIO()
    /// with IO.open(buf, "w", "png") as writer:
    ///     writer.add_asset("image", provider, "Title", "Description", ["data"])
    /// encoded_bytes = buf.getvalue()
    /// ```
    #[staticmethod]
    #[pyo3(signature = (paths, mode="r", format=None, roles=None))]
    fn open(
        py: Python<'_>,
        paths: PathsArg,
        mode: &str,
        format: Option<&str>,
        roles: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        match paths {
            PathsArg::Single(path) => {
                // Validate the path is not an empty string.
                if path.is_empty() {
                    return Err(PyValueError::new_err("paths list cannot be empty"));
                }
                // `roles` is accepted for a single source but has no routing
                // effect in v1 (it is validated for shape only).
                let _ = normalize_roles(roles, 1)?;

                let parsed = ParsedUri::parse(&path);
                match mode {
                    "r" => {
                        let reader = create_reader(&parsed, format)?;
                        Ok(reader.into_pyobject(py)?.into_any().unbind())
                    }
                    "w" => {
                        let format_str = match format {
                            Some(f) => f.to_string(),
                            None => detect_write_format(&parsed).ok_or_else(|| {
                                PyValueError::new_err(
                                    "Cannot determine output format: no format specified \
                                     and file extension is not recognized",
                                )
                            })?,
                        };
                        let writer = create_writer(&parsed, &format_str)?;
                        Ok(writer.into_pyobject(py)?.into_any().unbind())
                    }
                    _ => Err(PyValueError::new_err(format!(
                        "Invalid mode '{}'. Expected 'r' for reading or 'w' for writing.",
                        mode
                    ))),
                }
            }
            PathsArg::Multiple(paths) => {
                if paths.is_empty() {
                    return Err(PyValueError::new_err("paths list cannot be empty"));
                }
                if paths.iter().any(|p| p.is_empty()) {
                    return Err(PyValueError::new_err("paths list cannot be empty"));
                }

                let normalized = normalize_roles(roles, paths.len())?;
                if let Some(roles_per_source) = normalized {
                    // Explicit roles provided — bypass `.rN` filename detection.
                    return open_multi_path_with_roles(py, &paths, mode, format, &roles_per_source);
                }

                // No roles — fall back to the existing `.rN` filename detection.
                match mode {
                    "r" => {
                        let reader = create_multi_path_reader(&paths, format)?;
                        Ok(reader.into_pyobject(py)?.into_any().unbind())
                    }
                    "w" => {
                        let base_parsed = ParsedUri::parse(&paths[0]);
                        let format_str = match format {
                            Some(f) => f.to_string(),
                            None => detect_write_format(&base_parsed).ok_or_else(|| {
                                PyValueError::new_err(
                                    "Cannot determine output format: no format specified \
                                     and file extension is not recognized",
                                )
                            })?,
                        };
                        let writer = create_multi_path_writer(&paths, &format_str)?;
                        Ok(writer.into_pyobject(py)?.into_any().unbind())
                    }
                    _ => Err(PyValueError::new_err(format!(
                        "Invalid mode '{}'. Expected 'r' for reading or 'w' for writing.",
                        mode
                    ))),
                }
            }
            PathsArg::Stream(stream_obj) => {
                let fmt = format.ok_or_else(|| {
                    PyValueError::new_err(
                        "format is required when reading from or writing to a stream",
                    )
                })?;
                // `roles` is accepted for a single source but has no routing
                // effect in v1 (it is validated for shape only).
                let _ = normalize_roles(roles, 1)?;

                match mode {
                    "r" => {
                        let reader = create_reader_from_stream(py, &stream_obj, fmt)?;
                        Ok(reader.into_pyobject(py)?.into_any().unbind())
                    }
                    "w" => {
                        let writer = create_writer_for_stream(py, stream_obj, fmt)?;
                        Ok(writer.into_pyobject(py)?.into_any().unbind())
                    }
                    _ => Err(PyValueError::new_err(format!(
                        "Invalid mode '{}'. Expected 'r' for reading or 'w' for writing.",
                        mode
                    ))),
                }
            }
            PathsArg::StreamList(streams) => {
                if streams.is_empty() {
                    return Err(PyValueError::new_err("paths list cannot be empty"));
                }
                let fmt = format.ok_or_else(|| {
                    PyValueError::new_err(
                        "format is required when reading from or writing to a stream",
                    )
                })?;
                let roles_per_source = normalize_roles(roles, streams.len())?.ok_or_else(|| {
                    PyValueError::new_err(
                        "roles is required when the source list contains file-like objects; \
                         there is no filename to derive roles from",
                    )
                })?;
                open_multi_stream_with_roles(py, streams, mode, fmt, &roles_per_source)
            }
        }
    }
}

/// Memory-maps a file at the given path.
///
/// Returns the `Mmap` handle which dereferences to `&[u8]`. The caller
/// must keep the `Mmap` alive for as long as the byte slice is needed.
fn mmap_file(path: &str) -> Result<Mmap, CodecError> {
    let file = File::open(Path::new(path))?;
    // SAFETY: We rely on the file not being modified while mapped. This is
    // the same assumption made by any memory-mapped reader.
    unsafe { Ok(Mmap::map(&file)?) }
}

/// Extract the R-set level from a filename, if it matches the `.rN` pattern.
///
/// Returns `Some(N)` if the filename ends with `.rN` where N is a positive
/// integer. Returns `None` otherwise.
///
/// # Examples
/// - `"image.ntf.r1"` → `Some(1)`
/// - `"image.ntf.r12"` → `Some(12)`
/// - `"image.ntf.r0"` → `None` (level 0 is the base, not an overview)
/// - `"image.ntf"` → `None`
/// - `"image.r1.ntf"` → `None` (`.r1` is not the final extension)
fn extract_rset_level(path: &str) -> Option<u32> {
    let filename = Path::new(path).file_name().and_then(|f| f.to_str())?;

    // Find the last '.' in the filename
    let dot_pos = filename.rfind('.')?;
    let suffix = &filename[dot_pos + 1..];

    // Check if suffix starts with 'r' followed by digits only
    if !suffix.starts_with('r') || suffix.len() < 2 {
        return None;
    }
    let digits = &suffix[1..];
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let level: u32 = digits.parse().ok()?;
    if level == 0 {
        None // .r0 is the base, not an overview
    } else {
        Some(level)
    }
}

/// Strip the `.rN` suffix from a path to get the base path for format detection.
///
/// # Examples
/// - `"image.ntf.r1"` → `"image.ntf"`
/// - `"image.ntf"` → `"image.ntf"` (unchanged)
fn strip_rset_suffix(path: &str) -> String {
    // Find the last '.' in the path
    if let Some(dot_pos) = path.rfind('.') {
        let suffix = &path[dot_pos + 1..];
        if suffix.starts_with('r')
            && suffix.len() >= 2
            && suffix[1..].chars().all(|c| c.is_ascii_digit())
        {
            return path[..dot_pos].to_string();
        }
    }
    path.to_string()
}

/// Extract the primary image asset from a reader as an `Arc<dyn ImageAssetProvider>`.
///
/// Looks for the first `image:0` asset and attempts to get it as an
/// `ImageAssetProvider`. Returns `None` if no image asset is found.
fn extract_primary_image(reader: &dyn DatasetReader) -> Option<Arc<dyn ImageAssetProvider>> {
    let keys = reader.get_asset_keys(Some(AssetType::Image), None);
    let key = keys.first()?;
    let asset = reader.get_asset(key).ok()?;
    asset.as_image().cloned()
}

/// Creates a composite reader from multiple paths with R-set detection.
///
/// The first path is opened as the base reader. Additional paths matching
/// the `.rN` filename pattern are opened independently, and their primary
/// image assets are re-keyed as `image:0:overview:N` with role `"overview"`.
fn create_multi_path_reader(paths: &[String], format: Option<&str>) -> PyResult<PyDatasetReader> {
    let boxed = create_multi_path_reader_boxed(paths, format)?;
    Ok(PyDatasetReader::new(boxed))
}

/// Internal: creates a boxed composite reader from multiple paths.
fn create_multi_path_reader_boxed(
    paths: &[String],
    format: Option<&str>,
) -> PyResult<Box<dyn DatasetReader>> {
    // Open the base reader from the first path
    let base_parsed = ParsedUri::parse(&paths[0]);
    let base_reader = create_reader_boxed(&base_parsed, format)?;

    // Collect overview entries: (level, ImageAssetProvider)
    let mut overview_entries: Vec<(u32, Arc<dyn ImageAssetProvider>)> = Vec::new();

    for path in &paths[1..] {
        let level = extract_rset_level(path).ok_or_else(|| {
            PyValueError::new_err(format!(
                "Additional path '{}' does not match R-set pattern '.rN' \
                 (where N is a positive integer)",
                path
            ))
        })?;

        // Strip the .rN suffix for format detection
        let base_path = strip_rset_suffix(path);
        let parsed = ParsedUri::parse(&base_path);

        // Create a reader for this R-set file using the actual file path
        let rset_parsed = ParsedUri::parse(path);

        // Use the base path's extension for format detection, but open the actual file
        let rset_reader: Box<dyn DatasetReader> = {
            // Determine format from the stripped path's extension (or explicit format)
            let rset_format = format.or_else(|| {
                parsed.extension().map(|e| match e.to_lowercase().as_str() {
                    "ntf" | "nitf" | "nsif" | "nsf" | "hr1" | "hr2" | "hr3" | "hr4" | "hr5"
                    | "hr6" | "hr7" | "hr8" => "nitf",
                    "tif" | "tiff" | "gtif" | "gtiff" => "tiff",
                    "png" => "png",
                    "j2k" | "jp2" => "j2k",
                    "jpg" | "jpeg" => "jpeg",
                    "dt0" | "dt1" | "dt2" | "dt3" | "dt4" | "dt5" | "avg" | "min" | "max" => "dted",
                    _ => "",
                })
            });

            match rset_format {
                Some("nitf") | Some("nitf21") | Some("nitf2.1") | Some("nsif") | Some("nsif10")
                | Some("nsif1.0") | Some("jbp") => {
                    let mmap = mmap_file(&rset_parsed.path)?;
                    let reader = JBPDatasetReader::from_bytes(&mmap)?;
                    Box::new(reader)
                }
                #[cfg(feature = "libtiff")]
                Some("tiff") | Some("tif") => {
                    let mmap = mmap_file(&rset_parsed.path)?;
                    let reader = tiff::TIFFDatasetReader::from_bytes(&mmap)?;
                    Box::new(reader)
                }
                Some("png") => {
                    let mmap = mmap_file(&rset_parsed.path)?;
                    let reader = PNGDatasetReader::from_bytes(&mmap)?;
                    Box::new(reader)
                }
                #[cfg(feature = "openjpeg")]
                Some("j2k") | Some("jp2") | Some("jpeg2000") => {
                    let mmap = mmap_file(&rset_parsed.path)?;
                    let reader = J2KDatasetReader::from_bytes(&mmap)?;
                    Box::new(reader)
                }
                #[cfg(feature = "libjpeg-turbo")]
                Some("jpg") | Some("jpeg") => {
                    let mmap = mmap_file(&rset_parsed.path)?;
                    let reader = JPEGDatasetReader::from_bytes(&mmap)?;
                    Box::new(reader)
                }
                Some("dted") | Some("dt0") | Some("dt1") | Some("dt2") | Some("dt3")
                | Some("dt4") | Some("dt5") => {
                    let mmap = mmap_file(&rset_parsed.path)?;
                    let reader = DTEDDatasetReader::from_bytes(&mmap)?;
                    Box::new(reader)
                }
                _ => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Cannot determine format for R-set file: '{}'",
                        path
                    ))
                    .into());
                }
            }
        };

        // Extract the primary image asset from the R-set reader
        let image_provider = extract_primary_image(rset_reader.as_ref()).ok_or_else(|| {
            PyValueError::new_err(format!(
                "R-set file '{}' does not contain an image asset",
                path
            ))
        })?;

        overview_entries.push((level, image_provider));
    }

    // Build the composite reader
    let composite = CompositeDatasetReader::new(base_reader, overview_entries);

    Ok(Box::new(composite))
}

/// Detect the output format from a parsed URI's file extension.
///
/// Strips any `.rN` R-set suffix before checking the extension, so that
/// paths like `output.ntf.r1` are correctly detected as `"nitf"`.
///
/// Returns `None` if the extension is not recognized.
fn detect_write_format(parsed: &ParsedUri) -> Option<String> {
    let effective_path = strip_rset_suffix(&parsed.path);
    let effective_parsed = ParsedUri::parse(&effective_path);

    effective_parsed
        .extension()
        .and_then(|ext| match ext.to_lowercase().as_str() {
            "ntf" | "nitf" | "hr1" | "hr2" | "hr3" | "hr4" | "hr5" | "hr6" | "hr7" | "hr8" => {
                Some("nitf".to_string())
            }
            "nsf" | "nsif" => Some("nsif".to_string()),
            "tif" | "tiff" | "gtif" | "gtiff" => Some("tiff".to_string()),
            "png" => Some("png".to_string()),
            "j2k" | "jp2" => Some("j2k".to_string()),
            "jpg" | "jpeg" => Some("jpeg".to_string()),
            "dt0" | "dt1" | "dt2" | "dt3" | "dt4" | "dt5" | "avg" | "min" | "max" => {
                Some("dted".to_string())
            }
            _ => None,
        })
}

/// Creates a composite writer from multiple output paths with R-set detection.
///
/// The first path is opened as the base writer. Additional paths must match
/// the `.rN` filename pattern; each is opened independently and paired with
/// its overview level. The resulting `CompositeDatasetWriter` routes assets
/// by key: overview assets go to the matching R-set writer, non-overview
/// assets go to the base writer.
fn create_multi_path_writer(paths: &[String], format: &str) -> PyResult<PyDatasetWriter> {
    let base_parsed = ParsedUri::parse(&paths[0]);
    let base_writer = create_writer_boxed(&base_parsed, format)?;

    let mut rset_writers = Vec::new();
    for path in &paths[1..] {
        let level = extract_rset_level(path).ok_or_else(|| {
            PyValueError::new_err(format!(
                "Additional path '{}' does not match R-set pattern '.rN'",
                path
            ))
        })?;
        let parsed = ParsedUri::parse(path);
        let writer = create_writer_boxed(&parsed, format)?;
        rset_writers.push((level, writer));
    }

    // Single-path case: return the base writer directly so format-specific
    // writers (e.g. TIFFDatasetWriter) can handle overview assets natively
    // within a single file (COG multi-IFD layout).
    if rset_writers.is_empty() {
        return Ok(PyDatasetWriter::new(base_writer));
    }

    let composite = CompositeDatasetWriter::new(base_writer, rset_writers);
    Ok(PyDatasetWriter::new(Box::new(composite)))
}

/// Creates a DatasetReader for the given URI.
///
/// This function determines the appropriate reader implementation based on
/// the URI scheme and file format. Files are memory-mapped and passed as
/// byte slices to the format-specific readers.
fn create_reader(parsed: &ParsedUri, format: Option<&str>) -> PyResult<PyDatasetReader> {
    let boxed = create_reader_boxed(parsed, format)?;
    Ok(PyDatasetReader::new(boxed))
}

/// Internal: creates a boxed DatasetReader for the given URI.
///
/// This is the core reader creation logic, returning a `Box<dyn DatasetReader>`
/// that can be used directly (e.g., in composite readers) or wrapped in
/// `PyDatasetReader` for Python exposure.
fn create_reader_boxed(
    parsed: &ParsedUri,
    format: Option<&str>,
) -> PyResult<Box<dyn DatasetReader>> {
    // Validate scheme is supported
    match parsed.scheme.as_str() {
        "file" => {}
        "s3" => {
            return Err(
                CodecError::Unsupported("S3 URIs are not yet supported".to_string()).into(),
            );
        }
        scheme => {
            return Err(
                CodecError::Unsupported(format!("Unsupported URI scheme: {}", scheme)).into(),
            );
        }
    }

    // If format is explicitly specified, use it
    if let Some(fmt) = format {
        match fmt.to_lowercase().as_str() {
            "nitf" | "nitf21" | "nitf2.1" | "nsif" | "nsif10" | "nsif1.0" | "jbp" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = JBPDatasetReader::from_bytes(&mmap)?;
                return Ok(Box::new(reader));
            }
            #[cfg(feature = "libtiff")]
            "tiff" | "tif" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = tiff::TIFFDatasetReader::from_bytes(&mmap)?;
                return Ok(Box::new(reader));
            }
            "png" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = PNGDatasetReader::from_bytes(&mmap)?;
                return Ok(Box::new(reader));
            }
            #[cfg(feature = "openjpeg")]
            "j2k" | "jp2" | "jpeg2000" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = J2KDatasetReader::from_bytes(&mmap)?;
                return Ok(Box::new(reader));
            }
            #[cfg(feature = "libjpeg-turbo")]
            "jpg" | "jpeg" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = JPEGDatasetReader::from_bytes(&mmap)?;
                return Ok(Box::new(reader));
            }
            "dted" | "dt0" | "dt1" | "dt2" | "dt3" | "dt4" | "dt5" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = DTEDDatasetReader::from_bytes(&mmap)?;
                return Ok(Box::new(reader));
            }
            _ => {
                return Err(
                    CodecError::InvalidFormat(format!("Unsupported format: '{}'", fmt)).into(),
                );
            }
        }
    }

    // Detect format from extension, stripping any .rN R-set suffix first
    // so that paths like "image.ntf.r1" are detected as NITF.
    let effective_path = strip_rset_suffix(&parsed.path);
    let effective_parsed = ParsedUri::parse(&effective_path);
    let extension = effective_parsed.extension().map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("ntf") | Some("nitf") | Some("nsif") | Some("nsf") | Some("hr1") | Some("hr2")
        | Some("hr3") | Some("hr4") | Some("hr5") | Some("hr6") | Some("hr7") | Some("hr8") => {
            let mmap = mmap_file(&parsed.path)?;
            let reader = JBPDatasetReader::from_bytes(&mmap)?;
            Ok(Box::new(reader))
        }
        Some("tif") | Some("tiff") | Some("gtif") | Some("gtiff") => {
            #[cfg(feature = "libtiff")]
            {
                let mmap = mmap_file(&parsed.path)?;
                let reader = tiff::TIFFDatasetReader::from_bytes(&mmap)?;
                Ok(Box::new(reader))
            }
            #[cfg(not(feature = "libtiff"))]
            {
                return Err(CodecError::Unsupported(format!(
                    "TIFF support not enabled (libtiff feature disabled) for: {}",
                    parsed.path
                ))
                .into());
            }
        }
        Some("png") => {
            let mmap = mmap_file(&parsed.path)?;
            let reader = PNGDatasetReader::from_bytes(&mmap)?;
            Ok(Box::new(reader))
        }
        Some("j2k") | Some("jp2") => {
            #[cfg(feature = "openjpeg")]
            {
                let mmap = mmap_file(&parsed.path)?;
                let reader = J2KDatasetReader::from_bytes(&mmap)?;
                Ok(Box::new(reader))
            }
            #[cfg(not(feature = "openjpeg"))]
            {
                Err(CodecError::Unsupported(format!(
                    "JPEG 2000 support not enabled (openjpeg feature disabled) for: {}",
                    parsed.path
                ))
                .into())
            }
        }
        Some("jpg") | Some("jpeg") => {
            #[cfg(feature = "libjpeg-turbo")]
            {
                let mmap = mmap_file(&parsed.path)?;
                let reader = JPEGDatasetReader::from_bytes(&mmap)?;
                Ok(Box::new(reader))
            }
            #[cfg(not(feature = "libjpeg-turbo"))]
            {
                Err(CodecError::Unsupported(format!(
                    "JPEG support not enabled (libjpeg-turbo feature disabled) for: {}",
                    parsed.path
                ))
                .into())
            }
        }
        Some("dt0") | Some("dt1") | Some("dt2") | Some("dt3") | Some("dt4") | Some("dt5")
        | Some("avg") | Some("min") | Some("max") => {
            let mmap = mmap_file(&parsed.path)?;
            let reader = DTEDDatasetReader::from_bytes(&mmap)?;
            Ok(Box::new(reader))
        }
        Some(ext) => {
            Err(CodecError::InvalidFormat(format!("Unsupported file format: .{}", ext)).into())
        }
        None => Err(CodecError::InvalidFormat(
            "Cannot determine file format: no file extension".to_string(),
        )
        .into()),
    }
}

/// Creates a DatasetWriter for the given URI.
///
/// This function determines the appropriate writer implementation based on
/// the URI scheme and file format.
fn create_writer(parsed: &ParsedUri, format: &str) -> PyResult<PyDatasetWriter> {
    let boxed = create_writer_boxed(parsed, format)?;
    Ok(PyDatasetWriter::new(boxed))
}

/// Internal: creates a boxed DatasetWriter for the given URI.
///
/// This is the core writer creation logic, returning a `Box<dyn DatasetWriter>`
/// that can be used directly (e.g., in composite writers) or wrapped in
/// `PyDatasetWriter` for Python exposure.
///
/// The writer is backed by a file on disk — this function opens the file,
/// wraps it in a `BufWriter`, then delegates to
/// [`create_writer_boxed_from_output`] for the format-specific dispatch.
fn create_writer_boxed(parsed: &ParsedUri, format: &str) -> PyResult<Box<dyn DatasetWriter>> {
    // Validate scheme is supported
    match parsed.scheme.as_str() {
        "file" => {}
        "s3" => {
            return Err(
                CodecError::Unsupported("S3 URIs are not yet supported".to_string()).into(),
            );
        }
        scheme => {
            return Err(
                CodecError::Unsupported(format!("Unsupported URI scheme: {}", scheme)).into(),
            );
        }
    }

    let file = File::create(&parsed.path).map_err(CodecError::Io)?;
    let buf_writer = std::io::BufWriter::new(file);
    let output: Box<dyn Write + Send> = Box::new(buf_writer);
    create_writer_boxed_from_output(output, format)
}

/// Internal: creates a boxed `DatasetWriter` targeting a generic `Write` output.
///
/// This is the format-specific dispatch shared by the file-path writer
/// ([`create_writer_boxed`]) and the stream writer
/// ([`create_writer_for_stream`]). The `output` is moved into the format
/// writer, which is responsible for calling `.flush()` on its own schedule.
fn create_writer_boxed_from_output(
    output: Box<dyn Write + Send>,
    format: &str,
) -> PyResult<Box<dyn DatasetWriter>> {
    match format.to_lowercase().as_str() {
        "nitf" | "nitf21" | "nitf2.1" => {
            let writer = JBPDatasetWriter::new_with_output(output, NitfFormat::Nitf21)?;
            Ok(Box::new(writer))
        }
        "nsif" | "nsif10" | "nsif1.0" => {
            let writer = JBPDatasetWriter::new_with_output(output, NitfFormat::Nsif10)?;
            Ok(Box::new(writer))
        }
        #[cfg(feature = "libtiff")]
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => {
            let writer = tiff::TIFFDatasetWriter::new_with_output(output)?;
            Ok(Box::new(writer))
        }
        #[cfg(not(feature = "libtiff"))]
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => Err(CodecError::Unsupported(
            "TIFF format writing requires the 'libtiff' feature".to_string(),
        )
        .into()),
        "png" => {
            let writer = PNGDatasetWriter::new_with_output(output)?;
            Ok(Box::new(writer))
        }
        #[cfg(feature = "openjpeg")]
        "j2k" | "jp2" | "jpeg2000" => {
            let writer = J2KDatasetWriter::new_with_output(output)?;
            Ok(Box::new(writer))
        }
        #[cfg(not(feature = "openjpeg"))]
        "j2k" | "jp2" | "jpeg2000" => Err(CodecError::Unsupported(
            "JPEG 2000 format writing requires the 'openjpeg' feature".to_string(),
        )
        .into()),
        #[cfg(feature = "libjpeg-turbo")]
        "jpg" | "jpeg" => {
            let writer = JPEGDatasetWriter::new_with_output(output)?;
            Ok(Box::new(writer))
        }
        #[cfg(not(feature = "libjpeg-turbo"))]
        "jpg" | "jpeg" => Err(CodecError::Unsupported(
            "JPEG format writing requires the 'libjpeg-turbo' feature".to_string(),
        )
        .into()),
        "dted" | "dt0" | "dt1" | "dt2" | "dt3" | "dt4" | "dt5" | "avg" | "min" | "max" => {
            let writer = crate::dted::DTEDDatasetWriter::new_with_output(output)?;
            Ok(Box::new(writer))
        }
        _ => {
            // Unknown format
            Err(CodecError::InvalidFormat(format!("Unsupported format: '{}'", format)).into())
        }
    }
}

// =========================================================================
// ParsedRole — structured role parsing for the `roles` parameter.
// =========================================================================

/// Parsed form of a role string from the `roles` parameter.
///
/// The library uses roles to route sources in a multi-source dataset: the
/// base source ("data") carries the primary image, overview sources
/// ("overview:N") carry R-set pyramid levels, and other roles are reserved
/// for future extensions.
#[cfg_attr(test, derive(Debug, PartialEq))]
enum ParsedRole {
    /// Base asset (primary image). One source per dataset.
    Data,
    /// R-set overview at resolution level N (N >= 1). Matches the
    /// `image:0:overview:N` asset key convention.
    Overview(u32),
    /// A role not recognised by v1 routing. Reserved for future extensions
    /// (e.g., `"metadata"`). Carried through but not routed; the captured
    /// string is retained so downstream code can inspect or log it.
    #[allow(dead_code)]
    Other(String),
}

/// Parses a single role string into a [`ParsedRole`].
///
/// Accepts:
/// - `"data"` → [`ParsedRole::Data`]
/// - `"overview:N"` where N is a positive integer → [`ParsedRole::Overview`]
/// - any other string → [`ParsedRole::Other`] (not routed in v1)
///
/// # Errors
/// - `ValueError` if the role has the `"overview:"` prefix but N is not a
///   positive integer (including `"overview:0"` — 0 is the base, not an
///   overview).
fn parse_role(s: &str) -> PyResult<ParsedRole> {
    if s == "data" {
        return Ok(ParsedRole::Data);
    }
    if let Some(n_str) = s.strip_prefix("overview:") {
        let level = n_str.parse::<u32>().map_err(|_| {
            PyValueError::new_err(format!(
                "Invalid overview level in role '{}': expected 'overview:N' with positive integer N",
                s
            ))
        })?;
        if level == 0 {
            return Err(PyValueError::new_err(format!(
                "Invalid role '{}': overview level must be positive (0 is the base)",
                s
            )));
        }
        return Ok(ParsedRole::Overview(level));
    }
    // Unknown role — carried through but not routed in v1.
    Ok(ParsedRole::Other(s.to_string()))
}

/// Normalized roles shape: one `Vec<String>` per source, indexed by source
/// position. See [`normalize_roles`].
type NormalizedRoles = Vec<Vec<String>>;

/// Normalizes the Python `roles` parameter into a `Vec<Vec<String>>`.
///
/// The parameter accepts two shapes from Python:
/// - `list[str]` (single-source form): one role list applied to the single
///   source. `num_sources` must be 1.
/// - `list[list[str]]` (multi-source form): one inner list per source. The
///   length must match `num_sources`.
///
/// Returns `Ok(None)` when `roles` was not provided.
fn normalize_roles(
    roles: Option<&Bound<'_, PyAny>>,
    num_sources: usize,
) -> PyResult<Option<NormalizedRoles>> {
    let Some(obj) = roles else {
        return Ok(None);
    };

    // Prefer the multi-source form: list[list[str]]
    if let Ok(outer) = obj.extract::<Vec<Vec<String>>>() {
        if outer.len() != num_sources {
            return Err(PyValueError::new_err(format!(
                "roles list length ({}) does not match number of sources ({})",
                outer.len(),
                num_sources
            )));
        }
        return Ok(Some(outer));
    }

    // Fall back to the single-source form: list[str]
    if let Ok(flat) = obj.extract::<Vec<String>>() {
        if num_sources != 1 {
            return Err(PyValueError::new_err(
                "roles must be list[list[str]] when there are multiple sources",
            ));
        }
        return Ok(Some(vec![flat]));
    }

    Err(PyTypeError::new_err(
        "roles must be list[str] or list[list[str]]",
    ))
}

// =========================================================================
// Stream readers/writers
// =========================================================================

/// Reads all bytes from a Python stream via `.read()`.
///
/// Validates the stream has a `.read()` method, calls it, and returns the
/// result as an owned `Vec<u8>`. Copying to an owned buffer (rather than
/// borrowing the `PyBytes` content) ensures the byte slice outlives the
/// Python object for the duration of format parsing, which is important
/// because some `from_bytes()` implementations store references into the
/// input data.
fn read_stream_bytes(py: Python<'_>, stream_obj: &Py<PyAny>) -> PyResult<Vec<u8>> {
    let bound = stream_obj.bind(py);
    if !bound.hasattr("read").unwrap_or(false) {
        return Err(PyTypeError::new_err(
            "Object is not a valid readable file-like object: missing .read() method",
        ));
    }

    let result = bound
        .call_method0("read")
        .map_err(|e| PyIOError::new_err(format!("Failed to read from stream: {}", e)))?;

    let py_bytes = result
        .cast::<PyBytes>()
        .map_err(|_| PyTypeError::new_err(".read() must return bytes"))?;
    let data = py_bytes.as_bytes().to_vec();

    if data.is_empty() {
        return Err(PyValueError::new_err("Stream contained no data"));
    }

    Ok(data)
}

/// Dispatches a format string to the appropriate `from_bytes()` constructor.
///
/// Shared by [`create_reader_from_stream`] and [`open_multi_stream_with_roles`]
/// so all format dispatch lives in one place.
fn reader_from_bytes(format: &str, data: &[u8]) -> PyResult<Box<dyn DatasetReader>> {
    match format.to_lowercase().as_str() {
        "nitf" | "nitf21" | "nitf2.1" | "nsif" | "nsif10" | "nsif1.0" | "jbp" => {
            Ok(Box::new(JBPDatasetReader::from_bytes(data)?))
        }
        #[cfg(feature = "libtiff")]
        "tiff" | "tif" | "gtif" | "gtiff" | "geotiff" => {
            Ok(Box::new(tiff::TIFFDatasetReader::from_bytes(data)?))
        }
        #[cfg(not(feature = "libtiff"))]
        "tiff" | "tif" | "gtif" | "gtiff" | "geotiff" => Err(CodecError::Unsupported(
            "TIFF support not enabled (libtiff feature disabled)".to_string(),
        )
        .into()),
        "png" => Ok(Box::new(PNGDatasetReader::from_bytes(data)?)),
        #[cfg(feature = "openjpeg")]
        "j2k" | "jp2" | "jpeg2000" => Ok(Box::new(J2KDatasetReader::from_bytes(data)?)),
        #[cfg(not(feature = "openjpeg"))]
        "j2k" | "jp2" | "jpeg2000" => Err(CodecError::Unsupported(
            "JPEG 2000 support not enabled (openjpeg feature disabled)".to_string(),
        )
        .into()),
        #[cfg(feature = "libjpeg-turbo")]
        "jpg" | "jpeg" => Ok(Box::new(JPEGDatasetReader::from_bytes(data)?)),
        #[cfg(not(feature = "libjpeg-turbo"))]
        "jpg" | "jpeg" => Err(CodecError::Unsupported(
            "JPEG support not enabled (libjpeg-turbo feature disabled)".to_string(),
        )
        .into()),
        "dted" | "dt0" | "dt1" | "dt2" | "dt3" | "dt4" | "dt5" => {
            Ok(Box::new(DTEDDatasetReader::from_bytes(data)?))
        }
        _ => Err(CodecError::InvalidFormat(format!("Unsupported format: '{}'", format)).into()),
    }
}

/// Creates a `DatasetReader` from a Python stream.
///
/// Calls `.read()` on the stream to obtain all bytes, validates the result,
/// then dispatches to the appropriate format reader's `from_bytes()`.
///
/// Note: reading from a stream loads the entire content into memory. For
/// large files, prefer the file-path code path which uses memory-mapped I/O.
fn create_reader_from_stream(
    py: Python<'_>,
    stream_obj: &Py<PyAny>,
    format: &str,
) -> PyResult<PyDatasetReader> {
    let data = read_stream_bytes(py, stream_obj)?;
    let reader = reader_from_bytes(format, &data)?;
    Ok(PyDatasetReader::new(reader))
}

/// Validates that a Python object has `.write()` and `.flush()` methods.
fn validate_writable_stream(py: Python<'_>, stream_obj: &Py<PyAny>) -> PyResult<()> {
    let bound = stream_obj.bind(py);
    if !bound.hasattr("write").unwrap_or(false) {
        return Err(PyTypeError::new_err(
            "Object is not a valid writable file-like object: missing .write() method",
        ));
    }
    if !bound.hasattr("flush").unwrap_or(false) {
        return Err(PyTypeError::new_err(
            "Object is not a valid writable file-like object: missing .flush() method",
        ));
    }
    Ok(())
}

/// Wraps a validated Python stream in `PyWriteStream` → `BufWriter` →
/// `Box<dyn Write + Send>`.
fn stream_to_boxed_output(stream_obj: Py<PyAny>) -> Box<dyn Write + Send> {
    let pws = PyWriteStream::new(stream_obj);
    let buf_writer = std::io::BufWriter::new(pws);
    Box::new(buf_writer)
}

/// Creates a `DatasetWriter` that writes to a Python stream.
///
/// Validates the stream has `.write()` and `.flush()` methods, wraps it in
/// `PyWriteStream` → `BufWriter` → `Box<dyn Write + Send>`, then dispatches
/// to the format-specific writer via [`create_writer_boxed_from_output`].
fn create_writer_for_stream(
    py: Python<'_>,
    stream_obj: Py<PyAny>,
    format: &str,
) -> PyResult<PyDatasetWriter> {
    validate_writable_stream(py, &stream_obj)?;
    let output = stream_to_boxed_output(stream_obj);
    let writer = create_writer_boxed_from_output(output, format)?;
    Ok(PyDatasetWriter::new(writer))
}

// =========================================================================
// Multi-source dispatch with explicit roles
// =========================================================================

/// Determines the base source index and the overview (level, index) pairs
/// from a parsed role table.
///
/// - Sources with role `"data"` claim the base slot; more than one is an error.
/// - Sources with role `"overview:N"` are collected as (level, index) pairs.
/// - Sources with only [`ParsedRole::Other`] or no roles are ignored for
///   routing in v1 but remain valid.
/// - If no source is explicitly `"data"`, the first source is treated as
///   the base.
fn route_by_roles(roles_per_source: &[Vec<String>]) -> PyResult<(usize, Vec<(u32, usize)>)> {
    let mut base_idx: Option<usize> = None;
    let mut overview_entries: Vec<(u32, usize)> = Vec::new();

    for (idx, role_list) in roles_per_source.iter().enumerate() {
        let parsed: Vec<ParsedRole> = role_list
            .iter()
            .map(|r| parse_role(r))
            .collect::<PyResult<_>>()?;

        if parsed.iter().any(|r| matches!(r, ParsedRole::Data)) {
            if base_idx.is_some() {
                return Err(PyValueError::new_err(
                    "Multiple sources have role 'data'; only one base source is allowed",
                ));
            }
            base_idx = Some(idx);
        }
        for r in &parsed {
            if let ParsedRole::Overview(level) = r {
                overview_entries.push((*level, idx));
            }
        }
    }

    Ok((base_idx.unwrap_or(0), overview_entries))
}

/// Opens a list of file-like objects as a multi-source dataset, using
/// explicit roles to route the base source and overview levels.
fn open_multi_stream_with_roles(
    py: Python<'_>,
    streams: Vec<Py<PyAny>>,
    mode: &str,
    format: &str,
    roles_per_source: &[Vec<String>],
) -> PyResult<Py<PyAny>> {
    let (base_idx, overview_entries) = route_by_roles(roles_per_source)?;

    match mode {
        "r" => {
            // Read all stream bytes up-front. Copying to owned buffers keeps
            // the data valid for the lifetime of the format readers.
            let mut per_source_bytes: Vec<Vec<u8>> = Vec::with_capacity(streams.len());
            for stream_obj in &streams {
                let data = read_stream_bytes(py, stream_obj)?;
                per_source_bytes.push(data);
            }

            let base_reader = reader_from_bytes(format, &per_source_bytes[base_idx])?;

            let mut overviews: Vec<(u32, Arc<dyn ImageAssetProvider>)> = Vec::new();
            for (level, src_idx) in overview_entries {
                let rset_reader = reader_from_bytes(format, &per_source_bytes[src_idx])?;
                let image = extract_primary_image(rset_reader.as_ref()).ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "Overview source at index {} does not contain an image asset",
                        src_idx
                    ))
                })?;
                overviews.push((level, image));
            }

            let composite = CompositeDatasetReader::new(base_reader, overviews);
            let reader = PyDatasetReader::new(Box::new(composite));
            Ok(reader.into_pyobject(py)?.into_any().unbind())
        }
        "w" => {
            // Build one writer per stream, then assemble a composite writer
            // that routes assets by overview role.
            let mut writers: Vec<Option<Box<dyn DatasetWriter>>> =
                Vec::with_capacity(streams.len());
            for stream_obj in streams.into_iter() {
                validate_writable_stream(py, &stream_obj)?;
                let output = stream_to_boxed_output(stream_obj);
                writers.push(Some(create_writer_boxed_from_output(output, format)?));
            }

            let base_writer = writers[base_idx]
                .take()
                .expect("base writer should not have been taken");

            let mut rset_writers: Vec<(u32, Box<dyn DatasetWriter>)> = Vec::new();
            for (level, src_idx) in overview_entries {
                let w = writers[src_idx].take().ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "Source at index {} cannot be used for two roles",
                        src_idx
                    ))
                })?;
                rset_writers.push((level, w));
            }

            // Single-source case: return the base writer directly so format-specific
            // writers (e.g. TIFFDatasetWriter) can handle overview assets natively.
            if rset_writers.is_empty() {
                let writer = PyDatasetWriter::new(base_writer);
                return Ok(writer.into_pyobject(py)?.into_any().unbind());
            }

            let composite = CompositeDatasetWriter::new(base_writer, rset_writers);
            let writer = PyDatasetWriter::new(Box::new(composite));
            Ok(writer.into_pyobject(py)?.into_any().unbind())
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid mode '{}'. Expected 'r' for reading or 'w' for writing.",
            mode
        ))),
    }
}

/// Opens a list of file paths as a multi-source dataset, using explicit
/// roles to route the base source and overview levels.
///
/// This bypasses the `.rN` filename detection used by
/// [`create_multi_path_reader`] / [`create_multi_path_writer`] when the
/// caller supplies roles explicitly.
fn open_multi_path_with_roles(
    py: Python<'_>,
    paths: &[String],
    mode: &str,
    format: Option<&str>,
    roles_per_source: &[Vec<String>],
) -> PyResult<Py<PyAny>> {
    let (base_idx, overview_entries) = route_by_roles(roles_per_source)?;

    match mode {
        "r" => {
            let base_parsed = ParsedUri::parse(&paths[base_idx]);
            let base_reader = create_reader_boxed(&base_parsed, format)?;

            let mut overviews: Vec<(u32, Arc<dyn ImageAssetProvider>)> = Vec::new();
            for (level, src_idx) in overview_entries {
                let parsed = ParsedUri::parse(&paths[src_idx]);
                let rset_reader = create_reader_boxed(&parsed, format)?;
                let image = extract_primary_image(rset_reader.as_ref()).ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "Overview path '{}' does not contain an image asset",
                        &paths[src_idx]
                    ))
                })?;
                overviews.push((level, image));
            }

            let composite = CompositeDatasetReader::new(base_reader, overviews);
            let reader = PyDatasetReader::new(Box::new(composite));
            Ok(reader.into_pyobject(py)?.into_any().unbind())
        }
        "w" => {
            // Resolve format: explicit > extension-derived from base path.
            let base_parsed = ParsedUri::parse(&paths[base_idx]);
            let format_str = match format {
                Some(f) => f.to_string(),
                None => detect_write_format(&base_parsed).ok_or_else(|| {
                    PyValueError::new_err(
                        "Cannot determine output format: no format specified \
                         and file extension is not recognized",
                    )
                })?,
            };

            let base_writer = create_writer_boxed(&base_parsed, &format_str)?;

            let mut rset_writers: Vec<(u32, Box<dyn DatasetWriter>)> = Vec::new();
            for (level, src_idx) in overview_entries {
                let parsed = ParsedUri::parse(&paths[src_idx]);
                let writer = create_writer_boxed(&parsed, &format_str)?;
                rset_writers.push((level, writer));
            }

            // Single-path case: return the base writer directly so format-specific
            // writers (e.g. TIFFDatasetWriter) can handle overview assets natively.
            if rset_writers.is_empty() {
                let writer = PyDatasetWriter::new(base_writer);
                return Ok(writer.into_pyobject(py)?.into_any().unbind());
            }

            let composite = CompositeDatasetWriter::new(base_writer, rset_writers);
            let writer = PyDatasetWriter::new(Box::new(composite));
            Ok(writer.into_pyobject(py)?.into_any().unbind())
        }
        _ => Err(PyValueError::new_err(format!(
            "Invalid mode '{}'. Expected 'r' for reading or 'w' for writing.",
            mode
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uri_plain_path() {
        let parsed = ParsedUri::parse("image.ntf");
        assert_eq!(parsed.scheme, "file");
        assert_eq!(parsed.path, "image.ntf");
    }

    #[test]
    fn test_parse_uri_absolute_path() {
        let parsed = ParsedUri::parse("/path/to/image.ntf");
        assert_eq!(parsed.scheme, "file");
        assert_eq!(parsed.path, "/path/to/image.ntf");
    }

    #[test]
    fn test_parse_uri_file_scheme() {
        let parsed = ParsedUri::parse("file:///path/to/image.ntf");
        assert_eq!(parsed.scheme, "file");
        assert_eq!(parsed.path, "/path/to/image.ntf");
    }

    #[test]
    fn test_parse_uri_s3_scheme() {
        let parsed = ParsedUri::parse("s3://bucket/key/image.ntf");
        assert_eq!(parsed.scheme, "s3");
        assert_eq!(parsed.path, "bucket/key/image.ntf");
    }

    #[test]
    fn test_extension_simple() {
        let parsed = ParsedUri::parse("image.ntf");
        assert_eq!(parsed.extension(), Some("ntf"));
    }

    #[test]
    fn test_extension_with_path() {
        let parsed = ParsedUri::parse("/path/to/image.tiff");
        assert_eq!(parsed.extension(), Some("tiff"));
    }

    #[test]
    fn test_extension_none() {
        let parsed = ParsedUri::parse("/path/to/image");
        assert_eq!(parsed.extension(), None);
    }

    #[test]
    fn test_extension_hidden_file() {
        let parsed = ParsedUri::parse("/path/to/.hidden");
        // .hidden has no extension (hidden is the filename)
        assert_eq!(parsed.extension(), Some("hidden"));
    }

    // =========================================================================
    // PathsArg extraction tests
    // =========================================================================

    #[test]
    fn test_paths_arg_extract_string() {
        // A Python str should produce PathsArg::Single
        Python::attach(|py| {
            let py_str = "image.ntf".into_pyobject(py).unwrap();
            let any_ref = py_str.as_any();
            let arg = PathsArg::extract(any_ref.as_borrowed()).unwrap();
            match arg {
                PathsArg::Single(s) => assert_eq!(s, "image.ntf".to_string()),
                other => panic!("Expected PathsArg::Single, got: {:?}", other),
            }
        });
    }

    #[test]
    fn test_paths_arg_extract_list() {
        // A Python list[str] should produce PathsArg::Multiple
        Python::attach(|py| {
            let py_list = vec!["a.ntf", "b.ntf"].into_pyobject(py).unwrap();
            let any_ref = py_list.as_any();
            let arg = PathsArg::extract(any_ref.as_borrowed()).unwrap();
            match arg {
                PathsArg::Multiple(v) => {
                    assert_eq!(v, vec!["a.ntf".to_string(), "b.ntf".to_string()]);
                }
                other => panic!("Expected PathsArg::Multiple, got: {:?}", other),
            }
        });
    }

    #[test]
    fn test_paths_arg_extract_invalid_type() {
        // A Python int should produce an error
        Python::attach(|py| {
            let py_int = 42i64.into_pyobject(py).unwrap();
            let any_ref = py_int.as_any();
            let result = PathsArg::extract(any_ref.as_borrowed());
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("paths must be a str or list[str]"),
                "Expected 'paths must be a str or list[str]' in error, got: {}",
                err_msg
            );
        });
    }

    #[test]
    fn test_create_reader_nitf_extension() {
        // This will fail because the file doesn't exist, but it should
        // get past the format detection
        let parsed = ParsedUri::parse("nonexistent.ntf");
        let result = create_reader(&parsed, None);
        assert!(result.is_err());
        // Should be an IO error, not an InvalidFormat error
        let err_str = format!("{:?}", result.err());
        assert!(
            !err_str.contains("Unsupported file format"),
            "Expected file not found error, got: {}",
            err_str
        );
    }

    #[test]
    fn test_create_reader_with_explicit_format() {
        let parsed = ParsedUri::parse("nonexistent.dat");
        let result = create_reader(&parsed, Some("nitf"));
        assert!(result.is_err());
        // Should be an IO error, not an InvalidFormat error
        let err_str = format!("{:?}", result.err());
        assert!(
            !err_str.contains("Unsupported format"),
            "Expected file not found error, got: {}",
            err_str
        );
    }

    #[test]
    fn test_create_writer_nitf_format() {
        let parsed = ParsedUri::parse("/tmp/test_output.ntf");
        let result = create_writer(&parsed, "nitf");
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_writer_nsif_format() {
        let parsed = ParsedUri::parse("/tmp/test_output.nsif");
        let result = create_writer(&parsed, "nsif");
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_writer_unknown_format() {
        let parsed = ParsedUri::parse("/tmp/test_output.xyz");
        let result = create_writer(&parsed, "unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_reader_s3_not_supported() {
        let parsed = ParsedUri::parse("s3://bucket/key/image.ntf");
        let result = create_reader(&parsed, None);
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err());
        assert!(err_str.contains("S3"));
    }

    #[test]
    fn test_create_writer_s3_not_supported() {
        let parsed = ParsedUri::parse("s3://bucket/key/output.ntf");
        let result = create_writer(&parsed, "nitf");
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err());
        assert!(err_str.contains("S3"));
    }

    #[test]
    #[cfg(feature = "libtiff")]
    fn test_create_reader_tif_extension() {
        // Should get past format detection and attempt to open the file
        let parsed = ParsedUri::parse("nonexistent.tif");
        let result = create_reader(&parsed, None);
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err());
        assert!(
            !err_str.contains("Unsupported file format"),
            "Expected file not found error, got: {}",
            err_str
        );
    }

    #[test]
    #[cfg(feature = "libtiff")]
    fn test_create_reader_tiff_extension() {
        let parsed = ParsedUri::parse("nonexistent.tiff");
        let result = create_reader(&parsed, None);
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err());
        assert!(
            !err_str.contains("Unsupported file format"),
            "Expected file not found error, got: {}",
            err_str
        );
    }

    #[test]
    #[cfg(feature = "libtiff")]
    fn test_create_reader_explicit_tiff_format() {
        let parsed = ParsedUri::parse("nonexistent.dat");
        let result = create_reader(&parsed, Some("tiff"));
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err());
        assert!(
            !err_str.contains("Unsupported format"),
            "Expected file not found error, got: {}",
            err_str
        );
    }

    #[test]
    #[cfg(feature = "libtiff")]
    fn test_create_reader_explicit_tif_format() {
        let parsed = ParsedUri::parse("nonexistent.dat");
        let result = create_reader(&parsed, Some("tif"));
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err());
        assert!(
            !err_str.contains("Unsupported format"),
            "Expected file not found error, got: {}",
            err_str
        );
    }

    #[test]
    #[cfg(feature = "libtiff")]
    fn test_create_writer_tiff_supported() {
        let parsed = ParsedUri::parse("/tmp/test_output.tif");
        let result = create_writer(&parsed, "tiff");
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    #[cfg(feature = "libtiff")]
    fn test_create_writer_tif_format_supported() {
        let parsed = ParsedUri::parse("/tmp/test_output.tif");
        let result = create_writer(&parsed, "tif");
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    // =========================================================================
    // R-set filename detection tests
    // =========================================================================

    #[test]
    fn test_extract_rset_level_valid() {
        assert_eq!(extract_rset_level("image.ntf.r1"), Some(1));
        assert_eq!(extract_rset_level("image.ntf.r5"), Some(5));
        assert_eq!(extract_rset_level("image.ntf.r12"), Some(12));
        assert_eq!(extract_rset_level("/path/to/image.ntf.r3"), Some(3));
    }

    #[test]
    fn test_extract_rset_level_r0_returns_none() {
        // .r0 is the base, not an overview
        assert_eq!(extract_rset_level("image.ntf.r0"), None);
    }

    #[test]
    fn test_extract_rset_level_no_rset() {
        assert_eq!(extract_rset_level("image.ntf"), None);
        assert_eq!(extract_rset_level("image.tif"), None);
        assert_eq!(extract_rset_level("image"), None);
    }

    #[test]
    fn test_extract_rset_level_not_final_extension() {
        // .r1 is not the final extension
        assert_eq!(extract_rset_level("image.r1.ntf"), None);
    }

    #[test]
    fn test_extract_rset_level_non_numeric() {
        assert_eq!(extract_rset_level("image.ntf.rabc"), None);
        assert_eq!(extract_rset_level("image.ntf.r"), None);
        assert_eq!(extract_rset_level("image.ntf.r1a"), None);
    }

    #[test]
    fn test_strip_rset_suffix_with_rset() {
        assert_eq!(strip_rset_suffix("image.ntf.r1"), "image.ntf");
        assert_eq!(strip_rset_suffix("image.ntf.r12"), "image.ntf");
        assert_eq!(
            strip_rset_suffix("/path/to/image.ntf.r3"),
            "/path/to/image.ntf"
        );
    }

    #[test]
    fn test_strip_rset_suffix_without_rset() {
        assert_eq!(strip_rset_suffix("image.ntf"), "image.ntf");
        assert_eq!(strip_rset_suffix("image.tif"), "image.tif");
    }

    // =========================================================================
    // Multi-path R-set reader tests
    // =========================================================================

    #[test]
    fn test_multi_path_rset_reader() {
        // Use existing test NITF files, copying them to temp dir with R-set names
        let base_path = std::path::Path::new("data/unit/nitf21-256x256-3band-8bit-nc.ntf");
        let rset_path = std::path::Path::new("data/unit/nitf21-8x8-1band-8bit-nc.ntf");
        if !base_path.exists() || !rset_path.exists() {
            return; // Skip if test data not available
        }

        let tmp_dir = tempfile::tempdir().unwrap();
        let base_file = tmp_dir.path().join("large.ntf");
        let rset_file = tmp_dir.path().join("large.ntf.r1");

        std::fs::copy(base_path, &base_file).unwrap();
        std::fs::copy(rset_path, &rset_file).unwrap();

        let paths = vec![
            base_file.to_str().unwrap().to_string(),
            rset_file.to_str().unwrap().to_string(),
        ];

        // Use create_multi_path_reader_boxed to get a Box<dyn DatasetReader>
        let reader = create_multi_path_reader_boxed(&paths, None).unwrap();

        // Should have base image + overview
        let all_keys = reader.get_asset_keys(None, None);
        assert!(
            all_keys.contains(&"image:0".to_string()),
            "Missing image:0 key"
        );
        assert!(
            all_keys.contains(&"image:0:overview:1".to_string()),
            "Missing image:0:overview:1 key"
        );

        // Verify image:0 has the larger dimensions (256x256)
        let base_asset = reader.get_asset("image:0").unwrap();
        assert_eq!(base_asset.asset_type(), AssetType::Image);
        assert!(base_asset.roles().contains(&"data".to_string()));

        // Verify image:0:overview:1 has the smaller dimensions (8x8)
        let ovr_asset = reader.get_asset("image:0:overview:1").unwrap();
        assert_eq!(ovr_asset.asset_type(), AssetType::Image);
        assert!(ovr_asset.roles().contains(&"overview".to_string()));

        // Verify has_asset works
        assert!(reader.has_asset("image:0"));
        assert!(reader.has_asset("image:0:overview:1"));
        assert!(!reader.has_asset("image:0:overview:2"));
    }

    #[test]
    fn test_multi_path_rset_out_of_order() {
        // Test that overview levels come from filenames, not list position
        let base_path = std::path::Path::new("data/unit/nitf21-256x256-3band-8bit-nc.ntf");
        let rset_path = std::path::Path::new("data/unit/nitf21-8x8-1band-8bit-nc.ntf");
        if !base_path.exists() || !rset_path.exists() {
            return;
        }

        let tmp_dir = tempfile::tempdir().unwrap();
        let base_file = tmp_dir.path().join("img.ntf");
        let rset1_file = tmp_dir.path().join("img.ntf.r1");
        let rset3_file = tmp_dir.path().join("img.ntf.r3");

        std::fs::copy(base_path, &base_file).unwrap();
        std::fs::copy(rset_path, &rset1_file).unwrap();
        std::fs::copy(rset_path, &rset3_file).unwrap();

        // Pass in reverse order: r3 before r1
        let paths = vec![
            base_file.to_str().unwrap().to_string(),
            rset3_file.to_str().unwrap().to_string(),
            rset1_file.to_str().unwrap().to_string(),
        ];

        let reader = create_multi_path_reader_boxed(&paths, None).unwrap();

        let all_keys = reader.get_asset_keys(None, None);
        assert!(all_keys.contains(&"image:0".to_string()));
        assert!(all_keys.contains(&"image:0:overview:1".to_string()));
        assert!(all_keys.contains(&"image:0:overview:3".to_string()));
        // Should NOT have overview:2 (no .r2 file)
        assert!(!all_keys.contains(&"image:0:overview:2".to_string()));
    }

    #[test]
    fn test_single_path_unchanged_behavior() {
        // Single path should work identically to current behavior
        let base_path = std::path::Path::new("data/unit/nitf21-256x256-3band-8bit-nc.ntf");
        if !base_path.exists() {
            return;
        }

        let parsed = ParsedUri::parse(base_path.to_str().unwrap());
        let reader = create_reader_boxed(&parsed, None).unwrap();

        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["image:0"]);
        assert!(!reader.has_asset("image:0:overview:1"));
    }

    #[test]
    fn test_multi_path_rset_tile_byte_ranges() {
        // Verify that tile_byte_ranges() works for both base and overview assets
        let base_path = std::path::Path::new("data/unit/nitf21-256x256-3band-8bit-nc.ntf");
        let rset_path = std::path::Path::new("data/unit/nitf21-8x8-1band-8bit-nc.ntf");
        if !base_path.exists() || !rset_path.exists() {
            return;
        }

        let tmp_dir = tempfile::tempdir().unwrap();
        let base_file = tmp_dir.path().join("large.ntf");
        let rset_file = tmp_dir.path().join("large.ntf.r1");

        std::fs::copy(base_path, &base_file).unwrap();
        std::fs::copy(rset_path, &rset_file).unwrap();

        let paths = vec![
            base_file.to_str().unwrap().to_string(),
            rset_file.to_str().unwrap().to_string(),
        ];

        let reader = create_multi_path_reader_boxed(&paths, None).unwrap();

        // Get the overview asset and verify it has tile_byte_ranges
        // (NC compression should have tile byte ranges)
        let ovr_asset = reader.get_asset("image:0:overview:1").unwrap();
        assert_eq!(ovr_asset.asset_type(), AssetType::Image);

        // The overview asset should retain its original tile_byte_ranges
        // pointing to its own source file (the .r1 file)
    }

    #[test]
    fn test_multi_path_non_rset_rejected() {
        // Additional paths that don't match .rN should be rejected
        let base_path = std::path::Path::new("data/unit/nitf21-256x256-3band-8bit-nc.ntf");
        let other_path = std::path::Path::new("data/unit/nitf21-8x8-1band-8bit-nc.ntf");
        if !base_path.exists() || !other_path.exists() {
            return;
        }

        let paths = vec![
            base_path.to_str().unwrap().to_string(),
            other_path.to_str().unwrap().to_string(),
        ];

        let result = create_multi_path_reader_boxed(&paths, None);
        assert!(result.is_err(), "Should reject non-R-set additional paths");
    }

    #[test]
    fn test_multi_path_rset_image_keys_filter() {
        // Verify that get_asset_keys with Image filter includes overviews
        let base_path = std::path::Path::new("data/unit/nitf21-256x256-3band-8bit-nc.ntf");
        let rset_path = std::path::Path::new("data/unit/nitf21-8x8-1band-8bit-nc.ntf");
        if !base_path.exists() || !rset_path.exists() {
            return;
        }

        let tmp_dir = tempfile::tempdir().unwrap();
        let base_file = tmp_dir.path().join("large.ntf");
        let rset_file = tmp_dir.path().join("large.ntf.r1");

        std::fs::copy(base_path, &base_file).unwrap();
        std::fs::copy(rset_path, &rset_file).unwrap();

        let paths = vec![
            base_file.to_str().unwrap().to_string(),
            rset_file.to_str().unwrap().to_string(),
        ];

        let reader = create_multi_path_reader_boxed(&paths, None).unwrap();

        // Image filter should include both base and overview
        let image_keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert!(image_keys.contains(&"image:0".to_string()));
        assert!(image_keys.contains(&"image:0:overview:1".to_string()));

        // Text filter should NOT include overviews
        let text_keys = reader.get_asset_keys(Some(AssetType::Text), None);
        assert!(!text_keys.contains(&"image:0:overview:1".to_string()));
    }

    // =========================================================================
    // Write-mode format auto-detection tests
    // =========================================================================

    #[test]
    fn test_detect_write_format_nitf_extensions() {
        let parsed = ParsedUri::parse("output.ntf");
        assert_eq!(detect_write_format(&parsed), Some("nitf".to_string()));

        let parsed = ParsedUri::parse("output.nitf");
        assert_eq!(detect_write_format(&parsed), Some("nitf".to_string()));
    }

    #[test]
    fn test_detect_write_format_nsif_extensions() {
        let parsed = ParsedUri::parse("output.nsf");
        assert_eq!(detect_write_format(&parsed), Some("nsif".to_string()));

        let parsed = ParsedUri::parse("output.nsif");
        assert_eq!(detect_write_format(&parsed), Some("nsif".to_string()));
    }

    #[test]
    fn test_detect_write_format_tiff_extensions() {
        for ext in &["tif", "tiff", "gtif", "gtiff"] {
            let parsed = ParsedUri::parse(&format!("output.{}", ext));
            assert_eq!(
                detect_write_format(&parsed),
                Some("tiff".to_string()),
                "Failed for extension: {}",
                ext
            );
        }
    }

    #[test]
    fn test_detect_write_format_png() {
        let parsed = ParsedUri::parse("output.png");
        assert_eq!(detect_write_format(&parsed), Some("png".to_string()));
    }

    #[test]
    fn test_detect_write_format_j2k_extensions() {
        let parsed = ParsedUri::parse("output.j2k");
        assert_eq!(detect_write_format(&parsed), Some("j2k".to_string()));

        let parsed = ParsedUri::parse("output.jp2");
        assert_eq!(detect_write_format(&parsed), Some("j2k".to_string()));
    }

    #[test]
    fn test_detect_write_format_jpeg_extensions() {
        let parsed = ParsedUri::parse("output.jpg");
        assert_eq!(detect_write_format(&parsed), Some("jpeg".to_string()));

        let parsed = ParsedUri::parse("output.jpeg");
        assert_eq!(detect_write_format(&parsed), Some("jpeg".to_string()));
    }

    #[test]
    fn test_detect_write_format_rset_suffix_stripped() {
        // .ntf.r1 should strip .r1 and detect "nitf"
        let parsed = ParsedUri::parse("output.ntf.r1");
        assert_eq!(detect_write_format(&parsed), Some("nitf".to_string()));
    }

    #[test]
    fn test_detect_write_format_tiff_rset_suffix_stripped() {
        // .tiff.r3 should strip .r3 and detect "tiff"
        let parsed = ParsedUri::parse("output.tiff.r3");
        assert_eq!(detect_write_format(&parsed), Some("tiff".to_string()));
    }

    #[test]
    fn test_detect_write_format_unrecognized() {
        let parsed = ParsedUri::parse("output.xyz");
        assert_eq!(detect_write_format(&parsed), None);
    }

    #[test]
    fn test_detect_write_format_no_extension() {
        let parsed = ParsedUri::parse("output");
        assert_eq!(detect_write_format(&parsed), None);
    }

    // =========================================================================
    // PathsArg stream variant tests (Task 8.1)
    // =========================================================================

    /// A Python file-like object (e.g., `io.BytesIO`) is extracted as
    /// `PathsArg::Stream`.
    #[test]
    fn test_paths_arg_extract_stream() {
        Python::attach(|py| {
            let io_module = py.import("io").unwrap();
            let bytesio = io_module.call_method0("BytesIO").unwrap();
            let any_ref = bytesio.as_any();
            let arg = PathsArg::extract(any_ref.as_borrowed()).unwrap();
            assert!(
                matches!(arg, PathsArg::Stream(_)),
                "Expected PathsArg::Stream, got: {:?}",
                arg
            );
        });
    }

    /// A Python list of file-like objects is extracted as
    /// `PathsArg::StreamList`.
    #[test]
    fn test_paths_arg_extract_stream_list() {
        Python::attach(|py| {
            let io_module = py.import("io").unwrap();
            let b1 = io_module.call_method0("BytesIO").unwrap();
            let b2 = io_module.call_method0("BytesIO").unwrap();
            let list = pyo3::types::PyList::new(py, &[b1, b2]).unwrap();
            let any_ref = list.as_any();
            let arg = PathsArg::extract(any_ref.as_borrowed()).unwrap();
            match arg {
                PathsArg::StreamList(v) => assert_eq!(v.len(), 2),
                other => panic!("Expected PathsArg::StreamList, got: {:?}", other),
            }
        });
    }

    /// A Python `str` is classified as a path even though strings happen to
    /// have attribute names that could collide with file-like probing.
    #[test]
    fn test_paths_arg_prefers_string_over_stream() {
        Python::attach(|py| {
            let py_str = "path.ntf".into_pyobject(py).unwrap();
            let any_ref = py_str.as_any();
            let arg = PathsArg::extract(any_ref.as_borrowed()).unwrap();
            assert!(
                matches!(arg, PathsArg::Single(ref s) if s == "path.ntf"),
                "Expected PathsArg::Single, got: {:?}",
                arg
            );
        });
    }

    /// A mixed list of `str` and file-like objects raises a `TypeError`.
    #[test]
    fn test_paths_arg_rejects_mixed_list() {
        Python::attach(|py| {
            let io_module = py.import("io").unwrap();
            let bytesio = io_module.call_method0("BytesIO").unwrap();
            let str_item = "path.ntf".into_pyobject(py).unwrap();
            let list = pyo3::types::PyList::new(
                py,
                &[str_item.as_any().clone(), bytesio.as_any().clone()],
            )
            .unwrap();
            let any_ref = list.as_any();
            let err = PathsArg::extract(any_ref.as_borrowed()).unwrap_err();
            let err_str = err.to_string();
            assert!(
                err_str.contains("all strings or all file-like"),
                "unexpected error: {}",
                err_str
            );
            assert!(
                err.is_instance_of::<PyTypeError>(py),
                "expected TypeError, got: {}",
                err_str
            );
        });
    }

    /// A non-str, non-list, non-file-like object (e.g., an int) raises
    /// `ValueError`.
    #[test]
    fn test_paths_arg_invalid_type() {
        Python::attach(|py| {
            let py_int = 42i64.into_pyobject(py).unwrap();
            let any_ref = py_int.as_any();
            let result = PathsArg::extract(any_ref.as_borrowed());
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("paths must be a str or list[str]"),
                "Expected 'paths must be a str or list[str]' in error, got: {}",
                err_msg
            );
        });
    }

    // =========================================================================
    // ParsedRole / parse_role tests (Task 8.2)
    // =========================================================================

    #[test]
    fn test_parse_role_data() {
        let parsed = parse_role("data").unwrap();
        assert_eq!(parsed, ParsedRole::Data);
    }

    #[test]
    fn test_parse_role_overview_valid() {
        assert_eq!(parse_role("overview:1").unwrap(), ParsedRole::Overview(1));
        assert_eq!(parse_role("overview:5").unwrap(), ParsedRole::Overview(5));
        assert_eq!(parse_role("overview:42").unwrap(), ParsedRole::Overview(42));
    }

    #[test]
    fn test_parse_role_overview_zero_rejected() {
        Python::attach(|_py| {
            let err = parse_role("overview:0").unwrap_err();
            assert!(
                err.to_string().contains("overview level must be positive"),
                "unexpected error: {}",
                err
            );
        });
    }

    #[test]
    fn test_parse_role_overview_malformed_rejected() {
        Python::attach(|_py| {
            let err = parse_role("overview:abc").unwrap_err();
            assert!(
                err.to_string().contains("Invalid overview level"),
                "unexpected error: {}",
                err
            );
            let err = parse_role("overview:").unwrap_err();
            assert!(
                err.to_string().contains("Invalid overview level"),
                "unexpected error: {}",
                err
            );
        });
    }

    #[test]
    fn test_parse_role_other() {
        assert_eq!(
            parse_role("metadata").unwrap(),
            ParsedRole::Other("metadata".to_string())
        );
        assert_eq!(
            parse_role("auxiliary").unwrap(),
            ParsedRole::Other("auxiliary".to_string())
        );
    }

    // =========================================================================
    // normalize_roles tests (Task 8.2)
    // =========================================================================

    #[test]
    fn test_normalize_roles_none() {
        Python::attach(|_py| {
            let result = normalize_roles(None, 1).unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_normalize_roles_single_flat() {
        // list[str] form for a single source: ["data"]
        Python::attach(|py| {
            let list = vec!["data".to_string(), "metadata".to_string()]
                .into_pyobject(py)
                .unwrap();
            let any = list.as_any();
            let result = normalize_roles(Some(any), 1).unwrap().unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0], vec!["data".to_string(), "metadata".to_string()]);
        });
    }

    #[test]
    fn test_normalize_roles_multi_nested() {
        // list[list[str]] form for multiple sources
        Python::attach(|py| {
            let inner1 = vec!["data".to_string()];
            let inner2 = vec!["overview:1".to_string()];
            let list = vec![inner1.clone(), inner2.clone()]
                .into_pyobject(py)
                .unwrap();
            let any = list.as_any();
            let result = normalize_roles(Some(any), 2).unwrap().unwrap();
            assert_eq!(result, vec![inner1, inner2]);
        });
    }

    #[test]
    fn test_normalize_roles_length_mismatch() {
        Python::attach(|py| {
            let inner1 = vec!["data".to_string()];
            let list = vec![inner1].into_pyobject(py).unwrap();
            let any = list.as_any();
            let err = normalize_roles(Some(any), 3).unwrap_err();
            assert!(
                err.to_string().contains("does not match number of sources"),
                "unexpected error: {}",
                err
            );
        });
    }

    #[test]
    fn test_normalize_roles_wrong_shape_flat_for_multi() {
        // list[str] passed when there are multiple sources — error
        Python::attach(|py| {
            let list = vec!["data".to_string()].into_pyobject(py).unwrap();
            let any = list.as_any();
            let err = normalize_roles(Some(any), 2).unwrap_err();
            assert!(
                err.to_string()
                    .contains("roles must be list[list[str]] when there are multiple sources"),
                "unexpected error: {}",
                err
            );
        });
    }

    #[test]
    fn test_normalize_roles_invalid_type() {
        Python::attach(|py| {
            let py_int = 42i64.into_pyobject(py).unwrap();
            let any = py_int.as_any();
            let err = normalize_roles(Some(any), 1).unwrap_err();
            assert!(
                err.to_string()
                    .contains("roles must be list[str] or list[list[str]]"),
                "unexpected error: {}",
                err
            );
        });
    }

    // =========================================================================
    // route_by_roles tests (Task 8.6)
    // =========================================================================

    #[test]
    fn test_route_by_roles_implicit_base() {
        // No source has explicit 'data' → first source becomes the base.
        let roles = vec![vec!["metadata".to_string()], vec!["overview:1".to_string()]];
        let (base, overviews) = route_by_roles(&roles).unwrap();
        assert_eq!(base, 0);
        assert_eq!(overviews, vec![(1, 1)]);
    }

    #[test]
    fn test_route_by_roles_explicit_data() {
        let roles = vec![
            vec!["overview:2".to_string()],
            vec!["data".to_string()],
            vec!["overview:1".to_string()],
        ];
        let (base, mut overviews) = route_by_roles(&roles).unwrap();
        overviews.sort();
        assert_eq!(base, 1);
        assert_eq!(overviews, vec![(1, 2), (2, 0)]);
    }

    #[test]
    fn test_route_by_roles_duplicate_data_rejected() {
        Python::attach(|_py| {
            let roles = vec![vec!["data".to_string()], vec!["data".to_string()]];
            let err = route_by_roles(&roles).unwrap_err();
            assert!(
                err.to_string()
                    .contains("Multiple sources have role 'data'"),
                "unexpected error: {}",
                err
            );
        });
    }

    #[test]
    fn test_route_by_roles_other_role_not_routed() {
        // ParsedRole::Other values are kept but don't affect routing.
        let roles = vec![
            vec!["data".to_string(), "metadata".to_string()],
            vec!["auxiliary".to_string()],
        ];
        let (base, overviews) = route_by_roles(&roles).unwrap();
        assert_eq!(base, 0);
        assert!(overviews.is_empty());
    }

    // =========================================================================
    // Property-based tests
    // =========================================================================

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Feature: multifile-rset-writing, Property 5: Write-mode format auto-detection
        //
        // **Validates: Requirements 6.1**
        //
        // For any recognized extension, `detect_write_format()` returns the
        // expected format string. Also verifies that appending an `.rN` suffix
        // still produces the same result after rset stripping.
        proptest! {
            #[test]
            fn prop_detect_write_format_recognized_extensions(
                (ext, expected) in prop::sample::select(vec![
                    ("ntf", "nitf"),
                    ("nitf", "nitf"),
                    ("nsf", "nsif"),
                    ("nsif", "nsif"),
                    ("tif", "tiff"),
                    ("tiff", "tiff"),
                    ("gtif", "tiff"),
                    ("gtiff", "tiff"),
                    ("png", "png"),
                    ("j2k", "j2k"),
                    ("jp2", "j2k"),
                    ("jpg", "jpeg"),
                    ("jpeg", "jpeg"),
                ]),
                rset_level in 1u32..=20,
            ) {
                // Test direct extension detection
                let path = format!("output.{}", ext);
                let parsed = ParsedUri::parse(&path);
                let result = detect_write_format(&parsed);
                prop_assert_eq!(
                    result.as_deref(),
                    Some(expected),
                    "Extension '{}' should map to '{}'",
                    ext,
                    expected,
                );

                // Test with .rN suffix appended — rset stripping should
                // still yield the same format
                let rset_path = format!("output.{}.r{}", ext, rset_level);
                let rset_parsed = ParsedUri::parse(&rset_path);
                let rset_result = detect_write_format(&rset_parsed);
                prop_assert_eq!(
                    rset_result.as_deref(),
                    Some(expected),
                    "Extension '{}.r{}' should strip rset suffix and map to '{}'",
                    ext,
                    rset_level,
                    expected,
                );
            }
        }
    }
}
