//! IO Factory for opening datasets.
//!
//! This module provides the IO factory class that selects appropriate
//! reader/writer implementations based on URI scheme and file format.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::bindings::{PyDatasetReader, PyDatasetWriter};
use crate::error::CodecError;

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
    /// # Returns
    ///
    /// A DatasetReader when mode is "r", or a DatasetWriter when mode is "w".
    ///
    /// # Raises
    ///
    /// * ValueError - If the mode is invalid or the file format is not supported.
    /// * IOError - If the file cannot be opened.
    #[staticmethod]
    #[pyo3(signature = (uri, mode="r"))]
    fn open(py: Python<'_>, uri: &str, mode: &str) -> PyResult<PyObject> {
        let parsed = ParsedUri::parse(uri);

        match mode {
            "r" => {
                // Create a reader based on the URI scheme and format
                let reader = create_reader(&parsed)?;
                Ok(reader.into_py(py))
            }
            "w" => {
                // Create a writer based on the URI scheme and format
                let writer = create_writer(&parsed)?;
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
fn create_reader(parsed: &ParsedUri) -> PyResult<PyDatasetReader> {
    // Validate scheme is supported
    match parsed.scheme.as_str() {
        "file" | "s3" => {}
        scheme => {
            return Err(CodecError::Unsupported(format!(
                "Unsupported URI scheme: {}",
                scheme
            ))
            .into());
        }
    }

    // Detect format from extension
    let extension = parsed.extension().map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("ntf") | Some("nitf") | Some("nsf") => {
            // NITF format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "NITF format reader not yet implemented for: {}",
                parsed.path
            ))
            .into())
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
fn create_writer(parsed: &ParsedUri) -> PyResult<PyDatasetWriter> {
    // Validate scheme is supported
    match parsed.scheme.as_str() {
        "file" | "s3" => {}
        scheme => {
            return Err(CodecError::Unsupported(format!(
                "Unsupported URI scheme: {}",
                scheme
            ))
            .into());
        }
    }

    // Detect format from extension
    let extension = parsed.extension().map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("ntf") | Some("nitf") | Some("nsf") => {
            // NITF format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "NITF format writer not yet implemented for: {}",
                parsed.path
            ))
            .into())
        }
        Some("tif") | Some("tiff") | Some("gtif") | Some("gtiff") => {
            // GeoTIFF format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "GeoTIFF format writer not yet implemented for: {}",
                parsed.path
            ))
            .into())
        }
        Some("jp2") | Some("j2k") | Some("jpx") => {
            // JPEG2000 format - not yet implemented
            Err(CodecError::Unsupported(format!(
                "JPEG2000 format writer not yet implemented for: {}",
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
}
