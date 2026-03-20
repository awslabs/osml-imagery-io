//! Criterion benchmarks for the uncompressed (IC=NC) NITF decode pipeline.
//!
//! Measures:
//! - `bench_decode_block_imode_p`: full `decode_block` on synthetic 15-band, 16-bit, 1024×1024 IMODE=P data
//! - `bench_bip_to_bsq`: current `from_bip_to_bsq` via `to_band_sequential` in isolation
//! - `bench_swap_be_to_ne`: current `swap_be_to_ne` in isolation

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use _io::jbp::image::decoder::{BlockDecoder, UncompressedBlockDecoder};
use _io::jbp::image::interleave::{fused_bip_to_bsq_swap, fused_bip_to_bsq_swap_parallel, to_band_sequential};
use _io::jbp::image::types::{InterleaveMode, PixelJustification, PixelValueType};
use _io::jbp::image::swap_be_to_ne;

// ---------------------------------------------------------------------------
// Constants matching the target workload: 15-band, 16-bit, 1024×1024 IMODE=P
// ---------------------------------------------------------------------------
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
    // Fill with a repeating pattern that exercises byte-swap paths.
    // Use 16-bit big-endian values: high byte != low byte.
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
        1,    // nbpr: 1 block per row
        1,    // nbpc: 1 block per column
        NCOLS,
        NROWS,
        NBANDS,
        NBPP,
        NBPP, // abpp == nbpp
        PixelValueType::UnsignedInt,
        PixelJustification::Right,
        InterleaveMode::P,
        "NC".to_string(),
    )
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_decode_block_imode_p(c: &mut Criterion) {
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

fn bench_bip_to_bsq(c: &mut Criterion) {
    let data = generate_bip_data();

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(
        BenchmarkId::new("bip_to_bsq", "1024x1024x15_u16"),
        |b| {
            b.iter(|| {
                to_band_sequential(
                    &data,
                    InterleaveMode::P,
                    NROWS,
                    NCOLS,
                    NBANDS,
                    BPP,
                )
                .expect("to_band_sequential failed")
            });
        },
    );

    group.finish();
}

fn bench_swap_be_to_ne(c: &mut Criterion) {
    // Pre-convert to BSQ so we benchmark only the swap, not the interleave.
    let bip_data = generate_bip_data();
    let bsq_data = to_band_sequential(
        &bip_data,
        InterleaveMode::P,
        NROWS,
        NCOLS,
        NBANDS,
        BPP,
    )
    .expect("to_band_sequential failed");

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(
        BenchmarkId::new("swap_be_to_ne", "1024x1024x15_u16"),
        |b| {
            b.iter(|| swap_be_to_ne(&bsq_data, BPP));
        },
    );

    group.finish();
}

fn bench_fused_bip_to_bsq_swap(c: &mut Criterion) {
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

fn bench_fused_parallel_vs_serial(c: &mut Criterion) {
    let bip_data = generate_bip_data();

    let mut group = c.benchmark_group("nc_decode");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.sample_size(20);

    group.bench_function(
        BenchmarkId::new("fused_serial", "1024x1024x15_u16"),
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
                    64,
                )
                .expect("fused serial failed")
            });
        },
    );

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

fn bench_tiled_transpose_tile_sizes(c: &mut Criterion) {
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

criterion_group!(
    benches,
    bench_decode_block_imode_p,
    bench_bip_to_bsq,
    bench_swap_be_to_ne,
    bench_fused_bip_to_bsq_swap,
    bench_fused_parallel_vs_serial,
    bench_tiled_transpose_tile_sizes,
);
criterion_main!(benches);
