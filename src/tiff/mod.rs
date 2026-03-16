//! TIFF format support via libtiff FFI bindings.

mod sys;
mod ffi;
mod tags;
mod geotiff;
mod image;
mod reader;
mod metadata;
mod writer;

pub use reader::TIFFDatasetReader;
pub use writer::TIFFDatasetWriter;
pub(crate) use image::TIFFImageAssetProvider;
