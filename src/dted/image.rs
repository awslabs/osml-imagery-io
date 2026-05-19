//! DTEDImageAssetProvider — implements ImageAssetProvider for DTED elevation data.
//!
//! Exposes the elevation grid as a single-band Int16 raster in a single
//! full-image block (column-major source is transposed to row-major on read).

use std::collections::HashMap;
use std::sync::Arc;

use crate::dted::metadata::DTEDMetadataProvider;
use crate::dted::records::{decode_elevation, record_size, validate_record_checksum, DATA_OFFSET};
use crate::error::CodecError;
use crate::traits::asset::AssetMetadata;
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::types::PixelType;

/// Image asset provider for DTED elevation data.
///
/// Reads elevation posts from column-major data records, converts
/// signed-magnitude big-endian values to native i16, and transposes
/// to row-major BSQ output format.
pub struct DTEDImageAssetProvider {
    data: Arc<[u8]>,
    num_lon_lines: u32,
    num_lat_points: u32,
    record_size: usize,
    roles: Vec<String>,
    metadata: Arc<DTEDMetadataProvider>,
}

impl DTEDImageAssetProvider {
    pub fn new(
        data: Arc<[u8]>,
        num_lon_lines: u16,
        num_lat_points: u16,
        metadata: Arc<DTEDMetadataProvider>,
    ) -> Self {
        Self {
            data,
            num_lon_lines: num_lon_lines as u32,
            num_lat_points: num_lat_points as u32,
            record_size: record_size(num_lat_points),
            roles: vec!["data".to_string(), "elevation".to_string()],
            metadata,
        }
    }

    fn decode_full_grid(&self) -> Result<Vec<u8>, CodecError> {
        let cols = self.num_lon_lines as usize;
        let rows = self.num_lat_points as usize;
        let mut output = vec![0i16; rows * cols];

        for col in 0..cols {
            let record_offset = DATA_OFFSET + col * self.record_size;
            let record_end = record_offset + self.record_size;

            if record_end > self.data.len() {
                return Err(CodecError::Decode(format!(
                    "DTED data record {} extends beyond file (offset {} + size {} > file size {})",
                    col,
                    record_offset,
                    self.record_size,
                    self.data.len()
                )));
            }

            let record = &self.data[record_offset..record_end];

            if record[0] != 0xAA {
                return Err(CodecError::Decode(format!(
                    "DTED data record {} has invalid sentinel: expected 0xAA, got 0x{:02X}",
                    col, record[0]
                )));
            }

            if !validate_record_checksum(record) {
                return Err(CodecError::Decode(format!(
                    "DTED data record {} has invalid checksum",
                    col
                )));
            }

            // Elevation posts start at byte 8 of the record.
            // DTED stores columns south→north, we want rows north→south
            // (row 0 = northernmost). Transpose: output[row][col] reads
            // from column `col`, post index `(rows - 1 - row)`.
            let elev_start = 8;
            for row in 0..rows {
                let post_index = rows - 1 - row;
                let byte_offset = elev_start + post_index * 2;
                let bytes = [record[byte_offset], record[byte_offset + 1]];
                let value = decode_elevation(bytes);
                output[row * cols + col] = value;
            }
        }

        // Convert i16 slice to bytes (native endian)
        let byte_output: Vec<u8> = output.iter().flat_map(|&v| v.to_ne_bytes()).collect();

        Ok(byte_output)
    }
}

impl AssetMetadata for DTEDImageAssetProvider {
    fn key(&self) -> &str {
        "elevation"
    }

    fn title(&self) -> &str {
        "elevation"
    }

    fn description(&self) -> &str {
        "DTED elevation grid"
    }

    fn media_type(&self) -> &str {
        "application/octet-stream"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "raw_asset() not supported for DTED; use get_block()".to_string(),
        ))
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }
}

impl ImageAssetProvider for DTEDImageAssetProvider {
    fn has_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
    ) -> Result<bool, CodecError> {
        Ok(resolution_level == 0 && block_row == 0 && block_col == 0)
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        if resolution_level > 0 {
            return Err(CodecError::InvalidResolutionLevel(resolution_level));
        }
        if block_row != 0 || block_col != 0 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Band subsetting: DTED has only 1 band
        if let Some(b) = bands {
            if b.is_empty() || b.iter().any(|&i| i != 0) {
                return Err(CodecError::InvalidBlockCoordinates(
                    block_row,
                    block_col,
                    resolution_level,
                ));
            }
        }

        let pixels = self.decode_full_grid()?;
        Ok((pixels, [1, self.num_lat_points, self.num_lon_lines]))
    }

    fn num_resolution_levels(&self) -> u32 {
        1
    }

    fn num_bands(&self) -> u32 {
        1
    }

    fn num_rows(&self) -> u32 {
        self.num_lat_points
    }

    fn num_columns(&self) -> u32 {
        self.num_lon_lines
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.num_lon_lines
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.num_lat_points
    }

    fn num_bits_per_pixel(&self) -> u32 {
        16
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        16
    }

    fn pixel_value_type(&self) -> PixelType {
        PixelType::Int16
    }

    fn pad_pixel_value(&self) -> f64 {
        -32767.0
    }

    fn tile_byte_ranges(&self) -> Option<HashMap<(u32, u32), Vec<(u64, u64)>>> {
        let total_data_len = self.num_lon_lines as u64 * self.record_size as u64;
        let mut map = HashMap::new();
        map.insert((0u32, 0u32), vec![(DATA_OFFSET as u64, total_data_len)]);
        Some(map)
    }

    fn codec_configuration(&self) -> Option<HashMap<String, Vec<u8>>> {
        let mut config = HashMap::new();
        config.insert("dted_codec".to_string(), Vec::new());
        config.insert(
            "num_lat_points".to_string(),
            (self.num_lat_points as u32).to_le_bytes().to_vec(),
        );
        config.insert(
            "num_lon_lines".to_string(),
            (self.num_lon_lines as u32).to_le_bytes().to_vec(),
        );
        config.insert(
            "record_size".to_string(),
            (self.record_size as u32).to_le_bytes().to_vec(),
        );
        Some(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dted::records::{Acc, Dsi, Uhl};

    fn make_test_metadata() -> Arc<DTEDMetadataProvider> {
        let uhl = Uhl {
            origin_lon: -109.0,
            origin_lat: 38.0,
            lon_interval_tenths: 30,
            lat_interval_tenths: 30,
            num_lon_lines: 3,
            num_lat_points: 4,
            vertical_accuracy: Some(20),
            security_code: 'U',
            multiple_accuracy: false,
        };
        let dsi = Dsi {
            security_code: "U".to_string(),
            product_level: "DTED1".to_string(),
            edition_number: "01".to_string(),
            compilation_date: "0101".to_string(),
            producer_code: "US".to_string(),
            vertical_datum: "MSL".to_string(),
            horizontal_datum: "WGS84".to_string(),
            partial_cell_indicator: "00".to_string(),
        };
        let acc = Acc {
            absolute_horizontal_accuracy: "0050".to_string(),
            absolute_vertical_accuracy: "0030".to_string(),
            relative_vertical_accuracy: "0020".to_string(),
        };
        Arc::new(DTEDMetadataProvider::new(&uhl, &dsi, &acc, &[]))
    }

    /// Build a synthetic DTED file with 3 columns, 4 rows.
    /// Elevations: col 0 = [100, 200, 300, 400] (south→north)
    ///             col 1 = [500, 600, 700, 800]
    ///             col 2 = [900, 1000, 1100, 1200]
    fn make_synthetic_dted(num_lon_lines: u16, num_lat_points: u16) -> Vec<u8> {
        let rec_size = record_size(num_lat_points);
        let total_size = DATA_OFFSET + (num_lon_lines as usize) * rec_size;
        let mut data = vec![0u8; total_size];

        // UHL sentinel
        data[0..3].copy_from_slice(b"UHL");
        // DSI sentinel
        data[80..83].copy_from_slice(b"DSI");
        // ACC sentinel
        data[728..731].copy_from_slice(b"ACC");

        // Write data records
        let mut value = 100i16;
        for col in 0..num_lon_lines as usize {
            let offset = DATA_OFFSET + col * rec_size;
            data[offset] = 0xAA; // sentinel
                                 // block count (3 bytes)
            data[offset + 1] = 0;
            data[offset + 2] = 0;
            data[offset + 3] = col as u8;
            // lon count (2 bytes)
            data[offset + 4] = 0;
            data[offset + 5] = col as u8;
            // lat count (2 bytes)
            data[offset + 6] = 0;
            data[offset + 7] = 0;

            // Elevation posts (south→north)
            for post in 0..num_lat_points as usize {
                let elev_offset = offset + 8 + post * 2;
                // Encode as signed-magnitude BE
                let encoded = if value < 0 {
                    let mag = (-value) as u16;
                    (mag | 0x8000).to_be_bytes()
                } else {
                    (value as u16).to_be_bytes()
                };
                data[elev_offset] = encoded[0];
                data[elev_offset + 1] = encoded[1];
                value += 100;
            }

            // Checksum
            let payload = &data[offset..offset + rec_size - 4];
            let checksum: u32 = payload.iter().map(|&b| b as u32).sum();
            let cs_offset = offset + rec_size - 4;
            data[cs_offset..cs_offset + 4].copy_from_slice(&checksum.to_be_bytes());
        }

        data
    }

    #[test]
    fn test_image_provider_dimensions() {
        let data = make_synthetic_dted(3, 4);
        let metadata = make_test_metadata();
        let provider = DTEDImageAssetProvider::new(Arc::from(data.as_slice()), 3, 4, metadata);

        assert_eq!(provider.num_columns(), 3);
        assert_eq!(provider.num_rows(), 4);
        assert_eq!(provider.num_bands(), 1);
        assert_eq!(provider.pixel_value_type(), PixelType::Int16);
        assert_eq!(provider.num_bits_per_pixel(), 16);
        assert_eq!(provider.num_pixels_per_block_horizontal(), 3);
        assert_eq!(provider.num_pixels_per_block_vertical(), 4);
        assert_eq!(provider.num_resolution_levels(), 1);
        assert_eq!(provider.block_grid_size(), (1, 1));
    }

    #[test]
    fn test_image_provider_get_block() {
        let data = make_synthetic_dted(3, 4);
        let metadata = make_test_metadata();
        let provider = DTEDImageAssetProvider::new(Arc::from(data.as_slice()), 3, 4, metadata);

        let (pixels, shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, [1, 4, 3]);
        assert_eq!(pixels.len(), 4 * 3 * 2); // 4 rows × 3 cols × 2 bytes

        // Read back as i16 native-endian
        let values: Vec<i16> = pixels
            .chunks_exact(2)
            .map(|c| i16::from_ne_bytes([c[0], c[1]]))
            .collect();

        // Source data (south→north per column):
        //   col 0: [100, 200, 300, 400]
        //   col 1: [500, 600, 700, 800]
        //   col 2: [900, 1000, 1100, 1200]
        //
        // After transpose to row-major (north→south = reversed):
        //   row 0 (north): col0=400, col1=800, col2=1200
        //   row 1:         col0=300, col1=700, col2=1100
        //   row 2:         col0=200, col1=600, col2=1000
        //   row 3 (south): col0=100, col1=500, col2=900
        assert_eq!(
            values,
            vec![400, 800, 1200, 300, 700, 1100, 200, 600, 1000, 100, 500, 900]
        );
    }

    #[test]
    fn test_image_provider_has_block() {
        let data = make_synthetic_dted(3, 4);
        let metadata = make_test_metadata();
        let provider = DTEDImageAssetProvider::new(Arc::from(data.as_slice()), 3, 4, metadata);

        assert!(provider.has_block(0, 0, 0).unwrap());
        assert!(!provider.has_block(1, 0, 0).unwrap());
        assert!(!provider.has_block(0, 1, 0).unwrap());
        assert!(!provider.has_block(0, 0, 1).unwrap());
    }

    #[test]
    fn test_image_provider_invalid_block() {
        let data = make_synthetic_dted(3, 4);
        let metadata = make_test_metadata();
        let provider = DTEDImageAssetProvider::new(Arc::from(data.as_slice()), 3, 4, metadata);

        assert!(matches!(
            provider.get_block(1, 0, 0, None),
            Err(CodecError::InvalidBlockCoordinates(1, 0, 0))
        ));
        assert!(matches!(
            provider.get_block(0, 0, 1, None),
            Err(CodecError::InvalidResolutionLevel(1))
        ));
    }

    #[test]
    fn test_image_provider_asset_metadata() {
        let data = make_synthetic_dted(3, 4);
        let metadata = make_test_metadata();
        let provider = DTEDImageAssetProvider::new(Arc::from(data.as_slice()), 3, 4, metadata);

        assert_eq!(provider.key(), "elevation");
        assert_eq!(provider.roles(), &["data", "elevation"]);
        assert_eq!(provider.media_type(), "application/octet-stream");
        assert!(provider.raw_asset().is_err());
    }
}
