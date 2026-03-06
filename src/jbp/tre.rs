//! TRE (Tagged Record Extension) envelope parsing and serialization.
//!
//! This module provides the [`TreEnvelope`] struct for parsing and writing TRE
//! envelopes from NITF headers. A TRE envelope consists of:
//! - CETAG: 6-character alphanumeric tag identifying the TRE type
//! - CEL: 5-digit numeric string indicating CEDATA length
//! - CEDATA: Raw extension data bytes
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::TreEnvelope;
//!
//! // Parse a single TRE envelope
//! let (envelope, consumed) = TreEnvelope::parse(data)?;
//! println!("TRE tag: {}, data length: {}", envelope.tag, envelope.data.len());
//!
//! // Parse all TRE envelopes from a byte slice
//! let envelopes = TreEnvelope::parse_all(data)?;
//! for env in envelopes {
//!     println!("Found TRE: {}", env.tag);
//! }
//!
//! // Serialize back to bytes
//! let bytes = envelope.to_bytes();
//! ```

use super::error::JBPError;

/// CETAG field size in bytes (6 characters)
const CETAG_SIZE: usize = 6;

/// CEL field size in bytes (5 digits)
const CEL_SIZE: usize = 5;

/// Minimum TRE envelope size (CETAG + CEL)
const MIN_ENVELOPE_SIZE: usize = CETAG_SIZE + CEL_SIZE;

/// Raw TRE envelope containing tag, and data.
///
/// The TRE envelope is the wrapper structure for Tagged Record Extensions in NITF files.
/// It contains:
/// - `tag`: The 6-character CETAG identifying the TRE type
/// - `data`: The raw CEDATA bytes containing the extension data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreEnvelope {
    /// 6-character TRE type tag (CETAG)
    pub tag: String,
    /// Raw extension data (CEDATA)
    pub data: Vec<u8>,
}

impl TreEnvelope {
    /// Create a new TRE envelope with the given tag and data.
    ///
    /// # Arguments
    ///
    /// * `tag` - The 6-character CETAG (will be validated)
    /// * `data` - The raw CEDATA bytes
    ///
    /// # Errors
    ///
    /// Returns `JBPError::InvalidCetag` if the tag is not valid.
    pub fn new(tag: impl Into<String>, data: Vec<u8>) -> Result<Self, JBPError> {
        let tag = tag.into();
        validate_cetag(&tag)?;
        Ok(Self { tag, data })
    }

    /// Parse a single TRE envelope from bytes.
    ///
    /// Returns the parsed envelope and the number of bytes consumed.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice containing the TRE envelope
    ///
    /// # Errors
    ///
    /// - `JBPError::UnexpectedEof` if there aren't enough bytes
    /// - `JBPError::InvalidCetag` if the CETAG is invalid
    /// - `JBPError::LengthMismatch` if CEL doesn't match available data
    pub fn parse(data: &[u8]) -> Result<(Self, usize), JBPError> {
        // Check minimum size for CETAG + CEL
        if data.len() < MIN_ENVELOPE_SIZE {
            return Err(JBPError::UnexpectedEof {
                expected: MIN_ENVELOPE_SIZE,
                available: data.len(),
            });
        }

        // Extract and validate CETAG (6 characters)
        let cetag = std::str::from_utf8(&data[..CETAG_SIZE])
            .map_err(|_| JBPError::InvalidCetag {
                tag: String::from_utf8_lossy(&data[..CETAG_SIZE]).to_string(),
            })?
            .to_string();

        validate_cetag(&cetag)?;

        // Extract and parse CEL (5 digits)
        let cel_str = std::str::from_utf8(&data[CETAG_SIZE..MIN_ENVELOPE_SIZE]).map_err(|_| {
            JBPError::InvalidFormat {
                message: "CEL field contains invalid UTF-8".to_string(),
            }
        })?;

        let cel: usize = cel_str.trim().parse().map_err(|_| JBPError::InvalidFormat {
            message: format!("CEL field '{}' is not a valid number", cel_str),
        })?;

        // Calculate total envelope size
        let total_size = MIN_ENVELOPE_SIZE + cel;

        // Check if we have enough data for CEDATA
        if data.len() < total_size {
            return Err(JBPError::UnexpectedEof {
                expected: total_size,
                available: data.len(),
            });
        }

        // Extract CEDATA
        let cedata = data[MIN_ENVELOPE_SIZE..total_size].to_vec();

        Ok((
            Self {
                tag: cetag,
                data: cedata,
            },
            total_size,
        ))
    }

    /// Parse all TRE envelopes from a byte slice.
    ///
    /// Continues parsing until all bytes are consumed.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice containing one or more TRE envelopes
    ///
    /// # Errors
    ///
    /// Returns an error if any envelope fails to parse.
    pub fn parse_all(data: &[u8]) -> Result<Vec<Self>, JBPError> {
        let mut envelopes = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            let (envelope, consumed) = Self::parse(&data[offset..])?;
            envelopes.push(envelope);
            offset += consumed;
        }

        Ok(envelopes)
    }

    /// Serialize the envelope to bytes (CETAG + CEL + CEDATA).
    ///
    /// The output format is:
    /// - CETAG: 6 characters, left-justified, space-padded
    /// - CEL: 5 digits, zero-padded
    /// - CEDATA: Raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.envelope_size());

        // Write CETAG (6 chars, left-justified, space-padded)
        let tag_bytes = self.tag.as_bytes();
        bytes.extend_from_slice(tag_bytes);
        // Pad with spaces if tag is shorter than 6 characters
        for _ in tag_bytes.len()..CETAG_SIZE {
            bytes.push(b' ');
        }

        // Write CEL (5 digits, zero-padded)
        let cel = format!("{:05}", self.data.len());
        bytes.extend_from_slice(cel.as_bytes());

        // Write CEDATA
        bytes.extend_from_slice(&self.data);

        bytes
    }

    /// Get the total envelope size in bytes (CETAG + CEL + CEDATA).
    ///
    /// This is always 11 + data.len() (6 for CETAG + 5 for CEL + CEDATA length).
    #[inline]
    pub fn envelope_size(&self) -> usize {
        MIN_ENVELOPE_SIZE + self.data.len()
    }
}

/// Write multiple TRE envelopes to bytes.
///
/// Serializes a list of TRE envelopes by concatenating all envelope bytes.
/// Each envelope is serialized as CETAG (6 chars) + CEL (5 digits) + CEDATA.
///
/// # Arguments
///
/// * `envelopes` - Slice of TRE envelopes to serialize
///
/// # Returns
///
/// A byte vector containing all serialized envelopes concatenated together.
///
/// # Example
///
/// ```ignore
/// use osml_imagery_io::jbp::{TreEnvelope, write_tre_envelopes};
///
/// let envelopes = vec![
///     TreEnvelope::new("GEOLOB", vec![1, 2, 3]).unwrap(),
///     TreEnvelope::new("TEST", vec![4, 5]).unwrap(),
/// ];
/// let bytes = write_tre_envelopes(&envelopes);
/// ```
pub fn write_tre_envelopes(envelopes: &[TreEnvelope]) -> Vec<u8> {
    // Calculate total size for pre-allocation
    let total_size: usize = envelopes.iter().map(|e| e.envelope_size()).sum();
    let mut bytes = Vec::with_capacity(total_size);

    // Concatenate all envelope bytes
    for envelope in envelopes {
        bytes.extend(envelope.to_bytes());
    }

    bytes
}

/// TRE field values grouped by CETAG.
///
/// This struct holds field values for a single TRE type, organized by field name.
/// Field names are stored without the CETAG prefix (e.g., "ARV" not "GEOLOB.ARV").
#[derive(Debug, Clone, Default)]
pub struct TreFieldGroup {
    /// The CETAG identifying this TRE type
    pub tag: String,
    /// Field values keyed by field name (without CETAG prefix)
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

impl TreFieldGroup {
    /// Create a new TRE field group for the given tag.
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            fields: std::collections::HashMap::new(),
        }
    }

    /// Add a field value to this group.
    pub fn insert(&mut self, field_name: impl Into<String>, value: serde_json::Value) {
        self.fields.insert(field_name.into(), value);
    }

    /// Get a field value by name.
    pub fn get(&self, field_name: &str) -> Option<&serde_json::Value> {
        self.fields.get(field_name)
    }

    /// Check if this group has any fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Get the number of fields in this group.
    pub fn len(&self) -> usize {
        self.fields.len()
    }
}

/// Parse TRE field values from metadata with CETAG prefix.
///
/// This function extracts TRE field values from a metadata dictionary where
/// fields are prefixed with their CETAG (e.g., "GEOLOB.ARV", "SENSRB.PLATFORM").
/// Fields are grouped by their CETAG for subsequent serialization.
///
/// # Arguments
///
/// * `metadata` - A HashMap of field names to JSON values, where TRE fields
///   are prefixed with "{CETAG}." (e.g., "GEOLOB.ARV")
///
/// # Returns
///
/// A HashMap mapping CETAG strings to their corresponding TreFieldGroup.
/// Only fields with a dot separator are considered TRE fields.
///
/// # Example
///
/// ```ignore
/// use osml_imagery_io::jbp::tre::parse_tre_fields_from_metadata;
/// use std::collections::HashMap;
/// use serde_json::json;
///
/// let mut metadata = HashMap::new();
/// metadata.insert("GEOLOB.ARV".to_string(), json!("000360000"));
/// metadata.insert("GEOLOB.BRV".to_string(), json!("000360000"));
/// metadata.insert("SENSRB.PLATFORM".to_string(), json!("AIRCRAFT"));
/// metadata.insert("IID1".to_string(), json!("TEST")); // Not a TRE field
///
/// let groups = parse_tre_fields_from_metadata(&metadata);
///
/// assert!(groups.contains_key("GEOLOB"));
/// assert!(groups.contains_key("SENSRB"));
/// assert_eq!(groups["GEOLOB"].fields.len(), 2);
/// ```
///
/// # Requirements
///
/// _Requirements: 18.6_
pub fn parse_tre_fields_from_metadata(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> std::collections::HashMap<String, TreFieldGroup> {
    let mut groups: std::collections::HashMap<String, TreFieldGroup> =
        std::collections::HashMap::new();

    for (key, value) in metadata {
        // Check if this is a TRE field (has CETAG.field format)
        if let Some(dot_pos) = key.find('.') {
            let cetag = &key[..dot_pos];
            let field_name = &key[dot_pos + 1..];

            // Skip empty CETAGs or field names
            if cetag.is_empty() || field_name.is_empty() {
                continue;
            }

            // Get or create the group for this CETAG
            let group = groups
                .entry(cetag.to_string())
                .or_insert_with(|| TreFieldGroup::new(cetag));

            // Add the field value
            group.insert(field_name.to_string(), value.clone());
        }
        // Non-TRE fields (no dot) are ignored
    }

    groups
}

/// Extract TRE field groups from a MetadataProvider.
///
/// This is a convenience function that calls `as_dict(None)` on the provider
/// and then parses the TRE fields from the resulting metadata.
///
/// # Arguments
///
/// * `provider` - A MetadataProvider implementation
///
/// # Returns
///
/// A HashMap mapping CETAG strings to their corresponding TreFieldGroup.
///
/// # Example
///
/// ```ignore
/// use osml_imagery_io::jbp::tre::extract_tre_fields_from_provider;
///
/// let groups = extract_tre_fields_from_provider(&metadata_provider);
/// for (cetag, group) in groups {
///     println!("TRE {}: {} fields", cetag, group.len());
/// }
/// ```
pub fn extract_tre_fields_from_provider(
    provider: &dyn crate::traits::MetadataProvider,
) -> std::collections::HashMap<String, TreFieldGroup> {
    let metadata = provider.as_dict(None);
    parse_tre_fields_from_metadata(&metadata)
}

/// Validate that a CETAG is in the correct format.
///
/// A valid CETAG must:
/// - Be exactly 6 characters (or fewer, which will be space-padded)
/// - Contain only alphanumeric characters (A-Z, 0-9) and spaces
///
/// # Errors
///
/// Returns `JBPError::InvalidCetag` if the tag is invalid.
fn validate_cetag(tag: &str) -> Result<(), JBPError> {
    // Check length (must be at most 6 characters)
    if tag.len() > CETAG_SIZE {
        return Err(JBPError::InvalidCetag {
            tag: tag.to_string(),
        });
    }

    // Check that all characters are alphanumeric or space
    for c in tag.chars() {
        if !c.is_ascii_alphanumeric() && c != ' ' {
            return Err(JBPError::InvalidCetag {
                tag: tag.to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tre_envelope_new_valid() {
        let envelope = TreEnvelope::new("GEOLOB", vec![1, 2, 3]).unwrap();
        assert_eq!(envelope.tag, "GEOLOB");
        assert_eq!(envelope.data, vec![1, 2, 3]);
    }

    #[test]
    fn tre_envelope_new_short_tag() {
        let envelope = TreEnvelope::new("TEST", vec![]).unwrap();
        assert_eq!(envelope.tag, "TEST");
    }

    #[test]
    fn tre_envelope_new_invalid_tag() {
        let result = TreEnvelope::new("INVALID!", vec![]);
        assert!(result.is_err());
        match result {
            Err(JBPError::InvalidCetag { tag }) => assert_eq!(tag, "INVALID!"),
            _ => panic!("Expected InvalidCetag error"),
        }
    }

    #[test]
    fn tre_envelope_new_tag_too_long() {
        let result = TreEnvelope::new("TOOLONG1", vec![]);
        assert!(result.is_err());
        match result {
            Err(JBPError::InvalidCetag { tag }) => assert_eq!(tag, "TOOLONG1"),
            _ => panic!("Expected InvalidCetag error"),
        }
    }

    #[test]
    fn tre_envelope_parse_valid() {
        // CETAG: "GEOLOB", CEL: "00003", CEDATA: [0x01, 0x02, 0x03]
        let data = b"GEOLOB00003\x01\x02\x03";
        let (envelope, consumed) = TreEnvelope::parse(data).unwrap();

        assert_eq!(envelope.tag, "GEOLOB");
        assert_eq!(envelope.data, vec![0x01, 0x02, 0x03]);
        assert_eq!(consumed, 14); // 6 + 5 + 3
    }

    #[test]
    fn tre_envelope_parse_empty_cedata() {
        let data = b"TEST  00000";
        let (envelope, consumed) = TreEnvelope::parse(data).unwrap();

        assert_eq!(envelope.tag, "TEST  ");
        assert_eq!(envelope.data, Vec::<u8>::new());
        assert_eq!(consumed, 11);
    }

    #[test]
    fn tre_envelope_parse_insufficient_header() {
        let data = b"GEOLO"; // Only 5 bytes, need at least 11
        let result = TreEnvelope::parse(data);

        assert!(result.is_err());
        match result {
            Err(JBPError::UnexpectedEof {
                expected,
                available,
            }) => {
                assert_eq!(expected, 11);
                assert_eq!(available, 5);
            }
            _ => panic!("Expected UnexpectedEof error"),
        }
    }

    #[test]
    fn tre_envelope_parse_insufficient_cedata() {
        // CEL says 10 bytes but only 3 available
        let data = b"GEOLOB00010\x01\x02\x03";
        let result = TreEnvelope::parse(data);

        assert!(result.is_err());
        match result {
            Err(JBPError::UnexpectedEof {
                expected,
                available,
            }) => {
                assert_eq!(expected, 21); // 11 + 10
                assert_eq!(available, 14); // 11 + 3
            }
            _ => panic!("Expected UnexpectedEof error"),
        }
    }

    #[test]
    fn tre_envelope_parse_invalid_cetag() {
        let data = b"GEO!OB00003\x01\x02\x03";
        let result = TreEnvelope::parse(data);

        assert!(result.is_err());
        match result {
            Err(JBPError::InvalidCetag { tag }) => assert_eq!(tag, "GEO!OB"),
            _ => panic!("Expected InvalidCetag error"),
        }
    }

    #[test]
    fn tre_envelope_parse_invalid_cel() {
        let data = b"GEOLOBabcde\x01\x02\x03";
        let result = TreEnvelope::parse(data);

        assert!(result.is_err());
        match result {
            Err(JBPError::InvalidFormat { message }) => {
                assert!(message.contains("not a valid number"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn tre_envelope_parse_all_single() {
        let data = b"GEOLOB00003\x01\x02\x03";
        let envelopes = TreEnvelope::parse_all(data).unwrap();

        assert_eq!(envelopes.len(), 1);
        assert_eq!(envelopes[0].tag, "GEOLOB");
        assert_eq!(envelopes[0].data, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn tre_envelope_parse_all_multiple() {
        // Two TREs: GEOLOB with 3 bytes, TEST with 2 bytes
        let data = b"GEOLOB00003\x01\x02\x03TEST  00002\xAA\xBB";
        let envelopes = TreEnvelope::parse_all(data).unwrap();

        assert_eq!(envelopes.len(), 2);
        assert_eq!(envelopes[0].tag, "GEOLOB");
        assert_eq!(envelopes[0].data, vec![0x01, 0x02, 0x03]);
        assert_eq!(envelopes[1].tag, "TEST  ");
        assert_eq!(envelopes[1].data, vec![0xAA, 0xBB]);
    }

    #[test]
    fn tre_envelope_parse_all_empty() {
        let data = b"";
        let envelopes = TreEnvelope::parse_all(data).unwrap();
        assert!(envelopes.is_empty());
    }

    #[test]
    fn tre_envelope_to_bytes() {
        let envelope = TreEnvelope {
            tag: "GEOLOB".to_string(),
            data: vec![0x01, 0x02, 0x03],
        };

        let bytes = envelope.to_bytes();
        assert_eq!(bytes, b"GEOLOB00003\x01\x02\x03");
    }

    #[test]
    fn tre_envelope_to_bytes_short_tag() {
        let envelope = TreEnvelope {
            tag: "TEST".to_string(),
            data: vec![0x01],
        };

        let bytes = envelope.to_bytes();
        // Tag should be padded with spaces
        assert_eq!(bytes, b"TEST  00001\x01");
    }

    #[test]
    fn tre_envelope_to_bytes_empty_data() {
        let envelope = TreEnvelope {
            tag: "EMPTY ".to_string(),
            data: vec![],
        };

        let bytes = envelope.to_bytes();
        assert_eq!(bytes, b"EMPTY 00000");
    }

    #[test]
    fn tre_envelope_envelope_size() {
        let envelope = TreEnvelope {
            tag: "GEOLOB".to_string(),
            data: vec![0x01, 0x02, 0x03],
        };

        assert_eq!(envelope.envelope_size(), 14); // 6 + 5 + 3
    }

    #[test]
    fn tre_envelope_envelope_size_empty() {
        let envelope = TreEnvelope {
            tag: "TEST".to_string(),
            data: vec![],
        };

        assert_eq!(envelope.envelope_size(), 11); // 6 + 5 + 0
    }

    #[test]
    fn tre_envelope_round_trip() {
        let original = TreEnvelope {
            tag: "GEOLOB".to_string(),
            data: vec![0x01, 0x02, 0x03, 0x04, 0x05],
        };

        let bytes = original.to_bytes();
        let (parsed, consumed) = TreEnvelope::parse(&bytes).unwrap();

        assert_eq!(consumed, bytes.len());
        assert_eq!(parsed.tag, original.tag);
        assert_eq!(parsed.data, original.data);
    }

    #[test]
    fn tre_envelope_round_trip_short_tag() {
        let original = TreEnvelope {
            tag: "ABC".to_string(),
            data: vec![0xFF],
        };

        let bytes = original.to_bytes();
        let (parsed, _) = TreEnvelope::parse(&bytes).unwrap();

        // Tag will be padded with spaces when serialized
        assert_eq!(parsed.tag, "ABC   ");
        assert_eq!(parsed.data, original.data);
    }

    #[test]
    fn validate_cetag_valid() {
        assert!(validate_cetag("GEOLOB").is_ok());
        assert!(validate_cetag("TEST  ").is_ok());
        assert!(validate_cetag("ABC123").is_ok());
        assert!(validate_cetag("A").is_ok());
        assert!(validate_cetag("").is_ok());
    }

    #[test]
    fn validate_cetag_invalid_chars() {
        assert!(validate_cetag("GEO!OB").is_err());
        assert!(validate_cetag("TEST\n").is_err());
        assert!(validate_cetag("A@B").is_err());
        assert!(validate_cetag("TEST\t").is_err());
    }

    #[test]
    fn validate_cetag_too_long() {
        assert!(validate_cetag("TOOLONG1").is_err());
        assert!(validate_cetag("ABCDEFGH").is_err());
    }

    #[test]
    fn write_tre_envelopes_empty() {
        let envelopes: Vec<TreEnvelope> = vec![];
        let bytes = write_tre_envelopes(&envelopes);
        assert!(bytes.is_empty());
    }

    #[test]
    fn write_tre_envelopes_single() {
        let envelope = TreEnvelope::new("GEOLOB", vec![0x01, 0x02, 0x03]).unwrap();
        let bytes = write_tre_envelopes(&[envelope]);
        assert_eq!(bytes, b"GEOLOB00003\x01\x02\x03");
    }

    #[test]
    fn write_tre_envelopes_multiple() {
        let envelopes = vec![
            TreEnvelope::new("GEOLOB", vec![0x01, 0x02, 0x03]).unwrap(),
            TreEnvelope::new("TEST", vec![0xAA, 0xBB]).unwrap(),
        ];
        let bytes = write_tre_envelopes(&envelopes);
        // First envelope: "GEOLOB00003\x01\x02\x03" (14 bytes)
        // Second envelope: "TEST  00002\xAA\xBB" (13 bytes)
        assert_eq!(bytes.len(), 27);
        assert_eq!(&bytes[..14], b"GEOLOB00003\x01\x02\x03");
        assert_eq!(&bytes[14..], b"TEST  00002\xAA\xBB");
    }

    #[test]
    fn write_tre_envelopes_round_trip() {
        let original = vec![
            TreEnvelope::new("GEOLOB", vec![0x01, 0x02, 0x03]).unwrap(),
            TreEnvelope::new("SENSRB", vec![0x04, 0x05, 0x06, 0x07]).unwrap(),
        ];
        let bytes = write_tre_envelopes(&original);
        let parsed = TreEnvelope::parse_all(&bytes).unwrap();

        assert_eq!(parsed.len(), 2);
        // Tags are space-padded to 6 chars
        assert_eq!(parsed[0].tag, "GEOLOB");
        assert_eq!(parsed[0].data, vec![0x01, 0x02, 0x03]);
        assert_eq!(parsed[1].tag, "SENSRB");
        assert_eq!(parsed[1].data, vec![0x04, 0x05, 0x06, 0x07]);
    }

    // Tests for TreFieldGroup
    #[test]
    fn tre_field_group_new() {
        let group = TreFieldGroup::new("GEOLOB");
        assert_eq!(group.tag, "GEOLOB");
        assert!(group.is_empty());
        assert_eq!(group.len(), 0);
    }

    #[test]
    fn tre_field_group_insert_and_get() {
        let mut group = TreFieldGroup::new("GEOLOB");
        group.insert("ARV", serde_json::json!("000360000"));
        group.insert("BRV", serde_json::json!("000360000"));

        assert!(!group.is_empty());
        assert_eq!(group.len(), 2);
        assert_eq!(group.get("ARV"), Some(&serde_json::json!("000360000")));
        assert_eq!(group.get("BRV"), Some(&serde_json::json!("000360000")));
        assert_eq!(group.get("NONEXISTENT"), None);
    }

    // Tests for parse_tre_fields_from_metadata
    #[test]
    fn parse_tre_fields_empty_metadata() {
        let metadata = std::collections::HashMap::new();
        let groups = parse_tre_fields_from_metadata(&metadata);
        assert!(groups.is_empty());
    }

    #[test]
    fn parse_tre_fields_no_tre_fields() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("IID1".to_string(), serde_json::json!("TEST"));
        metadata.insert("IREP".to_string(), serde_json::json!("MONO"));

        let groups = parse_tre_fields_from_metadata(&metadata);
        assert!(groups.is_empty());
    }

    #[test]
    fn parse_tre_fields_single_tre() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("GEOLOB.ARV".to_string(), serde_json::json!("000360000"));
        metadata.insert("GEOLOB.BRV".to_string(), serde_json::json!("000360000"));
        metadata.insert("IID1".to_string(), serde_json::json!("TEST")); // Not a TRE field

        let groups = parse_tre_fields_from_metadata(&metadata);

        assert_eq!(groups.len(), 1);
        assert!(groups.contains_key("GEOLOB"));

        let geolob = &groups["GEOLOB"];
        assert_eq!(geolob.tag, "GEOLOB");
        assert_eq!(geolob.len(), 2);
        assert_eq!(geolob.get("ARV"), Some(&serde_json::json!("000360000")));
        assert_eq!(geolob.get("BRV"), Some(&serde_json::json!("000360000")));
    }

    #[test]
    fn parse_tre_fields_multiple_tres() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("GEOLOB.ARV".to_string(), serde_json::json!("000360000"));
        metadata.insert("GEOLOB.BRV".to_string(), serde_json::json!("000360000"));
        metadata.insert("SENSRB.PLATFORM".to_string(), serde_json::json!("AIRCRAFT"));
        metadata.insert("SENSRB.SENSOR".to_string(), serde_json::json!("EO"));

        let groups = parse_tre_fields_from_metadata(&metadata);

        assert_eq!(groups.len(), 2);
        assert!(groups.contains_key("GEOLOB"));
        assert!(groups.contains_key("SENSRB"));

        assert_eq!(groups["GEOLOB"].len(), 2);
        assert_eq!(groups["SENSRB"].len(), 2);
    }

    #[test]
    fn parse_tre_fields_ignores_empty_cetag() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(".ARV".to_string(), serde_json::json!("value")); // Empty CETAG

        let groups = parse_tre_fields_from_metadata(&metadata);
        assert!(groups.is_empty());
    }

    #[test]
    fn parse_tre_fields_ignores_empty_field_name() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("GEOLOB.".to_string(), serde_json::json!("value")); // Empty field name

        let groups = parse_tre_fields_from_metadata(&metadata);
        assert!(groups.is_empty());
    }

    #[test]
    fn parse_tre_fields_handles_nested_dots() {
        let mut metadata = std::collections::HashMap::new();
        // Only the first dot is used to split CETAG from field name
        metadata.insert("GEOLOB.NESTED.FIELD".to_string(), serde_json::json!("value"));

        let groups = parse_tre_fields_from_metadata(&metadata);

        assert_eq!(groups.len(), 1);
        assert!(groups.contains_key("GEOLOB"));
        // The field name includes everything after the first dot
        assert_eq!(
            groups["GEOLOB"].get("NESTED.FIELD"),
            Some(&serde_json::json!("value"))
        );
    }

    #[test]
    fn parse_tre_fields_preserves_value_types() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("TEST.STRING".to_string(), serde_json::json!("text"));
        metadata.insert("TEST.NUMBER".to_string(), serde_json::json!(42));
        metadata.insert("TEST.FLOAT".to_string(), serde_json::json!(3.14));
        metadata.insert("TEST.BOOL".to_string(), serde_json::json!(true));
        metadata.insert("TEST.NULL".to_string(), serde_json::Value::Null);

        let groups = parse_tre_fields_from_metadata(&metadata);

        assert_eq!(groups.len(), 1);
        let test_group = &groups["TEST"];
        assert_eq!(test_group.get("STRING"), Some(&serde_json::json!("text")));
        assert_eq!(test_group.get("NUMBER"), Some(&serde_json::json!(42)));
        assert_eq!(test_group.get("FLOAT"), Some(&serde_json::json!(3.14)));
        assert_eq!(test_group.get("BOOL"), Some(&serde_json::json!(true)));
        assert_eq!(test_group.get("NULL"), Some(&serde_json::Value::Null));
    }
}


#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy to generate valid CETAG strings (1-6 alphanumeric characters or spaces)
    fn valid_cetag_strategy() -> impl Strategy<Value = String> {
        // Generate 1-6 characters that are alphanumeric or space
        prop::collection::vec(
            prop::char::ranges(vec!['A'..='Z', '0'..='9', ' '..=' '].into()),
            1..=6,
        )
        .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    /// Strategy to generate CEDATA bytes (0 to 99999 bytes, limited for practical testing)
    fn cedata_strategy() -> impl Strategy<Value = Vec<u8>> {
        // Limit to 1000 bytes for practical test performance
        prop::collection::vec(any::<u8>(), 0..=1000)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Feature: tre-des-support, Property 1: TRE Envelope Round-Trip
        ///
        /// For any valid TRE envelope (CETAG + CEL + CEDATA), parsing the envelope
        /// and then writing it back SHALL produce byte-identical output.
        ///
        /// **Validates: Requirements 1.1, 1.2, 1.3, 9.1, 9.2, 9.3, 9.4, 17.1**
        #[test]
        fn tre_envelope_round_trip_property(
            tag in valid_cetag_strategy(),
            data in cedata_strategy()
        ) {
            // Create a TRE envelope
            let envelope = TreEnvelope::new(tag.clone(), data.clone()).unwrap();

            // Serialize to bytes
            let bytes = envelope.to_bytes();

            // Parse back from bytes
            let (parsed, consumed) = TreEnvelope::parse(&bytes).unwrap();

            // Verify all bytes were consumed
            prop_assert_eq!(consumed, bytes.len(), "All bytes should be consumed");

            // Verify envelope size matches
            prop_assert_eq!(
                parsed.envelope_size(),
                bytes.len(),
                "Envelope size should match serialized bytes length"
            );

            // Verify CEDATA is preserved exactly
            prop_assert_eq!(
                &parsed.data,
                &data,
                "CEDATA should be preserved exactly"
            );

            // Verify tag is preserved (accounting for space padding)
            let expected_tag = format!("{:<6}", tag); // Left-justify, space-pad to 6 chars
            prop_assert_eq!(
                &parsed.tag,
                &expected_tag,
                "CETAG should be preserved (with space padding)"
            );

            // Verify round-trip produces byte-identical output
            let re_serialized = parsed.to_bytes();
            prop_assert_eq!(
                re_serialized,
                bytes,
                "Re-serialization should produce byte-identical output"
            );
        }

        /// Feature: tre-des-support, Property 1 (Extended): Multiple TRE Envelope Round-Trip
        ///
        /// For any sequence of valid TRE envelopes, parsing all envelopes and then
        /// writing them back SHALL produce byte-identical output.
        ///
        /// **Validates: Requirements 1.1, 1.2, 1.3, 9.1, 9.2, 9.3, 9.4, 17.1**
        #[test]
        fn tre_envelope_parse_all_round_trip_property(
            envelopes_data in prop::collection::vec(
                (valid_cetag_strategy(), cedata_strategy()),
                0..=5
            )
        ) {
            // Create TRE envelopes
            let envelopes: Vec<TreEnvelope> = envelopes_data
                .iter()
                .map(|(tag, data)| TreEnvelope::new(tag.clone(), data.clone()).unwrap())
                .collect();

            // Serialize all envelopes to a single byte buffer
            let mut bytes = Vec::new();
            for envelope in &envelopes {
                bytes.extend(envelope.to_bytes());
            }

            // Parse all envelopes back
            let parsed = TreEnvelope::parse_all(&bytes).unwrap();

            // Verify count matches
            prop_assert_eq!(
                parsed.len(),
                envelopes.len(),
                "Parsed envelope count should match original"
            );

            // Verify each envelope's data is preserved
            for (i, (original, parsed_env)) in envelopes.iter().zip(parsed.iter()).enumerate() {
                prop_assert_eq!(
                    &parsed_env.data,
                    &original.data,
                    "CEDATA should be preserved for envelope {}", i
                );

                // Verify tag (accounting for space padding)
                let expected_tag = format!("{:<6}", original.tag);
                prop_assert_eq!(
                    &parsed_env.tag,
                    &expected_tag,
                    "CETAG should be preserved for envelope {}", i
                );
            }

            // Verify round-trip produces byte-identical output
            let mut re_serialized = Vec::new();
            for envelope in &parsed {
                re_serialized.extend(envelope.to_bytes());
            }
            prop_assert_eq!(
                re_serialized,
                bytes,
                "Re-serialization should produce byte-identical output"
            );
        }

        /// Feature: tre-des-support, Property 8: TRE Validation Error Handling
        ///
        /// For any TRE with invalid CETAG format or mismatched CEL/CEDATA length,
        /// the parser SHALL return an appropriate error.
        ///
        /// **Validates: Requirements 1.4, 1.5, 16.1, 16.2**
        #[test]
        fn tre_validation_error_handling_invalid_cetag(
            // Generate invalid characters (not alphanumeric or space)
            invalid_char in prop::char::ranges(vec!['!'..='/', ':'..='@', '['..='`', '{'..='~'].into()),
            prefix in "[A-Z0-9]{0,5}",
            data in cedata_strategy()
        ) {
            // Create a tag with an invalid character
            let invalid_tag = format!("{}{}", prefix, invalid_char);
            
            // Attempting to create a TreEnvelope with invalid tag should fail
            let result = TreEnvelope::new(invalid_tag.clone(), data.clone());
            prop_assert!(
                result.is_err(),
                "Creating TreEnvelope with invalid CETAG '{}' should fail",
                invalid_tag
            );
            
            // Verify it's specifically an InvalidCetag error
            match result {
                Err(JBPError::InvalidCetag { tag }) => {
                    prop_assert_eq!(
                        tag, invalid_tag,
                        "Error should contain the invalid tag"
                    );
                }
                _ => prop_assert!(false, "Expected InvalidCetag error"),
            }
        }

        /// Feature: tre-des-support, Property 8 (Extended): CEL/CEDATA Length Validation
        ///
        /// When parsing a TRE envelope where CEL exceeds available bytes,
        /// the parser SHALL return an UnexpectedEof error.
        ///
        /// **Validates: Requirements 1.5, 16.2**
        #[test]
        fn tre_validation_error_handling_insufficient_data(
            tag in valid_cetag_strategy(),
            declared_len in 1usize..1000,
            // Actual data is shorter than declared
            actual_len_ratio in 0.0f64..0.99
        ) {
            let actual_len = ((declared_len as f64) * actual_len_ratio) as usize;
            let actual_data: Vec<u8> = (0..actual_len).map(|i| (i % 256) as u8).collect();
            
            // Build raw bytes with mismatched CEL
            let padded_tag = format!("{:<6}", tag);
            let cel_str = format!("{:05}", declared_len);
            
            let mut bytes = Vec::new();
            bytes.extend(padded_tag.as_bytes());
            bytes.extend(cel_str.as_bytes());
            bytes.extend(&actual_data);
            
            // Parsing should fail with UnexpectedEof
            let result = TreEnvelope::parse(&bytes);
            prop_assert!(
                result.is_err(),
                "Parsing TRE with insufficient data should fail"
            );
            
            match result {
                Err(JBPError::UnexpectedEof { expected, available }) => {
                    // Expected is CETAG(6) + CEL(5) + declared_len
                    let expected_total = 11 + declared_len;
                    prop_assert_eq!(
                        expected, expected_total,
                        "Expected bytes should be header + declared length"
                    );
                    prop_assert_eq!(
                        available, bytes.len(),
                        "Available bytes should match actual buffer size"
                    );
                }
                Err(other) => prop_assert!(
                    false,
                    "Expected UnexpectedEof error, got: {:?}",
                    other
                ),
                Ok(_) => prop_assert!(false, "Should have failed with UnexpectedEof"),
            }
        }

        /// Feature: tre-des-support, Property 8 (Extended): Minimum Header Validation
        ///
        /// When parsing a TRE envelope with fewer than 11 bytes (CETAG + CEL),
        /// the parser SHALL return an UnexpectedEof error.
        ///
        /// **Validates: Requirements 1.5**
        #[test]
        fn tre_validation_error_handling_short_header(
            short_len in 0usize..11
        ) {
            let bytes: Vec<u8> = (0..short_len).map(|i| b'A' + (i % 26) as u8).collect();
            
            let result = TreEnvelope::parse(&bytes);
            prop_assert!(
                result.is_err(),
                "Parsing TRE with {} bytes (< 11) should fail",
                short_len
            );
            
            match result {
                Err(JBPError::UnexpectedEof { expected, available }) => {
                    prop_assert_eq!(expected, 11, "Expected minimum header size of 11");
                    prop_assert_eq!(available, short_len, "Available should match input size");
                }
                Err(other) => prop_assert!(
                    false,
                    "Expected UnexpectedEof error, got: {:?}",
                    other
                ),
                Ok(_) => prop_assert!(false, "Should have failed with UnexpectedEof"),
            }
        }
    }
}

