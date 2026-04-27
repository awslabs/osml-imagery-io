//! Core types for image segment handling.
//!
//! This module defines enumerations for pixel value types, image representation,
//! interleave modes, and pixel justification used in NITF image subheaders.

use std::str::FromStr;

use crate::error::CodecError;
use crate::types::PixelType;

/// Pixel value type (PVTYPE field) indicating the data type of pixel values.
///
/// This enum represents the NITF PVTYPE field which specifies how pixel
/// values should be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelValueType {
    /// Unsigned integer (INT) - NBPP can be 8, 16, or 32
    UnsignedInt,
    /// Signed integer (SI) - NBPP can be 8, 16, or 32
    SignedInt,
    /// IEEE floating-point (R) - NBPP must be 32 or 64
    Real,
    /// Complex number (C) - pairs of floats, NBPP must be 64
    Complex,
    /// Bi-level (B) - 1-bit values packed into bytes, NBPP must be 1
    BiLevel,
}

impl FromStr for PixelValueType {
    type Err = CodecError;

    /// Parse from PVTYPE string field (3 characters, space-padded).
    fn from_str(s: &str) -> Result<Self, CodecError> {
        match s.trim() {
            "INT" => Ok(PixelValueType::UnsignedInt),
            "SI" => Ok(PixelValueType::SignedInt),
            "R" => Ok(PixelValueType::Real),
            "C" => Ok(PixelValueType::Complex),
            "B" => Ok(PixelValueType::BiLevel),
            _ => Err(CodecError::Parse(format!("Invalid PVTYPE value: '{}'", s))),
        }
    }
}

impl PixelValueType {
    /// Convert to PVTYPE string for writing (3 characters, space-padded).
    pub fn to_str(&self) -> &'static str {
        match self {
            PixelValueType::UnsignedInt => "INT",
            PixelValueType::SignedInt => "SI ",
            PixelValueType::Real => "R  ",
            PixelValueType::Complex => "C  ",
            PixelValueType::BiLevel => "B  ",
        }
    }

    /// Convert to `PixelType` based on NBPP (number of bits per pixel).
    pub fn to_pixel_type(&self, nbpp: u8) -> PixelType {
        match self {
            PixelValueType::UnsignedInt => match nbpp {
                1..=8 => PixelType::UInt8,
                9..=16 => PixelType::UInt16,
                17..=32 => PixelType::UInt32,
                _ => PixelType::UInt32,
            },
            PixelValueType::SignedInt => match nbpp {
                1..=8 => PixelType::Int8,
                9..=16 => PixelType::Int16,
                17..=32 => PixelType::Int32,
                _ => PixelType::Int32,
            },
            PixelValueType::Real => match nbpp {
                32 => PixelType::Float32,
                64 => PixelType::Float64,
                _ => PixelType::Float32,
            },
            PixelValueType::Complex => PixelType::Float32,
            PixelValueType::BiLevel => PixelType::UInt8,
        }
    }
}

/// Image representation (IREP field) describing how the image should be displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRepresentation {
    /// Monochrome image (single band)
    Mono,
    /// RGB true color image (3 bands: R, G, B)
    Rgb,
    /// RGB with look-up table (1 band with 3 LUTs)
    RgbLut,
    /// Multispectral image (any number of bands)
    Multi,
    /// No display intended
    NoDisplay,
    /// N-vector representation
    NVector,
    /// Polar representation
    Polar,
    /// VPH (Video Phase History)
    Vph,
    /// YCbCr 601 color space (3 bands: Y, Cb, Cr)
    YCbCr601,
}

impl FromStr for ImageRepresentation {
    type Err = CodecError;

    /// Parse from IREP string field (8 characters, space-padded).
    fn from_str(s: &str) -> Result<Self, CodecError> {
        match s.trim() {
            "MONO" => Ok(ImageRepresentation::Mono),
            "RGB" => Ok(ImageRepresentation::Rgb),
            "RGB/LUT" => Ok(ImageRepresentation::RgbLut),
            "MULTI" => Ok(ImageRepresentation::Multi),
            "NODISPLY" => Ok(ImageRepresentation::NoDisplay),
            "NVECTOR" => Ok(ImageRepresentation::NVector),
            "POLAR" => Ok(ImageRepresentation::Polar),
            "VPH" => Ok(ImageRepresentation::Vph),
            "YCbCr601" => Ok(ImageRepresentation::YCbCr601),
            _ => Err(CodecError::Parse(format!("Invalid IREP value: '{}'", s))),
        }
    }
}

impl ImageRepresentation {
    /// Convert to IREP string for writing (8 characters, space-padded).
    pub fn to_str(&self) -> &'static str {
        match self {
            ImageRepresentation::Mono => "MONO    ",
            ImageRepresentation::Rgb => "RGB     ",
            ImageRepresentation::RgbLut => "RGB/LUT ",
            ImageRepresentation::Multi => "MULTI   ",
            ImageRepresentation::NoDisplay => "NODISPLY",
            ImageRepresentation::NVector => "NVECTOR ",
            ImageRepresentation::Polar => "POLAR   ",
            ImageRepresentation::Vph => "VPH     ",
            ImageRepresentation::YCbCr601 => "YCbCr601",
        }
    }

    /// Get the expected band count for this representation.
    pub fn expected_band_count(&self) -> Option<usize> {
        match self {
            ImageRepresentation::Mono => Some(1),
            ImageRepresentation::Rgb => Some(3),
            ImageRepresentation::RgbLut => Some(1),
            ImageRepresentation::YCbCr601 => Some(3),
            _ => None,
        }
    }
}

/// Image interleave mode (IMODE field) specifying how multi-band data is organized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterleaveMode {
    /// Band interleaved by block
    B,
    /// Band interleaved by pixel
    P,
    /// Band interleaved by row
    R,
    /// Band sequential
    S,
}

impl InterleaveMode {
    /// Parse from IMODE character field.
    pub fn from_char(c: char) -> Result<Self, CodecError> {
        match c {
            'B' => Ok(InterleaveMode::B),
            'P' => Ok(InterleaveMode::P),
            'R' => Ok(InterleaveMode::R),
            'S' => Ok(InterleaveMode::S),
            _ => Err(CodecError::Parse(format!("Invalid IMODE value: '{}'", c))),
        }
    }

    /// Convert to IMODE character for writing.
    pub fn to_char(&self) -> char {
        match self {
            InterleaveMode::B => 'B',
            InterleaveMode::P => 'P',
            InterleaveMode::R => 'R',
            InterleaveMode::S => 'S',
        }
    }
}

/// Pixel justification (PJUST field) indicating bit alignment within storage.
///
/// This enum represents the NITF PJUST field which specifies whether pixel
/// values are right-justified or left-justified within their storage bytes
/// when ABPP < NBPP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelJustification {
    /// Right-justified - significant bits are in the least significant positions
    Right,
    /// Left-justified - significant bits are in the most significant positions
    Left,
}

impl PixelJustification {
    /// Parse from PJUST character field.
    ///
    /// # Arguments
    /// * `c` - The PJUST field value ('R' or 'L')
    ///
    /// # Returns
    /// The corresponding `PixelJustification` variant, or an error if invalid.
    pub fn from_char(c: char) -> Result<Self, CodecError> {
        match c {
            'R' => Ok(PixelJustification::Right),
            'L' => Ok(PixelJustification::Left),
            _ => Err(CodecError::Parse(format!("Invalid PJUST value: '{}'", c))),
        }
    }

    /// Convert to PJUST character for writing.
    ///
    /// # Returns
    /// The PJUST field value as a single character.
    pub fn to_char(&self) -> char {
        match self {
            PixelJustification::Right => 'R',
            PixelJustification::Left => 'L',
        }
    }
}

/// Look-up table for indexed color mapping.
///
/// A LUT maps pixel values to display values. In NITF, each band can have
/// up to 4 LUTs, and for RGB/LUT images, there are typically 3 LUTs (R, G, B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookUpTable {
    /// LUT entries (raw bytes)
    pub entries: Vec<u8>,
}

impl LookUpTable {
    /// Create a new LookUpTable from raw bytes.
    ///
    /// # Arguments
    /// * `data` - The raw LUT data bytes
    ///
    /// # Returns
    /// A new `LookUpTable` containing the provided data.
    pub fn from_bytes(data: &[u8]) -> Self {
        Self {
            entries: data.to_vec(),
        }
    }

    /// Apply the LUT to a pixel value.
    ///
    /// # Arguments
    /// * `value` - The pixel value to look up (index into the LUT)
    ///
    /// # Returns
    /// The mapped value from the LUT, or an error if the value exceeds the LUT size.
    pub fn apply(&self, value: u8) -> Result<u8, CodecError> {
        let index = value as usize;
        if index >= self.entries.len() {
            return Err(CodecError::Parse(format!(
                "LUT index {} out of range (LUT size: {})",
                index,
                self.entries.len()
            )));
        }
        Ok(self.entries[index])
    }

    /// Get the number of entries in the LUT.
    ///
    /// # Returns
    /// The number of entries in the LUT.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the LUT is empty.
    ///
    /// # Returns
    /// `true` if the LUT has no entries, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the raw bytes of the LUT.
    ///
    /// # Returns
    /// A slice of the LUT entries.
    pub fn as_bytes(&self) -> &[u8] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // LookUpTable tests
    mod look_up_table {
        use super::*;

        #[test]
        fn from_bytes_creates_lut() {
            let data = vec![0, 10, 20, 30, 40];
            let lut = LookUpTable::from_bytes(&data);
            assert_eq!(lut.len(), 5);
            assert_eq!(lut.entries, data);
        }

        #[test]
        fn from_bytes_empty() {
            let lut = LookUpTable::from_bytes(&[]);
            assert!(lut.is_empty());
            assert_eq!(lut.len(), 0);
        }

        #[test]
        fn apply_valid_index() {
            let data = vec![100, 150, 200, 250];
            let lut = LookUpTable::from_bytes(&data);

            assert_eq!(lut.apply(0).unwrap(), 100);
            assert_eq!(lut.apply(1).unwrap(), 150);
            assert_eq!(lut.apply(2).unwrap(), 200);
            assert_eq!(lut.apply(3).unwrap(), 250);
        }

        #[test]
        fn apply_out_of_range() {
            let data = vec![100, 150, 200];
            let lut = LookUpTable::from_bytes(&data);

            assert!(lut.apply(3).is_err());
            assert!(lut.apply(255).is_err());
        }

        #[test]
        fn apply_empty_lut() {
            let lut = LookUpTable::from_bytes(&[]);
            assert!(lut.apply(0).is_err());
        }

        #[test]
        fn as_bytes_returns_entries() {
            let data = vec![1, 2, 3, 4, 5];
            let lut = LookUpTable::from_bytes(&data);
            assert_eq!(lut.as_bytes(), &data[..]);
        }

        #[test]
        fn len_and_is_empty() {
            let lut = LookUpTable::from_bytes(&[1, 2, 3]);
            assert_eq!(lut.len(), 3);
            assert!(!lut.is_empty());

            let empty_lut = LookUpTable::from_bytes(&[]);
            assert_eq!(empty_lut.len(), 0);
            assert!(empty_lut.is_empty());
        }
    }

    // PixelValueType tests
    mod pixel_value_type {
        use super::*;

        #[test]
        fn from_str_valid_values() {
            assert_eq!(
                PixelValueType::from_str("INT").unwrap(),
                PixelValueType::UnsignedInt
            );
            assert_eq!(
                PixelValueType::from_str("SI").unwrap(),
                PixelValueType::SignedInt
            );
            assert_eq!(PixelValueType::from_str("R").unwrap(), PixelValueType::Real);
            assert_eq!(
                PixelValueType::from_str("C").unwrap(),
                PixelValueType::Complex
            );
            assert_eq!(
                PixelValueType::from_str("B").unwrap(),
                PixelValueType::BiLevel
            );
        }

        #[test]
        fn from_str_with_padding() {
            assert_eq!(
                PixelValueType::from_str("SI ").unwrap(),
                PixelValueType::SignedInt
            );
            assert_eq!(
                PixelValueType::from_str("R  ").unwrap(),
                PixelValueType::Real
            );
            assert_eq!(
                PixelValueType::from_str("C  ").unwrap(),
                PixelValueType::Complex
            );
            assert_eq!(
                PixelValueType::from_str("B  ").unwrap(),
                PixelValueType::BiLevel
            );
        }

        #[test]
        fn from_str_invalid() {
            assert!(PixelValueType::from_str("INVALID").is_err());
            assert!(PixelValueType::from_str("").is_err());
            assert!(PixelValueType::from_str("X").is_err());
        }

        #[test]
        fn to_str_round_trip() {
            let variants = [
                PixelValueType::UnsignedInt,
                PixelValueType::SignedInt,
                PixelValueType::Real,
                PixelValueType::Complex,
                PixelValueType::BiLevel,
            ];
            for variant in variants {
                assert_eq!(PixelValueType::from_str(variant.to_str()).unwrap(), variant);
            }
        }

        #[test]
        fn to_pixel_type_unsigned_int() {
            assert_eq!(
                PixelValueType::UnsignedInt.to_pixel_type(8),
                PixelType::UInt8
            );
            assert_eq!(
                PixelValueType::UnsignedInt.to_pixel_type(16),
                PixelType::UInt16
            );
            assert_eq!(
                PixelValueType::UnsignedInt.to_pixel_type(32),
                PixelType::UInt32
            );
        }

        #[test]
        fn to_pixel_type_signed_int() {
            assert_eq!(PixelValueType::SignedInt.to_pixel_type(8), PixelType::Int8);
            assert_eq!(
                PixelValueType::SignedInt.to_pixel_type(16),
                PixelType::Int16
            );
            assert_eq!(
                PixelValueType::SignedInt.to_pixel_type(32),
                PixelType::Int32
            );
        }

        #[test]
        fn to_pixel_type_real() {
            assert_eq!(PixelValueType::Real.to_pixel_type(32), PixelType::Float32);
            assert_eq!(PixelValueType::Real.to_pixel_type(64), PixelType::Float64);
        }

        #[test]
        fn to_pixel_type_complex() {
            assert_eq!(
                PixelValueType::Complex.to_pixel_type(64),
                PixelType::Float32
            );
        }

        #[test]
        fn to_pixel_type_bilevel() {
            assert_eq!(PixelValueType::BiLevel.to_pixel_type(1), PixelType::UInt8);
        }
    }

    // ImageRepresentation tests
    mod image_representation {
        use super::*;

        #[test]
        fn from_str_valid_values() {
            assert_eq!(
                ImageRepresentation::from_str("MONO").unwrap(),
                ImageRepresentation::Mono
            );
            assert_eq!(
                ImageRepresentation::from_str("RGB").unwrap(),
                ImageRepresentation::Rgb
            );
            assert_eq!(
                ImageRepresentation::from_str("RGB/LUT").unwrap(),
                ImageRepresentation::RgbLut
            );
            assert_eq!(
                ImageRepresentation::from_str("MULTI").unwrap(),
                ImageRepresentation::Multi
            );
            assert_eq!(
                ImageRepresentation::from_str("NODISPLY").unwrap(),
                ImageRepresentation::NoDisplay
            );
            assert_eq!(
                ImageRepresentation::from_str("YCbCr601").unwrap(),
                ImageRepresentation::YCbCr601
            );
        }

        #[test]
        fn from_str_invalid() {
            assert!(ImageRepresentation::from_str("INVALID").is_err());
            assert!(ImageRepresentation::from_str("").is_err());
        }

        #[test]
        fn to_str_round_trip() {
            let variants = [
                ImageRepresentation::Mono,
                ImageRepresentation::Rgb,
                ImageRepresentation::RgbLut,
                ImageRepresentation::Multi,
                ImageRepresentation::NoDisplay,
                ImageRepresentation::NVector,
                ImageRepresentation::Polar,
                ImageRepresentation::Vph,
                ImageRepresentation::YCbCr601,
            ];
            for variant in variants {
                assert_eq!(
                    ImageRepresentation::from_str(variant.to_str()).unwrap(),
                    variant
                );
            }
        }

        #[test]
        fn expected_band_count() {
            assert_eq!(ImageRepresentation::Mono.expected_band_count(), Some(1));
            assert_eq!(ImageRepresentation::Rgb.expected_band_count(), Some(3));
            assert_eq!(ImageRepresentation::RgbLut.expected_band_count(), Some(1));
            assert_eq!(ImageRepresentation::YCbCr601.expected_band_count(), Some(3));
            assert_eq!(ImageRepresentation::Multi.expected_band_count(), None);
        }
    }

    // InterleaveMode tests
    mod interleave_mode {
        use super::*;

        #[test]
        fn from_char_valid_values() {
            assert_eq!(InterleaveMode::from_char('B').unwrap(), InterleaveMode::B);
            assert_eq!(InterleaveMode::from_char('P').unwrap(), InterleaveMode::P);
            assert_eq!(InterleaveMode::from_char('R').unwrap(), InterleaveMode::R);
            assert_eq!(InterleaveMode::from_char('S').unwrap(), InterleaveMode::S);
        }

        #[test]
        fn from_char_invalid() {
            assert!(InterleaveMode::from_char('X').is_err());
            assert!(InterleaveMode::from_char('b').is_err());
            assert!(InterleaveMode::from_char(' ').is_err());
        }

        #[test]
        fn to_char_round_trip() {
            for variant in [
                InterleaveMode::B,
                InterleaveMode::P,
                InterleaveMode::R,
                InterleaveMode::S,
            ] {
                assert_eq!(
                    InterleaveMode::from_char(variant.to_char()).unwrap(),
                    variant
                );
            }
        }
    }

    // PixelJustification tests
    mod pixel_justification {
        use super::*;

        #[test]
        fn from_char_valid_values() {
            assert_eq!(
                PixelJustification::from_char('R').unwrap(),
                PixelJustification::Right
            );
            assert_eq!(
                PixelJustification::from_char('L').unwrap(),
                PixelJustification::Left
            );
        }

        #[test]
        fn from_char_invalid() {
            assert!(PixelJustification::from_char('X').is_err());
            assert!(PixelJustification::from_char('r').is_err()); // lowercase
            assert!(PixelJustification::from_char(' ').is_err());
        }

        #[test]
        fn to_char_round_trip() {
            let variants = [PixelJustification::Right, PixelJustification::Left];
            for variant in variants {
                let c = variant.to_char();
                assert_eq!(PixelJustification::from_char(c).unwrap(), variant);
            }
        }
    }
}
