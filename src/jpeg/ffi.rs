//! Safe Rust wrappers for libjpeg-turbo FFI.
//!
//! This module provides safe abstractions over the raw libjpeg-turbo FFI bindings,
//! handling memory management, error handling, and type conversions.
//!
//! These wrappers are used internally by the `JpegBlockDecoder` and `JpegBlockEncoder`
//! implementations.

use std::cell::RefCell;
use std::ffi::CStr;
use std::ptr;

use crate::error::CodecError;

use super::sys::{self, tjhandle, TJPF_GRAY, TJPF_RGB, TJSAMP_420, TJSAMP_444, TJSAMP_GRAY};

// =============================================================================
// Thread-Local Error Storage
// =============================================================================

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Store an error message in thread-local storage.
fn store_error(msg: String) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(msg);
    });
}

/// Get and clear the last error message.
pub(super) fn take_last_error() -> Option<String> {
    LAST_ERROR.with(|e| e.borrow_mut().take())
}

/// Get the error message from a TurboJPEG handle.
fn get_tj_error(handle: tjhandle) -> String {
    unsafe {
        let err_ptr = sys::tjGetErrorStr2(handle);
        if err_ptr.is_null() {
            "Unknown TurboJPEG error".to_string()
        } else {
            CStr::from_ptr(err_ptr)
                .to_str()
                .unwrap_or("Invalid error message")
                .to_string()
        }
    }
}

// =============================================================================
// TurboJPEG Handle Wrapper
// =============================================================================

/// Safe wrapper for TurboJPEG compressor handle.
pub struct TjCompressor {
    handle: tjhandle,
}

impl TjCompressor {
    /// Create a new TurboJPEG compressor.
    pub fn new() -> Result<Self, CodecError> {
        let handle = unsafe { sys::tjInitCompress() };
        if handle.is_null() {
            return Err(CodecError::Encode(
                "Failed to initialize TurboJPEG compressor".into(),
            ));
        }
        Ok(Self { handle })
    }

    /// Compress an image to JPEG format.
    ///
    /// # Arguments
    /// * `src` - Source pixel data
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `num_bands` - Number of color bands (1 for grayscale, 3 for RGB)
    /// * `quality` - JPEG quality (1-100)
    ///
    /// # Returns
    /// The compressed JPEG data.
    pub fn compress(
        &self,
        src: &[u8],
        width: usize,
        height: usize,
        num_bands: usize,
        quality: u8,
    ) -> Result<Vec<u8>, CodecError> {
        let pixel_format = match num_bands {
            1 => TJPF_GRAY,
            3 => TJPF_RGB,
            _ => {
                return Err(CodecError::Encode(format!(
                    "Unsupported number of bands for JPEG: {}",
                    num_bands
                )));
            }
        };

        let subsamp = match num_bands {
            1 => TJSAMP_GRAY,
            3 => TJSAMP_420, // Use 4:2:0 subsampling for RGB (common for JPEG)
            _ => TJSAMP_444,
        };

        let expected_size = width * height * num_bands;
        if src.len() != expected_size {
            return Err(CodecError::Encode(format!(
                "Source buffer size {} doesn't match expected size {}",
                src.len(),
                expected_size
            )));
        }

        let mut jpeg_buf: *mut u8 = ptr::null_mut();
        let mut jpeg_size: std::os::raw::c_ulong = 0;

        let result = unsafe {
            sys::tjCompress2(
                self.handle,
                src.as_ptr(),
                width as i32,
                0, // pitch (0 = width * pixel_size)
                height as i32,
                pixel_format,
                &mut jpeg_buf,
                &mut jpeg_size,
                subsamp,
                quality as i32,
                0, // flags
            )
        };

        if result != 0 {
            let err = get_tj_error(self.handle);
            store_error(err.clone());
            if !jpeg_buf.is_null() {
                unsafe { sys::tjFree(jpeg_buf) };
            }
            return Err(CodecError::Encode(format!(
                "JPEG compression failed: {}",
                err
            )));
        }

        // Copy the data to a Vec and free the TurboJPEG buffer
        let jpeg_data = unsafe {
            let slice = std::slice::from_raw_parts(jpeg_buf, jpeg_size as usize);
            let vec = slice.to_vec();
            sys::tjFree(jpeg_buf);
            vec
        };

        Ok(jpeg_data)
    }
}

impl Drop for TjCompressor {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                sys::tjDestroy(self.handle);
            }
        }
    }
}

// Safety: TurboJPEG handles are thread-safe
unsafe impl Send for TjCompressor {}

// =============================================================================
// JPEG Stream Helpers
// =============================================================================

/// Skip leading 0xFF fill bytes to find the JPEG SOI marker (FF D8).
///
/// NITF files may pad JPEG data with 0xFF bytes before the actual stream
/// (e.g., for word-alignment). This function returns a slice starting at SOI.
fn skip_to_soi(data: &[u8]) -> Result<&[u8], CodecError> {
    // Fast path: already starts with SOI
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        return Ok(data);
    }

    // Skip consecutive 0xFF fill bytes
    let mut offset = 0;
    while offset < data.len() && data[offset] == 0xFF {
        if offset + 1 < data.len() && data[offset + 1] == 0xD8 {
            return Ok(&data[offset..]);
        }
        offset += 1;
    }

    Err(CodecError::Decode(format!(
        "Not a JPEG file: SOI marker (FF D8) not found in first {} bytes",
        offset.min(32)
    )))
}

/// Safe wrapper for TurboJPEG decompressor handle.
pub struct TjDecompressor {
    handle: tjhandle,
}

impl TjDecompressor {
    /// Create a new TurboJPEG decompressor.
    pub fn new() -> Result<Self, CodecError> {
        let handle = unsafe { sys::tjInitDecompress() };
        if handle.is_null() {
            return Err(CodecError::Decode(
                "Failed to initialize TurboJPEG decompressor".into(),
            ));
        }
        Ok(Self { handle })
    }

    /// Get information about a JPEG image without decompressing it.
    ///
    /// # Arguments
    /// * `jpeg_data` - The JPEG compressed data
    ///
    /// # Returns
    /// Tuple of (width, height, subsampling, colorspace).
    pub fn get_header(&self, jpeg_data: &[u8]) -> Result<(usize, usize, i32, i32), CodecError> {
        let mut width: i32 = 0;
        let mut height: i32 = 0;
        let mut subsamp: i32 = 0;
        let mut colorspace: i32 = 0;

        let result = unsafe {
            sys::tjDecompressHeader3(
                self.handle,
                jpeg_data.as_ptr(),
                jpeg_data.len() as std::os::raw::c_ulong,
                &mut width,
                &mut height,
                &mut subsamp,
                &mut colorspace,
            )
        };

        if result != 0 {
            let err = get_tj_error(self.handle);
            store_error(err.clone());
            return Err(CodecError::Decode(format!(
                "Failed to read JPEG header: {}",
                err
            )));
        }

        Ok((width as usize, height as usize, subsamp, colorspace))
    }

    /// Decompress a JPEG image.
    ///
    /// # Arguments
    /// * `jpeg_data` - The JPEG compressed data
    /// * `num_bands` - Expected number of output bands (1 for grayscale, 3 for RGB)
    ///
    /// # Returns
    /// The decompressed pixel data.
    pub fn decompress(&self, jpeg_data: &[u8], num_bands: usize) -> Result<Vec<u8>, CodecError> {
        // Get image dimensions
        let (width, height, subsamp, _colorspace) = self.get_header(jpeg_data)?;

        let pixel_format = match num_bands {
            1 => TJPF_GRAY,
            3 => TJPF_RGB,
            _ => {
                return Err(CodecError::Decode(format!(
                    "Unsupported number of bands for JPEG: {}",
                    num_bands
                )));
            }
        };

        // Handle grayscale JPEG with RGB output request
        let actual_bands = if subsamp == TJSAMP_GRAY && num_bands == 3 {
            // Grayscale JPEG, but RGB output requested - decompress as grayscale
            1
        } else {
            num_bands
        };

        let actual_pixel_format = if actual_bands == 1 {
            TJPF_GRAY
        } else {
            pixel_format
        };

        let output_size = width * height * actual_bands;
        let mut output = vec![0u8; output_size];

        let result = unsafe {
            sys::tjDecompress2(
                self.handle,
                jpeg_data.as_ptr(),
                jpeg_data.len() as std::os::raw::c_ulong,
                output.as_mut_ptr(),
                width as i32,
                0, // pitch
                height as i32,
                actual_pixel_format,
                0, // flags
            )
        };

        if result != 0 {
            let err = get_tj_error(self.handle);
            store_error(err.clone());
            return Err(CodecError::Decode(format!(
                "JPEG decompression failed: {}",
                err
            )));
        }

        // If we decompressed grayscale but RGB was requested, expand to RGB
        if actual_bands == 1 && num_bands == 3 {
            let mut rgb_output = Vec::with_capacity(width * height * 3);
            for &gray in &output {
                rgb_output.push(gray);
                rgb_output.push(gray);
                rgb_output.push(gray);
            }
            return Ok(rgb_output);
        }

        Ok(output)
    }
}

impl Drop for TjDecompressor {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                sys::tjDestroy(self.handle);
            }
        }
    }
}

// Safety: TurboJPEG handles are thread-safe
unsafe impl Send for TjDecompressor {}

// =============================================================================
// Public API Functions
// =============================================================================

/// Compress 8-bit image data to JPEG format.
///
/// # Arguments
/// * `src` - Source pixel data (row-major, band-interleaved for RGB)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `num_bands` - Number of bands (1 for grayscale, 3 for RGB)
/// * `quality` - JPEG quality (1-100)
///
/// # Returns
/// The compressed JPEG data.
pub fn compress_8bit(
    src: &[u8],
    width: usize,
    height: usize,
    num_bands: usize,
    quality: u8,
) -> Result<Vec<u8>, CodecError> {
    let compressor = TjCompressor::new()?;
    compressor.compress(src, width, height, num_bands, quality)
}

/// Decompress 8-bit JPEG data.
///
/// # Arguments
/// * `jpeg_data` - The JPEG compressed data
/// * `expected_width` - Expected image width (NPPBH from NITF image subheader)
/// * `expected_height` - Expected image height (NPPBV from NITF image subheader)
/// * `num_bands` - Expected number of output bands
///
/// # Returns
/// The decompressed pixel data at `expected_width × expected_height` dimensions.
/// If the JPEG stream is smaller than expected (partial edge block), the output
/// is zero-padded to the expected dimensions.
///
/// # Edge block handling
///
/// JBP-2024.1 Section 5.12.1.8.3 (requirements JBP-2021.2-063 and JBP-2021.2-064)
/// requires that writers pad edge blocks to full NPPBH×NPPBV dimensions before
/// encoding. A conformant C3/M3 JPEG stream should therefore always have dimensions
/// equal to the declared block size.
///
/// However, some encoders write the JPEG stream at the actual edge dimensions
/// without padding — producing, for example, a 360×360 stream for a 512×512
/// block. This is technically non-conformant per JBP requirements 063/064, but
/// common enough in real-world files that a robust decoder must handle it.
///
/// JBP Section 5.12.3 also notes that pad pixel masks are "of limited use with
/// lossy compressed STI" since lossy compression does not preserve pad pixel values,
/// which may explain why some encoders skip padding for JPEG blocks.
///
/// When the JPEG stream is smaller than expected, we decompress at the stream's
/// native dimensions and zero-pad the result to the full block size. The caller
/// (`JpegNitfBlockDecoder::decode_block`) then crops to actual image dimensions.
pub fn decompress_8bit(
    jpeg_data: &[u8],
    expected_width: usize,
    expected_height: usize,
    num_bands: usize,
) -> Result<Vec<u8>, CodecError> {
    // Skip leading 0xFF fill bytes to find the SOI marker (FF D8).
    // NITF files may pad JPEG streams with FF bytes for alignment.
    let jpeg_data = skip_to_soi(jpeg_data)?;

    let decompressor = TjDecompressor::new()?;

    let (width, height, _subsamp, _colorspace) = decompressor.get_header(jpeg_data)?;

    // JPEG stream must not exceed expected block dimensions — a larger stream
    // indicates a genuine format error, not an edge block.
    if width > expected_width || height > expected_height {
        return Err(CodecError::Decode(format!(
            "JPEG dimensions {}x{} exceed expected block size {}x{}",
            width, height, expected_width, expected_height
        )));
    }

    let decoded = decompressor.decompress(jpeg_data, num_bands)?;

    // Conformant case: JPEG dimensions match the block size exactly.
    if width == expected_width && height == expected_height {
        return Ok(decoded);
    }

    // Non-conformant edge block: JPEG stream is smaller than the declared block
    // size (see doc comment above). Zero-pad each band to the expected dimensions
    // so callers receive data at the full block size.
    let expected_band_size = expected_width * expected_height;
    let jpeg_band_size = width * height;
    let mut padded = vec![0u8; num_bands * expected_band_size];

    for band in 0..num_bands {
        let src_offset = band * jpeg_band_size;
        let dst_offset = band * expected_band_size;
        for row in 0..height {
            let src_start = src_offset + row * width;
            let dst_start = dst_offset + row * expected_width;
            padded[dst_start..dst_start + width]
                .copy_from_slice(&decoded[src_start..src_start + width]);
        }
    }

    Ok(padded)
}

/// Compress 12-bit image data to JPEG format.
///
/// Note: 12-bit JPEG requires the libjpeg API (not TurboJPEG).
/// This is a placeholder that will be implemented when 12-bit support is needed.
///
/// # Arguments
/// * `src` - Source pixel data (u16 values packed as bytes, little-endian)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `quality` - JPEG quality (1-100)
///
/// # Returns
/// The compressed JPEG data.
pub fn compress_12bit(
    _src: &[u8],
    _width: usize,
    _height: usize,
    _quality: u8,
) -> Result<Vec<u8>, CodecError> {
    // 12-bit JPEG compression requires the libjpeg API with 12-bit precision.
    // This is more complex and requires a specially compiled libjpeg library.
    // For now, return an error indicating this is not yet implemented.
    Err(CodecError::Unsupported(
        "12-bit JPEG compression is not yet implemented".into(),
    ))
}

/// Decompress 12-bit JPEG data.
///
/// 12-bit JPEG requires a specially compiled libjpeg library with 12-bit sample
/// precision (libjpeg12). The standard libjpeg/libjpeg-turbo libraries are
/// compiled for 8-bit precision and cannot decode 12-bit JPEG streams.
///
/// # Arguments
/// * `jpeg_data` - The JPEG compressed data
/// * `expected_width` - Expected image width (for validation)
/// * `expected_height` - Expected image height (for validation)
///
/// # Returns
/// The decompressed pixel data (u16 values packed as bytes, little-endian).
///
/// # Requirements
/// - 1.3: Decode 12-bit monochrome JPEG blocks
///
/// # Note
/// This function currently returns an error because 12-bit JPEG support requires
/// linking against a specially compiled libjpeg12 library, which is not commonly
/// available. To enable 12-bit support:
/// 1. Compile libjpeg with `-DBITS_IN_JSAMPLE=12`
/// 2. Link against the resulting library (typically named libjpeg12)
/// 3. Enable the `libjpeg-turbo-12bit` feature flag
pub fn decompress_12bit(
    _jpeg_data: &[u8],
    _expected_width: usize,
    _expected_height: usize,
) -> Result<Vec<u8>, CodecError> {
    // 12-bit JPEG decompression requires a specially compiled libjpeg library
    // with 12-bit sample precision. The standard libjpeg/libjpeg-turbo libraries
    // are compiled for 8-bit precision.
    //
    // To support 12-bit JPEG:
    // 1. A separate libjpeg12 library must be compiled with BITS_IN_JSAMPLE=12
    // 2. The library must be linked separately (it has different symbol names)
    // 3. The jpeg12_read_scanlines function must be used instead of jpeg_read_scanlines
    //
    // For now, return an error indicating this limitation.
    Err(CodecError::Unsupported(
        "12-bit JPEG decompression requires a specially compiled libjpeg12 library. \
         Standard libjpeg/libjpeg-turbo only supports 8-bit JPEG."
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_creation() {
        let result = TjCompressor::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_decompressor_creation() {
        let result = TjDecompressor::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_grayscale_roundtrip() {
        // Create a simple 8x8 grayscale image
        let width = 8;
        let height = 8;
        let mut src = vec![0u8; width * height];
        for i in 0..src.len() {
            src[i] = (i * 4) as u8;
        }

        // Compress
        let jpeg_data = compress_8bit(&src, width, height, 1, 90).unwrap();
        assert!(!jpeg_data.is_empty());

        // Decompress
        let decoded = decompress_8bit(&jpeg_data, width, height, 1).unwrap();
        assert_eq!(decoded.len(), src.len());

        // JPEG is lossy, so we can't expect exact match, but values should be close
        for (orig, dec) in src.iter().zip(decoded.iter()) {
            assert!(
                (*orig as i32 - *dec as i32).abs() < 20,
                "Pixel difference too large: {} vs {}",
                orig,
                dec
            );
        }
    }

    #[test]
    fn test_rgb_roundtrip() {
        // Create a simple 8x8 RGB image
        let width = 8;
        let height = 8;
        let mut src = vec![0u8; width * height * 3];
        for i in 0..width * height {
            src[i * 3] = (i * 4) as u8; // R
            src[i * 3 + 1] = (i * 2) as u8; // G
            src[i * 3 + 2] = (i * 3) as u8; // B
        }

        // Compress
        let jpeg_data = compress_8bit(&src, width, height, 3, 90).unwrap();
        assert!(!jpeg_data.is_empty());

        // Decompress
        let decoded = decompress_8bit(&jpeg_data, width, height, 3).unwrap();
        assert_eq!(decoded.len(), src.len());
    }

    #[test]
    fn test_invalid_bands() {
        let src = vec![0u8; 64];
        let result = compress_8bit(&src, 8, 8, 4, 90);
        assert!(result.is_err());
    }

    #[test]
    fn test_12bit_compress_not_implemented() {
        let src = vec![0u8; 128]; // 8x8 * 2 bytes per pixel
        let result = compress_12bit(&src, 8, 8, 90);
        assert!(result.is_err());
    }

    #[test]
    fn test_12bit_decompress_not_implemented() {
        // 12-bit JPEG requires a specially compiled libjpeg12 library
        let result = decompress_12bit(&[], 8, 8);
        assert!(result.is_err());

        // Verify the error message mentions the library requirement
        if let Err(CodecError::Unsupported(msg)) = result {
            assert!(msg.contains("libjpeg12") || msg.contains("12-bit"));
        }
    }

    #[test]
    fn test_partial_block_grayscale_pads_to_expected() {
        // Reproduce BUG_JPEG_DIMENSION_MISMATCH: a JPEG stream encoded at smaller
        // dimensions than the declared NITF block size (edge block scenario).
        let jpeg_w = 6;
        let jpeg_h = 6;
        let block_w = 8;
        let block_h = 8;

        let mut src = vec![0u8; jpeg_w * jpeg_h];
        for (i, px) in src.iter_mut().enumerate() {
            *px = (i * 7 % 256) as u8;
        }

        // Compress at the JPEG's native (smaller) dimensions
        let jpeg_data = compress_8bit(&src, jpeg_w, jpeg_h, 1, 95).unwrap();

        // Decompress expecting the larger block dimensions — this used to fail
        let decoded = decompress_8bit(&jpeg_data, block_w, block_h, 1).unwrap();
        assert_eq!(decoded.len(), block_w * block_h);

        // The top-left jpeg_w×jpeg_h region should contain the image data
        for row in 0..jpeg_h {
            for col in 0..jpeg_w {
                let dec = decoded[row * block_w + col] as i32;
                let orig = src[row * jpeg_w + col] as i32;
                assert!(
                    (dec - orig).abs() < 20,
                    "Pixel ({},{}) differs too much: {} vs {}",
                    row,
                    col,
                    dec,
                    orig
                );
            }
        }

        // The padding region should be zero
        for row in 0..block_h {
            for col in 0..block_w {
                if row >= jpeg_h || col >= jpeg_w {
                    assert_eq!(
                        decoded[row * block_w + col],
                        0,
                        "Padding pixel ({},{}) should be zero",
                        row,
                        col
                    );
                }
            }
        }
    }

    #[test]
    fn test_partial_block_rgb_pads_to_expected() {
        let jpeg_w = 6;
        let jpeg_h = 6;
        let block_w = 8;
        let block_h = 8;
        let num_bands = 3;

        let mut src = vec![128u8; jpeg_w * jpeg_h * num_bands];
        for (i, px) in src.iter_mut().enumerate() {
            *px = (i * 5 % 256) as u8;
        }

        let jpeg_data = compress_8bit(&src, jpeg_w, jpeg_h, num_bands, 95).unwrap();
        let decoded = decompress_8bit(&jpeg_data, block_w, block_h, num_bands).unwrap();
        assert_eq!(decoded.len(), block_w * block_h * num_bands);
    }

    #[test]
    fn test_jpeg_larger_than_expected_is_rejected() {
        // A JPEG stream larger than the expected block should still be an error
        let jpeg_w = 16;
        let jpeg_h = 16;
        let block_w = 8;
        let block_h = 8;

        let src = vec![128u8; jpeg_w * jpeg_h];
        let jpeg_data = compress_8bit(&src, jpeg_w, jpeg_h, 1, 90).unwrap();

        let result = decompress_8bit(&jpeg_data, block_w, block_h, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_skip_to_soi_no_padding() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        let result = skip_to_soi(&data).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], 0xFF);
        assert_eq!(result[1], 0xD8);
    }

    #[test]
    fn test_skip_to_soi_with_padding() {
        // 4 FF padding bytes before SOI
        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xD8, 0xFF, 0xE0];
        let result = skip_to_soi(&data).unwrap();
        assert_eq!(result.len(), 4); // FF D8 FF E0
        assert_eq!(result[0], 0xFF);
        assert_eq!(result[1], 0xD8);
    }

    #[test]
    fn test_skip_to_soi_no_soi_found() {
        let data = [0xFF, 0xFF, 0xFF, 0x00];
        let result = skip_to_soi(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_with_ff_padding() {
        // Simulate NITF-style FF padding before a valid JPEG stream
        let width = 8;
        let height = 8;
        let src = vec![128u8; width * height];
        let jpeg_data = compress_8bit(&src, width, height, 1, 90).unwrap();

        // Prepend FF padding (like NITF alignment)
        let mut padded = vec![0xFF; 6];
        padded.extend_from_slice(&jpeg_data);

        let decoded = decompress_8bit(&padded, width, height, 1).unwrap();
        assert_eq!(decoded.len(), width * height);
    }
}
