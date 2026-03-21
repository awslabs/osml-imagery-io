//! Criterion benchmark for JPEG-compressed TIFF decode (compression tag 7).
//!
//! Feature-gated on `libtiff`. Setup writes a synthetic 2048×2048×3 tiled TIFF
//! with JPEG compression (RGB color mode, quality 75), reads it back, and
//! benchmarks `get_block(0, 0, 0, None)`.

#![cfg(feature = "libtiff")]

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput};
use tempfile::NamedTempFile;

use _io::tiff::{TIFFDatasetReader, TIFFDatasetWriter, TIFFImageAssetProvider};
use _io::{
    BufferedImageAssetProvider, BufferedMetadataProvider, DatasetReader, DatasetWriter,
    ImageAssetProvider, MemoryImageConfig, PixelType,
};

use super::super::common;

/// Benchmark JPEG-compressed TIFF single-tile decode.
pub fn bench_tiff_jpeg(c: &mut Criterion) {
    // --- Setup (outside timed loop) ---

    let config = MemoryImageConfig::new(common::NCOLS, common::NROWS)
        .with_bands(common::NBANDS)
        .with_block_size(common::NCOLS, common::NROWS)
        .with_pixel_type(PixelType::UInt8);

    let provider = BufferedImageAssetProvider::new("image_segment_0", config);
    let pixels = common::generate_synthetic_pixels(common::DATA_SIZE);
    provider
        .set_full_image(&pixels)
        .expect("set_full_image failed");

    // Compression=7 (JPEG), tile 2048×2048, RGB color mode, quality 75
    let metadata = BufferedMetadataProvider::new();
    metadata.set_json("259", serde_json::json!(7));
    metadata.set_json("322", serde_json::json!(common::NCOLS));
    metadata.set_json("323", serde_json::json!(common::NROWS));
    metadata.set_json("65538", serde_json::json!(1)); // JPEGCOLORMODE_RGB
    metadata.set_json("65537", serde_json::json!(75)); // JPEG quality

    let tmp = NamedTempFile::new().expect("failed to create temp file");
    let mut writer = TIFFDatasetWriter::new(tmp.path()).expect("writer creation failed");
    writer
        .add_asset(
            "image_segment_0",
            Arc::new(provider),
            "Benchmark Image",
            "",
            &[],
        )
        .expect("add_asset failed");
    writer
        .set_metadata(Arc::new(metadata))
        .expect("set_metadata failed");
    writer.close().expect("writer close failed");

    let file_data = std::fs::read(tmp.path()).expect("failed to read TIFF file");
    let reader = TIFFDatasetReader::from_bytes(&file_data).expect("reader creation failed");
    let asset_keys = reader.get_asset_keys(Some(_io::AssetType::Image), None);
    let asset = reader
        .get_asset(&asset_keys[0])
        .expect("get_asset failed");
    let image_provider = asset
        .as_any()
        .downcast_ref::<TIFFImageAssetProvider>()
        .expect("downcast to TIFFImageAssetProvider failed");

    // --- Benchmark ---
    let mut group = c.benchmark_group("tiff_jpeg");
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
