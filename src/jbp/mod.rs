//! JBP (Joint BIIF Profile) dataset integration.
//!
//! This module provides NITF/NSIF file access through the DatasetReader/DatasetWriter
//! interfaces. It uses the data-driven binary parser infrastructure to parse and
//! generate NITF headers without hardcoding format details.
//!
//! # Key Components
//!
//! - [`JBPDatasetReader`] - Reader for NITF/NSIF files implementing DatasetReader
//! - [`JBPDatasetWriter`] - Writer for NITF/NSIF files implementing DatasetWriter
//! - [`NitfFormat`] - Detected format variant (NITF 2.1 or NSIF 1.0)
//! - [`SegmentOffsets`] - Pre-calculated segment offsets for direct access
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::{JBPDatasetReader, NitfFormat};
//!
//! let reader = JBPDatasetReader::open("image.ntf")?;
//! let keys = reader.get_asset_keys(None, None);
//! for key in keys {
//!     let asset = reader.get_asset(&key)?;
//!     println!("Asset: {} ({})", asset.key(), asset.media_type());
//! }
//! ```

pub mod asset;
pub mod datetime;
mod error;
pub mod format;
pub mod graphics;
pub mod image;
pub mod io;
pub mod j2k;
#[cfg(feature = "libjpeg-turbo")]
pub mod jpeg;
mod metadata;
pub mod overflow;
mod reader;
#[cfg(test)]
pub mod test_data_generator;
pub mod text;
pub mod tre;
pub mod tre_fields;
mod types;
mod writer;

pub use asset::{
    generate_asset_key, parse_asset_key, JBPDataAssetProvider, JBPGraphicsAssetProvider,
    JBPImageAssetProvider, JBPTextAssetProvider,
};
pub use datetime::{parse_nitf_datetime, DateTimeParseError, NitfDateTime};
pub use error::{JBPError, ValidationCode, ValidationWarning};
pub use format::{is_nitf_extension, validate_nitf_magic};
pub use graphics::GraphicSubheaderFacade;
pub use io::IO;
pub use metadata::{JBPFileMetadataProvider, JBPSegmentMetadataProvider};
pub use overflow::{create_overflow_des, OverflowSource};
pub use text::{create_text_subheader_definition, TextSubheaderFacade};
pub use tre::{
    extract_tre_fields_from_provider, parse_tre_fields_from_metadata, write_tre_envelopes,
    TreEnvelope, TreFieldGroup,
};
pub use types::{JBPReaderOptions, NitfFormat, SegmentLocation, SegmentOffsets, SegmentType};

// Re-export reader and writer
pub use reader::JBPDatasetReader;
pub use writer::JBPDatasetWriter;
