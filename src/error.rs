//! Error types for the aws-osml-io crate.

use pyo3::exceptions::{PyIOError, PyIndexError, PyKeyError, PyValueError};
use pyo3::prelude::*;
use thiserror::Error;

/// Errors that can occur during image codec operations.
#[derive(Error, Debug)]
pub enum CodecError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported feature: {0}")]
    Unsupported(String),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Encode error: {0}")]
    Encode(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("Invalid block coordinates: row={0}, col={1}, level={2}")]
    InvalidBlockCoordinates(u32, u32, u32),

    #[error("Invalid resolution level: {0}")]
    InvalidResolutionLevel(u32),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Duplicate asset key: {0}")]
    DuplicateKey(String),

    // =========================================================================
    // J2K-specific error variants (Requirements 16.1, 16.2, 16.5)
    // =========================================================================

    /// Invalid resolution level with context about available levels.
    ///
    /// This error provides more context than `InvalidResolutionLevel` by
    /// including the number of available resolution levels.
    ///
    /// # Requirements
    /// - 16.5: Error SHALL include requested level and available levels
    #[error(
        "Invalid resolution level: requested level {requested}, but only {available} levels available (0-{max_valid})"
    )]
    InvalidResolutionLevelRange {
        /// The resolution level that was requested
        requested: u32,
        /// The number of available resolution levels
        available: u32,
        /// The maximum valid resolution level (available - 1)
        max_valid: u32,
    },

    /// JPEG 2000 decode error with byte offset context.
    ///
    /// This error provides detailed context for J2K decoding failures,
    /// including the codec error message and the byte offset where the
    /// error occurred (if known).
    ///
    /// # Requirements
    /// - 16.1: Error SHALL include codec error message and byte offset
    #[error("J2K decode error at byte offset {byte_offset}: {message}")]
    J2KDecode {
        /// The error message from the codec
        message: String,
        /// The byte offset in the codestream where the error occurred
        byte_offset: usize,
    },

    /// JPEG 2000 encode error with encoding parameters context.
    ///
    /// This error provides detailed context for J2K encoding failures,
    /// including the encoding parameters that were used and the reason
    /// for the failure.
    ///
    /// # Requirements
    /// - 16.2: Error SHALL include encoding parameters and failure reason
    #[error("J2K encode error: {reason} (params: {width}x{height}, {num_components} bands, {bits_per_component} bpp, lossless={lossless})")]
    J2KEncode {
        /// The reason for the encoding failure
        reason: String,
        /// Image width in pixels
        width: u32,
        /// Image height in pixels
        height: u32,
        /// Number of components (bands)
        num_components: u32,
        /// Bits per component
        bits_per_component: u8,
        /// Whether lossless encoding was requested
        lossless: bool,
    },
}

impl From<CodecError> for PyErr {
    fn from(err: CodecError) -> PyErr {
        match &err {
            CodecError::AssetNotFound(key) => PyKeyError::new_err(key.clone()),
            CodecError::DuplicateKey(key) => {
                PyValueError::new_err(format!("Duplicate key: {}", key))
            }
            CodecError::InvalidBlockCoordinates(r, c, l) => {
                PyIndexError::new_err(format!("Invalid block: row={}, col={}, level={}", r, c, l))
            }
            CodecError::InvalidResolutionLevel(l) => {
                PyValueError::new_err(format!("Invalid resolution level: {}", l))
            }
            CodecError::InvalidResolutionLevelRange {
                requested,
                available,
                max_valid,
            } => PyValueError::new_err(format!(
                "Invalid resolution level: requested level {}, but only {} levels available (0-{})",
                requested, available, max_valid
            )),
            CodecError::J2KDecode {
                message,
                byte_offset,
            } => PyIOError::new_err(format!(
                "J2K decode error at byte offset {}: {}",
                byte_offset, message
            )),
            CodecError::J2KEncode {
                reason,
                width,
                height,
                num_components,
                bits_per_component,
                lossless,
            } => PyIOError::new_err(format!(
                "J2K encode error: {} (params: {}x{}, {} bands, {} bpp, lossless={})",
                reason, width, height, num_components, bits_per_component, lossless
            )),
            CodecError::Parse(msg) => PyValueError::new_err(format!("Parse error: {}", msg)),
            _ => PyIOError::new_err(err.to_string()),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // J2K-specific error variant tests (Requirements 16.1, 16.2, 16.5)
    // =========================================================================

    #[test]
    fn test_invalid_resolution_level_range_error_message() {
        let err = CodecError::InvalidResolutionLevelRange {
            requested: 7,
            available: 6,
            max_valid: 5,
        };

        let msg = err.to_string();
        assert!(msg.contains("requested level 7"));
        assert!(msg.contains("6 levels available"));
        assert!(msg.contains("0-5"));
    }

    #[test]
    fn test_invalid_resolution_level_range_zero_available() {
        // Edge case: 0 available levels (shouldn't happen in practice)
        let err = CodecError::InvalidResolutionLevelRange {
            requested: 0,
            available: 0,
            max_valid: 0,
        };

        let msg = err.to_string();
        assert!(msg.contains("requested level 0"));
        assert!(msg.contains("0 levels available"));
    }

    #[test]
    fn test_j2k_decode_error_includes_byte_offset() {
        let err = CodecError::J2KDecode {
            message: "Invalid SOC marker".to_string(),
            byte_offset: 1024,
        };

        let msg = err.to_string();
        assert!(msg.contains("byte offset 1024"));
        assert!(msg.contains("Invalid SOC marker"));
    }

    #[test]
    fn test_j2k_decode_error_zero_offset() {
        let err = CodecError::J2KDecode {
            message: "Missing header".to_string(),
            byte_offset: 0,
        };

        let msg = err.to_string();
        assert!(msg.contains("byte offset 0"));
        assert!(msg.contains("Missing header"));
    }

    #[test]
    fn test_j2k_encode_error_includes_parameters() {
        let err = CodecError::J2KEncode {
            reason: "Tile encoding failed".to_string(),
            width: 1024,
            height: 768,
            num_components: 3,
            bits_per_component: 8,
            lossless: true,
        };

        let msg = err.to_string();
        assert!(msg.contains("Tile encoding failed"));
        assert!(msg.contains("1024x768"));
        assert!(msg.contains("3 bands"));
        assert!(msg.contains("8 bpp"));
        assert!(msg.contains("lossless=true"));
    }

    #[test]
    fn test_j2k_encode_error_lossy_mode() {
        let err = CodecError::J2KEncode {
            reason: "Compression ratio too high".to_string(),
            width: 512,
            height: 512,
            num_components: 1,
            bits_per_component: 16,
            lossless: false,
        };

        let msg = err.to_string();
        assert!(msg.contains("Compression ratio too high"));
        assert!(msg.contains("512x512"));
        assert!(msg.contains("1 bands"));
        assert!(msg.contains("16 bpp"));
        assert!(msg.contains("lossless=false"));
    }

    #[test]
    fn test_j2k_encode_error_high_bit_depth() {
        let err = CodecError::J2KEncode {
            reason: "Bit depth exceeds codec maximum".to_string(),
            width: 256,
            height: 256,
            num_components: 4,
            bits_per_component: 38,
            lossless: true,
        };

        let msg = err.to_string();
        assert!(msg.contains("38 bpp"));
        assert!(msg.contains("4 bands"));
    }

    // =========================================================================
    // Python error conversion tests
    // =========================================================================

    #[test]
    fn test_invalid_resolution_level_range_to_pyerr() {
        pyo3::prepare_freethreaded_python();

        let err = CodecError::InvalidResolutionLevelRange {
            requested: 10,
            available: 6,
            max_valid: 5,
        };

        Python::with_gil(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn test_j2k_decode_to_pyerr() {
        pyo3::prepare_freethreaded_python();

        let err = CodecError::J2KDecode {
            message: "Test error".to_string(),
            byte_offset: 100,
        };

        Python::with_gil(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyIOError>(py));
        });
    }

    #[test]
    fn test_j2k_encode_to_pyerr() {
        pyo3::prepare_freethreaded_python();

        let err = CodecError::J2KEncode {
            reason: "Test error".to_string(),
            width: 100,
            height: 100,
            num_components: 1,
            bits_per_component: 8,
            lossless: false,
        };

        Python::with_gil(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyIOError>(py));
        });
    }
}
