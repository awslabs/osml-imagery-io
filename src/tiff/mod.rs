//! TIFF format support via libtiff FFI bindings.

pub(crate) mod ffi;
mod geotiff;
pub(crate) mod image;
mod metadata;
mod reader;
pub(crate) mod sys;
pub(crate) mod tags;
mod writer;

pub use image::TIFFImageAssetProvider;
pub use reader::TIFFDatasetReader;
pub use writer::TIFFDatasetWriter;
