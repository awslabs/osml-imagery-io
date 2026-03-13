//! TIFF tag constants, compression types, sample formats, and planar configurations.
//!
//! Named constants for standard TIFF tags to avoid magic numbers throughout the codebase.
//! Values are from the TIFF 6.0 specification.

#![allow(dead_code)]

// =============================================================================
// Standard TIFF Tags
// =============================================================================

/// Tag 254: A general indication of the kind of data in this subfile.
pub const NEW_SUBFILE_TYPE: u32 = 254;

/// Tag 256: The number of columns (pixels per row) in the image.
pub const IMAGE_WIDTH: u32 = 256;

/// Tag 257: The number of rows in the image.
pub const IMAGE_LENGTH: u32 = 257;

/// Tag 258: Number of bits per component (sample).
pub const BITS_PER_SAMPLE: u32 = 258;

/// Tag 259: Compression scheme used on the image data.
pub const COMPRESSION: u32 = 259;

/// Tag 262: The color space of the image data.
pub const PHOTOMETRIC_INTERPRETATION: u32 = 262;

/// Tag 277: The number of components (bands) per pixel.
pub const SAMPLES_PER_PIXEL: u32 = 277;

/// Tag 278: The number of rows per strip.
pub const ROWS_PER_STRIP: u32 = 278;

/// Tag 273: Byte offsets of each strip.
pub const STRIP_OFFSETS: u32 = 273;

/// Tag 279: Byte counts of each strip (compressed size).
pub const STRIP_BYTE_COUNTS: u32 = 279;

/// Tag 284: How the components of each pixel are stored.
pub const PLANAR_CONFIGURATION: u32 = 284;

/// Tag 322: The tile width in pixels.
pub const TILE_WIDTH: u32 = 322;

/// Tag 323: The tile length (height) in pixels.
pub const TILE_LENGTH: u32 = 323;

/// Tag 324: Byte offsets of each tile.
pub const TILE_OFFSETS: u32 = 324;

/// Tag 325: Byte counts of each tile (compressed size).
pub const TILE_BYTE_COUNTS: u32 = 325;

/// Tag 339: Specifies how to interpret each data sample in a pixel.
pub const SAMPLE_FORMAT: u32 = 339;

// =============================================================================
// Compression Constants
// =============================================================================

/// No compression.
pub const COMPRESSION_NONE: u16 = 1;

/// LZW compression.
pub const COMPRESSION_LZW: u16 = 5;

/// Deflate/ZLib compression (TIFF 6.0 registered).
pub const COMPRESSION_DEFLATE: u16 = 8;

/// PackBits compression (Macintosh RLE).
pub const COMPRESSION_PACKBITS: u16 = 32773;

/// Adobe Deflate compression (older registration, same algorithm as DEFLATE).
pub const COMPRESSION_ADOBE_DEFLATE: u16 = 32946;

// =============================================================================
// Sample Format Constants
// =============================================================================

/// Unsigned integer data.
pub const SAMPLE_FORMAT_UINT: u16 = 1;

/// Two's complement signed integer data.
pub const SAMPLE_FORMAT_INT: u16 = 2;

/// IEEE floating point data.
pub const SAMPLE_FORMAT_FLOAT: u16 = 3;

// =============================================================================
// Photometric Interpretation Constants
// =============================================================================

/// Min value is black (grayscale).
pub const PHOTOMETRIC_MINISBLACK: u16 = 1;

/// RGB color model.
pub const PHOTOMETRIC_RGB: u16 = 2;

/// Palette color (indexed via color map).
pub const PHOTOMETRIC_PALETTE: u16 = 3;

// =============================================================================
// Planar Configuration Constants
// =============================================================================

/// Chunky format: pixel components are interleaved (RGBRGB...).
pub const PLANAR_CONFIG_CONTIG: u16 = 1;

/// Planar format: components are stored in separate planes (RRR...GGG...BBB...).
pub const PLANAR_CONFIG_SEPARATE: u16 = 2;
