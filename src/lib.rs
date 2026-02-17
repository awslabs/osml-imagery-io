//! AWS OSML IO - Geospatial image format codecs
//!
//! This crate provides Rust implementations of image format decoders and encoders
//! for geospatial imagery formats including NITF and GeoTIFF.

use pyo3::prelude::*;

mod bindings;
mod error;
pub mod parser;
mod traits;
mod types;

pub use bindings::{
    PyAssetProvider, PyDataAssetProvider, PyDatasetReader, PyDatasetWriter,
    PyGraphicsAssetProvider, PyImageAssetProvider, PyMetadataProvider, PyTextAssetProvider,
    PyStructureAccessor, PyStructureDefinition, PyStructureRegistry, PyStructureWriter, PyValue,
    IO,
};
pub use traits::{
    AssetProvider, DataAssetProvider, DatasetReader, DatasetWriter, GraphicsAssetProvider,
    ImageAssetProvider, MetadataProvider, TextAssetProvider,
};
pub use types::{AssetType, PixelType};

/// Python module for aws.osml.io._io
#[pymodule]
fn _io(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<AssetType>()?;
    m.add_class::<PixelType>()?;
    m.add_class::<PyMetadataProvider>()?;
    m.add_class::<PyAssetProvider>()?;
    m.add_class::<PyImageAssetProvider>()?;
    m.add_class::<PyTextAssetProvider>()?;
    m.add_class::<PyDataAssetProvider>()?;
    m.add_class::<PyGraphicsAssetProvider>()?;
    m.add_class::<PyDatasetReader>()?;
    m.add_class::<PyDatasetWriter>()?;
    m.add_class::<IO>()?;
    // Parser bindings
    m.add_class::<PyStructureRegistry>()?;
    m.add_class::<PyStructureAccessor>()?;
    m.add_class::<PyStructureWriter>()?;
    m.add_class::<PyStructureDefinition>()?;
    m.add_class::<PyValue>()?;
    Ok(())
}
