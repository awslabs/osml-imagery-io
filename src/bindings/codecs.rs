//! Python bindings for standalone codec decode functions.
//!
//! This module provides PyO3-wrapped decode functions that expose the library's
//! existing Rust decoders as standalone Python-callable functions. These are
//! thin wrappers with no file or format context — they accept compressed bytes
//! and decoder parameters, and return NumPy ndarrays.

use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;

use crate::jbp::image::decoder::swap_be_to_ne;
use crate::jbp::image::interleave;
use crate::jbp::image::types::{InterleaveMode, PixelValueType};
use crate::types::PixelType;

#[cfg(feature = "libjpeg-turbo")]
use crate::jbp::image::jpeg_decoder::{JpegBlockDecoder, JpegColorSpace};

#[cfg(feature = "libtiff")]
use crate::tiff::ffi::TiffHandle;
#[cfg(feature = "libtiff")]
use crate::tiff::image::{deinterleave_chunky_to_bsq, map_pixel_type};
#[cfg(feature = "libtiff")]
use crate::tiff::tags;

use super::image::create_numpy_array;

/// Determine the PixelType from J2K decode result bit depth and signedness.
fn pixel_type_from_j2k(bits_per_component: u8, is_signed: bool) -> PyResult<PixelType> {
    match (bits_per_component, is_signed) {
        (1..=8, false) => Ok(PixelType::UInt8),
        (1..=8, true) => Ok(PixelType::Int8),
        (9..=16, false) => Ok(PixelType::UInt16),
        (9..=16, true) => Ok(PixelType::Int16),
        (17..=32, false) => Ok(PixelType::UInt32),
        (17..=32, true) => Ok(PixelType::Int32),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported J2K bit depth: {} bits, signed={}",
            bits_per_component, is_signed
        ))),
    }
}

/// Decode a JPEG 2000 codestream into a NumPy array.
///
/// If ``main_header`` is provided, reconstructs a single-tile codestream:
/// ``[main_header] + [codestream] + [EOC marker 0xFF 0xD9]``.
/// Otherwise decodes ``codestream`` directly as a complete J2K codestream.
///
/// Returns an ndarray with shape ``(bands, height, width)`` and appropriate dtype.
///
/// :param codestream: Compressed JPEG 2000 tile-part or complete codestream bytes.
/// :param main_header: Optional J2K main header bytes for single-tile reconstruction.
/// :param resolution_level: Target resolution level (0 = full resolution, default 0).
/// :returns: NumPy ndarray with shape (bands, height, width).
/// :raises ValueError: If the codestream is invalid or decoding fails.
#[cfg(feature = "openjpeg")]
#[pyfunction]
#[pyo3(signature = (codestream, main_header=None, resolution_level=0))]
pub fn decode_jpeg2000(
    py: Python<'_>,
    codestream: &[u8],
    main_header: Option<&[u8]>,
    resolution_level: u32,
) -> PyResult<Py<PyAny>> {
    use crate::j2k::{get_j2k_codec, J2KDecodeParams};

    let codec = get_j2k_codec();

    let params = J2KDecodeParams {
        resolution_level,
        region: None,
    };

    let result = match main_header {
        Some(header) => {
            use crate::j2k::markers::rewrite_siz_for_tile;

            // Extract original Isot from the tile-part SOT marker before patching.
            let isot = if codestream.len() >= 6 && codestream[0] == 0xFF && codestream[1] == 0x90 {
                Some(u16::from_be_bytes([codestream[4], codestream[5]]))
            } else {
                None
            };

            // Rewrite SIZ to describe a single-tile image with actual edge tile
            // dimensions. For interior tiles this is a no-op.
            let patched_header = match isot {
                Some(tile_index) => rewrite_siz_for_tile(header, tile_index),
                None => header.to_vec(),
            };

            // Reconstruct a single-tile codestream:
            // [patched header] + [tile-part bytes with Isot patched to 0] + [EOC]
            let mut full_codestream =
                Vec::with_capacity(patched_header.len() + codestream.len() + 2);
            full_codestream.extend_from_slice(&patched_header);

            // The codestream bytes start with SOT marker — patch Isot to 0
            if codestream.len() >= 6 && codestream[0] == 0xFF && codestream[1] == 0x90 {
                full_codestream.extend_from_slice(&codestream[..4]); // marker + Lsot
                full_codestream.extend_from_slice(&[0x00, 0x00]); // Isot = 0
                full_codestream.extend_from_slice(&codestream[6..]); // rest
            } else {
                full_codestream.extend_from_slice(codestream);
            }

            // Append EOC marker
            full_codestream.extend_from_slice(&[0xFF, 0xD9]);

            codec.decode_tile(&full_codestream, 0, &params)?
        }
        None => {
            // Decode the codestream directly as a complete J2K codestream
            codec.decode_tile(codestream, 0, &params)?
        }
    };

    let pixel_type = pixel_type_from_j2k(result.bits_per_component, result.is_signed)?;
    let shape = [result.num_components, result.height, result.width];

    create_numpy_array(py, &result.data, shape, pixel_type)
}

/// Parse an `imode` string parameter to `InterleaveMode`.
///
/// The string should be a single character: "B", "P", "R", or "S".
/// This helper is reused by both `decode_jpeg` and `decode_jbp_block`.
pub(crate) fn parse_imode(imode: &str) -> PyResult<InterleaveMode> {
    let c = imode.chars().next().ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err(
            "Invalid interleave mode: empty string. Expected: B, P, R, or S",
        )
    })?;
    InterleaveMode::from_char(c).map_err(|_| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Invalid interleave mode '{}'. Expected: B, P, R, or S",
            imode
        ))
    })
}

/// Parse a `color_space` string parameter to `JpegColorSpace`.
///
/// Valid values: "MONO", "RGB", "YCbCr601".
#[cfg(feature = "libjpeg-turbo")]
fn parse_jpeg_color_space(color_space: &str) -> PyResult<JpegColorSpace> {
    match color_space {
        "MONO" => Ok(JpegColorSpace::Grayscale),
        "RGB" => Ok(JpegColorSpace::Rgb),
        "YCbCr601" => Ok(JpegColorSpace::YCbCr601),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Invalid color space '{}'. Expected: MONO, RGB, or YCbCr601",
            color_space
        ))),
    }
}

/// Decode a JPEG stream into a NumPy array.
///
/// Returns an ndarray with shape ``(num_bands, block_height, block_width)``
/// in band-sequential (BSQ) format.
///
/// :param data: Compressed JPEG bytes.
/// :param bits_per_pixel: Bits per pixel (8 or 12).
/// :param num_bands: Number of image bands.
/// :param block_width: Block width in pixels.
/// :param block_height: Block height in pixels.
/// :param imode: Interleave mode string ("B", "P", "R", or "S").
/// :param color_space: Color space string ("MONO", "RGB", or "YCbCr601").
/// :returns: NumPy ndarray with shape (num_bands, block_height, block_width).
/// :raises ValueError: If parameters are invalid or decoding fails.
#[cfg(feature = "libjpeg-turbo")]
#[pyfunction]
pub fn decode_jpeg(
    py: Python<'_>,
    data: &[u8],
    bits_per_pixel: u8,
    num_bands: u32,
    block_width: u32,
    block_height: u32,
    imode: &str,
    color_space: &str,
) -> PyResult<Py<PyAny>> {
    let imode_enum = parse_imode(imode)?;
    let cs_enum = parse_jpeg_color_space(color_space)?;

    let decoder = JpegBlockDecoder::new(
        bits_per_pixel,
        num_bands as usize,
        block_width as usize,
        block_height as usize,
        imode_enum,
        cs_enum,
    )
    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    // decode_multiband_block handles all cases internally:
    // single-band, 3-band RGB/YCbCr with IMODE=P, and multiband IMODE=B/S
    let decoded = decoder
        .decode_multiband_block(data)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    // Determine pixel type: 8-bit → UInt8, 12-bit → UInt16
    let pixel_type = if bits_per_pixel <= 8 {
        PixelType::UInt8
    } else {
        PixelType::UInt16
    };

    let shape = [num_bands, block_height, block_width];
    create_numpy_array(py, &decoded, shape, pixel_type)
}

/// Parse a `pvtype` string parameter to `PixelValueType`.
///
/// Valid values for the codec binding: "INT", "SI", "R", "C".
/// Bi-level ("B") is not supported in this context.
pub(crate) fn parse_pvtype(pvtype: &str) -> PyResult<PixelValueType> {
    let pv = PixelValueType::from_str(pvtype).map_err(|_| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Invalid pixel value type '{}'. Expected: INT, SI, R, or C",
            pvtype
        ))
    })?;
    if pv == PixelValueType::BiLevel {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Invalid pixel value type '{}'. Expected: INT, SI, R, or C",
            pvtype
        )));
    }
    Ok(pv)
}

/// Validate that the nbpp/pvtype combination is supported.
///
/// Returns the `PixelType` on success, or raises `ValueError` for unsupported combos.
fn validate_nbpp_pvtype(nbpp: u8, pvtype: &str, pv: &PixelValueType) -> PyResult<PixelType> {
    let valid = match pv {
        PixelValueType::UnsignedInt => matches!(nbpp, 8 | 16 | 32),
        PixelValueType::SignedInt => matches!(nbpp, 8 | 16 | 32),
        PixelValueType::Real => matches!(nbpp, 32 | 64),
        PixelValueType::Complex => nbpp == 64,
        PixelValueType::BiLevel => false,
    };
    if !valid {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported nbpp={} for pvtype='{}'",
            nbpp, pvtype
        )));
    }
    Ok(pv.to_pixel_type(nbpp))
}

/// Decode an uncompressed JBP/NITF/NSIF image block into a NumPy array.
///
/// Performs interleave conversion (imode → BSQ) and big-endian to
/// native-endian byte swap.
///
/// Returns an ndarray with shape ``(num_bands, block_height, block_width)``
/// and the appropriate dtype for the given ``nbpp`` and ``pvtype``.
///
/// :param data: Raw pixel bytes from a JBP/NITF image block.
/// :param num_bands: Number of image bands.
/// :param block_height: Block height in pixels.
/// :param block_width: Block width in pixels.
/// :param nbpp: Number of bits per pixel per band (8, 16, 32, or 64).
/// :param imode: NITF interleave mode string ("B", "P", "R", or "S").
/// :param pvtype: NITF pixel value type string ("INT", "SI", "R", or "C").
/// :returns: NumPy ndarray with shape (num_bands, block_height, block_width).
/// :raises ValueError: If parameters are invalid or data length mismatches.
#[pyfunction]
pub fn decode_jbp_block(
    py: Python<'_>,
    data: &[u8],
    num_bands: u32,
    block_height: u32,
    block_width: u32,
    nbpp: u8,
    imode: &str,
    pvtype: &str,
) -> PyResult<Py<PyAny>> {
    let imode_enum = parse_imode(imode)?;
    let pv = parse_pvtype(pvtype)?;
    let pixel_type = validate_nbpp_pvtype(nbpp, pvtype, &pv)?;
    let bytes_per_pixel = pixel_type.bytes_per_pixel();

    // Validate data length
    let expected =
        num_bands as usize * block_height as usize * block_width as usize * bytes_per_pixel;
    if data.len() != expected {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Data size mismatch: expected {} bytes, got {}",
            expected,
            data.len()
        )));
    }

    // Convert from source interleave mode to band-sequential
    let bsq_data = interleave::to_band_sequential(
        data,
        imode_enum,
        block_height,
        block_width,
        num_bands,
        bytes_per_pixel,
    )
    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    // Swap big-endian to native-endian
    let native_data = swap_be_to_ne(&bsq_data, bytes_per_pixel);

    let shape = [num_bands, block_height, block_width];
    create_numpy_array(py, &native_data, shape, pixel_type)
}

// =============================================================================
// TIFF Tile Decode
// =============================================================================

/// Helper to write a 12-byte IFD entry into a buffer.
///
/// Each IFD entry: [tag:u16 LE][type:u16 LE][count:u32 LE][value/offset:u32 LE]
#[cfg(feature = "libtiff")]
fn write_ifd_entry(buf: &mut Vec<u8>, tag: u16, field_type: u16, count: u32, value: u32) {
    buf.extend_from_slice(&tag.to_le_bytes());
    buf.extend_from_slice(&field_type.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&value.to_le_bytes());
}

/// Construct a synthetic single-tile TIFF byte buffer from codec configuration
/// and compressed tile data.
///
/// The buffer layout:
/// - TIFF header (8 bytes): byte order "II", magic 42, IFD offset 8
/// - IFD: entry count, sorted tag entries, next IFD offset (0)
/// - JPEGTables data (if present)
/// - Compressed tile bytes
#[cfg(feature = "libtiff")]
fn build_synthetic_tiff(
    data: &[u8],
    compression: u16,
    bits_per_sample: u16,
    samples_per_pixel: u16,
    photometric: u16,
    planar_config: u16,
    predictor: u16,
    tile_width: u32,
    tile_height: u32,
    sample_format: u16,
    jpeg_tables: Option<&[u8]>,
) -> Vec<u8> {
    // Count IFD entries: base tags + optional predictor + optional jpeg_tables
    // Base tags (always present): ImageWidth(256), ImageLength(257),
    // BitsPerSample(258), Compression(259), Photometric(262),
    // SamplesPerPixel(277), PlanarConfig(284), TileWidth(322),
    // TileLength(323), TileOffsets(324), TileByteCounts(325),
    // SampleFormat(339)
    let mut num_entries: u16 = 12;
    if predictor != 1 {
        num_entries += 1; // Predictor(317)
    }
    if jpeg_tables.is_some() {
        num_entries += 1; // JPEGTables(347)
    }

    // Calculate offsets
    // IFD starts at offset 8 (right after header)
    // IFD size: 2 (count) + num_entries * 12 + 4 (next IFD offset)
    let ifd_size = 2 + (num_entries as usize) * 12 + 4;
    let after_ifd = 8 + ifd_size;

    // JPEGTables data goes right after IFD
    let jpeg_tables_offset = after_ifd;
    let jpeg_tables_len = jpeg_tables.map_or(0, |t| t.len());

    // Tile data goes after JPEGTables
    let tile_data_offset = jpeg_tables_offset + jpeg_tables_len;

    // Pre-allocate buffer
    let total_size = tile_data_offset + data.len();
    let mut buf = Vec::with_capacity(total_size);

    // --- TIFF Header (8 bytes) ---
    buf.extend_from_slice(b"II"); // Little-endian byte order
    buf.extend_from_slice(&42u16.to_le_bytes()); // TIFF magic number
    buf.extend_from_slice(&8u32.to_le_bytes()); // IFD offset

    // --- IFD ---
    buf.extend_from_slice(&num_entries.to_le_bytes());

    // IFD entries MUST be sorted by tag number (TIFF 6.0 requirement)
    // SHORT type = 3, LONG type = 4, UNDEFINED type = 7

    // Tag 256: ImageWidth (LONG)
    write_ifd_entry(&mut buf, 256, tags::TIFF_LONG, 1, tile_width);

    // Tag 257: ImageLength (LONG)
    write_ifd_entry(&mut buf, 257, tags::TIFF_LONG, 1, tile_height);

    // Tag 258: BitsPerSample (SHORT) — value fits in 4 bytes, stored inline
    write_ifd_entry(&mut buf, 258, tags::TIFF_SHORT, 1, bits_per_sample as u32);

    // Tag 259: Compression (SHORT)
    write_ifd_entry(&mut buf, 259, tags::TIFF_SHORT, 1, compression as u32);

    // Tag 262: PhotometricInterpretation (SHORT)
    write_ifd_entry(&mut buf, 262, tags::TIFF_SHORT, 1, photometric as u32);

    // Tag 277: SamplesPerPixel (SHORT)
    write_ifd_entry(&mut buf, 277, tags::TIFF_SHORT, 1, samples_per_pixel as u32);

    // Tag 284: PlanarConfiguration (SHORT)
    write_ifd_entry(&mut buf, 284, tags::TIFF_SHORT, 1, planar_config as u32);

    // Tag 317: Predictor (SHORT) — only if != 1
    if predictor != 1 {
        write_ifd_entry(&mut buf, 317, tags::TIFF_SHORT, 1, predictor as u32);
    }

    // Tag 322: TileWidth (LONG)
    write_ifd_entry(&mut buf, 322, tags::TIFF_LONG, 1, tile_width);

    // Tag 323: TileLength (LONG)
    write_ifd_entry(&mut buf, 323, tags::TIFF_LONG, 1, tile_height);

    // Tag 324: TileOffsets (LONG) — offset to tile data
    write_ifd_entry(&mut buf, 324, tags::TIFF_LONG, 1, tile_data_offset as u32);

    // Tag 325: TileByteCounts (LONG) — size of compressed tile data
    write_ifd_entry(&mut buf, 325, tags::TIFF_LONG, 1, data.len() as u32);

    // Tag 339: SampleFormat (SHORT)
    write_ifd_entry(&mut buf, 339, tags::TIFF_SHORT, 1, sample_format as u32);

    // Tag 347: JPEGTables (UNDEFINED) — only if present
    if let Some(tables) = jpeg_tables {
        write_ifd_entry(
            &mut buf,
            347,
            tags::TIFF_UNDEFINED,
            tables.len() as u32,
            jpeg_tables_offset as u32,
        );
    }

    // Next IFD offset: 0 (no more IFDs)
    buf.extend_from_slice(&0u32.to_le_bytes());

    // --- JPEGTables data (if present) ---
    if let Some(tables) = jpeg_tables {
        buf.extend_from_slice(tables);
    }

    // --- Compressed tile bytes ---
    buf.extend_from_slice(data);

    buf
}

/// Decode a TIFF-compressed tile into a NumPy array.
///
/// Constructs a synthetic single-tile TIFF in memory from the codec
/// configuration parameters and compressed tile bytes, then decodes it
/// using libtiff's `TIFFReadEncodedTile`.
///
/// Returns an ndarray with shape ``(bands, tile_height, tile_width)`` in
/// band-sequential (BSQ) format.
///
/// :param data: Compressed tile bytes.
/// :param compression: TIFF compression type (e.g. 5=LZW, 7=JPEG, 8=Deflate).
/// :param bits_per_sample: Bits per sample (8, 16, 32, or 64).
/// :param samples_per_pixel: Number of bands.
/// :param photometric: Photometric interpretation (1=MinIsBlack, 2=RGB, 6=YCbCr).
/// :param planar_config: Planar configuration (1=chunky, 2=separate).
/// :param predictor: Predictor type (1=none, 2=horizontal, 3=floating-point).
/// :param tile_width: Tile width in pixels.
/// :param tile_height: Tile height in pixels.
/// :param sample_format: Sample format (1=uint, 2=int, 3=float).
/// :param jpeg_tables: Optional JPEG quantization/Huffman tables for JPEG tiles.
/// :returns: NumPy ndarray with shape (bands, tile_height, tile_width).
/// :raises ValueError: If parameters are invalid or decoding fails.
#[cfg(feature = "libtiff")]
#[pyfunction]
#[pyo3(signature = (data, compression, bits_per_sample, samples_per_pixel,
                    photometric, planar_config, predictor, tile_width,
                    tile_height, sample_format, jpeg_tables=None))]
pub fn decode_tiff_tile(
    py: Python<'_>,
    data: &[u8],
    compression: u16,
    bits_per_sample: u16,
    samples_per_pixel: u16,
    photometric: u16,
    planar_config: u16,
    predictor: u16,
    tile_width: u32,
    tile_height: u32,
    sample_format: u16,
    jpeg_tables: Option<&[u8]>,
) -> PyResult<Py<PyAny>> {
    // Map sample_format + bits_per_sample to PixelType
    let pixel_type = map_pixel_type(Some(sample_format), bits_per_sample).map_err(
        |e: crate::error::CodecError| pyo3::exceptions::PyValueError::new_err(e.to_string()),
    )?;

    let bytes_per_sample = pixel_type.bytes_per_pixel();
    let bands = samples_per_pixel as u32;

    // Build the synthetic TIFF buffer
    let tiff_buf = build_synthetic_tiff(
        data,
        compression,
        bits_per_sample,
        samples_per_pixel,
        photometric,
        planar_config,
        predictor,
        tile_width,
        tile_height,
        sample_format,
        jpeg_tables,
    );

    // Open the synthetic TIFF with libtiff
    let handle = TiffHandle::from_bytes(&tiff_buf).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to open synthetic TIFF (compression={}): {}",
            compression, e
        ))
    })?;

    // For JPEG with YCbCr photometric, set JPEGCOLORMODE_RGB
    if compression == tags::COMPRESSION_JPEG && photometric == tags::PHOTOMETRIC_YCBCR {
        handle
            .set_field_u32(tags::TIFFTAG_JPEGCOLORMODE, tags::JPEGCOLORMODE_RGB as u32)
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "Failed to set JPEGCOLORMODE_RGB: {}",
                    e
                ))
            })?;
    }

    // Decode tile 0
    let decoded = handle.read_encoded_tile(0).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to decode TIFF tile (compression={}): {}",
            compression, e
        ))
    })?;

    // Handle edge tiles: allocate full nominal tile buffer, copy decoded bytes
    let full_tile_bytes =
        tile_width as usize * tile_height as usize * bands as usize * bytes_per_sample;
    let padded = if decoded.len() < full_tile_bytes {
        let mut buf = vec![0u8; full_tile_bytes];
        buf[..decoded.len()].copy_from_slice(&decoded);
        buf
    } else {
        decoded
    };

    // Convert chunky to BSQ if PlanarConfiguration=1
    let bsq_data = if planar_config == tags::PLANAR_CONFIG_CONTIG && bands > 1 {
        let all_bands: Vec<u32> = (0..bands).collect();
        deinterleave_chunky_to_bsq(
            &padded,
            tile_width,
            tile_height,
            tile_width,
            tile_height,
            bands,
            bytes_per_sample,
            &all_bands,
        )
    } else {
        padded
    };

    let shape = [bands, tile_height, tile_width];
    create_numpy_array(py, &bsq_data, shape, pixel_type)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[cfg(feature = "libtiff")]
mod tests {
    use super::*;
    use crate::tiff::ffi::TiffHandle;
    use crate::tiff::tags;

    /// Test that a synthetic TIFF buffer with known uncompressed tile data
    /// round-trips correctly through TIFFClientOpen + TIFFReadEncodedTile.
    ///
    /// Creates a 4x4 grayscale 8-bit uncompressed tile, builds a synthetic
    /// TIFF, opens it with TiffHandle, reads tile 0, and verifies the decoded
    /// bytes match the original input.
    #[test]
    fn test_synthetic_tiff_uncompressed_roundtrip() {
        // 4x4 grayscale, 8-bit, known pixel values
        let pixel_data: Vec<u8> = (0..16).collect(); // [0, 1, 2, ..., 15]

        let tiff_buf = build_synthetic_tiff(
            &pixel_data,
            tags::COMPRESSION_NONE,       // compression = 1 (uncompressed)
            8,                            // bits_per_sample
            1,                            // samples_per_pixel (grayscale)
            tags::PHOTOMETRIC_MINISBLACK, // photometric
            tags::PLANAR_CONFIG_CONTIG,   // planar_config
            1,                            // predictor (none)
            4,                            // tile_width
            4,                            // tile_height
            tags::SAMPLE_FORMAT_UINT,     // sample_format
            None,                         // no jpeg_tables
        );

        // Open the synthetic TIFF
        let handle =
            TiffHandle::from_bytes(&tiff_buf).expect("Failed to open synthetic TIFF from bytes");

        // Read tile 0
        let decoded = handle
            .read_encoded_tile(0)
            .expect("Failed to read encoded tile 0");

        // Verify the decoded bytes match the original pixel data
        assert_eq!(
            decoded, pixel_data,
            "Decoded tile data does not match original pixel data"
        );
    }

    /// Test that a synthetic TIFF with an unsupported/invalid compression type
    /// can be opened by TiffHandle but fails to decode the tile.
    ///
    /// Uses compression type 9999 which is not a valid TIFF compression scheme.
    /// libtiff should be able to parse the IFD structure but fail when attempting
    /// to decompress the tile data.
    #[test]
    fn test_synthetic_tiff_invalid_compression_fails_decode() {
        let pixel_data: Vec<u8> = vec![0u8; 16]; // 4x4 dummy data

        let tiff_buf = build_synthetic_tiff(
            &pixel_data,
            9999, // invalid compression type
            8,    // bits_per_sample
            1,    // samples_per_pixel
            tags::PHOTOMETRIC_MINISBLACK,
            tags::PLANAR_CONFIG_CONTIG,
            1, // predictor
            4, // tile_width
            4, // tile_height
            tags::SAMPLE_FORMAT_UINT,
            None,
        );

        // Opening may fail or succeed depending on libtiff version.
        // If it opens, reading the tile should fail.
        match TiffHandle::from_bytes(&tiff_buf) {
            Ok(handle) => {
                let result = handle.read_encoded_tile(0);
                assert!(
                    result.is_err(),
                    "Expected read_encoded_tile to fail with invalid compression type 9999"
                );
            }
            Err(_) => {
                // Also acceptable: libtiff rejects the buffer at open time
            }
        }
    }

    /// Test that build_synthetic_tiff produces a valid TIFF buffer that
    /// TiffHandle can open and that reports correct tile dimensions.
    ///
    /// This validates the buffer structure is well-formed by verifying
    /// libtiff can parse the IFD and report the expected tag values.
    #[test]
    fn test_synthetic_tiff_buffer_structure_valid() {
        let tile_w = 8u32;
        let tile_h = 4u32;
        let bands = 3u16;
        let pixel_data = vec![42u8; (tile_w * tile_h * bands as u32) as usize];

        let tiff_buf = build_synthetic_tiff(
            &pixel_data,
            tags::COMPRESSION_NONE,
            8,
            bands,
            tags::PHOTOMETRIC_RGB,
            tags::PLANAR_CONFIG_CONTIG,
            1,
            tile_w,
            tile_h,
            tags::SAMPLE_FORMAT_UINT,
            None,
        );

        let handle =
            TiffHandle::from_bytes(&tiff_buf).expect("Failed to open synthetic TIFF buffer");

        // Verify libtiff parsed the IFD correctly
        assert!(handle.is_tiled(), "Synthetic TIFF should be tiled");

        let read_width = handle
            .get_field_u32(tags::IMAGE_WIDTH)
            .expect("Failed to read ImageWidth");
        assert_eq!(read_width, tile_w, "ImageWidth mismatch");

        let read_height = handle
            .get_field_u32(tags::IMAGE_LENGTH)
            .expect("Failed to read ImageLength");
        assert_eq!(read_height, tile_h, "ImageLength mismatch");

        let read_bps = handle
            .get_field_u16(tags::BITS_PER_SAMPLE)
            .expect("Failed to read BitsPerSample");
        assert_eq!(read_bps, 8, "BitsPerSample mismatch");

        let read_spp = handle
            .get_field_u16(tags::SAMPLES_PER_PIXEL)
            .expect("Failed to read SamplesPerPixel");
        assert_eq!(read_spp, bands, "SamplesPerPixel mismatch");

        let read_tw = handle
            .get_field_u32(tags::TILE_WIDTH)
            .expect("Failed to read TileWidth");
        assert_eq!(read_tw, tile_w, "TileWidth mismatch");

        let read_tl = handle
            .get_field_u32(tags::TILE_LENGTH)
            .expect("Failed to read TileLength");
        assert_eq!(read_tl, tile_h, "TileLength mismatch");

        // Verify the tile data round-trips for multi-band uncompressed
        let decoded = handle.read_encoded_tile(0).expect("Failed to read tile 0");
        assert_eq!(
            decoded, pixel_data,
            "Multi-band uncompressed tile data should round-trip exactly"
        );
    }
}
