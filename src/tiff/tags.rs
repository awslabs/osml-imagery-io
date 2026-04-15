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

/// Tag 317: Predictor for compression pre-filtering.
pub const PREDICTOR: u32 = 317;

/// Tag 347: JPEGTables — shared JPEG quantization and Huffman tables.
/// Present in JPEG-compressed TIFFs (Compression=7). Contains SOI/EOI markers.
/// Individual JPEG tiles are not standalone JFIF files; this table data is
/// required to decode them.
pub const JPEG_TABLES: u32 = 347;

/// Tag 530: YCbCrSubSampling — chroma subsampling factors [horiz, vert].
/// Default is [2, 2]. Only meaningful when PhotometricInterpretation = YCbCr (6).
pub const YCBCR_SUB_SAMPLING: u32 = 530;


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

/// JPEG compression (TIFF Technical Note 2).
pub const COMPRESSION_JPEG: u16 = 7;

/// Adobe Deflate compression (older registration, same algorithm as DEFLATE).
pub const COMPRESSION_ADOBE_DEFLATE: u16 = 32946;

// =============================================================================
// libtiff Pseudo-Tags
// =============================================================================

/// Pseudo-tag for JPEG quality (1–100). Not a real TIFF tag — libtiff intercepts it.
pub const TIFFTAG_JPEGQUALITY: u32 = 65537;

/// Pseudo-tag for JPEG color mode. Controls YCbCr↔RGB conversion in libtiff.
pub const TIFFTAG_JPEGCOLORMODE: u32 = 65538;

/// JPEGCOLORMODE value: return raw colorspace data (no conversion).
#[allow(dead_code)]
pub const JPEGCOLORMODE_RAW: u32 = 0;

/// JPEGCOLORMODE value: convert YCbCr to RGB on read, RGB to YCbCr on write.
pub const JPEGCOLORMODE_RGB: u32 = 1;

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

/// YCbCr color space, required for JPEG-in-TIFF with RGB data.
pub const PHOTOMETRIC_YCBCR: u16 = 6;

// =============================================================================
// Planar Configuration Constants
// =============================================================================

/// Chunky format: pixel components are interleaved (RGBRGB...).
pub const PLANAR_CONFIG_CONTIG: u16 = 1;

/// Planar format: components are stored in separate planes (RRR...GGG...BBB...).
pub const PLANAR_CONFIG_SEPARATE: u16 = 2;

// =============================================================================
// GeoTIFF TIFF Tags
// =============================================================================

/// Tag 33550: ModelPixelScaleTag — pixel size in CRS units (3 DOUBLEs).
pub const MODEL_PIXEL_SCALE_TAG: u32 = 33550;

/// Tag 33922: ModelTiepointTag — pixel-to-CRS tiepoint tuples (N×6 DOUBLEs).
pub const MODEL_TIEPOINT_TAG: u32 = 33922;

/// Tag 34264: ModelTransformationTag — 4×4 affine transformation matrix (16 DOUBLEs).
pub const MODEL_TRANSFORMATION_TAG: u32 = 34264;

/// Tag 34735: GeoKeyDirectoryTag — GeoKey directory (SHORT array).
pub const GEO_KEY_DIRECTORY_TAG: u32 = 34735;

/// Tag 34736: GeoDoubleParamsTag — double-precision GeoKey parameters (DOUBLE array).
pub const GEO_DOUBLE_PARAMS_TAG: u32 = 34736;

/// Tag 34737: GeoAsciiParamsTag — ASCII GeoKey parameters (pipe-delimited string).
pub const GEO_ASCII_PARAMS_TAG: u32 = 34737;

// =============================================================================
// GeoKey ID Constants
// =============================================================================

/// GeoKey 1024: GTModelTypeGeoKey — coordinate model type.
pub const GT_MODEL_TYPE_GEO_KEY: u16 = 1024;

/// GeoKey 1025: GTRasterTypeGeoKey — raster space interpretation.
pub const GT_RASTER_TYPE_GEO_KEY: u16 = 1025;

/// GeoKey 2048: GeographicTypeGeoKey — geographic CRS EPSG code.
pub const GEOGRAPHIC_TYPE_GEO_KEY: u16 = 2048;

/// GeoKey 3072: ProjectedCSTypeGeoKey — projected CRS EPSG code.
pub const PROJECTED_CS_TYPE_GEO_KEY: u16 = 3072;

// =============================================================================
// GTModelTypeGeoKey Values
// =============================================================================

/// GTModelTypeGeoKey value 1: Projected coordinate system.
pub const MODEL_TYPE_PROJECTED: u16 = 1;

/// GTModelTypeGeoKey value 2: Geographic coordinate system.
pub const MODEL_TYPE_GEOGRAPHIC: u16 = 2;

// =============================================================================
// GTRasterTypeGeoKey Values
// =============================================================================

/// GTRasterTypeGeoKey value 1: Pixel represents an area.
pub const RASTER_PIXEL_IS_AREA: u16 = 1;

/// GTRasterTypeGeoKey value 2: Pixel represents a point.
pub const RASTER_PIXEL_IS_POINT: u16 = 2;

// =============================================================================
// TIFF 6.0 Field Type Constants (Section 2)
// =============================================================================

/// Field type 1: BYTE — 8-bit unsigned integer.
pub const TIFF_BYTE: u16 = 1;

/// Field type 2: ASCII — 8-bit byte containing a 7-bit ASCII code.
pub const TIFF_ASCII: u16 = 2;

/// Field type 3: SHORT — 16-bit unsigned integer.
pub const TIFF_SHORT: u16 = 3;

/// Field type 4: LONG — 32-bit unsigned integer.
pub const TIFF_LONG: u16 = 4;

/// Field type 5: RATIONAL — Two LONGs: numerator and denominator.
pub const TIFF_RATIONAL: u16 = 5;

/// Field type 6: SBYTE — 8-bit signed integer.
pub const TIFF_SBYTE: u16 = 6;

/// Field type 7: UNDEFINED — 8-bit byte (application-defined semantics).
pub const TIFF_UNDEFINED: u16 = 7;

/// Field type 8: SSHORT — 16-bit signed integer.
pub const TIFF_SSHORT: u16 = 8;

/// Field type 9: SLONG — 32-bit signed integer.
pub const TIFF_SLONG: u16 = 9;

/// Field type 10: SRATIONAL — Two SLONGs: signed numerator and denominator.
pub const TIFF_SRATIONAL: u16 = 10;

/// Field type 11: FLOAT — Single precision (4-byte) IEEE floating point.
pub const TIFF_FLOAT: u16 = 11;

/// Field type 12: DOUBLE — Double precision (8-byte) IEEE floating point.
pub const TIFF_DOUBLE: u16 = 12;

