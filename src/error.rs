//! Error types for the osml-imagery-io crate.

use pyo3::exceptions::{PyIOError, PyIndexError, PyKeyError, PyRuntimeError, PyValueError};
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

    // =========================================================================
    // Image masking error variants (Phase 6)
    // =========================================================================
    /// Block not found error for out-of-grid coordinates.
    ///
    /// This error is returned when attempting to access a block outside the valid
    /// block grid (row >= grid_rows or col >= grid_cols). Masked (absent) blocks
    /// within the grid are NOT errors — get_block() returns pad-pixel-filled data
    /// for those coordinates.
    ///
    /// # Requirements
    /// - 2.3: get_block() on out-of-grid coordinates SHALL raise BlockNotFound
    #[error("Block not found: row={row}, col={col}")]
    BlockNotFound {
        /// Block row index
        row: u32,
        /// Block column index
        col: u32,
    },

    /// Missing blocks error when non-masked IC is used with sparse data.
    ///
    /// This error is returned when attempting to write an image with a non-masked
    /// IC value (NC, C8, CD, etc.) but not all blocks have been provided.
    ///
    /// # Requirements
    /// - 7.2: Non-masked IC requires all blocks to be provided
    /// - 7.3: Raise MissingBlocks error with expected/provided counts
    #[error("Missing blocks for non-masked IC '{ic}': expected {expected} blocks, but only {provided} were provided")]
    MissingBlocks {
        /// Total number of blocks expected
        expected: u32,
        /// Number of blocks actually provided
        provided: u32,
        /// The IC value that was set
        ic: String,
    },

    /// Invalid mask table format error.
    ///
    /// This error is returned when parsing an Image Data Mask table that has
    /// an invalid or corrupt format.
    ///
    /// # Requirements
    /// - 1.1: Parse Image Data Mask table from masked image segments
    #[error("Invalid mask table: {reason}")]
    InvalidMaskTable {
        /// Description of why the mask table is invalid
        reason: String,
    },

    /// Python exception propagated through the callback adapter.
    ///
    /// This error is returned when a Python method called by
    /// `PyCallbackImageAssetProvider` raises an exception.
    ///
    /// # Requirements
    /// - 5.1: CodecError SHALL include a Python variant carrying the exception message
    #[error("Python error: {0}")]
    Python(String),
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
            CodecError::BlockNotFound { row, col } => {
                PyIndexError::new_err(format!("Block not found: row={}, col={}", row, col))
            }
            CodecError::MissingBlocks {
                expected,
                provided,
                ic,
            } => PyValueError::new_err(format!(
                "Missing blocks for non-masked IC '{}': expected {} blocks, but only {} were provided",
                ic, expected, provided
            )),
            CodecError::InvalidMaskTable { reason } => {
                PyValueError::new_err(format!("Invalid mask table: {}", reason))
            }
            CodecError::Python(msg) => {
                PyRuntimeError::new_err(format!("Python error: {}", msg))
            }
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
        Python::initialize();

        let err = CodecError::InvalidResolutionLevelRange {
            requested: 10,
            available: 6,
            max_valid: 5,
        };

        Python::attach(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn test_j2k_decode_to_pyerr() {
        Python::initialize();

        let err = CodecError::J2KDecode {
            message: "Test error".to_string(),
            byte_offset: 100,
        };

        Python::attach(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyIOError>(py));
        });
    }

    #[test]
    fn test_j2k_encode_to_pyerr() {
        Python::initialize();

        let err = CodecError::J2KEncode {
            reason: "Test error".to_string(),
            width: 100,
            height: 100,
            num_components: 1,
            bits_per_component: 8,
            lossless: false,
        };

        Python::attach(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyIOError>(py));
        });
    }

    // =========================================================================
    // CodecError::Python variant tests (Requirements 5.1, 5.4)
    // =========================================================================

    #[test]
    fn test_python_error_display_formatting() {
        let err = CodecError::Python("something went wrong".to_string());
        let msg = err.to_string();
        assert_eq!(msg, "Python error: something went wrong");
    }

    #[test]
    fn test_python_error_display_includes_message() {
        let err = CodecError::Python("custom callback failure".to_string());
        let msg = err.to_string();
        assert!(msg.contains("custom callback failure"));
        assert!(msg.starts_with("Python error: "));
    }

    #[test]
    fn test_python_error_to_pyerr_is_runtime_error() {
        Python::initialize();

        let err = CodecError::Python("test exception".to_string());

        Python::attach(|py| {
            let py_err: PyErr = err.into();
            assert!(py_err.is_instance_of::<PyRuntimeError>(py));
        });
    }

    #[test]
    fn test_python_error_to_pyerr_contains_message() {
        Python::initialize();

        let err = CodecError::Python("original error message".to_string());

        Python::attach(|py| {
            let py_err: PyErr = err.into();
            let msg = py_err.value(py).to_string();
            assert!(msg.contains("original error message"));
        });
    }
}
