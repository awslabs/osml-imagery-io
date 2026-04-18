//! Criterion benchmark for HTJ2K / JPEG 2000 Part 15 (IC=CD) decode via the JBP writer/reader pipeline.
//!
//! Feature-gated on `openjpeg`. Setup writes a synthetic 2048×2048×3 image as
//! a NITF with IC=CD, reads it back, and benchmarks `get_block(0, 0, 0, None)`.

#![cfg(feature = "openjpeg")]

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput};
use tempfile::NamedTempFile;

use _io::jbp::{JBPDatasetReader, JBPDatasetWriter, JBPImageAssetProvider, NitfFormat};
use _io::{
    BufferedImageAssetProvider, BufferedMetadataProvider, DatasetReader, DatasetWriter,
    ImageAssetProvider, MemoryImageConfig, PixelType,
};

use super::super::common;

/// Benchmark HTJ2K (IC=CD) single-block decode.
pub fn bench_jbp_cd(c: &mut Criterion) {
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

    let metadata = BufferedMetadataProvider::new();
    metadata.set("ic", "CD");
    metadata.set("nppbh", &common::NCOLS.to_string());
    metadata.set("nppbv", &common::NROWS.to_string());

    let tmp = NamedTempFile::new().expect("failed to create temp file");
    let mut writer =
        JBPDatasetWriter::new(tmp.path(), NitfFormat::Nitf21).expect("writer creation failed");
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

    let file_data = std::fs::read(tmp.path()).expect("failed to read NITF file");
    let reader = JBPDatasetReader::from_bytes(&file_data).expect("reader creation failed");
    let asset_keys = reader.get_asset_keys(Some(_io::AssetType::Image), None);
    let asset = reader.get_asset(&asset_keys[0]).expect("get_asset failed");
    let image_provider = asset
        .as_any()
        .downcast_ref::<JBPImageAssetProvider>()
        .expect("downcast to JBPImageAssetProvider failed");

    // --- Benchmark ---
    let mut group = c.benchmark_group("jbp_cd");
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
