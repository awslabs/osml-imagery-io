//! PyO3 Python bindings for the image IO API.
//!
//! This module contains wrapper types that expose Rust traits to Python.

pub mod asset;
pub mod buffered_image;
pub mod buffered_metadata;
pub mod data;
pub mod graphics;
pub mod image;
pub mod io;
pub mod metadata;
pub mod parser;
pub mod reader;
pub mod text;
pub mod writer;

pub use asset::PyAssetProvider;
pub use buffered_image::PyBufferedImageAssetProvider;
pub use buffered_metadata::PyBufferedMetadataProvider;
pub use data::PyDataAssetProvider;
pub use graphics::PyGraphicsAssetProvider;
pub use image::PyImageAssetProvider;
pub use io::IO;
pub use metadata::PyMetadataProvider;
pub use parser::{PyStructureAccessor, PyStructureDefinition, PyStructureRegistry, PyStructureWriter, PyValue};
pub use reader::PyDatasetReader;
pub use text::PyTextAssetProvider;
pub use writer::PyDatasetWriter;
