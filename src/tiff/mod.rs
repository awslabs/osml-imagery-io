//! TIFF format reading support via libtiff FFI bindings.

mod sys;
mod ffi;
mod tags;
mod image;
mod reader;
mod metadata;

pub use reader::TIFFDatasetReader;
pub(crate) use image::TIFFImageAssetProvider;
