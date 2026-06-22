//! DTEDDatasetReader — implements DatasetReader for DTED files.
//!
//! Opens a DTED file from a byte slice, validates the UHL/DSI/ACC record
//! sentinels, and exposes a single image asset keyed as `"elevation"`.

use std::sync::Arc;

use crate::dted::image::DTEDImageAssetProvider;
use crate::dted::metadata::DTEDMetadataProvider;
use crate::dted::records::{record_size, Acc, Dsi, Uhl, DATA_OFFSET};
use crate::error::CodecError;
use crate::owned_buffer::OwnedBuffer;
use crate::traits::asset::{AssetMetadata, AssetProvider};
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::traits::reader::DatasetReader;
use crate::types::AssetType;

/// DTED dataset reader implementing the `DatasetReader` trait.
///
/// Owns the file data, parsed header records, and the image asset provider.
/// The full elevation grid is decoded on demand when `get_block()` is called.
pub struct DTEDDatasetReader {
    image_asset: Option<Arc<DTEDImageAssetProvider>>,
    metadata: Arc<DTEDMetadataProvider>,
}

impl std::fmt::Debug for DTEDDatasetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DTEDDatasetReader")
            .field("has_image", &self.image_asset.is_some())
            .finish()
    }
}

impl DTEDDatasetReader {
    /// Construct from an `OwnedBuffer`.
    ///
    /// Validates the UHL, DSI, and ACC record sentinels, parses header
    /// metadata, and verifies the file length is consistent with the
    /// declared grid dimensions.
    pub fn from_buffer(buffer: OwnedBuffer) -> Result<Self, CodecError> {
        let data = buffer.as_bytes();

        if data.len() < DATA_OFFSET {
            return Err(CodecError::InvalidFormat(
                "DTED file too short: must be at least 3428 bytes for UHL+DSI+ACC".to_string(),
            ));
        }

        let uhl = Uhl::parse(data)?;
        let dsi = Dsi::parse(data)?;
        let acc = Acc::parse(data)?;

        let rec_size = record_size(uhl.num_lat_points);
        let expected_size = DATA_OFFSET + (uhl.num_lon_lines as usize) * rec_size;

        if data.len() < expected_size {
            return Err(CodecError::InvalidFormat(format!(
                "DTED file too short: expected {} bytes ({} records × {} bytes + {} header), got {}",
                expected_size, uhl.num_lon_lines, rec_size, DATA_OFFSET, data.len()
            )));
        }

        let raw_header = &data[..DATA_OFFSET];
        let metadata = Arc::new(DTEDMetadataProvider::new(&uhl, &dsi, &acc, raw_header));

        let image_asset = Arc::new(DTEDImageAssetProvider::new(
            buffer,
            uhl.num_lon_lines,
            uhl.num_lat_points,
            metadata.clone(),
        ));

        Ok(Self {
            image_asset: Some(image_asset),
            metadata,
        })
    }
}

impl DatasetReader for DTEDDatasetReader {
    fn get_asset(&self, key: &str) -> Result<AssetProvider, CodecError> {
        match &self.image_asset {
            Some(asset) if asset.key() == key => Ok(AssetProvider::Image(
                asset.clone() as Arc<dyn ImageAssetProvider>
            )),
            _ => Err(CodecError::AssetNotFound(key.to_string())),
        }
    }

    fn get_asset_keys(
        &self,
        asset_type: Option<AssetType>,
        roles: Option<&[String]>,
    ) -> Vec<String> {
        match asset_type {
            None | Some(AssetType::Image) => match &self.image_asset {
                Some(asset) => {
                    if let Some(requested) = roles {
                        let asset_roles = asset.roles();
                        if requested.iter().any(|r| asset_roles.contains(r)) {
                            vec!["elevation".to_string()]
                        } else {
                            Vec::new()
                        }
                    } else {
                        vec!["elevation".to_string()]
                    }
                }
                None => Vec::new(),
            },
            Some(AssetType::Text) | Some(AssetType::Graphics) | Some(AssetType::Data) => Vec::new(),
        }
    }

    fn has_asset(&self, key: &str) -> bool {
        match &self.image_asset {
            Some(asset) => asset.key() == key,
            None => false,
        }
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn close(&mut self) -> Result<(), CodecError> {
        self.image_asset = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dted::records::record_size;

    /// Build a minimal valid DTED file for testing.
    fn make_valid_dted(num_lon_lines: u16, num_lat_points: u16) -> Vec<u8> {
        let rec_size = record_size(num_lat_points);
        let total_size = DATA_OFFSET + (num_lon_lines as usize) * rec_size;
        let mut data = vec![0x20u8; total_size];

        // UHL
        data[0..3].copy_from_slice(b"UHL");
        data[3] = b'1';
        data[4..12].copy_from_slice(b"1090000W");
        data[12..20].copy_from_slice(b"380000N ");
        data[20..24].copy_from_slice(b"0030");
        data[24..28].copy_from_slice(b"0030");
        data[28..32].copy_from_slice(b"0020");
        data[32] = b'U';
        let lon_str = format!("{:>4}", num_lon_lines);
        let lat_str = format!("{:>4}", num_lat_points);
        data[47..51].copy_from_slice(lon_str.as_bytes());
        data[51..55].copy_from_slice(lat_str.as_bytes());
        data[55] = b'0';

        // DSI
        data[80..83].copy_from_slice(b"DSI");
        data[83] = b'U';
        data[139..144].copy_from_slice(b"DTED1");
        data[167..169].copy_from_slice(b"01");
        data[173..177].copy_from_slice(b"0101");
        data[145..147].copy_from_slice(b"US");
        data[221..224].copy_from_slice(b"MSL");
        data[224..229].copy_from_slice(b"WGS84");
        data[369..371].copy_from_slice(b"00");

        // ACC
        let acc_start = 80 + 648;
        data[acc_start..acc_start + 3].copy_from_slice(b"ACC");
        data[acc_start + 3..acc_start + 7].copy_from_slice(b"0050");
        data[acc_start + 7..acc_start + 11].copy_from_slice(b"0030");
        data[acc_start + 11..acc_start + 15].copy_from_slice(b"0020");

        // Data records
        for col in 0..num_lon_lines as usize {
            let offset = DATA_OFFSET + col * rec_size;
            data[offset] = 0xAA;
            data[offset + 1] = 0;
            data[offset + 2] = 0;
            data[offset + 3] = col as u8;
            data[offset + 4] = 0;
            data[offset + 5] = col as u8;
            data[offset + 6] = 0;
            data[offset + 7] = 0;

            // Write zero elevations
            for post in 0..num_lat_points as usize {
                let elev_offset = offset + 8 + post * 2;
                data[elev_offset] = 0;
                data[elev_offset + 1] = 0;
            }

            // Compute and write checksum
            let payload = &data[offset..offset + rec_size - 4];
            let checksum: u32 = payload.iter().map(|&b| b as u32).sum();
            let cs_offset = offset + rec_size - 4;
            data[cs_offset..cs_offset + 4].copy_from_slice(&checksum.to_be_bytes());
        }

        data
    }

    #[test]
    fn test_from_bytes_valid() {
        let data = make_valid_dted(3, 4);
        let reader = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(reader.has_asset("elevation"));
        assert!(!reader.has_asset("nonexistent"));
    }

    #[test]
    fn test_from_bytes_too_short() {
        let data = vec![0u8; 100];
        let result = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data));
        assert!(matches!(result, Err(CodecError::InvalidFormat(_))));
    }

    #[test]
    fn test_from_bytes_invalid_sentinel() {
        let data = vec![0x20u8; DATA_OFFSET + 100];
        // No valid sentinels
        let result = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data));
        assert!(result.is_err());

        // Valid UHL but invalid DSI
        let mut data2 = vec![0x20u8; DATA_OFFSET + 100];
        data2[0..3].copy_from_slice(b"UHL");
        data2[4..12].copy_from_slice(b"0000000E");
        data2[12..20].copy_from_slice(b"000000N ");
        data2[20..24].copy_from_slice(b"0030");
        data2[24..28].copy_from_slice(b"0030");
        data2[28..32].copy_from_slice(b"0020");
        data2[32] = b'U';
        data2[47..51].copy_from_slice(b"   3");
        data2[51..55].copy_from_slice(b"   4");
        data2[55] = b'0';
        let result = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data2));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_asset_keys() {
        let data = make_valid_dted(3, 4);
        let reader = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();

        let keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(keys, vec!["elevation"]);

        let keys = reader.get_asset_keys(None, None);
        assert_eq!(keys, vec!["elevation"]);

        let keys = reader.get_asset_keys(Some(AssetType::Text), None);
        assert!(keys.is_empty());

        let keys = reader.get_asset_keys(Some(AssetType::Image), Some(&["data".to_string()]));
        assert_eq!(keys, vec!["elevation"]);

        let keys = reader.get_asset_keys(Some(AssetType::Image), Some(&["thumbnail".to_string()]));
        assert!(keys.is_empty());
    }

    #[test]
    fn test_get_asset_valid() {
        let data = make_valid_dted(3, 4);
        let reader = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let asset = reader.get_asset("elevation").unwrap();
        assert_eq!(asset.key(), "elevation");
        assert_eq!(asset.asset_type(), AssetType::Image);
    }

    #[test]
    fn test_get_asset_not_found() {
        let data = make_valid_dted(3, 4);
        let reader = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let result = reader.get_asset("nonexistent");
        assert!(matches!(result, Err(CodecError::AssetNotFound(_))));
    }

    #[test]
    fn test_metadata() {
        let data = make_valid_dted(3, 4);
        let reader = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        let meta = reader.metadata();
        let dict = meta.entries(None);
        assert!(dict.contains_key("dted:origin_longitude"));
        assert!(dict.contains_key("dted:level"));
    }

    #[test]
    fn test_close() {
        let data = make_valid_dted(3, 4);
        let mut reader = DTEDDatasetReader::from_buffer(OwnedBuffer::from_vec(data)).unwrap();
        assert!(reader.has_asset("elevation"));
        reader.close().unwrap();
        assert!(!reader.has_asset("elevation"));
        assert!(reader.get_asset("elevation").is_err());
    }
}
