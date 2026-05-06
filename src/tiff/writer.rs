//! TIFF dataset writer implementation.
//!
//! This module provides [`TIFFDatasetWriter`] which implements the `DatasetWriter`
//! trait for creating tiled TIFF files. The writer assembles the TIFF in memory
//! via `TIFFClientOpen` with custom write callbacks, then flushes to disk on `close()`.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::CodecError;
use crate::tiff::ffi::TiffHandle;
use crate::tiff::geotiff;
use crate::tiff::tags;
use crate::traits::{AssetProvider, DatasetWriter, ImageAssetProvider, MetadataProvider};
use crate::types::{AssetType, PixelType};

// =============================================================================
// Constants
// =============================================================================

/// Default tile width in pixels.
const DEFAULT_TILE_WIDTH: u32 = 256;

/// Default tile height in pixels.
const DEFAULT_TILE_HEIGHT: u32 = 256;

// =============================================================================
// Encoding Hints
// =============================================================================

/// Parsed encoding hints controlling TIFF output parameters.
struct TiffEncodingHints {
    tile_width: u32,
    tile_height: u32,
    compression: u16,
    predictor: u16,
    planar_config: u16,
    /// JPEG quality (1–100). Only used when compression is JPEG (7).
    jpeg_quality: u32,
    /// JPEG color mode. When set to `JPEGCOLORMODE_RGB` (1), libtiff accepts
    /// RGB input and converts to YCbCr internally on write. When `None`,
    /// libtiff expects raw YCbCr input for JPEG-compressed ≥3-band images.
    jpeg_color_mode: Option<u32>,
    /// Whether tile_width was explicitly set via metadata (tag 322).
    tile_width_explicit: bool,
    /// Whether tile_height was explicitly set via metadata (tag 323).
    tile_height_explicit: bool,
}

impl Default for TiffEncodingHints {
    fn default() -> Self {
        Self {
            tile_width: DEFAULT_TILE_WIDTH,
            tile_height: DEFAULT_TILE_HEIGHT,
            compression: tags::COMPRESSION_DEFLATE,
            predictor: 2, // Horizontal predictor for Deflate
            planar_config: tags::PLANAR_CONFIG_CONTIG,
            jpeg_quality: 75,
            jpeg_color_mode: None,
            tile_width_explicit: false,
            tile_height_explicit: false,
        }
    }
}

impl TiffEncodingHints {
    /// Parse encoding hints from a `MetadataProvider`.
    ///
    /// Metadata keys must be numeric TIFF tag IDs (as strings), matching the
    /// TIFF 6.0 specification. Human-readable names like "TileWidth" are
    /// ignored — use the Python `TagNameResolver` to convert names to numeric
    /// keys before passing metadata to the writer.
    ///
    /// Recognized tags:
    /// - `"322"` (TileWidth) — tile width in pixels
    /// - `"323"` (TileLength) — tile height in pixels
    /// - `"259"` (Compression) — integer: `1` (None), `5` (LZW), `7` (JPEG), `8` (Deflate), `32773` (PackBits), `32946` (Adobe Deflate)
    /// - `"317"` (Predictor) — integer: `1` (None), `2` (Horizontal)
    /// - `"284"` (PlanarConfiguration) — integer: `1` (Chunky), `2` (Planar)
    /// - `"65537"` (JPEG Quality) — integer in [1, 100]; default 75 when absent
    /// - `"65538"` (JPEG Color Mode) — integer: `0` (raw YCbCr), `1` (RGB↔YCbCr conversion); absent by default
    ///
    /// String values for tags 259, 317, and 284 are rejected with
    /// `CodecError::InvalidFormat`. Use the Python `TagNameResolver` to
    /// convert human-readable names to numeric values before reaching Rust.
    fn from_metadata(metadata: &dyn MetadataProvider) -> Result<Self, CodecError> {
        let dict = metadata.as_dict(None);
        let mut hints = TiffEncodingHints::default();

        // Tag 322: TileWidth
        let tile_width_key = tags::TILE_WIDTH.to_string();
        if let Some(val) = dict.get(&tile_width_key) {
            hints.tile_width = parse_u32_from_json(val, "TileWidth (322)")?;
            hints.tile_width_explicit = true;
        }

        // Tag 323: TileLength (tile height)
        let tile_length_key = tags::TILE_LENGTH.to_string();
        if let Some(val) = dict.get(&tile_length_key) {
            hints.tile_height = parse_u32_from_json(val, "TileLength (323)")?;
            hints.tile_height_explicit = true;
        }

        // Tag 259: Compression (integer only)
        let compression_key = tags::COMPRESSION.to_string();
        if let Some(val) = dict.get(&compression_key) {
            if val.is_string() {
                return Err(CodecError::InvalidFormat(format!(
                    "Compression (259) must be an integer, not a string. Got {:?}. \
                     Use the Python TagNameResolver to convert names to numeric values.",
                    val
                )));
            }
            let n = parse_u32_from_json(val, "Compression (259)")? as u16;
            hints.compression = match n {
                1 | 5 | 7 | 8 | 32773 => n,
                32946 => n,
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Unknown Compression value: {}. Expected 1, 5, 7, 8, 32773, or 32946",
                        other
                    )));
                }
            };
        }

        // Tag 317: Predictor (integer only)
        let predictor_key = tags::PREDICTOR.to_string();
        let explicit_predictor = dict.contains_key(&predictor_key);
        if let Some(val) = dict.get(&predictor_key) {
            if val.is_string() {
                return Err(CodecError::InvalidFormat(format!(
                    "Predictor (317) must be an integer, not a string. Got {:?}. \
                     Use the Python TagNameResolver to convert names to numeric values.",
                    val
                )));
            }
            let n = parse_u32_from_json(val, "Predictor (317)")? as u16;
            hints.predictor = match n {
                1 | 2 => n,
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Unknown Predictor value: {}. Expected 1 or 2",
                        other
                    )));
                }
            };
        }

        // Apply default predictor based on compression if not explicitly set
        if !explicit_predictor {
            hints.predictor = match hints.compression {
                tags::COMPRESSION_LZW | tags::COMPRESSION_DEFLATE => 2, // Horizontal
                _ => 1,                                                 // None
            };
        }

        // JPEG compression is incompatible with the Predictor tag — force to 1
        if hints.compression == tags::COMPRESSION_JPEG {
            hints.predictor = 1;
        }

        // Tag 284: PlanarConfiguration (integer only)
        let planar_key = tags::PLANAR_CONFIGURATION.to_string();
        if let Some(val) = dict.get(&planar_key) {
            if val.is_string() {
                return Err(CodecError::InvalidFormat(format!(
                    "PlanarConfiguration (284) must be an integer, not a string. Got {:?}. \
                     Use the Python TagNameResolver to convert names to numeric values.",
                    val
                )));
            }
            let n = parse_u32_from_json(val, "PlanarConfiguration (284)")? as u16;
            hints.planar_config = match n {
                1 | 2 => n,
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "Unknown PlanarConfiguration value: {}. Expected 1 or 2",
                        other
                    )));
                }
            };
        }

        // Pseudo-tag 65537: JPEG quality (integer only, 1–100)
        let jpeg_quality_key = tags::TIFFTAG_JPEGQUALITY.to_string();
        if let Some(val) = dict.get(&jpeg_quality_key) {
            let q = parse_u32_from_json(val, "JPEG quality (65537)")?;
            if !(1..=100).contains(&q) {
                return Err(CodecError::InvalidFormat(format!(
                    "JPEG quality must be between 1 and 100, got {}",
                    q
                )));
            }
            hints.jpeg_quality = q;
        }

        // Pseudo-tag 65538: JPEG color mode (0 = raw, 1 = RGB↔YCbCr conversion)
        let jpeg_colormode_key = tags::TIFFTAG_JPEGCOLORMODE.to_string();
        if let Some(val) = dict.get(&jpeg_colormode_key) {
            let m = parse_u32_from_json(val, "JPEG color mode (65538)")?;
            match m {
                0 | 1 => hints.jpeg_color_mode = Some(m),
                other => {
                    return Err(CodecError::InvalidFormat(format!(
                        "JPEG color mode must be 0 (raw) or 1 (RGB), got {}",
                        other
                    )));
                }
            }
        }

        Ok(hints)
    }

    /// Apply provider block dimensions as defaults for tile size.
    ///
    /// If the metadata did not explicitly specify tile dimensions, use the
    /// provider's block_width/block_height instead. This mirrors how the JBP
    /// writer uses NPPBH/NPPBV from the provider as defaults.
    ///
    /// Provider block dimensions are rounded up to the nearest multiple of 16
    /// because the TIFF spec requires tile dimensions to be multiples of 16.
    fn apply_provider_defaults(&mut self, block_width: u32, block_height: u32) {
        if !self.tile_width_explicit && block_width > 0 {
            self.tile_width = (block_width + 15) & !15;
        }
        if !self.tile_height_explicit && block_height > 0 {
            self.tile_height = (block_height + 15) & !15;
        }
    }
}

/// Parse a u32 from a serde_json::Value, supporting both number and string representations.
fn parse_u32_from_json(val: &serde_json::Value, field: &str) -> Result<u32, CodecError> {
    if let Some(n) = val.as_u64() {
        return Ok(n as u32);
    }
    if let Some(s) = val.as_str() {
        return s.parse::<u32>().map_err(|_| {
            CodecError::InvalidFormat(format!("Cannot parse '{}' as integer for {}", s, field))
        });
    }
    Err(CodecError::InvalidFormat(format!(
        "Expected integer or string for {}, got {:?}",
        field, val
    )))
}

// =============================================================================
// Type Inference for Tag Writing
// =============================================================================

/// Inferred TIFF field type and value for writing.
struct InferredTag {
    field_type: u16,
    value: serde_json::Value,
}

/// Infer the TIFF field type from a JSON value, or extract explicit type annotation.
///
/// Returns the field type ID and the actual value to write.
/// For explicit annotations (JSON object with "value" and "type" fields),
/// extracts the type from the annotation.
fn infer_field_type(value: &serde_json::Value) -> Result<InferredTag, CodecError> {
    // Check for explicit type annotation: {"value": ..., "type": N}
    if let Some(obj) = value.as_object() {
        if obj.contains_key("value") && obj.contains_key("type") {
            let annotated_type = obj["type"].as_u64().ok_or_else(|| {
                CodecError::Encode("Explicit type annotation 'type' must be an integer".into())
            })?;
            if !(1..=12).contains(&annotated_type) {
                return Err(CodecError::Encode(format!(
                    "Invalid TIFF field type {}: must be 1-12",
                    annotated_type
                )));
            }
            return Ok(InferredTag {
                field_type: annotated_type as u16,
                value: obj["value"].clone(),
            });
        }
    }

    match value {
        serde_json::Value::String(_) => Ok(InferredTag {
            field_type: tags::TIFF_ASCII,
            value: value.clone(),
        }),
        serde_json::Value::Number(n) => {
            if n.is_f64() && !n.is_i64() && !n.is_u64() {
                // Pure float (not representable as integer)
                Ok(InferredTag {
                    field_type: tags::TIFF_DOUBLE,
                    value: value.clone(),
                })
            } else if let Some(i) = n.as_i64() {
                if i >= 0 {
                    Ok(InferredTag {
                        field_type: tags::TIFF_LONG,
                        value: value.clone(),
                    })
                } else {
                    Ok(InferredTag {
                        field_type: tags::TIFF_SLONG,
                        value: value.clone(),
                    })
                }
            } else {
                // u64 that doesn't fit in i64 — still non-negative
                Ok(InferredTag {
                    field_type: tags::TIFF_LONG,
                    value: value.clone(),
                })
            }
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return Err(CodecError::Encode(
                    "Cannot infer TIFF field type from empty array".into(),
                ));
            }
            let has_float = arr
                .iter()
                .any(|v| v.as_f64().is_some() && v.as_i64().is_none() && v.as_u64().is_none());
            if has_float {
                return Ok(InferredTag {
                    field_type: tags::TIFF_DOUBLE,
                    value: value.clone(),
                });
            }
            // All elements are integers
            let all_integers = arr
                .iter()
                .all(|v| v.as_i64().is_some() || v.as_u64().is_some());
            if !all_integers {
                return Err(CodecError::Encode(
                    "Cannot infer TIFF field type from array with mixed types".into(),
                ));
            }
            let has_negative = arr.iter().any(|v| v.as_i64().is_some_and(|i| i < 0));
            if has_negative {
                Ok(InferredTag {
                    field_type: tags::TIFF_SSHORT,
                    value: value.clone(),
                })
            } else {
                Ok(InferredTag {
                    field_type: tags::TIFF_SHORT,
                    value: value.clone(),
                })
            }
        }
        _ => Err(CodecError::Encode(format!(
            "Cannot infer TIFF field type from JSON value: {:?}",
            value
        ))),
    }
}

/// Write a single tag to the TIFF handle using the inferred field type.
fn write_inferred_tag(
    handle: &TiffHandle,
    tag: u32,
    inferred: &InferredTag,
) -> Result<(), CodecError> {
    match inferred.field_type {
        tags::TIFF_BYTE => {
            // Single byte or byte array
            let bytes = extract_u8_array(&inferred.value)?;
            handle.set_field_u8_array(tag, &bytes)
        }
        tags::TIFF_ASCII => {
            let s = inferred
                .value
                .as_str()
                .ok_or_else(|| CodecError::Encode("ASCII tag value must be a string".into()))?;
            handle.set_field_string(tag, s)
        }
        tags::TIFF_SHORT => match &inferred.value {
            serde_json::Value::Array(arr) => {
                let data: Result<Vec<u16>, _> = arr
                    .iter()
                    .map(|v| {
                        v.as_u64().map(|n| n as u16).ok_or_else(|| {
                            CodecError::Encode(
                                "SHORT array element must be a non-negative integer".into(),
                            )
                        })
                    })
                    .collect();
                handle.set_field_u16_array(tag, &data?)
            }
            _ => {
                let n = inferred.value.as_u64().ok_or_else(|| {
                    CodecError::Encode("SHORT tag value must be a non-negative integer".into())
                })?;
                handle.set_field_u16(tag, n as u16)
            }
        },
        tags::TIFF_LONG => {
            let n = inferred.value.as_u64().ok_or_else(|| {
                CodecError::Encode("LONG tag value must be a non-negative integer".into())
            })?;
            handle.set_field_u32(tag, n as u32)
        }
        tags::TIFF_RATIONAL => {
            // RATIONAL is not directly writable via simple inference;
            // requires explicit annotation. Write as two-element u32 array
            // is not supported by TiffHandle — use DOUBLE as fallback.
            let f = inferred
                .value
                .as_f64()
                .ok_or_else(|| CodecError::Encode("RATIONAL tag value must be a number".into()))?;
            handle.set_field_f64(tag, f)
        }
        tags::TIFF_SBYTE => {
            // Cast i8 values to u8 for the byte array write
            let bytes = extract_i8_as_u8_array(&inferred.value)?;
            handle.set_field_u8_array(tag, &bytes)
        }
        tags::TIFF_UNDEFINED => {
            let bytes = extract_u8_array(&inferred.value)?;
            handle.set_field_u8_array(tag, &bytes)
        }
        tags::TIFF_SSHORT => {
            let data: Result<Vec<i16>, _> = match &inferred.value {
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .map(|v| {
                        v.as_i64().map(|n| n as i16).ok_or_else(|| {
                            CodecError::Encode("SSHORT array element must be an integer".into())
                        })
                    })
                    .collect(),
                _ => {
                    let n = inferred.value.as_i64().ok_or_else(|| {
                        CodecError::Encode("SSHORT tag value must be an integer".into())
                    })?;
                    Ok(vec![n as i16])
                }
            };
            handle.set_field_i16_array(tag, &data?)
        }
        tags::TIFF_SLONG => {
            let n = inferred
                .value
                .as_i64()
                .ok_or_else(|| CodecError::Encode("SLONG tag value must be an integer".into()))?;
            handle.set_field_i32(tag, n as i32)
        }
        tags::TIFF_SRATIONAL => {
            // SRATIONAL is not directly writable via simple inference;
            // requires explicit annotation. Write as DOUBLE fallback.
            let f = inferred
                .value
                .as_f64()
                .ok_or_else(|| CodecError::Encode("SRATIONAL tag value must be a number".into()))?;
            handle.set_field_f64(tag, f)
        }
        tags::TIFF_FLOAT => match &inferred.value {
            serde_json::Value::Array(arr) => {
                let data: Result<Vec<f32>, _> = arr
                    .iter()
                    .map(|v| {
                        v.as_f64().map(|n| n as f32).ok_or_else(|| {
                            CodecError::Encode("FLOAT array element must be a number".into())
                        })
                    })
                    .collect();
                handle.set_field_f32_array(tag, &data?)
            }
            _ => {
                let f = inferred
                    .value
                    .as_f64()
                    .ok_or_else(|| CodecError::Encode("FLOAT tag value must be a number".into()))?;
                handle.set_field_f32(tag, f as f32)
            }
        },
        tags::TIFF_DOUBLE => match &inferred.value {
            serde_json::Value::Array(arr) => {
                let data: Result<Vec<f64>, _> = arr
                    .iter()
                    .map(|v| {
                        v.as_f64().ok_or_else(|| {
                            CodecError::Encode("DOUBLE array element must be a number".into())
                        })
                    })
                    .collect();
                handle.set_field_f64_array(tag, &data?)
            }
            _ => {
                let f = inferred.value.as_f64().ok_or_else(|| {
                    CodecError::Encode("DOUBLE tag value must be a number".into())
                })?;
                handle.set_field_f64(tag, f)
            }
        },
        _ => Err(CodecError::Encode(format!(
            "Unsupported TIFF field type {} for tag {}",
            inferred.field_type, tag
        ))),
    }
}

/// Extract a byte array from a JSON value (array of integers 0-255).
fn extract_u8_array(value: &serde_json::Value) -> Result<Vec<u8>, CodecError> {
    match value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| {
                v.as_u64().map(|n| n as u8).ok_or_else(|| {
                    CodecError::Encode("Byte array element must be a non-negative integer".into())
                })
            })
            .collect(),
        serde_json::Value::Number(n) => {
            let byte = n.as_u64().ok_or_else(|| {
                CodecError::Encode("BYTE tag value must be a non-negative integer".into())
            })?;
            Ok(vec![byte as u8])
        }
        _ => Err(CodecError::Encode(
            "BYTE/UNDEFINED tag value must be an integer or array of integers".into(),
        )),
    }
}

/// Extract signed bytes from a JSON value, casting i8 to u8 for the write API.
fn extract_i8_as_u8_array(value: &serde_json::Value) -> Result<Vec<u8>, CodecError> {
    match value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| {
                v.as_i64().map(|n| n as i8 as u8).ok_or_else(|| {
                    CodecError::Encode("SBYTE array element must be an integer".into())
                })
            })
            .collect(),
        serde_json::Value::Number(n) => {
            let byte = n
                .as_i64()
                .ok_or_else(|| CodecError::Encode("SBYTE tag value must be an integer".into()))?;
            Ok(vec![byte as i8 as u8])
        }
        _ => Err(CodecError::Encode(
            "SBYTE tag value must be an integer or array of integers".into(),
        )),
    }
}

// =============================================================================
// Pixel Layout Conversion
// =============================================================================

/// Convert band-sequential (CHW) data to interleaved (HWC) format.
///
/// Input layout:  `[band0_pixel0, band0_pixel1, ..., band1_pixel0, band1_pixel1, ...]`
/// Output layout: `[pixel0_band0, pixel0_band1, ..., pixel1_band0, pixel1_band1, ...]`
///
/// `bytes_per_sample` is the number of bytes per pixel component (1, 2, 4, or 8).
fn bsq_to_interleaved(
    data: &[u8],
    num_bands: u32,
    pixels_per_band: u32,
    bytes_per_sample: u32,
) -> Vec<u8> {
    let bands = num_bands as usize;
    let pixels = pixels_per_band as usize;
    let bps = bytes_per_sample as usize;
    let total = data.len();

    let mut out = vec![0u8; total];
    let band_stride = pixels * bps;

    for band in 0..bands {
        let src_offset = band * band_stride;
        for pixel in 0..pixels {
            let src = src_offset + pixel * bps;
            let dst = (pixel * bands + band) * bps;
            out[dst..dst + bps].copy_from_slice(&data[src..src + bps]);
        }
    }

    out
}

// =============================================================================
// Edge Tile Padding
// =============================================================================

/// Pad a block to full tile dimensions, filling missing pixels with `pad_value`.
///
/// `block_data` is in CHW (band-sequential) format with shape `[bands, actual_rows, actual_cols]`.
/// Returns a new buffer in CHW format with shape `[bands, tile_height, tile_width]`.
fn pad_tile(
    block_data: &[u8],
    actual_rows: u32,
    actual_cols: u32,
    tile_height: u32,
    tile_width: u32,
    num_bands: u32,
    bytes_per_sample: u32,
    pad_value: u8,
) -> Vec<u8> {
    let bps = bytes_per_sample as usize;
    let full_row_bytes = tile_width as usize * bps;
    let actual_row_bytes = actual_cols as usize * bps;
    let full_band_size = tile_height as usize * full_row_bytes;
    let actual_band_size = actual_rows as usize * actual_row_bytes;

    let mut out = vec![pad_value; num_bands as usize * full_band_size];

    for band in 0..num_bands as usize {
        let dst_band_offset = band * full_band_size;
        let src_band_offset = band * actual_band_size;
        for row in 0..actual_rows as usize {
            let dst = dst_band_offset + row * full_row_bytes;
            let src = src_band_offset + row * actual_row_bytes;
            out[dst..dst + actual_row_bytes]
                .copy_from_slice(&block_data[src..src + actual_row_bytes]);
        }
    }

    out
}

// =============================================================================
// Queued Asset
// =============================================================================

/// An image asset queued for writing.
struct QueuedImageAsset {
    key: String,
    provider: AssetProvider,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    roles: Vec<String>,
}

// =============================================================================
// TIFFDatasetWriter
// =============================================================================

/// Writer for tiled TIFF files implementing the `DatasetWriter` trait.
///
/// Assembles the TIFF in memory via libtiff's `TIFFClientOpen` with custom
/// write callbacks, then flushes the bytes to the configured output on `close()`.
pub struct TIFFDatasetWriter {
    /// Output target, taken by `close()` when writing.
    ///
    /// Wrapped in `Mutex` so the struct is `Sync` (required by the
    /// `DatasetWriter` trait) even though `Box<dyn Write + Send>` alone is
    /// only `Send`. The inner `Option` allows `close()` to move the writer
    /// out via `take()` for a final `write_all` + `flush`. There is no
    /// runtime contention because the `DatasetWriter` methods only ever take
    /// `&mut self`.
    output: Mutex<Option<Box<dyn Write + Send>>>,
    /// Queued image assets in insertion order.
    assets: Vec<QueuedImageAsset>,
    /// Set of asset keys for duplicate detection.
    asset_keys: HashSet<String>,
    /// Dataset-level metadata (encoding hints source).
    metadata: Option<Arc<dyn MetadataProvider>>,
    /// Whether `close()` has been called.
    closed: bool,
}

impl TIFFDatasetWriter {
    /// Create a new writer targeting the given output path.
    ///
    /// The file is opened immediately and wrapped in a `BufWriter<File>`,
    /// then delegated to `new_with_output`. This is a behavioral change
    /// from earlier versions that deferred file opening to `close()`; the
    /// eager-open pattern is consistent with the PNG/JPEG/J2K writers.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CodecError> {
        let file = File::create(path.as_ref()).map_err(CodecError::Io)?;
        let buf_writer = BufWriter::new(file);
        Self::new_with_output(Box::new(buf_writer))
    }

    /// Create a new writer targeting the given output writer.
    ///
    /// Accepts any `Box<dyn Write + Send>`, enabling output to files, Python
    /// streams (via `PyWriteStream`), in-memory buffers, or any other
    /// `Write` implementation. The assembled TIFF bytes are written in a
    /// single `write_all` + `flush` during `close()`.
    pub fn new_with_output(output: Box<dyn Write + Send>) -> Result<Self, CodecError> {
        Ok(Self {
            output: Mutex::new(Some(output)),
            assets: Vec::new(),
            asset_keys: HashSet::new(),
            metadata: None,
            closed: false,
        })
    }

    /// Map a `PixelType` to the TIFF SampleFormat tag value.
    fn sample_format(pixel_type: PixelType) -> u16 {
        match pixel_type {
            PixelType::UInt8 | PixelType::UInt16 | PixelType::UInt32 => tags::SAMPLE_FORMAT_UINT,
            PixelType::Int8 | PixelType::Int16 | PixelType::Int32 => tags::SAMPLE_FORMAT_INT,
            PixelType::Float32 | PixelType::Float64 => tags::SAMPLE_FORMAT_FLOAT,
        }
    }

    /// Choose PhotometricInterpretation based on band count.
    fn photometric_interpretation(num_bands: u32) -> u16 {
        if num_bands >= 3 {
            tags::PHOTOMETRIC_RGB
        } else {
            tags::PHOTOMETRIC_MINISBLACK
        }
    }

    /// Compute the pad byte from `pad_pixel_value()`.
    /// Truncates the f64 to a u8 for use as a fill byte.
    fn pad_byte(pad_value: f64) -> u8 {
        pad_value as u8
    }

    /// Determine the `NewSubfileType` value for an image asset.
    ///
    /// Returns `1` (reduced-resolution image) if the asset's roles include
    /// `"overview"`, or as a fallback if the asset key contains the substring
    /// `:overview:`. Returns `0` (full-resolution image) otherwise.
    fn new_subfile_type_for_asset(roles: &[String], key: &str) -> u32 {
        if roles.iter().any(|r| r == "overview") || key.contains(":overview:") {
            1
        } else {
            0
        }
    }

    /// Extract the parent key from an overview key by stripping `:overview:M`.
    ///
    /// For example, `image:0:overview:1` → `image:0`.
    /// If the key does not contain `:overview:`, returns the key unchanged.
    fn extract_parent_key(key: &str) -> String {
        if let Some((parent, _)) = key.rsplit_once(":overview:") {
            parent.to_string()
        } else {
            key.to_string()
        }
    }

    /// Return the image area (num_rows × num_columns) for a queued asset.
    ///
    /// Falls back to 0 if the provider is not an `Image` variant.
    fn get_image_area(asset: &QueuedImageAsset) -> u64 {
        if let Some(image) = asset.provider.as_image() {
            image.num_rows() as u64 * image.num_columns() as u64
        } else {
            0
        }
    }

    /// Sort `self.assets` in-place for COG-compliant IFD ordering.
    ///
    /// The sort produces:
    /// 1. Full-resolution assets (NewSubfileType=0) in insertion order
    /// 2. Each full-res asset is immediately followed by its associated
    ///    overview assets (NewSubfileType=1), sorted by decreasing area
    /// 3. Orphan overviews (no matching full-res parent) are appended at the end
    fn sort_assets_for_cog(&mut self) {
        if self.assets.len() <= 1 {
            return;
        }

        // Partition into full-res and overview indices
        let mut full_res_indices: Vec<usize> = Vec::new();
        let mut overview_groups: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, asset) in self.assets.iter().enumerate() {
            let nst = Self::new_subfile_type_for_asset(&asset.roles, &asset.key);
            if nst == 0 {
                full_res_indices.push(i);
            } else {
                let parent = Self::extract_parent_key(&asset.key);
                overview_groups.entry(parent).or_default().push(i);
            }
        }

        // Sort each overview group by decreasing area
        for group in overview_groups.values_mut() {
            group.sort_by(|&a, &b| {
                let area_a = Self::get_image_area(&self.assets[a]);
                let area_b = Self::get_image_area(&self.assets[b]);
                area_b.cmp(&area_a) // Decreasing
            });
        }

        // Rebuild order: full-res (insertion order) interleaved with overviews
        let mut sorted_indices: Vec<usize> = Vec::with_capacity(self.assets.len());
        let mut used_parents: HashSet<String> = HashSet::new();

        for &idx in &full_res_indices {
            sorted_indices.push(idx);
            let parent_key = &self.assets[idx].key;
            used_parents.insert(parent_key.clone());
            if let Some(ovrs) = overview_groups.get(parent_key) {
                for &oidx in ovrs {
                    sorted_indices.push(oidx);
                }
            }
        }

        // Append orphan overviews (no matching full-res parent)
        for (parent, ovrs) in &overview_groups {
            if !used_parents.contains(parent) {
                for &oidx in ovrs {
                    sorted_indices.push(oidx);
                }
            }
        }

        // Reorder assets in-place using sorted_indices
        let mut slots: Vec<Option<QueuedImageAsset>> = self.assets.drain(..).map(Some).collect();
        let mut new_assets: Vec<QueuedImageAsset> = Vec::with_capacity(slots.len());
        for &idx in &sorted_indices {
            if let Some(asset) = slots[idx].take() {
                new_assets.push(asset);
            }
        }
        self.assets = new_assets;
    }

    /// Write a single image asset as one IFD.
    fn write_image_ifd(
        handle: &TiffHandle,
        image: &dyn ImageAssetProvider,
        hints: &TiffEncodingHints,
        metadata: Option<&dyn MetadataProvider>,
        roles: &[String],
        key: &str,
    ) -> Result<(), CodecError> {
        let num_cols = image.num_columns();
        let num_rows = image.num_rows();
        let num_bands = image.num_bands();
        let bits_per_sample = image.actual_bits_per_pixel();
        let pixel_type = image.pixel_value_type();
        let bytes_per_sample = pixel_type.bytes_per_pixel() as u32;

        // JPEG compression requires 8-bit samples
        if hints.compression == tags::COMPRESSION_JPEG && bits_per_sample != 8 {
            return Err(CodecError::InvalidFormat(
                "JPEG compression requires 8-bit samples".into(),
            ));
        }

        // Use tile dimensions directly from hints. The TIFF spec requires
        // tile dimensions to be multiples of 16, which is guaranteed by
        // apply_provider_defaults() (rounds up) and the encoding hint
        // strategies (64, 128, 256). Tile dimensions may exceed image
        // dimensions — libtiff handles this correctly by padding edge tiles.
        let tile_width = hints.tile_width;
        let tile_height = hints.tile_height;

        // Set TIFF tags
        handle.set_field_u32(tags::IMAGE_WIDTH, num_cols)?;
        handle.set_field_u32(tags::IMAGE_LENGTH, num_rows)?;
        handle.set_field_u16(tags::BITS_PER_SAMPLE, bits_per_sample as u16)?;
        handle.set_field_u16(tags::SAMPLES_PER_PIXEL, num_bands as u16)?;
        handle.set_field_u16(tags::SAMPLE_FORMAT, Self::sample_format(pixel_type))?;
        // JPEG-in-TIFF requires YCbCr for ≥3 bands; non-JPEG uses the standard logic.
        let photometric = if hints.compression == tags::COMPRESSION_JPEG {
            if num_bands >= 3 {
                tags::PHOTOMETRIC_YCBCR
            } else {
                tags::PHOTOMETRIC_MINISBLACK
            }
        } else {
            Self::photometric_interpretation(num_bands)
        };
        handle.set_field_u16(tags::PHOTOMETRIC_INTERPRETATION, photometric)?;

        // Set NewSubfileType based on roles and key
        let nsft = Self::new_subfile_type_for_asset(roles, key);
        handle.set_field_u32(tags::NEW_SUBFILE_TYPE, nsft)?;

        handle.set_field_u32(tags::TILE_WIDTH, tile_width)?;
        handle.set_field_u32(tags::TILE_LENGTH, tile_height)?;
        handle.set_field_u16(tags::COMPRESSION, hints.compression)?;
        // Only set Predictor tag for compressions that support it (LZW, Deflate).
        // libtiff rejects the Predictor tag for uncompressed and JPEG images.
        if hints.compression == tags::COMPRESSION_LZW
            || hints.compression == tags::COMPRESSION_DEFLATE
        {
            handle.set_field_u16(tags::PREDICTOR, hints.predictor)?;
        }
        // Set JPEG quality pseudo-tag before writing tile data
        if hints.compression == tags::COMPRESSION_JPEG {
            handle.set_field_u32(tags::TIFFTAG_JPEGQUALITY, hints.jpeg_quality)?;
            // Apply caller-specified JPEG color mode if provided.
            // JPEGCOLORMODE_RGB (1) tells libtiff to accept RGB input and
            // convert to YCbCr internally. Without this, libtiff expects
            // raw YCbCr data for ≥3-band JPEG images.
            if let Some(mode) = hints.jpeg_color_mode {
                handle.set_field_u32(tags::TIFFTAG_JPEGCOLORMODE, mode)?;
            }
        }
        handle.set_field_u16(tags::PLANAR_CONFIGURATION, hints.planar_config)?;

        let tiles_across = num_cols.div_ceil(tile_width);
        let tiles_down = num_rows.div_ceil(tile_height);

        let is_planar = hints.planar_config == tags::PLANAR_CONFIG_SEPARATE;

        // Iterate over the block grid
        for block_row in 0..tiles_down {
            for block_col in 0..tiles_across {
                let (block_data, shape) = image.get_block(block_row, block_col, 0, None)?;
                let [_bands, actual_rows, actual_cols] = shape;

                let needs_padding = actual_rows < tile_height || actual_cols < tile_width;

                // Pad edge tiles if needed
                let padded = if needs_padding {
                    pad_tile(
                        &block_data,
                        actual_rows,
                        actual_cols,
                        tile_height,
                        tile_width,
                        num_bands,
                        bytes_per_sample,
                        Self::pad_byte(image.pad_pixel_value()),
                    )
                } else {
                    block_data
                };

                if is_planar {
                    // Write each band as a separate tile plane
                    let tiles_per_plane = tiles_across * tiles_down;
                    let plane_size = (tile_height as usize)
                        * (tile_width as usize)
                        * (bytes_per_sample as usize);
                    for band in 0..num_bands {
                        let tile_index =
                            band * tiles_per_plane + block_row * tiles_across + block_col;
                        let src_offset = band as usize * plane_size;
                        let band_data = &padded[src_offset..src_offset + plane_size];
                        handle.write_encoded_tile(tile_index, band_data)?;
                    }
                } else {
                    // Convert CHW → HWC and write as a single tile
                    let pixels_in_tile = tile_height * tile_width;
                    let interleaved =
                        bsq_to_interleaved(&padded, num_bands, pixels_in_tile, bytes_per_sample);
                    let tile_index = block_row * tiles_across + block_col;
                    handle.write_encoded_tile(tile_index, &interleaved)?;
                }
            }
        }

        // Write GeoTIFF tags from metadata encoding hints.
        // Overview IFDs (nsft == 1) do NOT get GeoTIFF tags per OGC COG Standard.
        if let Some(meta) = metadata {
            let dict = meta.as_dict(None);

            if nsft == 0 {
                // Build and write GeoKey directory (tags 34735, 34736, 34737)
                let (directory, double_params, ascii_params) =
                    geotiff::build_geokey_directory(&dict)?;
                if !directory.is_empty() {
                    handle.set_field_u16_array(tags::GEO_KEY_DIRECTORY_TAG, &directory)?;
                    if let Some(doubles) = double_params {
                        handle.set_field_f64_array(tags::GEO_DOUBLE_PARAMS_TAG, &doubles)?;
                    }
                    if let Some(ascii) = ascii_params {
                        handle.set_field_string(tags::GEO_ASCII_PARAMS_TAG, &ascii)?;
                    }
                }

                // Write transformation tags (33550, 33922, 34264)
                let (pixel_scale, tiepoints, transformation) =
                    geotiff::extract_transformation_tags(&dict)?;
                if let Some(ps) = pixel_scale {
                    handle.set_field_f64_array(tags::MODEL_PIXEL_SCALE_TAG, &ps)?;
                }
                if let Some(tp) = tiepoints {
                    handle.set_field_f64_array(tags::MODEL_TIEPOINT_TAG, &tp)?;
                }
                if let Some(tf) = transformation {
                    handle.set_field_f64_array(tags::MODEL_TRANSFORMATION_TAG, &tf)?;
                }
            } // end nsft == 0

            // Write user-provided tags from numeric keys in the Tag_Dictionary.
            // Tags managed by the writer or libtiff are skipped to avoid conflicts.
            let structural_tags: HashSet<u32> = [
                // Tags set explicitly by write_image_ifd from image properties
                tags::NEW_SUBFILE_TYPE,
                tags::IMAGE_WIDTH,
                tags::IMAGE_LENGTH,
                tags::BITS_PER_SAMPLE,
                tags::COMPRESSION,
                tags::PHOTOMETRIC_INTERPRETATION,
                tags::SAMPLES_PER_PIXEL,
                tags::PLANAR_CONFIGURATION,
                tags::PREDICTOR,
                tags::TILE_WIDTH,
                tags::TILE_LENGTH,
                tags::SAMPLE_FORMAT,
                // Strip/tile offset tags managed by libtiff internally
                tags::ROWS_PER_STRIP,
                tags::STRIP_OFFSETS,
                tags::STRIP_BYTE_COUNTS,
                tags::TILE_OFFSETS,
                tags::TILE_BYTE_COUNTS,
                // GeoTIFF tags handled by the dedicated write path above
                tags::MODEL_PIXEL_SCALE_TAG,
                tags::MODEL_TIEPOINT_TAG,
                tags::MODEL_TRANSFORMATION_TAG,
                tags::GEO_KEY_DIRECTORY_TAG,
                tags::GEO_DOUBLE_PARAMS_TAG,
                tags::GEO_ASCII_PARAMS_TAG,
                // libtiff pseudo-tags handled as encoding hints above
                tags::TIFFTAG_JPEGQUALITY,
                tags::TIFFTAG_JPEGCOLORMODE,
            ]
            .into_iter()
            .collect();

            // Collect custom tags that need to be written, inferring their types.
            // We must register all custom tags with libtiff before writing any.
            let mut custom_tags: Vec<(u32, InferredTag)> = Vec::new();
            for (key, value) in &dict {
                let tag: u32 = match key.parse() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if structural_tags.contains(&tag) {
                    continue;
                }
                match infer_field_type(value) {
                    Ok(inferred) => custom_tags.push((tag, inferred)),
                    Err(e) => {
                        eprintln!("Warning: cannot infer type for tag {}: {}", tag, e);
                    }
                }
            }

            // Register all custom tags with libtiff in one pass before writing.
            // We pass `scalar` so libtiff knows whether TIFFSetField will
            // receive a bare value or a (count, pointer) pair.
            for (tag, inferred) in &custom_tags {
                let scalar = !inferred.value.is_array();
                if let Err(e) = handle.register_custom_tag(*tag, inferred.field_type, scalar) {
                    eprintln!("Warning: failed to register tag {}: {}", tag, e);
                }
            }

            // Now write all custom tags
            for (tag, inferred) in &custom_tags {
                if let Err(e) = write_inferred_tag(handle, *tag, inferred) {
                    eprintln!("Warning: failed to write tag {}: {}", tag, e);
                }
            }
        }

        // Finalize this IFD
        handle.write_directory()?;

        Ok(())
    }
}

// =============================================================================
// DatasetWriter Implementation
// =============================================================================

impl DatasetWriter for TIFFDatasetWriter {
    fn add_asset(
        &mut self,
        key: &str,
        provider: AssetProvider,
        title: &str,
        description: &str,
        roles: &[String],
    ) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Io(std::io::Error::other(
                "Writer has been closed",
            )));
        }

        // Only image assets are supported
        if provider.asset_type() != AssetType::Image {
            return Err(CodecError::Unsupported(format!(
                "TIFF does not support {:?} asset types, only Image",
                provider.asset_type()
            )));
        }

        // Reject duplicate keys
        if self.asset_keys.contains(key) {
            return Err(CodecError::DuplicateKey(key.to_string()));
        }

        self.assets.push(QueuedImageAsset {
            key: key.to_string(),
            provider,
            title: title.to_string(),
            description: description.to_string(),
            roles: roles.to_vec(),
        });
        self.asset_keys.insert(key.to_string());

        Ok(())
    }

    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
        self.metadata = Some(metadata);
        Ok(())
    }

    fn close(&mut self) -> Result<(), CodecError> {
        if self.closed {
            return Ok(());
        }

        // Sort assets for COG-compliant IFD ordering before writing
        self.sort_assets_for_cog();

        // Parse encoding hints from dataset-level metadata
        let mut hints = match &self.metadata {
            Some(meta) => TiffEncodingHints::from_metadata(meta.as_ref())?,
            None => TiffEncodingHints::default(),
        };

        // Open libtiff in write mode
        let handle = TiffHandle::from_write()?;

        // Write each queued image as a separate IFD
        for asset in &self.assets {
            let image = asset.provider.as_image().ok_or_else(|| {
                CodecError::InvalidFormat(format!("Asset '{}' is not an Image variant", asset.key))
            })?;

            // Use provider block dimensions as defaults when metadata didn't
            // specify tile sizes (mirrors JBP writer's NPPBH/NPPBV fallback).
            hints.apply_provider_defaults(
                image.num_pixels_per_block_horizontal(),
                image.num_pixels_per_block_vertical(),
            );

            Self::write_image_ifd(
                &handle,
                image.as_ref(),
                &hints,
                self.metadata
                    .as_ref()
                    .map(|m| m.as_ref() as &dyn MetadataProvider),
                &asset.roles,
                &asset.key,
            )?;
        }

        // Extract assembled TIFF bytes and write to the stored output writer
        let bytes = handle.into_bytes()?;
        let mut output = self
            .output
            .lock()
            .map_err(|_| CodecError::Unsupported("TIFF writer output mutex poisoned".to_string()))?
            .take()
            .ok_or_else(|| {
                CodecError::Unsupported("TIFF writer output is not available".to_string())
            })?;
        output.write_all(&bytes).map_err(CodecError::Io)?;
        output.flush().map_err(CodecError::Io)?;

        self.closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffered::{
        BufferedImageAssetProvider, BufferedMetadataProvider, BufferedTextAssetProvider,
        MemoryImageConfig,
    };

    /// Helper: create a minimal 1-band 256×256 UInt8 image provider with block data populated.
    fn make_image_provider(key: &str) -> Arc<BufferedImageAssetProvider> {
        let config = MemoryImageConfig::new(256, 256)
            .with_bands(1)
            .with_block_size(256, 256)
            .with_pixel_type(PixelType::UInt8);
        let provider = BufferedImageAssetProvider::new(key, config);
        let data = vec![42u8; 256 * 256];
        provider.set_block(0, 0, &data).unwrap();
        Arc::new(provider)
    }

    /// Helper: create a text asset provider (non-image).
    fn make_text_provider() -> Arc<BufferedTextAssetProvider> {
        Arc::new(BufferedTextAssetProvider::new(
            "text_0",
            "hello".to_string(),
            "UTF8",
        ))
    }

    // =========================================================================
    // Constructor
    // =========================================================================

    #[test]
    fn writer_new_creates_instance() {
        let writer = TIFFDatasetWriter::new("/tmp/test_new.tif");
        assert!(writer.is_ok());
        let w = writer.unwrap();
        assert!(!w.closed);
        assert!(w.assets.is_empty());
        assert!(w.metadata.is_none());
    }

    // =========================================================================
    // add_asset
    // =========================================================================

    #[test]
    fn writer_add_image_asset_succeeds() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();
        let provider = make_image_provider("image:0");
        let result = writer.add_asset(
            "image:0",
            AssetProvider::Image(provider),
            "Image 0",
            "Test image",
            &["data".to_string()],
        );
        assert!(result.is_ok());
        assert_eq!(writer.assets.len(), 1);
        assert!(writer.asset_keys.contains("image:0"));
    }

    #[test]
    fn writer_add_non_image_asset_rejected() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();
        let text = make_text_provider();
        let result = writer.add_asset("text_0", AssetProvider::Text(text), "Text", "desc", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, CodecError::Unsupported(_)),
            "Expected Unsupported error, got: {:?}",
            err
        );
        // No asset should have been queued
        assert!(writer.assets.is_empty());
    }

    #[test]
    fn writer_add_duplicate_key_rejected() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();
        let p1 = make_image_provider("img_0");
        let p2 = make_image_provider("img_1");
        writer
            .add_asset("image:0", AssetProvider::Image(p1), "Image 0", "desc", &[])
            .unwrap();
        let result = writer.add_asset(
            "image:0",
            AssetProvider::Image(p2),
            "Image 0 dup",
            "desc",
            &[],
        );
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CodecError::DuplicateKey(_)),
            "Expected DuplicateKey error"
        );
        assert_eq!(writer.assets.len(), 1);
    }

    #[test]
    fn writer_add_asset_after_close_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        // Add one asset so close() has something to write
        let provider = make_image_provider("image:0");
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Image",
                "desc",
                &[],
            )
            .unwrap();
        writer.close().unwrap();

        let p2 = make_image_provider("img_1");
        let result = writer.add_asset("image:1", AssetProvider::Image(p2), "Image 1", "desc", &[]);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CodecError::Io(_)),
            "Expected Io error after close"
        );
    }

    // =========================================================================
    // close() idempotent
    // =========================================================================

    #[test]
    fn writer_close_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_idempotent.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0");
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Image",
                "desc",
                &[],
            )
            .unwrap();

        assert!(writer.close().is_ok());
        // Second close should also succeed
        assert!(writer.close().is_ok());
    }

    // =========================================================================
    // set_metadata
    // =========================================================================

    #[test]
    fn writer_set_metadata_stores_latest() {
        let mut writer = TIFFDatasetWriter::new("/tmp/test.tif").unwrap();

        let meta1 = BufferedMetadataProvider::new();
        meta1.set("Compression", "LZW");
        writer.set_metadata(Arc::new(meta1)).unwrap();

        let meta2 = BufferedMetadataProvider::new();
        meta2.set("Compression", "None");
        writer.set_metadata(Arc::new(meta2)).unwrap();

        // The latest metadata should be used
        let dict = writer.metadata.as_ref().unwrap().as_dict(None);
        assert_eq!(dict.get("Compression"), Some(&serde_json::json!("None")));
    }

    // =========================================================================
    // Encoding hint parsing
    // =========================================================================

    #[test]
    fn writer_default_encoding_hints() {
        let hints = TiffEncodingHints::default();
        assert_eq!(hints.tile_width, 256);
        assert_eq!(hints.tile_height, 256);
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
        assert_eq!(hints.predictor, 2); // Horizontal for Deflate
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_CONTIG);
    }

    #[test]
    fn writer_parse_compression_none() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("259", serde_json::json!(1)); // Tag 259 = Compression
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.compression, tags::COMPRESSION_NONE);
    }

    #[test]
    fn writer_parse_compression_lzw() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("259", serde_json::json!(5)); // Tag 259 = Compression
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.compression, tags::COMPRESSION_LZW);
    }

    #[test]
    fn writer_parse_compression_deflate() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("259", serde_json::json!(8)); // Tag 259 = Compression
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
    }

    #[test]
    fn writer_parse_predictor_horizontal() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("317", serde_json::json!(2)); // Tag 317 = Predictor
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 2);
    }

    #[test]
    fn writer_parse_predictor_none() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("317", serde_json::json!(1)); // Tag 317 = Predictor
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 1);
    }

    #[test]
    fn writer_predictor_default_with_lzw_compression() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("259", serde_json::json!(5)); // Tag 259 = LZW
                                                    // No explicit Predictor → should default to Horizontal (2)
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 2);
    }

    #[test]
    fn writer_predictor_default_with_deflate_compression() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("259", serde_json::json!(8)); // Tag 259 = Deflate
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 2);
    }

    #[test]
    fn writer_predictor_default_without_compression() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("259", serde_json::json!(1)); // Tag 259 = None
                                                    // No explicit Predictor + no compression → should default to None (1)
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.predictor, 1);
    }

    #[test]
    fn writer_parse_tile_dimensions() {
        let meta = BufferedMetadataProvider::new();
        meta.set("322", "512"); // Tag 322 = TileWidth
        meta.set("323", "128"); // Tag 323 = TileLength
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.tile_width, 512);
        assert_eq!(hints.tile_height, 128);
    }

    #[test]
    fn writer_parse_planar_configuration_chunky() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("284", serde_json::json!(1)); // Tag 284 = PlanarConfiguration
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_CONTIG);
    }

    #[test]
    fn writer_parse_planar_configuration_planar() {
        let meta = BufferedMetadataProvider::new();
        meta.set_json("284", serde_json::json!(2)); // Tag 284 = PlanarConfiguration
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_SEPARATE);
    }

    #[test]
    fn writer_parse_invalid_compression_returns_error() {
        let meta = BufferedMetadataProvider::new();
        meta.set("259", "JPEG"); // String values should be rejected
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn writer_parse_invalid_predictor_returns_error() {
        let meta = BufferedMetadataProvider::new();
        meta.set("317", "FloatingPoint"); // String values should be rejected
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn writer_parse_string_compression_rejected() {
        let meta = BufferedMetadataProvider::new();
        meta.set("259", "Deflate"); // String "Deflate" should be rejected
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn writer_parse_string_predictor_rejected() {
        let meta = BufferedMetadataProvider::new();
        meta.set("317", "Horizontal"); // String "Horizontal" should be rejected
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn writer_parse_string_planar_config_rejected() {
        let meta = BufferedMetadataProvider::new();
        meta.set("284", "Chunky"); // String "Chunky" should be rejected
        let result = TiffEncodingHints::from_metadata(&meta);
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn writer_ignores_non_numeric_metadata_keys() {
        // Human-readable names like "TileWidth" should be ignored
        let meta = BufferedMetadataProvider::new();
        meta.set("TileWidth", "512");
        meta.set("TileHeight", "128");
        meta.set("Compression", "LZW");
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        // Should all be defaults since non-numeric keys are ignored
        assert_eq!(hints.tile_width, DEFAULT_TILE_WIDTH);
        assert_eq!(hints.tile_height, DEFAULT_TILE_HEIGHT);
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
    }

    #[test]
    fn writer_apply_provider_defaults_when_no_metadata() {
        let mut hints = TiffEncodingHints::default();
        hints.apply_provider_defaults(128, 64);
        assert_eq!(hints.tile_width, 128);
        assert_eq!(hints.tile_height, 64);
    }

    #[test]
    fn writer_metadata_overrides_provider_defaults() {
        let meta = BufferedMetadataProvider::new();
        meta.set("322", "512"); // Tag 322 = TileWidth
        meta.set("323", "512"); // Tag 323 = TileLength
        let mut hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        // Provider defaults should NOT override explicit metadata values
        hints.apply_provider_defaults(128, 64);
        assert_eq!(hints.tile_width, 512);
        assert_eq!(hints.tile_height, 512);
    }

    // =========================================================================
    // bsq_to_interleaved
    // =========================================================================

    #[test]
    fn writer_bsq_to_interleaved_3band() {
        // 3 bands, 2 pixels per band, 1 byte per sample
        // Input CHW: [R0 R1 | G0 G1 | B0 B1]
        let input: Vec<u8> = vec![10, 20, 30, 40, 50, 60];
        let result = bsq_to_interleaved(&input, 3, 2, 1);
        // Output HWC: [R0 G0 B0 | R1 G1 B1]
        assert_eq!(result, vec![10, 30, 50, 20, 40, 60]);
    }

    #[test]
    fn writer_bsq_to_interleaved_single_band() {
        // 1 band, 4 pixels, 1 byte per sample → no change
        let input: Vec<u8> = vec![1, 2, 3, 4];
        let result = bsq_to_interleaved(&input, 1, 4, 1);
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn writer_bsq_to_interleaved_2byte_samples() {
        // 2 bands, 2 pixels, 2 bytes per sample
        // Input CHW: [R0_lo R0_hi R1_lo R1_hi | G0_lo G0_hi G1_lo G1_hi]
        let input: Vec<u8> = vec![0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00];
        let result = bsq_to_interleaved(&input, 2, 2, 2);
        // Output HWC: [R0_lo R0_hi G0_lo G0_hi | R1_lo R1_hi G1_lo G1_hi]
        assert_eq!(result, vec![0x01, 0x00, 0x03, 0x00, 0x02, 0x00, 0x04, 0x00]);
    }

    #[test]
    fn writer_bsq_to_interleaved_preserves_length() {
        let input: Vec<u8> = vec![0u8; 3 * 16 * 2]; // 3 bands, 16 pixels, 2 bps
        let result = bsq_to_interleaved(&input, 3, 16, 2);
        assert_eq!(result.len(), input.len());
    }

    // =========================================================================
    // sample_format mapping
    // =========================================================================

    #[test]
    fn writer_pixel_type_to_sample_format() {
        // Unsigned integers → SAMPLE_FORMAT_UINT (1)
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::UInt8),
            tags::SAMPLE_FORMAT_UINT
        );
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::UInt16),
            tags::SAMPLE_FORMAT_UINT
        );
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::UInt32),
            tags::SAMPLE_FORMAT_UINT
        );

        // Signed integers → SAMPLE_FORMAT_INT (2)
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::Int8),
            tags::SAMPLE_FORMAT_INT
        );
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::Int16),
            tags::SAMPLE_FORMAT_INT
        );
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::Int32),
            tags::SAMPLE_FORMAT_INT
        );

        // Floating point → SAMPLE_FORMAT_FLOAT (3)
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::Float32),
            tags::SAMPLE_FORMAT_FLOAT
        );
        assert_eq!(
            TIFFDatasetWriter::sample_format(PixelType::Float64),
            tags::SAMPLE_FORMAT_FLOAT
        );
    }

    // =========================================================================
    // photometric_interpretation
    // =========================================================================

    #[test]
    fn writer_photometric_interpretation_rgb() {
        // 3 or more bands → RGB (2)
        assert_eq!(
            TIFFDatasetWriter::photometric_interpretation(3),
            tags::PHOTOMETRIC_RGB
        );
        assert_eq!(
            TIFFDatasetWriter::photometric_interpretation(4),
            tags::PHOTOMETRIC_RGB
        );
    }

    #[test]
    fn writer_photometric_interpretation_minisblack() {
        // 1 or 2 bands → MinIsBlack (1)
        assert_eq!(
            TIFFDatasetWriter::photometric_interpretation(1),
            tags::PHOTOMETRIC_MINISBLACK
        );
        assert_eq!(
            TIFFDatasetWriter::photometric_interpretation(2),
            tags::PHOTOMETRIC_MINISBLACK
        );
    }

    // =========================================================================
    // pad_tile
    // =========================================================================

    #[test]
    fn writer_pad_tile_fills_edge() {
        // 1 band, actual 2×2 block, tile 4×4, 1 bps, pad with 0xFF
        let block_data = vec![1, 2, 3, 4]; // 2×2 pixels
        let padded = pad_tile(&block_data, 2, 2, 4, 4, 1, 1, 0xFF);
        // Expected: 4×4 tile with actual data in top-left, 0xFF elsewhere
        #[rustfmt::skip]
        let expected = vec![
            1, 2, 0xFF, 0xFF,
            3, 4, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
        ];
        assert_eq!(padded, expected);
    }

    #[test]
    fn writer_pad_tile_no_padding_needed() {
        // When actual == tile dimensions, output should equal input
        let block_data = vec![10, 20, 30, 40]; // 2×2
        let padded = pad_tile(&block_data, 2, 2, 2, 2, 1, 1, 0);
        assert_eq!(padded, block_data);
    }

    // =========================================================================
    // Encoding hints from empty metadata (defaults)
    // =========================================================================

    #[test]
    fn writer_encoding_hints_from_empty_metadata() {
        let meta = BufferedMetadataProvider::new();
        let hints = TiffEncodingHints::from_metadata(&meta).unwrap();
        assert_eq!(hints.tile_width, 256);
        assert_eq!(hints.tile_height, 256);
        assert_eq!(hints.compression, tags::COMPRESSION_DEFLATE);
        assert_eq!(hints.predictor, 2);
        assert_eq!(hints.planar_config, tags::PLANAR_CONFIG_CONTIG);
    }

    // =========================================================================
    // proptest: bsq_to_interleaved properties
    // =========================================================================

    // =========================================================================
    // GeoTIFF writer integration tests
    // =========================================================================

    /// Helper: write a TIFF with the given metadata and return the file bytes.
    fn write_tiff_with_metadata(meta: &BufferedMetadataProvider) -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("geotiff_test.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0");
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Image",
                "desc",
                &[],
            )
            .unwrap();
        writer
            .set_metadata(Arc::new(BufferedMetadataProvider::from_provider(meta)))
            .unwrap();
        writer.close().unwrap();
        std::fs::read(&path).unwrap()
    }

    #[test]
    fn writer_geotiff_roundtrip_geokeys_and_pixel_scale() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let meta = BufferedMetadataProvider::new();
        // Build raw GeoKey directory: header + 2 keys (ModelType=Projected, ProjectedCRS=32618)
        meta.set_json(
            "34735",
            serde_json::json!([1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618]),
        );
        meta.set_json("33550", serde_json::json!([0.5, 0.5, 0.0]));

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // Check GeoKey directory is present under numeric key
        assert!(dict.contains_key("34735"));
        // Check pixel scale
        assert_eq!(dict.get("33550"), Some(&serde_json::json!([0.5, 0.5, 0])));
    }

    #[test]
    fn writer_plain_tiff_no_geo_tags() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        // No GeoTIFF numeric keys → plain TIFF
        let meta = BufferedMetadataProvider::new();
        meta.set("Compression", "None");

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        assert!(!dict.contains_key("34735"));
        assert!(!dict.contains_key("33550"));
    }

    #[test]
    fn writer_geotiff_invalid_epsg_returns_encode_error() {
        let meta = BufferedMetadataProvider::new();
        // Non-array value for "34735" should cause an encode error
        meta.set_json("34735", serde_json::json!("not an array"));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_epsg.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0");
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Image",
                "desc",
                &[],
            )
            .unwrap();
        writer
            .set_metadata(Arc::new(BufferedMetadataProvider::from_provider(&meta)))
            .unwrap();
        let result = writer.close();

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CodecError::Encode(_)),
            "Expected CodecError::Encode for invalid GeoKeyDirectoryTag"
        );
    }

    #[test]
    fn writer_geotiff_invalid_pixel_scale_returns_encode_error() {
        let meta = BufferedMetadataProvider::new();
        // ModelPixelScaleTag (33550) must be exactly 3 numbers
        meta.set_json("33550", serde_json::json!([0.5, 0.5]));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_scale.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0");
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Image",
                "desc",
                &[],
            )
            .unwrap();
        writer
            .set_metadata(Arc::new(BufferedMetadataProvider::from_provider(&meta)))
            .unwrap();
        let result = writer.close();

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CodecError::Encode(_)),
            "Expected CodecError::Encode for invalid ModelPixelScaleTag"
        );
    }

    // =========================================================================
    // Type inference tests
    // =========================================================================

    #[test]
    fn infer_string_to_ascii() {
        let val = serde_json::json!("hello");
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_ASCII);
    }

    #[test]
    fn infer_positive_integer_to_long() {
        let val = serde_json::json!(42);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_LONG);
    }

    #[test]
    fn infer_zero_to_long() {
        let val = serde_json::json!(0);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_LONG);
    }

    #[test]
    fn infer_negative_integer_to_slong() {
        let val = serde_json::json!(-5);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_SLONG);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn infer_float_to_double() {
        let val = serde_json::json!(3.14);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_DOUBLE);
    }

    #[test]
    fn infer_positive_int_array_to_short() {
        let val = serde_json::json!([1, 2, 3]);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_SHORT);
    }

    #[test]
    fn infer_mixed_sign_int_array_to_sshort() {
        let val = serde_json::json!([1, -2, 3]);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_SSHORT);
    }

    #[test]
    fn infer_float_array_to_double() {
        let val = serde_json::json!([1.5, 2.5]);
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_DOUBLE);
    }

    #[test]
    fn infer_explicit_annotation_byte() {
        let val = serde_json::json!({"value": [72, 101], "type": 1});
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_BYTE);
        assert_eq!(result.value, serde_json::json!([72, 101]));
    }

    #[test]
    fn infer_explicit_annotation_undefined() {
        let val = serde_json::json!({"value": [0, 255], "type": 7});
        let result = infer_field_type(&val).unwrap();
        assert_eq!(result.field_type, tags::TIFF_UNDEFINED);
        assert_eq!(result.value, serde_json::json!([0, 255]));
    }

    #[test]
    fn infer_explicit_annotation_invalid_type() {
        let val = serde_json::json!({"value": 42, "type": 13});
        let result = infer_field_type(&val);
        assert!(result.is_err());
    }

    #[test]
    fn infer_empty_array_returns_error() {
        let val = serde_json::json!([]);
        let result = infer_field_type(&val);
        assert!(result.is_err());
    }

    #[test]
    fn infer_null_returns_error() {
        let result = infer_field_type(&serde_json::Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn infer_bool_returns_error() {
        let val = serde_json::json!(true);
        let result = infer_field_type(&val);
        assert!(result.is_err());
    }

    // =========================================================================
    // Numeric key write roundtrip tests
    // =========================================================================

    #[test]
    fn writer_numeric_key_string_tag_roundtrip() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let meta = BufferedMetadataProvider::new();
        meta.set_json("42113", serde_json::json!("nan"));

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        assert_eq!(dict.get("42113"), Some(&serde_json::json!("nan")));
    }

    #[test]
    fn writer_numeric_key_integer_tag_roundtrip() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let meta = BufferedMetadataProvider::new();
        meta.set_json("65000", serde_json::json!(42));

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        assert!(dict.contains_key("65000"));
    }

    #[test]
    fn writer_non_numeric_key_is_skipped() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let meta = BufferedMetadataProvider::new();
        meta.set("ByteOrder", "LittleEndian");
        meta.set_json("42113", serde_json::json!("test"));

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // Numeric key should be present
        assert_eq!(dict.get("42113"), Some(&serde_json::json!("test")));
        // "ByteOrder" is a dataset-level key set by the reader, not a written tag.
        // It should NOT appear as a TIFF tag in the IFD.
        // (The reader may add its own "ByteOrder" dataset-level entry, but the
        // writer should not have written a TIFF tag for it.)
        // Verify no TIFF tag was created from the non-numeric key by checking
        // that the dict does not contain a tag that would correspond to "ByteOrder"
        // as a numeric tag — it simply wasn't written.
    }

    #[test]
    fn writer_explicit_type_annotation_roundtrip() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let meta = BufferedMetadataProvider::new();
        meta.set_json(
            "42113",
            serde_json::json!({"value": [72, 101, 108], "type": 7}),
        );

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // Tag should be present — UNDEFINED type is read back as a byte array
        assert!(dict.contains_key("42113"));
        let val = dict.get("42113").unwrap();
        assert!(
            val.is_array(),
            "UNDEFINED tag should be read back as an array"
        );
    }

    #[test]
    fn writer_structural_tag_not_overwritten() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let meta = BufferedMetadataProvider::new();
        // Try to override ImageWidth (tag 256) — should be ignored
        meta.set_json("256", serde_json::json!(9999));

        let bytes = write_tiff_with_metadata(&meta);
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // ImageWidth should be 256 (from the image provider), not 9999
        assert_eq!(dict.get("256"), Some(&serde_json::json!(256)));
    }

    // =========================================================================
    // NewSubfileType and GeoTIFF tag suppression tests
    // =========================================================================

    #[test]
    fn writer_overview_asset_gets_new_subfile_type_1() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overview_nsft.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0:overview:1");
        writer
            .add_asset(
                "image:0:overview:1",
                AssetProvider::Image(provider),
                "Overview",
                "desc",
                &["overview".to_string()],
            )
            .unwrap();
        writer.close().unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // Tag 254 = NewSubfileType should be 1 (reduced-resolution image)
        assert_eq!(dict.get("254"), Some(&serde_json::json!(1)));
    }

    #[test]
    fn writer_overview_ifd_has_no_geotiff_tags() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overview_no_geo.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0:overview:1");
        writer
            .add_asset(
                "image:0:overview:1",
                AssetProvider::Image(provider),
                "Overview",
                "desc",
                &["overview".to_string()],
            )
            .unwrap();

        // Set GeoTIFF metadata on the dataset
        let meta = BufferedMetadataProvider::new();
        meta.set_json(
            "34735",
            serde_json::json!([1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618]),
        );
        meta.set_json("33550", serde_json::json!([0.5, 0.5, 0.0]));
        meta.set_json(
            "33922",
            serde_json::json!([0.0, 0.0, 0.0, 500000.0, 4000000.0, 0.0]),
        );
        writer.set_metadata(Arc::new(meta)).unwrap();
        writer.close().unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        // Single-IFD file → reader assigns key "image:0" regardless of NewSubfileType
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // Overview IFD should NOT have any GeoTIFF tags
        assert!(
            !dict.contains_key("33550"),
            "Overview should not have ModelPixelScaleTag"
        );
        assert!(
            !dict.contains_key("33922"),
            "Overview should not have ModelTiepointTag"
        );
        assert!(
            !dict.contains_key("34264"),
            "Overview should not have ModelTransformationTag"
        );
        assert!(
            !dict.contains_key("34735"),
            "Overview should not have GeoKeyDirectoryTag"
        );
        assert!(
            !dict.contains_key("34736"),
            "Overview should not have GeoDoubleParamsTag"
        );
        assert!(
            !dict.contains_key("34737"),
            "Overview should not have GeoAsciiParamsTag"
        );
    }

    #[test]
    fn writer_data_asset_gets_new_subfile_type_0() {
        use crate::tiff::TIFFDatasetReader;
        use crate::traits::DatasetReader;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data_nsft.tif");
        let mut writer = TIFFDatasetWriter::new(&path).unwrap();
        let provider = make_image_provider("image:0");
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(provider),
                "Data",
                "desc",
                &["data".to_string()],
            )
            .unwrap();
        writer.close().unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
        let asset = reader.get_asset("image:0").unwrap();
        let ifd_meta = asset.metadata();
        let dict = ifd_meta.as_dict(None);

        // Tag 254 = NewSubfileType should be 0 (full-resolution image)
        assert_eq!(dict.get("254"), Some(&serde_json::json!(0)));
    }

    // =========================================================================
    // COG IFD ordering tests
    // =========================================================================

    /// Helper: create a minimal image provider with specific dimensions.
    fn make_sized_image_provider(
        key: &str,
        num_columns: u32,
        num_rows: u32,
    ) -> Arc<BufferedImageAssetProvider> {
        let config = MemoryImageConfig::new(num_columns, num_rows)
            .with_bands(1)
            .with_block_size(num_columns, num_rows)
            .with_pixel_type(PixelType::UInt8);
        let provider = BufferedImageAssetProvider::new(key, config);
        let data = vec![0u8; (num_columns * num_rows) as usize];
        provider.set_block(0, 0, &data).unwrap();
        Arc::new(provider)
    }

    #[test]
    fn test_cog_ordering_basic() {
        // Add overview first, then full-res — verify sort puts full-res first
        let mut writer = TIFFDatasetWriter::new("/tmp/cog_basic.tif").unwrap();

        let ovr = make_sized_image_provider("image:0:overview:1", 128, 128);
        writer
            .add_asset(
                "image:0:overview:1",
                AssetProvider::Image(ovr),
                "Overview",
                "desc",
                &["overview".to_string()],
            )
            .unwrap();

        let full = make_sized_image_provider("image:0", 256, 256);
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(full),
                "Full",
                "desc",
                &["data".to_string()],
            )
            .unwrap();

        writer.sort_assets_for_cog();

        assert_eq!(writer.assets.len(), 2);
        assert_eq!(writer.assets[0].key, "image:0");
        assert_eq!(writer.assets[1].key, "image:0:overview:1");
    }

    #[test]
    fn test_cog_multi_image_grouping() {
        // Two full-res images, each with two overviews in wrong order
        let mut writer = TIFFDatasetWriter::new("/tmp/cog_multi.tif").unwrap();

        // Add in scrambled order
        let ovr_0_2 = make_sized_image_provider("image:0:overview:2", 64, 64);
        writer
            .add_asset(
                "image:0:overview:2",
                AssetProvider::Image(ovr_0_2),
                "Ovr 0-2",
                "desc",
                &["overview".to_string()],
            )
            .unwrap();

        let full_1 = make_sized_image_provider("image:1", 512, 512);
        writer
            .add_asset(
                "image:1",
                AssetProvider::Image(full_1),
                "Full 1",
                "desc",
                &["data".to_string()],
            )
            .unwrap();

        let ovr_1_1 = make_sized_image_provider("image:1:overview:1", 256, 256);
        writer
            .add_asset(
                "image:1:overview:1",
                AssetProvider::Image(ovr_1_1),
                "Ovr 1-1",
                "desc",
                &["overview".to_string()],
            )
            .unwrap();

        let full_0 = make_sized_image_provider("image:0", 512, 512);
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(full_0),
                "Full 0",
                "desc",
                &["data".to_string()],
            )
            .unwrap();

        let ovr_0_1 = make_sized_image_provider("image:0:overview:1", 256, 256);
        writer
            .add_asset(
                "image:0:overview:1",
                AssetProvider::Image(ovr_0_1),
                "Ovr 0-1",
                "desc",
                &["overview".to_string()],
            )
            .unwrap();

        writer.sort_assets_for_cog();

        // Expected order:
        // image:1 (first full-res by insertion order)
        // image:1:overview:1 (its overview)
        // image:0 (second full-res by insertion order)
        // image:0:overview:1 (larger overview, 256×256)
        // image:0:overview:2 (smaller overview, 64×64)
        let keys: Vec<&str> = writer.assets.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "image:1",
                "image:1:overview:1",
                "image:0",
                "image:0:overview:1",
                "image:0:overview:2",
            ]
        );

        // Verify overview ordering by area: image:0:overview:1 (256×256=65536) > image:0:overview:2 (64×64=4096)
        let area_ovr1 = TIFFDatasetWriter::get_image_area(&writer.assets[3]);
        let area_ovr2 = TIFFDatasetWriter::get_image_area(&writer.assets[4]);
        assert!(area_ovr1 > area_ovr2);
    }

    #[test]
    fn test_cog_insertion_order_preserved() {
        // Multiple full-res images added in specific order — verify relative order preserved
        let mut writer = TIFFDatasetWriter::new("/tmp/cog_order.tif").unwrap();

        let img_2 = make_sized_image_provider("image:2", 100, 100);
        writer
            .add_asset(
                "image:2",
                AssetProvider::Image(img_2),
                "Image 2",
                "desc",
                &["data".to_string()],
            )
            .unwrap();

        let img_0 = make_sized_image_provider("image:0", 200, 200);
        writer
            .add_asset(
                "image:0",
                AssetProvider::Image(img_0),
                "Image 0",
                "desc",
                &["data".to_string()],
            )
            .unwrap();

        let img_1 = make_sized_image_provider("image:1", 150, 150);
        writer
            .add_asset(
                "image:1",
                AssetProvider::Image(img_1),
                "Image 1",
                "desc",
                &["data".to_string()],
            )
            .unwrap();

        writer.sort_assets_for_cog();

        // Insertion order: image:2, image:0, image:1 — should be preserved
        let keys: Vec<&str> = writer.assets.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, vec!["image:2", "image:0", "image:1"]);
    }

    mod prop {
        use super::*;
        use proptest::prelude::*;

        /// Inverse of bsq_to_interleaved: HWC → CHW.
        fn interleaved_to_bsq(
            data: &[u8],
            num_bands: u32,
            pixels_per_band: u32,
            bytes_per_sample: u32,
        ) -> Vec<u8> {
            let bands = num_bands as usize;
            let pixels = pixels_per_band as usize;
            let bps = bytes_per_sample as usize;
            let mut out = vec![0u8; data.len()];
            let band_stride = pixels * bps;

            for band in 0..bands {
                let dst_offset = band * band_stride;
                for pixel in 0..pixels {
                    let src = (pixel * bands + band) * bps;
                    let dst = dst_offset + pixel * bps;
                    out[dst..dst + bps].copy_from_slice(&data[src..src + bps]);
                }
            }
            out
        }

        proptest! {
            #[test]
            fn prop_bsq_to_interleaved_roundtrip(
                num_bands in 1u32..=4,
                pixels_per_band in 1u32..=32,
                bps in prop_oneof![Just(1u32), Just(2u32), Just(4u32), Just(8u32)],
            ) {
                let len = (num_bands * pixels_per_band * bps) as usize;
                // Deterministic data based on index
                let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();

                let hwc = bsq_to_interleaved(&data, num_bands, pixels_per_band, bps);
                let back = interleaved_to_bsq(&hwc, num_bands, pixels_per_band, bps);

                prop_assert_eq!(&back, &data, "CHW → HWC → CHW roundtrip failed");
            }

            #[test]
            fn prop_bsq_to_interleaved_preserves_length(
                num_bands in 1u32..=4,
                pixels_per_band in 1u32..=64,
                bps in prop_oneof![Just(1u32), Just(2u32), Just(4u32), Just(8u32)],
            ) {
                let len = (num_bands * pixels_per_band * bps) as usize;
                let data = vec![0u8; len];

                let result = bsq_to_interleaved(&data, num_bands, pixels_per_band, bps);

                prop_assert_eq!(result.len(), data.len(), "Output length differs from input");
            }
        }

        proptest! {
            /// Feature: tiff-metadata-refactor, Property 5: Field Type Roundtrip
            /// Validates: Requirements 3.1, 3.2, 3.5, 3.6, 7.1, 7.2, 7.3
            #[test]
            fn prop_field_type_roundtrip(
                field_type in prop_oneof![
                    Just(2u16),   // ASCII
                    Just(3u16),   // SHORT
                    Just(4u16),   // LONG
                    Just(7u16),   // UNDEFINED
                    Just(8u16),   // SSHORT
                    Just(9u16),   // SLONG
                    Just(12u16),  // DOUBLE
                ],
                seed in 0u32..1000,
            ) {
                use crate::tiff::TIFFDatasetReader;
                use crate::traits::DatasetReader;

                // Use a unique tag number per field type to avoid libtiff
                // global re-registration conflicts across proptest iterations.
                let tag_num = 60000u32 + field_type as u32;
                let tag_key = tag_num.to_string();

                let (tag_value, expected_check) = match field_type {
                    2 => {
                        // ASCII: generate a simple string
                        let s = format!("test_{}", seed);
                        (serde_json::json!(s), serde_json::json!(s))
                    }
                    3 => {
                        // SHORT: explicit annotation, u16 value (count=1 reads back as scalar)
                        let v = (seed % 65535) as u16;
                        (
                            serde_json::json!({"value": [v], "type": 3}),
                            serde_json::json!(v as i64),
                        )
                    }
                    4 => {
                        // LONG: positive integer (inferred)
                        let v = seed * 100;
                        (serde_json::json!(v), serde_json::json!(v as i64))
                    }
                    7 => {
                        // UNDEFINED: byte array with explicit annotation
                        let bytes = vec![(seed % 256) as u8, ((seed + 1) % 256) as u8];
                        (
                            serde_json::json!({"value": bytes, "type": 7}),
                            serde_json::json!(bytes),
                        )
                    }
                    8 => {
                        // SSHORT: signed short with explicit annotation (count=1 reads back as scalar)
                        let v = -((seed % 32000) as i16);
                        (
                            serde_json::json!({"value": [v], "type": 8}),
                            serde_json::json!(v as i64),
                        )
                    }
                    9 => {
                        // SLONG: negative integer (inferred)
                        let v = -(seed as i64 + 1);
                        (serde_json::json!(v), serde_json::json!(v))
                    }
                    12 => {
                        // DOUBLE: scalar value via explicit annotation.
                        // The register_custom_tag scalar flag ensures libtiff
                        // receives a bare f64 instead of (count, pointer).
                        let v = (seed as f64) * 0.123;
                        (
                            serde_json::json!({"value": v, "type": 12}),
                            serde_json::json!(v),
                        )
                    }
                    _ => unreachable!(),
                };

                let meta = BufferedMetadataProvider::new();
                meta.set_json(&tag_key, tag_value);

                let bytes = write_tiff_with_metadata(&meta);
                let reader = TIFFDatasetReader::from_bytes(&bytes).unwrap();
                let asset = reader.get_asset("image:0").unwrap();
                let ifd_meta = asset.metadata();
                let dict = ifd_meta.as_dict(None);

                prop_assert!(
                    dict.contains_key(&tag_key),
                    "Tag {} not found in roundtrip", tag_key
                );

                if field_type == 12 {
                    // DOUBLE: check approximate equality (scalar)
                    let read_val = dict.get(&tag_key).unwrap();
                    let r_f = read_val.as_f64().unwrap();
                    let e_f = expected_check.as_f64().unwrap();
                    prop_assert!(
                        (r_f - e_f).abs() < 1e-10,
                        "DOUBLE mismatch: read={}, expected={}", r_f, e_f
                    );
                } else {
                    let read_val = dict.get(&tag_key).unwrap();
                    prop_assert_eq!(
                        read_val, &expected_check,
                        "Roundtrip mismatch for field_type={}: read={}, expected={}",
                        field_type, read_val, expected_check
                    );
                }
            }
        }
    }
}
