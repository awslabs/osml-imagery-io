//! Pixel value encoding and decoding for uncompressed NITF imagery.
//!
//! This module handles the conversion between raw bytes and pixel values
//! for all PVTYPE (pixel value type) and NBPP (number of bits per pixel)
//! combinations supported by NITF.
//!
//! # Supported Types
//!
//! | PVTYPE | NBPP | Description |
//! |--------|------|-------------|
//! | INT    | 8    | Unsigned 8-bit integer |
//! | INT    | 16   | Unsigned 16-bit integer |
//! | INT    | 32   | Unsigned 32-bit integer |
//! | SI     | 8    | Signed 8-bit integer |
//! | SI     | 16   | Signed 16-bit integer |
//! | SI     | 32   | Signed 32-bit integer |
//! | R      | 32   | IEEE 32-bit floating-point |
//! | R      | 64   | IEEE 64-bit floating-point |
//! | C      | 64   | Complex (two 32-bit floats) |
//! | B      | 1    | Bi-level (1-bit packed) |
//!
//! # Byte Order
//!
//! NITF uses big-endian byte order for all multi-byte values.

use crate::error::CodecError;
use super::types::{PixelJustification, PixelValueType};

/// Calculate the number of bytes per pixel for a given PVTYPE and NBPP.
///
/// # Arguments
/// * `pvtype` - The pixel value type
/// * `nbpp` - Number of bits per pixel
///
/// # Returns
/// The number of bytes required to store one pixel value.
///
/// # Examples
/// ```ignore
/// use osml_imagery_io::jbp::image::pixel::bytes_per_pixel;
/// use osml_imagery_io::jbp::image::types::PixelValueType;
///
/// assert_eq!(bytes_per_pixel(PixelValueType::UnsignedInt, 8), 1);
/// assert_eq!(bytes_per_pixel(PixelValueType::UnsignedInt, 16), 2);
/// assert_eq!(bytes_per_pixel(PixelValueType::Real, 32), 4);
/// assert_eq!(bytes_per_pixel(PixelValueType::Complex, 64), 8);
/// ```
pub fn bytes_per_pixel(pvtype: PixelValueType, nbpp: u8) -> usize {
    match pvtype {
        PixelValueType::BiLevel => {
            // Bi-level pixels are packed into bytes, but for single pixel access
            // we still need at least 1 byte
            1
        }
        _ => {
            // For all other types, bytes = ceil(nbpp / 8)
            (nbpp as usize).div_ceil(8)
        }
    }
}


/// Decode a single unsigned integer pixel value from bytes.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian)
/// * `nbpp` - Number of bits per pixel (8, 16, or 32)
///
/// # Returns
/// The decoded pixel value as f64.
fn decode_unsigned_int(data: &[u8], nbpp: u8) -> Result<f64, CodecError> {
    match nbpp {
        8 => {
            if data.is_empty() {
                return Err(CodecError::Decode("Insufficient data for u8 pixel".into()));
            }
            Ok(data[0] as f64)
        }
        16 => {
            if data.len() < 2 {
                return Err(CodecError::Decode("Insufficient data for u16 pixel".into()));
            }
            let value = u16::from_be_bytes([data[0], data[1]]);
            Ok(value as f64)
        }
        32 => {
            if data.len() < 4 {
                return Err(CodecError::Decode("Insufficient data for u32 pixel".into()));
            }
            let value = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            Ok(value as f64)
        }
        _ => Err(CodecError::Decode(format!(
            "Unsupported NBPP {} for unsigned integer",
            nbpp
        ))),
    }
}

/// Decode a single signed integer pixel value from bytes.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian)
/// * `nbpp` - Number of bits per pixel (8, 16, or 32)
///
/// # Returns
/// The decoded pixel value as f64.
fn decode_signed_int(data: &[u8], nbpp: u8) -> Result<f64, CodecError> {
    match nbpp {
        8 => {
            if data.is_empty() {
                return Err(CodecError::Decode("Insufficient data for i8 pixel".into()));
            }
            Ok((data[0] as i8) as f64)
        }
        16 => {
            if data.len() < 2 {
                return Err(CodecError::Decode("Insufficient data for i16 pixel".into()));
            }
            let value = i16::from_be_bytes([data[0], data[1]]);
            Ok(value as f64)
        }
        32 => {
            if data.len() < 4 {
                return Err(CodecError::Decode("Insufficient data for i32 pixel".into()));
            }
            let value = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            Ok(value as f64)
        }
        _ => Err(CodecError::Decode(format!(
            "Unsupported NBPP {} for signed integer",
            nbpp
        ))),
    }
}


/// Decode a single real (floating-point) pixel value from bytes.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian IEEE format)
/// * `nbpp` - Number of bits per pixel (32 or 64)
///
/// # Returns
/// The decoded pixel value as f64.
fn decode_real(data: &[u8], nbpp: u8) -> Result<f64, CodecError> {
    match nbpp {
        32 => {
            if data.len() < 4 {
                return Err(CodecError::Decode("Insufficient data for f32 pixel".into()));
            }
            let value = f32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            Ok(value as f64)
        }
        64 => {
            if data.len() < 8 {
                return Err(CodecError::Decode("Insufficient data for f64 pixel".into()));
            }
            let value = f64::from_be_bytes([
                data[0], data[1], data[2], data[3],
                data[4], data[5], data[6], data[7],
            ]);
            Ok(value)
        }
        _ => Err(CodecError::Decode(format!(
            "Unsupported NBPP {} for real type",
            nbpp
        ))),
    }
}

/// Decode a complex pixel value from bytes.
///
/// Complex values are stored as two consecutive 32-bit IEEE floats (real, imaginary).
/// NBPP must be 64 (32 bits for real + 32 bits for imaginary).
///
/// # Arguments
/// * `data` - The raw bytes (big-endian IEEE format)
///
/// # Returns
/// A tuple of (real, imaginary) as f64 values.
fn decode_complex(data: &[u8]) -> Result<(f64, f64), CodecError> {
    if data.len() < 8 {
        return Err(CodecError::Decode("Insufficient data for complex pixel".into()));
    }
    let real = f32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let imag = f32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    Ok((real as f64, imag as f64))
}


/// Decode a bi-level (1-bit) pixel value from a byte.
///
/// Bi-level pixels are packed 8 per byte, with the most significant bit first.
///
/// # Arguments
/// * `byte` - The byte containing the packed bits
/// * `bit_index` - The bit index within the byte (0-7, 0 is MSB)
///
/// # Returns
/// The decoded pixel value (0 or 1) as f64.
fn decode_bilevel_bit(byte: u8, bit_index: u8) -> f64 {
    let mask = 0x80 >> bit_index;
    if byte & mask != 0 { 1.0 } else { 0.0 }
}

/// Unpack all bi-level pixels from a byte array.
///
/// # Arguments
/// * `data` - The packed byte array
/// * `num_pixels` - The number of pixels to unpack
///
/// # Returns
/// A vector of unpacked pixel values (0.0 or 1.0).
fn unpack_bilevel(data: &[u8], num_pixels: usize) -> Vec<f64> {
    let mut result = Vec::with_capacity(num_pixels);
    for i in 0..num_pixels {
        let byte_index = i / 8;
        let bit_index = (i % 8) as u8;
        if byte_index < data.len() {
            result.push(decode_bilevel_bit(data[byte_index], bit_index));
        } else {
            result.push(0.0);
        }
    }
    result
}


/// Decode a single pixel value from raw bytes.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian)
/// * `pvtype` - The pixel value type
/// * `nbpp` - Number of bits per pixel
///
/// # Returns
/// The decoded pixel value as f64. For complex types, only the real part is returned;
/// use `decode_complex_pixel` for both components.
///
/// # Errors
/// Returns an error if the data is insufficient or the PVTYPE/NBPP combination is invalid.
pub fn decode_pixel(data: &[u8], pvtype: PixelValueType, nbpp: u8) -> Result<f64, CodecError> {
    match pvtype {
        PixelValueType::UnsignedInt => decode_unsigned_int(data, nbpp),
        PixelValueType::SignedInt => decode_signed_int(data, nbpp),
        PixelValueType::Real => decode_real(data, nbpp),
        PixelValueType::Complex => {
            if nbpp != 64 {
                return Err(CodecError::Decode(format!(
                    "Complex type requires NBPP=64, got {}",
                    nbpp
                )));
            }
            let (real, _imag) = decode_complex(data)?;
            Ok(real)
        }
        PixelValueType::BiLevel => {
            if nbpp != 1 {
                return Err(CodecError::Decode(format!(
                    "BiLevel type requires NBPP=1, got {}",
                    nbpp
                )));
            }
            if data.is_empty() {
                return Err(CodecError::Decode("Insufficient data for bi-level pixel".into()));
            }
            // For single pixel decode, return the first bit
            Ok(decode_bilevel_bit(data[0], 0))
        }
    }
}

/// Decode a complex pixel value from raw bytes, returning both real and imaginary parts.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian IEEE format)
///
/// # Returns
/// A tuple of (real, imaginary) as f64 values.
pub fn decode_complex_pixel(data: &[u8]) -> Result<(f64, f64), CodecError> {
    decode_complex(data)
}


/// Encode an unsigned integer pixel value to bytes.
///
/// # Arguments
/// * `value` - The pixel value as f64
/// * `nbpp` - Number of bits per pixel (8, 16, or 32)
///
/// # Returns
/// The encoded bytes in big-endian order.
fn encode_unsigned_int(value: f64, nbpp: u8) -> Result<Vec<u8>, CodecError> {
    match nbpp {
        8 => {
            let v = value.clamp(0.0, u8::MAX as f64) as u8;
            Ok(vec![v])
        }
        16 => {
            let v = value.clamp(0.0, u16::MAX as f64) as u16;
            Ok(v.to_be_bytes().to_vec())
        }
        32 => {
            let v = value.clamp(0.0, u32::MAX as f64) as u32;
            Ok(v.to_be_bytes().to_vec())
        }
        _ => Err(CodecError::Encode(format!(
            "Unsupported NBPP {} for unsigned integer",
            nbpp
        ))),
    }
}

/// Encode a signed integer pixel value to bytes.
///
/// # Arguments
/// * `value` - The pixel value as f64
/// * `nbpp` - Number of bits per pixel (8, 16, or 32)
///
/// # Returns
/// The encoded bytes in big-endian order.
fn encode_signed_int(value: f64, nbpp: u8) -> Result<Vec<u8>, CodecError> {
    match nbpp {
        8 => {
            let v = value.clamp(i8::MIN as f64, i8::MAX as f64) as i8;
            Ok(vec![v as u8])
        }
        16 => {
            let v = value.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            Ok(v.to_be_bytes().to_vec())
        }
        32 => {
            let v = value.clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            Ok(v.to_be_bytes().to_vec())
        }
        _ => Err(CodecError::Encode(format!(
            "Unsupported NBPP {} for signed integer",
            nbpp
        ))),
    }
}

/// Encode a real (floating-point) pixel value to bytes.
///
/// # Arguments
/// * `value` - The pixel value as f64
/// * `nbpp` - Number of bits per pixel (32 or 64)
///
/// # Returns
/// The encoded bytes in big-endian IEEE format.
fn encode_real(value: f64, nbpp: u8) -> Result<Vec<u8>, CodecError> {
    match nbpp {
        32 => {
            let v = value as f32;
            Ok(v.to_be_bytes().to_vec())
        }
        64 => {
            Ok(value.to_be_bytes().to_vec())
        }
        _ => Err(CodecError::Encode(format!(
            "Unsupported NBPP {} for real type",
            nbpp
        ))),
    }
}

/// Encode a complex pixel value to bytes.
///
/// # Arguments
/// * `real` - The real component
/// * `imag` - The imaginary component
///
/// # Returns
/// The encoded bytes (8 bytes: two 32-bit floats in big-endian).
fn encode_complex_value(real: f64, imag: f64) -> Vec<u8> {
    let mut result = Vec::with_capacity(8);
    result.extend_from_slice(&(real as f32).to_be_bytes());
    result.extend_from_slice(&(imag as f32).to_be_bytes());
    result
}

/// Pack bi-level pixel values into bytes.
///
/// # Arguments
/// * `values` - The pixel values (0.0 or non-zero for 1)
///
/// # Returns
/// The packed bytes with MSB first.
fn pack_bilevel(values: &[f64]) -> Vec<u8> {
    let num_bytes = values.len().div_ceil(8);
    let mut result = vec![0u8; num_bytes];
    
    for (i, &value) in values.iter().enumerate() {
        if value != 0.0 {
            let byte_index = i / 8;
            let bit_index = i % 8;
            result[byte_index] |= 0x80 >> bit_index;
        }
    }
    
    result
}


/// Encode a single pixel value to raw bytes.
///
/// # Arguments
/// * `value` - The pixel value as f64
/// * `pvtype` - The pixel value type
/// * `nbpp` - Number of bits per pixel
///
/// # Returns
/// The encoded bytes in big-endian order.
///
/// # Errors
/// Returns an error if the PVTYPE/NBPP combination is invalid.
pub fn encode_pixel(value: f64, pvtype: PixelValueType, nbpp: u8) -> Result<Vec<u8>, CodecError> {
    match pvtype {
        PixelValueType::UnsignedInt => encode_unsigned_int(value, nbpp),
        PixelValueType::SignedInt => encode_signed_int(value, nbpp),
        PixelValueType::Real => encode_real(value, nbpp),
        PixelValueType::Complex => {
            if nbpp != 64 {
                return Err(CodecError::Encode(format!(
                    "Complex type requires NBPP=64, got {}",
                    nbpp
                )));
            }
            // For single value encode, treat as real part only with zero imaginary
            Ok(encode_complex_value(value, 0.0))
        }
        PixelValueType::BiLevel => {
            if nbpp != 1 {
                return Err(CodecError::Encode(format!(
                    "BiLevel type requires NBPP=1, got {}",
                    nbpp
                )));
            }
            // For single pixel, pack into one byte
            Ok(pack_bilevel(&[value]))
        }
    }
}

/// Encode a complex pixel value to raw bytes.
///
/// # Arguments
/// * `real` - The real component
/// * `imag` - The imaginary component
///
/// # Returns
/// The encoded bytes (8 bytes: two 32-bit floats in big-endian).
pub fn encode_complex_pixel(real: f64, imag: f64) -> Vec<u8> {
    encode_complex_value(real, imag)
}


/// Apply bit shifting for ABPP < NBPP based on pixel justification.
///
/// When actual bits per pixel (ABPP) is less than storage bits (NBPP),
/// the significant bits may be right-justified (LSB) or left-justified (MSB).
///
/// # Arguments
/// * `value` - The raw decoded value
/// * `abpp` - Actual bits per pixel
/// * `nbpp` - Number of bits per pixel (storage size)
/// * `pjust` - Pixel justification
///
/// # Returns
/// The adjusted value with proper bit alignment.
fn apply_justification_decode(value: f64, abpp: u8, nbpp: u8, pjust: PixelJustification) -> f64 {
    if abpp >= nbpp {
        return value;
    }
    
    match pjust {
        PixelJustification::Right => {
            // Right-justified: significant bits are in LSB positions
            // Mask to keep only ABPP bits
            let mask = (1u64 << abpp) - 1;
            (value as u64 & mask) as f64
        }
        PixelJustification::Left => {
            // Left-justified: significant bits are in MSB positions
            // Shift right to align to LSB
            let shift = nbpp - abpp;
            ((value as u64) >> shift) as f64
        }
    }
}

/// Apply bit shifting for ABPP < NBPP based on pixel justification for encoding.
///
/// # Arguments
/// * `value` - The value to encode
/// * `abpp` - Actual bits per pixel
/// * `nbpp` - Number of bits per pixel (storage size)
/// * `pjust` - Pixel justification
///
/// # Returns
/// The adjusted value with proper bit alignment for storage.
fn apply_justification_encode(value: f64, abpp: u8, nbpp: u8, pjust: PixelJustification) -> f64 {
    if abpp >= nbpp {
        return value;
    }
    
    match pjust {
        PixelJustification::Right => {
            // Right-justified: significant bits go to LSB positions
            // Mask to keep only ABPP bits
            let mask = (1u64 << abpp) - 1;
            (value as u64 & mask) as f64
        }
        PixelJustification::Left => {
            // Left-justified: significant bits go to MSB positions
            // Shift left to align to MSB
            let shift = nbpp - abpp;
            let mask = (1u64 << abpp) - 1;
            ((value as u64 & mask) << shift) as f64
        }
    }
}

/// Decode an array of pixel values from raw bytes.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian)
/// * `pvtype` - The pixel value type
/// * `nbpp` - Number of bits per pixel (storage size)
/// * `abpp` - Actual bits per pixel (significant bits)
/// * `pjust` - Pixel justification
/// * `num_pixels` - Number of pixels to decode
///
/// # Returns
/// A vector of decoded pixel values as f64.
pub fn decode(
    data: &[u8],
    pvtype: PixelValueType,
    nbpp: u8,
    abpp: u8,
    pjust: PixelJustification,
    num_pixels: usize,
) -> Result<Vec<f64>, CodecError> {
    // Handle bi-level specially since it's bit-packed
    if pvtype == PixelValueType::BiLevel {
        if nbpp != 1 {
            return Err(CodecError::Decode(format!(
                "BiLevel type requires NBPP=1, got {}",
                nbpp
            )));
        }
        return Ok(unpack_bilevel(data, num_pixels));
    }
    
    let bpp = bytes_per_pixel(pvtype, nbpp);
    let mut result = Vec::with_capacity(num_pixels);
    
    for i in 0..num_pixels {
        let offset = i * bpp;
        if offset + bpp > data.len() {
            return Err(CodecError::Decode(format!(
                "Insufficient data: need {} bytes at offset {}, have {}",
                bpp, offset, data.len()
            )));
        }
        
        let pixel_data = &data[offset..offset + bpp];
        let value = decode_pixel(pixel_data, pvtype, nbpp)?;
        
        // Apply justification adjustment for integer types
        let adjusted = match pvtype {
            PixelValueType::UnsignedInt | PixelValueType::SignedInt => {
                apply_justification_decode(value, abpp, nbpp, pjust)
            }
            _ => value,
        };
        
        result.push(adjusted);
    }
    
    Ok(result)
}

/// Encode an array of pixel values to raw bytes.
///
/// # Arguments
/// * `values` - The pixel values as f64
/// * `pvtype` - The pixel value type
/// * `nbpp` - Number of bits per pixel (storage size)
/// * `abpp` - Actual bits per pixel (significant bits)
/// * `pjust` - Pixel justification
///
/// # Returns
/// The encoded bytes in big-endian order.
pub fn encode(
    values: &[f64],
    pvtype: PixelValueType,
    nbpp: u8,
    abpp: u8,
    pjust: PixelJustification,
) -> Result<Vec<u8>, CodecError> {
    // Handle bi-level specially since it's bit-packed
    if pvtype == PixelValueType::BiLevel {
        if nbpp != 1 {
            return Err(CodecError::Encode(format!(
                "BiLevel type requires NBPP=1, got {}",
                nbpp
            )));
        }
        return Ok(pack_bilevel(values));
    }
    
    let bpp = bytes_per_pixel(pvtype, nbpp);
    let mut result = Vec::with_capacity(values.len() * bpp);
    
    for &value in values {
        // Apply justification adjustment for integer types
        let adjusted = match pvtype {
            PixelValueType::UnsignedInt | PixelValueType::SignedInt => {
                apply_justification_encode(value, abpp, nbpp, pjust)
            }
            _ => value,
        };
        
        let encoded = encode_pixel(adjusted, pvtype, nbpp)?;
        result.extend_from_slice(&encoded);
    }
    
    Ok(result)
}

/// Decode an array of complex pixel values from raw bytes.
///
/// # Arguments
/// * `data` - The raw bytes (big-endian IEEE format)
/// * `num_pixels` - Number of complex pixels to decode
///
/// # Returns
/// A vector of (real, imaginary) tuples.
pub fn decode_complex_array(data: &[u8], num_pixels: usize) -> Result<Vec<(f64, f64)>, CodecError> {
    let mut result = Vec::with_capacity(num_pixels);
    
    for i in 0..num_pixels {
        let offset = i * 8;
        if offset + 8 > data.len() {
            return Err(CodecError::Decode(format!(
                "Insufficient data for complex pixel {}: need {} bytes, have {}",
                i, offset + 8, data.len()
            )));
        }
        
        let (real, imag) = decode_complex_pixel(&data[offset..offset + 8])?;
        result.push((real, imag));
    }
    
    Ok(result)
}

/// Encode an array of complex pixel values to raw bytes.
///
/// # Arguments
/// * `values` - The complex pixel values as (real, imaginary) tuples
///
/// # Returns
/// The encoded bytes in big-endian IEEE format.
pub fn encode_complex(values: &[(f64, f64)]) -> Vec<u8> {
    let mut result = Vec::with_capacity(values.len() * 8);
    
    for &(real, imag) in values {
        result.extend_from_slice(&encode_complex_pixel(real, imag));
    }
    
    result
}


#[cfg(test)]
mod tests {
    use super::*;

    mod bytes_per_pixel_tests {
        use super::*;

        #[test]
        fn unsigned_int_sizes() {
            assert_eq!(bytes_per_pixel(PixelValueType::UnsignedInt, 8), 1);
            assert_eq!(bytes_per_pixel(PixelValueType::UnsignedInt, 16), 2);
            assert_eq!(bytes_per_pixel(PixelValueType::UnsignedInt, 32), 4);
        }

        #[test]
        fn signed_int_sizes() {
            assert_eq!(bytes_per_pixel(PixelValueType::SignedInt, 8), 1);
            assert_eq!(bytes_per_pixel(PixelValueType::SignedInt, 16), 2);
            assert_eq!(bytes_per_pixel(PixelValueType::SignedInt, 32), 4);
        }

        #[test]
        fn real_sizes() {
            assert_eq!(bytes_per_pixel(PixelValueType::Real, 32), 4);
            assert_eq!(bytes_per_pixel(PixelValueType::Real, 64), 8);
        }

        #[test]
        fn complex_size() {
            assert_eq!(bytes_per_pixel(PixelValueType::Complex, 64), 8);
        }

        #[test]
        fn bilevel_size() {
            assert_eq!(bytes_per_pixel(PixelValueType::BiLevel, 1), 1);
        }
    }

    mod decode_pixel_tests {
        use super::*;

        #[test]
        fn decode_u8() {
            let data = [0x42];
            let value = decode_pixel(&data, PixelValueType::UnsignedInt, 8).unwrap();
            assert_eq!(value, 66.0);
        }

        #[test]
        fn decode_u16_big_endian() {
            let data = [0x01, 0x00]; // 256 in big-endian
            let value = decode_pixel(&data, PixelValueType::UnsignedInt, 16).unwrap();
            assert_eq!(value, 256.0);
        }

        #[test]
        fn decode_u32_big_endian() {
            let data = [0x00, 0x01, 0x00, 0x00]; // 65536 in big-endian
            let value = decode_pixel(&data, PixelValueType::UnsignedInt, 32).unwrap();
            assert_eq!(value, 65536.0);
        }

        #[test]
        fn decode_i8_positive() {
            let data = [0x7F]; // 127
            let value = decode_pixel(&data, PixelValueType::SignedInt, 8).unwrap();
            assert_eq!(value, 127.0);
        }

        #[test]
        fn decode_i8_negative() {
            let data = [0xFF]; // -1 as i8
            let value = decode_pixel(&data, PixelValueType::SignedInt, 8).unwrap();
            assert_eq!(value, -1.0);
        }

        #[test]
        fn decode_i16_negative() {
            let data = [0xFF, 0xFF]; // -1 as i16
            let value = decode_pixel(&data, PixelValueType::SignedInt, 16).unwrap();
            assert_eq!(value, -1.0);
        }

        #[test]
        fn decode_i32_negative() {
            let data = [0xFF, 0xFF, 0xFF, 0xFE]; // -2 as i32
            let value = decode_pixel(&data, PixelValueType::SignedInt, 32).unwrap();
            assert_eq!(value, -2.0);
        }

        #[test]
        fn decode_f32() {
            let value: f32 = 3.14;
            let data = value.to_be_bytes();
            let decoded = decode_pixel(&data, PixelValueType::Real, 32).unwrap();
            assert!((decoded - 3.14).abs() < 0.001);
        }

        #[test]
        fn decode_f64() {
            let value: f64 = 3.14159265358979;
            let data = value.to_be_bytes();
            let decoded = decode_pixel(&data, PixelValueType::Real, 64).unwrap();
            assert!((decoded - 3.14159265358979).abs() < 1e-10);
        }

        #[test]
        fn decode_complex() {
            let real: f32 = 1.5;
            let imag: f32 = 2.5;
            let mut data = Vec::new();
            data.extend_from_slice(&real.to_be_bytes());
            data.extend_from_slice(&imag.to_be_bytes());
            
            let (r, i) = decode_complex_pixel(&data).unwrap();
            assert!((r - 1.5).abs() < 0.001);
            assert!((i - 2.5).abs() < 0.001);
        }

        #[test]
        fn decode_bilevel_first_bit() {
            let data = [0b10000000]; // First bit set
            let value = decode_pixel(&data, PixelValueType::BiLevel, 1).unwrap();
            assert_eq!(value, 1.0);
        }

        #[test]
        fn decode_bilevel_first_bit_zero() {
            let data = [0b01111111]; // First bit not set
            let value = decode_pixel(&data, PixelValueType::BiLevel, 1).unwrap();
            assert_eq!(value, 0.0);
        }
    }

    mod encode_pixel_tests {
        use super::*;

        #[test]
        fn encode_u8() {
            let encoded = encode_pixel(66.0, PixelValueType::UnsignedInt, 8).unwrap();
            assert_eq!(encoded, vec![0x42]);
        }

        #[test]
        fn encode_u16() {
            let encoded = encode_pixel(256.0, PixelValueType::UnsignedInt, 16).unwrap();
            assert_eq!(encoded, vec![0x01, 0x00]);
        }

        #[test]
        fn encode_u32() {
            let encoded = encode_pixel(65536.0, PixelValueType::UnsignedInt, 32).unwrap();
            assert_eq!(encoded, vec![0x00, 0x01, 0x00, 0x00]);
        }

        #[test]
        fn encode_i8_negative() {
            let encoded = encode_pixel(-1.0, PixelValueType::SignedInt, 8).unwrap();
            assert_eq!(encoded, vec![0xFF]);
        }

        #[test]
        fn encode_i16_negative() {
            let encoded = encode_pixel(-1.0, PixelValueType::SignedInt, 16).unwrap();
            assert_eq!(encoded, vec![0xFF, 0xFF]);
        }

        #[test]
        fn encode_f32() {
            let encoded = encode_pixel(3.14, PixelValueType::Real, 32).unwrap();
            let decoded = f32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]);
            assert!((decoded - 3.14).abs() < 0.001);
        }

        #[test]
        fn encode_f64() {
            let encoded = encode_pixel(3.14159265358979, PixelValueType::Real, 64).unwrap();
            let decoded = f64::from_be_bytes([
                encoded[0], encoded[1], encoded[2], encoded[3],
                encoded[4], encoded[5], encoded[6], encoded[7],
            ]);
            assert!((decoded - 3.14159265358979).abs() < 1e-10);
        }

        #[test]
        fn encode_complex() {
            let encoded = encode_complex_pixel(1.5, 2.5);
            assert_eq!(encoded.len(), 8);
            
            let real = f32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]);
            let imag = f32::from_be_bytes([encoded[4], encoded[5], encoded[6], encoded[7]]);
            assert!((real - 1.5).abs() < 0.001);
            assert!((imag - 2.5).abs() < 0.001);
        }
    }

    mod bilevel_tests {
        use super::*;

        #[test]
        fn unpack_bilevel_all_ones() {
            let data = [0xFF];
            let unpacked = unpack_bilevel(&data, 8);
            assert_eq!(unpacked, vec![1.0; 8]);
        }

        #[test]
        fn unpack_bilevel_all_zeros() {
            let data = [0x00];
            let unpacked = unpack_bilevel(&data, 8);
            assert_eq!(unpacked, vec![0.0; 8]);
        }

        #[test]
        fn unpack_bilevel_alternating() {
            let data = [0b10101010];
            let unpacked = unpack_bilevel(&data, 8);
            assert_eq!(unpacked, vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
        }

        #[test]
        fn unpack_bilevel_partial_byte() {
            let data = [0b11100000];
            let unpacked = unpack_bilevel(&data, 5);
            assert_eq!(unpacked, vec![1.0, 1.0, 1.0, 0.0, 0.0]);
        }

        #[test]
        fn pack_bilevel_all_ones() {
            let values = vec![1.0; 8];
            let packed = pack_bilevel(&values);
            assert_eq!(packed, vec![0xFF]);
        }

        #[test]
        fn pack_bilevel_all_zeros() {
            let values = vec![0.0; 8];
            let packed = pack_bilevel(&values);
            assert_eq!(packed, vec![0x00]);
        }

        #[test]
        fn pack_bilevel_alternating() {
            let values = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
            let packed = pack_bilevel(&values);
            assert_eq!(packed, vec![0b10101010]);
        }

        #[test]
        fn pack_bilevel_partial_byte() {
            let values = vec![1.0, 1.0, 1.0, 0.0, 0.0];
            let packed = pack_bilevel(&values);
            assert_eq!(packed, vec![0b11100000]);
        }

        #[test]
        fn bilevel_round_trip() {
            let original = vec![1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0];
            let packed = pack_bilevel(&original);
            let unpacked = unpack_bilevel(&packed, original.len());
            assert_eq!(unpacked, original);
        }
    }

    mod bulk_encode_decode_tests {
        use super::*;

        #[test]
        fn bulk_u8_round_trip() {
            let values = vec![0.0, 127.0, 255.0, 42.0];
            let encoded = encode(&values, PixelValueType::UnsignedInt, 8, 8, PixelJustification::Right).unwrap();
            let decoded = decode(&encoded, PixelValueType::UnsignedInt, 8, 8, PixelJustification::Right, 4).unwrap();
            assert_eq!(decoded, values);
        }

        #[test]
        fn bulk_u16_round_trip() {
            let values = vec![0.0, 256.0, 65535.0, 1000.0];
            let encoded = encode(&values, PixelValueType::UnsignedInt, 16, 16, PixelJustification::Right).unwrap();
            let decoded = decode(&encoded, PixelValueType::UnsignedInt, 16, 16, PixelJustification::Right, 4).unwrap();
            assert_eq!(decoded, values);
        }

        #[test]
        fn bulk_i16_round_trip() {
            let values = vec![-32768.0, -1.0, 0.0, 32767.0];
            let encoded = encode(&values, PixelValueType::SignedInt, 16, 16, PixelJustification::Right).unwrap();
            let decoded = decode(&encoded, PixelValueType::SignedInt, 16, 16, PixelJustification::Right, 4).unwrap();
            assert_eq!(decoded, values);
        }

        #[test]
        fn bulk_f32_round_trip() {
            let values = vec![0.0, 1.5, -3.14, 1000.0];
            let encoded = encode(&values, PixelValueType::Real, 32, 32, PixelJustification::Right).unwrap();
            let decoded = decode(&encoded, PixelValueType::Real, 32, 32, PixelJustification::Right, 4).unwrap();
            
            for (orig, dec) in values.iter().zip(decoded.iter()) {
                assert!((orig - dec).abs() < 0.001);
            }
        }

        #[test]
        fn bulk_complex_round_trip() {
            let values = vec![(1.0, 2.0), (3.0, 4.0), (-1.5, 2.5)];
            let encoded = encode_complex(&values);
            let decoded = decode_complex_array(&encoded, 3).unwrap();
            
            for (i, ((r1, i1), (r2, i2))) in values.iter().zip(decoded.iter()).enumerate() {
                assert!((*r1 - *r2).abs() < 0.001, "Real mismatch at index {}", i);
                assert!((*i1 - *i2).abs() < 0.001, "Imag mismatch at index {}", i);
            }
        }

        #[test]
        fn bulk_bilevel_round_trip() {
            let values = vec![1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
            let encoded = encode(&values, PixelValueType::BiLevel, 1, 1, PixelJustification::Right).unwrap();
            let decoded = decode(&encoded, PixelValueType::BiLevel, 1, 1, PixelJustification::Right, values.len()).unwrap();
            assert_eq!(decoded, values);
        }
    }

    mod justification_tests {
        use super::*;

        #[test]
        fn right_justified_12bit_in_16bit() {
            // 12-bit value 0xABC stored right-justified in 16 bits
            let value = 0x0ABC as f64;
            let adjusted = apply_justification_decode(value, 12, 16, PixelJustification::Right);
            assert_eq!(adjusted, 0x0ABC as f64);
        }

        #[test]
        fn left_justified_12bit_in_16bit() {
            // 12-bit value stored left-justified: 0xABC0
            let stored = 0xABC0 as f64;
            let adjusted = apply_justification_decode(stored, 12, 16, PixelJustification::Left);
            assert_eq!(adjusted, 0x0ABC as f64);
        }

        #[test]
        fn encode_right_justified() {
            let value = 0x0ABC as f64;
            let adjusted = apply_justification_encode(value, 12, 16, PixelJustification::Right);
            assert_eq!(adjusted, 0x0ABC as f64);
        }

        #[test]
        fn encode_left_justified() {
            let value = 0x0ABC as f64;
            let adjusted = apply_justification_encode(value, 12, 16, PixelJustification::Left);
            assert_eq!(adjusted, 0xABC0 as f64);
        }

        #[test]
        fn justification_round_trip_right() {
            let original = 0x0ABC as f64;
            let encoded = apply_justification_encode(original, 12, 16, PixelJustification::Right);
            let decoded = apply_justification_decode(encoded, 12, 16, PixelJustification::Right);
            assert_eq!(decoded, original);
        }

        #[test]
        fn justification_round_trip_left() {
            let original = 0x0ABC as f64;
            let encoded = apply_justification_encode(original, 12, 16, PixelJustification::Left);
            let decoded = apply_justification_decode(encoded, 12, 16, PixelJustification::Left);
            assert_eq!(decoded, original);
        }
    }

    /// Property-based tests for pixel encoding/decoding
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        /// Property 5: Pixel Value Type Round-Trip
        /// For any valid pixel value and PVTYPE/NBPP combination, encoding the value
        /// and then decoding it SHALL produce an equivalent value (within floating-point
        /// tolerance for R and C types).
        /// **Validates: Requirements 5.7-5.12, 11.1-11.10**
        mod pixel_round_trip {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(100))]

                #[test]
                fn unsigned_int_8bit_round_trip(value in 0u8..=255u8) {
                    let encoded = encode_pixel(value as f64, PixelValueType::UnsignedInt, 8).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::UnsignedInt, 8).unwrap();
                    prop_assert_eq!(decoded as u8, value);
                }

                #[test]
                fn unsigned_int_16bit_round_trip(value in 0u16..=65535u16) {
                    let encoded = encode_pixel(value as f64, PixelValueType::UnsignedInt, 16).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::UnsignedInt, 16).unwrap();
                    prop_assert_eq!(decoded as u16, value);
                }

                #[test]
                fn unsigned_int_32bit_round_trip(value in 0u32..=u32::MAX) {
                    let encoded = encode_pixel(value as f64, PixelValueType::UnsignedInt, 32).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::UnsignedInt, 32).unwrap();
                    prop_assert_eq!(decoded as u32, value);
                }

                #[test]
                fn signed_int_8bit_round_trip(value in i8::MIN..=i8::MAX) {
                    let encoded = encode_pixel(value as f64, PixelValueType::SignedInt, 8).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::SignedInt, 8).unwrap();
                    prop_assert_eq!(decoded as i8, value);
                }

                #[test]
                fn signed_int_16bit_round_trip(value in i16::MIN..=i16::MAX) {
                    let encoded = encode_pixel(value as f64, PixelValueType::SignedInt, 16).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::SignedInt, 16).unwrap();
                    prop_assert_eq!(decoded as i16, value);
                }

                #[test]
                fn signed_int_32bit_round_trip(value in i32::MIN..=i32::MAX) {
                    let encoded = encode_pixel(value as f64, PixelValueType::SignedInt, 32).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::SignedInt, 32).unwrap();
                    prop_assert_eq!(decoded as i32, value);
                }

                #[test]
                fn real_32bit_round_trip(value in proptest::num::f32::NORMAL) {
                    let encoded = encode_pixel(value as f64, PixelValueType::Real, 32).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::Real, 32).unwrap();
                    // f32 precision: compare as f32
                    prop_assert!((decoded as f32 - value).abs() < f32::EPSILON * 10.0,
                        "Expected {} but got {}", value, decoded);
                }

                #[test]
                fn real_64bit_round_trip(value in proptest::num::f64::NORMAL) {
                    let encoded = encode_pixel(value, PixelValueType::Real, 64).unwrap();
                    let decoded = decode_pixel(&encoded, PixelValueType::Real, 64).unwrap();
                    prop_assert!((decoded - value).abs() < f64::EPSILON * 10.0,
                        "Expected {} but got {}", value, decoded);
                }

                #[test]
                fn complex_round_trip(
                    real in proptest::num::f32::NORMAL,
                    imag in proptest::num::f32::NORMAL
                ) {
                    let encoded = encode_complex_pixel(real as f64, imag as f64);
                    let (dec_real, dec_imag) = decode_complex_pixel(&encoded).unwrap();
                    prop_assert!((dec_real as f32 - real).abs() < f32::EPSILON * 10.0,
                        "Real: expected {} but got {}", real, dec_real);
                    prop_assert!((dec_imag as f32 - imag).abs() < f32::EPSILON * 10.0,
                        "Imag: expected {} but got {}", imag, dec_imag);
                }

                #[test]
                fn bilevel_round_trip(bits in proptest::collection::vec(proptest::bool::ANY, 1..100)) {
                    let values: Vec<f64> = bits.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
                    let packed = pack_bilevel(&values);
                    let unpacked = unpack_bilevel(&packed, values.len());
                    prop_assert_eq!(unpacked, values);
                }
            }

            proptest! {
                #![proptest_config(ProptestConfig::with_cases(100))]

                /// Test bulk encode/decode round-trip for unsigned integers
                #[test]
                fn bulk_unsigned_int_round_trip(
                    values in proptest::collection::vec(0u8..=255u8, 1..50)
                ) {
                    let f64_values: Vec<f64> = values.iter().map(|&v| v as f64).collect();
                    let encoded = encode(&f64_values, PixelValueType::UnsignedInt, 8, 8, PixelJustification::Right).unwrap();
                    let decoded = decode(&encoded, PixelValueType::UnsignedInt, 8, 8, PixelJustification::Right, values.len()).unwrap();
                    
                    for (orig, dec) in f64_values.iter().zip(decoded.iter()) {
                        prop_assert_eq!(*orig as u8, *dec as u8);
                    }
                }

                /// Test bulk encode/decode round-trip for signed integers
                #[test]
                fn bulk_signed_int_round_trip(
                    values in proptest::collection::vec(i16::MIN..=i16::MAX, 1..50)
                ) {
                    let f64_values: Vec<f64> = values.iter().map(|&v| v as f64).collect();
                    let encoded = encode(&f64_values, PixelValueType::SignedInt, 16, 16, PixelJustification::Right).unwrap();
                    let decoded = decode(&encoded, PixelValueType::SignedInt, 16, 16, PixelJustification::Right, values.len()).unwrap();
                    
                    for (orig, dec) in f64_values.iter().zip(decoded.iter()) {
                        prop_assert_eq!(*orig as i16, *dec as i16);
                    }
                }

                /// Test justification round-trip for various ABPP/NBPP combinations
                #[test]
                fn justification_round_trip(
                    value in 0u16..4096u16,  // 12-bit max
                    pjust in proptest::bool::ANY
                ) {
                    let pjust = if pjust { PixelJustification::Right } else { PixelJustification::Left };
                    let abpp = 12u8;
                    let nbpp = 16u8;
                    
                    let encoded = apply_justification_encode(value as f64, abpp, nbpp, pjust);
                    let decoded = apply_justification_decode(encoded, abpp, nbpp, pjust);
                    prop_assert_eq!(decoded as u16, value);
                }
            }
        }
    }
}
