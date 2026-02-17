//! IO factory for JBP dataset operations.
//!
//! This module provides the [`IO`] struct which serves as a factory for creating
//! dataset readers and writers with automatic format detection based on file extension.
//!
//! # Example
//!
//! ```ignore
//! use aws_osml_io::jbp::IO;
//!
//! // Open a NITF file for reading (format auto-detected from extension)
//! let reader = IO::open("image.ntf")?;
//!
//! // Open with explicit format specification
//! let reader = IO::open_as("image.dat", "nitf")?;
//!
//! // Create a new NITF file for writing
//! let writer = IO::create("output.ntf", "nitf")?;
//! ```

use std::path::Path;

use crate::error::CodecError;
use crate::jbp::format::is_nitf_extension;
use crate::jbp::reader::JBPDatasetReader;
use crate::jbp::types::NitfFormat;
use crate::jbp::writer::JBPDatasetWriter;
use crate::traits::{DatasetReader, DatasetWriter};

/// Factory for creating JBP dataset readers and writers.
///
/// The IO struct provides static methods for opening datasets for reading
/// or creating new datasets for writing. It supports automatic format detection
/// based on file extension, as well as explicit format specification.
///
/// # Format Detection
///
/// When using [`IO::open`], the format is detected from the file extension:
/// - `.ntf`, `.nitf` → NITF 2.1
/// - `.nsif` → NSIF 1.0
///
/// The actual format (NITF 2.1 vs NSIF 1.0) is determined by the magic number
/// in the file header during parsing.
///
/// # Supported Formats
///
/// - `"nitf"`, `"nitf21"`, `"nitf2.1"` → NITF 2.1
/// - `"nsif"`, `"nsif10"`, `"nsif1.0"` → NSIF 1.0
/// - `"jbp"` → JBP (auto-detect NITF/NSIF)
pub struct IO;

impl IO {
    /// Open a dataset for reading with automatic format detection.
    ///
    /// The format is detected from the file extension (case-insensitive):
    /// - `.ntf`, `.nitf` → NITF format
    /// - `.nsif` → NSIF format
    ///
    /// The reader validates the magic number when parsing the header,
    /// raising an error if the file content doesn't match the expected format.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the NITF/NSIF file
    ///
    /// # Returns
    ///
    /// A boxed [`DatasetReader`] implementation for the file.
    ///
    /// # Errors
    ///
    /// - [`CodecError::InvalidFormat`] if the file extension is not recognized
    /// - [`CodecError::Io`] if the file cannot be read
    /// - [`CodecError::InvalidFormat`] if the magic number is invalid
    ///
    /// # Example
    ///
    /// ```ignore
    /// use aws_osml_io::jbp::IO;
    ///
    /// let reader = IO::open("image.ntf")?;
    /// let keys = reader.get_asset_keys(None, None);
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Box<dyn DatasetReader>, CodecError> {
        let path = path.as_ref();

        if is_nitf_extension(path) {
            // Create JBP reader - magic number validated during header parse
            Ok(Box::new(JBPDatasetReader::open(path)?))
        } else {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("(none)");
            Err(CodecError::InvalidFormat(format!(
                "Unsupported file extension: .{}. Expected .ntf, .nitf, or .nsif",
                ext
            )))
        }
    }

    /// Open a dataset for reading with explicit format specification.
    ///
    /// Use this method when the file extension doesn't match the content,
    /// or when you want to explicitly specify the format.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    /// * `format` - Format specification (case-insensitive):
    ///   - `"nitf"`, `"nitf21"`, `"nitf2.1"` → NITF 2.1
    ///   - `"nsif"`, `"nsif10"`, `"nsif1.0"` → NSIF 1.0
    ///   - `"jbp"` → JBP (auto-detect from magic number)
    ///
    /// # Returns
    ///
    /// A boxed [`DatasetReader`] implementation for the file.
    ///
    /// # Errors
    ///
    /// - [`CodecError::InvalidFormat`] if the format is not recognized
    /// - [`CodecError::Io`] if the file cannot be read
    /// - [`CodecError::InvalidFormat`] if the magic number is invalid
    ///
    /// # Example
    ///
    /// ```ignore
    /// use aws_osml_io::jbp::IO;
    ///
    /// // Open a file with non-standard extension
    /// let reader = IO::open_as("image.dat", "nitf")?;
    /// ```
    pub fn open_as(
        path: impl AsRef<Path>,
        format: &str,
    ) -> Result<Box<dyn DatasetReader>, CodecError> {
        match format.to_lowercase().as_str() {
            "nitf" | "nitf21" | "nitf2.1" | "nsif" | "nsif10" | "nsif1.0" | "jbp" => {
                Ok(Box::new(JBPDatasetReader::open(path)?))
            }
            _ => Err(CodecError::InvalidFormat(format!(
                "Unsupported format: '{}'. Expected 'nitf', 'nsif', or 'jbp'",
                format
            ))),
        }
    }

    /// Create a new dataset for writing.
    ///
    /// The file is not created until [`DatasetWriter::close`] is called.
    ///
    /// # Arguments
    ///
    /// * `path` - Output file path
    /// * `format` - Format specification (case-insensitive):
    ///   - `"nitf"`, `"nitf21"`, `"nitf2.1"` → NITF 2.1
    ///   - `"nsif"`, `"nsif10"`, `"nsif1.0"` → NSIF 1.0
    ///
    /// # Returns
    ///
    /// A boxed [`DatasetWriter`] implementation for creating the file.
    ///
    /// # Errors
    ///
    /// - [`CodecError::InvalidFormat`] if the format is not recognized
    ///
    /// # Example
    ///
    /// ```ignore
    /// use aws_osml_io::jbp::IO;
    ///
    /// let mut writer = IO::create("output.ntf", "nitf")?;
    /// // Add assets...
    /// writer.close()?;
    /// ```
    pub fn create(
        path: impl AsRef<Path>,
        format: &str,
    ) -> Result<Box<dyn DatasetWriter>, CodecError> {
        let nitf_format = match format.to_lowercase().as_str() {
            "nitf" | "nitf21" | "nitf2.1" => NitfFormat::Nitf21,
            "nsif" | "nsif10" | "nsif1.0" => NitfFormat::Nsif10,
            _ => {
                return Err(CodecError::InvalidFormat(format!(
                    "Unsupported format: '{}'. Expected 'nitf' or 'nsif'",
                    format
                )))
            }
        };

        Ok(Box::new(JBPDatasetWriter::new(path, nitf_format)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== IO::open tests ====================

    #[test]
    fn open_rejects_unsupported_extension() {
        let result = IO::open("image.jpg");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Unsupported file extension"));
    }

    #[test]
    fn open_rejects_no_extension() {
        let result = IO::open("image");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Unsupported file extension"));
    }

    #[test]
    fn open_accepts_ntf_extension() {
        // This will fail because the file doesn't exist, but it should
        // get past the extension check
        let result = IO::open("nonexistent.ntf");
        assert!(result.is_err());
        // Should be an IO error, not an InvalidFormat error
        let err = result.err().unwrap();
        assert!(
            err.to_string().contains("No such file")
                || err.to_string().contains("cannot find")
                || err.to_string().contains("not found"),
            "Expected file not found error, got: {}",
            err
        );
    }

    #[test]
    fn open_accepts_nitf_extension() {
        let result = IO::open("nonexistent.nitf");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            err.to_string().contains("No such file")
                || err.to_string().contains("cannot find")
                || err.to_string().contains("not found"),
            "Expected file not found error, got: {}",
            err
        );
    }

    #[test]
    fn open_accepts_nsif_extension() {
        let result = IO::open("nonexistent.nsif");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            err.to_string().contains("No such file")
                || err.to_string().contains("cannot find")
                || err.to_string().contains("not found"),
            "Expected file not found error, got: {}",
            err
        );
    }

    #[test]
    fn open_extension_case_insensitive() {
        // All these should pass extension check (fail on file not found)
        for ext in &["NTF", "Ntf", "NITF", "Nitf", "NSIF", "Nsif"] {
            let path = format!("nonexistent.{}", ext);
            let result = IO::open(&path);
            assert!(result.is_err());
            let err = result.err().unwrap();
            assert!(
                !err.to_string().contains("Unsupported file extension"),
                "Extension {} should be accepted, got: {}",
                ext,
                err
            );
        }
    }

    // ==================== IO::open_as tests ====================

    #[test]
    fn open_as_accepts_nitf_format() {
        let result = IO::open_as("nonexistent.dat", "nitf");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            !err.to_string().contains("Unsupported format"),
            "Format 'nitf' should be accepted, got: {}",
            err
        );
    }

    #[test]
    fn open_as_accepts_nitf21_format() {
        let result = IO::open_as("nonexistent.dat", "nitf21");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            !err.to_string().contains("Unsupported format"),
            "Format 'nitf21' should be accepted, got: {}",
            err
        );
    }

    #[test]
    fn open_as_accepts_nsif_format() {
        let result = IO::open_as("nonexistent.dat", "nsif");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            !err.to_string().contains("Unsupported format"),
            "Format 'nsif' should be accepted, got: {}",
            err
        );
    }

    #[test]
    fn open_as_accepts_jbp_format() {
        let result = IO::open_as("nonexistent.dat", "jbp");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(
            !err.to_string().contains("Unsupported format"),
            "Format 'jbp' should be accepted, got: {}",
            err
        );
    }

    #[test]
    fn open_as_format_case_insensitive() {
        for format in &["NITF", "Nitf", "NSIF", "Nsif", "JBP", "Jbp"] {
            let result = IO::open_as("nonexistent.dat", format);
            assert!(result.is_err());
            let err = result.err().unwrap();
            assert!(
                !err.to_string().contains("Unsupported format"),
                "Format '{}' should be accepted, got: {}",
                format,
                err
            );
        }
    }

    #[test]
    fn open_as_rejects_unknown_format() {
        let result = IO::open_as("nonexistent.dat", "unknown");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Unsupported format"));
    }

    // ==================== IO::create tests ====================

    #[test]
    fn create_accepts_nitf_format() {
        let result = IO::create("/tmp/test_output.ntf", "nitf");
        assert!(result.is_ok());
    }

    #[test]
    fn create_accepts_nitf21_format() {
        let result = IO::create("/tmp/test_output.ntf", "nitf21");
        assert!(result.is_ok());
    }

    #[test]
    fn create_accepts_nsif_format() {
        let result = IO::create("/tmp/test_output.nsif", "nsif");
        assert!(result.is_ok());
    }

    #[test]
    fn create_accepts_nsif10_format() {
        let result = IO::create("/tmp/test_output.nsif", "nsif10");
        assert!(result.is_ok());
    }

    #[test]
    fn create_format_case_insensitive() {
        for format in &["NITF", "Nitf", "NSIF", "Nsif"] {
            let result = IO::create("/tmp/test_output.ntf", format);
            assert!(
                result.is_ok(),
                "Format '{}' should be accepted, got: {:?}",
                format,
                result.err()
            );
        }
    }

    #[test]
    fn create_rejects_unknown_format() {
        let result = IO::create("/tmp/test_output.ntf", "unknown");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Unsupported format"));
    }

    #[test]
    fn create_rejects_jbp_format() {
        // JBP is not a valid output format (it's for auto-detection on read)
        let result = IO::create("/tmp/test_output.ntf", "jbp");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("Unsupported format"));
    }
}

// Property-based tests for IO factory
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    /// Property 23: Python Format Auto-Detection
    /// For any NITF or NSIF file opened via `IO.open()`, the returned reader
    /// SHALL be able to access all segments without the caller specifying the format.
    /// **Validates: Requirements 19.3**
    mod prop_23_format_auto_detection {
        use super::*;

        /// Strategy for generating valid NITF extensions (case variations)
        fn valid_nitf_extension() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("ntf".to_string()),
                Just("NTF".to_string()),
                Just("Ntf".to_string()),
                Just("nitf".to_string()),
                Just("NITF".to_string()),
                Just("Nitf".to_string()),
                Just("nsif".to_string()),
                Just("NSIF".to_string()),
                Just("Nsif".to_string()),
            ]
        }

        /// Strategy for generating invalid (non-NITF) extensions
        fn invalid_extension() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("jpg".to_string()),
                Just("png".to_string()),
                Just("tiff".to_string()),
                Just("gif".to_string()),
                Just("txt".to_string()),
                Just("pdf".to_string()),
                Just("xml".to_string()),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// IO::open accepts valid NITF extensions and rejects invalid ones
            /// This tests the format auto-detection at the extension level
            #[test]
            fn open_accepts_valid_extensions_rejects_invalid(
                ext in prop_oneof![valid_nitf_extension(), invalid_extension()],
            ) {
                let path = PathBuf::from(format!("nonexistent_file.{}", ext));
                let result = IO::open(&path);

                let lower_ext = ext.to_lowercase();
                let is_valid = lower_ext == "ntf" || lower_ext == "nitf" || lower_ext == "nsif";

                if is_valid {
                    // Should pass extension check but fail on file not found
                    prop_assert!(result.is_err());
                    let err = result.err().unwrap();
                    // Error should be about file not found, not unsupported extension
                    prop_assert!(
                        !err.to_string().contains("Unsupported file extension"),
                        "Valid extension '{}' should be accepted, got: {}",
                        ext,
                        err
                    );
                } else {
                    // Should fail with unsupported extension error
                    prop_assert!(result.is_err());
                    let err = result.err().unwrap();
                    prop_assert!(
                        err.to_string().contains("Unsupported file extension"),
                        "Invalid extension '{}' should be rejected with extension error, got: {}",
                        ext,
                        err
                    );
                }
            }

            /// IO::open_as accepts all valid format strings
            #[test]
            fn open_as_accepts_valid_formats(
                format in prop_oneof![
                    Just("nitf".to_string()),
                    Just("NITF".to_string()),
                    Just("nitf21".to_string()),
                    Just("NITF21".to_string()),
                    Just("nitf2.1".to_string()),
                    Just("nsif".to_string()),
                    Just("NSIF".to_string()),
                    Just("nsif10".to_string()),
                    Just("NSIF10".to_string()),
                    Just("nsif1.0".to_string()),
                    Just("jbp".to_string()),
                    Just("JBP".to_string()),
                ],
            ) {
                let result = IO::open_as("nonexistent.dat", &format);
                prop_assert!(result.is_err());
                let err = result.err().unwrap();
                // Should fail on file not found, not unsupported format
                prop_assert!(
                    !err.to_string().contains("Unsupported format"),
                    "Format '{}' should be accepted, got: {}",
                    format,
                    err
                );
            }

            /// IO::open_as rejects invalid format strings
            #[test]
            fn open_as_rejects_invalid_formats(
                format in prop_oneof![
                    Just("jpeg".to_string()),
                    Just("png".to_string()),
                    Just("tiff".to_string()),
                    Just("geotiff".to_string()),
                    Just("unknown".to_string()),
                    Just("nitf20".to_string()),
                    Just("nsif20".to_string()),
                ],
            ) {
                let result = IO::open_as("nonexistent.dat", &format);
                prop_assert!(result.is_err());
                let err = result.err().unwrap();
                prop_assert!(
                    err.to_string().contains("Unsupported format"),
                    "Format '{}' should be rejected, got: {}",
                    format,
                    err
                );
            }

            /// IO::create accepts valid output formats
            #[test]
            fn create_accepts_valid_formats(
                format in prop_oneof![
                    Just("nitf".to_string()),
                    Just("NITF".to_string()),
                    Just("nitf21".to_string()),
                    Just("nitf2.1".to_string()),
                    Just("nsif".to_string()),
                    Just("NSIF".to_string()),
                    Just("nsif10".to_string()),
                    Just("nsif1.0".to_string()),
                ],
            ) {
                let result = IO::create("/tmp/test_prop23.ntf", &format);
                prop_assert!(
                    result.is_ok(),
                    "Format '{}' should be accepted for create, got: {:?}",
                    format,
                    result.err()
                );
            }

            /// IO::create rejects invalid output formats (including jbp which is read-only)
            #[test]
            fn create_rejects_invalid_formats(
                format in prop_oneof![
                    Just("jbp".to_string()),
                    Just("JBP".to_string()),
                    Just("jpeg".to_string()),
                    Just("png".to_string()),
                    Just("unknown".to_string()),
                ],
            ) {
                let result = IO::create("/tmp/test_prop23.ntf", &format);
                prop_assert!(result.is_err());
                let err = result.err().unwrap();
                prop_assert!(
                    err.to_string().contains("Unsupported format"),
                    "Format '{}' should be rejected for create, got: {}",
                    format,
                    err
                );
            }
        }

        /// Integration test: Open real NITF file and verify segment access
        /// This test uses the actual test data file to verify end-to-end functionality
        #[test]
        fn open_real_nitf_file_and_access_segments() {
            // Use the small.ntf test file from unit test data
            let test_file = std::path::Path::new("data/unit/small.ntf");
            if !test_file.exists() {
                // Skip test if test data not available
                return;
            }

            // Open without specifying format - should auto-detect
            let reader = IO::open(test_file);
            assert!(
                reader.is_ok(),
                "Should open NITF file without format specification: {:?}",
                reader.err()
            );

            let reader = reader.unwrap();

            // Should be able to get asset keys without errors
            let keys = reader.get_asset_keys(None, None);

            // The file should have at least one segment
            assert!(
                !keys.is_empty(),
                "NITF file should have at least one segment"
            );

            // Each key should follow the expected pattern
            for key in &keys {
                assert!(
                    key.contains("_segment_"),
                    "Asset key '{}' should follow pattern '{{type}}_segment_{{index}}'",
                    key
                );
            }

            // Should be able to check asset existence
            for key in &keys {
                assert!(
                    reader.has_asset(key),
                    "has_asset should return true for key '{}'",
                    key
                );
            }
        }
    }
}
