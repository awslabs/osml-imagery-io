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
            let isot = if codestream.len() >= 6
                && codestream[0] == 0xFF
                && codestream[1] == 0x90
            {
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
    let expected = num_bands as usize * block_height as usize * block_width as usize * bytes_per_pixel;
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
