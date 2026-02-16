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
            CodecError::Parse(msg) => PyValueError::new_err(format!("Parse error: {}", msg)),
            _ => PyIOError::new_err(err.to_string()),
        }
    }
}
