//! TIFF format support via libtiff FFI bindings.

pub(crate) mod sys;
pub(crate) mod ffi;
pub(crate) mod tags;
mod geotiff;
pub(crate) mod image;
mod reader;
mod metadata;
mod writer;

pub use reader::TIFFDatasetReader;
pub use writer::TIFFDatasetWriter;
pub use image::TIFFImageAssetProvider;
