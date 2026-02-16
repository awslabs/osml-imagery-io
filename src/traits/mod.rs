//! Core traits defining the image IO API interfaces.
//!
//! This module contains the abstract trait definitions that format-specific
//! implementations will fulfill.

pub mod asset;
pub mod data;
pub mod graphics;
pub mod image;
pub mod metadata;
pub mod reader;
pub mod text;
pub mod writer;

pub use asset::AssetProvider;
pub use data::DataAssetProvider;
pub use graphics::GraphicsAssetProvider;
pub use image::ImageAssetProvider;
pub use metadata::MetadataProvider;
pub use reader::DatasetReader;
pub use text::TextAssetProvider;
pub use writer::DatasetWriter;
