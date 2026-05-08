//! DTED (Digital Terrain Elevation Data) format support.
//!
//! Provides read support for DTED Level 0, 1, and 2 elevation files
//! (`.dt0`, `.dt1`, `.dt2`) and auxiliary statistics files (`.avg`,
//! `.min`, `.max`). The binary format is defined by MIL-PRF-89020B.

mod image;
mod metadata;
mod reader;
mod records;

pub use image::DTEDImageAssetProvider;
pub use reader::DTEDDatasetReader;
