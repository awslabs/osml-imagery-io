//! PyO3 Python bindings for the image IO API.
//!
//! This module contains wrapper types that expose Rust traits to Python.

pub mod asset;
pub mod data;
pub mod graphics;
pub mod image;
pub mod io;
pub mod memory_image;
pub mod metadata;
pub mod parser;
pub mod reader;
pub mod simple_metadata;
pub mod text;
pub mod writer;

pub use asset::PyAssetProvider;
pub use data::PyDataAssetProvider;
pub use graphics::PyGraphicsAssetProvider;
pub use image::PyImageAssetProvider;
pub use io::IO;
pub use memory_image::PyMemoryImageAssetProvider;
pub use metadata::PyMetadataProvider;
pub use parser::{PyStructureAccessor, PyStructureDefinition, PyStructureRegistry, PyStructureWriter, PyValue};
pub use reader::PyDatasetReader;
pub use simple_metadata::PySimpleMetadataProvider;
pub use text::PyTextAssetProvider;
pub use writer::PyDatasetWriter;
