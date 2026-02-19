//! Image segment support for JBP (NITF/NSIF) files.
//!
//! This module provides parsing, validation, and writing of image subheaders,
//! along with reading and writing uncompressed imagery with single and multi-band support.
//!
//! # Key Components
//!
//! - [`types`] - Core enums for pixel types, image representation, interleave modes
//! - [`facade`] - Facade pattern over StructureAccessor for typed field access
//! - [`builder`] - Builder pattern for constructing image subheaders
//! - [`decoder`] - Strategy pattern for block decoders (uncompressed, JPEG2000, etc.)
//! - [`pixel`] - Pixel value encoding/decoding for all PVTYPE values
//! - [`interleave`] - Conversion between interleave modes (B, P, R, S)
//! - [`validation`] - Image subheader validation rules

pub mod builder;
pub mod decoder;
pub mod facade;
pub mod interleave;
pub mod pixel;
pub mod types;
pub mod validation;

pub use builder::{BandInfoBuilder, ImageSubheaderBuilder};
pub use decoder::{create_block_decoder, BlockDecoder, UncompressedBlockDecoder};
pub use facade::{BandInfoFacade, ImageSubheaderFacade};
pub use interleave::{convert, from_band_sequential, to_band_sequential};
pub use types::{ImageRepresentation, InterleaveMode, LookUpTable, PixelJustification, PixelValueType};
pub use validation::{ImageValidationCode, ImageValidationResult, ImageValidator, ValidationSeverity};
