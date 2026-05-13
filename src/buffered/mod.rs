//! In-memory buffered implementations of asset providers.
//!
//! This module provides convenience implementations that store data in memory,
//! useful for creating synthetic assets or copying/modifying existing assets.
//!
//! - [`BufferedMetadataProvider`] - Mutable metadata storage for encoding hints
//! - [`BufferedImageAssetProvider`] - In-memory image asset for synthetic images
//! - [`BufferedTextAssetProvider`] - In-memory text asset for text segments
//! - [`BufferedDataAssetProvider`] - In-memory data asset for DES segments

mod data;
mod image;
mod metadata;
mod text;

pub use data::BufferedDataAssetProvider;
pub use image::{BufferedImageAssetProvider, MemoryImageConfig};
pub use metadata::BufferedMetadataProvider;
pub use text::BufferedTextAssetProvider;
