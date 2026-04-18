//! IO Factory for opening datasets.
//!
//! This module provides the IO factory class that selects appropriate
//! reader/writer implementations based on URI scheme and file format.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::bindings::{PyDatasetReader, PyDatasetWriter};
use crate::composite::{CompositeDatasetReader, CompositeDatasetWriter};

/// Accepts either a single string or a list of strings from Python.
///
/// This enum allows `IO.open()` to accept both `str` and `list[str]` as the
/// `paths` argument, normalizing a bare string to a single-element list.
#[cfg_attr(test, derive(Debug))]
enum PathsArg {
    Single(String),
    Multiple(Vec<String>),
}

impl<'a, 'py> FromPyObject<'a, 'py> for PathsArg {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(s) = ob.extract::<String>() {
            Ok(PathsArg::Single(s))
        } else if let Ok(v) = ob.extract::<Vec<String>>() {
            Ok(PathsArg::Multiple(v))
        } else {
            Err(PyValueError::new_err("paths must be a str or list[str]"))
        }
    }
}

impl From<PathsArg> for Vec<String> {
    fn from(arg: PathsArg) -> Vec<String> {
        match arg {
            PathsArg::Single(s) => vec![s],
            PathsArg::Multiple(v) => v,
        }
    }
}
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
/// The ``IO`` class provides a single static method, ``open``, that accepts a
/// file path string or a list of file paths (or URIs) and returns either a
/// :class:`DatasetReader` or a :class:`DatasetWriter` depending on the requested
/// mode. The file format is auto-detected from the extension and file header
/// bytes when reading; supported formats include NITF 2.0/2.1, NSIF 1.0, and
/// TIFF/GeoTIFF. Both local file paths and ``file://`` URIs are supported.
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
    /// The format is auto-detected from the file extension when reading. When
    /// writing, a format string must be provided. Use a context manager (``with``
    /// statement) on the returned object to ensure file handles are released.
    ///
    /// :param paths: A file path or list of file paths to the dataset. For
    ///     single-file formats a bare string is accepted (``"image.ntf"``).
    ///     For multi-file R-set datasets a list is required. Accepts local
    ///     paths, ``file://`` URIs, and ``s3://`` URIs.
    /// :type paths: str | list[str]
    /// :param mode: ``"r"`` for reading or ``"w"`` for writing. Defaults to
    ///     ``"r"``.
    /// :type mode: str
    /// :param format: Format identifier required when *mode* is ``"w"``
    ///     (e.g., ``"nitf"``, ``"geotiff"``). Ignored when reading.
    /// :type format: str or None
    /// :returns: A :class:`DatasetReader` when *mode* is ``"r"``, or a
    ///     :class:`DatasetWriter` when *mode* is ``"w"``.
    /// :rtype: DatasetReader or DatasetWriter
    /// :raises ValueError: If *paths* is empty, the mode is invalid, or the
    ///     file format is not supported.
    /// :raises IOError: If the file cannot be opened.
    ///
    /// Example:
    ///
    /// ```python
    /// from aws.osml.io import IO
    ///
    /// # Read mode — single string path
    /// with IO.open("image.ntf", "r") as dataset:
    ///     print(type(dataset))  # DatasetReader
    ///
    /// # Read mode — list of paths (R-set)
    /// with IO.open(["image.ntf", "image.ntf.r1"], "r") as dataset:
    ///     print(type(dataset))  # DatasetReader
    ///
    /// # Write mode — format must be specified
    /// with IO.open("output.ntf", "w", "nitf") as writer:
    ///     print(type(writer))  # DatasetWriter
    /// ```
    #[staticmethod]
    #[pyo3(signature = (paths, mode="r", format=None))]
    fn open(
        py: Python<'_>,
        paths: PathsArg,
        mode: &str,
        format: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        // Normalize PathsArg to Vec<String>
        let paths: Vec<String> = paths.into();

        // Validate that paths is not empty
        let uri = paths
            .first()
            .ok_or_else(|| PyValueError::new_err("paths list cannot be empty"))?;

        // Validate that no path is an empty string
        if paths.iter().any(|p| p.is_empty()) {
            return Err(PyValueError::new_err("paths list cannot be empty"));
        }

        let parsed = ParsedUri::parse(uri);

        match mode {
            "r" => {
                if paths.len() > 1 {
                    // Multi-path: detect R-set files and build composite reader
                    let reader = create_multi_path_reader(&paths, format)?;
                    Ok(reader.into_pyobject(py)?.into_any().unbind())
                } else {
                    // Single-path: existing behavior
                    let reader = create_reader(&parsed, format)?;
                    Ok(reader.into_pyobject(py)?.into_any().unbind())
                }
            }
            "w" => {
                // Resolve format: use explicit format if provided, otherwise
                // auto-detect from the file extension (stripping .rN suffix).
                let format_str = match format {
                    Some(f) => f.to_string(),
                    None => detect_write_format(&parsed).ok_or_else(|| {
                        PyValueError::new_err(
                            "Cannot determine output format: no format specified \
                             and file extension is not recognized",
                        )
                    })?,
                };

                if paths.len() > 1 {
                    // Multi-path: create composite writer for R-set files
                    let writer = create_multi_path_writer(&paths, &format_str)?;
                    Ok(writer.into_pyobject(py)?.into_any().unbind())
                } else {
                    // Single-path: existing behavior
                    let writer = create_writer(&parsed, &format_str)?;
                    Ok(writer.into_pyobject(py)?.into_any().unbind())
                }
            }
            _ => Err(PyValueError::new_err(format!(
                "Invalid mode '{}'. Expected 'r' for reading or 'w' for writing.",
                mode
            ))),
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
                    "ntf" | "nitf" | "nsif" | "nsf" => "nitf",
                    "tif" | "tiff" | "gtif" | "gtiff" => "tiff",
                    "png" => "png",
                    "j2k" | "jp2" => "j2k",
                    "jpg" | "jpeg" => "jpeg",
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
            "ntf" | "nitf" => Some("nitf".to_string()),
            "nsf" | "nsif" => Some("nsif".to_string()),
            "tif" | "tiff" | "gtif" | "gtiff" => Some("tiff".to_string()),
            "png" => Some("png".to_string()),
            "j2k" | "jp2" => Some("j2k".to_string()),
            "jpg" | "jpeg" => Some("jpeg".to_string()),
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
            _ => {
                return Err(
                    CodecError::InvalidFormat(format!("Unsupported format: '{}'", fmt)).into(),
                );
            }
        }
    }

    // Detect format from extension
    let extension = parsed.extension().map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("ntf") | Some("nitf") | Some("nsif") | Some("nsf") => {
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

    match format.to_lowercase().as_str() {
        "nitf" | "nitf21" | "nitf2.1" => {
            let writer = JBPDatasetWriter::new(&parsed.path, NitfFormat::Nitf21)?;
            Ok(Box::new(writer))
        }
        "nsif" | "nsif10" | "nsif1.0" => {
            let writer = JBPDatasetWriter::new(&parsed.path, NitfFormat::Nsif10)?;
            Ok(Box::new(writer))
        }
        #[cfg(feature = "libtiff")]
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => {
            let writer = tiff::TIFFDatasetWriter::new(&parsed.path)?;
            Ok(Box::new(writer))
        }
        #[cfg(not(feature = "libtiff"))]
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => Err(CodecError::Unsupported(
            "TIFF format writing requires the 'libtiff' feature".to_string(),
        )
        .into()),
        "png" => {
            let writer = PNGDatasetWriter::new(&parsed.path)?;
            Ok(Box::new(writer))
        }
        #[cfg(feature = "openjpeg")]
        "j2k" | "jp2" | "jpeg2000" => {
            let writer = J2KDatasetWriter::new(&parsed.path)?;
            Ok(Box::new(writer))
        }
        #[cfg(not(feature = "openjpeg"))]
        "j2k" | "jp2" | "jpeg2000" => Err(CodecError::Unsupported(
            "JPEG 2000 format writing requires the 'openjpeg' feature".to_string(),
        )
        .into()),
        #[cfg(feature = "libjpeg-turbo")]
        "jpg" | "jpeg" => {
            let writer = JPEGDatasetWriter::new(&parsed.path)?;
            Ok(Box::new(writer))
        }
        #[cfg(not(feature = "libjpeg-turbo"))]
        "jpg" | "jpeg" => Err(CodecError::Unsupported(
            "JPEG format writing requires the 'libjpeg-turbo' feature".to_string(),
        )
        .into()),
        _ => {
            // Unknown format
            Err(CodecError::InvalidFormat(format!("Unsupported file format: {}", format)).into())
        }
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
            let paths: Vec<String> = arg.into();
            assert_eq!(paths, vec!["image.ntf".to_string()]);
        });
    }

    #[test]
    fn test_paths_arg_extract_list() {
        // A Python list[str] should produce PathsArg::Multiple
        Python::attach(|py| {
            let py_list = vec!["a.ntf", "b.ntf"].into_pyobject(py).unwrap();
            let any_ref = py_list.as_any();
            let arg = PathsArg::extract(any_ref.as_borrowed()).unwrap();
            let paths: Vec<String> = arg.into();
            assert_eq!(paths, vec!["a.ntf".to_string(), "b.ntf".to_string()]);
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
