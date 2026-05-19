//! Composite dataset writer that routes assets to multiple writers.
//!
//! `CompositeDatasetWriter` wraps a base `DatasetWriter` and per-level R-set
//! writers. Overview assets (keyed as `image:N:overview:M`) are routed to the
//! matching R-set writer with the key re-mapped to `image:N`. Non-overview
//! assets are forwarded to the base writer unchanged.

use std::collections::HashSet;
use std::sync::Arc;

use crate::error::CodecError;
use crate::traits::{AssetProvider, DatasetWriter, MetadataProvider};

/// Parse an overview key like `image:N:overview:M` into `(parent_key, level)`.
///
/// Uses plain string parsing with `rsplit_once(":overview:")` — no regex needed.
/// Returns `None` if the key doesn't match the overview pattern.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(parse_overview_key("image:0:overview:1"), Some(("image:0", 1)));
/// assert_eq!(parse_overview_key("image:0"), None);
/// assert_eq!(parse_overview_key("text:0:overview:1"), None);
/// ```
fn parse_overview_key(key: &str) -> Option<(&str, u32)> {
    let (parent, suffix) = key.rsplit_once(":overview:")?;
    if parent.starts_with("image:") {
        let level = suffix.parse::<u32>().ok()?;
        Some((parent, level))
    } else {
        None
    }
}

/// A dataset writer that routes assets across a base writer and per-level R-set writers.
///
/// This mirrors `CompositeDatasetReader` on the write side. Overview assets are
/// dispatched to the appropriate R-set writer based on the overview level in the
/// asset key. Non-overview assets go to the base writer.
pub struct CompositeDatasetWriter {
    /// Base writer for non-overview assets (first path)
    base_writer: Box<dyn DatasetWriter>,
    /// Per-level R-set writers, sorted by ascending level
    rset_writers: Vec<(u32, Box<dyn DatasetWriter>)>,
    /// Dataset-level metadata, forwarded to all inner writers
    metadata: Option<Arc<dyn MetadataProvider>>,
    /// All asset keys seen so far (for duplicate detection)
    asset_keys: HashSet<String>,
    /// Whether close() has been called
    closed: bool,
}

impl CompositeDatasetWriter {
    /// Create a new composite writer from a base writer and per-level R-set writers.
    ///
    /// The R-set writers are sorted by ascending level for deterministic close ordering.
    ///
    /// # Arguments
    /// * `base_writer` - The base dataset writer (for the primary output file)
    /// * `rset_writers` - Vec of (overview_level, writer) pairs for R-set files
    pub fn new(
        base_writer: Box<dyn DatasetWriter>,
        mut rset_writers: Vec<(u32, Box<dyn DatasetWriter>)>,
    ) -> Self {
        rset_writers.sort_by_key(|(level, _)| *level);
        Self {
            base_writer,
            rset_writers,
            metadata: None,
            asset_keys: HashSet::new(),
            closed: false,
        }
    }
}

impl DatasetWriter for CompositeDatasetWriter {
    fn add_asset(
        &mut self,
        key: &str,
        provider: AssetProvider,
        title: &str,
        description: &str,
        roles: &[String],
    ) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Io(std::io::Error::other(
                "Writer has been closed",
            )));
        }

        if !self.asset_keys.insert(key.to_string()) {
            return Err(CodecError::DuplicateKey(key.to_string()));
        }

        if let Some((parent_key, level)) = parse_overview_key(key) {
            // Route to the matching rset_writer, re-keyed to parent key
            let writer = self
                .rset_writers
                .iter_mut()
                .find(|(l, _)| *l == level)
                .map(|(_, w)| w);

            match writer {
                Some(w) => w.add_asset(parent_key, provider, title, description, roles),
                None => Err(CodecError::InvalidFormat(format!(
                    "No writer registered for overview level {}",
                    level
                ))),
            }
        } else {
            // Non-overview asset goes to base writer
            self.base_writer
                .add_asset(key, provider, title, description, roles)
        }
    }

    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
        self.base_writer.set_metadata(metadata.clone())?;
        for (_, writer) in &mut self.rset_writers {
            writer.set_metadata(metadata.clone())?;
        }
        self.metadata = Some(metadata);
        Ok(())
    }

    fn close(&mut self) -> Result<(), CodecError> {
        if self.closed {
            return Ok(());
        }

        self.base_writer.close()?;
        for (_, writer) in &mut self.rset_writers {
            writer.close()?;
        }
        self.closed = true;
        Ok(())
    }
}

// SAFETY: CompositeDatasetWriter is Send + Sync because:
// - base_writer: Box<dyn DatasetWriter> is Send + Sync (trait bound on DatasetWriter)
// - rset_writers: Vec<(u32, Box<dyn DatasetWriter>)> is Send + Sync
// - metadata: Option<Arc<dyn MetadataProvider>> is Send + Sync
// - asset_keys: HashSet<String> is Send + Sync
// - closed: bool is Send + Sync
unsafe impl Send for CompositeDatasetWriter {}
unsafe impl Sync for CompositeDatasetWriter {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use crate::traits::{AssetMetadata, AssetProvider, ImageAssetProvider, MetadataProvider};

    // =========================================================================
    // Mock implementations
    // =========================================================================

    /// A mock MetadataProvider for testing set_metadata() forwarding.
    struct MockMetadataProvider;

    impl MetadataProvider for MockMetadataProvider {
        fn raw(&self) -> &[u8] {
            &[]
        }

        fn entries(&self, _name: Option<&str>) -> HashMap<String, serde_json::Value> {
            HashMap::new()
        }
    }

    /// A mock AssetProvider for testing add_asset() calls.
    struct MockAssetProvider {
        key: String,
    }

    impl MockAssetProvider {
        fn new(key: &str) -> AssetProvider {
            AssetProvider::Image(Arc::new(Self {
                key: key.to_string(),
            }))
        }
    }

    impl AssetMetadata for MockAssetProvider {
        fn key(&self) -> &str {
            &self.key
        }
        fn title(&self) -> &str {
            "mock"
        }
        fn description(&self) -> &str {
            "mock asset"
        }
        fn media_type(&self) -> &str {
            "application/octet-stream"
        }
        fn roles(&self) -> &[String] {
            &[]
        }
        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(vec![])
        }
        fn metadata(&self) -> Arc<dyn MetadataProvider> {
            Arc::new(MockMetadataProvider)
        }
    }

    impl ImageAssetProvider for MockAssetProvider {
        fn has_block(
            &self,
            _block_row: u32,
            _block_col: u32,
            _resolution_level: u32,
        ) -> Result<bool, CodecError> {
            Ok(true)
        }
        fn get_block(
            &self,
            _block_row: u32,
            _block_col: u32,
            _resolution_level: u32,
            _bands: Option<&[u32]>,
        ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
            Ok((vec![0u8; 1], [1, 1, 1]))
        }
        fn num_resolution_levels(&self) -> u32 {
            1
        }
        fn num_bands(&self) -> u32 {
            1
        }
        fn num_rows(&self) -> u32 {
            1
        }
        fn num_columns(&self) -> u32 {
            1
        }
        fn num_pixels_per_block_horizontal(&self) -> u32 {
            1
        }
        fn num_pixels_per_block_vertical(&self) -> u32 {
            1
        }
        fn num_bits_per_pixel(&self) -> u32 {
            8
        }
        fn actual_bits_per_pixel(&self) -> u32 {
            8
        }
        fn pixel_value_type(&self) -> crate::types::PixelType {
            crate::types::PixelType::UInt8
        }
        fn pad_pixel_value(&self) -> f64 {
            0.0
        }
    }

    /// A mock DatasetWriter that records all operations to a shared log.
    ///
    /// Each operation is recorded as a descriptive string in the log vec,
    /// allowing tests to verify routing, ordering, and forwarding behavior.
    struct MockDatasetWriter {
        name: String,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl MockDatasetWriter {
        fn new(name: &str, log: Arc<Mutex<Vec<String>>>) -> Box<Self> {
            Box::new(Self {
                name: name.to_string(),
                log,
            })
        }
    }

    impl DatasetWriter for MockDatasetWriter {
        fn add_asset(
            &mut self,
            key: &str,
            _provider: AssetProvider,
            _title: &str,
            _description: &str,
            _roles: &[String],
        ) -> Result<(), CodecError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("{}:add_asset({})", self.name, key));
            Ok(())
        }

        fn set_metadata(&mut self, _metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("{}:set_metadata", self.name));
            Ok(())
        }

        fn close(&mut self) -> Result<(), CodecError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("{}:close", self.name));
            Ok(())
        }
    }

    // SAFETY: MockDatasetWriter fields are Send + Sync (String, Arc<Mutex<_>>).
    unsafe impl Send for MockDatasetWriter {}
    unsafe impl Sync for MockDatasetWriter {}

    // =========================================================================
    // Helper to build a CompositeDatasetWriter with mock writers
    // =========================================================================

    /// Build a CompositeDatasetWriter with a base mock and rset mocks at the given levels.
    /// Returns the writer and the shared log for assertions.
    fn build_writer(levels: &[u32]) -> (CompositeDatasetWriter, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let base = MockDatasetWriter::new("base", log.clone());
        let rset_writers: Vec<(u32, Box<dyn DatasetWriter>)> = levels
            .iter()
            .map(|&lvl| {
                let w: Box<dyn DatasetWriter> =
                    MockDatasetWriter::new(&format!("rset_{}", lvl), log.clone());
                (lvl, w)
            })
            .collect();
        let writer = CompositeDatasetWriter::new(base, rset_writers);
        (writer, log)
    }

    // =========================================================================
    // Unit tests (Task 2.2)
    // =========================================================================

    /// Validates: Requirements 1.2
    /// Non-overview asset is forwarded to the base writer with key unchanged.
    #[test]
    fn test_routes_base_asset() {
        let (mut writer, log) = build_writer(&[1, 2]);
        let provider = MockAssetProvider::new("image:0");

        writer
            .add_asset("image:0", provider, "title", "desc", &[])
            .unwrap();

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "base:add_asset(image:0)");
    }

    /// Validates: Requirements 1.1, 1.3
    /// Overview asset is routed to the correct rset writer with re-keyed key.
    #[test]
    fn test_routes_overview_asset() {
        let (mut writer, log) = build_writer(&[1, 2]);
        let provider = MockAssetProvider::new("image:0:overview:1");

        writer
            .add_asset("image:0:overview:1", provider, "title", "desc", &[])
            .unwrap();

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "rset_1:add_asset(image:0)");
    }

    /// Validates: Requirements 1.4
    /// Adding an overview asset for an unregistered level returns InvalidFormat.
    #[test]
    fn test_rejects_unregistered_level() {
        let (mut writer, _log) = build_writer(&[1, 2]);
        let provider = MockAssetProvider::new("image:0:overview:3");

        let result = writer.add_asset("image:0:overview:3", provider, "title", "desc", &[]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            CodecError::InvalidFormat(msg) => {
                assert!(
                    msg.contains("3"),
                    "Error message should mention level 3: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    /// Validates: Requirements 1.5
    /// Adding the same key twice returns DuplicateKey on the second call.
    #[test]
    fn test_rejects_duplicate_key() {
        let (mut writer, _log) = build_writer(&[1]);
        let provider1 = MockAssetProvider::new("image:0");
        let provider2 = MockAssetProvider::new("image:0");

        writer
            .add_asset("image:0", provider1, "title", "desc", &[])
            .unwrap();

        let result = writer.add_asset("image:0", provider2, "title", "desc", &[]);

        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::DuplicateKey(key) => assert_eq!(key, "image:0"),
            other => panic!("Expected DuplicateKey, got: {:?}", other),
        }
    }

    /// Validates: Requirements 3.1, 3.2 (design: close after close returns Ok)
    /// Adding an asset after close returns an error.
    #[test]
    fn test_rejects_add_after_close() {
        let (mut writer, _log) = build_writer(&[1]);

        writer.close().unwrap();

        let provider = MockAssetProvider::new("image:0");
        let result = writer.add_asset("image:0", provider, "title", "desc", &[]);

        assert!(result.is_err());
    }

    /// Validates: Design decision — close() is idempotent.
    /// Calling close() twice returns Ok(()) both times.
    #[test]
    fn test_close_is_idempotent() {
        let (mut writer, _log) = build_writer(&[1]);

        assert!(writer.close().is_ok());
        assert!(writer.close().is_ok());
    }

    /// Validates: Requirements 2.1, 2.2
    /// set_metadata() forwards to the base writer and all rset writers.
    #[test]
    fn test_metadata_forwarding() {
        let (mut writer, log) = build_writer(&[1, 2]);
        let metadata: Arc<dyn MetadataProvider> = Arc::new(MockMetadataProvider);

        writer.set_metadata(metadata).unwrap();

        let entries = log.lock().unwrap();
        assert_eq!(
            entries.len(),
            3,
            "Expected 3 set_metadata calls (base + 2 rset)"
        );
        assert_eq!(entries[0], "base:set_metadata");
        // rset writers are sorted by ascending level, so rset_1 then rset_2
        assert!(entries.contains(&"rset_1:set_metadata".to_string()));
        assert!(entries.contains(&"rset_2:set_metadata".to_string()));
    }

    /// Validates: Requirements 3.1, 3.2, 9.2
    /// Constructed with levels [3, 1, 2], close order is base → 1 → 2 → 3.
    #[test]
    fn test_close_ordering() {
        let (mut writer, log) = build_writer(&[3, 1, 2]);

        writer.close().unwrap();

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 4, "Expected 4 close calls (base + 3 rset)");
        assert_eq!(entries[0], "base:close");
        assert_eq!(entries[1], "rset_1:close");
        assert_eq!(entries[2], "rset_2:close");
        assert_eq!(entries[3], "rset_3:close");
    }
}
