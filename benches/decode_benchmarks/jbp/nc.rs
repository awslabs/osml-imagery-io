//! Criterion benchmarks for the uncompressed (IC=NC) NITF decode pipeline.
//!
//! Contains two groups:
//! - `"nc_decode"`: migrated from `benches/nc_decode.rs` (15-band, 16-bit, 1024×1024 IMODE=P)
//! - `"jbp_nc"`: new IMODE B/P/R/S benchmarks (3-band, 8-bit, 2048×2048)

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput};

use _io::jbp::image::decoder::BlockDecoder;
use _io::jbp::image::interleave::{
    fused_bip_to_bsq_swap, fused_bip_to_bsq_swap_parallel, to_band_sequential,
};
use _io::jbp::image::nc_decoder::UncompressedBlockDecoder;
use _io::jbp::image::swap_be_to_ne;
use _io::jbp::image::types::{InterleaveMode, PixelJustification, PixelValueType};

use super::super::common;

// ===========================================================================
// Migrated constants (15-band, 16-bit, 1024×1024 IMODE=P)
// ===========================================================================
const NROWS: u32 = 1024;
const NCOLS: u32 = 1024;
const NBANDS: u32 = 15;
const NBPP: u8 = 16;
const BPP: usize = 2; // 16-bit = 2 bytes per pixel
const DATA_SIZE: usize = (NROWS as usize) * (NCOLS as usize) * (NBANDS as usize) * BPP;

/// Generate synthetic BIP image data (deterministic, not random).
///
/// Fills with a simple pattern so endian-swap has non-trivial work.
fn generate_bip_data() -> Vec<u8> {
    let mut data = vec![0u8; DATA_SIZE];
    for (i, chunk) in data.chunks_exact_mut(2).enumerate() {
        let val = (i as u16).wrapping_mul(0x0101).wrapping_add(0x0A0B);
        chunk.copy_from_slice(&val.to_be_bytes());
    }
    data
}

/// Build an `UncompressedBlockDecoder` for a single-block 1024×1024×15 IMODE=P image.
fn create_imode_p_decoder(data: Arc<[u8]>) -> UncompressedBlockDecoder {
    UncompressedBlockDecoder::from_raw_params(
        data,
        NROWS,
        NCOLS,
        1,
        1,
        NCOLS,
        NROWS,
        NBANDS,
        NBPP,
        NBPP,
        PixelValueType::UnsignedInt,
        PixelJustification::Right,
        InterleaveMode::P,
        "NC".to_string(),
    )
}

// ===========================================================================
// Migrated benchmarks (group: "nc_decode")
// ===========================================================================

pub fn bench_decode_block_imode_p(c: &mut Criterion) {
    let data = generate_bip_data();
    let arc_data: Arc<[u8]> = Arc::from(data);
    let decoder = create_imode_p_decoder(arc_data);

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(
        BenchmarkId::new("decode_block_imode_p", "1024x1024x15_u16"),
        |b| {
            b.iter(|| {
                decoder
                    .decode_block(0, 0, 0, None)
                    .expect("decode_block failed")
            });
        },
    );

    group.finish();
}

pub fn bench_bip_to_bsq(c: &mut Criterion) {
    let data = generate_bip_data();

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("bip_to_bsq", "1024x1024x15_u16"), |b| {
        b.iter(|| {
            to_band_sequential(&data, InterleaveMode::P, NROWS, NCOLS, NBANDS, BPP)
                .expect("to_band_sequential failed")
        });
    });

    group.finish();
}

pub fn bench_swap_be_to_ne(c: &mut Criterion) {
    let bip_data = generate_bip_data();
    let bsq_data = to_band_sequential(&bip_data, InterleaveMode::P, NROWS, NCOLS, NBANDS, BPP)
        .expect("to_band_sequential failed");

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("swap_be_to_ne", "1024x1024x15_u16"), |b| {
        b.iter(|| swap_be_to_ne(&bsq_data, BPP));
    });

    group.finish();
}

pub fn bench_fused_bip_to_bsq_swap(c: &mut Criterion) {
    let bip_data = generate_bip_data();
    let mut dst = vec![0u8; DATA_SIZE];

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(
        BenchmarkId::new("fused_bip_to_bsq_swap", "1024x1024x15_u16"),
        |b| {
            b.iter(|| {
                fused_bip_to_bsq_swap(
                    &bip_data,
                    &mut dst,
                    NROWS as usize,
                    NCOLS as usize,
                    NBANDS as usize,
                    BPP,
                    None,
                    64,
                )
                .expect("fused_bip_to_bsq_swap failed")
            });
        },
    );

    group.finish();
}

pub fn bench_fused_parallel_vs_serial(c: &mut Criterion) {
    let bip_data = generate_bip_data();

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("fused_serial", "1024x1024x15_u16"), |b| {
        let mut dst = vec![0u8; DATA_SIZE];
        b.iter(|| {
            fused_bip_to_bsq_swap(
                &bip_data,
                &mut dst,
                NROWS as usize,
                NCOLS as usize,
                NBANDS as usize,
                BPP,
                None,
                64,
            )
            .expect("fused serial failed")
        });
    });

    group.bench_function(
        BenchmarkId::new("fused_parallel", "1024x1024x15_u16"),
        |b| {
            let mut dst = vec![0u8; DATA_SIZE];
            b.iter(|| {
                fused_bip_to_bsq_swap_parallel(
                    &bip_data,
                    &mut dst,
                    NROWS as usize,
                    NCOLS as usize,
                    NBANDS as usize,
                    BPP,
                    None,
                    64,
                )
                .expect("fused parallel failed")
            });
        },
    );

    group.finish();
}

pub fn bench_tiled_transpose_tile_sizes(c: &mut Criterion) {
    let bip_data = generate_bip_data();

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    for tile_rows in [16, 32, 64, 128] {
        group.bench_function(
            BenchmarkId::new("tiled_transpose", format!("tile_{tile_rows}")),
            |b| {
                let mut dst = vec![0u8; DATA_SIZE];
                b.iter(|| {
                    fused_bip_to_bsq_swap(
                        &bip_data,
                        &mut dst,
                        NROWS as usize,
                        NCOLS as usize,
                        NBANDS as usize,
                        BPP,
                        None,
                        tile_rows,
                    )
                    .expect("fused_bip_to_bsq_swap failed")
                });
            },
        );
    }

    group.finish();
}

// ===========================================================================
// New IMODE benchmarks (group: "jbp_nc", 2048×2048×3, 8-bit)
// ===========================================================================

/// Helper: build an `UncompressedBlockDecoder` for a single 2048×2048×3 block
/// with the given interleave mode and 8-bit unsigned pixels.
fn create_imode_decoder(data: Arc<[u8]>, imode: InterleaveMode) -> UncompressedBlockDecoder {
    UncompressedBlockDecoder::from_raw_params(
        data,
        common::NROWS,
        common::NCOLS,
        1,             // nbpr
        1,             // nbpc
        common::NCOLS, // nppbh
        common::NROWS, // nppbv
        common::NBANDS,
        common::NBPP,
        common::NBPP, // abpp == nbpp
        PixelValueType::UnsignedInt,
        PixelJustification::Right,
        imode,
        "NC".to_string(),
    )
}

pub fn bench_nc_imode_b(c: &mut Criterion) {
    let pixels = common::generate_synthetic_pixels(common::DATA_SIZE);
    let decoder = create_imode_decoder(Arc::from(pixels.as_slice()), InterleaveMode::B);

    let mut group = c.benchmark_group("jbp_nc");
    group.throughput(Throughput::Bytes(common::DATA_SIZE as u64));
    group.sample_size(common::SAMPLE_SIZE);

    group.bench_function(BenchmarkId::new("imode_b", "2048x2048x3_u8"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=B failed")
        });
    });

    group.finish();
}

pub fn bench_nc_imode_p(c: &mut Criterion) {
    let pixels = common::generate_synthetic_pixels(common::DATA_SIZE);
    let decoder = create_imode_decoder(Arc::from(pixels.as_slice()), InterleaveMode::P);

    let mut group = c.benchmark_group("jbp_nc");
    group.throughput(Throughput::Bytes(common::DATA_SIZE as u64));
    group.sample_size(common::SAMPLE_SIZE);

    group.bench_function(BenchmarkId::new("imode_p", "2048x2048x3_u8"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=P failed")
        });
    });

    group.finish();
}

pub fn bench_nc_imode_r(c: &mut Criterion) {
    let pixels = common::generate_synthetic_pixels(common::DATA_SIZE);
    let decoder = create_imode_decoder(Arc::from(pixels.as_slice()), InterleaveMode::R);

    let mut group = c.benchmark_group("jbp_nc");
    group.throughput(Throughput::Bytes(common::DATA_SIZE as u64));
    group.sample_size(common::SAMPLE_SIZE);

    group.bench_function(BenchmarkId::new("imode_r", "2048x2048x3_u8"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=R failed")
        });
    });

    group.finish();
}

pub fn bench_nc_imode_s(c: &mut Criterion) {
    let pixels = common::generate_synthetic_pixels(common::DATA_SIZE);
    let decoder = create_imode_decoder(Arc::from(pixels.as_slice()), InterleaveMode::S);

    let mut group = c.benchmark_group("jbp_nc");
    group.throughput(Throughput::Bytes(common::DATA_SIZE as u64));
    group.sample_size(common::SAMPLE_SIZE);

    group.bench_function(BenchmarkId::new("imode_s", "2048x2048x3_u8"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=S failed")
        });
    });

    group.finish();
}

// ===========================================================================
// Multi-band 16-bit IMODE benchmarks (group: "jbp_nc_multiband")
//
// 1024×1024×15 bands × 2 bytes = 30 MiB per block.  This stresses the
// allocation and traversal patterns that the TODO targets: separate Vec
// allocations for swap + band-select (S/B) and BIL→BSQ + swap (R).
// ===========================================================================

/// Helper: build a decoder for a single 1024×1024×15 block at the given IMODE.
fn create_multiband_decoder(data: Arc<[u8]>, imode: InterleaveMode) -> UncompressedBlockDecoder {
    UncompressedBlockDecoder::from_raw_params(
        data,
        NROWS,
        NCOLS,
        1,     // nbpr
        1,     // nbpc
        NCOLS, // nppbh
        NROWS, // nppbv
        NBANDS,
        NBPP,
        NBPP,
        PixelValueType::UnsignedInt,
        PixelJustification::Right,
        imode,
        "NC".to_string(),
    )
}

pub fn bench_nc_multiband_imode_b(c: &mut Criterion) {
    let data = generate_bip_data(); // 30 MiB, deterministic pattern
    let decoder = create_multiband_decoder(Arc::from(data.as_slice()), InterleaveMode::B);

    let mut group = c.benchmark_group("jbp_nc_multiband");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("imode_b", "1024x1024x15_u16"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=B failed")
        });
    });

    group.finish();
}

pub fn bench_nc_multiband_imode_p(c: &mut Criterion) {
    let data = generate_bip_data();
    let decoder = create_multiband_decoder(Arc::from(data.as_slice()), InterleaveMode::P);

    let mut group = c.benchmark_group("jbp_nc_multiband");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("imode_p", "1024x1024x15_u16"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=P failed")
        });
    });

    group.finish();
}

pub fn bench_nc_multiband_imode_r(c: &mut Criterion) {
    let data = generate_bip_data();
    let decoder = create_multiband_decoder(Arc::from(data.as_slice()), InterleaveMode::R);

    let mut group = c.benchmark_group("jbp_nc_multiband");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("imode_r", "1024x1024x15_u16"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=R failed")
        });
    });

    group.finish();
}

pub fn bench_nc_multiband_imode_s(c: &mut Criterion) {
    let data = generate_bip_data();
    let decoder = create_multiband_decoder(Arc::from(data.as_slice()), InterleaveMode::S);

    let mut group = c.benchmark_group("jbp_nc_multiband");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(BenchmarkId::new("imode_s", "1024x1024x15_u16"), |b| {
        b.iter(|| {
            decoder
                .decode_block(0, 0, 0, None)
                .expect("decode_block IMODE=S failed")
        });
    });

    group.finish();
}
