//! In-memory buffered implementations of asset providers.
//!
//! This module provides convenience implementations that store data in memory,
//! useful for creating synthetic assets or copying/modifying existing assets.
//!
//! - [`BufferedMetadataProvider`] - Mutable metadata storage for encoding hints
//! - [`BufferedImageAssetProvider`] - In-memory image asset for synthetic images

mod image;
mod metadata;

pub use image::{BufferedImageAssetProvider, MemoryImageConfig};
pub use metadata::BufferedMetadataProvider;
