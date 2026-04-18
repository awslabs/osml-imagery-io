//! Builder pattern for constructing image subheaders.
//!
//! The [`ImageSubheaderBuilder`] provides a fluent API for creating image
//! subheaders when writing NITF files. It handles field validation, blocking
//! parameter calculation, and band information management.
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::builder::{ImageSubheaderBuilder, BandInfoBuilder};
//! use osml_io::jbp::image::types::{PixelValueType, ImageRepresentation, InterleaveMode};
//!
//! let builder = ImageSubheaderBuilder::new()
//!     .iid1("TestImage")
//!     .nrows(512)
//!     .ncols(512)
//!     .pvtype(PixelValueType::UnsignedInt)
//!     .irep(ImageRepresentation::Mono)
//!     .nbpp(8)
//!     .abpp(8)
//!     .block_size(512, 512)
//!     .imode(InterleaveMode::B)
//!     .add_band(BandInfoBuilder::new().irepband("M"));
//!
//! let bytes = builder.build(&registry, NitfFormat::Nitf21)?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::CodecError;
use crate::jbp::image::types::{
    ImageRepresentation, InterleaveMode, LookUpTable, PixelJustification, PixelValueType,
};
use crate::jbp::types::NitfFormat;
use crate::parser::{StructureRegistry, StructureWriter};

/// Builder for constructing image subheaders.
///
/// This builder provides a fluent API for setting image subheader fields
/// and automatically calculates blocking parameters based on image dimensions
/// and block sizes.
#[derive(Debug, Clone)]
pub struct ImageSubheaderBuilder {
    /// Field values stored by field name
    fields: HashMap<String, FieldValue>,
    /// Band information builders
    bands: Vec<BandInfoBuilder>,
}

/// Internal representation of field values.
#[derive(Debug, Clone)]
enum FieldValue {
    String(String),
    U8(u8),
    U32(u32),
    Char(char),
}

impl Default for ImageSubheaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageSubheaderBuilder {
    /// Create a new image subheader builder with default values.
    pub fn new() -> Self {
        let mut fields = HashMap::new();

        // Set required defaults (uppercase to match .ksy field IDs)
        fields.insert("IM".to_string(), FieldValue::String("IM".to_string()));
        fields.insert("ENCRYP".to_string(), FieldValue::String("0".to_string()));
        fields.insert("ISCLAS".to_string(), FieldValue::String("U".to_string()));
        fields.insert("ISYNC".to_string(), FieldValue::String("0".to_string()));
        fields.insert("PJUST".to_string(), FieldValue::Char('R'));
        fields.insert("IMODE".to_string(), FieldValue::Char('B'));
        fields.insert("IC".to_string(), FieldValue::String("NC".to_string()));
        fields.insert("IDLVL".to_string(), FieldValue::U32(1));
        fields.insert("IALVL".to_string(), FieldValue::U32(0));
        fields.insert(
            "ILOC".to_string(),
            FieldValue::String("0000000000".to_string()),
        );
        fields.insert("IMAG".to_string(), FieldValue::String("1.0 ".to_string()));

        Self {
            fields,
            bands: Vec::new(),
        }
    }

    // ==================== Identification Field Setters ====================

    /// Set the image identifier 1 (IID1).
    ///
    /// This is a 10-character identifier for the image.
    pub fn iid1(mut self, value: &str) -> Self {
        self.fields
            .insert("IID1".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the image identifier 2 (IID2).
    ///
    /// This is an 80-character free-text identifier for the image.
    pub fn iid2(mut self, value: &str) -> Self {
        self.fields
            .insert("IID2".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the image date and time (IDATIM).
    ///
    /// This should be a 14-character date/time string in CCYYMMDDhhmmss format.
    pub fn idatim(mut self, value: &str) -> Self {
        self.fields
            .insert("IDATIM".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the target identifier (TGTID).
    ///
    /// This is a 17-character target identifier.
    pub fn tgtid(mut self, value: &str) -> Self {
        self.fields
            .insert("TGTID".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the image source (ISORCE).
    ///
    /// This is a 42-character description of the image source.
    pub fn isorce(mut self, value: &str) -> Self {
        self.fields
            .insert("ISORCE".to_string(), FieldValue::String(value.to_string()));
        self
    }

    // ==================== Dimension Field Setters ====================

    /// Set the number of significant rows in the image (NROWS).
    pub fn nrows(mut self, value: u32) -> Self {
        self.fields
            .insert("NROWS".to_string(), FieldValue::U32(value));
        self
    }

    /// Set the number of significant columns in the image (NCOLS).
    pub fn ncols(mut self, value: u32) -> Self {
        self.fields
            .insert("NCOLS".to_string(), FieldValue::U32(value));
        self
    }

    // ==================== Pixel Characteristic Setters ====================

    /// Set the pixel value type (PVTYPE).
    pub fn pvtype(mut self, value: PixelValueType) -> Self {
        self.fields.insert(
            "PVTYPE".to_string(),
            FieldValue::String(value.to_str().to_string()),
        );
        self
    }

    /// Set the image representation (IREP).
    pub fn irep(mut self, value: ImageRepresentation) -> Self {
        self.fields.insert(
            "IREP".to_string(),
            FieldValue::String(value.to_str().to_string()),
        );
        self
    }

    /// Set the image category (ICAT).
    ///
    /// This is an 8-character image category code.
    pub fn icat(mut self, value: &str) -> Self {
        self.fields
            .insert("ICAT".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the actual bits per pixel (ABPP).
    ///
    /// This is the number of significant bits in each pixel value.
    pub fn abpp(mut self, value: u8) -> Self {
        self.fields
            .insert("ABPP".to_string(), FieldValue::U8(value));
        self
    }

    /// Set the number of bits per pixel (NBPP).
    ///
    /// This is the storage size for each pixel value.
    pub fn nbpp(mut self, value: u8) -> Self {
        self.fields
            .insert("NBPP".to_string(), FieldValue::U8(value));
        self
    }

    /// Set the pixel justification (PJUST).
    pub fn pjust(mut self, value: PixelJustification) -> Self {
        self.fields
            .insert("PJUST".to_string(), FieldValue::Char(value.to_char()));
        self
    }

    // ==================== Blocking Parameter Setters ====================

    /// Set the block size (NPPBH and NPPBV).
    ///
    /// NBPR and NBPC will be calculated automatically based on image dimensions.
    pub fn block_size(mut self, width: u32, height: u32) -> Self {
        self.fields
            .insert("NPPBH".to_string(), FieldValue::U32(width));
        self.fields
            .insert("NPPBV".to_string(), FieldValue::U32(height));
        self
    }

    /// Set the interleave mode (IMODE).
    pub fn imode(mut self, value: InterleaveMode) -> Self {
        self.fields
            .insert("IMODE".to_string(), FieldValue::Char(value.to_char()));
        self
    }

    // ==================== Compression Setters ====================

    /// Set the image compression code (IC).
    ///
    /// Common values: "NC" (no compression), "NM" (no compression with mask),
    /// "C8" (JPEG 2000), "M8" (JPEG 2000 with mask).
    pub fn ic(mut self, value: &str) -> Self {
        self.fields
            .insert("IC".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the compression rate code (COMRAT).
    pub fn comrat(mut self, value: &str) -> Self {
        self.fields
            .insert("COMRAT".to_string(), FieldValue::String(value.to_string()));
        self
    }

    // ==================== Security Field Setters ====================

    /// Set the security classification (ISCLAS).
    pub fn isclas(mut self, value: &str) -> Self {
        self.fields
            .insert("ISCLAS".to_string(), FieldValue::String(value.to_string()));
        self
    }

    // ==================== Display Level Setters ====================

    /// Set the image display level (IDLVL).
    pub fn idlvl(mut self, value: u32) -> Self {
        self.fields
            .insert("IDLVL".to_string(), FieldValue::U32(value));
        self
    }

    /// Set the image attachment level (IALVL).
    pub fn ialvl(mut self, value: u32) -> Self {
        self.fields
            .insert("IALVL".to_string(), FieldValue::U32(value));
        self
    }

    /// Set the image location (ILOC).
    pub fn iloc(mut self, value: &str) -> Self {
        self.fields
            .insert("ILOC".to_string(), FieldValue::String(value.to_string()));
        self
    }

    /// Set the image magnification (IMAG).
    pub fn imag(mut self, value: &str) -> Self {
        self.fields
            .insert("IMAG".to_string(), FieldValue::String(value.to_string()));
        self
    }

    // ==================== Band Management ====================

    /// Add a band to the image.
    pub fn add_band(mut self, band: BandInfoBuilder) -> Self {
        self.bands.push(band);
        self
    }

    /// Get the current band count.
    pub fn band_count(&self) -> usize {
        self.bands.len()
    }

    // ==================== Build Method ====================

    /// Build the image subheader bytes.
    ///
    /// This method:
    /// 1. Validates required fields are set
    /// 2. Calculates blocking parameters (NBPR, NBPC)
    /// 3. Writes all fields to a StructureWriter
    /// 4. Returns the encoded bytes
    pub fn build(
        &self,
        registry: &StructureRegistry,
        format: NitfFormat,
    ) -> Result<Vec<u8>, CodecError> {
        // Get the structure definition
        let def_name = format.image_subheader_definition();
        let definition = registry.get(def_name).ok_or_else(|| {
            CodecError::InvalidFormat(format!("Structure definition not found: {}", def_name))
        })?;

        // Create a streaming writer (since we have variable-length band info)
        let mut writer = StructureWriter::new_streaming(Arc::clone(&definition));

        // Write fields in order
        self.write_fields(&mut writer)?;

        // Finish and return bytes
        writer
            .finish()
            .map_err(|e| CodecError::Encode(format!("Failed to write image subheader: {}", e)))
    }

    /// Calculate blocking parameters from image dimensions and block size.
    fn calculate_blocking(&self) -> Result<(u32, u32), CodecError> {
        let ncols = self
            .get_u32("NCOLS")
            .ok_or_else(|| CodecError::Encode("NCOLS is required".to_string()))?;
        let nrows = self
            .get_u32("NROWS")
            .ok_or_else(|| CodecError::Encode("NROWS is required".to_string()))?;
        let nppbh = self
            .get_u32("NPPBH")
            .ok_or_else(|| CodecError::Encode("NPPBH (block width) is required".to_string()))?;
        let nppbv = self
            .get_u32("NPPBV")
            .ok_or_else(|| CodecError::Encode("NPPBV (block height) is required".to_string()))?;

        // Calculate NBPR and NBPC to cover image dimensions
        // NBPR × NPPBH ≥ NCOLS
        // NBPC × NPPBV ≥ NROWS
        let nbpr = ncols.div_ceil(nppbh);
        let nbpc = nrows.div_ceil(nppbv);

        Ok((nbpr, nbpc))
    }

    /// Write all fields to the structure writer.
    fn write_fields(&self, writer: &mut StructureWriter) -> Result<(), CodecError> {
        // IM marker
        self.write_str_field(writer, "IM", "IM", 2)?;

        // Identification fields
        self.write_str_field(writer, "IID1", "", 10)?;
        self.write_str_field(writer, "IDATIM", "", 14)?;
        self.write_str_field(writer, "TGTID", "", 17)?;
        self.write_str_field(writer, "IID2", "", 80)?;

        // Security fields
        self.write_str_field(writer, "ISCLAS", "U", 1)?;
        self.write_str_field(writer, "ISCLSY", "", 2)?;
        self.write_str_field(writer, "ISCODE", "", 11)?;
        self.write_str_field(writer, "ISCTLH", "", 2)?;
        self.write_str_field(writer, "ISREL", "", 20)?;
        self.write_str_field(writer, "ISDCTP", "", 2)?;
        self.write_str_field(writer, "ISDCDT", "", 8)?;
        self.write_str_field(writer, "ISDCXM", "", 4)?;
        self.write_str_field(writer, "ISDG", "", 1)?;
        self.write_str_field(writer, "ISDGDT", "", 8)?;
        self.write_str_field(writer, "ISCLTX", "", 43)?;
        self.write_str_field(writer, "ISCATP", "", 1)?;
        self.write_str_field(writer, "ISCAUT", "", 40)?;
        self.write_str_field(writer, "ISCRSN", "", 1)?;
        self.write_str_field(writer, "ISSRDT", "", 8)?;
        self.write_str_field(writer, "ISCTLN", "", 15)?;

        // ENCRYP
        self.write_str_field(writer, "ENCRYP", "0", 1)?;

        // ISORCE
        self.write_str_field(writer, "ISORCE", "", 42)?;

        // Dimension fields
        self.write_numeric_field(writer, "NROWS", 8)?;
        self.write_numeric_field(writer, "NCOLS", 8)?;

        // Pixel characteristics
        self.write_str_field(writer, "PVTYPE", "INT", 3)?;
        self.write_str_field(writer, "IREP", "MONO    ", 8)?;
        self.write_str_field(writer, "ICAT", "VIS     ", 8)?;
        self.write_numeric_field(writer, "ABPP", 2)?;
        self.write_char_field(writer, "PJUST", 'R')?;

        // ICORDS - blank to skip IGEOLO
        self.write_str_field(writer, "ICORDS", "", 1)?;

        // NICOM - no comments
        writer
            .set("NICOM", "0")
            .map_err(|e| CodecError::Encode(format!("Failed to write NICOM: {}", e)))?;

        // IC
        self.write_str_field(writer, "IC", "NC", 2)?;

        // NBANDS / XBANDS
        let band_count = self.bands.len();
        if band_count == 0 {
            return Err(CodecError::Encode(
                "At least one band is required".to_string(),
            ));
        }

        if band_count <= 9 {
            // Use NBANDS
            writer
                .set("NBANDS", format!("{}", band_count))
                .map_err(|e| CodecError::Encode(format!("Failed to write NBANDS: {}", e)))?;
        } else {
            // Use XBANDS
            writer
                .set("NBANDS", "0")
                .map_err(|e| CodecError::Encode(format!("Failed to write NBANDS: {}", e)))?;
            writer
                .set("XBANDS", format!("{:05}", band_count))
                .map_err(|e| CodecError::Encode(format!("Failed to write XBANDS: {}", e)))?;
        }

        // Write band info for each band
        for (i, band) in self.bands.iter().enumerate() {
            band.write_to(writer, i, band_count > 9)?;
        }

        // ISYNC
        self.write_str_field(writer, "ISYNC", "0", 1)?;

        // IMODE
        self.write_char_field(writer, "IMODE", 'B')?;

        // Calculate and write blocking parameters
        let (nbpr, nbpc) = self.calculate_blocking()?;
        writer
            .set("NBPR", format!("{:04}", nbpr))
            .map_err(|e| CodecError::Encode(format!("Failed to write NBPR: {}", e)))?;
        writer
            .set("NBPC", format!("{:04}", nbpc))
            .map_err(|e| CodecError::Encode(format!("Failed to write NBPC: {}", e)))?;

        // NPPBH and NPPBV
        self.write_numeric_field(writer, "NPPBH", 4)?;
        self.write_numeric_field(writer, "NPPBV", 4)?;

        // NBPP
        self.write_numeric_field(writer, "NBPP", 2)?;

        // Display levels
        self.write_numeric_field(writer, "IDLVL", 3)?;
        self.write_numeric_field(writer, "IALVL", 3)?;
        self.write_str_field(writer, "ILOC", "0000000000", 10)?;
        self.write_str_field(writer, "IMAG", "1.0 ", 4)?;

        // UDIDL - no user defined data
        writer
            .set("UDIDL", "00000")
            .map_err(|e| CodecError::Encode(format!("Failed to write UDIDL: {}", e)))?;

        // IXSHDL - no extended subheader data
        writer
            .set("IXSHDL", "00000")
            .map_err(|e| CodecError::Encode(format!("Failed to write IXSHDL: {}", e)))?;

        Ok(())
    }

    /// Write a string field with default value.
    fn write_str_field(
        &self,
        writer: &mut StructureWriter,
        field: &str,
        default: &str,
        _size: usize,
    ) -> Result<(), CodecError> {
        let value = self
            .get_string(field)
            .unwrap_or_else(|| default.to_string());
        writer
            .set(field, value)
            .map_err(|e| CodecError::Encode(format!("Failed to write {}: {}", field, e)))
    }

    /// Write a numeric field.
    fn write_numeric_field(
        &self,
        writer: &mut StructureWriter,
        field: &str,
        width: usize,
    ) -> Result<(), CodecError> {
        let value = match self.fields.get(field) {
            Some(FieldValue::U32(n)) => format!("{:0width$}", n, width = width),
            Some(FieldValue::U8(n)) => format!("{:0width$}", n, width = width),
            _ => format!("{:0width$}", 0, width = width),
        };
        writer
            .set(field, value)
            .map_err(|e| CodecError::Encode(format!("Failed to write {}: {}", field, e)))
    }

    /// Write a character field.
    fn write_char_field(
        &self,
        writer: &mut StructureWriter,
        field: &str,
        default: char,
    ) -> Result<(), CodecError> {
        let value = self.get_char(field).unwrap_or(default);
        writer
            .set(field, value.to_string())
            .map_err(|e| CodecError::Encode(format!("Failed to write {}: {}", field, e)))
    }

    /// Get a string field value.
    fn get_string(&self, field: &str) -> Option<String> {
        match self.fields.get(field) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Get a u32 field value.
    fn get_u32(&self, field: &str) -> Option<u32> {
        match self.fields.get(field) {
            Some(FieldValue::U32(n)) => Some(*n),
            _ => None,
        }
    }

    /// Get a char field value.
    fn get_char(&self, field: &str) -> Option<char> {
        match self.fields.get(field) {
            Some(FieldValue::Char(c)) => Some(*c),
            _ => None,
        }
    }
}

/// Builder for band information.
///
/// This builder provides a fluent API for setting per-band metadata fields.
#[derive(Debug, Clone, Default)]
pub struct BandInfoBuilder {
    /// Band representation (IREPBAND) - 2 characters
    irepband: Option<String>,
    /// Band subcategory (ISUBCAT) - 6 characters
    isubcat: Option<String>,
    /// Image filter condition (IFC) - 1 character
    ifc: Option<char>,
    /// Standard image filter code (IMFLT) - 3 characters
    imflt: Option<String>,
    /// Look-up tables for this band
    luts: Vec<LookUpTable>,
}

impl BandInfoBuilder {
    /// Create a new band info builder with default values.
    pub fn new() -> Self {
        Self {
            irepband: None,
            isubcat: None,
            ifc: Some('N'),
            imflt: None,
            luts: Vec::new(),
        }
    }

    /// Set the band representation (IREPBAND).
    ///
    /// Common values: "R", "G", "B", "M" (mono), "LU" (lookup), "Y", "Cb", "Cr".
    pub fn irepband(mut self, value: &str) -> Self {
        self.irepband = Some(value.to_string());
        self
    }

    /// Set the band subcategory (ISUBCAT).
    pub fn isubcat(mut self, value: &str) -> Self {
        self.isubcat = Some(value.to_string());
        self
    }

    /// Set the image filter condition (IFC).
    ///
    /// Default is 'N' for no filter condition.
    pub fn ifc(mut self, value: char) -> Self {
        self.ifc = Some(value);
        self
    }

    /// Set the standard image filter code (IMFLT).
    pub fn imflt(mut self, value: &str) -> Self {
        self.imflt = Some(value.to_string());
        self
    }

    /// Add a look-up table to this band.
    ///
    /// Up to 4 LUTs can be added per band.
    pub fn add_lut(mut self, lut: LookUpTable) -> Self {
        if self.luts.len() < 4 {
            self.luts.push(lut);
        }
        self
    }

    /// Get the number of LUTs for this band.
    pub fn lut_count(&self) -> usize {
        self.luts.len()
    }

    /// Write band info to the structure writer.
    ///
    /// # Arguments
    /// * `writer` - The structure writer to write to
    /// * `index` - The band index (zero-based)
    /// * `use_extended` - Whether to use extended band info path (XBANDS)
    pub(crate) fn write_to(
        &self,
        writer: &mut StructureWriter,
        index: usize,
        use_extended: bool,
    ) -> Result<(), CodecError> {
        let prefix = if use_extended {
            format!("BAND_INFO_EXTENDED_{}", index)
        } else {
            format!("BAND_INFO_{}", index)
        };

        // IREPBAND (2 characters)
        let irepband = self.irepband.as_deref().unwrap_or("  ");
        let irepband_padded = format!("{:<2}", irepband);
        writer
            .set(&format!("{}.IREPBAND", prefix), &irepband_padded[..2])
            .map_err(|e| CodecError::Encode(format!("Failed to write IREPBAND: {}", e)))?;

        // ISUBCAT (6 characters)
        let isubcat = self.isubcat.as_deref().unwrap_or("");
        let isubcat_padded = format!("{:<6}", isubcat);
        writer
            .set(&format!("{}.ISUBCAT", prefix), &isubcat_padded[..6])
            .map_err(|e| CodecError::Encode(format!("Failed to write ISUBCAT: {}", e)))?;

        // IFC (1 character)
        let ifc = self.ifc.unwrap_or('N');
        writer
            .set(&format!("{}.IFC", prefix), ifc.to_string())
            .map_err(|e| CodecError::Encode(format!("Failed to write IFC: {}", e)))?;

        // IMFLT (3 characters)
        let imflt = self.imflt.as_deref().unwrap_or("");
        let imflt_padded = format!("{:<3}", imflt);
        writer
            .set(&format!("{}.IMFLT", prefix), &imflt_padded[..3])
            .map_err(|e| CodecError::Encode(format!("Failed to write IMFLT: {}", e)))?;

        // NLUTS (1 character)
        let nluts = self.luts.len();
        writer
            .set(&format!("{}.NLUTS", prefix), format!("{}", nluts))
            .map_err(|e| CodecError::Encode(format!("Failed to write NLUTS: {}", e)))?;

        // If we have LUTs, write NELUT and LUT data
        if nluts > 0 {
            // All LUTs must have the same size
            let nelut = self.luts[0].len();
            writer
                .set(&format!("{}.NELUT", prefix), format!("{:05}", nelut))
                .map_err(|e| CodecError::Encode(format!("Failed to write NELUT: {}", e)))?;

            // Write LUT data for each LUT
            for (lut_idx, lut) in self.luts.iter().enumerate() {
                writer
                    .set(&format!("{}.LUT_DATA_{}", prefix, lut_idx), lut.as_bytes())
                    .map_err(|e| {
                        CodecError::Encode(format!("Failed to write LUT_DATA_{}: {}", lut_idx, e))
                    })?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_new_has_defaults() {
        let builder = ImageSubheaderBuilder::new();

        // Check defaults are set (uppercase keys to match .ksy field IDs)
        assert_eq!(builder.get_string("IM"), Some("IM".to_string()));
        assert_eq!(builder.get_string("ENCRYP"), Some("0".to_string()));
        assert_eq!(builder.get_string("ISCLAS"), Some("U".to_string()));
        assert_eq!(builder.get_char("PJUST"), Some('R'));
        assert_eq!(builder.get_char("IMODE"), Some('B'));
        assert_eq!(builder.get_string("IC"), Some("NC".to_string()));
    }

    #[test]
    fn test_builder_fluent_setters() {
        let builder = ImageSubheaderBuilder::new()
            .iid1("TestImage")
            .nrows(512)
            .ncols(1024)
            .pvtype(PixelValueType::UnsignedInt)
            .irep(ImageRepresentation::Mono)
            .nbpp(8)
            .abpp(8)
            .block_size(256, 256)
            .imode(InterleaveMode::P);

        assert_eq!(builder.get_string("IID1"), Some("TestImage".to_string()));
        assert_eq!(builder.get_u32("NROWS"), Some(512));
        assert_eq!(builder.get_u32("NCOLS"), Some(1024));
        assert_eq!(builder.get_string("PVTYPE"), Some("INT".to_string()));
        assert_eq!(builder.get_string("IREP"), Some("MONO    ".to_string()));
        assert_eq!(builder.get_u32("NPPBH"), Some(256));
        assert_eq!(builder.get_u32("NPPBV"), Some(256));
        assert_eq!(builder.get_char("IMODE"), Some('P'));
    }

    #[test]
    fn test_builder_add_band() {
        let builder = ImageSubheaderBuilder::new()
            .add_band(BandInfoBuilder::new().irepband("R"))
            .add_band(BandInfoBuilder::new().irepband("G"))
            .add_band(BandInfoBuilder::new().irepband("B"));

        assert_eq!(builder.band_count(), 3);
    }

    #[test]
    fn test_calculate_blocking() {
        let builder = ImageSubheaderBuilder::new()
            .nrows(1000)
            .ncols(1500)
            .block_size(512, 512);

        let (nbpr, nbpc) = builder.calculate_blocking().unwrap();

        // NBPR × NPPBH ≥ NCOLS: 3 × 512 = 1536 ≥ 1500
        assert_eq!(nbpr, 3);
        // NBPC × NPPBV ≥ NROWS: 2 × 512 = 1024 ≥ 1000
        assert_eq!(nbpc, 2);
    }

    #[test]
    fn test_calculate_blocking_exact_fit() {
        let builder = ImageSubheaderBuilder::new()
            .nrows(512)
            .ncols(512)
            .block_size(512, 512);

        let (nbpr, nbpc) = builder.calculate_blocking().unwrap();

        assert_eq!(nbpr, 1);
        assert_eq!(nbpc, 1);
    }

    #[test]
    fn test_calculate_blocking_small_blocks() {
        let builder = ImageSubheaderBuilder::new()
            .nrows(100)
            .ncols(100)
            .block_size(32, 32);

        let (nbpr, nbpc) = builder.calculate_blocking().unwrap();

        // NBPR × NPPBH ≥ NCOLS: 4 × 32 = 128 ≥ 100
        assert_eq!(nbpr, 4);
        // NBPC × NPPBV ≥ NROWS: 4 × 32 = 128 ≥ 100
        assert_eq!(nbpc, 4);
    }

    #[test]
    fn test_band_info_builder_defaults() {
        let band = BandInfoBuilder::new();

        assert_eq!(band.ifc, Some('N'));
        assert_eq!(band.lut_count(), 0);
    }

    #[test]
    fn test_band_info_builder_fluent_setters() {
        let band = BandInfoBuilder::new()
            .irepband("R")
            .isubcat("VIS")
            .ifc('Y')
            .imflt("ABC");

        assert_eq!(band.irepband, Some("R".to_string()));
        assert_eq!(band.isubcat, Some("VIS".to_string()));
        assert_eq!(band.ifc, Some('Y'));
        assert_eq!(band.imflt, Some("ABC".to_string()));
    }

    #[test]
    fn test_band_info_builder_add_lut() {
        let lut1 = LookUpTable::from_bytes(&[0, 1, 2, 3]);
        let lut2 = LookUpTable::from_bytes(&[4, 5, 6, 7]);

        let band = BandInfoBuilder::new().add_lut(lut1).add_lut(lut2);

        assert_eq!(band.lut_count(), 2);
    }

    #[test]
    fn test_band_info_builder_max_luts() {
        let band = BandInfoBuilder::new()
            .add_lut(LookUpTable::from_bytes(&[0]))
            .add_lut(LookUpTable::from_bytes(&[1]))
            .add_lut(LookUpTable::from_bytes(&[2]))
            .add_lut(LookUpTable::from_bytes(&[3]))
            .add_lut(LookUpTable::from_bytes(&[4])); // Should be ignored

        // Max 4 LUTs
        assert_eq!(band.lut_count(), 4);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 8: Blocking Parameters Cover Image Dimensions
        /// For any image dimensions (NROWS, NCOLS) and block sizes (NPPBH, NPPBV),
        /// the calculated blocking parameters SHALL satisfy:
        /// NBPR × NPPBH ≥ NCOLS and NBPC × NPPBV ≥ NROWS.
        /// **Validates: Requirements 8.1-8.8, 13.5, 13.6**
        #[test]
        fn prop_8_blocking_parameters_cover_image_dimensions(
            nrows in 1u32..100000,
            ncols in 1u32..100000,
            // Block sizes from 1 to 8192 (NITF max block size)
            nppbh in 1u32..8193,
            nppbv in 1u32..8193,
        ) {
            let builder = ImageSubheaderBuilder::new()
                .nrows(nrows)
                .ncols(ncols)
                .block_size(nppbh, nppbv);

            let (nbpr, nbpc) = builder.calculate_blocking().unwrap();

            // Property: NBPR × NPPBH ≥ NCOLS
            let coverage_h = nbpr * nppbh;
            prop_assert!(
                coverage_h >= ncols,
                "NBPR × NPPBH ({} × {} = {}) must be >= NCOLS ({})",
                nbpr, nppbh, coverage_h, ncols
            );

            // Property: NBPC × NPPBV ≥ NROWS
            let coverage_v = nbpc * nppbv;
            prop_assert!(
                coverage_v >= nrows,
                "NBPC × NPPBV ({} × {} = {}) must be >= NROWS ({})",
                nbpc, nppbv, coverage_v, nrows
            );

            // Additional property: blocking should be minimal (no extra blocks)
            // NBPR should be the ceiling of NCOLS / NPPBH
            let expected_nbpr = ncols.div_ceil(nppbh);
            prop_assert_eq!(
                nbpr, expected_nbpr,
                "NBPR should be ceiling(NCOLS / NPPBH)"
            );

            // NBPC should be the ceiling of NROWS / NPPBV
            let expected_nbpc = nrows.div_ceil(nppbv);
            prop_assert_eq!(
                nbpc, expected_nbpc,
                "NBPC should be ceiling(NROWS / NPPBV)"
            );
        }

        /// Property: Blocking calculation handles edge cases correctly
        /// When image dimensions equal block size, we should get exactly 1 block.
        #[test]
        fn prop_blocking_exact_fit(
            size in 1u32..8193,
        ) {
            let builder = ImageSubheaderBuilder::new()
                .nrows(size)
                .ncols(size)
                .block_size(size, size);

            let (nbpr, nbpc) = builder.calculate_blocking().unwrap();

            prop_assert_eq!(nbpr, 1, "Exact fit should produce 1 block per row");
            prop_assert_eq!(nbpc, 1, "Exact fit should produce 1 block per column");
        }

        /// Property: Blocking calculation handles single-pixel overflow correctly
        /// When image is 1 pixel larger than block size, we need 2 blocks.
        #[test]
        fn prop_blocking_single_pixel_overflow(
            block_size in 1u32..4096,
        ) {
            let image_size = block_size + 1;

            let builder = ImageSubheaderBuilder::new()
                .nrows(image_size)
                .ncols(image_size)
                .block_size(block_size, block_size);

            let (nbpr, nbpc) = builder.calculate_blocking().unwrap();

            prop_assert_eq!(nbpr, 2, "Single pixel overflow should produce 2 blocks per row");
            prop_assert_eq!(nbpc, 2, "Single pixel overflow should produce 2 blocks per column");
        }
    }
}
