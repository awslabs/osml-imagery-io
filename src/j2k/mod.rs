//! JPEG 2000 codec support.
//!
//! This module provides JPEG 2000 encoding and decoding capabilities,
//! supporting JPEG 2000 Part 1 (IC=C8) and HTJ2K Part 15 (IC=CD).
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
//! use osml_imagery_io::j2k::{OpenJpegCodec, J2KCodec, J2KDecodeParams};
//!
//! let codec = OpenJpegCodec::new();
//! let params = J2KDecodeParams::default();
//! let result = codec.decode(&codestream, &params)?;
//! ```

// Codec trait and types (always available)
mod codec;
pub mod comrat;
pub mod markers;

// Standalone reader/writer components
pub(crate) mod image;
pub(crate) mod metadata;
pub(crate) mod reader;
pub(crate) mod writer;

pub use codec::{
    J2KCodec, J2KCodecCapabilities, J2KDecodeParams, J2KDecodeResult, J2KEncodeParams,
    J2KEncodeState,
};
pub use comrat::{generate_comrat, hints_to_comrat, J2KComrat, J2KEncodingHints};

// Standalone reader/writer types
pub use image::J2KImageAssetProvider;
#[cfg(feature = "openjpeg")]
pub use reader::J2KDatasetReader;
#[cfg(feature = "openjpeg")]
pub use writer::J2KDatasetWriter;

// OpenJPEG implementation (feature-gated)
#[cfg(feature = "openjpeg")]
pub(crate) mod sys;

#[cfg(feature = "openjpeg")]
pub(crate) mod ffi;

#[cfg(feature = "openjpeg")]
mod openjpeg;

#[cfg(feature = "openjpeg")]
pub use openjpeg::{get_j2k_codec, OpenJpegCodec, OpenJpegEncodeState};
