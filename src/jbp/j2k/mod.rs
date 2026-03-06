//! JPEG 2000 codec support for JBP/NITF imagery.
//!
//! This module provides JPEG 2000 encoding and decoding capabilities for
//! NITF files with IC=C8 (JPEG 2000 Part 1) and IC=CD (HTJ2K Part 15).
//!
//! # Architecture
//!
//! The module uses a pluggable codec abstraction (`J2KCodec` trait) that allows
//! different backend implementations (OpenJPEG, nvJPEG2000, etc.) to be used
//! without changing application code.
//!
//! # Feature Flags
//!
//! - `openjpeg` (default): Enables the OpenJPEG-based codec implementation
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::j2k::{OpenJpegCodec, J2KCodec, J2KDecodeParams};
//!
//! let codec = OpenJpegCodec::new();
//! let params = J2KDecodeParams::default();
//! let result = codec.decode(&codestream, &params)?;
//! ```

// Codec trait and types (always available)
mod codec;
pub mod comrat;
mod decoder;
mod encoder;

pub use codec::{
    J2KCodec, J2KCodecCapabilities, J2KDecodeParams, J2KDecodeResult, J2KEncodeParams,
    J2KEncodeState,
};
pub use comrat::{generate_comrat, hints_to_comrat, J2KComrat, J2KEncodingHints};
pub use decoder::Jpeg2000BlockDecoder;
pub use encoder::Jpeg2000BlockEncoder;

// OpenJPEG implementation (feature-gated)
#[cfg(feature = "openjpeg")]
mod sys;

#[cfg(feature = "openjpeg")]
pub(crate) mod ffi;

#[cfg(feature = "openjpeg")]
mod openjpeg;

#[cfg(feature = "openjpeg")]
pub use openjpeg::{get_j2k_codec, OpenJpegCodec, OpenJpegEncodeState};
