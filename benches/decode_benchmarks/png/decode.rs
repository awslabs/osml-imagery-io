//! Criterion benchmark for PNG decode.
//!
//! Setup writes a synthetic 2048×2048×3 UInt8 PNG, reads it back, and
//! benchmarks `get_block(0, 0, 0, None)`.

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput};
use tempfile::NamedTempFile;

use _io::png::{PNGDatasetReader, PNGDatasetWriter};
use _io::{BufferedImageAssetProvider, DatasetReader, DatasetWriter, MemoryImageConfig, PixelType};

use super::super::common;

/// Benchmark PNG single-block decode.
pub fn bench_png_decode(c: &mut Criterion) {
    // --- Setup (outside timed loop) ---

    // 1. Create synthetic image config: 2048×2048, 3-band, 8-bit, single block
    let config = MemoryImageConfig::new(common::NCOLS, common::NROWS)
        .with_bands(common::NBANDS)
        .with_block_size(common::NCOLS, common::NROWS)
        .with_pixel_type(PixelType::UInt8);

    // 2. Create provider and fill with deterministic synthetic data
    let provider = BufferedImageAssetProvider::new("image_segment_0", config);
    let pixels = common::generate_synthetic_pixels(common::DATA_SIZE);
    provider
        .set_full_image(&pixels)
        .expect("set_full_image failed");

    // 3. Write PNG
    let tmp = NamedTempFile::new().expect("failed to create temp file");
    let mut writer = PNGDatasetWriter::new(tmp.path()).expect("writer creation failed");
    writer
        .add_asset(
            "image_segment_0",
            _io::AssetProvider::Image(Arc::new(provider)),
            "Benchmark Image",
            "",
            &[],
        )
        .expect("add_asset failed");
    writer.close().expect("writer close failed");

    // 4. Read back and obtain the image asset provider
    let file_data = std::fs::read(tmp.path()).expect("failed to read PNG file");
    let reader = PNGDatasetReader::from_bytes(&file_data).expect("reader creation failed");
    let asset_keys = reader.get_asset_keys(Some(_io::AssetType::Image), None);
    let asset = reader.get_asset(&asset_keys[0]).expect("get_asset failed");
    let image_provider = asset.as_image().expect("expected Image asset variant");

    // --- Benchmark ---
    let mut group = c.benchmark_group("png_decode");
    group.throughput(Throughput::Bytes(common::DATA_SIZE as u64));
    group.sample_size(common::SAMPLE_SIZE);

    group.bench_function(BenchmarkId::new("decode_block", "2048x2048x3_u8"), |b| {
        b.iter(|| {
            image_provider
                .get_block(0, 0, 0, None)
                .expect("get_block failed")
        });
    });

    group.finish();
}
