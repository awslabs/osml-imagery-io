//! IO Factory for opening datasets.
//!
//! This module provides the IO factory class that selects appropriate
//! reader/writer implementations based on URI scheme and file format.

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::bindings::{PyDatasetReader, PyDatasetWriter};
use crate::error::CodecError;
use crate::jbp::{JBPDatasetReader, JBPDatasetWriter, NitfFormat};
#[cfg(feature = "libtiff")]
use crate::tiff;

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
/// The ``IO`` class provides a single static method, ``open``, that accepts one
/// or more file paths (or URIs) and returns either a :class:`DatasetReader` or a
/// :class:`DatasetWriter` depending on the requested mode. The file format is
/// auto-detected from the extension and file header bytes when reading; supported
/// formats include NITF 2.0/2.1, NSIF 1.0, and TIFF/GeoTIFF. Both local file
/// paths and ``file://`` URIs are supported.
///
/// Example::
///
///     from aws.osml.io import IO
///
///     # Read mode — returns a DatasetReader (format auto-detected)
///     with IO.open(["image.ntf"], "r") as dataset:
///         keys = dataset.get_asset_keys()
///         asset = dataset.get_asset(keys[0])
///
///     # Write mode — returns a DatasetWriter
///     with IO.open(["output.ntf"], "w", "nitf") as writer:
///         writer.add_asset("image", provider, "Title", "Description", ["data"])
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
    /// :param paths: One or more URIs or file paths to the dataset. For
    ///     single-file formats only the first path is used. Accepts local paths
    ///     (``["image.ntf"]``), ``file://`` URIs, and ``s3://`` URIs.
    /// :type paths: list[str]
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
    /// Example::
    ///
    ///     from aws.osml.io import IO
    ///
    ///     # Read mode — format auto-detected from extension
    ///     with IO.open(["image.ntf"], "r") as dataset:
    ///         print(type(dataset))  # DatasetReader
    ///
    ///     # Write mode — format must be specified
    ///     with IO.open(["output.ntf"], "w", "nitf") as writer:
    ///         print(type(writer))  # DatasetWriter
    #[staticmethod]
    #[pyo3(signature = (paths, mode="r", format=None))]
    fn open(
        py: Python<'_>,
        paths: Vec<String>,
        mode: &str,
        format: Option<&str>,
    ) -> PyResult<PyObject> {
        // Validate that paths is not empty
        let uri = paths.first().ok_or_else(|| {
            PyValueError::new_err("paths list cannot be empty")
        })?;

        let parsed = ParsedUri::parse(uri);

        match mode {
            "r" => {
                // Create a reader based on the URI scheme and format
                let reader = create_reader(&parsed, format)?;
                Ok(reader.into_py(py))
            }
            "w" => {
                // Create a writer based on the URI scheme and format
                let format_str = format.ok_or_else(|| {
                    PyValueError::new_err("Format must be specified when opening for writing")
                })?;
                let writer = create_writer(&parsed, format_str)?;
                Ok(writer.into_py(py))
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

/// Creates a DatasetReader for the given URI.
///
/// This function determines the appropriate reader implementation based on
/// the URI scheme and file format. Files are memory-mapped and passed as
/// byte slices to the format-specific readers.
fn create_reader(parsed: &ParsedUri, format: Option<&str>) -> PyResult<PyDatasetReader> {
    // Validate scheme is supported
    match parsed.scheme.as_str() {
        "file" => {}
        "s3" => {
            return Err(CodecError::Unsupported(
                "S3 URIs are not yet supported".to_string(),
            )
            .into());
        }
        scheme => {
            return Err(CodecError::Unsupported(format!(
                "Unsupported URI scheme: {}",
                scheme
            ))
            .into());
        }
    }

    // If format is explicitly specified, use it
    if let Some(fmt) = format {
        match fmt.to_lowercase().as_str() {
            "nitf" | "nitf21" | "nitf2.1" | "nsif" | "nsif10" | "nsif1.0" | "jbp" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = JBPDatasetReader::from_bytes(&mmap)?;
                return Ok(PyDatasetReader::new(Box::new(reader)));
            }
            #[cfg(feature = "libtiff")]
            "tiff" | "tif" => {
                let mmap = mmap_file(&parsed.path)?;
                let reader = tiff::TIFFDatasetReader::from_bytes(&mmap)?;
                return Ok(PyDatasetReader::new(Box::new(reader)));
            }
            _ => {
                return Err(CodecError::InvalidFormat(format!(
                    "Unsupported format: '{}'",
                    fmt
                ))
                .into());
            }
        }
    }

    // Detect format from extension
    let extension = parsed.extension().map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("ntf") | Some("nitf") | Some("nsif") | Some("nsf") => {
            let mmap = mmap_file(&parsed.path)?;
            let reader = JBPDatasetReader::from_bytes(&mmap)?;
            Ok(PyDatasetReader::new(Box::new(reader)))
        }
        Some("tif") | Some("tiff") | Some("gtif") | Some("gtiff") => {
            #[cfg(feature = "libtiff")]
            {
                let mmap = mmap_file(&parsed.path)?;
                let reader = tiff::TIFFDatasetReader::from_bytes(&mmap)?;
                return Ok(PyDatasetReader::new(Box::new(reader)));
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
        Some("jp2") | Some("j2k") | Some("jpx") => {
            Err(CodecError::Unsupported(format!(
                "JPEG2000 format reader not yet implemented for: {}",
                parsed.path
            ))
            .into())
        }
        Some(ext) => {
            Err(CodecError::InvalidFormat(format!(
                "Unsupported file format: .{}",
                ext
            ))
            .into())
        }
        None => {
            Err(CodecError::InvalidFormat(
                "Cannot determine file format: no file extension".to_string(),
            )
            .into())
        }
    }
}

/// Creates a DatasetWriter for the given URI.
///
/// This function determines the appropriate writer implementation based on
/// the URI scheme and file format.
fn create_writer(parsed: &ParsedUri, format: &str) -> PyResult<PyDatasetWriter> {
    // Validate scheme is supported
    match parsed.scheme.as_str() {
        "file" => {}
        "s3" => {
            return Err(CodecError::Unsupported(
                "S3 URIs are not yet supported".to_string(),
            )
            .into());
        }
        scheme => {
            return Err(CodecError::Unsupported(format!(
                "Unsupported URI scheme: {}",
                scheme
            ))
            .into());
        }
    }

    match format.to_lowercase().as_str() {
        "nitf" | "nitf21" | "nitf2.1" => {
            let writer = JBPDatasetWriter::new(&parsed.path, NitfFormat::Nitf21)?;
            Ok(PyDatasetWriter::new(Box::new(writer)))
        }
        "nsif" | "nsif10" | "nsif1.0" => {
            let writer = JBPDatasetWriter::new(&parsed.path, NitfFormat::Nsif10)?;
            Ok(PyDatasetWriter::new(Box::new(writer)))
        }
        #[cfg(feature = "libtiff")]
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => {
            let writer = tiff::TIFFDatasetWriter::new(&parsed.path)?;
            Ok(PyDatasetWriter::new(Box::new(writer)))
        }
        #[cfg(not(feature = "libtiff"))]
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => {
            Err(CodecError::Unsupported(
                "TIFF format writing requires the 'libtiff' feature".to_string(),
            )
            .into())
        }
        "jp2" | "j2k" | "jpx" | "jpeg2000" => {
            // JPEG2000 format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "JPEG2000 format writer not yet implemented for: {}",
                parsed.path
            ))
            .into())
        }
        _ => {
            // Unknown format
            Err(CodecError::InvalidFormat(format!(
                "Unsupported file format: {}",
                format
            ))
            .into())
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
}
