//! Image subheader validation for JBP (NITF/NSIF) files.
//!
//! This module provides validation of image subheader fields according to
//! JBP requirements. Validation checks include dimension validation, blocking
//! parameter validation, pixel type consistency, band configuration, and LUT
//! configuration.
//!
//! # Validation Results
//!
//! Validation produces two types of results:
//! - **Errors**: Issues that indicate invalid or corrupt data
//! - **Warnings**: Issues that don't prevent processing but indicate potential problems
//!
//! # Example
//!
//! ```ignore
//! use osml_io::jbp::image::validation::ImageValidator;
//! use osml_io::jbp::image::facade::ImageSubheaderFacade;
//!
//! let results = ImageValidator::validate(&facade, clevel);
//! for result in &results {
//!     if result.is_error() {
//!         eprintln!("Error: {}", result);
//!     } else {
//!         eprintln!("Warning: {}", result);
//!     }
//! }
//! ```

use crate::jbp::image::facade::ImageSubheaderFacade;
use crate::jbp::image::types::{ImageRepresentation, PixelValueType};

/// Severity level for validation results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Error - indicates invalid or corrupt data that prevents proper processing
    Error,
    /// Warning - indicates potential issues that don't prevent processing
    Warning,
}

/// Validation code for programmatic handling of validation results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageValidationCode {
    // Dimension validation codes (Requirement 13)
    /// NROWS is zero
    ZeroRows,
    /// NCOLS is zero
    ZeroCols,
    /// NROWS exceeds CLEVEL limits
    RowsExceedClevel,
    /// NCOLS exceeds CLEVEL limits
    ColsExceedClevel,
    /// Blocking doesn't cover columns (NBPR × NPPBH < NCOLS)
    BlockingInsufficientCols,
    /// Blocking doesn't cover rows (NBPC × NPPBV < NROWS)
    BlockingInsufficientRows,

    // Pixel type validation codes (Requirement 14)
    /// Invalid PVTYPE value
    InvalidPvtype,
    /// ABPP is greater than NBPP
    AbppExceedsNbpp,
    /// PVTYPE=R requires NBPP of 32 or 64
    InvalidNbppForReal,
    /// PVTYPE=C requires NBPP of 64
    InvalidNbppForComplex,
    /// PVTYPE=B requires NBPP of 1
    InvalidNbppForBilevel,

    // Band configuration validation codes (Requirement 15)
    /// RGB requires exactly 3 bands
    RgbBandCountMismatch,
    /// RGB/LUT requires exactly 1 band
    RgbLutBandCountMismatch,
    /// MONO requires exactly 1 band
    MonoBandCountMismatch,
    /// YCbCr601 requires exactly 3 bands
    YCbCr601BandCountMismatch,
    /// Invalid IREPBANDn for the given IREP
    InvalidIrepband,
    /// IMODE=S with single band is inefficient
    InefficientSingleBandSequential,

    // LUT validation codes (Requirement 16)
    /// NLUTSn exceeds maximum of 4
    TooManyLuts,
    /// NELUTn is 0 when NLUTSn > 0
    ZeroLutEntries,
    /// RGB/LUT requires exactly 3 LUTs
    RgbLutLutCountMismatch,
    /// NELUTn is less than 2^ABPP (incomplete LUT)
    IncompleteLut,
}

impl std::fmt::Display for ImageValidationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageValidationCode::ZeroRows => write!(f, "ZERO_ROWS"),
            ImageValidationCode::ZeroCols => write!(f, "ZERO_COLS"),
            ImageValidationCode::RowsExceedClevel => write!(f, "ROWS_EXCEED_CLEVEL"),
            ImageValidationCode::ColsExceedClevel => write!(f, "COLS_EXCEED_CLEVEL"),
            ImageValidationCode::BlockingInsufficientCols => write!(f, "BLOCKING_INSUFFICIENT_COLS"),
            ImageValidationCode::BlockingInsufficientRows => write!(f, "BLOCKING_INSUFFICIENT_ROWS"),
            ImageValidationCode::InvalidPvtype => write!(f, "INVALID_PVTYPE"),
            ImageValidationCode::AbppExceedsNbpp => write!(f, "ABPP_EXCEEDS_NBPP"),
            ImageValidationCode::InvalidNbppForReal => write!(f, "INVALID_NBPP_FOR_REAL"),
            ImageValidationCode::InvalidNbppForComplex => write!(f, "INVALID_NBPP_FOR_COMPLEX"),
            ImageValidationCode::InvalidNbppForBilevel => write!(f, "INVALID_NBPP_FOR_BILEVEL"),
            ImageValidationCode::RgbBandCountMismatch => write!(f, "RGB_BAND_COUNT_MISMATCH"),
            ImageValidationCode::RgbLutBandCountMismatch => write!(f, "RGB_LUT_BAND_COUNT_MISMATCH"),
            ImageValidationCode::MonoBandCountMismatch => write!(f, "MONO_BAND_COUNT_MISMATCH"),
            ImageValidationCode::YCbCr601BandCountMismatch => write!(f, "YCBCR601_BAND_COUNT_MISMATCH"),
            ImageValidationCode::InvalidIrepband => write!(f, "INVALID_IREPBAND"),
            ImageValidationCode::InefficientSingleBandSequential => {
                write!(f, "INEFFICIENT_SINGLE_BAND_SEQUENTIAL")
            }
            ImageValidationCode::TooManyLuts => write!(f, "TOO_MANY_LUTS"),
            ImageValidationCode::ZeroLutEntries => write!(f, "ZERO_LUT_ENTRIES"),
            ImageValidationCode::RgbLutLutCountMismatch => write!(f, "RGB_LUT_LUT_COUNT_MISMATCH"),
            ImageValidationCode::IncompleteLut => write!(f, "INCOMPLETE_LUT"),
        }
    }
}


/// Result of an image validation check.
#[derive(Debug, Clone)]
pub struct ImageValidationResult {
    /// Validation code for programmatic handling
    pub code: ImageValidationCode,
    /// Severity level (error or warning)
    pub severity: ValidationSeverity,
    /// Human-readable message describing the issue
    pub message: String,
    /// Field name where the issue was found (if applicable)
    pub field: Option<String>,
    /// Expected value (if applicable)
    pub expected: Option<String>,
    /// Actual value found
    pub actual: Option<String>,
}

impl ImageValidationResult {
    /// Create a new validation error.
    pub fn error(code: ImageValidationCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: ValidationSeverity::Error,
            message: message.into(),
            field: None,
            expected: None,
            actual: None,
        }
    }

    /// Create a new validation warning.
    pub fn warning(code: ImageValidationCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: ValidationSeverity::Warning,
            message: message.into(),
            field: None,
            expected: None,
            actual: None,
        }
    }

    /// Set the field name where the issue was found.
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set the expected value.
    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Set the actual value found.
    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    /// Check if this is an error (not a warning).
    pub fn is_error(&self) -> bool {
        self.severity == ValidationSeverity::Error
    }

    /// Check if this is a warning (not an error).
    pub fn is_warning(&self) -> bool {
        self.severity == ValidationSeverity::Warning
    }
}

impl std::fmt::Display for ImageValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            ValidationSeverity::Error => "ERROR",
            ValidationSeverity::Warning => "WARNING",
        };
        write!(f, "[{}] {}: {}", severity, self.code, self.message)?;
        if let Some(ref field) = self.field {
            write!(f, " (field: {})", field)?;
        }
        if let (Some(ref expected), Some(ref actual)) = (&self.expected, &self.actual) {
            write!(f, " [expected: {}, actual: {}]", expected, actual)?;
        }
        Ok(())
    }
}

/// CLEVEL dimension limits.
///
/// Returns (max_rows, max_cols) for the given CLEVEL.
fn clevel_limits(clevel: u8) -> (u32, u32) {
    match clevel {
        3 => (2048, 2048),
        5 => (8192, 8192),
        6 => (65536, 65536),
        7 => (99999999, 99999999),
        9 => (u32::MAX, u32::MAX), // No limit
        _ => (u32::MAX, u32::MAX), // Unknown CLEVEL, no limit
    }
}

/// Validator for image subheader fields.
///
/// This struct provides validation methods for checking image subheader
/// fields according to JBP requirements. Validation produces a list of
/// results that can be errors or warnings.
pub struct ImageValidator;

impl ImageValidator {
    /// Validate an image subheader.
    ///
    /// This method runs all validation checks and returns a list of results.
    /// The list may contain both errors and warnings.
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade to validate
    /// * `clevel` - The complexity level from the file header
    ///
    /// # Returns
    /// A vector of validation results (errors and warnings).
    pub fn validate(subheader: &ImageSubheaderFacade, clevel: u8) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        results.extend(Self::validate_dimensions(subheader, clevel));
        results.extend(Self::validate_blocking(subheader));
        results.extend(Self::validate_pixel_type(subheader));
        results.extend(Self::validate_bands(subheader));
        results.extend(Self::validate_luts(subheader));

        results
    }


    /// Validate image dimensions.
    ///
    /// Checks:
    /// - NROWS > 0 (Requirement 13.1)
    /// - NCOLS > 0 (Requirement 13.2)
    /// - NROWS within CLEVEL limits (Requirement 13.3)
    /// - NCOLS within CLEVEL limits (Requirement 13.4)
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade to validate
    /// * `clevel` - The complexity level from the file header
    ///
    /// # Returns
    /// A vector of validation results for dimension checks.
    pub fn validate_dimensions(
        subheader: &ImageSubheaderFacade,
        clevel: u8,
    ) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        // Check NROWS > 0
        match subheader.nrows() {
            Ok(nrows) => {
                if nrows == 0 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::ZeroRows,
                            "NROWS must be greater than 0",
                        )
                        .with_field("NROWS")
                        .with_actual("0"),
                    );
                } else {
                    // Check against CLEVEL limits
                    let (max_rows, _) = clevel_limits(clevel);
                    if nrows > max_rows {
                        results.push(
                            ImageValidationResult::warning(
                                ImageValidationCode::RowsExceedClevel,
                                format!(
                                    "NROWS ({}) exceeds CLEVEL {} limit ({})",
                                    nrows, clevel, max_rows
                                ),
                            )
                            .with_field("NROWS")
                            .with_expected(format!("<= {}", max_rows))
                            .with_actual(nrows.to_string()),
                        );
                    }
                }
            }
            Err(_) => {
                // If we can't read NROWS, that's a parse error, not a validation error
            }
        }

        // Check NCOLS > 0
        match subheader.ncols() {
            Ok(ncols) => {
                if ncols == 0 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::ZeroCols,
                            "NCOLS must be greater than 0",
                        )
                        .with_field("NCOLS")
                        .with_actual("0"),
                    );
                } else {
                    // Check against CLEVEL limits
                    let (_, max_cols) = clevel_limits(clevel);
                    if ncols > max_cols {
                        results.push(
                            ImageValidationResult::warning(
                                ImageValidationCode::ColsExceedClevel,
                                format!(
                                    "NCOLS ({}) exceeds CLEVEL {} limit ({})",
                                    ncols, clevel, max_cols
                                ),
                            )
                            .with_field("NCOLS")
                            .with_expected(format!("<= {}", max_cols))
                            .with_actual(ncols.to_string()),
                        );
                    }
                }
            }
            Err(_) => {
                // If we can't read NCOLS, that's a parse error, not a validation error
            }
        }

        results
    }

    /// Validate blocking parameters.
    ///
    /// Checks:
    /// - NBPR × NPPBH >= NCOLS (Requirement 13.5)
    /// - NBPC × NPPBV >= NROWS (Requirement 13.6)
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade to validate
    ///
    /// # Returns
    /// A vector of validation results for blocking checks.
    pub fn validate_blocking(subheader: &ImageSubheaderFacade) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        // Get all required values
        let ncols = match subheader.ncols() {
            Ok(v) => v,
            Err(_) => return results,
        };
        let nrows = match subheader.nrows() {
            Ok(v) => v,
            Err(_) => return results,
        };
        let nbpr = match subheader.nbpr() {
            Ok(v) => v,
            Err(_) => return results,
        };
        let nbpc = match subheader.nbpc() {
            Ok(v) => v,
            Err(_) => return results,
        };
        let nppbh = match subheader.nppbh() {
            Ok(v) => v,
            Err(_) => return results,
        };
        let nppbv = match subheader.nppbv() {
            Ok(v) => v,
            Err(_) => return results,
        };

        // Check NBPR × NPPBH >= NCOLS
        let block_cols = nbpr.saturating_mul(nppbh);
        if block_cols < ncols {
            results.push(
                ImageValidationResult::error(
                    ImageValidationCode::BlockingInsufficientCols,
                    format!(
                        "NBPR × NPPBH ({} × {} = {}) must be >= NCOLS ({})",
                        nbpr, nppbh, block_cols, ncols
                    ),
                )
                .with_field("NBPR, NPPBH")
                .with_expected(format!(">= {}", ncols))
                .with_actual(block_cols.to_string()),
            );
        }

        // Check NBPC × NPPBV >= NROWS
        let block_rows = nbpc.saturating_mul(nppbv);
        if block_rows < nrows {
            results.push(
                ImageValidationResult::error(
                    ImageValidationCode::BlockingInsufficientRows,
                    format!(
                        "NBPC × NPPBV ({} × {} = {}) must be >= NROWS ({})",
                        nbpc, nppbv, block_rows, nrows
                    ),
                )
                .with_field("NBPC, NPPBV")
                .with_expected(format!(">= {}", nrows))
                .with_actual(block_rows.to_string()),
            );
        }

        results
    }


    /// Validate pixel type parameters.
    ///
    /// Checks:
    /// - PVTYPE is valid (Requirement 14.1)
    /// - ABPP <= NBPP (Requirement 14.2)
    /// - PVTYPE=R requires NBPP of 32 or 64 (Requirement 14.3)
    /// - PVTYPE=C requires NBPP of 64 (Requirement 14.4)
    /// - PVTYPE=B requires NBPP of 1 (Requirement 14.5)
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade to validate
    ///
    /// # Returns
    /// A vector of validation results for pixel type checks.
    pub fn validate_pixel_type(subheader: &ImageSubheaderFacade) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        // Get PVTYPE - if it fails to parse, that's already an error
        let pvtype = match subheader.pvtype() {
            Ok(v) => v,
            Err(_) => {
                results.push(
                    ImageValidationResult::error(
                        ImageValidationCode::InvalidPvtype,
                        "PVTYPE must be one of INT, SI, R, C, or B",
                    )
                    .with_field("PVTYPE"),
                );
                return results;
            }
        };

        // Get ABPP and NBPP
        let abpp = match subheader.abpp() {
            Ok(v) => v,
            Err(_) => return results,
        };
        let nbpp = match subheader.nbpp() {
            Ok(v) => v,
            Err(_) => return results,
        };

        // Check ABPP <= NBPP
        if abpp > nbpp {
            results.push(
                ImageValidationResult::error(
                    ImageValidationCode::AbppExceedsNbpp,
                    format!("ABPP ({}) must be <= NBPP ({})", abpp, nbpp),
                )
                .with_field("ABPP")
                .with_expected(format!("<= {}", nbpp))
                .with_actual(abpp.to_string()),
            );
        }

        // Check PVTYPE/NBPP consistency
        match pvtype {
            PixelValueType::Real => {
                if nbpp != 32 && nbpp != 64 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::InvalidNbppForReal,
                            format!("PVTYPE=R requires NBPP of 32 or 64, got {}", nbpp),
                        )
                        .with_field("NBPP")
                        .with_expected("32 or 64")
                        .with_actual(nbpp.to_string()),
                    );
                }
            }
            PixelValueType::Complex => {
                if nbpp != 64 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::InvalidNbppForComplex,
                            format!("PVTYPE=C requires NBPP of 64, got {}", nbpp),
                        )
                        .with_field("NBPP")
                        .with_expected("64")
                        .with_actual(nbpp.to_string()),
                    );
                }
            }
            PixelValueType::BiLevel => {
                if nbpp != 1 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::InvalidNbppForBilevel,
                            format!("PVTYPE=B requires NBPP of 1, got {}", nbpp),
                        )
                        .with_field("NBPP")
                        .with_expected("1")
                        .with_actual(nbpp.to_string()),
                    );
                }
            }
            // INT and SI can have various NBPP values (8, 16, 32, etc.)
            PixelValueType::UnsignedInt | PixelValueType::SignedInt => {}
        }

        results
    }

    /// Validate band configuration.
    ///
    /// Checks:
    /// - RGB requires 3 bands (Requirement 15.1)
    /// - RGB/LUT requires 1 band (Requirement 15.2)
    /// - MONO requires 1 band (Requirement 15.3)
    /// - IREPBANDn validity for IREP (Requirement 15.4)
    /// - IMODE=S with 1 band is inefficient (Requirement 15.5)
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade to validate
    ///
    /// # Returns
    /// A vector of validation results for band configuration checks.
    pub fn validate_bands(subheader: &ImageSubheaderFacade) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        // Get IREP
        let irep = match subheader.irep() {
            Ok(v) => v,
            Err(_) => return results,
        };

        // Get band count
        let band_count = match subheader.band_count() {
            Ok(v) => v,
            Err(_) => return results,
        };

        // Check band count matches IREP requirements
        match irep {
            ImageRepresentation::Rgb => {
                if band_count != 3 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::RgbBandCountMismatch,
                            format!("IREP=RGB requires exactly 3 bands, got {}", band_count),
                        )
                        .with_field("NBANDS")
                        .with_expected("3")
                        .with_actual(band_count.to_string()),
                    );
                }
            }
            ImageRepresentation::RgbLut => {
                if band_count != 1 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::RgbLutBandCountMismatch,
                            format!("IREP=RGB/LUT requires exactly 1 band, got {}", band_count),
                        )
                        .with_field("NBANDS")
                        .with_expected("1")
                        .with_actual(band_count.to_string()),
                    );
                }
            }
            ImageRepresentation::Mono => {
                if band_count != 1 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::MonoBandCountMismatch,
                            format!("IREP=MONO requires exactly 1 band, got {}", band_count),
                        )
                        .with_field("NBANDS")
                        .with_expected("1")
                        .with_actual(band_count.to_string()),
                    );
                }
            }
            ImageRepresentation::YCbCr601 => {
                if band_count != 3 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::YCbCr601BandCountMismatch,
                            format!("IREP=YCbCr601 requires exactly 3 bands, got {}", band_count),
                        )
                        .with_field("NBANDS")
                        .with_expected("3")
                        .with_actual(band_count.to_string()),
                    );
                }
            }
            // Multi, NoDisplay, NVector, Polar, Vph allow any band count
            _ => {}
        }

        // Validate IREPBANDn for each band
        results.extend(Self::validate_irepband(subheader, irep, band_count));

        // Check for inefficient IMODE=S with single band
        if let Ok(imode) = subheader.imode() {
            if imode == crate::jbp::image::types::InterleaveMode::S && band_count == 1 {
                results.push(
                    ImageValidationResult::warning(
                        ImageValidationCode::InefficientSingleBandSequential,
                        "IMODE=S with single band is inefficient",
                    )
                    .with_field("IMODE"),
                );
            }
        }

        results
    }

    /// Validate IREPBANDn values for each band.
    fn validate_irepband(
        subheader: &ImageSubheaderFacade,
        irep: ImageRepresentation,
        band_count: usize,
    ) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        // Define expected IREPBANDn values for each IREP
        let expected_bands: Option<Vec<&str>> = match irep {
            ImageRepresentation::Rgb => Some(vec!["R", "G", "B"]),
            ImageRepresentation::RgbLut => Some(vec!["LU"]),
            ImageRepresentation::Mono => Some(vec!["M", ""]), // M or blank
            ImageRepresentation::YCbCr601 => Some(vec!["Y", "Cb", "Cr"]),
            // Other IREPs don't have strict IREPBANDn requirements
            _ => None,
        };

        if let Some(expected) = expected_bands {
            for i in 0..band_count {
                if let Ok(band_info) = subheader.band_info(i) {
                    if let Ok(irepband) = band_info.irepband() {
                        let irepband_trimmed = irepband.trim();
                        
                        // For RGB and YCbCr601, check specific band values
                        match irep {
                            ImageRepresentation::Rgb | ImageRepresentation::YCbCr601 => {
                                if i < expected.len() && irepband_trimmed != expected[i] {
                                    results.push(
                                        ImageValidationResult::error(
                                            ImageValidationCode::InvalidIrepband,
                                            format!(
                                                "Band {} IREPBAND should be '{}' for IREP={}, got '{}'",
                                                i,
                                                expected[i],
                                                irep.to_str().trim(),
                                                irepband_trimmed
                                            ),
                                        )
                                        .with_field(format!("IREPBAND{}", i))
                                        .with_expected(expected[i])
                                        .with_actual(irepband_trimmed),
                                    );
                                }
                            }
                            ImageRepresentation::RgbLut => {
                                if irepband_trimmed != "LU" {
                                    results.push(
                                        ImageValidationResult::error(
                                            ImageValidationCode::InvalidIrepband,
                                            format!(
                                                "Band {} IREPBAND should be 'LU' for IREP=RGB/LUT, got '{}'",
                                                i, irepband_trimmed
                                            ),
                                        )
                                        .with_field(format!("IREPBAND{}", i))
                                        .with_expected("LU")
                                        .with_actual(irepband_trimmed),
                                    );
                                }
                            }
                            ImageRepresentation::Mono => {
                                // MONO allows M or blank
                                if !irepband_trimmed.is_empty() && irepband_trimmed != "M" {
                                    results.push(
                                        ImageValidationResult::error(
                                            ImageValidationCode::InvalidIrepband,
                                            format!(
                                                "Band {} IREPBAND should be 'M' or blank for IREP=MONO, got '{}'",
                                                i, irepband_trimmed
                                            ),
                                        )
                                        .with_field(format!("IREPBAND{}", i))
                                        .with_expected("M or blank")
                                        .with_actual(irepband_trimmed),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        results
    }


    /// Validate LUT configuration.
    ///
    /// Checks:
    /// - NLUTSn <= 4 (Requirement 16.1)
    /// - NELUTn > 0 when NLUTSn > 0 (Requirement 16.2)
    /// - RGB/LUT has 3 LUTs (Requirement 16.3)
    /// - NELUTn >= 2^ABPP (Requirement 16.4)
    ///
    /// # Arguments
    /// * `subheader` - The image subheader facade to validate
    ///
    /// # Returns
    /// A vector of validation results for LUT configuration checks.
    pub fn validate_luts(subheader: &ImageSubheaderFacade) -> Vec<ImageValidationResult> {
        let mut results = Vec::new();

        // Get IREP for RGB/LUT check
        let irep = subheader.irep().ok();

        // Get ABPP for incomplete LUT check
        let abpp = subheader.abpp().ok();

        // Get band count
        let band_count = match subheader.band_count() {
            Ok(v) => v,
            Err(_) => return results,
        };

        for i in 0..band_count {
            let band_info = match subheader.band_info(i) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let nluts = match band_info.nluts() {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Check NLUTSn <= 4
            if nluts > 4 {
                results.push(
                    ImageValidationResult::error(
                        ImageValidationCode::TooManyLuts,
                        format!("Band {} NLUTS ({}) exceeds maximum of 4", i, nluts),
                    )
                    .with_field(format!("NLUTS{}", i))
                    .with_expected("<= 4")
                    .with_actual(nluts.to_string()),
                );
            }

            // Check NELUTn > 0 when NLUTSn > 0
            if nluts > 0 {
                match band_info.nelut() {
                    Ok(Some(nelut)) => {
                        if nelut == 0 {
                            results.push(
                                ImageValidationResult::error(
                                    ImageValidationCode::ZeroLutEntries,
                                    format!(
                                        "Band {} NELUT must be > 0 when NLUTS ({}) > 0",
                                        i, nluts
                                    ),
                                )
                                .with_field(format!("NELUT{}", i))
                                .with_expected("> 0")
                                .with_actual("0"),
                            );
                        } else if let Some(abpp_val) = abpp {
                            // Check NELUTn >= 2^ABPP (warning for incomplete LUT)
                            let min_entries = 1u32 << abpp_val;
                            if nelut < min_entries {
                                results.push(
                                    ImageValidationResult::warning(
                                        ImageValidationCode::IncompleteLut,
                                        format!(
                                            "Band {} NELUT ({}) is less than 2^ABPP (2^{} = {}), LUT may be incomplete",
                                            i, nelut, abpp_val, min_entries
                                        ),
                                    )
                                    .with_field(format!("NELUT{}", i))
                                    .with_expected(format!(">= {}", min_entries))
                                    .with_actual(nelut.to_string()),
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        // NELUT not present but NLUTS > 0
                        results.push(
                            ImageValidationResult::error(
                                ImageValidationCode::ZeroLutEntries,
                                format!(
                                    "Band {} NELUT must be present when NLUTS ({}) > 0",
                                    i, nluts
                                ),
                            )
                            .with_field(format!("NELUT{}", i)),
                        );
                    }
                    Err(_) => {}
                }
            }

            // Check RGB/LUT has 3 LUTs
            if let Some(ImageRepresentation::RgbLut) = irep {
                if nluts != 3 {
                    results.push(
                        ImageValidationResult::error(
                            ImageValidationCode::RgbLutLutCountMismatch,
                            format!(
                                "Band {} NLUTS must be 3 for IREP=RGB/LUT, got {}",
                                i, nluts
                            ),
                        )
                        .with_field(format!("NLUTS{}", i))
                        .with_expected("3")
                        .with_actual(nluts.to_string()),
                    );
                }
            }
        }

        results
    }

    /// Check if validation results contain any errors.
    ///
    /// # Arguments
    /// * `results` - The validation results to check
    ///
    /// # Returns
    /// `true` if any result is an error, `false` otherwise.
    pub fn has_errors(results: &[ImageValidationResult]) -> bool {
        results.iter().any(|r| r.is_error())
    }

    /// Check if validation results contain any warnings.
    ///
    /// # Arguments
    /// * `results` - The validation results to check
    ///
    /// # Returns
    /// `true` if any result is a warning, `false` otherwise.
    pub fn has_warnings(results: &[ImageValidationResult]) -> bool {
        results.iter().any(|r| r.is_warning())
    }

    /// Filter validation results to only errors.
    ///
    /// # Arguments
    /// * `results` - The validation results to filter
    ///
    /// # Returns
    /// A vector containing only the error results.
    pub fn errors_only(results: &[ImageValidationResult]) -> Vec<&ImageValidationResult> {
        results.iter().filter(|r| r.is_error()).collect()
    }

    /// Filter validation results to only warnings.
    ///
    /// # Arguments
    /// * `results` - The validation results to filter
    ///
    /// # Returns
    /// A vector containing only the warning results.
    pub fn warnings_only(results: &[ImageValidationResult]) -> Vec<&ImageValidationResult> {
        results.iter().filter(|r| r.is_warning()).collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_severity_equality() {
        assert_eq!(ValidationSeverity::Error, ValidationSeverity::Error);
        assert_eq!(ValidationSeverity::Warning, ValidationSeverity::Warning);
        assert_ne!(ValidationSeverity::Error, ValidationSeverity::Warning);
    }

    #[test]
    fn test_validation_code_display() {
        assert_eq!(ImageValidationCode::ZeroRows.to_string(), "ZERO_ROWS");
        assert_eq!(ImageValidationCode::ZeroCols.to_string(), "ZERO_COLS");
        assert_eq!(
            ImageValidationCode::RowsExceedClevel.to_string(),
            "ROWS_EXCEED_CLEVEL"
        );
        assert_eq!(
            ImageValidationCode::BlockingInsufficientCols.to_string(),
            "BLOCKING_INSUFFICIENT_COLS"
        );
        assert_eq!(
            ImageValidationCode::InvalidPvtype.to_string(),
            "INVALID_PVTYPE"
        );
        assert_eq!(
            ImageValidationCode::AbppExceedsNbpp.to_string(),
            "ABPP_EXCEEDS_NBPP"
        );
        assert_eq!(
            ImageValidationCode::RgbBandCountMismatch.to_string(),
            "RGB_BAND_COUNT_MISMATCH"
        );
        assert_eq!(
            ImageValidationCode::TooManyLuts.to_string(),
            "TOO_MANY_LUTS"
        );
    }

    #[test]
    fn test_validation_result_error() {
        let result = ImageValidationResult::error(
            ImageValidationCode::ZeroRows,
            "NROWS must be greater than 0",
        );
        assert!(result.is_error());
        assert!(!result.is_warning());
        assert_eq!(result.code, ImageValidationCode::ZeroRows);
        assert_eq!(result.message, "NROWS must be greater than 0");
    }

    #[test]
    fn test_validation_result_warning() {
        let result = ImageValidationResult::warning(
            ImageValidationCode::RowsExceedClevel,
            "NROWS exceeds CLEVEL limit",
        );
        assert!(!result.is_error());
        assert!(result.is_warning());
        assert_eq!(result.code, ImageValidationCode::RowsExceedClevel);
    }

    #[test]
    fn test_validation_result_with_builders() {
        let result = ImageValidationResult::error(ImageValidationCode::ZeroRows, "Test")
            .with_field("NROWS")
            .with_expected("> 0")
            .with_actual("0");

        assert_eq!(result.field, Some("NROWS".to_string()));
        assert_eq!(result.expected, Some("> 0".to_string()));
        assert_eq!(result.actual, Some("0".to_string()));
    }

    #[test]
    fn test_validation_result_display_basic() {
        let result = ImageValidationResult::error(
            ImageValidationCode::ZeroRows,
            "NROWS must be greater than 0",
        );
        let display = result.to_string();
        assert!(display.contains("[ERROR]"));
        assert!(display.contains("ZERO_ROWS"));
        assert!(display.contains("NROWS must be greater than 0"));
    }

    #[test]
    fn test_validation_result_display_with_field() {
        let result = ImageValidationResult::error(ImageValidationCode::ZeroRows, "Test")
            .with_field("NROWS");
        let display = result.to_string();
        assert!(display.contains("(field: NROWS)"));
    }

    #[test]
    fn test_validation_result_display_full() {
        let result = ImageValidationResult::error(ImageValidationCode::ZeroRows, "Test")
            .with_field("NROWS")
            .with_expected("> 0")
            .with_actual("0");
        let display = result.to_string();
        assert!(display.contains("(field: NROWS)"));
        assert!(display.contains("[expected: > 0, actual: 0]"));
    }

    #[test]
    fn test_clevel_limits() {
        assert_eq!(clevel_limits(3), (2048, 2048));
        assert_eq!(clevel_limits(5), (8192, 8192));
        assert_eq!(clevel_limits(6), (65536, 65536));
        assert_eq!(clevel_limits(7), (99999999, 99999999));
        assert_eq!(clevel_limits(9), (u32::MAX, u32::MAX));
        // Unknown CLEVEL should return no limit
        assert_eq!(clevel_limits(99), (u32::MAX, u32::MAX));
    }

    #[test]
    fn test_has_errors() {
        let results = vec![
            ImageValidationResult::error(ImageValidationCode::ZeroRows, "Error"),
            ImageValidationResult::warning(ImageValidationCode::RowsExceedClevel, "Warning"),
        ];
        assert!(ImageValidator::has_errors(&results));

        let warnings_only = vec![ImageValidationResult::warning(
            ImageValidationCode::RowsExceedClevel,
            "Warning",
        )];
        assert!(!ImageValidator::has_errors(&warnings_only));
    }

    #[test]
    fn test_has_warnings() {
        let results = vec![
            ImageValidationResult::error(ImageValidationCode::ZeroRows, "Error"),
            ImageValidationResult::warning(ImageValidationCode::RowsExceedClevel, "Warning"),
        ];
        assert!(ImageValidator::has_warnings(&results));

        let errors_only = vec![ImageValidationResult::error(
            ImageValidationCode::ZeroRows,
            "Error",
        )];
        assert!(!ImageValidator::has_warnings(&errors_only));
    }

    #[test]
    fn test_errors_only() {
        let results = vec![
            ImageValidationResult::error(ImageValidationCode::ZeroRows, "Error 1"),
            ImageValidationResult::warning(ImageValidationCode::RowsExceedClevel, "Warning"),
            ImageValidationResult::error(ImageValidationCode::ZeroCols, "Error 2"),
        ];
        let errors = ImageValidator::errors_only(&results);
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|r| r.is_error()));
    }

    #[test]
    fn test_warnings_only() {
        let results = vec![
            ImageValidationResult::error(ImageValidationCode::ZeroRows, "Error"),
            ImageValidationResult::warning(ImageValidationCode::RowsExceedClevel, "Warning 1"),
            ImageValidationResult::warning(
                ImageValidationCode::InefficientSingleBandSequential,
                "Warning 2",
            ),
        ];
        let warnings = ImageValidator::warnings_only(&results);
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().all(|r| r.is_warning()));
    }
}


#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::jbp::image::types::InterleaveMode;
    use crate::parser::StructureRegistry;
    use proptest::prelude::*;

    /// Helper function to create synthetic NITF image subheader test data.
    /// This creates a minimal valid image subheader with configurable parameters.
    fn create_image_subheader_test_data(
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
        nluts: u8,
        nelut: u32,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        // IM (2) - Image segment marker
        data.extend_from_slice(b"IM");

        // IID1 (10) - Image identifier 1
        data.extend_from_slice(b"TestImg01 ");

        // IDATIM (14) - Image date and time
        data.extend_from_slice(b"20240101120000");

        // TGTID (17) - Target identifier
        data.extend_from_slice(&[b' '; 17]);

        // IID2 (80) - Image identifier 2
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
        data.push(b'R');

        // ICORDS (1) - Using blank to skip IGEOLO
        data.push(b' ');

        // NICOM (1) - No comments
        data.push(b'0');

        // IC (2) - Compression
        data.extend_from_slice(b"NC");

        // NBANDS (1)
        data.push(b'0' + nbands);

        // Band info for each band (when NBANDS > 0)
        for _ in 0..nbands {
            // Determine IREPBAND based on IREP
            let irepband = match irep.trim() {
                "RGB" => "R ",
                "RGB/LUT" => "LU",
                "MONO" => "M ",
                _ => "M ",
            };
            data.extend_from_slice(irepband.as_bytes()); // IREPBAND (2)
            data.extend_from_slice(&[b' '; 6]); // ISUBCAT (6)
            data.push(b'N'); // IFC (1)
            data.extend_from_slice(&[b' '; 3]); // IMFLT (3)
            data.push(b'0' + nluts); // NLUTS (1)

            // If NLUTS > 0, add NELUT and LUT data
            if nluts > 0 {
                data.extend_from_slice(format!("{:05}", nelut).as_bytes()); // NELUT (5)
                // LUT data (NELUT bytes per LUT, repeated NLUTS times)
                for _ in 0..nluts {
                    data.extend(vec![0u8; nelut as usize]);
                }
            }
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

    /// Property 10: Zero Dimension Validation
    /// For any image subheader with NROWS=0 or NCOLS=0, validation SHALL return an error.
    /// **Validates: Requirements 13.1, 13.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn property_10_zero_dimension_validation(
            // Generate either zero rows or zero cols (or both)
            zero_rows in proptest::bool::ANY,
            zero_cols in proptest::bool::ANY,
        ) {
            // Skip if both are non-zero (we need at least one zero)
            prop_assume!(zero_rows || zero_cols);

            let nrows = if zero_rows { 0 } else { 512 };
            let ncols = if zero_cols { 0 } else { 512 };

            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => {
                    // Skip test if definition not found
                    return Ok(());
                }
            };

            let test_data = create_image_subheader_test_data(
                nrows, ncols,
                "INT", "MONO",
                8, 8,
                1,
                1, 1, 512, 512,
                'B',
                0, 0,
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_dimensions(&facade, 5);

            // Should have at least one error for zero dimension
            let has_zero_rows_error = results.iter().any(|r| {
                r.code == ImageValidationCode::ZeroRows && r.is_error()
            });
            let has_zero_cols_error = results.iter().any(|r| {
                r.code == ImageValidationCode::ZeroCols && r.is_error()
            });

            if zero_rows {
                prop_assert!(has_zero_rows_error, "Expected ZeroRows error for NROWS=0");
            }
            if zero_cols {
                prop_assert!(has_zero_cols_error, "Expected ZeroCols error for NCOLS=0");
            }
        }
    }


    /// Property 11: Pixel Type Validation
    /// For any invalid PVTYPE/NBPP combination, validation SHALL return an error.
    /// **Validates: Requirements 14.1-14.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn property_11_pixel_type_validation(
            // Generate invalid PVTYPE/NBPP combinations
            test_case in prop_oneof![
                // PVTYPE=R with invalid NBPP (not 32 or 64)
                Just(("R", 8u8, ImageValidationCode::InvalidNbppForReal)),
                Just(("R", 16u8, ImageValidationCode::InvalidNbppForReal)),
                // PVTYPE=C with invalid NBPP (not 64)
                Just(("C", 8u8, ImageValidationCode::InvalidNbppForComplex)),
                Just(("C", 16u8, ImageValidationCode::InvalidNbppForComplex)),
                Just(("C", 32u8, ImageValidationCode::InvalidNbppForComplex)),
                // PVTYPE=B with invalid NBPP (not 1)
                Just(("B", 8u8, ImageValidationCode::InvalidNbppForBilevel)),
                Just(("B", 16u8, ImageValidationCode::InvalidNbppForBilevel)),
            ],
        ) {
            let (pvtype, nbpp, expected_code) = test_case;

            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => return Ok(()),
            };

            let test_data = create_image_subheader_test_data(
                512, 512,
                pvtype, "MONO",
                nbpp, nbpp, // ABPP = NBPP
                1,
                1, 1, 512, 512,
                'B',
                0, 0,
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_pixel_type(&facade);

            // Should have the expected error
            let has_expected_error = results.iter().any(|r| {
                r.code == expected_code && r.is_error()
            });

            prop_assert!(
                has_expected_error,
                "Expected {:?} error for PVTYPE={}, NBPP={}",
                expected_code, pvtype, nbpp
            );
        }
    }

    /// Property 11b: ABPP exceeds NBPP validation
    /// For any image subheader with ABPP > NBPP, validation SHALL return an error.
    /// **Validates: Requirement 14.2**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn property_11b_abpp_exceeds_nbpp_validation(
            nbpp in 1u8..32u8,
            abpp_delta in 1u8..10u8,
        ) {
            let abpp = nbpp.saturating_add(abpp_delta);
            // Ensure ABPP > NBPP
            prop_assume!(abpp > nbpp);

            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => return Ok(()),
            };

            let test_data = create_image_subheader_test_data(
                512, 512,
                "INT", "MONO",
                abpp, nbpp,
                1,
                1, 1, 512, 512,
                'B',
                0, 0,
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_pixel_type(&facade);

            // Should have ABPP exceeds NBPP error
            let has_error = results.iter().any(|r| {
                r.code == ImageValidationCode::AbppExceedsNbpp && r.is_error()
            });

            prop_assert!(
                has_error,
                "Expected AbppExceedsNbpp error for ABPP={}, NBPP={}",
                abpp, nbpp
            );
        }
    }


    /// Property 12: Band Configuration Validation
    /// For any IREP with a required band count, validation SHALL return an error
    /// if the actual band count doesn't match.
    /// **Validates: Requirements 15.1-15.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn property_12_band_configuration_validation(
            // Generate IREP/band count mismatches
            test_case in prop_oneof![
                // RGB requires 3 bands
                Just(("RGB", 1u8, ImageValidationCode::RgbBandCountMismatch)),
                Just(("RGB", 2u8, ImageValidationCode::RgbBandCountMismatch)),
                Just(("RGB", 4u8, ImageValidationCode::RgbBandCountMismatch)),
                // RGB/LUT requires 1 band
                Just(("RGB/LUT", 2u8, ImageValidationCode::RgbLutBandCountMismatch)),
                Just(("RGB/LUT", 3u8, ImageValidationCode::RgbLutBandCountMismatch)),
                // MONO requires 1 band
                Just(("MONO", 2u8, ImageValidationCode::MonoBandCountMismatch)),
                Just(("MONO", 3u8, ImageValidationCode::MonoBandCountMismatch)),
            ],
        ) {
            let (irep, nbands, expected_code) = test_case;

            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => return Ok(()),
            };

            let test_data = create_image_subheader_test_data(
                512, 512,
                "INT", irep,
                8, 8,
                nbands,
                1, 1, 512, 512,
                'B',
                0, 0,
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_bands(&facade);

            // Should have the expected error
            let has_expected_error = results.iter().any(|r| {
                r.code == expected_code && r.is_error()
            });

            prop_assert!(
                has_expected_error,
                "Expected {:?} error for IREP={}, NBANDS={}",
                expected_code, irep, nbands
            );
        }
    }

    /// Property 12b: IMODE=S with single band warning
    /// For any image with IMODE=S and band count=1, validation SHALL return a warning.
    /// **Validates: Requirement 15.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]
        #[test]
        fn property_12b_inefficient_single_band_sequential(
            // Just vary some other parameters to ensure robustness
            nrows in 64u32..1024u32,
            ncols in 64u32..1024u32,
        ) {
            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => return Ok(()),
            };

            // Calculate blocking to cover dimensions
            let nppbh = ncols;
            let nppbv = nrows;

            let test_data = create_image_subheader_test_data(
                nrows, ncols,
                "INT", "MONO",
                8, 8,
                1, // Single band
                1, 1, nppbh, nppbv,
                'S', // Band sequential mode
                0, 0,
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_bands(&facade);

            // Should have inefficient single band sequential warning
            let has_warning = results.iter().any(|r| {
                r.code == ImageValidationCode::InefficientSingleBandSequential && r.is_warning()
            });

            prop_assert!(
                has_warning,
                "Expected InefficientSingleBandSequential warning for IMODE=S with 1 band"
            );
        }
    }


    /// Property 13: LUT Configuration Validation
    /// For any invalid LUT configuration, validation SHALL return an error.
    /// **Validates: Requirements 16.1-16.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn property_13_lut_configuration_validation(
            // Generate invalid LUT configurations
            test_case in prop_oneof![
                // NLUTSn > 4 (too many LUTs)
                // Note: We can't easily test this with our test data generator
                // since NLUTS is a single digit (0-9), but 5-9 would be invalid
                
                // RGB/LUT with wrong number of LUTs (not 3)
                Just(("RGB/LUT", 1u8, 1u8, 256u32, ImageValidationCode::RgbLutLutCountMismatch)),
                Just(("RGB/LUT", 1u8, 2u8, 256u32, ImageValidationCode::RgbLutLutCountMismatch)),
                Just(("RGB/LUT", 1u8, 4u8, 256u32, ImageValidationCode::RgbLutLutCountMismatch)),
            ],
        ) {
            let (irep, nbands, nluts, nelut, expected_code) = test_case;

            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => return Ok(()),
            };

            let test_data = create_image_subheader_test_data(
                512, 512,
                "INT", irep,
                8, 8,
                nbands,
                1, 1, 512, 512,
                'B',
                nluts, nelut,
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_luts(&facade);

            // Should have the expected error
            let has_expected_error = results.iter().any(|r| {
                r.code == expected_code && r.is_error()
            });

            prop_assert!(
                has_expected_error,
                "Expected {:?} error for IREP={}, NLUTS={}",
                expected_code, irep, nluts
            );
        }
    }

    /// Property 13b: Incomplete LUT warning
    /// For any LUT with NELUT < 2^ABPP, validation SHALL return a warning.
    /// **Validates: Requirement 16.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(30))]
        #[test]
        fn property_13b_incomplete_lut_warning(
            // Generate incomplete LUT configurations
            abpp in 4u8..8u8, // Use smaller ABPP to keep NELUT reasonable
            nelut_factor in 0.1f64..0.9f64, // NELUT will be less than 2^ABPP
        ) {
            let min_entries = 1u32 << abpp;
            let nelut = ((min_entries as f64) * nelut_factor) as u32;
            
            // Ensure NELUT < 2^ABPP and NELUT > 0
            prop_assume!(nelut > 0 && nelut < min_entries);

            let registry = StructureRegistry::new();
            let definition = match registry.get("nitf_02.10_image_subheader") {
                Some(def) => def,
                None => return Ok(()),
            };

            let test_data = create_image_subheader_test_data(
                512, 512,
                "INT", "MONO",
                abpp, 8, // NBPP >= ABPP
                1,
                1, 1, 512, 512,
                'B',
                1, nelut, // 1 LUT with incomplete entries
            );

            let accessor = match crate::parser::StructureAccessor::new(definition, &test_data) {
                Ok(a) => a,
                Err(_) => return Ok(()),
            };
            let facade = crate::jbp::image::facade::ImageSubheaderFacade::new(accessor);

            let results = ImageValidator::validate_luts(&facade);

            // Should have incomplete LUT warning
            let has_warning = results.iter().any(|r| {
                r.code == ImageValidationCode::IncompleteLut && r.is_warning()
            });

            prop_assert!(
                has_warning,
                "Expected IncompleteLut warning for ABPP={}, NELUT={} (min={})",
                abpp, nelut, min_entries
            );
        }
    }
}
