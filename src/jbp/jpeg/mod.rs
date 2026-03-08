//! JPEG DCT codec support for JBP/NITF imagery.
//!
//! This module provides JPEG DCT encoding and decoding capabilities for
//! NITF files with IC=C3 (JPEG DCT), IC=M3 (Masked JPEG DCT), and IC=I1
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
//! use osml_imagery_io::jbp::jpeg::{JpegCodec, JpegBlockDecoder, JpegBlockEncoder};
//!
//! let codec = JpegCodec::new();
//! let decoder = JpegBlockDecoder::new(...)?;
//! let pixels = decoder.decode_block(&jpeg_data)?;
//! ```

// Codec types (always available for API compatibility)
mod codec;
pub mod comrat;
pub mod decoder;
mod encoder;

pub use codec::{JpegCodec, JpegCodecCapabilities};
pub use comrat::JpegComrat;
pub use decoder::{JpegBlockDecoder, JpegColorSpace};
pub use encoder::JpegBlockEncoder;

// FFI bindings (feature-gated)
#[cfg(feature = "libjpeg-turbo")]
mod sys;

#[cfg(feature = "libjpeg-turbo")]
pub(crate) mod ffi;
