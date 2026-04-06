//! Single entry point for all decode benchmarks.
//!
//! Aggregates JBP (NC, C3, C8, CD), TIFF (None, LZW, JPEG, Deflate, PackBits),
//! and PNG benchmarks into one Criterion executable. Feature-gated benchmarks are
//! compiled only when their respective features are enabled. PNG benchmarks are
//! always available (pure Rust, no feature gate).

#[path = "decode_benchmarks/mod.rs"]
mod decode_benchmarks;

use criterion::{criterion_group, criterion_main};

// JBP NC benchmarks — always available (no feature gate)
criterion_group!(
    jbp_nc_benches,
    // Migrated from nc_decode.rs
    decode_benchmarks::jbp::nc::bench_decode_block_imode_p,
    decode_benchmarks::jbp::nc::bench_bip_to_bsq,
    decode_benchmarks::jbp::nc::bench_swap_be_to_ne,
    decode_benchmarks::jbp::nc::bench_fused_bip_to_bsq_swap,
    decode_benchmarks::jbp::nc::bench_fused_parallel_vs_serial,
    decode_benchmarks::jbp::nc::bench_tiled_transpose_tile_sizes,
    // New IMODE benchmarks
    decode_benchmarks::jbp::nc::bench_nc_imode_b,
    decode_benchmarks::jbp::nc::bench_nc_imode_p,
    decode_benchmarks::jbp::nc::bench_nc_imode_r,
    decode_benchmarks::jbp::nc::bench_nc_imode_s,
    // Multi-band 16-bit IMODE benchmarks (1024×1024×15, u16)
    decode_benchmarks::jbp::nc::bench_nc_multiband_imode_b,
    decode_benchmarks::jbp::nc::bench_nc_multiband_imode_p,
    decode_benchmarks::jbp::nc::bench_nc_multiband_imode_r,
    decode_benchmarks::jbp::nc::bench_nc_multiband_imode_s,
);

// JBP C3 (JPEG DCT) — requires libjpeg-turbo
#[cfg(feature = "libjpeg-turbo")]
criterion_group!(
    jbp_c3_benches,
    decode_benchmarks::jbp::c3::bench_jbp_c3,
);

// JBP C8 (JPEG 2000) — requires openjpeg
#[cfg(feature = "openjpeg")]
criterion_group!(
    jbp_c8_benches,
    decode_benchmarks::jbp::c8::bench_jbp_c8,
);

// JBP CD (HTJ2K) — requires openjpeg
#[cfg(feature = "openjpeg")]
criterion_group!(
    jbp_cd_benches,
    decode_benchmarks::jbp::cd::bench_jbp_cd,
);

// TIFF benchmarks — requires libtiff
#[cfg(feature = "libtiff")]
criterion_group!(
    tiff_benches,
    decode_benchmarks::tiff::none::bench_tiff_none,
    decode_benchmarks::tiff::lzw::bench_tiff_lzw,
    decode_benchmarks::tiff::jpeg::bench_tiff_jpeg,
    decode_benchmarks::tiff::deflate::bench_tiff_deflate,
    decode_benchmarks::tiff::packbits::bench_tiff_packbits,
);

// PNG benchmarks — always available (pure Rust, no feature gate)
criterion_group!(
    png_benches,
    decode_benchmarks::png::decode::bench_png_decode,
);

// Combine all groups. The cfg attributes ensure only enabled groups are included.
// PNG benchmarks are always included (no feature gate).
#[cfg(all(feature = "libjpeg-turbo", feature = "openjpeg", feature = "libtiff"))]
criterion_main!(jbp_nc_benches, jbp_c3_benches, jbp_c8_benches, jbp_cd_benches, tiff_benches, png_benches);

#[cfg(all(feature = "libjpeg-turbo", feature = "openjpeg", not(feature = "libtiff")))]
criterion_main!(jbp_nc_benches, jbp_c3_benches, jbp_c8_benches, jbp_cd_benches, png_benches);

#[cfg(all(feature = "libjpeg-turbo", not(feature = "openjpeg"), feature = "libtiff"))]
criterion_main!(jbp_nc_benches, jbp_c3_benches, tiff_benches, png_benches);

#[cfg(all(feature = "libjpeg-turbo", not(feature = "openjpeg"), not(feature = "libtiff")))]
criterion_main!(jbp_nc_benches, jbp_c3_benches, png_benches);

#[cfg(all(not(feature = "libjpeg-turbo"), feature = "openjpeg", feature = "libtiff"))]
criterion_main!(jbp_nc_benches, jbp_c8_benches, jbp_cd_benches, tiff_benches, png_benches);

#[cfg(all(not(feature = "libjpeg-turbo"), feature = "openjpeg", not(feature = "libtiff")))]
criterion_main!(jbp_nc_benches, jbp_c8_benches, jbp_cd_benches, png_benches);

#[cfg(all(not(feature = "libjpeg-turbo"), not(feature = "openjpeg"), feature = "libtiff"))]
criterion_main!(jbp_nc_benches, tiff_benches, png_benches);

#[cfg(all(not(feature = "libjpeg-turbo"), not(feature = "openjpeg"), not(feature = "libtiff")))]
criterion_main!(jbp_nc_benches, png_benches);
