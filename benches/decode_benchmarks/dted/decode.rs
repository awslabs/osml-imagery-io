//! Criterion benchmarks for DTED parse and decode.
//!
//! Uses a synthetic DTED file (writer output) to benchmark:
//! - Header parsing (UHL + DSI + ACC)
//! - Full cell decode (signed-magnitude conversion + transpose)
//! - Signed-magnitude conversion in isolation

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput};

use _io::dted::{DTEDDatasetReader, DTEDDatasetWriter};
use _io::{
    AssetProvider, BufferedImageAssetProvider, BufferedMetadataProvider, DatasetReader,
    DatasetWriter, MemoryImageConfig, OwnedBuffer, PixelType,
};

const DTED_ROWS: u32 = 1201;
const DTED_COLS: u32 = 1201;
const DTED_PIXEL_COUNT: usize = (DTED_ROWS as usize) * (DTED_COLS as usize);

fn generate_synthetic_dted() -> Vec<u8> {
    let pixels: Vec<u8> = (0..DTED_PIXEL_COUNT)
        .map(|i| {
            let x = (i % DTED_COLS as usize) as i16;
            let y = (i / DTED_COLS as usize) as i16;
            (x.wrapping_mul(7).wrapping_add(y.wrapping_mul(13))) % 4000 - 2000
        })
        .flat_map(|v| v.to_ne_bytes())
        .collect();

    let config = MemoryImageConfig::new(DTED_COLS, DTED_ROWS)
        .with_bands(1)
        .with_block_size(DTED_COLS, DTED_ROWS)
        .with_pixel_type(PixelType::Int16);
    let provider = BufferedImageAssetProvider::new("elevation", config);
    provider.set_full_image(&pixels).expect("set_full_image");

    let metadata = BufferedMetadataProvider::new();
    metadata.set("dted:origin_longitude", serde_json::json!(-109.0));
    metadata.set("dted:origin_latitude", serde_json::json!(38.0));
    metadata.set("dted:longitude_interval", serde_json::json!(30));
    metadata.set("dted:latitude_interval", serde_json::json!(30));
    metadata.set("dted:level", serde_json::json!("DTED1"));
    metadata.set("dted:security_code", serde_json::json!("U"));
    metadata.set("dted:vertical_datum", serde_json::json!("MSL"));
    metadata.set("dted:horizontal_datum", serde_json::json!("WGS84"));
    metadata.set("dted:producer_code", serde_json::json!("US"));
    metadata.set("dted:edition_number", serde_json::json!("01"));
    metadata.set("dted:compilation_date", serde_json::json!("0101"));
    metadata.set("dted:partial_cell_indicator", serde_json::json!("00"));
    metadata.set(
        "dted:absolute_horizontal_accuracy",
        serde_json::json!("0050"),
    );
    metadata.set("dted:absolute_vertical_accuracy", serde_json::json!("0030"));
    metadata.set("dted:relative_vertical_accuracy", serde_json::json!("0020"));
    metadata.set("dted:vertical_accuracy", serde_json::json!(20));

    let tmp = tempfile::NamedTempFile::new().expect("tmp");
    let mut writer = DTEDDatasetWriter::new(tmp.path()).expect("writer");
    writer
        .add_asset(
            "elevation",
            AssetProvider::Image(Arc::new(provider)),
            "Elevation",
            "",
            &[],
        )
        .expect("add_asset");
    writer
        .set_metadata(Arc::new(metadata))
        .expect("set_metadata");
    writer.close().expect("close");

    std::fs::read(tmp.path()).expect("read")
}

pub fn bench_dted_decode(c: &mut Criterion) {
    let file_data = generate_synthetic_dted();
    let file_size = file_data.len();
    // Wrap once; OwnedBuffer clones are O(1) refcount bumps, so the timed loop
    // measures parsing rather than repeated Vec copies.
    let buffer = OwnedBuffer::from_vec(file_data);

    let mut group = c.benchmark_group("dted_decode");
    group.sample_size(20);

    // Benchmark: parse header (UHL + DSI + ACC)
    group.throughput(Throughput::Bytes(3428));
    group.bench_function(BenchmarkId::new("parse_headers", "1201x1201"), |b| {
        b.iter(|| DTEDDatasetReader::from_buffer(buffer.clone()).expect("parse"));
    });

    // Benchmark: full cell decode (parse + get_block)
    group.throughput(Throughput::Bytes(file_size as u64));
    let reader = DTEDDatasetReader::from_buffer(buffer.clone()).expect("parse");
    let asset_keys = reader.get_asset_keys(Some(_io::AssetType::Image), None);
    let asset = reader.get_asset(&asset_keys[0]).expect("get_asset");
    let image_provider = asset.as_image().expect("image");

    group.bench_function(BenchmarkId::new("decode_full_cell", "1201x1201_i16"), |b| {
        b.iter(|| image_provider.get_block(0, 0, 0, None).expect("get_block"));
    });

    // Benchmark: signed-magnitude conversion in isolation
    let raw_values: Vec<[u8; 2]> = (0..10000u32)
        .map(|i| ((i as u16) ^ 0x1234).to_be_bytes())
        .collect();
    group.throughput(Throughput::Elements(10000));
    group.bench_function(BenchmarkId::new("signed_magnitude_10k", ""), |b| {
        b.iter(|| {
            raw_values
                .iter()
                .map(|v| _io::dted::records::decode_elevation(*v))
                .sum::<i16>()
        });
    });

    group.finish();
}
