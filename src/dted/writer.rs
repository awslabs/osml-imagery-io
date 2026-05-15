//! DTEDDatasetWriter — implements DatasetWriter for DTED files.
//!
//! Encodes a single-band Int16 elevation grid into a valid DTED binary file
//! with UHL, DSI, ACC header records and column-major data records with
//! signed-magnitude encoding and per-record checksums.

use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex};

use crate::dted::records::{
    compute_record_checksum, encode_elevation, ACC_SIZE, DSI_SIZE, UHL_SIZE,
};
use crate::error::CodecError;
use crate::traits::asset::AssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::traits::writer::DatasetWriter;
use crate::types::{AssetType, PixelType};

struct QueuedDtedAsset {
    provider: AssetProvider,
}

/// Writer for DTED datasets implementing the `DatasetWriter` trait.
///
/// Queues a single Int16 image asset and metadata, then encodes a complete
/// DTED file (UHL + DSI + ACC + data records) on `close()`.
pub struct DTEDDatasetWriter {
    output: Mutex<Option<Box<dyn Write + Send>>>,
    image_queued: bool,
    metadata: Option<Arc<dyn MetadataProvider>>,
    closed: bool,
    assets: Vec<QueuedDtedAsset>,
}

impl DTEDDatasetWriter {
    pub fn new_with_output(output: Box<dyn Write + Send>) -> Result<Self, CodecError> {
        Ok(Self {
            output: Mutex::new(Some(output)),
            image_queued: false,
            metadata: None,
            closed: false,
            assets: Vec::new(),
        })
    }

    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, CodecError> {
        let file = std::fs::File::create(path.as_ref()).map_err(CodecError::Io)?;
        let buf_writer = BufWriter::new(file);
        Self::new_with_output(Box::new(buf_writer))
    }

    fn get_metadata_str(&self, key: &str) -> Option<String> {
        self.metadata.as_ref().and_then(|m| {
            let dict = m.entries(None);
            dict.get(key).and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| Some(v.to_string()))
            })
        })
    }

    fn get_metadata_u16(&self, key: &str) -> Option<u16> {
        self.metadata.as_ref().and_then(|m| {
            let dict = m.entries(None);
            dict.get(key).and_then(|v| v.as_u64().map(|n| n as u16))
        })
    }

    fn get_metadata_f64(&self, key: &str) -> Option<f64> {
        self.metadata.as_ref().and_then(|m| {
            let dict = m.entries(None);
            dict.get(key).and_then(|v| v.as_f64())
        })
    }

    fn format_longitude(degrees: f64) -> [u8; 8] {
        let hemi = if degrees < 0.0 { b'W' } else { b'E' };
        let abs = degrees.abs();
        let d = abs as u32;
        let rem = (abs - d as f64) * 60.0;
        let m = rem as u32;
        let s = ((rem - m as f64) * 60.0).round() as u32;
        let s = format!("{:03}{:02}{:02}{}", d, m, s, hemi as char);
        let mut out = [b' '; 8];
        out[..s.len().min(8)].copy_from_slice(&s.as_bytes()[..s.len().min(8)]);
        out
    }

    fn format_latitude(degrees: f64) -> [u8; 8] {
        let hemi = if degrees < 0.0 { b'S' } else { b'N' };
        let abs = degrees.abs();
        let d = abs as u32;
        let rem = (abs - d as f64) * 60.0;
        let m = rem as u32;
        let s = ((rem - m as f64) * 60.0).round() as u32;
        let s = format!("{:02}{:02}{:02}{} ", d, m, s, hemi as char);
        let mut out = [b' '; 8];
        out[..s.len().min(8)].copy_from_slice(&s.as_bytes()[..s.len().min(8)]);
        out
    }

    fn build_uhl(&self, num_lon_lines: u16, num_lat_points: u16) -> [u8; UHL_SIZE] {
        let mut uhl = [b' '; UHL_SIZE];

        // Sentinel + fixed '1'
        uhl[0..3].copy_from_slice(b"UHL");
        uhl[3] = b'1';

        // Origin coordinates
        let origin_lon = self
            .get_metadata_f64("dted:origin_longitude")
            .unwrap_or(0.0);
        let origin_lat = self.get_metadata_f64("dted:origin_latitude").unwrap_or(0.0);
        uhl[4..12].copy_from_slice(&Self::format_longitude(origin_lon));
        uhl[12..20].copy_from_slice(&Self::format_latitude(origin_lat));

        // Intervals
        let lon_interval = self
            .get_metadata_u16("dted:longitude_interval")
            .unwrap_or(30);
        let lat_interval = self
            .get_metadata_u16("dted:latitude_interval")
            .unwrap_or(30);
        let lon_int_str = format!("{:04}", lon_interval);
        let lat_int_str = format!("{:04}", lat_interval);
        uhl[20..24].copy_from_slice(lon_int_str.as_bytes());
        uhl[24..28].copy_from_slice(lat_int_str.as_bytes());

        // Vertical accuracy
        let va = self.get_metadata_u16("dted:vertical_accuracy");
        match va {
            Some(v) => {
                let va_str = format!("{:04}", v);
                uhl[28..32].copy_from_slice(va_str.as_bytes());
            }
            None => uhl[28..32].copy_from_slice(b"NA  "),
        }

        // Security code
        let sec = self
            .get_metadata_str("dted:security_code")
            .unwrap_or_else(|| "U".to_string());
        uhl[32] = sec.as_bytes().first().copied().unwrap_or(b'U');

        // Unique reference (12 bytes, bytes 33..45): spaces
        // Reserved (2 bytes, bytes 45..47): spaces

        // Num longitude lines
        let lon_str = format!("{:>4}", num_lon_lines);
        uhl[47..51].copy_from_slice(lon_str.as_bytes());

        // Num latitude points
        let lat_str = format!("{:>4}", num_lat_points);
        uhl[51..55].copy_from_slice(lat_str.as_bytes());

        // Multiple accuracy
        uhl[55] = b'0';

        uhl
    }

    fn build_dsi(&self) -> [u8; DSI_SIZE] {
        let mut dsi = [b' '; DSI_SIZE];

        // Sentinel
        dsi[0..3].copy_from_slice(b"DSI");

        // Security code
        let sec = self
            .get_metadata_str("dted:security_code")
            .unwrap_or_else(|| "U".to_string());
        dsi[3] = sec.as_bytes().first().copied().unwrap_or(b'U');

        // Product level (5 chars at offset 59)
        let level = self
            .get_metadata_str("dted:level")
            .unwrap_or_else(|| "DTED1".to_string());
        let level_bytes = level.as_bytes();
        let len = level_bytes.len().min(5);
        dsi[59..59 + len].copy_from_slice(&level_bytes[..len]);

        // Producer code (8 chars at offset 65)
        let producer = self
            .get_metadata_str("dted:producer_code")
            .unwrap_or_default();
        let pb = producer.as_bytes();
        let len = pb.len().min(8);
        dsi[65..65 + len].copy_from_slice(&pb[..len]);

        // Edition number (2 chars at offset 87)
        let edition = self
            .get_metadata_str("dted:edition_number")
            .unwrap_or_else(|| "01".to_string());
        let eb = edition.as_bytes();
        let len = eb.len().min(2);
        dsi[87..87 + len].copy_from_slice(&eb[..len]);

        // Compilation date (4 chars at offset 93)
        let comp_date = self
            .get_metadata_str("dted:compilation_date")
            .unwrap_or_else(|| "0101".to_string());
        let cb = comp_date.as_bytes();
        let len = cb.len().min(4);
        dsi[93..93 + len].copy_from_slice(&cb[..len]);

        // Vertical datum (3 chars at offset 141)
        let vdatum = self
            .get_metadata_str("dted:vertical_datum")
            .unwrap_or_else(|| "MSL".to_string());
        let vb = vdatum.as_bytes();
        let len = vb.len().min(3);
        dsi[141..141 + len].copy_from_slice(&vb[..len]);

        // Horizontal datum (5 chars at offset 144)
        let hdatum = self
            .get_metadata_str("dted:horizontal_datum")
            .unwrap_or_else(|| "WGS84".to_string());
        let hb = hdatum.as_bytes();
        let len = hb.len().min(5);
        dsi[144..144 + len].copy_from_slice(&hb[..len]);

        // Partial cell indicator (2 chars at offset 289)
        let pci = self
            .get_metadata_str("dted:partial_cell_indicator")
            .unwrap_or_else(|| "00".to_string());
        let pb2 = pci.as_bytes();
        let len = pb2.len().min(2);
        dsi[289..289 + len].copy_from_slice(&pb2[..len]);

        dsi
    }

    fn build_acc(&self) -> [u8; ACC_SIZE] {
        let mut acc = [b' '; ACC_SIZE];

        // Sentinel
        acc[0..3].copy_from_slice(b"ACC");

        // Absolute horizontal accuracy (4 chars at offset 3)
        let aha = self
            .get_metadata_str("dted:absolute_horizontal_accuracy")
            .unwrap_or_else(|| "NA  ".to_string());
        let ab = aha.as_bytes();
        let len = ab.len().min(4);
        acc[3..3 + len].copy_from_slice(&ab[..len]);

        // Absolute vertical accuracy (4 chars at offset 7)
        let ava = self
            .get_metadata_str("dted:absolute_vertical_accuracy")
            .unwrap_or_else(|| "NA  ".to_string());
        let ab2 = ava.as_bytes();
        let len = ab2.len().min(4);
        acc[7..7 + len].copy_from_slice(&ab2[..len]);

        // Relative vertical accuracy (4 chars at offset 11)
        let rva = self
            .get_metadata_str("dted:relative_vertical_accuracy")
            .unwrap_or_else(|| "NA  ".to_string());
        let rb = rva.as_bytes();
        let len = rb.len().min(4);
        acc[11..11 + len].copy_from_slice(&rb[..len]);

        acc
    }
}

impl DatasetWriter for DTEDDatasetWriter {
    fn add_asset(
        &mut self,
        _key: &str,
        provider: AssetProvider,
        _title: &str,
        _description: &str,
        _roles: &[String],
    ) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Unsupported(
                "Writer is already closed".to_string(),
            ));
        }

        if provider.asset_type() != AssetType::Image {
            return Err(CodecError::Unsupported(
                "DTED format supports only image assets".to_string(),
            ));
        }

        if self.image_queued {
            return Err(CodecError::Unsupported(
                "DTED format supports only a single image per file".to_string(),
            ));
        }

        // Validate pixel type
        let image = provider
            .as_image()
            .ok_or_else(|| CodecError::Unsupported("Asset is not an Image variant".to_string()))?;
        if image.pixel_value_type() != PixelType::Int16 {
            return Err(CodecError::Unsupported(format!(
                "DTED format requires Int16 pixel type, got {:?}",
                image.pixel_value_type()
            )));
        }
        if image.num_bands() != 1 {
            return Err(CodecError::Unsupported(format!(
                "DTED format requires exactly 1 band, got {}",
                image.num_bands()
            )));
        }

        self.assets.push(QueuedDtedAsset { provider });
        self.image_queued = true;
        Ok(())
    }

    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
        self.metadata = Some(metadata);
        Ok(())
    }

    fn close(&mut self) -> Result<(), CodecError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;

        let asset = match self.assets.first() {
            Some(a) => a,
            None => return Ok(()),
        };

        let image = asset
            .provider
            .as_image()
            .ok_or_else(|| CodecError::Unsupported("Asset is not an Image variant".to_string()))?;
        let image = image.as_ref();

        let num_lon_lines = image.num_columns() as u16;
        let num_lat_points = image.num_rows() as u16;

        // Read all pixel data (BSQ, single band, native-endian i16)
        let (bsq_data, shape) = image.get_block(0, 0, 0, None)?;
        let expected_pixels = num_lon_lines as usize * num_lat_points as usize;
        if bsq_data.len() != expected_pixels * 2 {
            return Err(CodecError::Encode(format!(
                "Expected {} bytes of pixel data ({} pixels × 2), got {}",
                expected_pixels * 2,
                expected_pixels,
                bsq_data.len()
            )));
        }

        // Verify shape matches
        if shape[1] != num_lat_points as u32 || shape[2] != num_lon_lines as u32 {
            return Err(CodecError::Encode(format!(
                "Image shape {:?} does not match expected [{}, {}]",
                shape, num_lat_points, num_lon_lines
            )));
        }

        // Build header records
        let uhl = self.build_uhl(num_lon_lines, num_lat_points);
        let dsi = self.build_dsi();
        let acc = self.build_acc();

        // Take the output writer
        let mut output = self
            .output
            .lock()
            .map_err(|_| CodecError::Unsupported("DTED writer output mutex poisoned".to_string()))?
            .take()
            .ok_or_else(|| {
                CodecError::Unsupported("DTED writer output is not available".to_string())
            })?;

        // Write headers
        output.write_all(&uhl).map_err(CodecError::Io)?;
        output.write_all(&dsi).map_err(CodecError::Io)?;
        output.write_all(&acc).map_err(CodecError::Io)?;

        // Interpret pixel data as row-major i16 array (north→south rows, west→east cols).
        // We need to write column-major data records (south→north posts per column).
        let rows = num_lat_points as usize;
        let cols = num_lon_lines as usize;

        let pixels: Vec<i16> = bsq_data
            .chunks_exact(2)
            .map(|c| i16::from_ne_bytes([c[0], c[1]]))
            .collect();

        // Write data records (one per longitude column, west→east)
        for col in 0..cols {
            let rec_data_size = 8 + rows * 2;
            let mut record = Vec::with_capacity(rec_data_size + 4);

            // Record header
            record.push(0xAA); // sentinel
                               // Block count (3 bytes, 0-indexed column)
            record.push(((col >> 16) & 0xFF) as u8);
            record.push(((col >> 8) & 0xFF) as u8);
            record.push((col & 0xFF) as u8);
            // Longitude count (2 bytes)
            record.push(((col >> 8) & 0xFF) as u8);
            record.push((col & 0xFF) as u8);
            // Latitude count (2 bytes) — starting latitude point = 0
            record.push(0);
            record.push(0);

            // Elevation posts (south→north = reverse row order)
            for row in (0..rows).rev() {
                let value = pixels[row * cols + col];
                let encoded = encode_elevation(value);
                record.push(encoded[0]);
                record.push(encoded[1]);
            }

            // Compute and append checksum
            let checksum = compute_record_checksum(&record);
            record.extend_from_slice(&checksum.to_be_bytes());

            output.write_all(&record).map_err(CodecError::Io)?;
        }

        output.flush().map_err(CodecError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffered::{
        BufferedImageAssetProvider, BufferedMetadataProvider, MemoryImageConfig,
    };
    use crate::dted::reader::DTEDDatasetReader;
    use crate::dted::records::DATA_OFFSET;
    use crate::traits::reader::DatasetReader;
    use serde_json::json;

    fn make_image_provider(
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Arc<BufferedImageAssetProvider> {
        let config = MemoryImageConfig::new(width, height)
            .with_bands(1)
            .with_block_size(width, height)
            .with_pixel_type(PixelType::Int16);
        let provider = BufferedImageAssetProvider::new("elevation", config);
        provider.set_block(0, 0, data).unwrap();
        Arc::new(provider)
    }

    fn make_metadata(
        origin_lon: f64,
        origin_lat: f64,
        lon_interval: u16,
        lat_interval: u16,
    ) -> Arc<BufferedMetadataProvider> {
        let meta = BufferedMetadataProvider::new();
        meta.set("dted:origin_longitude", json!(origin_lon));
        meta.set("dted:origin_latitude", json!(origin_lat));
        meta.set("dted:longitude_interval", json!(lon_interval));
        meta.set("dted:latitude_interval", json!(lat_interval));
        meta.set("dted:level", serde_json::json!("DTED1"));
        meta.set("dted:security_code", serde_json::json!("U"));
        meta.set("dted:vertical_datum", serde_json::json!("MSL"));
        meta.set("dted:horizontal_datum", serde_json::json!("WGS84"));
        meta.set("dted:producer_code", serde_json::json!("US"));
        meta.set("dted:edition_number", serde_json::json!("01"));
        meta.set("dted:compilation_date", serde_json::json!("0101"));
        meta.set("dted:partial_cell_indicator", serde_json::json!("00"));
        meta.set("dted:absolute_horizontal_accuracy", serde_json::json!("0050"));
        meta.set("dted:absolute_vertical_accuracy", serde_json::json!("0030"));
        meta.set("dted:relative_vertical_accuracy", serde_json::json!("0020"));
        meta.set("dted:vertical_accuracy", json!(20));
        Arc::new(meta)
    }

    #[test]
    fn test_writer_new() {
        let output: Box<dyn Write + Send> = Box::new(Vec::<u8>::new());
        let writer = DTEDDatasetWriter::new_with_output(output);
        assert!(writer.is_ok());
        let w = writer.unwrap();
        assert!(!w.closed);
        assert!(!w.image_queued);
    }

    #[test]
    fn test_add_non_image_rejected() {
        use crate::buffered::BufferedTextAssetProvider;

        let output: Box<dyn Write + Send> = Box::new(Vec::<u8>::new());
        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        let text = Arc::new(BufferedTextAssetProvider::new(
            "t",
            "hello".to_string(),
            "utf-8",
        ));
        let result = writer.add_asset("text", AssetProvider::Text(text), "Text", "desc", &[]);
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_add_duplicate_rejected() {
        let output: Box<dyn Write + Send> = Box::new(Vec::<u8>::new());
        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();

        let pixels: Vec<u8> = vec![0; 3 * 4 * 2];
        let p1 = make_image_provider(3, 4, &pixels);
        let p2 = make_image_provider(3, 4, &pixels);

        writer
            .add_asset("e1", AssetProvider::Image(p1), "T", "D", &[])
            .unwrap();
        let result = writer.add_asset("e2", AssetProvider::Image(p2), "T", "D", &[]);
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_close_idempotent() {
        let output: Box<dyn Write + Send> = Box::new(Vec::<u8>::new());
        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();

        let pixels: Vec<u8> = vec![0; 3 * 4 * 2];
        let provider = make_image_provider(3, 4, &pixels);
        let metadata = make_metadata(-109.0, 38.0, 30, 30);

        writer
            .add_asset("elevation", AssetProvider::Image(provider), "T", "D", &[])
            .unwrap();
        writer.set_metadata(metadata).unwrap();
        assert!(writer.close().is_ok());
        assert!(writer.close().is_ok());
    }

    #[test]
    fn test_roundtrip_small_grid() {
        // Create a 3×4 grid (3 columns, 4 rows):
        // Row 0 (north): 400, 800, 1200
        // Row 1:         300, 700, 1100
        // Row 2:         200, 600, 1000
        // Row 3 (south): 100, 500, 900
        let values: Vec<i16> = vec![
            400, 800, 1200, 300, 700, 1100, 200, 600, 1000, 100, 500, 900,
        ];
        let pixels: Vec<u8> = values.iter().flat_map(|&v| v.to_ne_bytes()).collect();

        let provider = make_image_provider(3, 4, &pixels);
        let metadata = make_metadata(-109.0, 38.0, 30, 30);

        let buf: Vec<u8> = Vec::new();
        let output: Box<dyn Write + Send> = Box::new(std::io::Cursor::new(buf));
        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        writer
            .add_asset("elevation", AssetProvider::Image(provider), "T", "D", &[])
            .unwrap();
        writer.set_metadata(metadata).unwrap();
        writer.close().unwrap();

        // Extract the written bytes from the cursor
        let output_ref = writer.output.lock().unwrap();
        assert!(output_ref.is_none()); // writer consumed the output
        drop(output_ref);

        // Re-create writer with a shared buffer we can read back
        let shared_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let write_buf = shared_buf.clone();
        let output: Box<dyn Write + Send> = Box::new(SharedVecWriter(write_buf));

        let provider2 = make_image_provider(3, 4, &pixels);
        let metadata2 = make_metadata(-109.0, 38.0, 30, 30);

        let mut writer2 = DTEDDatasetWriter::new_with_output(output).unwrap();
        writer2
            .add_asset("elevation", AssetProvider::Image(provider2), "T", "D", &[])
            .unwrap();
        writer2.set_metadata(metadata2).unwrap();
        writer2.close().unwrap();

        let written = shared_buf.lock().unwrap().clone();

        // Verify we can read it back
        let reader = DTEDDatasetReader::from_bytes(&written).unwrap();
        let asset = reader.get_asset("elevation").unwrap();
        let image = asset.as_image().unwrap();
        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();

        assert_eq!(shape, [1, 4, 3]);

        let read_values: Vec<i16> = read_pixels
            .chunks_exact(2)
            .map(|c| i16::from_ne_bytes([c[0], c[1]]))
            .collect();

        assert_eq!(read_values, values);
    }

    #[test]
    fn test_roundtrip_negative_values() {
        let values: Vec<i16> = vec![-100, 0, 500, -32767, 200, -1];
        let pixels: Vec<u8> = values.iter().flat_map(|&v| v.to_ne_bytes()).collect();

        let provider = make_image_provider(3, 2, &pixels);
        let metadata = make_metadata(0.0, 0.0, 30, 30);

        let shared_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let write_buf = shared_buf.clone();
        let output: Box<dyn Write + Send> = Box::new(SharedVecWriter(write_buf));

        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        writer
            .add_asset("elevation", AssetProvider::Image(provider), "T", "D", &[])
            .unwrap();
        writer.set_metadata(metadata).unwrap();
        writer.close().unwrap();

        let written = shared_buf.lock().unwrap().clone();

        let reader = DTEDDatasetReader::from_bytes(&written).unwrap();
        let asset = reader.get_asset("elevation").unwrap();
        let image = asset.as_image().unwrap();
        let (read_pixels, shape) = image.get_block(0, 0, 0, None).unwrap();

        assert_eq!(shape, [1, 2, 3]);
        let read_values: Vec<i16> = read_pixels
            .chunks_exact(2)
            .map(|c| i16::from_ne_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(read_values, values);
    }

    #[test]
    fn test_file_structure_valid() {
        let values: Vec<i16> = vec![100, 200, 300, 400, 500, 600];
        let pixels: Vec<u8> = values.iter().flat_map(|&v| v.to_ne_bytes()).collect();

        let provider = make_image_provider(2, 3, &pixels);
        let metadata = make_metadata(-109.0, 38.0, 30, 30);

        let shared_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let write_buf = shared_buf.clone();
        let output: Box<dyn Write + Send> = Box::new(SharedVecWriter(write_buf));

        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        writer
            .add_asset("elevation", AssetProvider::Image(provider), "T", "D", &[])
            .unwrap();
        writer.set_metadata(metadata).unwrap();
        writer.close().unwrap();

        let written = shared_buf.lock().unwrap().clone();

        // Verify structure: UHL + DSI + ACC + 2 records
        assert_eq!(&written[0..3], b"UHL");
        assert_eq!(&written[UHL_SIZE..UHL_SIZE + 3], b"DSI");
        assert_eq!(
            &written[UHL_SIZE + DSI_SIZE..UHL_SIZE + DSI_SIZE + 3],
            b"ACC"
        );

        // Each record: 8 header + 3 posts × 2 bytes + 4 checksum = 18 bytes
        let rec_size = 8 + 3 * 2 + 4;
        let expected_total = DATA_OFFSET + 2 * rec_size;
        assert_eq!(written.len(), expected_total);

        // Verify record sentinels
        assert_eq!(written[DATA_OFFSET], 0xAA);
        assert_eq!(written[DATA_OFFSET + rec_size], 0xAA);

        // Verify checksums are valid
        let rec1 = &written[DATA_OFFSET..DATA_OFFSET + rec_size];
        let rec2 = &written[DATA_OFFSET + rec_size..DATA_OFFSET + 2 * rec_size];
        assert!(crate::dted::records::validate_record_checksum(rec1));
        assert!(crate::dted::records::validate_record_checksum(rec2));
    }

    #[test]
    fn test_wrong_pixel_type_rejected() {
        let config = MemoryImageConfig::new(3, 4)
            .with_bands(1)
            .with_block_size(3, 4)
            .with_pixel_type(PixelType::UInt8);
        let provider = BufferedImageAssetProvider::new("elevation", config);
        provider.set_block(0, 0, &[0; 12]).unwrap();

        let output: Box<dyn Write + Send> = Box::new(Vec::<u8>::new());
        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        let result = writer.add_asset(
            "elevation",
            AssetProvider::Image(Arc::new(provider)),
            "T",
            "D",
            &[],
        );
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_multi_band_rejected() {
        let config = MemoryImageConfig::new(3, 4)
            .with_bands(3)
            .with_block_size(3, 4)
            .with_pixel_type(PixelType::Int16);
        let provider = BufferedImageAssetProvider::new("elevation", config);
        provider.set_block(0, 0, &[0; 3 * 4 * 3 * 2]).unwrap();

        let output: Box<dyn Write + Send> = Box::new(Vec::<u8>::new());
        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        let result = writer.add_asset(
            "elevation",
            AssetProvider::Image(Arc::new(provider)),
            "T",
            "D",
            &[],
        );
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_metadata_written_to_headers() {
        let values: Vec<i16> = vec![100; 6];
        let pixels: Vec<u8> = values.iter().flat_map(|&v| v.to_ne_bytes()).collect();

        let provider = make_image_provider(3, 2, &pixels);
        let metadata = make_metadata(-109.0, 38.0, 30, 30);

        let shared_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let write_buf = shared_buf.clone();
        let output: Box<dyn Write + Send> = Box::new(SharedVecWriter(write_buf));

        let mut writer = DTEDDatasetWriter::new_with_output(output).unwrap();
        writer
            .add_asset("elevation", AssetProvider::Image(provider), "T", "D", &[])
            .unwrap();
        writer.set_metadata(metadata).unwrap();
        writer.close().unwrap();

        let written = shared_buf.lock().unwrap().clone();

        // Read back and check metadata
        let reader = DTEDDatasetReader::from_bytes(&written).unwrap();
        let meta = reader.metadata();
        let dict = meta.entries(None);
        assert_eq!(
            dict.get("dted:horizontal_datum").unwrap().as_str(),
            Some("WGS84")
        );
        assert_eq!(
            dict.get("dted:vertical_datum").unwrap().as_str(),
            Some("MSL")
        );
        assert_eq!(dict.get("dted:security_code").unwrap().as_str(), Some("U"));
    }

    // Helper: a Write impl that appends to a shared Vec
    struct SharedVecWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedVecWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
