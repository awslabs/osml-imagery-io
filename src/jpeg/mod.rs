//! JPEG DCT codec support.
//!
//! This module provides JPEG DCT encoding and decoding capabilities for
//! JPEG files with IC=C3 (JPEG DCT), IC=M3 (Masked JPEG DCT), and IC=I1
//! (Downsampled JPEG).
//!
//! # Architecture
//!
//! The module uses FFI bindings to libjpeg-turbo for high-performance JPEG
//! encoding and decoding. The turbojpeg API is used for 8-bit images, while
//! the lower-level libjpeg API is used for 12-bit extended JPEG support.
//!
//! # Feature Flags
//!
//! - `libjpeg-turbo` (default): Enables the libjpeg-turbo-based codec implementation
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jpeg::{JpegCodec, JpegBlockDecoder, JpegBlockEncoder};
//!
//! let codec = JpegCodec::new();
//! let decoder = JpegBlockDecoder::new(...)?;
//! let pixels = decoder.decode_block(&jpeg_data)?;
//! ```

// Codec types (always available for API compatibility)
mod codec;
pub mod comrat;

// Standalone reader/writer components
pub(crate) mod image;
pub(crate) mod metadata;
pub(crate) mod reader;
pub(crate) mod writer;

pub use codec::{JpegCodec, JpegCodecCapabilities};
pub use comrat::JpegComrat;

// Standalone reader/writer types
pub use image::JPEGImageAssetProvider;
#[cfg(feature = "libjpeg-turbo")]
pub use reader::JPEGDatasetReader;
#[cfg(feature = "libjpeg-turbo")]
pub use writer::JPEGDatasetWriter;

// FFI bindings (feature-gated)
#[cfg(feature = "libjpeg-turbo")]
mod sys;

#[cfg(feature = "libjpeg-turbo")]
pub(crate) mod ffi;
