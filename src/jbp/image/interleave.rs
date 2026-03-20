//! Interleave mode conversion for NITF image data.
//!
//! This module provides utilities for converting between different interleave modes
//! (IMODE) used in NITF imagery. The four interleave modes are:
//!
//! - **B (Band Interleaved by Block)**: All bands for each block stored sequentially
//! - **P (Band Interleaved by Pixel)**: Bands interleaved within each pixel
//! - **R (Band Interleaved by Row)**: Bands interleaved by row within each block
//! - **S (Band Sequential)**: Each band stored as a separate set of blocks
//!
//! # Data Layout Examples
//!
//! For a 2x2 image with 3 bands (R, G, B):
//!
//! ## IMODE B (Band Interleaved by Block)
//! ```text
//! [R00, R01, R10, R11, G00, G01, G10, G11, B00, B01, B10, B11]
//! ```
//!
//! ## IMODE P (Band Interleaved by Pixel)
//! ```text
//! [R00, G00, B00, R01, G01, B01, R10, G10, B10, R11, G11, B11]
//! ```
//!
//! ## IMODE R (Band Interleaved by Row)
//! ```text
//! [R00, R01, G00, G01, B00, B01, R10, R11, G10, G11, B10, B11]
//! ```
//!
//! ## IMODE S (Band Sequential) - same as B for single block
//! ```text
//! [R00, R01, R10, R11, G00, G01, G10, G11, B00, B01, B10, B11]
//! ```

use crate::error::CodecError;
use crate::jbp::image::types::InterleaveMode;

/// Convert image data from one interleave mode to another.
///
/// This function converts image data between any two interleave modes by first
/// converting to band-sequential format (if needed) and then to the target format.
///
/// # Arguments
/// * `data` - The source image data bytes
/// * `from_mode` - The source interleave mode
/// * `to_mode` - The target interleave mode
/// * `nrows` - Number of rows in the image/block
/// * `ncols` - Number of columns in the image/block
/// * `nbands` - Number of bands
/// * `bytes_per_pixel` - Number of bytes per pixel value
///
/// # Returns
/// The converted image data, or an error if conversion fails.
///
/// # Example
/// ```ignore
/// let bip_data = vec![1, 2, 3, 4, 5, 6]; // 2 pixels, 3 bands each
/// let bsq_data = convert(&bip_data, InterleaveMode::P, InterleaveMode::S, 1, 2, 3, 1)?;
/// // bsq_data = [1, 4, 2, 5, 3, 6] (band sequential)
/// ```
pub fn convert(
    data: &[u8],
    from_mode: InterleaveMode,
    to_mode: InterleaveMode,
    nrows: u32,
    ncols: u32,
    nbands: u32,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    // Optimize for same-mode case (no-op)
    if from_mode == to_mode {
        return Ok(data.to_vec());
    }

    // Convert via band-sequential as intermediate format
    let band_sequential = to_band_sequential(data, from_mode, nrows, ncols, nbands, bytes_per_pixel)?;
    from_band_sequential(&band_sequential, to_mode, nrows, ncols, nbands, bytes_per_pixel)
}

/// Convert image data to band-sequential format.
///
/// Band-sequential format stores all pixels for band 0, then all pixels for band 1, etc.
/// This is the standard format for processing and is used as an intermediate format
/// for conversions between other modes.
///
/// # Arguments
/// * `data` - The source image data bytes
/// * `from_mode` - The source interleave mode
/// * `nrows` - Number of rows in the image/block
/// * `ncols` - Number of columns in the image/block
/// * `nbands` - Number of bands
/// * `bytes_per_pixel` - Number of bytes per pixel value
///
/// # Returns
/// The data in band-sequential format, or an error if conversion fails.
pub fn to_band_sequential(
    data: &[u8],
    from_mode: InterleaveMode,
    nrows: u32,
    ncols: u32,
    nbands: u32,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    let nrows = nrows as usize;
    let ncols = ncols as usize;
    let nbands = nbands as usize;
    let pixels_per_band = nrows * ncols;
    let expected_size = pixels_per_band * nbands * bytes_per_pixel;

    if data.len() != expected_size {
        return Err(CodecError::Decode(format!(
            "Data size mismatch: expected {} bytes for {}x{}x{} image with {} bytes/pixel, got {}",
            expected_size, nrows, ncols, nbands, bytes_per_pixel, data.len()
        )));
    }

    // Handle single-band case (all modes are equivalent)
    if nbands == 1 {
        return Ok(data.to_vec());
    }

    match from_mode {
        InterleaveMode::S | InterleaveMode::B => {
            // Band-sequential and band-interleaved-by-block have the same layout
            // for a single block: all pixels of band 0, then band 1, etc.
            Ok(data.to_vec())
        }
        InterleaveMode::P => from_bip_to_bsq(data, nrows, ncols, nbands, bytes_per_pixel),
        InterleaveMode::R => from_bil_to_bsq(data, nrows, ncols, nbands, bytes_per_pixel),
    }
}

/// Convert image data from band-sequential format to the target interleave mode.
///
/// # Arguments
/// * `data` - The source image data in band-sequential format
/// * `to_mode` - The target interleave mode
/// * `nrows` - Number of rows in the image/block
/// * `ncols` - Number of columns in the image/block
/// * `nbands` - Number of bands
/// * `bytes_per_pixel` - Number of bytes per pixel value
///
/// # Returns
/// The data in the target interleave format, or an error if conversion fails.
pub fn from_band_sequential(
    data: &[u8],
    to_mode: InterleaveMode,
    nrows: u32,
    ncols: u32,
    nbands: u32,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    let nrows = nrows as usize;
    let ncols = ncols as usize;
    let nbands = nbands as usize;
    let pixels_per_band = nrows * ncols;
    let expected_size = pixels_per_band * nbands * bytes_per_pixel;

    if data.len() != expected_size {
        return Err(CodecError::Decode(format!(
            "Data size mismatch: expected {} bytes for {}x{}x{} image with {} bytes/pixel, got {}",
            expected_size, nrows, ncols, nbands, bytes_per_pixel, data.len()
        )));
    }

    // Handle single-band case (all modes are equivalent)
    if nbands == 1 {
        return Ok(data.to_vec());
    }

    match to_mode {
        InterleaveMode::S | InterleaveMode::B => {
            // Band-sequential and band-interleaved-by-block have the same layout
            Ok(data.to_vec())
        }
        InterleaveMode::P => from_bsq_to_bip(data, nrows, ncols, nbands, bytes_per_pixel),
        InterleaveMode::R => from_bsq_to_bil(data, nrows, ncols, nbands, bytes_per_pixel),
    }
}

/// Convert from Band Interleaved by Pixel (BIP) to Band Sequential (BSQ).
///
/// BIP layout: [P0B0, P0B1, P0B2, P1B0, P1B1, P1B2, ...]
/// BSQ layout: [P0B0, P1B0, ..., P0B1, P1B1, ..., P0B2, P1B2, ...]
fn from_bip_to_bsq(
    data: &[u8],
    nrows: usize,
    ncols: usize,
    nbands: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    let pixels_per_band = nrows * ncols;
    let band_size = pixels_per_band * bytes_per_pixel;
    let mut output = vec![0u8; data.len()];

    for row in 0..nrows {
        for col in 0..ncols {
            let pixel_idx = row * ncols + col;
            for band in 0..nbands {
                // Source: pixel-major order (all bands for pixel, then next pixel)
                let src_offset = (pixel_idx * nbands + band) * bytes_per_pixel;
                // Destination: band-major order (all pixels for band, then next band)
                let dst_offset = band * band_size + pixel_idx * bytes_per_pixel;

                output[dst_offset..dst_offset + bytes_per_pixel]
                    .copy_from_slice(&data[src_offset..src_offset + bytes_per_pixel]);
            }
        }
    }

    Ok(output)
}

/// Convert from Band Sequential (BSQ) to Band Interleaved by Pixel (BIP).
///
/// BSQ layout: [P0B0, P1B0, ..., P0B1, P1B1, ..., P0B2, P1B2, ...]
/// BIP layout: [P0B0, P0B1, P0B2, P1B0, P1B1, P1B2, ...]
fn from_bsq_to_bip(
    data: &[u8],
    nrows: usize,
    ncols: usize,
    nbands: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    let pixels_per_band = nrows * ncols;
    let band_size = pixels_per_band * bytes_per_pixel;
    let mut output = vec![0u8; data.len()];

    for row in 0..nrows {
        for col in 0..ncols {
            let pixel_idx = row * ncols + col;
            for band in 0..nbands {
                // Source: band-major order (all pixels for band, then next band)
                let src_offset = band * band_size + pixel_idx * bytes_per_pixel;
                // Destination: pixel-major order (all bands for pixel, then next pixel)
                let dst_offset = (pixel_idx * nbands + band) * bytes_per_pixel;

                output[dst_offset..dst_offset + bytes_per_pixel]
                    .copy_from_slice(&data[src_offset..src_offset + bytes_per_pixel]);
            }
        }
    }

    Ok(output)
}

/// Convert from Band Interleaved by Line/Row (BIL) to Band Sequential (BSQ).
///
/// BIL layout: [Row0B0, Row0B1, Row0B2, Row1B0, Row1B1, Row1B2, ...]
/// BSQ layout: [Row0B0, Row1B0, ..., Row0B1, Row1B1, ..., Row0B2, Row1B2, ...]
fn from_bil_to_bsq(
    data: &[u8],
    nrows: usize,
    ncols: usize,
    nbands: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    let row_size = ncols * bytes_per_pixel;
    let pixels_per_band = nrows * ncols;
    let band_size = pixels_per_band * bytes_per_pixel;
    let mut output = vec![0u8; data.len()];

    for row in 0..nrows {
        for band in 0..nbands {
            // Source: row-major with bands interleaved per row
            // Each row has: [row_band0, row_band1, row_band2, ...]
            let src_offset = (row * nbands + band) * row_size;
            // Destination: band-major order
            let dst_offset = band * band_size + row * row_size;

            output[dst_offset..dst_offset + row_size]
                .copy_from_slice(&data[src_offset..src_offset + row_size]);
        }
    }

    Ok(output)
}

/// Convert from Band Sequential (BSQ) to Band Interleaved by Line/Row (BIL).
///
/// BSQ layout: [Row0B0, Row1B0, ..., Row0B1, Row1B1, ..., Row0B2, Row1B2, ...]
/// BIL layout: [Row0B0, Row0B1, Row0B2, Row1B0, Row1B1, Row1B2, ...]
fn from_bsq_to_bil(
    data: &[u8],
    nrows: usize,
    ncols: usize,
    nbands: usize,
    bytes_per_pixel: usize,
) -> Result<Vec<u8>, CodecError> {
    let row_size = ncols * bytes_per_pixel;
    let pixels_per_band = nrows * ncols;
    let band_size = pixels_per_band * bytes_per_pixel;
    let mut output = vec![0u8; data.len()];

    for row in 0..nrows {
        for band in 0..nbands {
            // Source: band-major order
            let src_offset = band * band_size + row * row_size;
            // Destination: row-major with bands interleaved per row
            let dst_offset = (row * nbands + band) * row_size;

            output[dst_offset..dst_offset + row_size]
                .copy_from_slice(&data[src_offset..src_offset + row_size]);
        }
    }

    Ok(output)
}

/// Fused BIP→BSQ interleave conversion with endian swap and optional band selection.
///
/// Performs the BIP→BSQ transpose and big-endian to native-endian byte swap in a
/// single pass over the data, using a tiled access pattern for cache friendliness.
/// When `bands` is `Some(subset)`, only the selected bands are written to `dst`,
/// reducing both computation and memory usage.
///
/// # Arguments
/// * `src` - Source data in BIP (Band Interleaved by Pixel) layout, big-endian
/// * `dst` - Destination buffer for BSQ (Band Sequential) layout, native-endian.
///   Must be pre-allocated to the correct size.
/// * `nrows` - Number of rows in the image
/// * `ncols` - Number of columns in the image
/// * `nbands` - Total number of bands in the source data
/// * `bytes_per_pixel` - Bytes per pixel sample (1, 2, 4, or 8)
/// * `bands` - Optional subset of band indices to extract. `None` means all bands.
/// * `tile_rows` - Number of rows per tile for cache-friendly processing
///
/// # Errors
/// Returns `CodecError::Decode` if input/output sizes don't match expected dimensions,
/// or if any band index in `bands` is out of range.
pub fn fused_bip_to_bsq_swap(
    src: &[u8],
    dst: &mut [u8],
    nrows: usize,
    ncols: usize,
    nbands: usize,
    bytes_per_pixel: usize,
    bands: Option<&[u32]>,
    tile_rows: usize,
) -> Result<(), CodecError> {
    let pixels_per_band = nrows * ncols;
    let expected_src = pixels_per_band * nbands * bytes_per_pixel;

    if src.len() != expected_src {
        return Err(CodecError::Decode(format!(
            "Data size mismatch: expected {} bytes for {}x{}x{} image with {} bytes/pixel, got {}",
            expected_src, nrows, ncols, nbands, bytes_per_pixel, src.len()
        )));
    }

    let out_bands = match bands {
        Some(subset) => {
            for &b in subset {
                if (b as usize) >= nbands {
                    return Err(CodecError::Decode(format!(
                        "Band index {} out of range for image with {} bands",
                        b, nbands
                    )));
                }
            }
            subset.len()
        }
        None => nbands,
    };

    let expected_dst = pixels_per_band * out_bands * bytes_per_pixel;
    if dst.len() != expected_dst {
        return Err(CodecError::Decode(format!(
            "Destination size mismatch: expected {} bytes for {}x{}x{} output with {} bytes/pixel, got {}",
            expected_dst, nrows, ncols, out_bands, bytes_per_pixel, dst.len()
        )));
    }

    let tile_rows = tile_rows.max(1);
    let bip_pixel_stride = nbands * bytes_per_pixel;
    let dst_band_size = pixels_per_band * bytes_per_pixel;

    // Process in tiles of `tile_rows` rows for cache locality
    let mut tile_start = 0;
    while tile_start < nrows {
        let tile_end = (tile_start + tile_rows).min(nrows);

        for row in tile_start..tile_end {
            let row_pixel_base = row * ncols;

            for col in 0..ncols {
                let pixel_idx = row_pixel_base + col;
                let src_pixel_offset = pixel_idx * bip_pixel_stride;

                match bands {
                    Some(subset) => {
                        for (dst_band_idx, &src_band) in subset.iter().enumerate() {
                            let src_off =
                                src_pixel_offset + (src_band as usize) * bytes_per_pixel;
                            let dst_off =
                                dst_band_idx * dst_band_size + pixel_idx * bytes_per_pixel;
                            swap_copy(src, dst, src_off, dst_off, bytes_per_pixel);
                        }
                    }
                    None => {
                        for band in 0..nbands {
                            let src_off = src_pixel_offset + band * bytes_per_pixel;
                            let dst_off = band * dst_band_size + pixel_idx * bytes_per_pixel;
                            swap_copy(src, dst, src_off, dst_off, bytes_per_pixel);
                        }
                    }
                }
            }
        }

        tile_start = tile_end;
    }

    Ok(())
}

/// Rayon-parallelized variant of [`fused_bip_to_bsq_swap`].
///
/// Same semantics as the single-threaded version, but dispatches tile groups to
/// the Rayon thread pool using [`rayon::scope`]. Each thread processes a disjoint
/// range of rows, reading from the shared `src` slice and writing to non-overlapping
/// regions of `dst`.
///
/// # Safety contract (upheld via safe code)
///
/// Each spawned task writes only to the BSQ positions corresponding to its assigned
/// row range. Because BSQ layout stores pixels for a given row contiguously within
/// each band plane, and row ranges are disjoint, no two tasks write to the same byte.
/// The `src` slice is shared immutably across all tasks.
pub fn fused_bip_to_bsq_swap_parallel(
    src: &[u8],
    dst: &mut [u8],
    nrows: usize,
    ncols: usize,
    nbands: usize,
    bytes_per_pixel: usize,
    bands: Option<&[u32]>,
    tile_rows: usize,
) -> Result<(), CodecError> {
    let pixels_per_band = nrows * ncols;
    let expected_src = pixels_per_band * nbands * bytes_per_pixel;

    if src.len() != expected_src {
        return Err(CodecError::Decode(format!(
            "Data size mismatch: expected {} bytes for {}x{}x{} image with {} bytes/pixel, got {}",
            expected_src, nrows, ncols, nbands, bytes_per_pixel, src.len()
        )));
    }

    let out_bands = match bands {
        Some(subset) => {
            for &b in subset {
                if (b as usize) >= nbands {
                    return Err(CodecError::Decode(format!(
                        "Band index {} out of range for image with {} bands",
                        b, nbands
                    )));
                }
            }
            subset.len()
        }
        None => nbands,
    };

    let expected_dst = pixels_per_band * out_bands * bytes_per_pixel;
    if dst.len() != expected_dst {
        return Err(CodecError::Decode(format!(
            "Destination size mismatch: expected {} bytes for {}x{}x{} output with {} bytes/pixel, got {}",
            expected_dst, nrows, ncols, out_bands, bytes_per_pixel, dst.len()
        )));
    }

    let tile_rows = tile_rows.max(1);
    let bip_pixel_stride = nbands * bytes_per_pixel;
    let dst_band_size = pixels_per_band * bytes_per_pixel;

    // Wrapper to send a raw pointer across thread boundaries.
    // SAFETY: we guarantee disjoint access — each tile group writes to a unique set
    // of row positions within each band plane, so no two threads touch the same byte.
    struct SendPtr(*mut u8);
    unsafe impl Send for SendPtr {}
    unsafe impl Sync for SendPtr {}

    let dst_ptr = SendPtr(dst.as_mut_ptr());
    let dst_len = dst.len();

    // Build the list of tile row-ranges up front.
    let mut tile_ranges: Vec<(usize, usize)> = Vec::new();
    let mut tile_start = 0;
    while tile_start < nrows {
        let tile_end = (tile_start + tile_rows).min(nrows);
        tile_ranges.push((tile_start, tile_end));
        tile_start = tile_end;
    }

    // Dispatch tile groups to Rayon.
    rayon::scope(|s| {
        for &(tile_start, tile_end) in &tile_ranges {
            let src = src;
            let bands = bands;
            let dst_ptr = &dst_ptr;

            s.spawn(move |_| {
                // SAFETY: tile row ranges are disjoint, so each task writes to unique offsets.
                let dst_slice =
                    unsafe { std::slice::from_raw_parts_mut(dst_ptr.0, dst_len) };

                for row in tile_start..tile_end {
                    let row_pixel_base = row * ncols;

                    for col in 0..ncols {
                        let pixel_idx = row_pixel_base + col;
                        let src_pixel_offset = pixel_idx * bip_pixel_stride;

                        match bands {
                            Some(subset) => {
                                for (dst_band_idx, &src_band) in subset.iter().enumerate() {
                                    let src_off =
                                        src_pixel_offset + (src_band as usize) * bytes_per_pixel;
                                    let dst_off =
                                        dst_band_idx * dst_band_size + pixel_idx * bytes_per_pixel;
                                    swap_copy(src, dst_slice, src_off, dst_off, bytes_per_pixel);
                                }
                            }
                            None => {
                                for band in 0..nbands {
                                    let src_off = src_pixel_offset + band * bytes_per_pixel;
                                    let dst_off =
                                        band * dst_band_size + pixel_idx * bytes_per_pixel;
                                    swap_copy(src, dst_slice, src_off, dst_off, bytes_per_pixel);
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    Ok(())
}

/// Copy a single pixel sample from `src` to `dst`, swapping from big-endian to
/// native-endian. For single-byte data, this is a plain copy.
#[inline(always)]
fn swap_copy(src: &[u8], dst: &mut [u8], src_off: usize, dst_off: usize, bpp: usize) {
    if cfg!(target_endian = "big") || bpp == 1 {
        dst[dst_off..dst_off + bpp].copy_from_slice(&src[src_off..src_off + bpp]);
        return;
    }
    match bpp {
        2 => {
            let val = u16::from_be_bytes([src[src_off], src[src_off + 1]]);
            dst[dst_off..dst_off + 2].copy_from_slice(&val.to_ne_bytes());
        }
        4 => {
            let val = u32::from_be_bytes([
                src[src_off],
                src[src_off + 1],
                src[src_off + 2],
                src[src_off + 3],
            ]);
            dst[dst_off..dst_off + 4].copy_from_slice(&val.to_ne_bytes());
        }
        8 => {
            let val = u64::from_be_bytes([
                src[src_off],
                src[src_off + 1],
                src[src_off + 2],
                src[src_off + 3],
                src[src_off + 4],
                src[src_off + 5],
                src[src_off + 6],
                src[src_off + 7],
            ]);
            dst[dst_off..dst_off + 8].copy_from_slice(&val.to_ne_bytes());
        }
        _ => {
            dst[dst_off..dst_off + bpp].copy_from_slice(&src[src_off..src_off + bpp]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test data with known pattern
    fn create_test_data(nrows: usize, ncols: usize, nbands: usize, bytes_per_pixel: usize) -> Vec<u8> {
        let total_pixels = nrows * ncols * nbands;
        let total_bytes = total_pixels * bytes_per_pixel;
        (0..total_bytes).map(|i| (i % 256) as u8).collect()
    }

    mod to_band_sequential {
        use super::*;

        #[test]
        fn single_band_is_noop() {
            let data = vec![1, 2, 3, 4];
            let result = to_band_sequential(&data, InterleaveMode::P, 2, 2, 1, 1).unwrap();
            assert_eq!(result, data);
        }

        #[test]
        fn mode_s_is_noop() {
            let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let result = to_band_sequential(&data, InterleaveMode::S, 2, 2, 3, 1).unwrap();
            assert_eq!(result, data);
        }

        #[test]
        fn mode_b_is_noop() {
            let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let result = to_band_sequential(&data, InterleaveMode::B, 2, 2, 3, 1).unwrap();
            assert_eq!(result, data);
        }

        #[test]
        fn from_bip_2x2x3() {
            // BIP: [P00_B0, P00_B1, P00_B2, P01_B0, P01_B1, P01_B2, P10_B0, P10_B1, P10_B2, P11_B0, P11_B1, P11_B2]
            // Pixel order: (0,0), (0,1), (1,0), (1,1)
            let bip_data = vec![
                1, 2, 3,    // pixel (0,0): bands 0,1,2
                4, 5, 6,    // pixel (0,1): bands 0,1,2
                7, 8, 9,    // pixel (1,0): bands 0,1,2
                10, 11, 12, // pixel (1,1): bands 0,1,2
            ];
            // BSQ: [B0_all_pixels, B1_all_pixels, B2_all_pixels]
            let expected_bsq = vec![
                1, 4, 7, 10,  // band 0: all pixels
                2, 5, 8, 11,  // band 1: all pixels
                3, 6, 9, 12,  // band 2: all pixels
            ];
            let result = to_band_sequential(&bip_data, InterleaveMode::P, 2, 2, 3, 1).unwrap();
            assert_eq!(result, expected_bsq);
        }

        #[test]
        fn from_bil_2x2x3() {
            // BIL: [Row0_B0, Row0_B1, Row0_B2, Row1_B0, Row1_B1, Row1_B2]
            // Each row segment has ncols pixels
            let bil_data = vec![
                1, 2,       // row 0, band 0: pixels (0,0), (0,1)
                3, 4,       // row 0, band 1: pixels (0,0), (0,1)
                5, 6,       // row 0, band 2: pixels (0,0), (0,1)
                7, 8,       // row 1, band 0: pixels (1,0), (1,1)
                9, 10,      // row 1, band 1: pixels (1,0), (1,1)
                11, 12,     // row 1, band 2: pixels (1,0), (1,1)
            ];
            // BSQ: [B0_all_pixels, B1_all_pixels, B2_all_pixels]
            let expected_bsq = vec![
                1, 2, 7, 8,     // band 0: row0, row1
                3, 4, 9, 10,    // band 1: row0, row1
                5, 6, 11, 12,   // band 2: row0, row1
            ];
            let result = to_band_sequential(&bil_data, InterleaveMode::R, 2, 2, 3, 1).unwrap();
            assert_eq!(result, expected_bsq);
        }

        #[test]
        fn data_size_mismatch_error() {
            let data = vec![1, 2, 3]; // Too small for 2x2x3
            let result = to_band_sequential(&data, InterleaveMode::P, 2, 2, 3, 1);
            assert!(result.is_err());
        }

        #[test]
        fn multi_byte_pixels_bip() {
            // 2 pixels, 2 bands, 2 bytes per pixel
            // BIP: [P0B0_hi, P0B0_lo, P0B1_hi, P0B1_lo, P1B0_hi, P1B0_lo, P1B1_hi, P1B1_lo]
            let bip_data = vec![
                0x00, 0x01,  // pixel 0, band 0
                0x00, 0x02,  // pixel 0, band 1
                0x00, 0x03,  // pixel 1, band 0
                0x00, 0x04,  // pixel 1, band 1
            ];
            // BSQ: [B0_all_pixels, B1_all_pixels]
            let expected_bsq = vec![
                0x00, 0x01, 0x00, 0x03,  // band 0: pixel 0, pixel 1
                0x00, 0x02, 0x00, 0x04,  // band 1: pixel 0, pixel 1
            ];
            let result = to_band_sequential(&bip_data, InterleaveMode::P, 1, 2, 2, 2).unwrap();
            assert_eq!(result, expected_bsq);
        }
    }

    mod from_band_sequential {
        use super::*;

        #[test]
        fn single_band_is_noop() {
            let data = vec![1, 2, 3, 4];
            let result = from_band_sequential(&data, InterleaveMode::P, 2, 2, 1, 1).unwrap();
            assert_eq!(result, data);
        }

        #[test]
        fn to_mode_s_is_noop() {
            let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let result = from_band_sequential(&data, InterleaveMode::S, 2, 2, 3, 1).unwrap();
            assert_eq!(result, data);
        }

        #[test]
        fn to_mode_b_is_noop() {
            let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let result = from_band_sequential(&data, InterleaveMode::B, 2, 2, 3, 1).unwrap();
            assert_eq!(result, data);
        }

        #[test]
        fn to_bip_2x2x3() {
            // BSQ: [B0_all_pixels, B1_all_pixels, B2_all_pixels]
            let bsq_data = vec![
                1, 4, 7, 10,  // band 0: all pixels
                2, 5, 8, 11,  // band 1: all pixels
                3, 6, 9, 12,  // band 2: all pixels
            ];
            // BIP: [P00_B0, P00_B1, P00_B2, P01_B0, P01_B1, P01_B2, ...]
            let expected_bip = vec![
                1, 2, 3,    // pixel (0,0): bands 0,1,2
                4, 5, 6,    // pixel (0,1): bands 0,1,2
                7, 8, 9,    // pixel (1,0): bands 0,1,2
                10, 11, 12, // pixel (1,1): bands 0,1,2
            ];
            let result = from_band_sequential(&bsq_data, InterleaveMode::P, 2, 2, 3, 1).unwrap();
            assert_eq!(result, expected_bip);
        }

        #[test]
        fn to_bil_2x2x3() {
            // BSQ: [B0_all_pixels, B1_all_pixels, B2_all_pixels]
            let bsq_data = vec![
                1, 2, 7, 8,     // band 0: row0, row1
                3, 4, 9, 10,    // band 1: row0, row1
                5, 6, 11, 12,   // band 2: row0, row1
            ];
            // BIL: [Row0_B0, Row0_B1, Row0_B2, Row1_B0, Row1_B1, Row1_B2]
            let expected_bil = vec![
                1, 2,       // row 0, band 0
                3, 4,       // row 0, band 1
                5, 6,       // row 0, band 2
                7, 8,       // row 1, band 0
                9, 10,      // row 1, band 1
                11, 12,     // row 1, band 2
            ];
            let result = from_band_sequential(&bsq_data, InterleaveMode::R, 2, 2, 3, 1).unwrap();
            assert_eq!(result, expected_bil);
        }

        #[test]
        fn data_size_mismatch_error() {
            let data = vec![1, 2, 3]; // Too small for 2x2x3
            let result = from_band_sequential(&data, InterleaveMode::P, 2, 2, 3, 1);
            assert!(result.is_err());
        }
    }

    mod fused_bip_to_bsq_swap {
        use super::*;
        use crate::jbp::image::decoder::swap_be_to_ne;

        /// Reference implementation: from_bip_to_bsq then swap_be_to_ne
        fn reference_bip_to_bsq_swap(
            data: &[u8],
            nrows: usize,
            ncols: usize,
            nbands: usize,
            bpp: usize,
        ) -> Vec<u8> {
            let bsq = from_bip_to_bsq(data, nrows, ncols, nbands, bpp).unwrap();
            swap_be_to_ne(&bsq, bpp)
        }

        #[test]
        fn basic_2x2x3_1bpp() {
            // 1-byte pixels: no swap, just transpose
            let bip = vec![
                1, 2, 3, // pixel (0,0)
                4, 5, 6, // pixel (0,1)
                7, 8, 9, // pixel (1,0)
                10, 11, 12, // pixel (1,1)
            ];
            let expected = reference_bip_to_bsq_swap(&bip, 2, 2, 3, 1);
            let mut dst = vec![0u8; bip.len()];
            fused_bip_to_bsq_swap(&bip, &mut dst, 2, 2, 3, 1, None, 64).unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn basic_2x2x3_2bpp() {
            // 2-byte pixels: transpose + endian swap
            let bip: Vec<u8> = (0..24).map(|i| (i * 7 + 3) as u8).collect();
            let expected = reference_bip_to_bsq_swap(&bip, 2, 2, 3, 2);
            let mut dst = vec![0u8; bip.len()];
            fused_bip_to_bsq_swap(&bip, &mut dst, 2, 2, 3, 2, None, 64).unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn basic_4bpp() {
            let bip: Vec<u8> = (0..48).map(|i| (i * 13 + 5) as u8).collect();
            let expected = reference_bip_to_bsq_swap(&bip, 2, 2, 3, 4);
            let mut dst = vec![0u8; bip.len()];
            fused_bip_to_bsq_swap(&bip, &mut dst, 2, 2, 3, 4, None, 64).unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn basic_8bpp() {
            let bip: Vec<u8> = (0..96).map(|i| (i * 17 + 11) as u8).collect();
            let expected = reference_bip_to_bsq_swap(&bip, 2, 2, 3, 8);
            let mut dst = vec![0u8; bip.len()];
            fused_bip_to_bsq_swap(&bip, &mut dst, 2, 2, 3, 8, None, 64).unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn partial_tile_rows() {
            // 5 rows with tile_rows=2 → tiles of [2, 2, 1]
            let nrows = 5;
            let ncols = 3;
            let nbands = 2;
            let bpp = 2;
            let size = nrows * ncols * nbands * bpp;
            let bip: Vec<u8> = (0..size).map(|i| (i * 7) as u8).collect();
            let expected = reference_bip_to_bsq_swap(&bip, nrows, ncols, nbands, bpp);
            let mut dst = vec![0u8; size];
            fused_bip_to_bsq_swap(&bip, &mut dst, nrows, ncols, nbands, bpp, None, 2).unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn tile_rows_1() {
            // Degenerate tile size of 1 row
            let nrows = 4;
            let ncols = 4;
            let nbands = 3;
            let bpp = 2;
            let size = nrows * ncols * nbands * bpp;
            let bip: Vec<u8> = (0..size).map(|i| (i * 11) as u8).collect();
            let expected = reference_bip_to_bsq_swap(&bip, nrows, ncols, nbands, bpp);
            let mut dst = vec![0u8; size];
            fused_bip_to_bsq_swap(&bip, &mut dst, nrows, ncols, nbands, bpp, None, 1).unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn band_selection_subset() {
            // Select bands 0 and 2 from a 3-band image
            let nrows = 2;
            let ncols = 2;
            let nbands = 3;
            let bpp = 2;
            let src_size = nrows * ncols * nbands * bpp;
            let bip: Vec<u8> = (0..src_size).map(|i| (i * 7 + 3) as u8).collect();

            let bands = [0u32, 2];
            let out_bands = bands.len();
            let dst_size = nrows * ncols * out_bands * bpp;

            // Reference: full fused then extract selected bands
            let full = reference_bip_to_bsq_swap(&bip, nrows, ncols, nbands, bpp);
            let pixels_per_band = nrows * ncols;
            let band_size = pixels_per_band * bpp;
            let mut expected = vec![0u8; dst_size];
            for (dst_idx, &src_band) in bands.iter().enumerate() {
                let src_start = (src_band as usize) * band_size;
                let dst_start = dst_idx * band_size;
                expected[dst_start..dst_start + band_size]
                    .copy_from_slice(&full[src_start..src_start + band_size]);
            }

            let mut dst = vec![0u8; dst_size];
            fused_bip_to_bsq_swap(&bip, &mut dst, nrows, ncols, nbands, bpp, Some(&bands), 64)
                .unwrap();
            assert_eq!(dst, expected);
        }

        #[test]
        fn band_selection_none_equals_all() {
            let nrows = 3;
            let ncols = 3;
            let nbands = 4;
            let bpp = 2;
            let size = nrows * ncols * nbands * bpp;
            let bip: Vec<u8> = (0..size).map(|i| (i * 13) as u8).collect();

            let mut dst_none = vec![0u8; size];
            fused_bip_to_bsq_swap(&bip, &mut dst_none, nrows, ncols, nbands, bpp, None, 64)
                .unwrap();

            let all_bands: Vec<u32> = (0..nbands as u32).collect();
            let mut dst_all = vec![0u8; size];
            fused_bip_to_bsq_swap(
                &bip,
                &mut dst_all,
                nrows,
                ncols,
                nbands,
                bpp,
                Some(&all_bands),
                64,
            )
            .unwrap();

            assert_eq!(dst_none, dst_all);
        }

        #[test]
        fn src_size_mismatch_error() {
            let src = vec![0u8; 10]; // wrong size for 2x2x3x1
            let mut dst = vec![0u8; 12];
            let result = fused_bip_to_bsq_swap(&src, &mut dst, 2, 2, 3, 1, None, 64);
            assert!(result.is_err());
        }

        #[test]
        fn dst_size_mismatch_error() {
            let src = vec![0u8; 12]; // correct for 2x2x3x1
            let mut dst = vec![0u8; 10]; // wrong
            let result = fused_bip_to_bsq_swap(&src, &mut dst, 2, 2, 3, 1, None, 64);
            assert!(result.is_err());
        }

        #[test]
        fn band_index_out_of_range_error() {
            let src = vec![0u8; 12]; // 2x2x3x1
            let mut dst = vec![0u8; 4]; // 2x2x1x1
            let bands = [5u32]; // out of range
            let result = fused_bip_to_bsq_swap(&src, &mut dst, 2, 2, 3, 1, Some(&bands), 64);
            assert!(result.is_err());
        }
    }

    mod convert {
        use super::*;

        #[test]
        fn same_mode_is_noop() {
            let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            for mode in [InterleaveMode::B, InterleaveMode::P, InterleaveMode::R, InterleaveMode::S] {
                let result = convert(&data, mode, mode, 2, 2, 3, 1).unwrap();
                assert_eq!(result, data, "Same mode conversion should be identity for {:?}", mode);
            }
        }

        #[test]
        fn bip_to_bsq_and_back() {
            let original = vec![
                1, 2, 3,    // pixel 0
                4, 5, 6,    // pixel 1
                7, 8, 9,    // pixel 2
                10, 11, 12, // pixel 3
            ];
            let bsq = convert(&original, InterleaveMode::P, InterleaveMode::S, 2, 2, 3, 1).unwrap();
            let back = convert(&bsq, InterleaveMode::S, InterleaveMode::P, 2, 2, 3, 1).unwrap();
            assert_eq!(back, original);
        }

        #[test]
        fn bil_to_bsq_and_back() {
            let original = vec![
                1, 2,       // row 0, band 0
                3, 4,       // row 0, band 1
                5, 6,       // row 0, band 2
                7, 8,       // row 1, band 0
                9, 10,      // row 1, band 1
                11, 12,     // row 1, band 2
            ];
            let bsq = convert(&original, InterleaveMode::R, InterleaveMode::S, 2, 2, 3, 1).unwrap();
            let back = convert(&bsq, InterleaveMode::S, InterleaveMode::R, 2, 2, 3, 1).unwrap();
            assert_eq!(back, original);
        }

        #[test]
        fn bip_to_bil_and_back() {
            let original = vec![
                1, 2, 3,    // pixel (0,0)
                4, 5, 6,    // pixel (0,1)
                7, 8, 9,    // pixel (1,0)
                10, 11, 12, // pixel (1,1)
            ];
            let bil = convert(&original, InterleaveMode::P, InterleaveMode::R, 2, 2, 3, 1).unwrap();
            let back = convert(&bil, InterleaveMode::R, InterleaveMode::P, 2, 2, 3, 1).unwrap();
            assert_eq!(back, original);
        }

        #[test]
        fn larger_image_round_trip() {
            // 4x4 image with 4 bands, 2 bytes per pixel
            let data = create_test_data(4, 4, 4, 2);
            
            // Test all mode combinations
            let modes = [InterleaveMode::B, InterleaveMode::P, InterleaveMode::R, InterleaveMode::S];
            for from in &modes {
                for to in &modes {
                    let converted = convert(&data, *from, *to, 4, 4, 4, 2).unwrap();
                    let back = convert(&converted, *to, *from, 4, 4, 4, 2).unwrap();
                    assert_eq!(back, data, "Round trip failed for {:?} -> {:?}", from, to);
                }
            }
        }
    }
}


/// Property-based tests for interleave conversion
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Generate a valid InterleaveMode
    fn interleave_mode_strategy() -> impl Strategy<Value = InterleaveMode> {
        prop_oneof![
            Just(InterleaveMode::B),
            Just(InterleaveMode::P),
            Just(InterleaveMode::R),
            Just(InterleaveMode::S),
        ]
    }

    /// Generate valid image dimensions (small for testing)
    fn image_dimensions_strategy() -> impl Strategy<Value = (u32, u32, u32, usize)> {
        (
            1u32..=16,      // nrows
            1u32..=16,      // ncols
            1u32..=8,       // nbands
            prop_oneof![Just(1usize), Just(2usize), Just(4usize)], // bytes_per_pixel
        )
    }

    /// Property 9: Interleave Conversion Preserves Pixel Values
    /// For any valid image data and source/target interleave mode pair,
    /// converting from source to target and back to source SHALL produce
    /// byte-identical output.
    /// **Validates: Requirements 12.1-12.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn interleave_round_trip(
            (nrows, ncols, nbands, bytes_per_pixel) in image_dimensions_strategy(),
            from_mode in interleave_mode_strategy(),
            to_mode in interleave_mode_strategy(),
        ) {
            // Generate random image data
            let data_size = (nrows as usize) * (ncols as usize) * (nbands as usize) * bytes_per_pixel;
            let original_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

            // Convert from source mode to target mode
            let converted = convert(
                &original_data,
                from_mode,
                to_mode,
                nrows,
                ncols,
                nbands,
                bytes_per_pixel,
            ).unwrap();

            // Convert back to source mode
            let round_tripped = convert(
                &converted,
                to_mode,
                from_mode,
                nrows,
                ncols,
                nbands,
                bytes_per_pixel,
            ).unwrap();

            // Verify byte-identical output
            prop_assert_eq!(
                round_tripped, original_data,
                "Round trip {:?} -> {:?} -> {:?} should preserve data for {}x{}x{} image with {} bytes/pixel",
                from_mode, to_mode, from_mode, nrows, ncols, nbands, bytes_per_pixel
            );
        }

        #[test]
        fn to_band_sequential_preserves_data_size(
            (nrows, ncols, nbands, bytes_per_pixel) in image_dimensions_strategy(),
            from_mode in interleave_mode_strategy(),
        ) {
            let data_size = (nrows as usize) * (ncols as usize) * (nbands as usize) * bytes_per_pixel;
            let original_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

            let bsq_data = to_band_sequential(
                &original_data,
                from_mode,
                nrows,
                ncols,
                nbands,
                bytes_per_pixel,
            ).unwrap();

            prop_assert_eq!(
                bsq_data.len(), original_data.len(),
                "to_band_sequential should preserve data size"
            );
        }

        #[test]
        fn from_band_sequential_preserves_data_size(
            (nrows, ncols, nbands, bytes_per_pixel) in image_dimensions_strategy(),
            to_mode in interleave_mode_strategy(),
        ) {
            let data_size = (nrows as usize) * (ncols as usize) * (nbands as usize) * bytes_per_pixel;
            let original_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

            let converted_data = from_band_sequential(
                &original_data,
                to_mode,
                nrows,
                ncols,
                nbands,
                bytes_per_pixel,
            ).unwrap();

            prop_assert_eq!(
                converted_data.len(), original_data.len(),
                "from_band_sequential should preserve data size"
            );
        }

        #[test]
        fn same_mode_conversion_is_identity(
            (nrows, ncols, nbands, bytes_per_pixel) in image_dimensions_strategy(),
            mode in interleave_mode_strategy(),
        ) {
            let data_size = (nrows as usize) * (ncols as usize) * (nbands as usize) * bytes_per_pixel;
            let original_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

            let converted = convert(
                &original_data,
                mode,
                mode,
                nrows,
                ncols,
                nbands,
                bytes_per_pixel,
            ).unwrap();

            prop_assert_eq!(
                converted, original_data,
                "Same mode conversion should be identity for {:?}",
                mode
            );
        }

        #[test]
        fn single_band_all_modes_equivalent(
            nrows in 1u32..=16,
            ncols in 1u32..=16,
            bytes_per_pixel in prop_oneof![Just(1usize), Just(2usize), Just(4usize)],
            from_mode in interleave_mode_strategy(),
            to_mode in interleave_mode_strategy(),
        ) {
            // For single-band images, all interleave modes should be equivalent
            let nbands = 1u32;
            let data_size = (nrows as usize) * (ncols as usize) * bytes_per_pixel;
            let original_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

            let converted = convert(
                &original_data,
                from_mode,
                to_mode,
                nrows,
                ncols,
                nbands,
                bytes_per_pixel,
            ).unwrap();

            prop_assert_eq!(
                converted, original_data,
                "Single-band conversion {:?} -> {:?} should be identity",
                from_mode, to_mode
            );
        }
    }
}
