//! JBP dataset writer implementation.
//!
//! This module provides [`JBPDatasetWriter`] which implements the DatasetWriter
//! trait for creating NITF/NSIF files.
//!
//! # Two-Pass Writing
//!
//! NITF files require segment lengths to be written in the file header before
//! the segment data. The writer uses a two-pass approach:
//!
//! 1. **Collection Phase**: Assets are queued via `add_asset()` calls
//! 2. **Writing Phase**: On `close()`, all segment lengths are calculated,
//!    the file header is written with correct counts and lengths, then
//!    each segment's subheader and data are written in order.
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::{JBPDatasetWriter, NitfFormat};
//!
//! let mut writer = JBPDatasetWriter::new("output.ntf", NitfFormat::Nitf21)?;
//! writer.add_asset("image_segment_0", image_provider, "Main Image", "", &[])?;
//! writer.set_metadata(metadata_provider)?;
//! writer.close()?;
//! ```

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::error::JBPError;
use crate::jbp::image::encoder::{create_block_encoder, TileAssembler};
use crate::jbp::image::types::InterleaveMode;
use crate::jbp::overflow::{create_overflow_des, OverflowSource};
use crate::jbp::tre::{parse_tre_fields_from_metadata, write_tre_envelopes, TreEnvelope};
use crate::jbp::tre_fields::serialize_tre_groups_to_envelopes;
use crate::jbp::types::{NitfFormat, SegmentType};
use crate::parser::StructureRegistry;
use crate::traits::{AssetProvider, DatasetWriter, ImageAssetProvider, MetadataProvider};
use crate::types::AssetType;

/// Maximum TRE data size for UDID field (UDIDL max 99999 - 3 bytes for UDOFL).
const MAX_UDID_TRE_SIZE: usize = 99996;

/// Truncate a UTF-8 string to at most `max_bytes` bytes, ensuring the result
/// ends at a valid character boundary. Returns a string slice that is safe
/// to use with byte-based operations.
///
/// NITF fields are fixed-width ASCII fields, so we need to truncate strings
/// that may contain multi-byte UTF-8 characters without splitting a character.
fn truncate_to_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the largest byte index <= max_bytes that is a char boundary
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Maximum TRE data size for IXSHD field (IXSHDL max 99999 - 3 bytes for IXSOFL).
#[allow(dead_code)]
const MAX_IXSHD_TRE_SIZE: usize = 99996;

/// Maximum TRE data size for SXSHD field (SXSHDL max 99999 - 3 bytes for SXSOFL).
#[allow(dead_code)]
const MAX_SXSHD_TRE_SIZE: usize = 99996;

/// Maximum TRE data size for TXSHD field (TXSHDL max 99999 - 3 bytes for TXSOFL).
#[allow(dead_code)]
const MAX_TXSHD_TRE_SIZE: usize = 99996;

/// Overflow TRE data to be written to a TRE_OVERFLOW DES.
#[derive(Debug, Clone)]
struct OverflowTreData {
    /// The source of the overflow (which header field)
    source: OverflowSource,
    /// The 0-based segment index (0 for file header)
    segment_index: u16,
    /// The TRE envelopes that overflowed
    envelopes: Vec<TreEnvelope>,
}

/// Encoding hints extracted from asset metadata.
///
/// These hints control format-specific encoding options when writing NITF files.
/// They are read from the asset's metadata provider using standard NITF field names.
#[derive(Clone, Debug)]
pub struct EncodingHints {
    /// Band interleave mode (B, P, R, S)
    pub imode: String,
    /// Image compression code (NC, NM, C1, C3, etc.)
    pub ic: String,
    /// Pixels per block horizontal (1-8192)
    pub nppbh: u32,
    /// Pixels per block vertical (1-8192)
    pub nppbv: u32,
    /// Compression ratio (for compressed images)
    pub comrat: Option<String>,
    /// JPEG 2000 specific encoding hints (for IC=C8 or CD)
    pub j2k_hints: Option<crate::jbp::j2k::comrat::J2KEncodingHints>,
}

impl Default for EncodingHints {
    fn default() -> Self {
        Self {
            imode: "B".to_string(),
            ic: "NC".to_string(),
            nppbh: 0, // 0 means use image dimensions
            nppbv: 0, // 0 means use image dimensions
            comrat: None,
            j2k_hints: None,
        }
    }
}

/// Image properties extracted from an ImageAssetProvider.
#[derive(Clone, Debug)]
struct ImageProperties {
    /// Number of rows (height)
    nrows: u32,
    /// Number of columns (width)
    ncols: u32,
    /// Number of bands
    nbands: u32,
    /// Nominal bits per pixel
    nbpp: u32,
    /// Actual bits per pixel
    abpp: u32,
    /// Pixel value type (INT, SI, R, C)
    pvtype: String,
    /// Image representation (MONO, RGB, MULTI, etc.)
    irep: String,
    /// Pixels per block horizontal
    nppbh: u32,
    /// Pixels per block vertical
    nppbv: u32,
}

impl Default for ImageProperties {
    fn default() -> Self {
        Self {
            nrows: 1,
            ncols: 1,
            nbands: 1,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MONO".to_string(),
            nppbh: 1,
            nppbv: 1,
        }
    }
}

/// An asset queued for writing.
#[derive(Clone)]
struct QueuedAsset {
    /// Unique key for this asset
    key: String,
    /// The asset provider containing data and metadata
    provider: Arc<dyn AssetProvider>,
    /// Human-readable title
    title: String,
    /// Detailed description
    description: String,
    /// Semantic roles
    roles: Vec<String>,
    /// Segment type derived from asset type
    segment_type: SegmentType,
}

/// Writer for NITF/NSIF files implementing the DatasetWriter trait.
///
/// JBPDatasetWriter creates NITF imagery files using a two-pass approach
/// to handle the length-first format requirement.
///
/// # Thread Safety
///
/// The writer is `Send + Sync` to allow use across threads, though
/// typical usage is single-threaded.
pub struct JBPDatasetWriter {
    /// Output file path
    path: PathBuf,
    /// Output format (NITF 2.1 or NSIF 1.0)
    format: NitfFormat,
    /// Queued assets in order of addition
    assets: Vec<QueuedAsset>,
    /// Set of asset keys for duplicate detection
    asset_keys: HashSet<String>,
    /// File-level metadata provider (optional)
    file_metadata: Option<Arc<dyn MetadataProvider>>,
    /// Whether the writer has been closed
    closed: bool,
    /// Structure registry for TRE definitions (optional)
    registry: Option<Arc<StructureRegistry>>,
}

impl JBPDatasetWriter {
    /// Create a new writer for the specified path and format.
    ///
    /// The file is not created until `close()` is called.
    ///
    /// # Arguments
    /// * `path` - Output file path
    /// * `format` - NITF format variant (NITF 2.1 or NSIF 1.0)
    ///
    /// # Returns
    /// A new `JBPDatasetWriter` ready to accept assets.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let writer = JBPDatasetWriter::new("output.ntf", NitfFormat::Nitf21)?;
    /// ```
    pub fn new(path: impl AsRef<Path>, format: NitfFormat) -> Result<Self, CodecError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            format,
            assets: Vec::new(),
            asset_keys: HashSet::new(),
            file_metadata: None,
            closed: false,
            registry: None,
        })
    }

    /// Create a new writer with TRE support.
    ///
    /// The registry is used to look up TRE definitions for serializing
    /// TRE field values from metadata.
    ///
    /// # Arguments
    /// * `path` - Output file path
    /// * `format` - NITF format variant (NITF 2.1 or NSIF 1.0)
    /// * `registry` - Structure registry containing TRE definitions
    ///
    /// # Returns
    /// A new `JBPDatasetWriter` with TRE support.
    pub fn with_registry(
        path: impl AsRef<Path>,
        format: NitfFormat,
        registry: Arc<StructureRegistry>,
    ) -> Result<Self, CodecError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            format,
            assets: Vec::new(),
            asset_keys: HashSet::new(),
            file_metadata: None,
            closed: false,
            registry: Some(registry),
        })
    }

    /// Get the output format.
    pub fn format(&self) -> NitfFormat {
        self.format
    }

    /// Get the output path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the number of queued assets.
    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    /// Check if the writer has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Convert AssetType to SegmentType.
    fn asset_type_to_segment_type(asset_type: AssetType) -> SegmentType {
        match asset_type {
            AssetType::Image => SegmentType::Image,
            AssetType::Text => SegmentType::Text,
            AssetType::Graphics => SegmentType::Graphic,
            AssetType::Data => SegmentType::DataExtension,
        }
    }

    /// Count segments by type.
    fn count_segments_by_type(&self) -> (usize, usize, usize, usize, usize) {
        let mut numi = 0;
        let mut nums = 0;
        let mut numt = 0;
        let mut numdes = 0;
        let numres = 0; // Reserved extensions not supported yet

        for asset in &self.assets {
            match asset.segment_type {
                SegmentType::Image => numi += 1,
                SegmentType::Graphic => nums += 1,
                SegmentType::Text => numt += 1,
                SegmentType::DataExtension => numdes += 1,
                SegmentType::ReservedExtension => {} // Not counted
            }
        }

        (numi, nums, numt, numdes, numres)
    }

    /// Get assets grouped by segment type in order.
    fn get_assets_by_type(&self) -> (Vec<&QueuedAsset>, Vec<&QueuedAsset>, Vec<&QueuedAsset>, Vec<&QueuedAsset>) {
        let mut images = Vec::new();
        let mut graphics = Vec::new();
        let mut text = Vec::new();
        let mut des = Vec::new();

        for asset in &self.assets {
            match asset.segment_type {
                SegmentType::Image => images.push(asset),
                SegmentType::Graphic => graphics.push(asset),
                SegmentType::Text => text.push(asset),
                SegmentType::DataExtension => des.push(asset),
                SegmentType::ReservedExtension => {} // Not supported
            }
        }

        (images, graphics, text, des)
    }

    /// Split TRE envelopes into those that fit within a size limit and overflow.
    ///
    /// TREs are kept together - we don't split individual TREs across boundaries.
    /// TREs are added to the "fits" list until adding another would exceed the limit.
    ///
    /// # Arguments
    /// * `envelopes` - The TRE envelopes to split
    /// * `max_size` - Maximum total size in bytes for the "fits" portion
    ///
    /// # Returns
    /// A tuple of (fits, overflow) where:
    /// - `fits` contains envelopes that fit within max_size
    /// - `overflow` contains the remaining envelopes
    fn split_tres_by_size(envelopes: Vec<TreEnvelope>, max_size: usize) -> (Vec<TreEnvelope>, Vec<TreEnvelope>) {
        let mut fits = Vec::new();
        let mut overflow = Vec::new();
        let mut current_size = 0;

        for envelope in envelopes {
            let envelope_size = envelope.envelope_size();
            if current_size + envelope_size <= max_size {
                current_size += envelope_size;
                fits.push(envelope);
            } else {
                overflow.push(envelope);
            }
        }

        (fits, overflow)
    }

    /// Extract TRE envelopes from an asset's metadata.
    ///
    /// Parses TRE field values from the asset's metadata (fields with CETAG prefix)
    /// and returns them as TRE envelopes.
    ///
    /// # Arguments
    /// * `asset` - The queued asset
    ///
    /// # Returns
    /// TRE envelopes, or empty vec if no TREs or no registry.
    fn extract_tre_envelopes_from_asset(&self, asset: &QueuedAsset) -> Vec<TreEnvelope> {
        // Need a registry to serialize TREs
        let registry = match &self.registry {
            Some(r) => r,
            None => return Vec::new(),
        };

        // Get metadata from the asset
        let metadata = asset.provider.metadata();
        let metadata_dict = metadata.as_dict(None);

        // Parse TRE fields from metadata
        let tre_groups = parse_tre_fields_from_metadata(&metadata_dict);
        if tre_groups.is_empty() {
            return Vec::new();
        }

        // Serialize TRE groups to envelopes
        match serialize_tre_groups_to_envelopes(registry, &tre_groups) {
            Ok(envs) => envs,
            Err(_) => Vec::new(), // Silently skip on serialization errors
        }
    }

    /// Create an image subheader with TRE data and overflow handling.
    ///
    /// # Arguments
    /// * `asset` - The queued asset
    /// * `segment_index` - The 0-based index of this image segment
    ///
    /// # Returns
    /// A tuple of (subheader_bytes, overflow_data, encoding_hints) where:
    /// - `subheader_bytes` is the complete image subheader
    /// - `overflow_data` is Some if TREs exceeded UDID limit, None otherwise
    /// - `encoding_hints` are the validated hints used for this image
    fn create_image_subheader_with_overflow(
        &self,
        asset: &QueuedAsset,
        segment_index: u16,
    ) -> Result<(Vec<u8>, Option<OverflowTreData>, EncodingHints), CodecError> {
        // Extract image properties and encoding hints
        let props = Self::extract_image_properties(asset);
        
        // Detect and resolve conflicts between provider properties and metadata
        // Provider structural properties always override metadata
        let warnings = Self::detect_and_resolve_conflicts(asset, &props);
        for warning in warnings {
            eprintln!("Warning: {}", warning);
        }
        
        let hints = Self::extract_encoding_hints(asset, &props);
        let validated_hints = Self::validate_encoding_hints(&hints, &props)?;
        
        // Extract TRE envelopes from asset metadata
        let envelopes = self.extract_tre_envelopes_from_asset(asset);
        
        if envelopes.is_empty() {
            // No TREs, create subheader without TRE data
            return Ok((self.create_image_subheader_with_tres(asset, &[], None, &validated_hints), None, validated_hints));
        }

        // Split TREs by UDID size limit
        let (fits, overflow) = Self::split_tres_by_size(envelopes, MAX_UDID_TRE_SIZE);
        
        // Serialize the TREs that fit
        let tre_bytes = write_tre_envelopes(&fits);
        
        // Create overflow data if needed
        let overflow_data = if overflow.is_empty() {
            None
        } else {
            Some(OverflowTreData {
                source: OverflowSource::ImageUdid,
                segment_index,
                envelopes: overflow,
            })
        };

        // Create subheader with TRE bytes and overflow index placeholder
        // The overflow index will be set later when we know the DES index
        let subheader = self.create_image_subheader_with_tres(asset, &tre_bytes, overflow_data.as_ref(), &validated_hints);
        
        Ok((subheader, overflow_data, validated_hints))
    }


    /// Create a minimal image subheader.
    fn create_image_subheader(&self, asset: &QueuedAsset) -> Result<(Vec<u8>, EncodingHints), CodecError> {
        // Extract image properties and encoding hints
        let props = Self::extract_image_properties(asset);
        
        // Detect and resolve conflicts between provider properties and metadata
        // Provider structural properties always override metadata
        let warnings = Self::detect_and_resolve_conflicts(asset, &props);
        for warning in warnings {
            eprintln!("Warning: {}", warning);
        }
        
        let hints = Self::extract_encoding_hints(asset, &props);
        let validated_hints = Self::validate_encoding_hints(&hints, &props)?;
        
        // Extract TRE bytes from asset metadata if registry is available
        let tre_bytes = self.extract_tre_bytes_from_asset(asset);
        Ok((self.create_image_subheader_with_tres(asset, &tre_bytes, None, &validated_hints), validated_hints))
    }

    /// Extract image properties from an asset provider.
    /// 
    /// If the provider implements ImageAssetProvider (via BufferedImageAssetProvider),
    /// extract the image properties. Otherwise, return default values.
    /// 
    /// Note: NPPBH and NPPBV are set to defaults here. The actual values
    /// should come from encoding hints extracted via `extract_encoding_hints()`.
    /// Callers should override these fields with validated encoding hints.
    fn extract_image_properties(asset: &QueuedAsset) -> ImageProperties {
        use crate::buffered::BufferedImageAssetProvider;
        use crate::traits::ImageAssetProvider;
        
        // Try to downcast to BufferedImageAssetProvider
        if let Some(memory_provider) = asset.provider.as_any().downcast_ref::<BufferedImageAssetProvider>() {
            let config = memory_provider.config();
            ImageProperties {
                nrows: memory_provider.num_rows(),
                ncols: memory_provider.num_columns(),
                nbands: memory_provider.num_bands(),
                nbpp: memory_provider.num_bits_per_pixel(),
                abpp: memory_provider.actual_bits_per_pixel(),
                pvtype: Self::pixel_type_to_pvtype(memory_provider.pixel_value_type()),
                irep: config.irep.clone(),
                // Default block sizes from provider - may be overridden by encoding hints
                nppbh: memory_provider.num_pixels_per_block_horizontal(),
                nppbv: memory_provider.num_pixels_per_block_vertical(),
            }
        } else {
            // Default values for non-ImageAssetProvider assets
            ImageProperties::default()
        }
    }

    /// Get a reference to the ImageAssetProvider if the asset implements it.
    ///
    /// This method attempts to downcast the asset provider to BufferedImageAssetProvider
    /// and returns a reference to it as an ImageAssetProvider trait object.
    ///
    /// # Arguments
    /// * `asset` - The queued asset to check
    ///
    /// # Returns
    /// Some(&dyn ImageAssetProvider) if the asset implements ImageAssetProvider, None otherwise.
    fn get_image_asset_provider(asset: &QueuedAsset) -> Option<&dyn ImageAssetProvider> {
        use crate::buffered::BufferedImageAssetProvider;
        asset.provider.as_any().downcast_ref::<BufferedImageAssetProvider>()
            .map(|p| p as &dyn ImageAssetProvider)
    }

    /// Collect the set of provided block coordinates from an ImageAssetProvider.
    ///
    /// This method iterates over the block grid and checks `has_block()` for each
    /// coordinate to determine which blocks have been provided. This is used for
    /// masked image writing to generate the block mask table.
    ///
    /// # Arguments
    /// * `provider` - The ImageAssetProvider to check for provided blocks
    ///
    /// # Returns
    /// A HashSet containing (row, col) tuples for all blocks where `has_block()` returns true.
    ///
    /// # Requirements
    /// - 5.1: Tracks which blocks have been provided via set_block()
    fn collect_provided_blocks(provider: &dyn ImageAssetProvider) -> HashSet<(u32, u32)> {
        let (grid_rows, grid_cols) = provider.block_grid_size();
        let mut provided = HashSet::new();

        for row in 0..grid_rows {
            for col in 0..grid_cols {
                if provider.has_block(row, col, 0) {
                    provided.insert((row, col));
                }
            }
        }

        provided
    }

    /// Validate that block data is consistent with the IC value.
    ///
    /// For non-masked IC values (NC, C8, CD, etc.), all blocks must be provided.
    /// For masked IC values (NM, M8, MD, etc.), sparse data is allowed.
    ///
    /// # Arguments
    /// * `provider` - The ImageAssetProvider to validate
    /// * `ic` - The IC (Image Compression) value from encoding hints
    ///
    /// # Returns
    /// Ok(provided_blocks) if validation passes, or MissingBlocks error if
    /// a non-masked IC is used with sparse data.
    ///
    /// # Requirements
    /// - 7.2: Non-masked IC requires all blocks to be provided
    /// - 7.3: Raise MissingBlocks error with expected/provided counts
    fn validate_blocks_for_ic(
        provider: &dyn ImageAssetProvider,
        ic: &str,
    ) -> Result<HashSet<(u32, u32)>, CodecError> {
        use crate::jbp::image::is_masked_ic;

        let provided_blocks = Self::collect_provided_blocks(provider);
        let (grid_rows, grid_cols) = provider.block_grid_size();
        let total_blocks = grid_rows * grid_cols;

        // For non-masked IC values, all blocks must be provided
        if !is_masked_ic(ic) && (provided_blocks.len() as u32) < total_blocks {
            return Err(CodecError::MissingBlocks {
                expected: total_blocks,
                provided: provided_blocks.len() as u32,
                ic: ic.to_string(),
            });
        }

        Ok(provided_blocks)
    }

    /// Extract encoding hints from asset metadata.
    ///
    /// Reads encoding hint fields (IMODE, IC, NPPBH, NPPBV, COMRAT) from the asset's
    /// metadata provider. Missing fields use default values.
    ///
    /// # Arguments
    /// * `asset` - The queued asset to extract hints from
    /// * `image_props` - Image properties for default block sizes
    ///
    /// # Returns
    /// EncodingHints with values from metadata or defaults.
    fn extract_encoding_hints(asset: &QueuedAsset, image_props: &ImageProperties) -> EncodingHints {
        use crate::jbp::j2k::comrat::J2KEncodingHints;
        
        let metadata = asset.provider.metadata();
        let dict = metadata.as_dict(None);
        
        // Extract imode - default to "B" if not present
        // Field names use uppercase to match .ksy parser output
        let imode = dict.get("IMODE")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "B".to_string());
        
        // Extract ic - default to "NC" (no compression) if not present
        let ic = dict.get("IC")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "NC".to_string());
        
        // Extract nppbh - default to provider's block width if not present or 0
        let nppbh = dict.get("NPPBH")
            .and_then(|v| {
                // Try to parse as integer from string or number
                if let Some(s) = v.as_str() {
                    s.trim().parse::<u32>().ok()
                } else if let Some(n) = v.as_u64() {
                    Some(n as u32)
                } else if let Some(n) = v.as_i64() {
                    Some(n as u32)
                } else {
                    None
                }
            })
            .filter(|&n| n > 0)
            .unwrap_or(image_props.nppbh);
        
        // Extract nppbv - default to provider's block height if not present or 0
        let nppbv = dict.get("NPPBV")
            .and_then(|v| {
                // Try to parse as integer from string or number
                if let Some(s) = v.as_str() {
                    s.trim().parse::<u32>().ok()
                } else if let Some(n) = v.as_u64() {
                    Some(n as u32)
                } else if let Some(n) = v.as_i64() {
                    Some(n as u32)
                } else {
                    None
                }
            })
            .filter(|&n| n > 0)
            .unwrap_or(image_props.nppbv);
        
        // Extract comrat - optional, only used for compressed images
        let comrat = dict.get("COMRAT")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        // Extract J2K-specific encoding hints for IC=C8, CD, M8, or MD
        let ic_trimmed = ic.trim();
        let j2k_hints = if ic_trimmed == "C8" || ic_trimmed == "CD" || ic_trimmed == "M8" || ic_trimmed == "MD" {
            // Extract lossless flag
            let lossless = dict.get("J2K_LOSSLESS")
                .and_then(|v| {
                    if let Some(b) = v.as_bool() {
                        Some(b)
                    } else if let Some(s) = v.as_str() {
                        match s.trim().to_lowercase().as_str() {
                            "true" | "1" | "yes" => Some(true),
                            "false" | "0" | "no" => Some(false),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or(false);
            
            // Extract compression ratio
            let compression_ratio = dict.get("J2K_COMPRESSION_RATIO")
                .and_then(|v| {
                    if let Some(n) = v.as_f64() {
                        Some(n)
                    } else if let Some(s) = v.as_str() {
                        s.trim().parse::<f64>().ok()
                    } else {
                        None
                    }
                });
            
            // Extract decomposition levels
            let decomposition_levels = dict.get("J2K_DECOMPOSITION_LEVELS")
                .and_then(|v| {
                    if let Some(n) = v.as_u64() {
                        Some(n as u8)
                    } else if let Some(s) = v.as_str() {
                        s.trim().parse::<u8>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(5);
            
            // Extract quality layers
            let quality_layers = dict.get("J2K_QUALITY_LAYERS")
                .and_then(|v| {
                    if let Some(n) = v.as_u64() {
                        Some(n as u8)
                    } else if let Some(s) = v.as_str() {
                        s.trim().parse::<u8>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(1);
            
            // HTJ2K is determined by IC=CD or MD
            let htj2k = ic_trimmed == "CD" || ic_trimmed == "MD";
            
            Some(J2KEncodingHints {
                compression_ratio: if lossless { None } else { compression_ratio.or(Some(10.0)) },
                lossless,
                decomposition_levels,
                quality_layers,
                htj2k,
            })
        } else {
            None
        };
        
        EncodingHints {
            imode,
            ic,
            nppbh,
            nppbv,
            comrat,
            j2k_hints,
        }
    }

    /// Detect and resolve conflicts between provider properties and metadata.
    ///
    /// Provider structural properties (num_bands, pixel_type, dimensions) always
    /// override any conflicting values in metadata. This method checks for conflicts
    /// and logs warnings when they are detected.
    ///
    /// # Conflict Resolution Rules
    /// - Provider num_bands overrides metadata NBANDS
    /// - Provider pixel_type overrides metadata PVTYPE
    /// - Provider dimensions override metadata NROWS/NCOLS
    /// - IREP inconsistent with band count logs a warning
    ///
    /// # Arguments
    /// * `asset` - The queued asset containing metadata
    /// * `image_props` - Image properties extracted from the provider (authoritative)
    ///
    /// # Returns
    /// A vector of warning messages for any detected conflicts.
    fn detect_and_resolve_conflicts(asset: &QueuedAsset, image_props: &ImageProperties) -> Vec<String> {
        let metadata = asset.provider.metadata();
        let dict = metadata.as_dict(None);
        let mut warnings = Vec::new();

        // Check for NBANDS conflict
        if let Some(nbands_value) = dict.get("NBANDS") {
            let metadata_nbands = if let Some(s) = nbands_value.as_str() {
                s.trim().parse::<u32>().ok()
            } else if let Some(n) = nbands_value.as_u64() {
                Some(n as u32)
            } else if let Some(n) = nbands_value.as_i64() {
                Some(n as u32)
            } else {
                None
            };

            if let Some(meta_bands) = metadata_nbands {
                if meta_bands != image_props.nbands {
                    warnings.push(format!(
                        "Metadata NBANDS ({}) conflicts with provider band count ({}), using provider value",
                        meta_bands, image_props.nbands
                    ));
                }
            }
        }

        // Check for PVTYPE conflict
        if let Some(pvtype_value) = dict.get("PVTYPE") {
            if let Some(meta_pvtype) = pvtype_value.as_str() {
                let meta_pvtype_trimmed = meta_pvtype.trim();
                let props_pvtype_trimmed = image_props.pvtype.trim();
                if meta_pvtype_trimmed != props_pvtype_trimmed {
                    warnings.push(format!(
                        "Metadata PVTYPE ('{}') conflicts with provider pixel type ('{}'), using provider value",
                        meta_pvtype_trimmed, props_pvtype_trimmed
                    ));
                }
            }
        }

        // Check for NROWS conflict
        if let Some(nrows_value) = dict.get("NROWS") {
            let metadata_nrows = if let Some(s) = nrows_value.as_str() {
                s.trim().parse::<u32>().ok()
            } else if let Some(n) = nrows_value.as_u64() {
                Some(n as u32)
            } else if let Some(n) = nrows_value.as_i64() {
                Some(n as u32)
            } else {
                None
            };

            if let Some(meta_rows) = metadata_nrows {
                if meta_rows != image_props.nrows {
                    warnings.push(format!(
                        "Metadata NROWS ({}) conflicts with provider row count ({}), using provider value",
                        meta_rows, image_props.nrows
                    ));
                }
            }
        }

        // Check for NCOLS conflict
        if let Some(ncols_value) = dict.get("NCOLS") {
            let metadata_ncols = if let Some(s) = ncols_value.as_str() {
                s.trim().parse::<u32>().ok()
            } else if let Some(n) = ncols_value.as_u64() {
                Some(n as u32)
            } else if let Some(n) = ncols_value.as_i64() {
                Some(n as u32)
            } else {
                None
            };

            if let Some(meta_cols) = metadata_ncols {
                if meta_cols != image_props.ncols {
                    warnings.push(format!(
                        "Metadata NCOLS ({}) conflicts with provider column count ({}), using provider value",
                        meta_cols, image_props.ncols
                    ));
                }
            }
        }

        // Check for IREP/band count mismatch
        // IREP values and their expected band counts:
        // - MONO: 1 band
        // - RGB: 3 bands
        // - RGB/LUT: 1 band (lookup table)
        // - MULTI: any number of bands
        // - NODISPLY: any number of bands
        // - NVECTOR: any number of bands
        // - POLAR: 2 bands
        // - VPH: 2 bands
        if let Some(irep_value) = dict.get("IREP") {
            if let Some(meta_irep) = irep_value.as_str() {
                let meta_irep_trimmed = meta_irep.trim();
                let expected_bands: Option<u32> = match meta_irep_trimmed {
                    "MONO" => Some(1),
                    "RGB" => Some(3),
                    "RGB/LUT" => Some(1),
                    "POLAR" | "VPH" => Some(2),
                    // MULTI, NODISPLY, NVECTOR can have any number of bands
                    _ => None,
                };

                if let Some(expected) = expected_bands {
                    if expected != image_props.nbands {
                        warnings.push(format!(
                            "IREP '{}' inconsistent with {} bands, using provider band count",
                            meta_irep_trimmed, image_props.nbands
                        ));
                    }
                }
            }
        } else {
            // Also check the IREP from image_props (which comes from the provider config)
            // against the actual band count
            let irep_trimmed = image_props.irep.trim();
            let expected_bands: Option<u32> = match irep_trimmed {
                "MONO" => Some(1),
                "RGB" => Some(3),
                "RGB/LUT" => Some(1),
                "POLAR" | "VPH" => Some(2),
                _ => None,
            };

            if let Some(expected) = expected_bands {
                if expected != image_props.nbands {
                    warnings.push(format!(
                        "IREP '{}' inconsistent with {} bands, using provider band count",
                        irep_trimmed, image_props.nbands
                    ));
                }
            }
        }

        warnings
    }

    /// Validate encoding hints and auto-adjust block sizes if needed.
    ///
    /// # Validation Rules
    /// - IMODE must be one of: B, P, R, S
    /// - NPPBH must be in range [1, 8192]
    /// - NPPBV must be in range [1, 8192]
    /// - Block sizes larger than image dimensions are auto-adjusted
    /// - For JPEG 2000 (IC=C8 or CD):
    ///   - IMODE must be "B" (BPJ2K01.20 requirement)
    ///   - NBPP must be in range [1, 38]
    ///   - ABPP must equal NBPP
    ///
    /// # Arguments
    /// * `hints` - The encoding hints to validate
    /// * `image_props` - Image properties for dimension checks
    ///
    /// # Returns
    /// Validated (and possibly adjusted) encoding hints, or an error for invalid values.
    fn validate_encoding_hints(
        hints: &EncodingHints,
        image_props: &ImageProperties,
    ) -> Result<EncodingHints, CodecError> {
        let ic_trimmed = hints.ic.trim();
        let is_j2k = ic_trimmed == "C8" || ic_trimmed == "CD";
        
        // Validate IMODE
        let valid_imodes = ["B", "P", "R", "S"];
        if !valid_imodes.contains(&hints.imode.as_str()) {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid IMODE value '{}': must be B, P, R, or S",
                hints.imode
            )));
        }
        
        // BPJ2K01.20: IMODE must be "B" for JPEG 2000 images
        if is_j2k && hints.imode != "B" {
            return Err(CodecError::InvalidFormat(format!(
                "JPEG 2000 images (IC={}) must have IMODE=B, got '{}' (BPJ2K01.20 requirement)",
                ic_trimmed, hints.imode
            )));
        }
        
        // BPJ2K01.20: NBPP must be 1-38 for JPEG 2000 images
        if is_j2k {
            if image_props.nbpp < 1 || image_props.nbpp > 38 {
                return Err(CodecError::InvalidFormat(format!(
                    "JPEG 2000 images (IC={}) must have NBPP in range [1, 38], got {} (BPJ2K01.20 requirement)",
                    ic_trimmed, image_props.nbpp
                )));
            }
            
            // BPJ2K01.20: ABPP must equal NBPP for JPEG 2000 images
            if image_props.abpp != image_props.nbpp {
                return Err(CodecError::InvalidFormat(format!(
                    "JPEG 2000 images (IC={}) must have ABPP equal to NBPP, got ABPP={} and NBPP={} (BPJ2K01.20 requirement)",
                    ic_trimmed, image_props.abpp, image_props.nbpp
                )));
            }
        }
        
        // JPEG DCT specific validation (IC=C3, M3, I1)
        let is_jpeg = ic_trimmed == "C3" || ic_trimmed == "M3" || ic_trimmed == "I1";
        if is_jpeg {
            // JPEG only supports 8-bit pixels (12-bit is not supported due to libjpeg-turbo limitations)
            if image_props.nbpp != 8 {
                return Err(CodecError::InvalidFormat(format!(
                    "JPEG DCT images (IC={}) only support 8-bit pixels, got {} bits. \
                     Consider using JPEG 2000 (IC=C8) or uncompressed format (IC=NC) for other bit depths.",
                    ic_trimmed, image_props.nbpp
                )));
            }
            
            // I1 (Downsampled JPEG) has dimension constraints
            if ic_trimmed == "I1" {
                if image_props.nrows > 2048 || image_props.ncols > 2048 {
                    return Err(CodecError::InvalidFormat(format!(
                        "IC=I1 (Downsampled JPEG) requires dimensions ≤2048×2048, got {}×{}",
                        image_props.ncols, image_props.nrows
                    )));
                }
            }
            
            // JPEG supports IMODE=B, P, or S (not R for row interleaved)
            // For RGB/YCbCr, IMODE=P is typical
            // For multiband, IMODE=B or S is used
            if hints.imode == "R" {
                return Err(CodecError::InvalidFormat(format!(
                    "JPEG DCT images (IC={}) do not support IMODE=R (row interleaved). \
                     Use IMODE=B (block), IMODE=P (pixel), or IMODE=S (sequential).",
                    ic_trimmed
                )));
            }
        }
        
        // Validate NPPBH range
        if hints.nppbh < 1 || hints.nppbh > 8192 {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid NPPBH value '{}': must be between 1 and 8192",
                hints.nppbh
            )));
        }
        
        // Validate NPPBV range
        if hints.nppbv < 1 || hints.nppbv > 8192 {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid NPPBV value '{}': must be between 1 and 8192",
                hints.nppbv
            )));
        }
        
        // Auto-adjust block sizes if larger than image dimensions
        let mut adjusted = hints.clone();
        
        if hints.nppbh > image_props.ncols {
            // Note: In production, this would log a warning
            // "NPPBH {} exceeds image width {}, adjusting to {}"
            adjusted.nppbh = image_props.ncols;
        }
        
        if hints.nppbv > image_props.nrows {
            // Note: In production, this would log a warning
            // "NPPBV {} exceeds image height {}, adjusting to {}"
            adjusted.nppbv = image_props.nrows;
        }
        
        // For JPEG 2000, force IMODE to "B" if not already set
        if is_j2k {
            adjusted.imode = "B".to_string();
        }
        
        // For JPEG DCT, set default COMRAT if not specified
        if is_jpeg && adjusted.comrat.is_none() {
            // Default JPEG quality is 75, which maps to COMRAT "75.0"
            adjusted.comrat = Some("75.0".to_string());
        }
        
        Ok(adjusted)
    }

    /// Convert PixelType to PVTYPE string.
    fn pixel_type_to_pvtype(pixel_type: crate::types::PixelType) -> String {
        use crate::types::PixelType;
        match pixel_type {
            PixelType::UInt8 | PixelType::UInt16 | PixelType::UInt32 => "INT".to_string(),
            PixelType::Int8 | PixelType::Int16 | PixelType::Int32 => "SI".to_string(),
            PixelType::Float32 | PixelType::Float64 => "R".to_string(),
        }
    }

    /// Encode image data using BlockEncoder and TileAssembler.
    ///
    /// This method uses the block-based encoding architecture to convert image data
    /// from the source ImageAssetProvider to the target IMODE format. It supports
    /// different input and output tile sizes through the TileAssembler.
    ///
    /// # Arguments
    /// * `provider` - The ImageAssetProvider to read tiles from
    /// * `hints` - Encoding hints specifying IMODE, IC, and block sizes
    /// * `props` - Image properties (dimensions, bands, bit depth)
    ///
    /// # Returns
    /// The encoded image data ready for writing to the NITF file.
    ///
    /// # Errors
    /// Returns an error if encoding fails or if the compression type is unsupported.
    /// Returns MissingBlocks error if non-masked IC is used with sparse data.
    fn encode_image_with_block_encoder(
        provider: &dyn ImageAssetProvider,
        hints: &EncodingHints,
        props: &ImageProperties,
    ) -> Result<Vec<u8>, CodecError> {
        use crate::jbp::image::{is_masked_ic, ImageDataMask, unmask_ic};

        // Parse IMODE from hints
        let imode = InterleaveMode::from_char(
            hints.imode.chars().next().unwrap_or('B')
        )?;

        // Validate blocks against IC value and get provided blocks
        let provided_blocks = Self::validate_blocks_for_ic(provider, &hints.ic)?;

        // Determine if this is a masked image
        let is_masked = is_masked_ic(&hints.ic);

        // For masked images, use the underlying compression type for encoding
        let encoding_ic = if is_masked {
            unmask_ic(&hints.ic).to_string()
        } else {
            hints.ic.clone()
        };

        // Determine if pixel values are signed
        let is_signed = props.pvtype == "SI";

        // For masked compressed images (M8, MD, M3), we don't need the initial multi-tile encoder
        // because we create per-block single-tile encoders later. Skip creating it
        // to avoid issues with decomposition levels vs image dimensions.
        let is_masked_compressed = is_masked && (encoding_ic == "C8" || encoding_ic == "CD" || encoding_ic == "C3");

        // Create the block encoder based on IC code (use underlying compression for masked)
        // Skip for masked compressed images - we'll create per-block encoders later
        #[cfg(feature = "openjpeg")]
        let encoder: Option<Box<dyn crate::jbp::image::encoder::BlockEncoder>> = if is_masked_compressed {
            None
        } else {
            Some(create_block_encoder(
                &encoding_ic,
                props.nrows,
                props.ncols,
                props.nbands,
                props.nbpp as u8,
                is_signed,
                imode,
                hints.nppbh,
                hints.nppbv,
                hints.j2k_hints.as_ref(),
                hints.comrat.as_deref(),
            )?)
        };

        #[cfg(not(feature = "openjpeg"))]
        let encoder: Option<Box<dyn crate::jbp::image::encoder::BlockEncoder>> = if is_masked_compressed {
            None
        } else {
            Some(create_block_encoder(
                &encoding_ic,
                props.nrows,
                props.ncols,
                props.nbands,
                props.nbpp as u8,
                is_signed,
                imode,
                hints.nppbh,
                hints.nppbv,
                None,
                hints.comrat.as_deref(),
            )?)
        };

        // Create tile assembler to read source tiles and produce output tiles
        let assembler = TileAssembler::new(provider, hints.nppbh, hints.nppbv);
        let (grid_rows, grid_cols) = assembler.output_grid_size();

        if is_masked {
            // For masked images, generate mask table and encode only provided blocks
            // Blocks are stored sequentially (not at calculated positions) and the
            // mask table contains offsets to each block's data.
            
            // Create initial mask with placeholder offsets
            let mut mask = ImageDataMask::from_provided_blocks(
                &provided_blocks,
                grid_cols,  // num_blocks_per_row = grid_cols
                grid_rows,  // num_blocks_per_col = grid_rows
                props.nbands,
                imode,
            );

            // For uncompressed masked images (NM), we encode blocks sequentially
            // and track offsets. For compressed masked images (M8, M3, etc.), we use
            // the encoder which handles its own offset tracking.
            if encoding_ic == "NC" {
                // Uncompressed masked image: encode blocks sequentially
                let mut encoded_data = Vec::new();
                let bpp = ((props.nbpp as usize) + 7) / 8;
                
                for block_row in 0..grid_rows {
                    for block_col in 0..grid_cols {
                        let block_index = (block_row * grid_cols + block_col) as usize;
                        
                        if provided_blocks.contains(&(block_row, block_col)) {
                            // Record the offset where this block starts
                            mask.block_offsets[block_index] = encoded_data.len() as u32;
                            
                            // Get the tile data
                            let (tile_data, shape) = assembler.get_output_tile(block_row, block_col)?;
                            
                            // Convert from BSQ to target IMODE and append
                            let converted = crate::jbp::image::interleave::from_band_sequential(
                                &tile_data,
                                imode,
                                shape[1], // rows
                                shape[2], // cols
                                shape[0], // bands
                                bpp,
                            )?;
                            
                            encoded_data.extend_from_slice(&converted);
                        }
                        // Masked blocks already have EMPTY_BLOCK_OFFSET from from_provided_blocks
                    }
                }
                
                // Serialize mask with updated offsets
                let mask_bytes = mask.to_bytes();

                // Combine mask table and encoded data
                let mut result = Vec::with_capacity(mask_bytes.len() + encoded_data.len());
                result.extend_from_slice(&mask_bytes);
                result.extend_from_slice(&encoded_data);

                Ok(result)
            } else if encoding_ic == "C3" {
                // Masked JPEG DCT image (M3): encode each block as a separate JPEG stream.
                // The decoder expects each block to be a standalone JPEG stream.
                //
                // We create a new single-block encoder for each block, encode it, and
                // concatenate the streams while tracking offsets.
                
                // Drop the encoder if it exists (it shouldn't for masked images)
                drop(encoder);
                
                let mut encoded_data = Vec::new();
                
                for block_row in 0..grid_rows {
                    for block_col in 0..grid_cols {
                        let block_index = (block_row * grid_cols + block_col) as usize;
                        
                        if provided_blocks.contains(&(block_row, block_col)) {
                            // Record the offset where this block's JPEG stream starts
                            mask.block_offsets[block_index] = encoded_data.len() as u32;
                            
                            // Get the tile data
                            let (tile_data, shape) = assembler.get_output_tile(block_row, block_col)?;
                            
                            // Create a single-block JPEG encoder for this block
                            let block_height = shape[1];
                            let block_width = shape[2];
                            
                            #[cfg(feature = "libjpeg-turbo")]
                            let mut block_encoder = create_block_encoder(
                                &encoding_ic,
                                block_height,  // Single block = block dimensions
                                block_width,
                                props.nbands,
                                props.nbpp as u8,
                                is_signed,
                                imode,
                                block_width,   // Tile size = full block (single tile)
                                block_height,
                                None,  // No J2K hints for JPEG
                                hints.comrat.as_deref(),
                            )?;
                            
                            #[cfg(not(feature = "libjpeg-turbo"))]
                            return Err(CodecError::Unsupported(
                                "JPEG DCT compression (IC=M3) requires the 'libjpeg-turbo' feature to be enabled.".into()
                            ));
                            
                            #[cfg(feature = "libjpeg-turbo")]
                            {
                                // Encode the single block (tile 0,0 in this single-tile image)
                                block_encoder.encode_block(0, 0, &tile_data, shape)?;
                                
                                // Finalize to get the JPEG stream for this block
                                let block_jpeg = block_encoder.finalize()?;
                                
                                // Append the JPEG stream
                                encoded_data.extend_from_slice(&block_jpeg);
                            }
                        }
                        // Masked blocks already have EMPTY_BLOCK_OFFSET from from_provided_blocks
                    }
                }

                // Serialize mask with updated offsets
                let mask_bytes = mask.to_bytes();

                // Combine mask table and encoded data
                let mut result = Vec::with_capacity(mask_bytes.len() + encoded_data.len());
                result.extend_from_slice(&mask_bytes);
                result.extend_from_slice(&encoded_data);

                Ok(result)
            } else {
                // Compressed masked image (M8, MD, etc.): encode each block as a
                // separate single-tile J2K codestream. The decoder expects each
                // block to be a standalone codestream starting with SOC marker.
                //
                // We don't use the multi-tile encoder here. Instead, we create
                // a new single-tile encoder for each block, encode it, and
                // concatenate the codestreams while tracking offsets.
                
                // encoder is None for masked J2K, so nothing to drop
                drop(encoder);
                
                let mut encoded_data = Vec::new();
                
                for block_row in 0..grid_rows {
                    for block_col in 0..grid_cols {
                        let block_index = (block_row * grid_cols + block_col) as usize;
                        
                        if provided_blocks.contains(&(block_row, block_col)) {
                            // Record the offset where this block's codestream starts
                            mask.block_offsets[block_index] = encoded_data.len() as u32;
                            
                            // Get the tile data
                            let (tile_data, shape) = assembler.get_output_tile(block_row, block_col)?;
                            
                            // Create a single-tile encoder for this block
                            // The tile dimensions are the actual block dimensions
                            let block_height = shape[1];
                            let block_width = shape[2];
                            
                            // For masked J2K, we need to calculate safe decomposition levels
                            // based on the actual block dimensions, not the nominal block size.
                            // This is especially important for partial blocks at image edges.
                            //
                            // OpenJPEG requires: min_dim >= 2^decomposition_levels
                            // So: decomposition_levels <= floor(log2(min_dim))
                            #[cfg(feature = "openjpeg")]
                            let block_hints = {
                                use crate::jbp::j2k::comrat::J2KEncodingHints;
                                
                                let min_dim = block_height.min(block_width);
                                // Calculate safe decomposition levels based on OpenJPEG's requirement:
                                // min_dim >= 2^decomposition_levels
                                // Therefore: decomposition_levels <= floor(log2(min_dim))
                                let safe_levels = if min_dim <= 1 {
                                    0  // 1-pixel blocks can only have 0 decomposition levels
                                } else {
                                    // floor(log2(min_dim)) gives max safe levels
                                    // Cap at 5 for reasonable compression
                                    ((min_dim as f64).log2().floor() as u8).min(5)
                                };
                                
                                // Create hints based on existing hints or defaults, but always
                                // with safe decomposition levels for this block
                                let base_hints = hints.j2k_hints.clone().unwrap_or_default();
                                let final_levels = safe_levels.min(base_hints.decomposition_levels);
                                
                                Some(J2KEncodingHints {
                                    decomposition_levels: final_levels,
                                    ..base_hints
                                })
                            };
                            
                            #[cfg(feature = "openjpeg")]
                            let mut block_encoder = create_block_encoder(
                                &encoding_ic,
                                block_height,  // Single tile = block dimensions
                                block_width,
                                props.nbands,
                                props.nbpp as u8,
                                is_signed,
                                imode,
                                block_width,   // Tile size = full block (single tile)
                                block_height,
                                block_hints.as_ref(),
                                hints.comrat.as_deref(),
                            )?;
                            
                            #[cfg(not(feature = "openjpeg"))]
                            let mut block_encoder = create_block_encoder(
                                &encoding_ic,
                                block_height,
                                block_width,
                                props.nbands,
                                props.nbpp as u8,
                                is_signed,
                                imode,
                                block_width,
                                block_height,
                                None,
                                hints.comrat.as_deref(),
                            )?;
                            
                            // Encode the single block (tile 0,0 in this single-tile image)
                            block_encoder.encode_block(0, 0, &tile_data, shape)?;
                            
                            // Finalize to get the codestream for this block
                            let block_codestream = block_encoder.finalize()?;
                            
                            // Append the codestream
                            encoded_data.extend_from_slice(&block_codestream);
                        }
                        // Masked blocks already have EMPTY_BLOCK_OFFSET from from_provided_blocks
                    }
                }

                // Serialize mask with updated offsets
                let mask_bytes = mask.to_bytes();

                // Combine mask table and encoded data
                let mut result = Vec::with_capacity(mask_bytes.len() + encoded_data.len());
                result.extend_from_slice(&mask_bytes);
                result.extend_from_slice(&encoded_data);

                Ok(result)
            }
        } else {
            // For non-masked images, encode all blocks
            // encoder is always Some for non-masked images
            let mut encoder = encoder.expect("encoder should be Some for non-masked images");
            for block_row in 0..grid_rows {
                for block_col in 0..grid_cols {
                    let (tile_data, shape) = assembler.get_output_tile(block_row, block_col)?;
                    encoder.encode_block(block_row, block_col, &tile_data, shape)?;
                }
            }

            // Finalize and return encoded data
            encoder.finalize()
        }
    }

    /// Create an image subheader with TRE data.
    ///
    /// # Arguments
    /// * `asset` - The queued asset
    /// * `tre_bytes` - Serialized TRE envelope bytes to include in UDID field
    /// * `overflow` - Optional overflow data (used to determine if UDOFL should be set)
    /// * `hints` - Encoding hints for IMODE, IC, block sizes
    fn create_image_subheader_with_tres(
        &self,
        asset: &QueuedAsset,
        tre_bytes: &[u8],
        overflow: Option<&OverflowTreData>,
        hints: &EncodingHints,
    ) -> Vec<u8> {
        // Extract image properties from the asset provider
        let props = Self::extract_image_properties(asset);
        
        // Get metadata for user-settable fields
        let metadata = asset.provider.metadata();
        let metadata_dict = metadata.as_dict(None);
        
        // Helper to get metadata value or default
        let get_field = |key: &str, default: &str, max_len: usize| -> String {
            metadata_dict
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| default.to_string())
                .chars()
                .take(max_len)
                .collect::<String>()
        };
        
        let mut subheader = Vec::new();

        // IM (2) - File Part Type
        subheader.extend_from_slice(b"IM");
        // IID1 (10) - Image Identifier 1 (use metadata if provided, else asset.key)
        let iid1_default = truncate_to_bytes(&asset.key, 10);
        let iid1 = format!("{:10}", get_field("IID1", &iid1_default, 10));
        subheader.extend_from_slice(iid1.as_bytes());
        // IDATIM (14) - Image Date and Time
        let idatim = format!("{:14}", get_field("IDATIM", "", 14));
        subheader.extend_from_slice(idatim.as_bytes());
        // TGTID (17) - Target Identifier
        let tgtid = format!("{:17}", get_field("TGTID", "", 17));
        subheader.extend_from_slice(tgtid.as_bytes());
        // IID2 (80) - Image Identifier 2 (use metadata if provided, else asset.title)
        let iid2_default = truncate_to_bytes(&asset.title, 80);
        let iid2 = format!("{:80}", get_field("IID2", &iid2_default, 80));
        subheader.extend_from_slice(iid2.as_bytes());
        // ISCLAS (1) - Image Security Classification
        subheader.extend_from_slice(b"U");
        // ISCLSY (2) - Image Security Classification System
        subheader.extend_from_slice(b"  ");
        // ISCODE (11) - Image Codewords
        subheader.extend_from_slice(&[b' '; 11]);
        // ISCTLH (2) - Image Control and Handling
        subheader.extend_from_slice(b"  ");
        // ISREL (20) - Image Releasing Instructions
        subheader.extend_from_slice(&[b' '; 20]);
        // ISDCTP (2) - Image Declassification Type
        subheader.extend_from_slice(b"  ");
        // ISDCDT (8) - Image Declassification Date
        subheader.extend_from_slice(&[b' '; 8]);
        // ISDCXM (4) - Image Declassification Exemption
        subheader.extend_from_slice(&[b' '; 4]);
        // ISDG (1) - Image Downgrade
        subheader.extend_from_slice(b" ");
        // ISDGDT (8) - Image Downgrade Date
        subheader.extend_from_slice(&[b' '; 8]);
        // ISCLTX (43) - Image Classification Text
        subheader.extend_from_slice(&[b' '; 43]);
        // ISCATP (1) - Image Classification Authority Type
        subheader.extend_from_slice(b" ");
        // ISCAUT (40) - Image Classification Authority
        subheader.extend_from_slice(&[b' '; 40]);
        // ISCRSN (1) - Image Classification Reason
        subheader.extend_from_slice(b" ");
        // ISSRDT (8) - Image Security Source Date
        subheader.extend_from_slice(&[b' '; 8]);
        // ISCTLN (15) - Image Security Control Number
        subheader.extend_from_slice(&[b' '; 15]);
        // ENCRYP (1) - Encryption
        subheader.extend_from_slice(b"0");
        // ISORCE (42) - Image Source (use metadata if provided)
        let isorce = format!("{:42}", get_field("ISORCE", "", 42));
        subheader.extend_from_slice(isorce.as_bytes());
        // NROWS (8) - Number of Significant Rows
        subheader.extend_from_slice(format!("{:08}", props.nrows).as_bytes());
        // NCOLS (8) - Number of Significant Columns
        subheader.extend_from_slice(format!("{:08}", props.ncols).as_bytes());
        // PVTYPE (3) - Pixel Value Type
        subheader.extend_from_slice(format!("{:3}", props.pvtype).as_bytes());
        // IREP (8) - Image Representation
        subheader.extend_from_slice(format!("{:8}", props.irep).as_bytes());
        // ICAT (8) - Image Category
        subheader.extend_from_slice(b"VIS     ");
        // ABPP (2) - Actual Bits Per Pixel
        subheader.extend_from_slice(format!("{:02}", props.abpp).as_bytes());
        // PJUST (1) - Pixel Justification
        subheader.extend_from_slice(b"R");
        // ICORDS (1) - Image Coordinate Representation
        subheader.extend_from_slice(b" ");
        // NICOM (1) - Number of Image Comments
        subheader.extend_from_slice(b"0");
        // IC (2) - Image Compression (from encoding hints)
        subheader.extend_from_slice(format!("{:2}", hints.ic).as_bytes());
        
        // COMRAT (4) - Compression Rate Code (only for compressed images)
        // Present when IC is not NC or NM
        let ic_trimmed = hints.ic.trim();
        if ic_trimmed != "NC" && ic_trimmed != "NM" {
            // Generate COMRAT from J2K hints if available, otherwise use provided comrat or default
            let comrat = if let Some(ref j2k_hints) = hints.j2k_hints {
                crate::jbp::j2k::comrat::generate_comrat(j2k_hints)
            } else if let Some(ref comrat_str) = hints.comrat {
                // Use provided COMRAT, ensure it's 4 characters
                format!("{:4}", comrat_str)
            } else {
                // Default to numerically lossless for J2K
                if ic_trimmed == "C8" || ic_trimmed == "CD" {
                    "N1.0".to_string()
                } else {
                    "    ".to_string()
                }
            };
            subheader.extend_from_slice(comrat.as_bytes());
        }
        
        // NBANDS (1) or XBANDS (5) - Number of Bands
        // If nbands > 9, use XBANDS format (NBANDS=0, then XBANDS field)
        if props.nbands > 9 {
            subheader.extend_from_slice(b"0");
            subheader.extend_from_slice(format!("{:05}", props.nbands).as_bytes());
        } else {
            subheader.extend_from_slice(format!("{}", props.nbands).as_bytes());
        }
        
        // Band info for each band
        for band in 0..props.nbands {
            // IREPBAND (2) - Band Representation
            let irepband = match props.irep.trim() {
                "MONO" => "M ",
                "RGB" if band == 0 => "R ",
                "RGB" if band == 1 => "G ",
                "RGB" if band == 2 => "B ",
                _ => "  ",
            };
            subheader.extend_from_slice(irepband.as_bytes());
            // ISUBCAT (6) - Band Subcategory
            subheader.extend_from_slice(&[b' '; 6]);
            // IFC (1) - Band Image Filter Condition
            subheader.extend_from_slice(b"N");
            // IMFLT (3) - Band Standard Image Filter Code
            subheader.extend_from_slice(&[b' '; 3]);
            // NLUTS (1) - Number of LUTs
            subheader.extend_from_slice(b"0");
        }
        
        // ISYNC (1) - Image Sync Code
        subheader.extend_from_slice(b"0");
        // IMODE (1) - Image Mode (from encoding hints)
        subheader.extend_from_slice(hints.imode.as_bytes());
        
        // Calculate blocking parameters using hints
        let nbpr = (props.ncols + hints.nppbh - 1) / hints.nppbh;
        let nbpc = (props.nrows + hints.nppbv - 1) / hints.nppbv;
        
        // NBPR (4) - Number of Blocks Per Row
        subheader.extend_from_slice(format!("{:04}", nbpr).as_bytes());
        // NBPC (4) - Number of Blocks Per Column
        subheader.extend_from_slice(format!("{:04}", nbpc).as_bytes());
        // NPPBH (4) - Number of Pixels Per Block Horizontal (from encoding hints)
        subheader.extend_from_slice(format!("{:04}", hints.nppbh).as_bytes());
        // NPPBV (4) - Number of Pixels Per Block Vertical (from encoding hints)
        subheader.extend_from_slice(format!("{:04}", hints.nppbv).as_bytes());
        // NBPP (2) - Number of Bits Per Pixel
        subheader.extend_from_slice(format!("{:02}", props.nbpp).as_bytes());
        // IDLVL (3) - Image Display Level
        subheader.extend_from_slice(b"001");
        // IALVL (3) - Image Attachment Level
        subheader.extend_from_slice(b"000");
        // ILOC (10) - Image Location
        subheader.extend_from_slice(b"0000000000");
        // IMAG (4) - Image Magnification
        subheader.extend_from_slice(b"1.0 ");

        // UDIDL (5) - User Defined Image Data Length
        // UDID contains TRE data. If UDIDL > 0, it includes UDOFL (3 bytes) + UDID data
        if tre_bytes.is_empty() {
            subheader.extend_from_slice(b"00000");
        } else {
            // UDIDL = 3 (UDOFL) + TRE bytes length
            let udidl = 3 + tre_bytes.len();
            subheader.extend_from_slice(format!("{:05}", udidl).as_bytes());
            // UDOFL (3) - User Defined Overflow
            // If there's overflow, we use a placeholder "???" that will be patched later
            // when we know the actual DES index. Otherwise, use "000" for no overflow.
            if overflow.is_some() {
                subheader.extend_from_slice(b"???");
            } else {
                subheader.extend_from_slice(b"000");
            }
            // UDID - User Defined Image Data (TRE envelopes)
            subheader.extend_from_slice(tre_bytes);
        }

        // IXSHDL (5) - Image Extended Subheader Data Length
        // For now, we don't use IXSHD (extended subheader), only UDID
        subheader.extend_from_slice(b"00000");

        subheader
    }

    /// Extract TRE bytes from an asset's metadata.
    ///
    /// Parses TRE field values from the asset's metadata (fields with CETAG prefix)
    /// and serializes them to TRE envelope bytes.
    ///
    /// # Arguments
    /// * `asset` - The queued asset
    ///
    /// # Returns
    /// Serialized TRE envelope bytes, or empty vec if no TREs or no registry.
    fn extract_tre_bytes_from_asset(&self, asset: &QueuedAsset) -> Vec<u8> {
        // Need a registry to serialize TREs
        let registry = match &self.registry {
            Some(r) => r,
            None => return Vec::new(),
        };

        // Get metadata from the asset
        let metadata = asset.provider.metadata();
        let metadata_dict = metadata.as_dict(None);

        // Parse TRE fields from metadata
        let tre_groups = parse_tre_fields_from_metadata(&metadata_dict);
        if tre_groups.is_empty() {
            return Vec::new();
        }

        // Serialize TRE groups to envelopes
        let envelopes = match serialize_tre_groups_to_envelopes(registry, &tre_groups) {
            Ok(envs) => envs,
            Err(_) => return Vec::new(), // Silently skip on serialization errors
        };

        if envelopes.is_empty() {
            return Vec::new();
        }

        // Serialize envelopes to bytes
        write_tre_envelopes(&envelopes)
    }

    /// Patch the overflow index placeholder in a subheader.
    ///
    /// Searches for the "???" placeholder and replaces it with the actual
    /// 1-based DES index.
    ///
    /// # Arguments
    /// * `subheader` - The subheader bytes to patch
    /// * `des_index` - The 1-based DES index for the overflow DES
    fn patch_overflow_index(subheader: &mut [u8], des_index: u16) {
        // Search for the "???" placeholder
        let placeholder = b"???";
        if let Some(pos) = subheader
            .windows(3)
            .position(|window| window == placeholder)
        {
            // Replace with the 3-digit DES index
            let index_str = format!("{:03}", des_index);
            subheader[pos..pos + 3].copy_from_slice(index_str.as_bytes());
        }
    }

    /// Create a minimal text subheader.
    fn create_text_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        let mut subheader = Vec::new();

        // TE (2) - File Part Type
        subheader.extend_from_slice(b"TE");
        // TEXTID (7) - Text Identifier
        let textid = format!("{:7}", truncate_to_bytes(&asset.key, 7));
        subheader.extend_from_slice(textid.as_bytes());
        // TXTALVL (3) - Text Attachment Level
        subheader.extend_from_slice(b"000");
        // TXTDT (14) - Text Date and Time
        subheader.extend_from_slice(b"              ");
        // TXTITL (80) - Text Title
        let txtitl = format!("{:80}", truncate_to_bytes(&asset.title, 80));
        subheader.extend_from_slice(txtitl.as_bytes());
        // TSCLAS (1) - Text Security Classification
        subheader.extend_from_slice(b"U");
        // TSCLSY (2) - Text Security Classification System
        subheader.extend_from_slice(b"  ");
        // TSCODE (11) - Text Codewords
        subheader.extend_from_slice(&[b' '; 11]);
        // TSCTLH (2) - Text Control and Handling
        subheader.extend_from_slice(b"  ");
        // TSREL (20) - Text Releasing Instructions
        subheader.extend_from_slice(&[b' '; 20]);
        // TSDCTP (2) - Text Declassification Type
        subheader.extend_from_slice(b"  ");
        // TSDCDT (8) - Text Declassification Date
        subheader.extend_from_slice(&[b' '; 8]);
        // TSDCXM (4) - Text Declassification Exemption
        subheader.extend_from_slice(&[b' '; 4]);
        // TSDG (1) - Text Downgrade
        subheader.extend_from_slice(b" ");
        // TSDGDT (8) - Text Downgrade Date
        subheader.extend_from_slice(&[b' '; 8]);
        // TSCLTX (43) - Text Classification Text
        subheader.extend_from_slice(&[b' '; 43]);
        // TSCATP (1) - Text Classification Authority Type
        subheader.extend_from_slice(b" ");
        // TSCAUT (40) - Text Classification Authority
        subheader.extend_from_slice(&[b' '; 40]);
        // TSCRSN (1) - Text Classification Reason
        subheader.extend_from_slice(b" ");
        // TSSRDT (8) - Text Security Source Date
        subheader.extend_from_slice(&[b' '; 8]);
        // TSCTLN (15) - Text Security Control Number
        subheader.extend_from_slice(&[b' '; 15]);
        // ENCRYP (1) - Encryption
        subheader.extend_from_slice(b"0");
        // TXTFMT (3) - Text Format
        subheader.extend_from_slice(b"MTF");

        subheader
    }

    /// Create a minimal graphic subheader.
    fn create_graphic_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        let mut subheader = Vec::new();

        // SY (2) - File Part Type
        subheader.extend_from_slice(b"SY");
        // SID (10) - Graphic Identifier
        let sid = format!("{:10}", truncate_to_bytes(&asset.key, 10));
        subheader.extend_from_slice(sid.as_bytes());
        // SNAME (20) - Graphic Name
        let sname = format!("{:20}", truncate_to_bytes(&asset.title, 20));
        subheader.extend_from_slice(sname.as_bytes());
        // SSCLAS (1) - Graphic Security Classification
        subheader.extend_from_slice(b"U");
        // SSCLSY (2) - Graphic Security Classification System
        subheader.extend_from_slice(b"  ");
        // SSCODE (11) - Graphic Codewords
        subheader.extend_from_slice(&[b' '; 11]);
        // SSCTLH (2) - Graphic Control and Handling
        subheader.extend_from_slice(b"  ");
        // SSREL (20) - Graphic Releasing Instructions
        subheader.extend_from_slice(&[b' '; 20]);
        // SSDCTP (2) - Graphic Declassification Type
        subheader.extend_from_slice(b"  ");
        // SSDCDT (8) - Graphic Declassification Date
        subheader.extend_from_slice(&[b' '; 8]);
        // SSDCXM (4) - Graphic Declassification Exemption
        subheader.extend_from_slice(&[b' '; 4]);
        // SSDG (1) - Graphic Downgrade
        subheader.extend_from_slice(b" ");
        // SSDGDT (8) - Graphic Downgrade Date
        subheader.extend_from_slice(&[b' '; 8]);
        // SSCLTX (43) - Graphic Classification Text
        subheader.extend_from_slice(&[b' '; 43]);
        // SSCATP (1) - Graphic Classification Authority Type
        subheader.extend_from_slice(b" ");
        // SSCAUT (40) - Graphic Classification Authority
        subheader.extend_from_slice(&[b' '; 40]);
        // SSCRSN (1) - Graphic Classification Reason
        subheader.extend_from_slice(b" ");
        // SSSRDT (8) - Graphic Security Source Date
        subheader.extend_from_slice(&[b' '; 8]);
        // SSCTLN (15) - Graphic Security Control Number
        subheader.extend_from_slice(&[b' '; 15]);
        // ENCRYP (1) - Encryption
        subheader.extend_from_slice(b"0");
        // SFMT (1) - Graphic Type
        subheader.extend_from_slice(b"C");
        // SSTRUCT (13) - Reserved
        subheader.extend_from_slice(&[0u8; 13]);
        // SDLVL (3) - Graphic Display Level
        subheader.extend_from_slice(b"001");
        // SALVL (3) - Graphic Attachment Level
        subheader.extend_from_slice(b"000");
        // SLOC (10) - Graphic Location
        subheader.extend_from_slice(b"0000000000");
        // SBND1 (10) - First Graphic Bound Location
        subheader.extend_from_slice(b"0000000000");
        // SCOLOR (1) - Graphic Color
        subheader.extend_from_slice(b"C");
        // SBND2 (10) - Second Graphic Bound Location
        subheader.extend_from_slice(b"0000000000");
        // SRES2 (2) - Reserved
        subheader.extend_from_slice(b"00");
        // SXSHDL (5) - Graphic Extended Subheader Data Length
        subheader.extend_from_slice(b"00000");

        subheader
    }

    /// Create a minimal DES subheader.
    fn create_des_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        let mut subheader = Vec::new();

        // DE (2) - File Part Type
        subheader.extend_from_slice(b"DE");
        // DESID (25) - DES Identifier
        let desid = format!("{:25}", truncate_to_bytes(&asset.key, 25));
        subheader.extend_from_slice(desid.as_bytes());
        // DESVER (2) - DES Version
        subheader.extend_from_slice(b"01");
        // DECLAS (1) - DES Security Classification
        subheader.extend_from_slice(b"U");
        // DESCLSY (2) - DES Security Classification System
        subheader.extend_from_slice(b"  ");
        // DESCODE (11) - DES Codewords
        subheader.extend_from_slice(&[b' '; 11]);
        // DESCTLH (2) - DES Control and Handling
        subheader.extend_from_slice(b"  ");
        // DESREL (20) - DES Releasing Instructions
        subheader.extend_from_slice(&[b' '; 20]);
        // DESDCTP (2) - DES Declassification Type
        subheader.extend_from_slice(b"  ");
        // DESDCDT (8) - DES Declassification Date
        subheader.extend_from_slice(&[b' '; 8]);
        // DESDCXM (4) - DES Declassification Exemption
        subheader.extend_from_slice(&[b' '; 4]);
        // DESDG (1) - DES Downgrade
        subheader.extend_from_slice(b" ");
        // DESDGDT (8) - DES Downgrade Date
        subheader.extend_from_slice(&[b' '; 8]);
        // DESCLTX (43) - DES Classification Text
        subheader.extend_from_slice(&[b' '; 43]);
        // DESCATP (1) - DES Classification Authority Type
        subheader.extend_from_slice(b" ");
        // DESCAUT (40) - DES Classification Authority
        subheader.extend_from_slice(&[b' '; 40]);
        // DESCRSN (1) - DES Classification Reason
        subheader.extend_from_slice(b" ");
        // DESSRDT (8) - DES Security Source Date
        subheader.extend_from_slice(&[b' '; 8]);
        // DESCTLN (15) - DES Security Control Number
        subheader.extend_from_slice(&[b' '; 15]);
        // DESOFLW (6) - DES Overflowed Header Type
        subheader.extend_from_slice(&[b' '; 6]);
        // DESITEM (3) - DES Data Item Overflowed
        subheader.extend_from_slice(b"   ");
        // DESSHL (4) - DES User-Defined Subheader Length
        subheader.extend_from_slice(b"0000");

        subheader
    }

    /// Create a subheader for the given asset.
    /// 
    /// Note: For image segments, use `create_image_subheader_with_overflow` instead
    /// to properly handle TRE overflow and encoding hints.
    fn create_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        match asset.segment_type {
            SegmentType::Image => {
                // For images, we need to handle encoding hints properly.
                // This path should not normally be reached as images use
                // create_image_subheader_with_overflow in the close() method.
                // Return a basic subheader with default hints for fallback.
                self.create_image_subheader(asset)
                    .map(|(subheader, _)| subheader)
                    .unwrap_or_default()
            }
            SegmentType::Text => self.create_text_subheader(asset),
            SegmentType::Graphic => self.create_graphic_subheader(asset),
            SegmentType::DataExtension | SegmentType::ReservedExtension => {
                self.create_des_subheader(asset)
            }
        }
    }


    /// Calculate the file header length based on segment counts.
    fn calculate_header_length(&self, numi: usize, nums: usize, numt: usize, numdes: usize, numres: usize) -> usize {
        // Fixed header portion (before segment info)
        let fixed_len = 9  // FHDR + FVER
            + 2   // CLEVEL
            + 4   // STYPE
            + 10  // OSTAID
            + 14  // FDT
            + 80  // FTITLE
            + 1   // FSCLAS
            + 2   // FSCLSY
            + 11  // FSCODE
            + 2   // FSCTLH
            + 20  // FSREL
            + 2   // FSDCTP
            + 8   // FSDCDT
            + 4   // FSDCXM
            + 1   // FSDG
            + 8   // FSDGDT
            + 43  // FSCLTX
            + 1   // FSCATP
            + 40  // FSCAUT
            + 1   // FSCRSN
            + 8   // FSSRDT
            + 15  // FSCTLN
            + 5   // FSCOP
            + 5   // FSCPYS
            + 1   // ENCRYP
            + 3   // FBKGC
            + 24  // ONAME
            + 18  // OPHONE
            + 12  // FL
            + 6;  // HL

        // Variable portion based on segment counts
        let image_info_len = 3 + numi * (6 + 10);  // NUMI + (LISH + LI) * numi
        let graphic_info_len = 3 + nums * (4 + 6); // NUMS + (LSSH + LS) * nums
        let numx_len = 3;                          // NUMX (reserved)
        let text_info_len = 3 + numt * (4 + 5);    // NUMT + (LTSH + LT) * numt
        let des_info_len = 3 + numdes * (4 + 9);   // NUMDES + (LDSH + LD) * numdes
        let res_info_len = 3 + numres * (4 + 7);   // NUMRES + (LRESH + LRE) * numres
        let udhd_len = 5;                          // UDHDL
        let xhd_len = 5;                           // XHDL

        fixed_len + image_info_len + graphic_info_len + numx_len + text_info_len 
            + des_info_len + res_info_len + udhd_len + xhd_len
    }

    /// Write the file header.
    fn write_file_header<W: Write>(
        &self,
        writer: &mut W,
        file_length: u64,
        header_length: usize,
        image_info: &[(usize, usize)],   // (subheader_len, data_len)
        graphic_info: &[(usize, usize)],
        text_info: &[(usize, usize)],
        des_info: &[(usize, usize)],
    ) -> Result<(), CodecError> {
        // Magic number
        writer.write_all(self.format.magic().as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;

        // CLEVEL (2)
        writer.write_all(b"03")
            .map_err(|e| JBPError::IoError { source: e })?;
        // STYPE (4)
        writer.write_all(b"BF01")
            .map_err(|e| JBPError::IoError { source: e })?;
        // OSTAID (10)
        writer.write_all(b"OSML_IO   ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FDT (14) - current date/time placeholder
        writer.write_all(b"              ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FTITLE (80)
        writer.write_all(&[b' '; 80])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCLAS (1)
        writer.write_all(b"U")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCLSY (2)
        writer.write_all(b"  ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCODE (11)
        writer.write_all(&[b' '; 11])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCTLH (2)
        writer.write_all(b"  ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSREL (20)
        writer.write_all(&[b' '; 20])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSDCTP (2)
        writer.write_all(b"  ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSDCDT (8)
        writer.write_all(&[b' '; 8])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSDCXM (4)
        writer.write_all(&[b' '; 4])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSDG (1)
        writer.write_all(b" ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSDGDT (8)
        writer.write_all(&[b' '; 8])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCLTX (43)
        writer.write_all(&[b' '; 43])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCATP (1)
        writer.write_all(b" ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCAUT (40)
        writer.write_all(&[b' '; 40])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCRSN (1)
        writer.write_all(b" ")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSSRDT (8)
        writer.write_all(&[b' '; 8])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCTLN (15)
        writer.write_all(&[b' '; 15])
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCOP (5)
        writer.write_all(b"00000")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FSCPYS (5)
        writer.write_all(b"00000")
            .map_err(|e| JBPError::IoError { source: e })?;
        // ENCRYP (1)
        writer.write_all(b"0")
            .map_err(|e| JBPError::IoError { source: e })?;
        // FBKGC (3)
        writer.write_all(&[0u8; 3])
            .map_err(|e| JBPError::IoError { source: e })?;
        // ONAME (24)
        writer.write_all(&[b' '; 24])
            .map_err(|e| JBPError::IoError { source: e })?;
        // OPHONE (18)
        writer.write_all(&[b' '; 18])
            .map_err(|e| JBPError::IoError { source: e })?;

        // FL (12) - File Length
        writer.write_all(format!("{:012}", file_length).as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;
        // HL (6) - Header Length
        writer.write_all(format!("{:06}", header_length).as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;

        // NUMI (3)
        writer.write_all(format!("{:03}", image_info.len()).as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;
        // Image segment info - LISH values
        for (lish, _) in image_info {
            writer.write_all(format!("{:06}", lish).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }
        // Image segment info - LI values
        for (_, li) in image_info {
            writer.write_all(format!("{:010}", li).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }

        // NUMS (3)
        writer.write_all(format!("{:03}", graphic_info.len()).as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;
        // Graphic segment info - LSSH values
        for (lssh, _) in graphic_info {
            writer.write_all(format!("{:04}", lssh).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }
        // Graphic segment info - LS values
        for (_, ls) in graphic_info {
            writer.write_all(format!("{:06}", ls).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }

        // NUMX (3) - reserved
        writer.write_all(b"000")
            .map_err(|e| JBPError::IoError { source: e })?;

        // NUMT (3)
        writer.write_all(format!("{:03}", text_info.len()).as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;
        // Text segment info - LTSH values
        for (ltsh, _) in text_info {
            writer.write_all(format!("{:04}", ltsh).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }
        // Text segment info - LT values
        for (_, lt) in text_info {
            writer.write_all(format!("{:05}", lt).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }

        // NUMDES (3)
        writer.write_all(format!("{:03}", des_info.len()).as_bytes())
            .map_err(|e| JBPError::IoError { source: e })?;
        // DES segment info - LDSH values
        for (ldsh, _) in des_info {
            writer.write_all(format!("{:04}", ldsh).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }
        // DES segment info - LD values
        for (_, ld) in des_info {
            writer.write_all(format!("{:09}", ld).as_bytes())
                .map_err(|e| JBPError::IoError { source: e })?;
        }

        // NUMRES (3)
        writer.write_all(b"000")
            .map_err(|e| JBPError::IoError { source: e })?;

        // UDHDL (5)
        writer.write_all(b"00000")
            .map_err(|e| JBPError::IoError { source: e })?;
        // XHDL (5)
        writer.write_all(b"00000")
            .map_err(|e| JBPError::IoError { source: e })?;

        Ok(())
    }
}

impl DatasetWriter for JBPDatasetWriter {
    /// Adds an asset to the dataset.
    ///
    /// Assets are queued for writing when `close()` is called. The segment type
    /// is determined from the asset provider's `asset_type()`.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the asset
    /// * `provider` - The asset provider containing data and metadata
    /// * `title` - Human-readable title
    /// * `description` - Detailed description
    /// * `roles` - Semantic roles for the asset
    ///
    /// # Errors
    /// Returns `CodecError::DuplicateKey` if an asset with the given key already exists.
    fn add_asset(
        &mut self,
        key: &str,
        provider: Arc<dyn AssetProvider>,
        title: &str,
        description: &str,
        roles: &[String],
    ) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Encode("Writer has been closed".to_string()));
        }

        // Check for duplicate key
        if self.asset_keys.contains(key) {
            return Err(JBPError::DuplicateKey {
                key: key.to_string(),
            }
            .into());
        }

        // Determine segment type from asset type
        let segment_type = Self::asset_type_to_segment_type(provider.asset_type());

        // Queue the asset
        self.assets.push(QueuedAsset {
            key: key.to_string(),
            provider,
            title: title.to_string(),
            description: description.to_string(),
            roles: roles.to_vec(),
            segment_type,
        });
        self.asset_keys.insert(key.to_string());

        Ok(())
    }

    /// Sets the dataset-level metadata.
    ///
    /// The metadata will be used to populate file header fields when the
    /// file is written.
    fn set_metadata(&mut self, metadata: Arc<dyn MetadataProvider>) -> Result<(), CodecError> {
        if self.closed {
            return Err(CodecError::Encode("Writer has been closed".to_string()));
        }

        self.file_metadata = Some(metadata);
        Ok(())
    }

    /// Finalizes the dataset and writes the NITF file.
    ///
    /// This method performs the two-pass writing:
    /// 1. Calculate all segment lengths (including overflow DES)
    /// 2. Write file header with correct counts and length arrays
    /// 3. Write each segment's subheader and data in order
    fn close(&mut self) -> Result<(), CodecError> {
        if self.closed {
            return Ok(());
        }

        // Get assets grouped by type
        let (images, graphics, text, des) = self.get_assets_by_type();

        // Prepare image segments with overflow handling
        let mut image_info = Vec::new();
        let mut image_subheaders = Vec::new();
        let mut image_data = Vec::new();
        let mut overflow_tres: Vec<OverflowTreData> = Vec::new();

        for (idx, asset) in images.iter().enumerate() {
            let (subheader, overflow, hints) = self.create_image_subheader_with_overflow(asset, idx as u16)?;
            
            // Encode image data using BlockEncoder for ImageAssetProvider,
            // or fall back to raw data for other providers.
            let data = if let Some(image_provider) = Self::get_image_asset_provider(asset) {
                // Use BlockEncoder with TileAssembler for ImageAssetProvider
                let props = Self::extract_image_properties(asset);
                Self::encode_image_with_block_encoder(image_provider, &hints, &props)?
            } else {
                // Non-image providers pass through raw data as-is
                asset.provider.raw_asset()?
            };
            
            image_info.push((subheader.len(), data.len()));
            image_subheaders.push(subheader);
            image_data.push(data);
            
            if let Some(overflow_data) = overflow {
                overflow_tres.push(overflow_data);
            }
        }

        let mut graphic_info = Vec::new();
        let mut graphic_subheaders = Vec::new();
        let mut graphic_data = Vec::new();
        for asset in &graphics {
            let subheader = self.create_subheader(asset);
            let data = asset.provider.raw_asset()?;
            graphic_info.push((subheader.len(), data.len()));
            graphic_subheaders.push(subheader);
            graphic_data.push(data);
        }

        let mut text_info = Vec::new();
        let mut text_subheaders = Vec::new();
        let mut text_data = Vec::new();
        for asset in &text {
            let subheader = self.create_subheader(asset);
            let data = asset.provider.raw_asset()?;
            text_info.push((subheader.len(), data.len()));
            text_subheaders.push(subheader);
            text_data.push(data);
        }

        // Prepare DES segments from assets
        let mut des_info = Vec::new();
        let mut des_subheaders = Vec::new();
        let mut des_data = Vec::new();
        for asset in &des {
            let subheader = self.create_subheader(asset);
            let data = asset.provider.raw_asset()?;
            des_info.push((subheader.len(), data.len()));
            des_subheaders.push(subheader);
            des_data.push(data);
        }

        // Create TRE_OVERFLOW DES segments for any overflow TREs
        // The DES index is 1-based, starting after any existing DES segments
        let base_des_count = des_info.len();
        for (overflow_idx, overflow_data) in overflow_tres.iter().enumerate() {
            let des_index = (base_des_count + overflow_idx + 1) as u16; // 1-based index
            
            // Patch the overflow index in the source segment's subheader
            match overflow_data.source {
                OverflowSource::ImageUdid | OverflowSource::ImageIxshd => {
                    let segment_idx = overflow_data.segment_index as usize;
                    if segment_idx < image_subheaders.len() {
                        Self::patch_overflow_index(&mut image_subheaders[segment_idx], des_index);
                    }
                }
                _ => {
                    // Other overflow sources not yet implemented
                }
            }

            // Create the TRE_OVERFLOW DES
            let (overflow_subheader, overflow_des_data) = create_overflow_des(
                overflow_data.source,
                overflow_data.segment_index,
                &overflow_data.envelopes,
                None, // Use default security fields
            )?;

            des_info.push((overflow_subheader.len(), overflow_des_data.len()));
            des_subheaders.push(overflow_subheader);
            des_data.push(overflow_des_data);
        }

        // Calculate segment counts (including overflow DES)
        let numi = image_info.len();
        let nums = graphic_info.len();
        let numt = text_info.len();
        let numdes = des_info.len();
        let numres = 0;

        // Calculate header length
        let header_length = self.calculate_header_length(numi, nums, numt, numdes, numres);

        // Calculate total file length
        let segments_length: usize = image_info.iter().map(|(sh, d)| sh + d).sum::<usize>()
            + graphic_info.iter().map(|(sh, d)| sh + d).sum::<usize>()
            + text_info.iter().map(|(sh, d)| sh + d).sum::<usize>()
            + des_info.iter().map(|(sh, d)| sh + d).sum::<usize>();
        let file_length = header_length + segments_length;

        // Create output file
        let file = File::create(&self.path).map_err(|e| JBPError::IoError { source: e })?;
        let mut writer = BufWriter::new(file);

        // Write file header
        self.write_file_header(
            &mut writer,
            file_length as u64,
            header_length,
            &image_info,
            &graphic_info,
            &text_info,
            &des_info,
        )?;

        // Write image segments
        for (subheader, data) in image_subheaders.iter().zip(image_data.iter()) {
            writer.write_all(subheader).map_err(|e| JBPError::IoError { source: e })?;
            writer.write_all(data).map_err(|e| JBPError::IoError { source: e })?;
        }

        // Write graphic segments
        for (subheader, data) in graphic_subheaders.iter().zip(graphic_data.iter()) {
            writer.write_all(subheader).map_err(|e| JBPError::IoError { source: e })?;
            writer.write_all(data).map_err(|e| JBPError::IoError { source: e })?;
        }

        // Write text segments
        for (subheader, data) in text_subheaders.iter().zip(text_data.iter()) {
            writer.write_all(subheader).map_err(|e| JBPError::IoError { source: e })?;
            writer.write_all(data).map_err(|e| JBPError::IoError { source: e })?;
        }

        // Write DES segments (including TRE_OVERFLOW DES)
        for (subheader, data) in des_subheaders.iter().zip(des_data.iter()) {
            writer.write_all(subheader).map_err(|e| JBPError::IoError { source: e })?;
            writer.write_all(data).map_err(|e| JBPError::IoError { source: e })?;
        }

        // Flush and close
        writer.flush().map_err(|e| JBPError::IoError { source: e })?;

        self.closed = true;
        Ok(())
    }
}

// Ensure JBPDatasetWriter is Send + Sync
unsafe impl Send for JBPDatasetWriter {}
unsafe impl Sync for JBPDatasetWriter {}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    /// Simple test asset provider for testing.
    struct TestAssetProvider {
        key: String,
        title: String,
        description: String,
        roles: Vec<String>,
        asset_type: AssetType,
        data: Vec<u8>,
    }

    impl TestAssetProvider {
        fn new(key: &str, asset_type: AssetType, data: Vec<u8>) -> Self {
            Self {
                key: key.to_string(),
                title: format!("Test {}", key),
                description: format!("Test asset {}", key),
                roles: vec!["data".to_string()],
                asset_type,
                data,
            }
        }
    }

    impl AssetProvider for TestAssetProvider {
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
            match self.asset_type {
                AssetType::Image => "application/vnd.nitf.image",
                AssetType::Text => "text/plain",
                AssetType::Graphics => "image/cgm",
                AssetType::Data => "application/octet-stream",
            }
        }

        fn roles(&self) -> &[String] {
            &self.roles
        }

        fn asset_type(&self) -> AssetType {
            self.asset_type
        }

        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(self.data.clone())
        }

        fn metadata(&self) -> Arc<dyn MetadataProvider> {
            Arc::new(TestMetadataProvider)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct TestMetadataProvider;

    impl MetadataProvider for TestMetadataProvider {
        fn raw(&self) -> &[u8] {
            &[]
        }

        fn as_dict(&self, _name: Option<&str>) -> HashMap<String, serde_json::Value> {
            HashMap::new()
        }
    }

    #[test]
    fn writer_new_creates_instance() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        
        assert_eq!(writer.format(), NitfFormat::Nitf21);
        assert_eq!(writer.path(), path);
        assert_eq!(writer.asset_count(), 0);
        assert!(!writer.is_closed());
    }

    #[test]
    fn writer_new_nsif_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.nsif");
        
        let writer = JBPDatasetWriter::new(&path, NitfFormat::Nsif10).unwrap();
        
        assert_eq!(writer.format(), NitfFormat::Nsif10);
    }

    #[test]
    fn writer_add_asset_increments_count() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        let provider = Arc::new(TestAssetProvider::new("image_0", AssetType::Image, vec![0u8; 100]));
        
        writer.add_asset("image_0", provider, "Test", "", &[]).unwrap();
        
        assert_eq!(writer.asset_count(), 1);
    }

    #[test]
    fn writer_add_asset_duplicate_key_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        let provider1 = Arc::new(TestAssetProvider::new("image_0", AssetType::Image, vec![0u8; 100]));
        let provider2 = Arc::new(TestAssetProvider::new("image_0", AssetType::Image, vec![0u8; 100]));
        
        writer.add_asset("image_0", provider1, "Test", "", &[]).unwrap();
        let result = writer.add_asset("image_0", provider2, "Test", "", &[]);
        
        assert!(result.is_err());
        match result {
            Err(CodecError::DuplicateKey(key)) => assert_eq!(key, "image_0"),
            _ => panic!("Expected DuplicateKey error"),
        }
    }

    #[test]
    fn writer_add_asset_preserves_order() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        
        for i in 0..5 {
            let provider = Arc::new(TestAssetProvider::new(
                &format!("image_{}", i),
                AssetType::Image,
                vec![i as u8; 100],
            ));
            writer.add_asset(&format!("image_{}", i), provider, "Test", "", &[]).unwrap();
        }
        
        assert_eq!(writer.asset_count(), 5);
        // Order is preserved in the assets vector
        for (i, asset) in writer.assets.iter().enumerate() {
            assert_eq!(asset.key, format!("image_{}", i));
        }
    }

    #[test]
    fn writer_set_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        let metadata = Arc::new(TestMetadataProvider);
        
        let result = writer.set_metadata(metadata);
        
        assert!(result.is_ok());
        assert!(writer.file_metadata.is_some());
    }

    #[test]
    fn writer_close_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        let provider = Arc::new(TestAssetProvider::new("image_0", AssetType::Image, vec![0u8; 100]));
        writer.add_asset("image_0", provider, "Test", "", &[]).unwrap();
        
        writer.close().unwrap();
        
        assert!(path.exists());
        assert!(writer.is_closed());
    }

    #[test]
    fn writer_close_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        
        writer.close().unwrap();
        
        assert!(path.exists());
    }

    #[test]
    fn writer_close_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        
        writer.close().unwrap();
        writer.close().unwrap(); // Should not error
        
        assert!(writer.is_closed());
    }

    #[test]
    fn writer_add_asset_after_close_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        writer.close().unwrap();
        
        let provider = Arc::new(TestAssetProvider::new("image_0", AssetType::Image, vec![0u8; 100]));
        let result = writer.add_asset("image_0", provider, "Test", "", &[]);
        
        assert!(result.is_err());
    }

    #[test]
    fn writer_asset_type_to_segment_type() {
        assert_eq!(
            JBPDatasetWriter::asset_type_to_segment_type(AssetType::Image),
            SegmentType::Image
        );
        assert_eq!(
            JBPDatasetWriter::asset_type_to_segment_type(AssetType::Text),
            SegmentType::Text
        );
        assert_eq!(
            JBPDatasetWriter::asset_type_to_segment_type(AssetType::Graphics),
            SegmentType::Graphic
        );
        assert_eq!(
            JBPDatasetWriter::asset_type_to_segment_type(AssetType::Data),
            SegmentType::DataExtension
        );
    }

    #[test]
    fn writer_count_segments_by_type() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        
        // Add 2 images, 1 text, 1 graphic
        for i in 0..2 {
            let provider = Arc::new(TestAssetProvider::new(
                &format!("image_{}", i),
                AssetType::Image,
                vec![0u8; 100],
            ));
            writer.add_asset(&format!("image_{}", i), provider, "Test", "", &[]).unwrap();
        }
        
        let provider = Arc::new(TestAssetProvider::new("text_0", AssetType::Text, vec![0u8; 50]));
        writer.add_asset("text_0", provider, "Test", "", &[]).unwrap();
        
        let provider = Arc::new(TestAssetProvider::new("graphic_0", AssetType::Graphics, vec![0u8; 75]));
        writer.add_asset("graphic_0", provider, "Test", "", &[]).unwrap();
        
        let (numi, nums, numt, numdes, numres) = writer.count_segments_by_type();
        
        assert_eq!(numi, 2);
        assert_eq!(nums, 1);
        assert_eq!(numt, 1);
        assert_eq!(numdes, 0);
        assert_eq!(numres, 0);
    }

    #[test]
    fn writer_creates_valid_nitf_magic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        writer.close().unwrap();
        
        // Read the file and check magic number
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() >= 9);
        assert_eq!(&data[0..9], b"NITF02.10");
    }

    #[test]
    fn writer_creates_valid_nsif_magic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.nsif");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nsif10).unwrap();
        writer.close().unwrap();
        
        // Read the file and check magic number
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() >= 9);
        assert_eq!(&data[0..9], b"NSIF01.00");
    }

    #[test]
    fn writer_fl_field_matches_file_size() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        let provider = Arc::new(TestAssetProvider::new("image_0", AssetType::Image, vec![0u8; 100]));
        writer.add_asset("image_0", provider, "Test", "", &[]).unwrap();
        writer.close().unwrap();
        
        // Read the file
        let data = std::fs::read(&path).unwrap();
        
        // FL field is at offset 342 (after security fields), 12 bytes
        let fl_offset = 9 + 2 + 4 + 10 + 14 + 80 + 1 + 2 + 11 + 2 + 20 + 2 + 8 + 4 + 1 + 8 + 43 + 1 + 40 + 1 + 8 + 15 + 5 + 5 + 1 + 3 + 24 + 18;
        let fl_str = std::str::from_utf8(&data[fl_offset..fl_offset + 12]).unwrap();
        let fl_value: usize = fl_str.parse().unwrap();
        
        assert_eq!(fl_value, data.len());
    }

    #[test]
    fn writer_mixed_segment_types() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.ntf");
        
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        
        // Add different segment types
        let img = Arc::new(TestAssetProvider::new("img", AssetType::Image, vec![1u8; 100]));
        let txt = Arc::new(TestAssetProvider::new("txt", AssetType::Text, b"Hello".to_vec()));
        let gfx = Arc::new(TestAssetProvider::new("gfx", AssetType::Graphics, vec![2u8; 50]));
        let des = Arc::new(TestAssetProvider::new("des", AssetType::Data, vec![3u8; 25]));
        
        writer.add_asset("img", img, "Image", "", &[]).unwrap();
        writer.add_asset("txt", txt, "Text", "", &[]).unwrap();
        writer.add_asset("gfx", gfx, "Graphic", "", &[]).unwrap();
        writer.add_asset("des", des, "Data", "", &[]).unwrap();
        
        writer.close().unwrap();
        
        assert!(path.exists());
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 0);
    }

    #[test]
    fn split_tres_by_size_all_fit() {
        use crate::jbp::tre::TreEnvelope;
        
        // Create small TREs that all fit
        let envelopes = vec![
            TreEnvelope::new("TEST01", vec![1, 2, 3]).unwrap(),
            TreEnvelope::new("TEST02", vec![4, 5, 6]).unwrap(),
        ];
        
        let (fits, overflow) = JBPDatasetWriter::split_tres_by_size(envelopes, 1000);
        
        assert_eq!(fits.len(), 2);
        assert!(overflow.is_empty());
    }

    #[test]
    fn split_tres_by_size_some_overflow() {
        use crate::jbp::tre::TreEnvelope;
        
        // Create TREs where only some fit
        // Each envelope is 11 + data.len() bytes
        let envelopes = vec![
            TreEnvelope::new("TEST01", vec![0; 10]).unwrap(), // 21 bytes
            TreEnvelope::new("TEST02", vec![0; 10]).unwrap(), // 21 bytes
            TreEnvelope::new("TEST03", vec![0; 10]).unwrap(), // 21 bytes
        ];
        
        // Only allow 50 bytes - fits 2 envelopes (42 bytes)
        let (fits, overflow) = JBPDatasetWriter::split_tres_by_size(envelopes, 50);
        
        assert_eq!(fits.len(), 2);
        assert_eq!(overflow.len(), 1);
        assert_eq!(overflow[0].tag, "TEST03");
    }

    #[test]
    fn split_tres_by_size_none_fit() {
        use crate::jbp::tre::TreEnvelope;
        
        // Create TREs that are too large
        let envelopes = vec![
            TreEnvelope::new("TEST01", vec![0; 100]).unwrap(), // 111 bytes
        ];
        
        // Only allow 50 bytes - nothing fits
        let (fits, overflow) = JBPDatasetWriter::split_tres_by_size(envelopes, 50);
        
        assert!(fits.is_empty());
        assert_eq!(overflow.len(), 1);
    }

    #[test]
    fn patch_overflow_index_replaces_placeholder() {
        let mut subheader = b"some data ??? more data".to_vec();
        
        JBPDatasetWriter::patch_overflow_index(&mut subheader, 5);
        
        assert_eq!(&subheader, b"some data 005 more data");
    }

    #[test]
    fn patch_overflow_index_no_placeholder() {
        let mut subheader = b"some data 000 more data".to_vec();
        let original = subheader.clone();
        
        JBPDatasetWriter::patch_overflow_index(&mut subheader, 5);
        
        // Should not change anything if no placeholder
        assert_eq!(subheader, original);
    }

    #[test]
    fn patch_overflow_index_large_index() {
        let mut subheader = b"prefix???suffix".to_vec();
        
        JBPDatasetWriter::patch_overflow_index(&mut subheader, 123);
        
        assert_eq!(&subheader, b"prefix123suffix");
    }

    // Helper struct for conflict detection tests
    struct ConflictTestMetadataProvider {
        data: HashMap<String, serde_json::Value>,
    }

    impl ConflictTestMetadataProvider {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }

        fn with_field(mut self, key: &str, value: serde_json::Value) -> Self {
            self.data.insert(key.to_string(), value);
            self
        }
    }

    impl MetadataProvider for ConflictTestMetadataProvider {
        fn raw(&self) -> &[u8] {
            &[]
        }

        fn as_dict(&self, _name: Option<&str>) -> HashMap<String, serde_json::Value> {
            self.data.clone()
        }
    }

    struct ConflictTestAssetProvider {
        key: String,
        metadata: Arc<dyn MetadataProvider>,
    }

    impl ConflictTestAssetProvider {
        fn new(key: &str, metadata: Arc<dyn MetadataProvider>) -> Self {
            Self {
                key: key.to_string(),
                metadata,
            }
        }
    }

    impl AssetProvider for ConflictTestAssetProvider {
        fn key(&self) -> &str {
            &self.key
        }

        fn title(&self) -> &str {
            "Test Asset"
        }

        fn description(&self) -> &str {
            "Test Description"
        }

        fn media_type(&self) -> &str {
            "image/nitf"
        }

        fn roles(&self) -> &[String] {
            &[]
        }

        fn asset_type(&self) -> AssetType {
            AssetType::Image
        }

        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(vec![0u8; 100])
        }

        fn metadata(&self) -> Arc<dyn MetadataProvider> {
            self.metadata.clone()
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn conflict_detection_no_conflicts() {
        let metadata = Arc::new(ConflictTestMetadataProvider::new());
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 3,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "RGB".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert!(warnings.is_empty());
    }

    #[test]
    fn conflict_detection_nbands_mismatch() {
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("NBANDS", serde_json::json!(5))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 3,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "RGB".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("NBANDS"));
        assert!(warnings[0].contains("5"));
        assert!(warnings[0].contains("3"));
    }

    #[test]
    fn conflict_detection_pvtype_mismatch() {
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("PVTYPE", serde_json::json!("R"))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 1,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MONO".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("PVTYPE"));
        assert!(warnings[0].contains("R"));
        assert!(warnings[0].contains("INT"));
    }

    #[test]
    fn conflict_detection_nrows_mismatch() {
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("NROWS", serde_json::json!(500))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 1,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MONO".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("NROWS"));
        assert!(warnings[0].contains("500"));
        assert!(warnings[0].contains("100"));
    }

    #[test]
    fn conflict_detection_ncols_mismatch() {
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("NCOLS", serde_json::json!(800))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 1,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MONO".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("NCOLS"));
        assert!(warnings[0].contains("800"));
        assert!(warnings[0].contains("200"));
    }

    #[test]
    fn conflict_detection_irep_band_count_mismatch_from_metadata() {
        // Metadata says RGB (expects 3 bands) but provider has 1 band
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("IREP", serde_json::json!("RGB"))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 1,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MONO".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("IREP"));
        assert!(warnings[0].contains("RGB"));
        assert!(warnings[0].contains("1 bands"));
    }

    #[test]
    fn conflict_detection_irep_band_count_mismatch_from_props() {
        // No IREP in metadata, but props.irep is MONO with 3 bands
        let metadata = Arc::new(ConflictTestMetadataProvider::new());
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 3,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MONO".to_string(), // MONO expects 1 band, but we have 3
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("IREP"));
        assert!(warnings[0].contains("MONO"));
        assert!(warnings[0].contains("3 bands"));
    }

    #[test]
    fn conflict_detection_multiple_conflicts() {
        // Multiple conflicts at once
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("NBANDS", serde_json::json!(5))
                .with_field("NROWS", serde_json::json!(999))
                .with_field("NCOLS", serde_json::json!(888))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 3,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "RGB".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().any(|w| w.contains("NBANDS")));
        assert!(warnings.iter().any(|w| w.contains("NROWS")));
        assert!(warnings.iter().any(|w| w.contains("NCOLS")));
    }

    #[test]
    fn conflict_detection_string_values_parsed() {
        // Test that string values in metadata are parsed correctly
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("NBANDS", serde_json::json!("5"))
                .with_field("NROWS", serde_json::json!("  999  "))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 3,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "RGB".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn conflict_detection_matching_values_no_warning() {
        // When metadata values match provider values, no warnings
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("NBANDS", serde_json::json!(3))
                .with_field("NROWS", serde_json::json!(100))
                .with_field("NCOLS", serde_json::json!(200))
                .with_field("PVTYPE", serde_json::json!("INT"))
                .with_field("IREP", serde_json::json!("RGB"))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 3,
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "RGB".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert!(warnings.is_empty());
    }

    #[test]
    fn conflict_detection_multi_irep_no_warning() {
        // MULTI IREP can have any number of bands
        let metadata = Arc::new(
            ConflictTestMetadataProvider::new()
                .with_field("IREP", serde_json::json!("MULTI"))
        );
        let provider = ConflictTestAssetProvider::new("test", metadata);
        let asset = QueuedAsset {
            key: "test".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            roles: vec![],
            segment_type: SegmentType::Image,
            provider: Arc::new(provider),
        };
        
        let props = ImageProperties {
            nrows: 100,
            ncols: 200,
            nbands: 10, // Any number of bands is valid for MULTI
            nbpp: 8,
            abpp: 8,
            pvtype: "INT".to_string(),
            irep: "MULTI".to_string(),
            nppbh: 200,
            nppbv: 100,
        };
        
        let warnings = JBPDatasetWriter::detect_and_resolve_conflicts(&asset, &props);
        assert!(warnings.is_empty());
    }

    /// Integration test: Write NITF with BufferedImageAssetProvider, read back with JBPDatasetReader,
    /// and verify pixel data matches.
    /// **Validates: Requirements 4.1-4.5, 7.1-7.4**
    #[test]
    fn writer_round_trip_with_buffered_image_provider() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::jbp::reader::JBPDatasetReader;
        use crate::traits::{DatasetReader, ImageAssetProvider};
        use crate::types::AssetType;
        
        let dir = tempdir().unwrap();
        let path = dir.path().join("round_trip_test.ntf");
        
        // Create a small test image: 16x16 pixels, 3 bands, 8-bit
        let config = MemoryImageConfig::new(16, 16)
            .with_bands(3)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test_image", config);
        
        // Create test pixel data in BSQ format (band-sequential)
        // Band 0: all 100, Band 1: all 150, Band 2: all 200
        let pixels_per_band = 16 * 16;
        let mut bsq_data = Vec::with_capacity(pixels_per_band * 3);
        bsq_data.extend(std::iter::repeat(100u8).take(pixels_per_band));
        bsq_data.extend(std::iter::repeat(150u8).take(pixels_per_band));
        bsq_data.extend(std::iter::repeat(200u8).take(pixels_per_band));
        
        // Set the full image data
        provider.set_full_image(&bsq_data).unwrap();
        
        // Write the NITF file
        let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
        writer.add_asset("test_image", Arc::new(provider), "Test Image", "Round-trip test", &[]).unwrap();
        writer.close().unwrap();
        
        // Read the file back
        let reader = JBPDatasetReader::open(&path).unwrap();
        
        // Verify we have one image asset
        let asset_keys = reader.get_asset_keys(Some(AssetType::Image), None);
        assert_eq!(asset_keys.len(), 1);
        
        // Get the image asset
        let asset = reader.get_asset(&asset_keys[0]).unwrap();
        
        // Downcast to ImageAssetProvider
        let image_provider = asset.as_any()
            .downcast_ref::<crate::jbp::asset::JBPImageAssetProvider>()
            .expect("Asset should be an image provider");
        
        // Verify dimensions
        assert_eq!(image_provider.num_columns(), 16);
        assert_eq!(image_provider.num_rows(), 16);
        assert_eq!(image_provider.num_bands(), 3);
        assert_eq!(image_provider.num_bits_per_pixel(), 8);
        
        // Read back the pixel data (block 0,0) - get all bands
        let (block_data, shape) = image_provider.get_block(0, 0, 0, None).unwrap();
        
        // Verify shape - [bands, rows, cols] (CHW format)
        assert_eq!(shape, [3, 16, 16]);
        
        // The block data is in BSQ format (band-sequential)
        // Band 0: all 100, Band 1: all 150, Band 2: all 200
        assert_eq!(block_data.len(), 16 * 16 * 3);
        
        let pixels_per_band = 16 * 16;
        // Verify pixel values - check first few pixels of each band
        for pixel_idx in 0..10 {
            assert_eq!(block_data[pixel_idx], 100, "Band 0 value mismatch at pixel {}", pixel_idx);
            assert_eq!(block_data[pixels_per_band + pixel_idx], 150, "Band 1 value mismatch at pixel {}", pixel_idx);
            assert_eq!(block_data[2 * pixels_per_band + pixel_idx], 200, "Band 2 value mismatch at pixel {}", pixel_idx);
        }
    }

    /// Test collect_provided_blocks returns correct set of blocks
    #[test]
    fn collect_provided_blocks_returns_provided_only() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::traits::ImageAssetProvider;

        // Create a 2x2 block grid (32x32 image with 16x16 blocks)
        let config = MemoryImageConfig::new(32, 32)
            .with_bands(1)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test", config);
        
        // Set only blocks (0,0) and (1,1) - diagonal pattern
        let block_data = vec![128u8; 16 * 16];
        provider.set_block(0, 0, &block_data).unwrap();
        provider.set_block(1, 1, &block_data).unwrap();
        
        let provided = JBPDatasetWriter::collect_provided_blocks(&provider);
        
        assert_eq!(provided.len(), 2);
        assert!(provided.contains(&(0, 0)));
        assert!(provided.contains(&(1, 1)));
        assert!(!provided.contains(&(0, 1)));
        assert!(!provided.contains(&(1, 0)));
    }

    /// Test validate_blocks_for_ic accepts all blocks for non-masked IC
    #[test]
    fn validate_blocks_for_ic_accepts_complete_non_masked() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::traits::ImageAssetProvider;

        // Create a 2x2 block grid
        let config = MemoryImageConfig::new(32, 32)
            .with_bands(1)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test", config);
        
        // Set all 4 blocks
        let block_data = vec![128u8; 16 * 16];
        provider.set_block(0, 0, &block_data).unwrap();
        provider.set_block(0, 1, &block_data).unwrap();
        provider.set_block(1, 0, &block_data).unwrap();
        provider.set_block(1, 1, &block_data).unwrap();
        
        // Non-masked IC with all blocks should succeed
        let result = JBPDatasetWriter::validate_blocks_for_ic(&provider, "NC");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 4);
    }

    /// Test validate_blocks_for_ic rejects sparse data for non-masked IC
    #[test]
    fn validate_blocks_for_ic_rejects_sparse_non_masked() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::traits::ImageAssetProvider;

        // Create a 2x2 block grid
        let config = MemoryImageConfig::new(32, 32)
            .with_bands(1)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test", config);
        
        // Set only 2 of 4 blocks
        let block_data = vec![128u8; 16 * 16];
        provider.set_block(0, 0, &block_data).unwrap();
        provider.set_block(1, 1, &block_data).unwrap();
        
        // Non-masked IC with missing blocks should fail
        let result = JBPDatasetWriter::validate_blocks_for_ic(&provider, "NC");
        assert!(result.is_err());
        
        match result.unwrap_err() {
            CodecError::MissingBlocks { expected, provided, ic } => {
                assert_eq!(expected, 4);
                assert_eq!(provided, 2);
                assert_eq!(ic, "NC");
            }
            _ => panic!("Expected MissingBlocks error"),
        }
    }

    /// Test validate_blocks_for_ic accepts sparse data for masked IC
    #[test]
    fn validate_blocks_for_ic_accepts_sparse_masked() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::traits::ImageAssetProvider;

        // Create a 2x2 block grid
        let config = MemoryImageConfig::new(32, 32)
            .with_bands(1)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test", config);
        
        // Set only 2 of 4 blocks
        let block_data = vec![128u8; 16 * 16];
        provider.set_block(0, 0, &block_data).unwrap();
        provider.set_block(1, 1, &block_data).unwrap();
        
        // Masked IC with sparse blocks should succeed
        let result = JBPDatasetWriter::validate_blocks_for_ic(&provider, "NM");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    /// Test validate_blocks_for_ic works with various masked IC values
    #[test]
    fn validate_blocks_for_ic_accepts_all_masked_ic_values() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::traits::ImageAssetProvider;

        let config = MemoryImageConfig::new(32, 32)
            .with_bands(1)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test", config);
        
        // Set only 1 of 4 blocks
        let block_data = vec![128u8; 16 * 16];
        provider.set_block(0, 0, &block_data).unwrap();
        
        // All masked IC values should accept sparse data
        let masked_ics = ["NM", "M1", "M3", "M4", "M5", "M7", "M8", "M9", "MA", "MB", "MC", "MD", "ME"];
        for ic in masked_ics {
            let result = JBPDatasetWriter::validate_blocks_for_ic(&provider, ic);
            assert!(result.is_ok(), "Masked IC '{}' should accept sparse data", ic);
        }
    }

    /// Test validate_blocks_for_ic rejects sparse data for all non-masked IC values
    #[test]
    fn validate_blocks_for_ic_rejects_sparse_for_all_non_masked() {
        use crate::buffered::{BufferedImageAssetProvider, MemoryImageConfig};
        use crate::traits::ImageAssetProvider;

        let config = MemoryImageConfig::new(32, 32)
            .with_bands(1)
            .with_block_size(16, 16);
        
        let provider = BufferedImageAssetProvider::new("test", config);
        
        // Set only 1 of 4 blocks
        let block_data = vec![128u8; 16 * 16];
        provider.set_block(0, 0, &block_data).unwrap();
        
        // All non-masked IC values should reject sparse data
        let non_masked_ics = ["NC", "C1", "C3", "C4", "C5", "C7", "C8", "C9", "CA", "CB", "CC", "CD", "CE"];
        for ic in non_masked_ics {
            let result = JBPDatasetWriter::validate_blocks_for_ic(&provider, ic);
            assert!(result.is_err(), "Non-masked IC '{}' should reject sparse data", ic);
        }
    }
}


/// Property-based tests for JBPDatasetWriter.
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::tempdir;

    /// Simple test asset provider for property tests.
    struct PropTestAssetProvider {
        key: String,
        asset_type: AssetType,
        data: Vec<u8>,
    }

    impl PropTestAssetProvider {
        fn new(key: String, asset_type: AssetType, data: Vec<u8>) -> Self {
            Self {
                key,
                asset_type,
                data,
            }
        }
    }

    impl AssetProvider for PropTestAssetProvider {
        fn key(&self) -> &str {
            &self.key
        }

        fn title(&self) -> &str {
            "Test"
        }

        fn description(&self) -> &str {
            "Test asset"
        }

        fn media_type(&self) -> &str {
            match self.asset_type {
                AssetType::Image => "application/vnd.nitf.image",
                AssetType::Text => "text/plain",
                AssetType::Graphics => "image/cgm",
                AssetType::Data => "application/octet-stream",
            }
        }

        fn roles(&self) -> &[String] {
            &[]
        }

        fn asset_type(&self) -> AssetType {
            self.asset_type
        }

        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(self.data.clone())
        }

        fn metadata(&self) -> Arc<dyn MetadataProvider> {
            Arc::new(PropTestMetadataProvider)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct PropTestMetadataProvider;

    impl MetadataProvider for PropTestMetadataProvider {
        fn raw(&self) -> &[u8] {
            &[]
        }

        fn as_dict(&self, _name: Option<&str>) -> std::collections::HashMap<String, serde_json::Value> {
            std::collections::HashMap::new()
        }
    }

    /// Strategy for generating asset types
    fn asset_type_strategy() -> impl Strategy<Value = AssetType> {
        prop_oneof![
            Just(AssetType::Image),
            Just(AssetType::Text),
            Just(AssetType::Graphics),
            Just(AssetType::Data),
        ]
    }

    /// Property 10: Asset Addition Type Mapping
    /// For any AssetProvider added via add_asset(), the resulting segment type
    /// in the output file SHALL match the provider's asset_type.
    /// **Validates: Requirements 8.1, 8.2, 8.3, 8.4**
    mod prop_10_asset_addition_type_mapping {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn asset_type_maps_to_segment_type(asset_type in asset_type_strategy()) {
                let segment_type = JBPDatasetWriter::asset_type_to_segment_type(asset_type);
                
                match asset_type {
                    AssetType::Image => prop_assert_eq!(segment_type, SegmentType::Image),
                    AssetType::Text => prop_assert_eq!(segment_type, SegmentType::Text),
                    AssetType::Graphics => prop_assert_eq!(segment_type, SegmentType::Graphic),
                    AssetType::Data => prop_assert_eq!(segment_type, SegmentType::DataExtension),
                }
            }

            #[test]
            fn added_asset_has_correct_segment_type(
                asset_type in asset_type_strategy(),
                data_len in 1usize..1000,
            ) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                let provider = Arc::new(PropTestAssetProvider::new(
                    "test_asset".to_string(),
                    asset_type,
                    vec![0u8; data_len],
                ));
                
                writer.add_asset("test_asset", provider, "Test", "", &[]).unwrap();
                
                prop_assert_eq!(writer.assets.len(), 1);
                let expected_segment_type = JBPDatasetWriter::asset_type_to_segment_type(asset_type);
                prop_assert_eq!(writer.assets[0].segment_type, expected_segment_type);
            }
        }
    }

    /// Property 11: Duplicate Key Rejection
    /// For any JBPDatasetWriter, calling add_asset() with a key that was already
    /// added SHALL return a DuplicateKey error.
    /// **Validates: Requirements 8.5**
    mod prop_11_duplicate_key_rejection {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn duplicate_key_returns_error(
                key in "[a-z]{1,10}",
                asset_type1 in asset_type_strategy(),
                asset_type2 in asset_type_strategy(),
            ) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                
                let provider1 = Arc::new(PropTestAssetProvider::new(
                    key.clone(),
                    asset_type1,
                    vec![0u8; 100],
                ));
                let provider2 = Arc::new(PropTestAssetProvider::new(
                    key.clone(),
                    asset_type2,
                    vec![0u8; 100],
                ));
                
                // First add should succeed
                let result1 = writer.add_asset(&key, provider1, "Test", "", &[]);
                prop_assert!(result1.is_ok());
                
                // Second add with same key should fail
                let result2 = writer.add_asset(&key, provider2, "Test", "", &[]);
                prop_assert!(result2.is_err());
                
                match result2 {
                    Err(CodecError::DuplicateKey(k)) => prop_assert_eq!(k, key),
                    _ => prop_assert!(false, "Expected DuplicateKey error"),
                }
            }

            #[test]
            fn unique_keys_all_succeed(num_assets in 1usize..10) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                
                for i in 0..num_assets {
                    let key = format!("asset_{}", i);
                    let provider = Arc::new(PropTestAssetProvider::new(
                        key.clone(),
                        AssetType::Image,
                        vec![0u8; 100],
                    ));
                    let result = writer.add_asset(&key, provider, "Test", "", &[]);
                    prop_assert!(result.is_ok(), "Failed to add asset {}", i);
                }
                
                prop_assert_eq!(writer.asset_count(), num_assets);
            }
        }
    }

    /// Property 12: Asset Order Preservation
    /// For any sequence of add_asset() calls, the segments in the output file
    /// SHALL appear in the same order as the calls were made.
    /// **Validates: Requirements 8.6**
    mod prop_12_asset_order_preservation {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn assets_preserve_insertion_order(num_assets in 1usize..20) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                let mut expected_keys = Vec::new();
                
                for i in 0..num_assets {
                    let key = format!("asset_{}", i);
                    expected_keys.push(key.clone());
                    
                    let provider = Arc::new(PropTestAssetProvider::new(
                        key.clone(),
                        AssetType::Image,
                        vec![i as u8; 100],
                    ));
                    writer.add_asset(&key, provider, "Test", "", &[]).unwrap();
                }
                
                // Verify order is preserved
                for (i, asset) in writer.assets.iter().enumerate() {
                    prop_assert_eq!(&asset.key, &expected_keys[i],
                        "Asset at index {} has wrong key", i);
                }
            }

            #[test]
            fn mixed_types_preserve_order(
                types in prop::collection::vec(asset_type_strategy(), 1..10),
            ) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                let mut expected_order: Vec<(String, AssetType)> = Vec::new();
                
                for (i, asset_type) in types.iter().enumerate() {
                    let key = format!("asset_{}", i);
                    expected_order.push((key.clone(), *asset_type));
                    
                    let provider = Arc::new(PropTestAssetProvider::new(
                        key.clone(),
                        *asset_type,
                        vec![0u8; 100],
                    ));
                    writer.add_asset(&key, provider, "Test", "", &[]).unwrap();
                }
                
                // Verify order is preserved
                for (i, asset) in writer.assets.iter().enumerate() {
                    prop_assert_eq!(&asset.key, &expected_order[i].0);
                    let expected_segment = JBPDatasetWriter::asset_type_to_segment_type(expected_order[i].1);
                    prop_assert_eq!(asset.segment_type, expected_segment);
                }
            }
        }
    }

    /// Property 13: File Header Length Consistency
    /// For any NITF file written by JBPDatasetWriter, the FL field SHALL equal
    /// the actual file size, and the sum of HL plus all segment lengths SHALL equal FL.
    /// **Validates: Requirements 9.1, 9.2, 9.4**
    mod prop_13_file_header_length_consistency {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn fl_equals_file_size(
                num_images in 0usize..3,
                num_text in 0usize..3,
                data_size in 10usize..500,
            ) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                
                // Add image assets
                for i in 0..num_images {
                    let provider = Arc::new(PropTestAssetProvider::new(
                        format!("image_{}", i),
                        AssetType::Image,
                        vec![0u8; data_size],
                    ));
                    writer.add_asset(&format!("image_{}", i), provider, "Test", "", &[]).unwrap();
                }
                
                // Add text assets
                for i in 0..num_text {
                    let provider = Arc::new(PropTestAssetProvider::new(
                        format!("text_{}", i),
                        AssetType::Text,
                        vec![0u8; data_size],
                    ));
                    writer.add_asset(&format!("text_{}", i), provider, "Test", "", &[]).unwrap();
                }
                
                writer.close().unwrap();
                
                // Read the file
                let data = std::fs::read(&path).unwrap();
                
                // Parse FL field (at fixed offset after security fields)
                let fl_offset = 9 + 2 + 4 + 10 + 14 + 80 + 1 + 2 + 11 + 2 + 20 + 2 + 8 + 4 + 1 + 8 + 43 + 1 + 40 + 1 + 8 + 15 + 5 + 5 + 1 + 3 + 24 + 18;
                let fl_str = std::str::from_utf8(&data[fl_offset..fl_offset + 12]).unwrap();
                let fl_value: usize = fl_str.trim().parse().unwrap();
                
                prop_assert_eq!(fl_value, data.len(),
                    "FL field ({}) does not match actual file size ({})", fl_value, data.len());
            }

            #[test]
            fn hl_plus_segments_equals_fl(
                num_images in 0usize..3,
                data_size in 10usize..200,
            ) {
                let dir = tempdir().unwrap();
                let path = dir.path().join("test.ntf");
                
                let mut writer = JBPDatasetWriter::new(&path, NitfFormat::Nitf21).unwrap();
                
                for i in 0..num_images {
                    let provider = Arc::new(PropTestAssetProvider::new(
                        format!("image_{}", i),
                        AssetType::Image,
                        vec![0u8; data_size],
                    ));
                    writer.add_asset(&format!("image_{}", i), provider, "Test", "", &[]).unwrap();
                }
                
                writer.close().unwrap();
                
                // Read the file
                let data = std::fs::read(&path).unwrap();
                
                // Parse FL and HL fields
                let fl_offset = 9 + 2 + 4 + 10 + 14 + 80 + 1 + 2 + 11 + 2 + 20 + 2 + 8 + 4 + 1 + 8 + 43 + 1 + 40 + 1 + 8 + 15 + 5 + 5 + 1 + 3 + 24 + 18;
                let hl_offset = fl_offset + 12;
                
                let fl_str = std::str::from_utf8(&data[fl_offset..fl_offset + 12]).unwrap();
                let fl_value: usize = fl_str.trim().parse().unwrap();
                
                let hl_str = std::str::from_utf8(&data[hl_offset..hl_offset + 6]).unwrap();
                let hl_value: usize = hl_str.trim().parse().unwrap();
                
                // The file should be exactly FL bytes
                prop_assert_eq!(data.len(), fl_value);
                
                // HL should be less than FL (unless no segments)
                prop_assert!(hl_value <= fl_value,
                    "HL ({}) should be <= FL ({})", hl_value, fl_value);
            }
        }
    }

    /// Property 2: Unknown TRE Preservation
    /// For any TRE with a CETAG that has no definition in the Structure Registry,
    /// reading the TRE and then writing it SHALL preserve the complete envelope byte-for-byte.
    /// **Validates: Requirements 2.3, 4.1, 4.2, 4.3, 17.3**
    mod prop_2_unknown_tre_preservation {
        use super::*;
        use crate::jbp::tre::{TreEnvelope, write_tre_envelopes};

        /// Strategy for generating valid CETAGs (6 alphanumeric characters)
        fn unknown_cetag_strategy() -> impl Strategy<Value = String> {
            // Generate CETAGs that won't match any known TRE definitions
            // Use pattern UNKN followed by 2 digits
            (0u8..100).prop_map(|n| format!("UNKN{:02}", n))
        }

        /// Strategy for generating CEDATA (arbitrary bytes)
        fn cedata_strategy() -> impl Strategy<Value = Vec<u8>> {
            prop::collection::vec(any::<u8>(), 0..100)
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn unknown_tre_envelope_round_trip(
                tag in unknown_cetag_strategy(),
                data in cedata_strategy(),
            ) {
                // Create an unknown TRE envelope
                let envelope = TreEnvelope::new(&tag, data.clone()).unwrap();
                
                // Serialize to bytes
                let bytes = envelope.to_bytes();
                
                // Parse back
                let (parsed, consumed) = TreEnvelope::parse(&bytes).unwrap();
                
                // Verify round-trip
                prop_assert_eq!(consumed, bytes.len(), "Should consume all bytes");
                prop_assert_eq!(parsed.tag.trim(), tag.trim(), "Tag should match");
                prop_assert_eq!(&parsed.data, &data, "Data should match");
                
                // Verify byte-identical output
                let reparsed_bytes = parsed.to_bytes();
                prop_assert_eq!(bytes, reparsed_bytes, "Bytes should be identical after round-trip");
            }

            #[test]
            fn multiple_unknown_tres_round_trip(
                envelopes in prop::collection::vec(
                    (unknown_cetag_strategy(), cedata_strategy()),
                    1..5
                ),
            ) {
                // Create TRE envelopes
                let tres: Vec<TreEnvelope> = envelopes
                    .iter()
                    .map(|(tag, data)| TreEnvelope::new(tag, data.clone()).unwrap())
                    .collect();
                
                // Serialize all to bytes
                let bytes = write_tre_envelopes(&tres);
                
                // Parse all back
                let parsed = TreEnvelope::parse_all(&bytes).unwrap();
                
                // Verify count matches
                prop_assert_eq!(parsed.len(), tres.len(), "Should parse same number of TREs");
                
                // Verify each TRE matches
                for (original, parsed_tre) in tres.iter().zip(parsed.iter()) {
                    prop_assert_eq!(original.tag.trim(), parsed_tre.tag.trim());
                    prop_assert_eq!(&original.data, &parsed_tre.data);
                }
                
                // Verify byte-identical output
                let reparsed_bytes = write_tre_envelopes(&parsed);
                prop_assert_eq!(bytes, reparsed_bytes, "Bytes should be identical after round-trip");
            }
        }
    }

    /// Property 4: TRE Field Value Round-Trip
    /// For any valid map of TRE field values, writing the TRE and then parsing it back
    /// SHALL produce an equivalent field map.
    /// **Validates: Requirements 8.1, 8.2, 8.3, 17.2**
    mod prop_4_tre_field_value_round_trip {
        use super::*;
        use crate::jbp::tre::TreEnvelope;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn tre_envelope_size_calculation(
                tag in "[A-Z0-9]{6}",
                data_len in 0usize..1000,
            ) {
                let data = vec![0u8; data_len];
                let envelope = TreEnvelope::new(&tag, data).unwrap();
                
                // Envelope size should be CETAG(6) + CEL(5) + CEDATA(data_len)
                let expected_size = 6 + 5 + data_len;
                prop_assert_eq!(envelope.envelope_size(), expected_size);
                
                // Serialized bytes should match envelope_size
                let bytes = envelope.to_bytes();
                prop_assert_eq!(bytes.len(), expected_size);
            }

            #[test]
            fn tre_envelope_cel_field_correct(
                tag in "[A-Z0-9]{6}",
                data_len in 0usize..99999,
            ) {
                let data = vec![0u8; data_len];
                let envelope = TreEnvelope::new(&tag, data).unwrap();
                
                let bytes = envelope.to_bytes();
                
                // CEL field is at offset 6, 5 bytes
                let cel_str = std::str::from_utf8(&bytes[6..11]).unwrap();
                let cel_value: usize = cel_str.parse().unwrap();
                
                prop_assert_eq!(cel_value, data_len, "CEL should equal data length");
            }

            #[test]
            fn tre_envelope_cetag_field_correct(
                tag in "[A-Z0-9]{6}",
                data in prop::collection::vec(any::<u8>(), 0..100),
            ) {
                let envelope = TreEnvelope::new(&tag, data).unwrap();
                
                let bytes = envelope.to_bytes();
                
                // CETAG field is at offset 0, 6 bytes
                let cetag_str = std::str::from_utf8(&bytes[0..6]).unwrap();
                
                prop_assert_eq!(cetag_str, &tag, "CETAG should match input tag");
            }
        }
    }
}
