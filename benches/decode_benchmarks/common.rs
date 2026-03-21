//! Shared constants and synthetic data generation helpers for all decode benchmarks.

/// Number of rows in the standard benchmark image.
pub const NROWS: u32 = 2048;
/// Number of columns in the standard benchmark image.
pub const NCOLS: u32 = 2048;
/// Number of bands in the standard benchmark image.
pub const NBANDS: u32 = 3;
/// Number of bits per pixel.
pub const NBPP: u8 = 8;
/// Total uncompressed data size in bytes: 2048 × 2048 × 3 = 12,582,912.
pub const DATA_SIZE: usize = (NROWS as usize) * (NCOLS as usize) * (NBANDS as usize);
/// Criterion sample size for all benchmark groups.
pub const SAMPLE_SIZE: usize = 20;

/// Generate deterministic, non-trivial synthetic pixel data.
///
/// Produces a repeatable byte pattern that avoids all-zero or all-constant data,
/// ensuring compression codecs and interleave kernels do real work.
pub fn generate_synthetic_pixels(size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| {
            let x = (i % 2048) as u8;
            let y = (i / 2048) as u8;
            x.wrapping_mul(7)
                .wrapping_add(y.wrapping_mul(13))
                .wrapping_add(42)
        })
        .collect()
}
