//! Core type definitions for the image IO API.
//!
//! This module contains enumerations and basic types used throughout the library.

use pyo3::prelude::*;

/// Categorizes assets within a dataset.
///
/// Assets are classified into one of four categories based on their content type.
/// This enumeration follows STAC (SpatioTemporal Asset Catalog) patterns.
#[pyclass(eq, eq_int, hash, frozen, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AssetType {
    /// Raster image data (e.g., satellite imagery, aerial photos)
    Image,
    /// Text content (e.g., embedded text segments)
    Text,
    /// Vector graphics and annotations
    Graphics,
    /// Structured data (e.g., XML, JSON metadata)
    Data,
}


/// Supported pixel data types for image assets.
///
/// This enumeration represents the various numeric types that can be used
/// to store pixel values in imagery data.
#[pyclass(eq, eq_int, frozen, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelType {
    /// Unsigned 8-bit integer (0-255)
    UInt8,
    /// Unsigned 16-bit integer (0-65535)
    UInt16,
    /// Unsigned 32-bit integer
    UInt32,
    /// Signed 8-bit integer (-128 to 127)
    Int8,
    /// Signed 16-bit integer
    Int16,
    /// Signed 32-bit integer
    Int32,
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
}

#[pymethods]
impl PixelType {
    /// Convert to numpy dtype string.
    ///
    /// Returns the string representation used by numpy to identify this data type.
    pub fn to_numpy_dtype(&self) -> &'static str {
        match self {
            PixelType::UInt8 => "uint8",
            PixelType::UInt16 => "uint16",
            PixelType::UInt32 => "uint32",
            PixelType::Int8 => "int8",
            PixelType::Int16 => "int16",
            PixelType::Int32 => "int32",
            PixelType::Float32 => "float32",
            PixelType::Float64 => "float64",
        }
    }

    /// Returns the number of bytes required to store a single pixel of this type.
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelType::UInt8 | PixelType::Int8 => 1,
            PixelType::UInt16 | PixelType::Int16 => 2,
            PixelType::UInt32 | PixelType::Int32 | PixelType::Float32 => 4,
            PixelType::Float64 => 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // AssetType equality tests
    #[test]
    fn asset_type_equality_same_variants() {
        assert_eq!(AssetType::Image, AssetType::Image);
        assert_eq!(AssetType::Text, AssetType::Text);
        assert_eq!(AssetType::Graphics, AssetType::Graphics);
        assert_eq!(AssetType::Data, AssetType::Data);
    }

    #[test]
    fn asset_type_inequality_different_variants() {
        assert_ne!(AssetType::Image, AssetType::Text);
        assert_ne!(AssetType::Image, AssetType::Graphics);
        assert_ne!(AssetType::Image, AssetType::Data);
        assert_ne!(AssetType::Text, AssetType::Graphics);
        assert_ne!(AssetType::Text, AssetType::Data);
        assert_ne!(AssetType::Graphics, AssetType::Data);
    }

    // PixelType::to_numpy_dtype() tests
    #[test]
    fn pixel_type_to_numpy_dtype_unsigned_integers() {
        assert_eq!(PixelType::UInt8.to_numpy_dtype(), "uint8");
        assert_eq!(PixelType::UInt16.to_numpy_dtype(), "uint16");
        assert_eq!(PixelType::UInt32.to_numpy_dtype(), "uint32");
    }

    #[test]
    fn pixel_type_to_numpy_dtype_signed_integers() {
        assert_eq!(PixelType::Int8.to_numpy_dtype(), "int8");
        assert_eq!(PixelType::Int16.to_numpy_dtype(), "int16");
        assert_eq!(PixelType::Int32.to_numpy_dtype(), "int32");
    }

    #[test]
    fn pixel_type_to_numpy_dtype_floats() {
        assert_eq!(PixelType::Float32.to_numpy_dtype(), "float32");
        assert_eq!(PixelType::Float64.to_numpy_dtype(), "float64");
    }

    // PixelType::bytes_per_pixel() tests
    #[test]
    fn pixel_type_bytes_per_pixel_one_byte() {
        assert_eq!(PixelType::UInt8.bytes_per_pixel(), 1);
        assert_eq!(PixelType::Int8.bytes_per_pixel(), 1);
    }

    #[test]
    fn pixel_type_bytes_per_pixel_two_bytes() {
        assert_eq!(PixelType::UInt16.bytes_per_pixel(), 2);
        assert_eq!(PixelType::Int16.bytes_per_pixel(), 2);
    }

    #[test]
    fn pixel_type_bytes_per_pixel_four_bytes() {
        assert_eq!(PixelType::UInt32.bytes_per_pixel(), 4);
        assert_eq!(PixelType::Int32.bytes_per_pixel(), 4);
        assert_eq!(PixelType::Float32.bytes_per_pixel(), 4);
    }

    #[test]
    fn pixel_type_bytes_per_pixel_eight_bytes() {
        assert_eq!(PixelType::Float64.bytes_per_pixel(), 8);
    }
}
