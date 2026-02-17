//! Error types for JBP dataset operations.
//!
//! This module defines error types specific to JBP (NITF/NSIF) file operations,
//! including validation warnings that allow parsing to continue while collecting
//! issues for later inspection.

use crate::error::CodecError;
use crate::parser::AccessError;
use thiserror::Error;

/// Errors specific to JBP dataset operations.
#[derive(Error, Debug)]
pub enum JBPError {
    /// Invalid NITF/NSIF format (e.g., bad magic number)
    #[error("Invalid NITF format: {message}")]
    InvalidFormat {
        /// Description of the format error
        message: String,
    },

    /// Requested asset key does not exist
    #[error("Asset not found: {key}")]
    AssetNotFound {
        /// The asset key that was not found
        key: String,
    },

    /// Attempted to add an asset with a key that already exists
    #[error("Duplicate asset key: {key}")]
    DuplicateKey {
        /// The duplicate key
        key: String,
    },

    /// Validation error that prevents further processing
    #[error("Validation error: {message}")]
    ValidationError {
        /// Description of the validation failure
        message: String,
    },

    /// Error parsing a segment at a specific offset
    #[error("Segment parse error at offset {offset}: {message}")]
    SegmentParseError {
        /// Byte offset where the error occurred
        offset: u64,
        /// Description of the parse error
        message: String,
    },

    /// Required structure definition not found
    #[error("Structure definition not found: {name}")]
    DefinitionNotFound {
        /// Name of the missing definition
        name: String,
    },

    /// I/O error during file operations
    #[error("IO error: {source}")]
    IoError {
        /// The underlying I/O error
        #[from]
        source: std::io::Error,
    },

    /// Error from the binary parser
    #[error("Parser error: {0}")]
    ParserError(#[from] AccessError),
}

impl From<JBPError> for CodecError {
    fn from(err: JBPError) -> Self {
        match err {
            JBPError::InvalidFormat { message } => CodecError::InvalidFormat(message),
            JBPError::AssetNotFound { key } => CodecError::AssetNotFound(key),
            JBPError::DuplicateKey { key } => CodecError::DuplicateKey(key),
            JBPError::ValidationError { message } => CodecError::Parse(message),
            JBPError::SegmentParseError { offset, message } => {
                CodecError::Parse(format!("Segment error at offset {}: {}", offset, message))
            }
            JBPError::DefinitionNotFound { name } => {
                CodecError::InvalidFormat(format!("Structure definition not found: {}", name))
            }
            JBPError::IoError { source } => CodecError::Io(source),
            JBPError::ParserError(err) => CodecError::Parse(err.to_string()),
        }
    }
}

/// A validation warning collected during parsing.
///
/// Warnings represent issues that don't prevent parsing from continuing,
/// but indicate potential problems with the file.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// Warning code for programmatic handling
    pub code: ValidationCode,
    /// Human-readable message describing the warning
    pub message: String,
    /// Field path where the warning occurred (if applicable)
    pub field: Option<String>,
    /// Expected value (if applicable)
    pub expected: Option<String>,
    /// Actual value found
    pub actual: Option<String>,
}

impl ValidationWarning {
    /// Create a new validation warning.
    pub fn new(code: ValidationCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
            expected: None,
            actual: None,
        }
    }

    /// Set the field path where the warning occurred.
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set the expected value.
    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Set the actual value found.
    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(ref field) = self.field {
            write!(f, " (field: {})", field)?;
        }
        if let (Some(ref expected), Some(ref actual)) = (&self.expected, &self.actual) {
            write!(f, " [expected: {}, actual: {}]", expected, actual)?;
        }
        Ok(())
    }
}

/// Validation warning codes for programmatic handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationCode {
    /// File length (FL field) doesn't match actual file size
    FileLengthMismatch,
    /// Complexity level (CLEVEL) is outside valid range
    InvalidComplexityLevel,
    /// Segment count doesn't match number of segment info entries
    SegmentCountMismatch,
}

impl std::fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationCode::FileLengthMismatch => write!(f, "FILE_LENGTH_MISMATCH"),
            ValidationCode::InvalidComplexityLevel => write!(f, "INVALID_COMPLEXITY_LEVEL"),
            ValidationCode::SegmentCountMismatch => write!(f, "SEGMENT_COUNT_MISMATCH"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jbp_error_invalid_format_display() {
        let err = JBPError::InvalidFormat {
            message: "bad magic number".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid NITF format: bad magic number");
    }

    #[test]
    fn jbp_error_asset_not_found_display() {
        let err = JBPError::AssetNotFound {
            key: "image_segment_0".to_string(),
        };
        assert_eq!(err.to_string(), "Asset not found: image_segment_0");
    }

    #[test]
    fn jbp_error_duplicate_key_display() {
        let err = JBPError::DuplicateKey {
            key: "image_segment_0".to_string(),
        };
        assert_eq!(err.to_string(), "Duplicate asset key: image_segment_0");
    }

    #[test]
    fn jbp_error_validation_error_display() {
        let err = JBPError::ValidationError {
            message: "segment count mismatch".to_string(),
        };
        assert_eq!(err.to_string(), "Validation error: segment count mismatch");
    }

    #[test]
    fn jbp_error_segment_parse_error_display() {
        let err = JBPError::SegmentParseError {
            offset: 1024,
            message: "invalid subheader".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Segment parse error at offset 1024: invalid subheader"
        );
    }

    #[test]
    fn jbp_error_to_codec_error_invalid_format() {
        let jbp_err = JBPError::InvalidFormat {
            message: "test".to_string(),
        };
        let codec_err: CodecError = jbp_err.into();
        assert!(matches!(codec_err, CodecError::InvalidFormat(_)));
    }

    #[test]
    fn jbp_error_to_codec_error_asset_not_found() {
        let jbp_err = JBPError::AssetNotFound {
            key: "test_key".to_string(),
        };
        let codec_err: CodecError = jbp_err.into();
        assert!(matches!(codec_err, CodecError::AssetNotFound(_)));
    }

    #[test]
    fn jbp_error_to_codec_error_duplicate_key() {
        let jbp_err = JBPError::DuplicateKey {
            key: "test_key".to_string(),
        };
        let codec_err: CodecError = jbp_err.into();
        assert!(matches!(codec_err, CodecError::DuplicateKey(_)));
    }

    #[test]
    fn validation_warning_new() {
        let warning = ValidationWarning::new(
            ValidationCode::InvalidComplexityLevel,
            "CLEVEL 99 is not valid",
        );
        assert_eq!(warning.code, ValidationCode::InvalidComplexityLevel);
        assert_eq!(warning.message, "CLEVEL 99 is not valid");
        assert!(warning.field.is_none());
        assert!(warning.expected.is_none());
        assert!(warning.actual.is_none());
    }

    #[test]
    fn validation_warning_with_builders() {
        let warning = ValidationWarning::new(ValidationCode::FileLengthMismatch, "File length mismatch")
            .with_field("FL")
            .with_expected("1000")
            .with_actual("900");

        assert_eq!(warning.field, Some("FL".to_string()));
        assert_eq!(warning.expected, Some("1000".to_string()));
        assert_eq!(warning.actual, Some("900".to_string()));
    }

    #[test]
    fn validation_warning_display_basic() {
        let warning = ValidationWarning::new(
            ValidationCode::InvalidComplexityLevel,
            "Invalid CLEVEL",
        );
        assert_eq!(
            warning.to_string(),
            "INVALID_COMPLEXITY_LEVEL: Invalid CLEVEL"
        );
    }

    #[test]
    fn validation_warning_display_with_field() {
        let warning = ValidationWarning::new(ValidationCode::FileLengthMismatch, "Mismatch")
            .with_field("FL");
        assert_eq!(
            warning.to_string(),
            "FILE_LENGTH_MISMATCH: Mismatch (field: FL)"
        );
    }

    #[test]
    fn validation_warning_display_full() {
        let warning = ValidationWarning::new(ValidationCode::FileLengthMismatch, "Mismatch")
            .with_field("FL")
            .with_expected("1000")
            .with_actual("900");
        assert_eq!(
            warning.to_string(),
            "FILE_LENGTH_MISMATCH: Mismatch (field: FL) [expected: 1000, actual: 900]"
        );
    }

    #[test]
    fn validation_code_display() {
        assert_eq!(
            ValidationCode::FileLengthMismatch.to_string(),
            "FILE_LENGTH_MISMATCH"
        );
        assert_eq!(
            ValidationCode::InvalidComplexityLevel.to_string(),
            "INVALID_COMPLEXITY_LEVEL"
        );
        assert_eq!(
            ValidationCode::SegmentCountMismatch.to_string(),
            "SEGMENT_COUNT_MISMATCH"
        );
    }
}
