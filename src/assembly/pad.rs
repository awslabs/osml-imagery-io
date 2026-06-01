use crate::types::PixelType;

/// Convert a pad pixel value to a byte pattern for the given pixel type.
///
/// Returns one pixel's worth of bytes in native endian, suitable for
/// repeating across a buffer to initialize pad regions.
pub fn pad_pixel_bytes(value: f64, pixel_type: PixelType) -> Vec<u8> {
    match pixel_type {
        PixelType::UInt8 => vec![value as u8],
        PixelType::Int8 => vec![(value as i8) as u8],
        PixelType::UInt16 => (value as u16).to_ne_bytes().to_vec(),
        PixelType::Int16 => (value as i16).to_ne_bytes().to_vec(),
        PixelType::UInt32 => (value as u32).to_ne_bytes().to_vec(),
        PixelType::Int32 => (value as i32).to_ne_bytes().to_vec(),
        PixelType::Float32 => (value as f32).to_ne_bytes().to_vec(),
        PixelType::Float64 => value.to_ne_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_pixel_bytes_uint8_zero() {
        let result = pad_pixel_bytes(0.0, PixelType::UInt8);
        assert_eq!(result, vec![0u8]);
    }

    #[test]
    fn pad_pixel_bytes_uint8_max() {
        let result = pad_pixel_bytes(255.0, PixelType::UInt8);
        assert_eq!(result, vec![255u8]);
    }

    #[test]
    fn pad_pixel_bytes_uint8_truncates() {
        let result = pad_pixel_bytes(42.7, PixelType::UInt8);
        assert_eq!(result, vec![42u8]);
    }

    #[test]
    fn pad_pixel_bytes_int8_negative() {
        let result = pad_pixel_bytes(-1.0, PixelType::Int8);
        assert_eq!(result, vec![(-1i8) as u8]);
    }

    #[test]
    fn pad_pixel_bytes_uint16() {
        let result = pad_pixel_bytes(1000.0, PixelType::UInt16);
        assert_eq!(result, 1000u16.to_ne_bytes().to_vec());
    }

    #[test]
    fn pad_pixel_bytes_int16_negative() {
        let result = pad_pixel_bytes(-500.0, PixelType::Int16);
        assert_eq!(result, (-500i16).to_ne_bytes().to_vec());
    }

    #[test]
    fn pad_pixel_bytes_uint32() {
        let result = pad_pixel_bytes(100000.0, PixelType::UInt32);
        assert_eq!(result, 100000u32.to_ne_bytes().to_vec());
    }

    #[test]
    fn pad_pixel_bytes_int32_negative() {
        let result = pad_pixel_bytes(-100000.0, PixelType::Int32);
        assert_eq!(result, (-100000i32).to_ne_bytes().to_vec());
    }

    #[test]
    fn pad_pixel_bytes_float32() {
        let result = pad_pixel_bytes(1.5, PixelType::Float32);
        assert_eq!(result, (1.5f32).to_ne_bytes().to_vec());
    }

    #[test]
    fn pad_pixel_bytes_float64() {
        let result = pad_pixel_bytes(std::f64::consts::PI, PixelType::Float64);
        assert_eq!(result, std::f64::consts::PI.to_ne_bytes().to_vec());
    }

    #[test]
    fn pad_pixel_bytes_byte_lengths() {
        assert_eq!(pad_pixel_bytes(0.0, PixelType::UInt8).len(), 1);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::Int8).len(), 1);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::UInt16).len(), 2);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::Int16).len(), 2);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::UInt32).len(), 4);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::Int32).len(), 4);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::Float32).len(), 4);
        assert_eq!(pad_pixel_bytes(0.0, PixelType::Float64).len(), 8);
    }
}
