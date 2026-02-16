//! PyO3 Python bindings for the image IO API.
//!
//! This module contains wrapper types that expose Rust traits to Python.

pub mod asset;
pub mod data;
pub mod graphics;
pub mod image;
pub mod io;
pub mod metadata;
pub mod reader;
pub mod text;
pub mod writer;

pub use asset::PyAssetProvider;
pub use data::PyDataAssetProvider;
pub use graphics::PyGraphicsAssetProvider;
pub use image::PyImageAssetProvider;
pub use io::IO;
pub use metadata::PyMetadataProvider;
pub use reader::PyDatasetReader;
pub use text::PyTextAssetProvider;
pub use writer::PyDatasetWriter;
