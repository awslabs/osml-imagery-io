use std::collections::HashMap;
use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::{AssetMetadata, DataAssetProvider, MetadataProvider};

#[derive(Default)]
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
}

impl MetadataProvider for EmptyMetadataProvider {
    fn entries(&self, _prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    fn raw(&self) -> &[u8] {
        &self.empty_bytes
    }
}

/// In-memory data asset provider for creating DES segments programmatically.
///
/// This provider stores arbitrary bytes in memory with an associated MIME type,
/// supporting XML and JSON parsing for structured payloads.
pub struct BufferedDataAssetProvider {
    key: String,
    title: String,
    description: String,
    roles: Vec<String>,
    data: Vec<u8>,
    mime_type: String,
    metadata: Arc<dyn MetadataProvider>,
}

impl BufferedDataAssetProvider {
    pub fn new(key: &str, data: Vec<u8>, mime_type: &str) -> Self {
        Self {
            key: key.to_string(),
            title: format!("Data Segment {}", key),
            description: format!("Buffered data segment with {} content", mime_type),
            roles: vec!["data".to_string()],
            data,
            mime_type: mime_type.to_string(),
            metadata: Arc::new(EmptyMetadataProvider::default()),
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    pub fn with_metadata(mut self, metadata: Arc<dyn MetadataProvider>) -> Self {
        self.metadata = metadata;
        self
    }
}

impl AssetMetadata for BufferedDataAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn media_type(&self) -> &str {
        &self.mime_type
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Ok(self.data.clone())
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }
}

impl DataAssetProvider for BufferedDataAssetProvider {
    fn mime_type(&self) -> &str {
        &self.mime_type
    }

    fn parse_as_xml(&self) -> Result<String, CodecError> {
        String::from_utf8(self.data.clone())
            .map_err(|e| CodecError::Parse(format!("Data is not valid UTF-8 for XML: {}", e)))
    }

    fn parse_as_json(&self) -> Result<serde_json::Value, CodecError> {
        serde_json::from_slice(&self.data)
            .map_err(|e| CodecError::Parse(format!("Data is not valid JSON: {}", e)))
    }
}

unsafe impl Send for BufferedDataAssetProvider {}
unsafe impl Sync for BufferedDataAssetProvider {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffered::BufferedMetadataProvider;

    #[test]
    fn test_new_creates_provider() {
        let provider =
            BufferedDataAssetProvider::new("des_0", vec![1, 2, 3], "application/octet-stream");

        assert_eq!(provider.key(), "des_0");
        assert_eq!(provider.mime_type(), "application/octet-stream");
        assert_eq!(provider.media_type(), "application/octet-stream");
        assert_eq!(provider.raw_asset().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_with_title() {
        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/octet-stream")
            .with_title("My DES");

        assert_eq!(provider.title(), "My DES");
    }

    #[test]
    fn test_with_description() {
        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/octet-stream")
            .with_description("A custom DES segment");

        assert_eq!(provider.description(), "A custom DES segment");
    }

    #[test]
    fn test_with_roles() {
        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/octet-stream")
            .with_roles(vec!["metadata".to_string(), "annotation".to_string()]);

        assert_eq!(provider.roles(), &["metadata", "annotation"]);
    }

    #[test]
    fn test_with_metadata() {
        let metadata = BufferedMetadataProvider::new();
        metadata.set("DESID", serde_json::json!("XML_DATA_CONTENT"));
        metadata.set("DESVER", serde_json::json!("01"));

        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/xml")
            .with_metadata(Arc::new(metadata));

        let dict = provider.metadata().entries(None);
        assert_eq!(
            dict.get("DESID"),
            Some(&serde_json::json!("XML_DATA_CONTENT"))
        );
        assert_eq!(dict.get("DESVER"), Some(&serde_json::json!("01")));
    }

    #[test]
    fn test_parse_as_xml_valid() {
        let xml = b"<root><child>value</child></root>".to_vec();
        let provider = BufferedDataAssetProvider::new("des_0", xml, "application/xml");

        let result = provider.parse_as_xml().unwrap();
        assert_eq!(result, "<root><child>value</child></root>");
    }

    #[test]
    fn test_parse_as_xml_invalid_utf8() {
        let binary = vec![0xFF, 0xFE, 0x00, 0x01];
        let provider = BufferedDataAssetProvider::new("des_0", binary, "application/octet-stream");

        assert!(provider.parse_as_xml().is_err());
    }

    #[test]
    fn test_parse_as_json_valid() {
        let json = br#"{"key": "value", "num": 42}"#.to_vec();
        let provider = BufferedDataAssetProvider::new("des_0", json, "application/json");

        let result = provider.parse_as_json().unwrap();
        assert_eq!(result["key"], "value");
        assert_eq!(result["num"], 42);
    }

    #[test]
    fn test_parse_as_json_invalid() {
        let not_json = b"this is not json".to_vec();
        let provider =
            BufferedDataAssetProvider::new("des_0", not_json, "application/octet-stream");

        assert!(provider.parse_as_json().is_err());
    }

    #[test]
    fn test_raw_asset_returns_exact_bytes() {
        let data: Vec<u8> = (0..=255).collect();
        let provider =
            BufferedDataAssetProvider::new("des_0", data.clone(), "application/octet-stream");

        assert_eq!(provider.raw_asset().unwrap(), data);
    }

    #[test]
    fn test_default_metadata_is_empty() {
        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/octet-stream");

        let dict = provider.metadata().entries(None);
        assert!(dict.is_empty());
    }

    #[test]
    fn test_default_title_and_description() {
        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/xml");

        assert_eq!(provider.title(), "Data Segment des_0");
        assert_eq!(
            provider.description(),
            "Buffered data segment with application/xml content"
        );
    }

    #[test]
    fn test_default_roles() {
        let provider = BufferedDataAssetProvider::new("des_0", vec![], "application/octet-stream");

        assert_eq!(provider.roles(), &["data"]);
    }
}
