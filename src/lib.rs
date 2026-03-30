//! OSML Imagery IO - Geospatial image format codecs
//!
//! This crate provides Rust implementations of image format decoders and encoders
//! for geospatial imagery formats including NITF and GeoTIFF.

use pyo3::prelude::*;

mod bindings;
pub mod buffered;
mod error;
pub mod j2k;
pub mod jbp;
#[cfg(feature = "libjpeg-turbo")]
pub mod jpeg;
pub mod parser;
pub mod png;
#[cfg(feature = "libtiff")]
pub mod tiff;
mod traits;
mod types;

pub use bindings::{
    PyAssetProvider, PyDataAssetProvider, PyDatasetReader, PyDatasetWriter,
    PyGraphicsAssetProvider, PyImageAssetProvider, PyBufferedImageAssetProvider,
    PyBufferedTextAssetProvider, PyMetadataProvider, PyBufferedMetadataProvider,
    PyTextAssetProvider, PyStructureAccessor, PyStructureDefinition, PyStructureRegistry,
    PyStructureWriter, PyValue, IO,
};
pub use buffered::{BufferedImageAssetProvider, BufferedMetadataProvider, BufferedTextAssetProvider, MemoryImageConfig};
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
    m.add_class::<PyBufferedMetadataProvider>()?;
    m.add_class::<PyAssetProvider>()?;
    m.add_class::<PyImageAssetProvider>()?;
    m.add_class::<PyBufferedImageAssetProvider>()?;
    m.add_class::<PyTextAssetProvider>()?;
    m.add_class::<PyBufferedTextAssetProvider>()?;
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
    // Codec decode bindings
    #[cfg(feature = "openjpeg")]
    m.add_function(wrap_pyfunction!(bindings::codecs::decode_jpeg2000, m)?)?;
    #[cfg(feature = "libjpeg-turbo")]
    m.add_function(wrap_pyfunction!(bindings::codecs::decode_jpeg, m)?)?;
    m.add_function(wrap_pyfunction!(bindings::codecs::decode_jbp_block, m)?)?;
    Ok(())
}
