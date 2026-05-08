//! DTEDMetadataProvider — implements MetadataProvider for DTED UHL/DSI/ACC fields.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::dted::records::{Acc, Dsi, Uhl};
use crate::traits::metadata::MetadataProvider;

/// Metadata provider for DTED header fields.
///
/// Exposes UHL, DSI, and ACC fields as key-value pairs under the `dted:`
/// namespace prefix.
pub struct DTEDMetadataProvider {
    entries: HashMap<String, Value>,
    raw_bytes: Vec<u8>,
}

impl DTEDMetadataProvider {
    pub fn new(uhl: &Uhl, dsi: &Dsi, acc: &Acc, raw_header: &[u8]) -> Self {
        let mut entries = HashMap::new();

        // UHL fields
        entries.insert("dted:origin_longitude".to_string(), json!(uhl.origin_lon));
        entries.insert("dted:origin_latitude".to_string(), json!(uhl.origin_lat));
        entries.insert(
            "dted:longitude_interval".to_string(),
            json!(uhl.lon_interval_tenths),
        );
        entries.insert(
            "dted:latitude_interval".to_string(),
            json!(uhl.lat_interval_tenths),
        );
        entries.insert(
            "dted:num_longitude_lines".to_string(),
            json!(uhl.num_lon_lines),
        );
        entries.insert(
            "dted:num_latitude_points".to_string(),
            json!(uhl.num_lat_points),
        );
        if let Some(va) = uhl.vertical_accuracy {
            entries.insert("dted:vertical_accuracy".to_string(), json!(va));
        }
        entries.insert(
            "dted:security_code".to_string(),
            json!(uhl.security_code.to_string()),
        );
        entries.insert(
            "dted:multiple_accuracy".to_string(),
            json!(uhl.multiple_accuracy),
        );

        // DSI fields
        entries.insert("dted:level".to_string(), json!(dsi.product_level));
        entries.insert("dted:edition_number".to_string(), json!(dsi.edition_number));
        entries.insert(
            "dted:compilation_date".to_string(),
            json!(dsi.compilation_date),
        );
        entries.insert("dted:producer_code".to_string(), json!(dsi.producer_code));
        entries.insert("dted:vertical_datum".to_string(), json!(dsi.vertical_datum));
        entries.insert(
            "dted:horizontal_datum".to_string(),
            json!(dsi.horizontal_datum),
        );
        entries.insert(
            "dted:partial_cell_indicator".to_string(),
            json!(dsi.partial_cell_indicator),
        );

        // ACC fields
        entries.insert(
            "dted:absolute_horizontal_accuracy".to_string(),
            json!(acc.absolute_horizontal_accuracy),
        );
        entries.insert(
            "dted:absolute_vertical_accuracy".to_string(),
            json!(acc.absolute_vertical_accuracy),
        );
        entries.insert(
            "dted:relative_vertical_accuracy".to_string(),
            json!(acc.relative_vertical_accuracy),
        );

        Self {
            entries,
            raw_bytes: raw_header.to_vec(),
        }
    }
}

impl MetadataProvider for DTEDMetadataProvider {
    fn raw(&self) -> &[u8] {
        &self.raw_bytes
    }

    fn as_dict(&self, name: Option<&str>) -> HashMap<String, Value> {
        match name {
            None => self.entries.clone(),
            Some(prefix) => self
                .entries
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_uhl() -> Uhl {
        Uhl {
            origin_lon: -109.0,
            origin_lat: 38.0,
            lon_interval_tenths: 30,
            lat_interval_tenths: 30,
            num_lon_lines: 1201,
            num_lat_points: 1201,
            vertical_accuracy: Some(20),
            security_code: 'U',
            multiple_accuracy: false,
        }
    }

    fn sample_dsi() -> Dsi {
        Dsi {
            security_code: "U".to_string(),
            product_level: "DTED1".to_string(),
            edition_number: "02".to_string(),
            compilation_date: "0502".to_string(),
            producer_code: "US".to_string(),
            vertical_datum: "MSL".to_string(),
            horizontal_datum: "WGS84".to_string(),
            partial_cell_indicator: "00".to_string(),
        }
    }

    fn sample_acc() -> Acc {
        Acc {
            absolute_horizontal_accuracy: "0050".to_string(),
            absolute_vertical_accuracy: "0030".to_string(),
            relative_vertical_accuracy: "0020".to_string(),
        }
    }

    #[test]
    fn test_metadata_keys_present() {
        let provider = DTEDMetadataProvider::new(&sample_uhl(), &sample_dsi(), &sample_acc(), &[]);
        let dict = provider.as_dict(None);

        assert_eq!(
            dict.get("dted:origin_longitude").and_then(|v| v.as_f64()),
            Some(-109.0)
        );
        assert_eq!(
            dict.get("dted:origin_latitude").and_then(|v| v.as_f64()),
            Some(38.0)
        );
        assert_eq!(
            dict.get("dted:num_longitude_lines")
                .and_then(|v| v.as_u64()),
            Some(1201)
        );
        assert_eq!(
            dict.get("dted:level").and_then(|v| v.as_str()),
            Some("DTED1")
        );
        assert_eq!(
            dict.get("dted:horizontal_datum").and_then(|v| v.as_str()),
            Some("WGS84")
        );
        assert_eq!(
            dict.get("dted:absolute_vertical_accuracy")
                .and_then(|v| v.as_str()),
            Some("0030")
        );
    }

    #[test]
    fn test_metadata_prefix_filter() {
        let provider = DTEDMetadataProvider::new(&sample_uhl(), &sample_dsi(), &sample_acc(), &[]);
        let filtered = provider.as_dict(Some("dted:origin"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("dted:origin_longitude"));
        assert!(filtered.contains_key("dted:origin_latitude"));
    }

    #[test]
    fn test_raw_returns_header_bytes() {
        let header = vec![1, 2, 3, 4, 5];
        let provider =
            DTEDMetadataProvider::new(&sample_uhl(), &sample_dsi(), &sample_acc(), &header);
        assert_eq!(provider.raw(), &[1, 2, 3, 4, 5]);
    }
}
