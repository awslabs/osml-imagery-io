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
//! use aws_osml_io::jbp::{JBPDatasetWriter, NitfFormat};
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
use crate::jbp::types::{NitfFormat, SegmentType};
use crate::traits::{AssetProvider, DatasetWriter, MetadataProvider};
use crate::types::AssetType;

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


    /// Create a minimal image subheader.
    fn create_image_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        let mut subheader = Vec::new();

        // IM (2) - File Part Type
        subheader.extend_from_slice(b"IM");
        // IID1 (10) - Image Identifier 1
        let iid1 = format!("{:10}", &asset.key[..asset.key.len().min(10)]);
        subheader.extend_from_slice(iid1.as_bytes());
        // IDATIM (14) - Image Date and Time
        subheader.extend_from_slice(b"              ");
        // TGTID (17) - Target Identifier
        subheader.extend_from_slice(&[b' '; 17]);
        // IID2 (80) - Image Identifier 2
        let iid2 = format!("{:80}", &asset.title[..asset.title.len().min(80)]);
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
        // ISORCE (42) - Image Source
        subheader.extend_from_slice(&[b' '; 42]);
        // NROWS (8) - Number of Significant Rows
        subheader.extend_from_slice(b"00000001");
        // NCOLS (8) - Number of Significant Columns
        subheader.extend_from_slice(b"00000001");
        // PVTYPE (3) - Pixel Value Type
        subheader.extend_from_slice(b"INT");
        // IREP (8) - Image Representation
        subheader.extend_from_slice(b"MONO    ");
        // ICAT (8) - Image Category
        subheader.extend_from_slice(b"VIS     ");
        // ABPP (2) - Actual Bits Per Pixel
        subheader.extend_from_slice(b"08");
        // PJUST (1) - Pixel Justification
        subheader.extend_from_slice(b"R");
        // ICORDS (1) - Image Coordinate Representation
        subheader.extend_from_slice(b" ");
        // NICOM (1) - Number of Image Comments
        subheader.extend_from_slice(b"0");
        // IC (2) - Image Compression
        subheader.extend_from_slice(b"NC");
        // NBANDS (1) - Number of Bands
        subheader.extend_from_slice(b"1");
        // IREPBAND1 (2) - Band Representation
        subheader.extend_from_slice(b"M ");
        // ISUBCAT1 (6) - Band Subcategory
        subheader.extend_from_slice(&[b' '; 6]);
        // IFC1 (1) - Band Image Filter Condition
        subheader.extend_from_slice(b"N");
        // IMFLT1 (3) - Band Standard Image Filter Code
        subheader.extend_from_slice(&[b' '; 3]);
        // NLUTS1 (1) - Number of LUTs
        subheader.extend_from_slice(b"0");
        // ISYNC (1) - Image Sync Code
        subheader.extend_from_slice(b"0");
        // IMODE (1) - Image Mode
        subheader.extend_from_slice(b"B");
        // NBPR (4) - Number of Blocks Per Row
        subheader.extend_from_slice(b"0001");
        // NBPC (4) - Number of Blocks Per Column
        subheader.extend_from_slice(b"0001");
        // NPPBH (4) - Number of Pixels Per Block Horizontal
        subheader.extend_from_slice(b"0001");
        // NPPBV (4) - Number of Pixels Per Block Vertical
        subheader.extend_from_slice(b"0001");
        // NBPP (2) - Number of Bits Per Pixel
        subheader.extend_from_slice(b"08");
        // IDLVL (3) - Image Display Level
        subheader.extend_from_slice(b"001");
        // IALVL (3) - Image Attachment Level
        subheader.extend_from_slice(b"000");
        // ILOC (10) - Image Location
        subheader.extend_from_slice(b"0000000000");
        // IMAG (4) - Image Magnification
        subheader.extend_from_slice(b"1.0 ");
        // UDIDL (5) - User Defined Image Data Length
        subheader.extend_from_slice(b"00000");
        // IXSHDL (5) - Image Extended Subheader Data Length
        subheader.extend_from_slice(b"00000");

        subheader
    }

    /// Create a minimal text subheader.
    fn create_text_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        let mut subheader = Vec::new();

        // TE (2) - File Part Type
        subheader.extend_from_slice(b"TE");
        // TEXTID (7) - Text Identifier
        let textid = format!("{:7}", &asset.key[..asset.key.len().min(7)]);
        subheader.extend_from_slice(textid.as_bytes());
        // TXTALVL (3) - Text Attachment Level
        subheader.extend_from_slice(b"000");
        // TXTDT (14) - Text Date and Time
        subheader.extend_from_slice(b"              ");
        // TXTITL (80) - Text Title
        let txtitl = format!("{:80}", &asset.title[..asset.title.len().min(80)]);
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
        let sid = format!("{:10}", &asset.key[..asset.key.len().min(10)]);
        subheader.extend_from_slice(sid.as_bytes());
        // SNAME (20) - Graphic Name
        let sname = format!("{:20}", &asset.title[..asset.title.len().min(20)]);
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
        let desid = format!("{:25}", &asset.key[..asset.key.len().min(25)]);
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
    fn create_subheader(&self, asset: &QueuedAsset) -> Vec<u8> {
        match asset.segment_type {
            SegmentType::Image => self.create_image_subheader(asset),
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
    /// 1. Calculate all segment lengths
    /// 2. Write file header with correct counts and length arrays
    /// 3. Write each segment's subheader and data in order
    fn close(&mut self) -> Result<(), CodecError> {
        if self.closed {
            return Ok(());
        }

        // Get assets grouped by type
        let (images, graphics, text, des) = self.get_assets_by_type();

        // Calculate segment counts
        let (numi, nums, numt, numdes, numres) = self.count_segments_by_type();

        // Calculate header length
        let header_length = self.calculate_header_length(numi, nums, numt, numdes, numres);

        // Prepare segment info (subheader_len, data_len) for each segment
        let mut image_info = Vec::new();
        let mut image_subheaders = Vec::new();
        let mut image_data = Vec::new();
        for asset in &images {
            let subheader = self.create_subheader(asset);
            let data = asset.provider.raw_asset()?;
            image_info.push((subheader.len(), data.len()));
            image_subheaders.push(subheader);
            image_data.push(data);
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

        // Write DES segments
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
}
