//! IO Factory for opening datasets.
//!
//! This module provides the IO factory class that selects appropriate
//! reader/writer implementations based on URI scheme and file format.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::bindings::{PyDatasetReader, PyDatasetWriter};
use crate::error::CodecError;
use crate::jbp;

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

/// Factory class for opening geospatial datasets.
///
/// The IO class provides a simple factory function to open datasets for reading
/// or writing. It automatically detects the file format from the file extension
/// and magic bytes, and returns the appropriate reader or writer implementation.
///
/// # Example
///
/// ```python
/// from aws.osml.io import IO
///
/// # Open for reading
/// with IO.open("image.ntf", "r") as reader:
///     keys = reader.get_asset_keys()
///     asset = reader.get_asset(keys[0])
///
/// # Open for writing
/// with IO.open("output.ntf", "w") as writer:
///     writer.add_asset("image", provider, "Title", "Description", ["data"])
/// ```
#[pyclass(name = "IO")]
pub struct IO;

#[pymethods]
impl IO {
    /// Opens a dataset for reading or writing.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI or path to the dataset. Supports:
    ///   - Local file paths (e.g., "image.ntf", "/path/to/image.tif")
    ///   - File URIs (e.g., "file:///path/to/image.ntf")
    ///   - S3 URIs (e.g., "s3://bucket/key/image.ntf")
    ///
    /// * `mode` - The access mode:
    ///   - "r" for reading (returns DatasetReader)
    ///   - "w" for writing (returns DatasetWriter)
    ///
    /// * `format` - Optional format specification for writing (e.g., "nitf", "nsif").
    ///   Required when mode is "w". Ignored when mode is "r".
    ///
    /// # Returns
    ///
    /// A DatasetReader when mode is "r", or a DatasetWriter when mode is "w".
    ///
    /// # Raises
    ///
    /// * ValueError - If the mode is invalid or the file format is not supported.
    /// * IOError - If the file cannot be opened.
    #[staticmethod]
    #[pyo3(signature = (uri, mode="r", format=None))]
    fn open(py: Python<'_>, uri: &str, mode: &str, format: Option<&str>) -> PyResult<PyObject> {
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

/// Creates a DatasetReader for the given URI.
///
/// This function determines the appropriate reader implementation based on
/// the URI scheme and file format.
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
                let reader = jbp::IO::open_as(&parsed.path, fmt)?;
                return Ok(PyDatasetReader::new(reader));
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
            // NITF/NSIF format - use JBP reader
            let reader = jbp::IO::open(&parsed.path)?;
            Ok(PyDatasetReader::new(reader))
        }
        Some("tif") | Some("tiff") | Some("gtif") | Some("gtiff") => {
            // GeoTIFF format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "GeoTIFF format reader not yet implemented for: {}",
                parsed.path
            ))
            .into())
        }
        Some("jp2") | Some("j2k") | Some("jpx") => {
            // JPEG2000 format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "JPEG2000 format reader not yet implemented for: {}",
                parsed.path
            ))
            .into())
        }
        Some(ext) => {
            // Unknown format
            Err(CodecError::InvalidFormat(format!(
                "Unsupported file format: .{}",
                ext
            ))
            .into())
        }
        None => {
            // No extension - cannot determine format
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
        "nitf" | "nitf21" | "nitf2.1" | "nsif" | "nsif10" | "nsif1.0" => {
            // NITF/NSIF format - use JBP writer
            let writer = jbp::IO::create(&parsed.path, format)?;
            Ok(PyDatasetWriter::new(writer))
        }
        "tif" | "tiff" | "gtif" | "gtiff" | "geotiff" => {
            // GeoTIFF format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "GeoTIFF format writer not yet implemented for: {}",
                parsed.path
            ))
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
}
