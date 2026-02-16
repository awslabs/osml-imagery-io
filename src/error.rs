//! Error types for the aws-osml-io crate.

use pyo3::exceptions::PyIOError;
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
}

impl From<CodecError> for PyErr {
    fn from(err: CodecError) -> PyErr {
        PyIOError::new_err(err.to_string())
    }
}
