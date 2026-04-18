//! Facade pattern for typed access to image subheader fields.
//!
//! The [`ImageSubheaderFacade`] wraps a [`StructureAccessor`] to provide
//! convenient, typed access to image subheader fields. This pattern allows
//! the underlying structure definition to vary (e.g., NITF 2.0 vs 2.1)
//! while presenting a consistent API.
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::facade::ImageSubheaderFacade;
//! use osml_io::parser::StructureAccessor;
//!
//! let facade = ImageSubheaderFacade::new(accessor);
//! let nrows = facade.nrows()?;
//! let ncols = facade.ncols()?;
//! let pvtype = facade.pvtype()?;
//! ```

use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::image::types::{
    ImageRepresentation, InterleaveMode, PixelJustification, PixelValueType,
};
use crate::jbp::types::NitfFormat;
use crate::parser::{StructureAccessor, StructureRegistry, Value};

/// Facade providing typed access to image subheader fields via StructureAccessor.
///
/// This struct wraps a `StructureAccessor` and provides methods to access
/// image subheader fields with proper type conversion. The facade handles
/// the details of field naming and parsing, presenting a clean API for
/// accessing image metadata.
pub struct ImageSubheaderFacade<'a> {
    /// The underlying structure accessor
    accessor: StructureAccessor<'a>,
}

impl<'a> ImageSubheaderFacade<'a> {
    /// Create a facade from a StructureAccessor.
    ///
    /// # Arguments
    /// * `accessor` - The structure accessor for the image subheader
    ///
    /// # Returns
    /// A new `ImageSubheaderFacade` wrapping the accessor.
    pub fn new(accessor: StructureAccessor<'a>) -> Self {
        Self { accessor }
    }

    /// Create from raw bytes using the appropriate structure definition.
    ///
    /// # Arguments
    /// * `data` - Raw bytes of the image subheader
    /// * `registry` - Structure registry for looking up definitions
    /// * `format` - NITF format variant (determines which definition to use)
    ///
    /// # Returns
    /// A new `ImageSubheaderFacade` or an error if parsing fails.
    pub fn from_bytes(
        data: &'a [u8],
        registry: &StructureRegistry,
        format: NitfFormat,
    ) -> Result<Self, CodecError> {
        let def_name = format.image_subheader_definition();
        let definition = registry.get(def_name).ok_or_else(|| {
            CodecError::InvalidFormat(format!("Structure definition not found: {}", def_name))
        })?;

        let accessor = StructureAccessor::new(definition, data)
            .map_err(|e| CodecError::Parse(format!("Failed to create accessor: {}", e)))?;

        Ok(Self { accessor })
    }

    /// Get the underlying accessor for direct field access.
    ///
    /// This is useful when you need to access fields not exposed by the facade,
    /// or when you need to perform custom operations on the accessor.
    pub fn accessor(&self) -> &StructureAccessor<'a> {
        &self.accessor
    }

    // ==================== Identification Field Accessors ====================

    /// Get the image identifier 1 (IID1).
    ///
    /// This is a 10-character identifier for the image.
    pub fn iid1(&self) -> Result<String, CodecError> {
        self.get_str_field("IID1")
    }

    /// Get the image identifier 2 (IID2).
    ///
    /// This is an 80-character free-text identifier for the image.
    pub fn iid2(&self) -> Result<String, CodecError> {
        self.get_str_field("IID2")
    }

    /// Get the image date and time (IDATIM).
    ///
    /// This is a 14-character date/time string in CCYYMMDDhhmmss format.
    pub fn idatim(&self) -> Result<String, CodecError> {
        self.get_str_field("IDATIM")
    }

    /// Get the target identifier (TGTID).
    ///
    /// This is a 17-character target identifier.
    pub fn tgtid(&self) -> Result<String, CodecError> {
        self.get_str_field("TGTID")
    }

    /// Get the image source (ISORCE).
    ///
    /// This is a 42-character description of the image source.
    pub fn isorce(&self) -> Result<String, CodecError> {
        self.get_str_field("ISORCE")
    }

    // ==================== Dimension and Pixel Field Accessors ====================

    /// Get the number of significant rows in the image (NROWS).
    pub fn nrows(&self) -> Result<u32, CodecError> {
        self.get_u32_field("NROWS")
    }

    /// Get the number of significant columns in the image (NCOLS).
    pub fn ncols(&self) -> Result<u32, CodecError> {
        self.get_u32_field("NCOLS")
    }

    /// Get the pixel value type (PVTYPE).
    ///
    /// Returns the parsed `PixelValueType` enum.
    pub fn pvtype(&self) -> Result<PixelValueType, CodecError> {
        let s = self.get_str_field("PVTYPE")?;
        PixelValueType::from_str(&s)
    }

    /// Get the image representation (IREP).
    ///
    /// Returns the parsed `ImageRepresentation` enum.
    pub fn irep(&self) -> Result<ImageRepresentation, CodecError> {
        let s = self.get_str_field("IREP")?;
        ImageRepresentation::from_str(&s)
    }

    /// Get the image category (ICAT).
    ///
    /// This is an 8-character image category code.
    pub fn icat(&self) -> Result<String, CodecError> {
        self.get_str_field("ICAT")
    }

    /// Get the actual bits per pixel (ABPP).
    ///
    /// This is the number of significant bits in each pixel value.
    pub fn abpp(&self) -> Result<u8, CodecError> {
        self.get_u8_field("ABPP")
    }

    /// Get the number of bits per pixel (NBPP).
    ///
    /// This is the storage size for each pixel value.
    pub fn nbpp(&self) -> Result<u8, CodecError> {
        self.get_u8_field("NBPP")
    }

    /// Get the pixel justification (PJUST).
    ///
    /// Returns the parsed `PixelJustification` enum.
    pub fn pjust(&self) -> Result<PixelJustification, CodecError> {
        let s = self.get_str_field("PJUST")?;
        let c = s
            .chars()
            .next()
            .ok_or_else(|| CodecError::Parse("PJUST field is empty".to_string()))?;
        PixelJustification::from_char(c)
    }

    // ==================== Blocking Parameter Accessors ====================

    /// Get the number of blocks per row (NBPR).
    pub fn nbpr(&self) -> Result<u32, CodecError> {
        self.get_u32_field("NBPR")
    }

    /// Get the number of blocks per column (NBPC).
    pub fn nbpc(&self) -> Result<u32, CodecError> {
        self.get_u32_field("NBPC")
    }

    /// Get the raw number of pixels per block horizontal (NPPBH).
    ///
    /// Note: Per JBP spec section 5.13.2.35, when NBPR=1 and NPPBH=0, the actual
    /// block width is NCOLS. Use `effective_nppbh()` to get the interpreted value.
    pub fn nppbh(&self) -> Result<u32, CodecError> {
        self.get_u32_field("NPPBH")
    }

    /// Get the raw number of pixels per block vertical (NPPBV).
    ///
    /// Note: Per JBP spec section 5.13.2.36, when NBPC=1 and NPPBV=0, the actual
    /// block height is NROWS. Use `effective_nppbv()` to get the interpreted value.
    pub fn nppbv(&self) -> Result<u32, CodecError> {
        self.get_u32_field("NPPBV")
    }

    /// Get the effective number of pixels per block horizontal.
    ///
    /// Per JBP spec section 5.13.2.35: "When NBPR=0001, setting the NPPBH value
    /// 0000 designates that the number of pixels horizontally is specified by
    /// the value in NCOLS."
    ///
    /// This method returns NCOLS when NBPR=1 and NPPBH=0, otherwise returns NPPBH.
    pub fn effective_nppbh(&self) -> Result<u32, CodecError> {
        let nppbh = self.nppbh()?;
        if nppbh == 0 {
            let nbpr = self.nbpr()?;
            if nbpr == 1 {
                // Single block row with NPPBH=0 means block width = image width
                return self.ncols();
            }
        }
        Ok(nppbh)
    }

    /// Get the effective number of pixels per block vertical.
    ///
    /// Per JBP spec section 5.13.2.36: "When NBPC=0001, setting the NPPBV value
    /// 0000 designates that the number of pixels vertically is specified by
    /// the value in NROWS."
    ///
    /// This method returns NROWS when NBPC=1 and NPPBV=0, otherwise returns NPPBV.
    pub fn effective_nppbv(&self) -> Result<u32, CodecError> {
        let nppbv = self.nppbv()?;
        if nppbv == 0 {
            let nbpc = self.nbpc()?;
            if nbpc == 1 {
                // Single block column with NPPBV=0 means block height = image height
                return self.nrows();
            }
        }
        Ok(nppbv)
    }

    /// Get the image interleave mode (IMODE).
    ///
    /// Returns the parsed `InterleaveMode` enum.
    pub fn imode(&self) -> Result<InterleaveMode, CodecError> {
        let s = self.get_str_field("IMODE")?;
        let c = s
            .chars()
            .next()
            .ok_or_else(|| CodecError::Parse("IMODE field is empty".to_string()))?;
        InterleaveMode::from_char(c)
    }

    // ==================== Band Information Accessors ====================

    /// Get the number of bands in the image.
    ///
    /// This handles the NBANDS/XBANDS logic: if NBANDS is 0, the actual
    /// band count is in XBANDS.
    pub fn band_count(&self) -> Result<usize, CodecError> {
        let nbands = self.get_u8_field("NBANDS")? as usize;
        if nbands == 0 {
            // Use XBANDS for extended band count
            self.get_u32_field("XBANDS").map(|v| v as usize)
        } else {
            Ok(nbands)
        }
    }

    /// Get band information for a specific band.
    ///
    /// # Arguments
    /// * `index` - Zero-based band index
    ///
    /// # Returns
    /// A `BandInfoFacade` for accessing the band's metadata.
    pub fn band_info(&self, index: usize) -> Result<BandInfoFacade<'_>, CodecError> {
        let band_count = self.band_count()?;
        if index >= band_count {
            return Err(CodecError::Parse(format!(
                "Band index {} out of range (band count: {})",
                index, band_count
            )));
        }

        let nbands = self.get_u8_field("NBANDS")? as usize;
        let use_extended = nbands == 0;

        Ok(BandInfoFacade {
            accessor: &self.accessor,
            index,
            use_extended,
        })
    }

    // ==================== Compression Field Accessors ====================

    /// Get the image compression code (IC).
    ///
    /// Common values: NC (no compression), NM (no compression with mask),
    /// C8 (JPEG 2000), M8 (JPEG 2000 with mask).
    pub fn ic(&self) -> Result<String, CodecError> {
        self.get_str_field("IC")
    }

    /// Get the compression rate code (COMRAT).
    ///
    /// This field is only present when IC is not NC or NM.
    /// Returns `None` if the field is not present.
    pub fn comrat(&self) -> Result<Option<String>, CodecError> {
        if self.accessor.has("COMRAT") {
            Ok(Some(self.get_str_field("COMRAT")?))
        } else {
            Ok(None)
        }
    }

    /// Check if the image is uncompressed.
    ///
    /// Returns `true` if IC is "NC" or "NM".
    pub fn is_uncompressed(&self) -> Result<bool, CodecError> {
        let ic = self.ic()?;
        Ok(ic.trim() == "NC" || ic.trim() == "NM")
    }

    // ==================== Computed Helper Methods ====================

    /// Calculate the number of bytes per pixel.
    ///
    /// This is based on NBPP (number of bits per pixel).
    pub fn bytes_per_pixel(&self) -> Result<usize, CodecError> {
        let nbpp = self.nbpp()? as usize;
        // Round up to nearest byte
        Ok(nbpp.div_ceil(8))
    }

    /// Calculate the size of a single block in bytes.
    ///
    /// This accounts for block dimensions, band count, and bytes per pixel.
    /// Uses effective block dimensions (handles NPPBH=0/NPPBV=0 case).
    pub fn block_size_bytes(&self) -> Result<usize, CodecError> {
        let nppbh = self.effective_nppbh()? as usize;
        let nppbv = self.effective_nppbv()? as usize;
        let band_count = self.band_count()?;
        let bytes_per_pixel = self.bytes_per_pixel()?;

        Ok(nppbh * nppbv * band_count * bytes_per_pixel)
    }

    /// Calculate the total image data size in bytes.
    ///
    /// This is the size of all blocks combined.
    pub fn image_data_size(&self) -> Result<u64, CodecError> {
        let nbpr = self.nbpr()? as u64;
        let nbpc = self.nbpc()? as u64;
        let block_size = self.block_size_bytes()? as u64;

        Ok(nbpr * nbpc * block_size)
    }

    // ==================== Private Helper Methods ====================

    /// Get a string field from the accessor.
    fn get_str_field(&self, field: &str) -> Result<String, CodecError> {
        let value = self
            .accessor
            .get(field)
            .map_err(|e| CodecError::Parse(format!("Failed to read field '{}': {}", field, e)))?;
        let s = value.as_str().map_err(|e| {
            CodecError::Parse(format!(
                "Failed to parse field '{}' as string: {}",
                field, e
            ))
        })?;
        Ok(s.to_string())
    }

    /// Get a u8 field from the accessor (parsed from string).
    fn get_u8_field(&self, field: &str) -> Result<u8, CodecError> {
        let s = self.get_str_field(field)?;
        s.trim().parse::<u8>().map_err(|e| {
            CodecError::Parse(format!("Failed to parse field '{}' as u8: {}", field, e))
        })
    }

    /// Get a u32 field from the accessor (parsed from string).
    fn get_u32_field(&self, field: &str) -> Result<u32, CodecError> {
        let s = self.get_str_field(field)?;
        s.trim().parse::<u32>().map_err(|e| {
            CodecError::Parse(format!("Failed to parse field '{}' as u32: {}", field, e))
        })
    }
}

/// Facade for per-band metadata access.
///
/// This struct provides typed access to band information fields within
/// an image subheader. It handles the difference between regular band
/// info (NBANDS > 0) and extended band info (NBANDS == 0, using XBANDS).
pub struct BandInfoFacade<'a> {
    /// Reference to the parent accessor
    accessor: &'a StructureAccessor<'a>,
    /// Band index (zero-based)
    index: usize,
    /// Whether to use extended band info path
    use_extended: bool,
}

impl<'a> BandInfoFacade<'a> {
    /// Get the band representation (IREPBANDn).
    ///
    /// Common values: R, G, B, M (mono), LU (lookup), Y, Cb, Cr.
    pub fn irepband(&self) -> Result<String, CodecError> {
        self.get_band_str_field("IREPBAND")
    }

    /// Get the band subcategory (ISUBCATn).
    pub fn isubcat(&self) -> Result<String, CodecError> {
        self.get_band_str_field("ISUBCAT")
    }

    /// Get the band image filter condition (IFCn).
    ///
    /// Returns 'N' for no filter condition.
    pub fn ifc(&self) -> Result<char, CodecError> {
        let s = self.get_band_str_field("IFC")?;
        s.chars()
            .next()
            .ok_or_else(|| CodecError::Parse("IFC field is empty".to_string()))
    }

    /// Get the band standard image filter code (IMFLTn).
    pub fn imflt(&self) -> Result<String, CodecError> {
        self.get_band_str_field("IMFLT")
    }

    /// Get the number of LUTs for this band (NLUTSn).
    pub fn nluts(&self) -> Result<u8, CodecError> {
        self.get_band_u8_field("NLUTS")
    }

    /// Get the number of entries in each LUT (NELUTn).
    ///
    /// Returns `None` if NLUTSn is 0.
    pub fn nelut(&self) -> Result<Option<u32>, CodecError> {
        let nluts = self.nluts()?;
        if nluts == 0 {
            Ok(None)
        } else {
            let nested_accessor = self.get_band_accessor()?;
            if nested_accessor.has("NELUT") {
                let value = nested_accessor
                    .get("NELUT")
                    .map_err(|e| CodecError::Parse(format!("Failed to read NELUT: {}", e)))?;
                let s = value
                    .as_str()
                    .map_err(|e| CodecError::Parse(format!("Failed to parse NELUT: {}", e)))?;
                let num = s.trim().parse::<u32>().map_err(|e| {
                    CodecError::Parse(format!("Failed to parse NELUT as u32: {}", e))
                })?;
                Ok(Some(num))
            } else {
                Ok(None)
            }
        }
    }

    /// Get the LUT data for a specific LUT index.
    ///
    /// # Arguments
    /// * `lut_index` - Zero-based LUT index (0-3)
    ///
    /// # Returns
    /// The raw LUT data bytes, or `None` if the LUT doesn't exist.
    pub fn lut_data(&self, lut_index: usize) -> Result<Option<Vec<u8>>, CodecError> {
        let nluts = self.nluts()? as usize;
        if lut_index >= nluts {
            return Ok(None);
        }

        // Get NELUT to know the size of each LUT
        let nelut = match self.nelut()? {
            Some(n) => n as usize,
            None => return Ok(None),
        };

        // Get the byte offset for this specific band_info element using
        // calculate_field_offset with an explicit index. The offset cache
        // internally stores keys as "BAND_INFO_0", "BAND_INFO_1", etc.
        let field_name = self.band_info_field_name();
        let (band_info_offset, _band_info_size) = self
            .accessor
            .calculate_field_offset(field_name, Some(self.index))
            .map_err(|e| CodecError::Parse(format!("Failed to get band info byte range: {}", e)))?;

        // Calculate offset to LUT data within the band info
        // Band info structure:
        // - IREPBAND (2 bytes)
        // - ISUBCAT (6 bytes)
        // - IFC (1 byte)
        // - IMFLT (3 bytes)
        // - NLUTS (1 byte)
        // - NELUT (5 bytes) - only if NLUTS > 0
        // - LUT data (NELUT bytes per LUT, repeated NLUTS times)
        let lut_data_offset_in_band = 13 + 5; // 18 bytes of fixed fields before LUT data

        // Calculate absolute offset for this LUT
        let lut_start = band_info_offset + lut_data_offset_in_band + (lut_index * nelut);
        let lut_end = lut_start + nelut;

        // Get the raw data from the accessor's underlying buffer
        // We need to access the data directly since the accessor doesn't handle
        // expression-based sizes in nested structures properly
        let data = self.accessor.data();

        if lut_end <= data.len() {
            Ok(Some(data[lut_start..lut_end].to_vec()))
        } else {
            Err(CodecError::Parse(format!(
                "LUT data {} out of bounds: need bytes {}..{}, have {} bytes",
                lut_index,
                lut_start,
                lut_end,
                data.len()
            )))
        }
    }

    // ==================== Private Helper Methods ====================

    /// Get the base field name for band info (without index suffix).
    fn band_info_field_name(&self) -> &str {
        if self.use_extended {
            "BAND_INFO_EXTENDED"
        } else {
            "BAND_INFO"
        }
    }

    /// Get a StructureAccessor for this band's struct element.
    ///
    /// Retrieves the BAND_INFO array from the parent accessor, extracts the
    /// Value::Struct at self.index, and creates a nested StructureAccessor
    /// using the type definition from the parent's definition types.
    fn get_band_accessor(&self) -> Result<StructureAccessor<'a>, CodecError> {
        let field_name = self.band_info_field_name();

        // Get the array of band info structs
        let array_value = self
            .accessor
            .get(field_name)
            .map_err(|e| CodecError::Parse(format!("Failed to get {}: {}", field_name, e)))?;

        let elements = match &array_value {
            Value::Array(arr) => arr,
            _ => {
                return Err(CodecError::Parse(format!(
                    "Expected {} to be an array, got {:?}",
                    field_name, array_value
                )))
            }
        };

        if self.index >= elements.len() {
            return Err(CodecError::Parse(format!(
                "Band index {} out of range (array length: {})",
                self.index,
                elements.len()
            )));
        }

        let struct_val = match &elements[self.index] {
            Value::Struct(sv) => sv,
            _ => {
                return Err(CodecError::Parse(format!(
                    "Expected {}[{}] to be a struct",
                    field_name, self.index
                )))
            }
        };

        // Look up the type definition from the parent accessor's definition types
        let type_def = self
            .accessor
            .definition()
            .types
            .get(&struct_val.type_name)
            .ok_or_else(|| {
                CodecError::Parse(format!(
                    "Type definition '{}' not found in parent definition types",
                    struct_val.type_name
                ))
            })?;

        let nested_accessor = StructureAccessor::new(Arc::new(type_def.clone()), struct_val.data)
            .map_err(|e| {
            CodecError::Parse(format!(
                "Failed to create nested accessor for {}: {}",
                struct_val.type_name, e
            ))
        })?;

        Ok(nested_accessor)
    }

    /// Get a string field from the band info.
    fn get_band_str_field(&self, field: &str) -> Result<String, CodecError> {
        let nested_accessor = self.get_band_accessor()?;
        let value = nested_accessor.get(field).map_err(|e| {
            CodecError::Parse(format!("Failed to read band field '{}': {}", field, e))
        })?;
        let s = value.as_str().map_err(|e| {
            CodecError::Parse(format!(
                "Failed to parse band field '{}' as string: {}",
                field, e
            ))
        })?;
        Ok(s.to_string())
    }

    /// Get a u8 field from the band info.
    fn get_band_u8_field(&self, field: &str) -> Result<u8, CodecError> {
        let s = self.get_band_str_field(field)?;
        s.trim().parse::<u8>().map_err(|e| {
            CodecError::Parse(format!(
                "Failed to parse band field '{}' as u8: {}",
                field, e
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::StructureRegistry;

    /// Helper function to create synthetic NITF image subheader test data.
    /// This creates a minimal valid image subheader with configurable parameters.
    fn create_image_subheader_test_data(
        iid1: &str,
        nrows: u32,
        ncols: u32,
        pvtype: &str,
        irep: &str,
        abpp: u8,
        nbpp: u8,
        nbands: u8,
        nbpr: u32,
        nbpc: u32,
        nppbh: u32,
        nppbv: u32,
        imode: char,
        pjust: char,
        ic: &str,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        // IM (2) - Image segment marker
        data.extend_from_slice(b"IM");

        // IID1 (10) - Image identifier 1
        let iid1_padded = format!("{:<10}", iid1);
        data.extend_from_slice(&iid1_padded.as_bytes()[..10]);

        // IDATIM (14) - Image date and time
        data.extend_from_slice(b"20240101120000");

        // TGTID (17) - Target identifier
        data.extend_from_slice(b"                 ");

        // IID2 (80) - Image identifier 2
        data.extend_from_slice(&[b' '; 80]);

        // Security fields
        data.push(b'U'); // ISCLAS (1)
        data.extend_from_slice(b"  "); // ISCLSY (2)
        data.extend_from_slice(&[b' '; 11]); // ISCODE (11)
        data.extend_from_slice(b"  "); // ISCTLH (2)
        data.extend_from_slice(&[b' '; 20]); // ISREL (20)
        data.extend_from_slice(b"  "); // ISDCTP (2)
        data.extend_from_slice(&[b' '; 8]); // ISDCDT (8)
        data.extend_from_slice(&[b' '; 4]); // ISDCXM (4)
        data.push(b' '); // ISDG (1)
        data.extend_from_slice(&[b' '; 8]); // ISDGDT (8)
        data.extend_from_slice(&[b' '; 43]); // ISCLTX (43)
        data.push(b' '); // ISCATP (1)
        data.extend_from_slice(&[b' '; 40]); // ISCAUT (40)
        data.push(b' '); // ISCRSN (1)
        data.extend_from_slice(&[b' '; 8]); // ISSRDT (8)
        data.extend_from_slice(&[b' '; 15]); // ISCTLN (15)

        // ENCRYP (1)
        data.push(b'0');

        // ISORCE (42)
        data.extend_from_slice(&[b' '; 42]);

        // NROWS (8)
        data.extend_from_slice(format!("{:08}", nrows).as_bytes());

        // NCOLS (8)
        data.extend_from_slice(format!("{:08}", ncols).as_bytes());

        // PVTYPE (3)
        let pvtype_padded = format!("{:<3}", pvtype);
        data.extend_from_slice(&pvtype_padded.as_bytes()[..3]);

        // IREP (8)
        let irep_padded = format!("{:<8}", irep);
        data.extend_from_slice(&irep_padded.as_bytes()[..8]);

        // ICAT (8)
        data.extend_from_slice(b"VIS     ");

        // ABPP (2)
        data.extend_from_slice(format!("{:02}", abpp).as_bytes());

        // PJUST (1)
        data.push(pjust as u8);

        // ICORDS (1) - Using blank to skip IGEOLO
        data.push(b' ');

        // NICOM (1) - No comments
        data.push(b'0');

        // IC (2) - Compression
        let ic_padded = format!("{:<2}", ic);
        data.extend_from_slice(&ic_padded.as_bytes()[..2]);

        // NBANDS (1)
        data.push(b'0' + nbands);

        // Band info for each band (when NBANDS > 0)
        for _ in 0..nbands {
            data.extend_from_slice(b"M "); // IREPBAND (2)
            data.extend_from_slice(&[b' '; 6]); // ISUBCAT (6)
            data.push(b'N'); // IFC (1)
            data.extend_from_slice(&[b' '; 3]); // IMFLT (3)
            data.push(b'0'); // NLUTS (1) - No LUTs
        }

        // ISYNC (1)
        data.push(b'0');

        // IMODE (1)
        data.push(imode as u8);

        // NBPR (4)
        data.extend_from_slice(format!("{:04}", nbpr).as_bytes());

        // NBPC (4)
        data.extend_from_slice(format!("{:04}", nbpc).as_bytes());

        // NPPBH (4)
        data.extend_from_slice(format!("{:04}", nppbh).as_bytes());

        // NPPBV (4)
        data.extend_from_slice(format!("{:04}", nppbv).as_bytes());

        // NBPP (2)
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // IDLVL (3)
        data.extend_from_slice(b"001");

        // IALVL (3)
        data.extend_from_slice(b"000");

        // ILOC (10)
        data.extend_from_slice(b"0000000000");

        // IMAG (4)
        data.extend_from_slice(b"1.0 ");

        // UDIDL (5) - No user defined data
        data.extend_from_slice(b"00000");

        // IXSHDL (5) - No extended subheader data
        data.extend_from_slice(b"00000");

        data
    }

    #[test]
    fn test_facade_basic_fields() {
        let registry = StructureRegistry::new();
        let definition = match registry.get("nitf_02.10_image_subheader") {
            Some(def) => def,
            None => {
                eprintln!("Skipping test: nitf_02.10_image_subheader definition not found");
                return;
            }
        };

        let test_data = create_image_subheader_test_data(
            "TestImg01",
            512,
            512,
            "INT",
            "MONO",
            8,
            8,
            1,
            1,
            1,
            512,
            512,
            'B',
            'R',
            "NC",
        );

        let accessor = crate::parser::StructureAccessor::new(definition, &test_data).unwrap();
        let facade = ImageSubheaderFacade::new(accessor);

        assert_eq!(facade.iid1().unwrap().trim(), "TestImg01");
        assert_eq!(facade.nrows().unwrap(), 512);
        assert_eq!(facade.ncols().unwrap(), 512);
        assert_eq!(facade.pvtype().unwrap(), PixelValueType::UnsignedInt);
        assert_eq!(facade.irep().unwrap(), ImageRepresentation::Mono);
        assert_eq!(facade.abpp().unwrap(), 8);
        assert_eq!(facade.nbpp().unwrap(), 8);
        assert_eq!(facade.band_count().unwrap(), 1);
        assert_eq!(facade.nbpr().unwrap(), 1);
        assert_eq!(facade.nbpc().unwrap(), 1);
        assert_eq!(facade.nppbh().unwrap(), 512);
        assert_eq!(facade.nppbv().unwrap(), 512);
        assert_eq!(facade.imode().unwrap(), InterleaveMode::B);
        assert_eq!(facade.pjust().unwrap(), PixelJustification::Right);
        assert_eq!(facade.ic().unwrap().trim(), "NC");
        assert!(facade.is_uncompressed().unwrap());
    }

    #[test]
    fn test_facade_computed_helpers() {
        let registry = StructureRegistry::new();
        let definition = match registry.get("nitf_02.10_image_subheader") {
            Some(def) => def,
            None => {
                eprintln!("Skipping test: nitf_02.10_image_subheader definition not found");
                return;
            }
        };

        // 16-bit pixels, 2 bands
        let test_data = create_image_subheader_test_data(
            "TestImg02",
            256,
            256,
            "INT",
            "MULTI",
            16,
            16,
            2,
            1,
            1,
            256,
            256,
            'B',
            'R',
            "NC",
        );

        let accessor = crate::parser::StructureAccessor::new(definition, &test_data).unwrap();
        let facade = ImageSubheaderFacade::new(accessor);

        // bytes_per_pixel = ceil(16/8) = 2
        assert_eq!(facade.bytes_per_pixel().unwrap(), 2);

        // block_size_bytes = 256 * 256 * 2 bands * 2 bytes = 262144
        assert_eq!(facade.block_size_bytes().unwrap(), 256 * 256 * 2 * 2);

        // image_data_size = 1 * 1 * block_size = 262144
        assert_eq!(facade.image_data_size().unwrap(), 262144);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::parser::StructureRegistry;
    use proptest::prelude::*;

    /// Property 1: Image Subheader Round-Trip
    /// For any valid image subheader configuration, writing the subheader to bytes
    /// and then parsing it back SHALL produce an equivalent ImageSubheaderFacade
    /// with identical field values.
    /// **Validates: Requirements 1.1-1.10, 2.1-2.5, 7.1-7.8, 17.1**

    /// Generate a valid BCS-A string of specified length (printable ASCII 0x20-0x7E)
    /// Ensures at least one non-space character to avoid all-space strings
    fn valid_bcs_a_string(len: usize) -> impl Strategy<Value = String> {
        // Generate a string with at least one non-space character
        (
            0x21u8..=0x7Eu8,
            proptest::collection::vec(
                0x20u8..=0x7Eu8,
                len.saturating_sub(1)..=len.saturating_sub(1),
            ),
        )
            .prop_map(move |(first_char, mut rest)| {
                let mut result = vec![first_char];
                result.append(&mut rest);
                // Truncate to exact length
                result.truncate(len);
                // Pad with spaces if needed
                while result.len() < len {
                    result.push(b' ');
                }
                String::from_utf8(result).unwrap()
            })
    }

    /// Generate valid PVTYPE values
    fn valid_pvtype() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("INT"), Just("SI "), Just("R  "),]
    }

    /// Generate valid IREP values
    fn valid_irep() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("MONO    "), Just("MULTI   "),]
    }

    /// Generate valid IMODE values
    fn valid_imode() -> impl Strategy<Value = char> {
        prop_oneof![Just('B'), Just('P'), Just('R'), Just('S'),]
    }

    /// Generate valid PJUST values
    fn valid_pjust() -> impl Strategy<Value = char> {
        prop_oneof![Just('R'), Just('L'),]
    }

    /// Generate valid IC values (uncompressed only for this test)
    fn valid_ic() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("NC"), Just("NM"),]
    }

    /// Generate valid NBPP values based on PVTYPE
    fn valid_nbpp_for_pvtype(pvtype: &str) -> u8 {
        match pvtype.trim() {
            "INT" | "SI" => 8, // Use 8-bit for simplicity
            "R" => 32,
            _ => 8,
        }
    }

    /// Helper function to create synthetic NITF image subheader test data.
    fn create_test_subheader(
        iid1: &str,
        nrows: u32,
        ncols: u32,
        pvtype: &str,
        irep: &str,
        nbpp: u8,
        nbands: u8,
        nbpr: u32,
        nbpc: u32,
        nppbh: u32,
        nppbv: u32,
        imode: char,
        pjust: char,
        ic: &str,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        // IM (2)
        data.extend_from_slice(b"IM");

        // IID1 (10)
        let iid1_bytes = iid1.as_bytes();
        let mut iid1_field = [b' '; 10];
        let copy_len = iid1_bytes.len().min(10);
        iid1_field[..copy_len].copy_from_slice(&iid1_bytes[..copy_len]);
        data.extend_from_slice(&iid1_field);

        // IDATIM (14)
        data.extend_from_slice(b"20240101120000");

        // TGTID (17)
        data.extend_from_slice(&[b' '; 17]);

        // IID2 (80)
        data.extend_from_slice(&[b' '; 80]);

        // Security fields (total 167 bytes)
        data.push(b'U'); // ISCLAS (1)
        data.extend_from_slice(&[b' '; 2]); // ISCLSY (2)
        data.extend_from_slice(&[b' '; 11]); // ISCODE (11)
        data.extend_from_slice(&[b' '; 2]); // ISCTLH (2)
        data.extend_from_slice(&[b' '; 20]); // ISREL (20)
        data.extend_from_slice(&[b' '; 2]); // ISDCTP (2)
        data.extend_from_slice(&[b' '; 8]); // ISDCDT (8)
        data.extend_from_slice(&[b' '; 4]); // ISDCXM (4)
        data.push(b' '); // ISDG (1)
        data.extend_from_slice(&[b' '; 8]); // ISDGDT (8)
        data.extend_from_slice(&[b' '; 43]); // ISCLTX (43)
        data.push(b' '); // ISCATP (1)
        data.extend_from_slice(&[b' '; 40]); // ISCAUT (40)
        data.push(b' '); // ISCRSN (1)
        data.extend_from_slice(&[b' '; 8]); // ISSRDT (8)
        data.extend_from_slice(&[b' '; 15]); // ISCTLN (15)

        // ENCRYP (1)
        data.push(b'0');

        // ISORCE (42)
        data.extend_from_slice(&[b' '; 42]);

        // NROWS (8)
        data.extend_from_slice(format!("{:08}", nrows).as_bytes());

        // NCOLS (8)
        data.extend_from_slice(format!("{:08}", ncols).as_bytes());

        // PVTYPE (3)
        let pvtype_bytes = pvtype.as_bytes();
        let mut pvtype_field = [b' '; 3];
        let copy_len = pvtype_bytes.len().min(3);
        pvtype_field[..copy_len].copy_from_slice(&pvtype_bytes[..copy_len]);
        data.extend_from_slice(&pvtype_field);

        // IREP (8)
        let irep_bytes = irep.as_bytes();
        let mut irep_field = [b' '; 8];
        let copy_len = irep_bytes.len().min(8);
        irep_field[..copy_len].copy_from_slice(&irep_bytes[..copy_len]);
        data.extend_from_slice(&irep_field);

        // ICAT (8)
        data.extend_from_slice(b"VIS     ");

        // ABPP (2) - same as NBPP for simplicity
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // PJUST (1)
        data.push(pjust as u8);

        // ICORDS (1) - blank to skip IGEOLO
        data.push(b' ');

        // NICOM (1) - No comments
        data.push(b'0');

        // IC (2)
        let ic_bytes = ic.as_bytes();
        let mut ic_field = [b' '; 2];
        let copy_len = ic_bytes.len().min(2);
        ic_field[..copy_len].copy_from_slice(&ic_bytes[..copy_len]);
        data.extend_from_slice(&ic_field);

        // NBANDS (1)
        data.push(b'0' + nbands);

        // Band info for each band
        for _ in 0..nbands {
            data.extend_from_slice(b"M "); // IREPBAND (2)
            data.extend_from_slice(&[b' '; 6]); // ISUBCAT (6)
            data.push(b'N'); // IFC (1)
            data.extend_from_slice(&[b' '; 3]); // IMFLT (3)
            data.push(b'0'); // NLUTS (1)
        }

        // ISYNC (1)
        data.push(b'0');

        // IMODE (1)
        data.push(imode as u8);

        // NBPR (4)
        data.extend_from_slice(format!("{:04}", nbpr).as_bytes());

        // NBPC (4)
        data.extend_from_slice(format!("{:04}", nbpc).as_bytes());

        // NPPBH (4)
        data.extend_from_slice(format!("{:04}", nppbh).as_bytes());

        // NPPBV (4)
        data.extend_from_slice(format!("{:04}", nppbv).as_bytes());

        // NBPP (2)
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // IDLVL (3)
        data.extend_from_slice(b"001");

        // IALVL (3)
        data.extend_from_slice(b"000");

        // ILOC (10)
        data.extend_from_slice(b"0000000000");

        // IMAG (4)
        data.extend_from_slice(b"1.0 ");

        // UDIDL (5)
        data.extend_from_slice(b"00000");

        // IXSHDL (5)
        data.extend_from_slice(b"00000");

        data
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 1: Image Subheader Round-Trip
        /// Generate random valid subheader configurations, create test data,
        /// parse with facade, and verify field values match input.
        #[test]
        fn prop_1_image_subheader_round_trip(
            iid1 in valid_bcs_a_string(10),
            nrows in 1u32..8192,
            ncols in 1u32..8192,
            pvtype in valid_pvtype(),
            irep in valid_irep(),
            nbands in 1u8..9,
            imode in valid_imode(),
            pjust in valid_pjust(),
            ic in valid_ic(),
        ) {
            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => {
                    // Skip test if definition not available
                    return Ok(());
                }
            };

            let nbpp = valid_nbpp_for_pvtype(pvtype);

            // Calculate blocking parameters to cover image dimensions
            let nppbh = ncols.min(8192);
            let nppbv = nrows.min(8192);
            let nbpr = ncols.div_ceil(nppbh);
            let nbpc = nrows.div_ceil(nppbv);

            // Create test data
            let test_data = create_test_subheader(
                &iid1, nrows, ncols, pvtype, irep, nbpp, nbands,
                nbpr, nbpc, nppbh, nppbv, imode, pjust, ic,
            );

            // Parse with accessor and create facade
            let accessor = crate::parser::StructureAccessor::new(definition, &test_data)
                .map_err(|e| TestCaseError::fail(format!("Failed to create accessor: {}", e)))?;
            let facade = ImageSubheaderFacade::new(accessor);

            // Verify identification fields
            // Note: We compare trimmed values since trailing spaces are not semantically significant
            let facade_iid1 = facade.iid1().unwrap();
            prop_assert_eq!(facade_iid1.trim(), iid1.trim(),
                "IID1 should match input (after trimming)");

            // Verify dimension fields
            prop_assert_eq!(facade.nrows().unwrap(), nrows,
                "NROWS should match input");
            prop_assert_eq!(facade.ncols().unwrap(), ncols,
                "NCOLS should match input");

            // Verify pixel type fields
            let expected_pvtype = PixelValueType::from_str(pvtype).unwrap();
            prop_assert_eq!(facade.pvtype().unwrap(), expected_pvtype,
                "PVTYPE should match input");

            let expected_irep = ImageRepresentation::from_str(irep).unwrap();
            prop_assert_eq!(facade.irep().unwrap(), expected_irep,
                "IREP should match input");

            prop_assert_eq!(facade.nbpp().unwrap(), nbpp,
                "NBPP should match input");
            prop_assert_eq!(facade.abpp().unwrap(), nbpp,
                "ABPP should match NBPP");

            // Verify blocking parameters
            prop_assert_eq!(facade.nbpr().unwrap(), nbpr,
                "NBPR should match calculated value");
            prop_assert_eq!(facade.nbpc().unwrap(), nbpc,
                "NBPC should match calculated value");
            prop_assert_eq!(facade.nppbh().unwrap(), nppbh,
                "NPPBH should match input");
            prop_assert_eq!(facade.nppbv().unwrap(), nppbv,
                "NPPBV should match input");

            // Verify interleave mode
            let expected_imode = InterleaveMode::from_char(imode).unwrap();
            prop_assert_eq!(facade.imode().unwrap(), expected_imode,
                "IMODE should match input");

            // Verify pixel justification
            let expected_pjust = PixelJustification::from_char(pjust).unwrap();
            prop_assert_eq!(facade.pjust().unwrap(), expected_pjust,
                "PJUST should match input");

            // Verify compression
            let ic_value = facade.ic().unwrap();
            prop_assert_eq!(ic_value.trim(), ic.trim(),
                "IC should match input");
            prop_assert!(facade.is_uncompressed().unwrap(),
                "Image should be uncompressed for NC/NM");

            // Verify band count
            prop_assert_eq!(facade.band_count().unwrap(), nbands as usize,
                "Band count should match input");

            // Verify computed helpers are consistent
            let bytes_per_pixel = facade.bytes_per_pixel().unwrap();
            prop_assert!(bytes_per_pixel > 0,
                "Bytes per pixel should be positive");

            let block_size = facade.block_size_bytes().unwrap();
            let expected_block_size = (nppbh as usize) * (nppbv as usize) * (nbands as usize) * bytes_per_pixel;
            prop_assert_eq!(block_size, expected_block_size,
                "Block size should be nppbh * nppbv * nbands * bytes_per_pixel");

            let image_data_size = facade.image_data_size().unwrap();
            let expected_image_size = (nbpr as u64) * (nbpc as u64) * (block_size as u64);
            prop_assert_eq!(image_data_size, expected_image_size,
                "Image data size should be nbpr * nbpc * block_size");
        }
    }

    /// Band info configuration for testing
    #[derive(Debug, Clone)]
    struct BandInfoConfig {
        irepband: String,
        isubcat: String,
        ifc: char,
        imflt: String,
        nluts: u8,
    }

    /// Generate valid IREPBAND values (2 characters)
    fn valid_irepband() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("M ".to_string()), // Mono
            Just("R ".to_string()), // Red
            Just("G ".to_string()), // Green
            Just("B ".to_string()), // Blue
            Just("LU".to_string()), // Lookup
            Just("Y ".to_string()), // Y (luminance)
            Just("  ".to_string()), // Blank
        ]
    }

    /// Generate valid ISUBCAT values (6 characters, BCS-A)
    fn valid_isubcat() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("      ".to_string()),
            Just("VIS   ".to_string()),
            Just("NIR   ".to_string()),
            Just("SWIR  ".to_string()),
        ]
    }

    /// Generate valid IFC values (1 character)
    fn valid_ifc() -> impl Strategy<Value = char> {
        prop_oneof![
            Just('N'), // No filter
            Just(' '), // Blank
        ]
    }

    /// Generate valid IMFLT values (3 characters)
    fn valid_imflt() -> impl Strategy<Value = String> {
        prop_oneof![Just("   ".to_string()), Just("A  ".to_string()),]
    }

    /// Generate a valid band info configuration
    fn valid_band_info_config() -> impl Strategy<Value = BandInfoConfig> {
        (
            valid_irepband(),
            valid_isubcat(),
            valid_ifc(),
            valid_imflt(),
        )
            .prop_map(|(irepband, isubcat, ifc, imflt)| {
                BandInfoConfig {
                    irepband,
                    isubcat,
                    ifc,
                    imflt,
                    nluts: 0, // No LUTs for basic band info test
                }
            })
    }

    /// Helper function to create synthetic NITF image subheader with configurable band info.
    fn create_test_subheader_with_bands(
        nrows: u32,
        ncols: u32,
        pvtype: &str,
        irep: &str,
        nbpp: u8,
        bands: &[BandInfoConfig],
        imode: char,
        pjust: char,
        ic: &str,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        let nbands = bands.len() as u8;
        let use_xbands = nbands == 0 || bands.len() > 9;

        // IM (2)
        data.extend_from_slice(b"IM");

        // IID1 (10)
        data.extend_from_slice(b"TestImage ");

        // IDATIM (14)
        data.extend_from_slice(b"20240101120000");

        // TGTID (17)
        data.extend_from_slice(&[b' '; 17]);

        // IID2 (80)
        data.extend_from_slice(&[b' '; 80]);

        // Security fields
        data.push(b'U'); // ISCLAS (1)
        data.extend_from_slice(&[b' '; 2]); // ISCLSY (2)
        data.extend_from_slice(&[b' '; 11]); // ISCODE (11)
        data.extend_from_slice(&[b' '; 2]); // ISCTLH (2)
        data.extend_from_slice(&[b' '; 20]); // ISREL (20)
        data.extend_from_slice(&[b' '; 2]); // ISDCTP (2)
        data.extend_from_slice(&[b' '; 8]); // ISDCDT (8)
        data.extend_from_slice(&[b' '; 4]); // ISDCXM (4)
        data.push(b' '); // ISDG (1)
        data.extend_from_slice(&[b' '; 8]); // ISDGDT (8)
        data.extend_from_slice(&[b' '; 43]); // ISCLTX (43)
        data.push(b' '); // ISCATP (1)
        data.extend_from_slice(&[b' '; 40]); // ISCAUT (40)
        data.push(b' '); // ISCRSN (1)
        data.extend_from_slice(&[b' '; 8]); // ISSRDT (8)
        data.extend_from_slice(&[b' '; 15]); // ISCTLN (15)

        // ENCRYP (1)
        data.push(b'0');

        // ISORCE (42)
        data.extend_from_slice(&[b' '; 42]);

        // NROWS (8)
        data.extend_from_slice(format!("{:08}", nrows).as_bytes());

        // NCOLS (8)
        data.extend_from_slice(format!("{:08}", ncols).as_bytes());

        // PVTYPE (3)
        let pvtype_bytes = pvtype.as_bytes();
        let mut pvtype_field = [b' '; 3];
        let copy_len = pvtype_bytes.len().min(3);
        pvtype_field[..copy_len].copy_from_slice(&pvtype_bytes[..copy_len]);
        data.extend_from_slice(&pvtype_field);

        // IREP (8)
        let irep_bytes = irep.as_bytes();
        let mut irep_field = [b' '; 8];
        let copy_len = irep_bytes.len().min(8);
        irep_field[..copy_len].copy_from_slice(&irep_bytes[..copy_len]);
        data.extend_from_slice(&irep_field);

        // ICAT (8)
        data.extend_from_slice(b"VIS     ");

        // ABPP (2)
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // PJUST (1)
        data.push(pjust as u8);

        // ICORDS (1) - blank to skip IGEOLO
        data.push(b' ');

        // NICOM (1) - No comments
        data.push(b'0');

        // IC (2)
        let ic_bytes = ic.as_bytes();
        let mut ic_field = [b' '; 2];
        let copy_len = ic_bytes.len().min(2);
        ic_field[..copy_len].copy_from_slice(&ic_bytes[..copy_len]);
        data.extend_from_slice(&ic_field);

        // NBANDS (1) - 0 if using XBANDS
        if use_xbands {
            data.push(b'0');
            // XBANDS (5)
            data.extend_from_slice(format!("{:05}", bands.len()).as_bytes());
        } else {
            data.push(b'0' + nbands);
        }

        // Band info for each band
        for band in bands {
            // IREPBAND (2)
            let irepband_bytes = band.irepband.as_bytes();
            let mut irepband_field = [b' '; 2];
            let copy_len = irepband_bytes.len().min(2);
            irepband_field[..copy_len].copy_from_slice(&irepband_bytes[..copy_len]);
            data.extend_from_slice(&irepband_field);

            // ISUBCAT (6)
            let isubcat_bytes = band.isubcat.as_bytes();
            let mut isubcat_field = [b' '; 6];
            let copy_len = isubcat_bytes.len().min(6);
            isubcat_field[..copy_len].copy_from_slice(&isubcat_bytes[..copy_len]);
            data.extend_from_slice(&isubcat_field);

            // IFC (1)
            data.push(band.ifc as u8);

            // IMFLT (3)
            let imflt_bytes = band.imflt.as_bytes();
            let mut imflt_field = [b' '; 3];
            let copy_len = imflt_bytes.len().min(3);
            imflt_field[..copy_len].copy_from_slice(&imflt_bytes[..copy_len]);
            data.extend_from_slice(&imflt_field);

            // NLUTS (1)
            data.push(b'0' + band.nluts);

            // If NLUTS > 0, we'd need NELUT and LUT data, but for this test we use 0
        }

        // ISYNC (1)
        data.push(b'0');

        // IMODE (1)
        data.push(imode as u8);

        // Calculate blocking parameters
        let nppbh = ncols.min(8192);
        let nppbv = nrows.min(8192);
        let nbpr = ncols.div_ceil(nppbh);
        let nbpc = nrows.div_ceil(nppbv);

        // NBPR (4)
        data.extend_from_slice(format!("{:04}", nbpr).as_bytes());

        // NBPC (4)
        data.extend_from_slice(format!("{:04}", nbpc).as_bytes());

        // NPPBH (4)
        data.extend_from_slice(format!("{:04}", nppbh).as_bytes());

        // NPPBV (4)
        data.extend_from_slice(format!("{:04}", nppbv).as_bytes());

        // NBPP (2)
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // IDLVL (3)
        data.extend_from_slice(b"001");

        // IALVL (3)
        data.extend_from_slice(b"000");

        // ILOC (10)
        data.extend_from_slice(b"0000000000");

        // IMAG (4)
        data.extend_from_slice(b"1.0 ");

        // UDIDL (5)
        data.extend_from_slice(b"00000");

        // IXSHDL (5)
        data.extend_from_slice(b"00000");

        data
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 2: Band Information Round-Trip
        /// For any valid band configuration (1-9 bands or 10+ bands via XBANDS),
        /// writing band information and then parsing it back SHALL produce
        /// equivalent BandInfo structs for all bands.
        /// **Validates: Requirements 3.1-3.9, 9.1-9.9**
        #[test]
        fn prop_2_band_info_round_trip(
            nrows in 1u32..1024,
            ncols in 1u32..1024,
            // Test both regular bands (1-9) and extended bands (10+)
            band_count in prop_oneof![1usize..=9, 10usize..=15],
            imode in valid_imode(),
            pjust in valid_pjust(),
        ) {
            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => {
                    // Skip test if definition not available
                    return Ok(());
                }
            };

            // Generate band configurations
            // Note: The facade's as_str() trims trailing spaces, so we store trimmed values
            // for comparison, but write the full padded values to the test data
            let mut bands = Vec::new();
            for i in 0..band_count {
                // Cycle through different IREPBAND values (trimmed for comparison)
                let irepband = match i % 5 {
                    0 => "M".to_string(),
                    1 => "R".to_string(),
                    2 => "G".to_string(),
                    3 => "B".to_string(),
                    _ => "".to_string(),  // All spaces trims to empty
                };

                // Cycle through different ISUBCAT values (trimmed for comparison)
                let isubcat = match i % 3 {
                    0 => "".to_string(),      // All spaces trims to empty
                    1 => "VIS".to_string(),
                    _ => "NIR".to_string(),
                };

                bands.push(BandInfoConfig {
                    irepband,
                    isubcat,
                    ifc: 'N',
                    imflt: "".to_string(),  // All spaces trims to empty
                    nluts: 0,
                });
            }

            // Create test data with padded values
            let padded_bands: Vec<BandInfoConfig> = bands.iter().enumerate().map(|(i, _)| {
                // Cycle through different IREPBAND values (padded for writing)
                let irepband = match i % 5 {
                    0 => "M ".to_string(),
                    1 => "R ".to_string(),
                    2 => "G ".to_string(),
                    3 => "B ".to_string(),
                    _ => "  ".to_string(),
                };

                // Cycle through different ISUBCAT values (padded for writing)
                let isubcat = match i % 3 {
                    0 => "      ".to_string(),
                    1 => "VIS   ".to_string(),
                    _ => "NIR   ".to_string(),
                };

                BandInfoConfig {
                    irepband,
                    isubcat,
                    ifc: 'N',
                    imflt: "   ".to_string(),
                    nluts: 0,
                }
            }).collect();

            let test_data = create_test_subheader_with_bands(
                nrows, ncols, "INT", "MULTI   ", 8,
                &padded_bands, imode, pjust, "NC",
            );

            // Parse with accessor and create facade
            let accessor = crate::parser::StructureAccessor::new(definition, &test_data)
                .map_err(|e| TestCaseError::fail(format!("Failed to create accessor: {}", e)))?;
            let facade = ImageSubheaderFacade::new(accessor);

            // Verify band count
            let parsed_band_count = facade.band_count()
                .map_err(|e| TestCaseError::fail(format!("Failed to get band count: {}", e)))?;
            prop_assert_eq!(parsed_band_count, band_count,
                "Band count should match input");

            // Verify each band's fields
            for (i, expected_band) in bands.iter().enumerate() {
                let band_info = facade.band_info(i)
                    .map_err(|e| TestCaseError::fail(format!("Failed to get band info {}: {}", i, e)))?;

                // Verify IREPBAND (compare full string - facade returns untrimmed)
                let irepband = band_info.irepband()
                    .map_err(|e| TestCaseError::fail(format!("Failed to get irepband for band {}: {}", i, e)))?;
                prop_assert_eq!(&irepband, &expected_band.irepband,
                    "IREPBAND for band {} should match", i);

                // Verify ISUBCAT
                let isubcat = band_info.isubcat()
                    .map_err(|e| TestCaseError::fail(format!("Failed to get isubcat for band {}: {}", i, e)))?;
                prop_assert_eq!(&isubcat, &expected_band.isubcat,
                    "ISUBCAT for band {} should match", i);

                // Verify IFC
                let ifc = band_info.ifc()
                    .map_err(|e| TestCaseError::fail(format!("Failed to get ifc for band {}: {}", i, e)))?;
                prop_assert_eq!(ifc, expected_band.ifc,
                    "IFC for band {} should match", i);

                // Verify IMFLT
                let imflt = band_info.imflt()
                    .map_err(|e| TestCaseError::fail(format!("Failed to get imflt for band {}: {}", i, e)))?;
                prop_assert_eq!(&imflt, &expected_band.imflt,
                    "IMFLT for band {} should match", i);

                // Verify NLUTS
                let nluts = band_info.nluts()
                    .map_err(|e| TestCaseError::fail(format!("Failed to get nluts for band {}: {}", i, e)))?;
                prop_assert_eq!(nluts, expected_band.nluts,
                    "NLUTS for band {} should match", i);
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Property 3: LUT Data Round-Trip
        /// For any valid LUT configuration (1-4 LUTs per band with valid entry counts),
        /// writing LUT data and then parsing it back SHALL produce byte-identical LUT entries.
        /// **Validates: Requirements 4.1, 4.2, 4.5**
        #[test]
        fn prop_3_lut_data_round_trip(
            nluts in 1u8..=4,
            // Use specific values to test boundary cases without huge allocations
            nelut in prop_oneof![Just(16u32), Just(64u32), Just(256u32)],
            imode in valid_imode(),
            pjust in valid_pjust(),
        ) {
            // Fixed dimensions since they don't affect LUT round-trip
            let nrows = 64u32;
            let ncols = 64u32;
            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => {
                    // Skip test if definition not available
                    return Ok(());
                }
            };

            // Generate LUT data - use deterministic pattern based on LUT index
            let mut lut_data_vec: Vec<Vec<u8>> = Vec::new();
            for lut_idx in 0..nluts {
                let mut lut_bytes = Vec::with_capacity(nelut as usize);
                for entry_idx in 0..nelut {
                    // Create a deterministic pattern: (lut_idx * 64 + entry_idx) % 256
                    let value = ((lut_idx as u32 * 64 + entry_idx) % 256) as u8;
                    lut_bytes.push(value);
                }
                lut_data_vec.push(lut_bytes);
            }

            // Create test data with LUT
            let test_data = create_test_subheader_with_lut(
                nrows, ncols, "INT", "RGB/LUT ", 8,
                nluts, nelut, &lut_data_vec,
                imode, pjust, "NC",
            );

            // Parse with accessor and create facade
            let accessor = crate::parser::StructureAccessor::new(definition, &test_data)
                .map_err(|e| TestCaseError::fail(format!("Failed to create accessor: {}", e)))?;
            let facade = ImageSubheaderFacade::new(accessor);

            // Verify band count is 1 (RGB/LUT requires 1 band)
            let band_count = facade.band_count()
                .map_err(|e| TestCaseError::fail(format!("Failed to get band count: {}", e)))?;
            prop_assert_eq!(band_count, 1, "RGB/LUT should have 1 band");

            // Get band info
            let band_info = facade.band_info(0)
                .map_err(|e| TestCaseError::fail(format!("Failed to get band info: {}", e)))?;

            // Verify NLUTS
            let parsed_nluts = band_info.nluts()
                .map_err(|e| TestCaseError::fail(format!("Failed to get nluts: {}", e)))?;
            prop_assert_eq!(parsed_nluts, nluts, "NLUTS should match");

            // Verify NELUT
            let parsed_nelut = band_info.nelut()
                .map_err(|e| TestCaseError::fail(format!("Failed to get nelut: {}", e)))?;
            prop_assert!(parsed_nelut.is_some(), "NELUT should be present when NLUTS > 0");
            prop_assert_eq!(parsed_nelut.unwrap(), nelut, "NELUT should match");

            // Verify each LUT's data is byte-identical
            for lut_idx in 0..nluts as usize {
                let parsed_lut_data = band_info.lut_data(lut_idx)
                    .map_err(|e| TestCaseError::fail(format!("Failed to get LUT data {}: {}", lut_idx, e)))?;

                prop_assert!(parsed_lut_data.is_some(),
                    "LUT data {} should be present", lut_idx);

                let parsed_bytes = parsed_lut_data.unwrap();
                prop_assert_eq!(parsed_bytes.len(), nelut as usize,
                    "LUT {} should have {} entries", lut_idx, nelut);

                // Verify byte-identical
                prop_assert_eq!(&parsed_bytes, &lut_data_vec[lut_idx],
                    "LUT {} data should be byte-identical", lut_idx);
            }

            // Verify LUT data beyond nluts returns None
            let extra_lut = band_info.lut_data(nluts as usize)
                .map_err(|e| TestCaseError::fail(format!("Failed to check extra LUT: {}", e)))?;
            prop_assert!(extra_lut.is_none(),
                "LUT data beyond NLUTS should return None");
        }
    }

    /// Helper function to create synthetic NITF image subheader with LUT data.
    fn create_test_subheader_with_lut(
        nrows: u32,
        ncols: u32,
        pvtype: &str,
        irep: &str,
        nbpp: u8,
        nluts: u8,
        nelut: u32,
        lut_data: &[Vec<u8>],
        imode: char,
        pjust: char,
        ic: &str,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        // IM (2)
        data.extend_from_slice(b"IM");

        // IID1 (10)
        data.extend_from_slice(b"LUTTest   ");

        // IDATIM (14)
        data.extend_from_slice(b"20240101120000");

        // TGTID (17)
        data.extend_from_slice(&[b' '; 17]);

        // IID2 (80)
        data.extend_from_slice(&[b' '; 80]);

        // Security fields
        data.push(b'U'); // ISCLAS (1)
        data.extend_from_slice(&[b' '; 2]); // ISCLSY (2)
        data.extend_from_slice(&[b' '; 11]); // ISCODE (11)
        data.extend_from_slice(&[b' '; 2]); // ISCTLH (2)
        data.extend_from_slice(&[b' '; 20]); // ISREL (20)
        data.extend_from_slice(&[b' '; 2]); // ISDCTP (2)
        data.extend_from_slice(&[b' '; 8]); // ISDCDT (8)
        data.extend_from_slice(&[b' '; 4]); // ISDCXM (4)
        data.push(b' '); // ISDG (1)
        data.extend_from_slice(&[b' '; 8]); // ISDGDT (8)
        data.extend_from_slice(&[b' '; 43]); // ISCLTX (43)
        data.push(b' '); // ISCATP (1)
        data.extend_from_slice(&[b' '; 40]); // ISCAUT (40)
        data.push(b' '); // ISCRSN (1)
        data.extend_from_slice(&[b' '; 8]); // ISSRDT (8)
        data.extend_from_slice(&[b' '; 15]); // ISCTLN (15)

        // ENCRYP (1)
        data.push(b'0');

        // ISORCE (42)
        data.extend_from_slice(&[b' '; 42]);

        // NROWS (8)
        data.extend_from_slice(format!("{:08}", nrows).as_bytes());

        // NCOLS (8)
        data.extend_from_slice(format!("{:08}", ncols).as_bytes());

        // PVTYPE (3)
        let pvtype_bytes = pvtype.as_bytes();
        let mut pvtype_field = [b' '; 3];
        let copy_len = pvtype_bytes.len().min(3);
        pvtype_field[..copy_len].copy_from_slice(&pvtype_bytes[..copy_len]);
        data.extend_from_slice(&pvtype_field);

        // IREP (8)
        let irep_bytes = irep.as_bytes();
        let mut irep_field = [b' '; 8];
        let copy_len = irep_bytes.len().min(8);
        irep_field[..copy_len].copy_from_slice(&irep_bytes[..copy_len]);
        data.extend_from_slice(&irep_field);

        // ICAT (8)
        data.extend_from_slice(b"VIS     ");

        // ABPP (2)
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // PJUST (1)
        data.push(pjust as u8);

        // ICORDS (1) - blank to skip IGEOLO
        data.push(b' ');

        // NICOM (1) - No comments
        data.push(b'0');

        // IC (2)
        let ic_bytes = ic.as_bytes();
        let mut ic_field = [b' '; 2];
        let copy_len = ic_bytes.len().min(2);
        ic_field[..copy_len].copy_from_slice(&ic_bytes[..copy_len]);
        data.extend_from_slice(&ic_field);

        // NBANDS (1) - 1 band for RGB/LUT
        data.push(b'1');

        // Band info for the single band with LUT
        // IREPBAND (2) - LU for lookup
        data.extend_from_slice(b"LU");

        // ISUBCAT (6)
        data.extend_from_slice(&[b' '; 6]);

        // IFC (1)
        data.push(b'N');

        // IMFLT (3)
        data.extend_from_slice(&[b' '; 3]);

        // NLUTS (1)
        data.push(b'0' + nluts);

        // NELUT (5) - only present when NLUTS > 0
        data.extend_from_slice(format!("{:05}", nelut).as_bytes());

        // LUT data for each LUT
        for lut_bytes in lut_data.iter().take(nluts as usize) {
            data.extend_from_slice(lut_bytes);
        }

        // ISYNC (1)
        data.push(b'0');

        // IMODE (1)
        data.push(imode as u8);

        // Calculate blocking parameters
        let nppbh = ncols.min(8192);
        let nppbv = nrows.min(8192);
        let nbpr = ncols.div_ceil(nppbh);
        let nbpc = nrows.div_ceil(nppbv);

        // NBPR (4)
        data.extend_from_slice(format!("{:04}", nbpr).as_bytes());

        // NBPC (4)
        data.extend_from_slice(format!("{:04}", nbpc).as_bytes());

        // NPPBH (4)
        data.extend_from_slice(format!("{:04}", nppbh).as_bytes());

        // NPPBV (4)
        data.extend_from_slice(format!("{:04}", nppbv).as_bytes());

        // NBPP (2)
        data.extend_from_slice(format!("{:02}", nbpp).as_bytes());

        // IDLVL (3)
        data.extend_from_slice(b"001");

        // IALVL (3)
        data.extend_from_slice(b"000");

        // ILOC (10)
        data.extend_from_slice(b"0000000000");

        // IMAG (4)
        data.extend_from_slice(b"1.0 ");

        // UDIDL (5)
        data.extend_from_slice(b"00000");

        // IXSHDL (5)
        data.extend_from_slice(b"00000");

        data
    }
}
