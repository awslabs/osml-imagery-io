//! In-memory text asset provider for creating text segments programmatically.
//!
//! This module provides [`BufferedTextAssetProvider`] which implements the
//! [`TextAssetProvider`] trait for creating text segments in memory.
//! It allows setting text content and encoding programmatically.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::text::encode_with_crlf;
use crate::traits::{AssetMetadata, MetadataProvider, TextAssetProvider};

/// Empty metadata provider for BufferedTextAssetProvider.
#[derive(Default)]
struct EmptyMetadataProvider {
    empty_bytes: Vec<u8>,
}


impl MetadataProvider for EmptyMetadataProvider {
    fn as_dict(&self, _prefix: Option<&str>) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    fn raw(&self) -> &[u8] {
        &self.empty_bytes
    }
}

/// In-memory text asset provider for creating text segments programmatically.
///
/// This provider stores text content in memory and allows setting text data
/// programmatically. It's useful for creating text segments for NITF files.
///
/// # Example
///
/// ```ignore
/// use osml_imagery_io::buffered::BufferedTextAssetProvider;
///
/// let provider = BufferedTextAssetProvider::new(
///     "text_0",
///     "Hello, World!".to_string(),
///     "UTF-8",
/// )
/// .with_title("Sample Text".to_string())
/// .with_description("A sample text segment".to_string());
///
/// // Access text content
/// let text = provider.text().unwrap();
/// assert_eq!(text, "Hello, World!");
///
/// // Access encoding
/// assert_eq!(provider.encoding(), "UTF-8");
///
/// // Access format code
/// assert_eq!(provider.format(), "U8S");
/// ```
///
/// # Requirements
///
/// - 7.1: Implements TextAssetProvider trait
/// - 7.2: Accepts text content as String during construction
/// - 7.3: Accepts encoding parameter during construction
pub struct BufferedTextAssetProvider {
    /// Unique key identifying this asset
    key: String,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles
    roles: Vec<String>,
    /// Text content
    text_content: String,
    /// Character encoding (ASCII, UTF-8, ECS, MTF)
    encoding: String,
    /// Metadata provider
    metadata: Arc<dyn MetadataProvider>,
    /// Cached media type string
    media_type: String,
}

impl BufferedTextAssetProvider {
    /// Create a new buffered text asset provider.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for this asset
    /// * `text_content` - The text content as a String
    /// * `encoding` - Character encoding ("ASCII", "UTF-8", "ECS", "MTF")
    ///
    /// # Requirements
    /// - 7.2: Accepts text content as String during construction
    /// - 7.3: Accepts encoding parameter during construction
    pub fn new(key: impl Into<String>, text_content: String, encoding: impl Into<String>) -> Self {
        let key = key.into();
        let encoding = encoding.into();
        let media_type = Self::compute_media_type(&encoding);

        Self {
            key: key.clone(),
            title: format!("Text Segment {}", key),
            description: format!("Buffered text segment with {} encoding", encoding),
            roles: vec!["data".to_string()],
            text_content,
            encoding,
            metadata: Arc::new(EmptyMetadataProvider::default()),
            media_type,
        }
    }

    /// Set a custom title for the text asset.
    ///
    /// # Arguments
    /// * `title` - Human-readable title
    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    /// Set a custom description for the text asset.
    ///
    /// # Arguments
    /// * `description` - Detailed description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// Set custom roles for the text asset.
    ///
    /// # Arguments
    /// * `roles` - Semantic roles for this asset
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Set a custom metadata provider for the text asset.
    ///
    /// # Arguments
    /// * `metadata` - The metadata provider to attach
    pub fn with_metadata(mut self, metadata: Arc<dyn MetadataProvider>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Compute the media type based on encoding.
    fn compute_media_type(encoding: &str) -> String {
        match encoding {
            "ASCII" => "text/plain; charset=us-ascii".to_string(),
            "UTF-8" => "text/plain; charset=utf-8".to_string(),
            "ECS" => "text/plain; charset=iso-8859-1".to_string(),
            "MTF" => "text/plain".to_string(),
            _ => "text/plain".to_string(),
        }
    }
}

impl AssetMetadata for BufferedTextAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn description(&self) -> &str {
        &self.description
    }

    /// Returns the MIME type with charset parameter.
    ///
    /// # Requirements
    /// - 7.4: ASCII encoding returns "text/plain; charset=us-ascii"
    /// - 7.5: UTF-8 encoding returns "text/plain; charset=utf-8"
    fn media_type(&self) -> &str {
        &self.media_type
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    /// Returns the raw text bytes with CR/LF line delimiters.
    ///
    /// # Requirements
    /// - 7.4: ASCII encoding returns bytes with CR/LF line delimiters
    /// - 7.5: UTF-8 encoding returns UTF-8 bytes with CR/LF line delimiters
    /// - 7.6: Platform-native line endings converted to CR/LF
    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        encode_with_crlf(&self.text_content, &self.encoding)
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }
}

impl TextAssetProvider for BufferedTextAssetProvider {
    /// Returns the stored text content.
    ///
    /// # Requirements
    /// - 7.1: Implements TextAssetProvider trait
    fn text(&self) -> Result<String, CodecError> {
        Ok(self.text_content.clone())
    }

    /// Returns the character encoding.
    ///
    /// # Requirements
    /// - 7.1: Implements TextAssetProvider trait
    fn encoding(&self) -> &str {
        &self.encoding
    }

    /// Returns the TXTFMT code based on encoding.
    ///
    /// # Requirements
    /// - 7.1: Implements TextAssetProvider trait
    fn format(&self) -> &str {
        match self.encoding.as_str() {
            "ASCII" => "STA",
            "UTF-8" => "U8S",
            "ECS" => "UT1",
            "MTF" => "MTF",
            _ => "STA", // Default to STA for unknown encodings
        }
    }
}

// Ensure BufferedTextAssetProvider is Send + Sync
unsafe impl Send for BufferedTextAssetProvider {}
unsafe impl Sync for BufferedTextAssetProvider {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffered::BufferedMetadataProvider;

    #[test]
    fn test_new_creates_provider() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8");

        assert_eq!(provider.key(), "text_0");
        assert_eq!(provider.text().unwrap(), "Hello");
        assert_eq!(provider.encoding(), "UTF-8");
        assert_eq!(provider.format(), "U8S");
    }

    #[test]
    fn test_with_title() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8")
            .with_title("My Title".to_string());

        assert_eq!(provider.title(), "My Title");
    }

    #[test]
    fn test_with_description() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8")
            .with_description("My Description".to_string());

        assert_eq!(provider.description(), "My Description");
    }

    #[test]
    fn test_with_roles() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8")
            .with_roles(vec!["annotation".to_string(), "metadata".to_string()]);

        assert_eq!(provider.roles(), &["annotation", "metadata"]);
    }

    #[test]
    fn test_with_metadata() {
        let metadata = BufferedMetadataProvider::new();
        metadata.set("custom_key", "custom_value");

        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8")
            .with_metadata(Arc::new(metadata));

        let dict = provider.metadata().as_dict(None);
        assert_eq!(
            dict.get("custom_key"),
            Some(&serde_json::json!("custom_value"))
        );
    }

    #[test]
    fn test_encoding_ascii() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "ASCII");

        assert_eq!(provider.encoding(), "ASCII");
        assert_eq!(provider.format(), "STA");
        assert_eq!(provider.media_type(), "text/plain; charset=us-ascii");
    }

    #[test]
    fn test_encoding_utf8() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8");

        assert_eq!(provider.encoding(), "UTF-8");
        assert_eq!(provider.format(), "U8S");
        assert_eq!(provider.media_type(), "text/plain; charset=utf-8");
    }

    #[test]
    fn test_encoding_ecs() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "ECS");

        assert_eq!(provider.encoding(), "ECS");
        assert_eq!(provider.format(), "UT1");
        assert_eq!(provider.media_type(), "text/plain; charset=iso-8859-1");
    }

    #[test]
    fn test_encoding_mtf() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "MTF");

        assert_eq!(provider.encoding(), "MTF");
        assert_eq!(provider.format(), "MTF");
        assert_eq!(provider.media_type(), "text/plain");
    }

    #[test]
    fn test_raw_asset_converts_line_endings() {
        // Text with Unix line endings
        let provider =
            BufferedTextAssetProvider::new("text_0", "Line1\nLine2\nLine3".to_string(), "ASCII");

        let raw = provider.raw_asset().unwrap();
        let raw_str = String::from_utf8(raw).unwrap();

        // Should have CR/LF line endings
        assert_eq!(raw_str, "Line1\r\nLine2\r\nLine3");
    }

    #[test]
    fn test_raw_asset_preserves_crlf() {
        // Text already with CR/LF line endings
        let provider = BufferedTextAssetProvider::new(
            "text_0",
            "Line1\r\nLine2\r\nLine3".to_string(),
            "ASCII",
        );

        let raw = provider.raw_asset().unwrap();
        let raw_str = String::from_utf8(raw).unwrap();

        // Should still have CR/LF line endings
        assert_eq!(raw_str, "Line1\r\nLine2\r\nLine3");
    }

    #[test]
    fn test_raw_asset_utf8_encoding() {
        // UTF-8 text with special characters
        let provider =
            BufferedTextAssetProvider::new("text_0", "Héllo Wörld".to_string(), "UTF-8");

        let raw = provider.raw_asset().unwrap();
        let raw_str = String::from_utf8(raw).unwrap();

        assert_eq!(raw_str, "Héllo Wörld");
    }

    #[test]
    fn test_raw_asset_ascii_error_on_non_ascii() {
        // ASCII encoding with non-ASCII characters should fail
        let provider =
            BufferedTextAssetProvider::new("text_0", "Héllo Wörld".to_string(), "ASCII");

        let result = provider.raw_asset();
        assert!(result.is_err());
    }

    #[test]
    fn test_default_metadata_is_empty() {
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8");

        let dict = provider.metadata().as_dict(None);
        assert!(dict.is_empty());
    }

    #[test]
    fn test_asset_type_is_text() {
        // asset_type() is now on the AssetProvider enum, not on concrete types.
        // This is validated by Property 1 (variant-type consistency) in the enum tests.
        let provider = BufferedTextAssetProvider::new("text_0", "Hello".to_string(), "UTF-8");
        assert_eq!(provider.key(), "text_0");
    }
}
