//! AWS OSML IO - Geospatial image format codecs
//!
//! This crate provides Rust implementations of image format decoders and encoders
//! for geospatial imagery formats including NITF and GeoTIFF.

use pyo3::prelude::*;

mod error;

/// Python module for aws.osml.io._io
#[pymodule]
fn _io(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
