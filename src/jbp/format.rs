//! Format detection utilities for NITF/NSIF files.
//!
//! This module provides functions for detecting NITF/NSIF file formats:
//! - Extension-based detection for file path filtering
//! - Magic number validation during parsing

use std::path::Path;

use crate::jbp::error::JBPError;
use crate::jbp::types::NitfFormat;

/// NITF 2.1 magic number (first 9 bytes of file)
pub const NITF21_MAGIC: &[u8; 9] = b"NITF02.10";

/// NSIF 1.0 magic number (first 9 bytes of file)
pub const NSIF10_MAGIC: &[u8; 9] = b"NSIF01.00";

/// Minimum file size required for magic number validation
pub const MAGIC_SIZE: usize = 9;

/// Detect if a file path indicates a NITF/NSIF file based on extension.
///
/// This function checks the file extension (case-insensitive) to determine
/// if the file is likely a NITF or NSIF file. Supported extensions:
/// - `.ntf` - NITF file
/// - `.nitf` - NITF file
/// - `.nsif` - NSIF file
/// - `.nsf` - NSIF file (alternate extension)
///
/// # Arguments
/// * `path` - File path to check
///
/// # Returns
/// `true` if the extension indicates a NITF/NSIF file, `false` otherwise.
///
/// # Example
/// ```
/// use std::path::Path;
/// use aws_osml_io::jbp::format::is_nitf_extension;
///
/// assert!(is_nitf_extension(Path::new("image.ntf")));
/// assert!(is_nitf_extension(Path::new("image.NITF")));
/// assert!(is_nitf_extension(Path::new("data.nsif")));
/// assert!(is_nitf_extension(Path::new("data.nsf")));
/// assert!(!is_nitf_extension(Path::new("image.jpg")));
/// ```
pub fn is_nitf_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_lowercase();
            lower == "ntf" || lower == "nitf" || lower == "nsif" || lower == "nsf"
        })
        .unwrap_or(false)
}

/// Validate NITF magic number from file header bytes.
///
/// This function validates the first 9 bytes of a file to determine
/// if it is a valid NITF 2.1 or NSIF 1.0 file. The magic number is:
/// - `NITF02.10` for NITF 2.1 files
/// - `NSIF01.00` for NSIF 1.0 files
///
/// # Arguments
/// * `data` - Byte slice containing at least the first 9 bytes of the file
///
/// # Returns
/// - `Ok(NitfFormat::Nitf21)` if the magic number is "NITF02.10"
/// - `Ok(NitfFormat::Nsif10)` if the magic number is "NSIF01.00"
/// - `Err(JBPError::InvalidFormat)` if the file is too small or has invalid magic
///
/// # Example
/// ```
/// use aws_osml_io::jbp::format::validate_nitf_magic;
/// use aws_osml_io::jbp::types::NitfFormat;
///
/// let nitf_data = b"NITF02.10rest of file...";
/// assert_eq!(validate_nitf_magic(nitf_data).unwrap(), NitfFormat::Nitf21);
///
/// let nsif_data = b"NSIF01.00rest of file...";
/// assert_eq!(validate_nitf_magic(nsif_data).unwrap(), NitfFormat::Nsif10);
///
/// let invalid_data = b"INVALID";
/// assert!(validate_nitf_magic(invalid_data).is_err());
/// ```
pub fn validate_nitf_magic(data: &[u8]) -> Result<NitfFormat, JBPError> {
    if data.len() < MAGIC_SIZE {
        return Err(JBPError::InvalidFormat {
            message: format!(
                "File too small for NITF header: expected at least {} bytes, got {}",
                MAGIC_SIZE,
                data.len()
            ),
        });
    }

    let magic = &data[0..MAGIC_SIZE];

    if magic == NITF21_MAGIC {
        Ok(NitfFormat::Nitf21)
    } else if magic == NSIF10_MAGIC {
        Ok(NitfFormat::Nsif10)
    } else {
        Err(JBPError::InvalidFormat {
            message: format!(
                "Invalid NITF magic number: expected 'NITF02.10' or 'NSIF01.00', got '{}'",
                String::from_utf8_lossy(magic)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ==================== is_nitf_extension tests ====================

    #[test]
    fn is_nitf_extension_ntf_lowercase() {
        assert!(is_nitf_extension(Path::new("image.ntf")));
    }

    #[test]
    fn is_nitf_extension_ntf_uppercase() {
        assert!(is_nitf_extension(Path::new("image.NTF")));
    }

    #[test]
    fn is_nitf_extension_ntf_mixed_case() {
        assert!(is_nitf_extension(Path::new("image.NtF")));
    }

    #[test]
    fn is_nitf_extension_nitf_lowercase() {
        assert!(is_nitf_extension(Path::new("image.nitf")));
    }

    #[test]
    fn is_nitf_extension_nitf_uppercase() {
        assert!(is_nitf_extension(Path::new("image.NITF")));
    }

    #[test]
    fn is_nitf_extension_nsif_lowercase() {
        assert!(is_nitf_extension(Path::new("data.nsif")));
    }

    #[test]
    fn is_nitf_extension_nsif_uppercase() {
        assert!(is_nitf_extension(Path::new("data.NSIF")));
    }

    #[test]
    fn is_nitf_extension_nsf_lowercase() {
        assert!(is_nitf_extension(Path::new("data.nsf")));
    }

    #[test]
    fn is_nitf_extension_nsf_uppercase() {
        assert!(is_nitf_extension(Path::new("data.NSF")));
    }

    #[test]
    fn is_nitf_extension_jpg_not_nitf() {
        assert!(!is_nitf_extension(Path::new("image.jpg")));
    }

    #[test]
    fn is_nitf_extension_png_not_nitf() {
        assert!(!is_nitf_extension(Path::new("image.png")));
    }

    #[test]
    fn is_nitf_extension_tiff_not_nitf() {
        assert!(!is_nitf_extension(Path::new("image.tiff")));
    }

    #[test]
    fn is_nitf_extension_no_extension() {
        assert!(!is_nitf_extension(Path::new("image")));
    }

    #[test]
    fn is_nitf_extension_empty_path() {
        assert!(!is_nitf_extension(Path::new("")));
    }

    #[test]
    fn is_nitf_extension_with_directory() {
        assert!(is_nitf_extension(Path::new("/path/to/image.ntf")));
        assert!(is_nitf_extension(Path::new("relative/path/image.NITF")));
    }

    #[test]
    fn is_nitf_extension_pathbuf() {
        let path = PathBuf::from("test.ntf");
        assert!(is_nitf_extension(&path));
    }

    // ==================== validate_nitf_magic tests ====================

    #[test]
    fn validate_nitf_magic_nitf21() {
        let data = b"NITF02.10rest of the file content";
        let result = validate_nitf_magic(data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), NitfFormat::Nitf21);
    }

    #[test]
    fn validate_nitf_magic_nsif10() {
        let data = b"NSIF01.00rest of the file content";
        let result = validate_nitf_magic(data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), NitfFormat::Nsif10);
    }

    #[test]
    fn validate_nitf_magic_exact_size() {
        let data = b"NITF02.10";
        let result = validate_nitf_magic(data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), NitfFormat::Nitf21);
    }

    #[test]
    fn validate_nitf_magic_too_small() {
        let data = b"NITF02.1"; // 8 bytes, need 9
        let result = validate_nitf_magic(data);
        assert!(result.is_err());
        match result.unwrap_err() {
            JBPError::InvalidFormat { message } => {
                assert!(message.contains("too small"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn validate_nitf_magic_empty() {
        let data: &[u8] = b"";
        let result = validate_nitf_magic(data);
        assert!(result.is_err());
        match result.unwrap_err() {
            JBPError::InvalidFormat { message } => {
                assert!(message.contains("too small"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn validate_nitf_magic_invalid_magic() {
        let data = b"INVALID00rest of file";
        let result = validate_nitf_magic(data);
        assert!(result.is_err());
        match result.unwrap_err() {
            JBPError::InvalidFormat { message } => {
                assert!(message.contains("Invalid NITF magic number"));
                assert!(message.contains("INVALID00"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn validate_nitf_magic_wrong_version() {
        let data = b"NITF02.00rest of file"; // Wrong version
        let result = validate_nitf_magic(data);
        assert!(result.is_err());
    }

    #[test]
    fn validate_nitf_magic_nitf_wrong_format() {
        let data = b"NITF01.00rest of file"; // NITF with wrong version
        let result = validate_nitf_magic(data);
        assert!(result.is_err());
    }

    #[test]
    fn validate_nitf_magic_nsif_wrong_version() {
        let data = b"NSIF02.10rest of file"; // NSIF with wrong version
        let result = validate_nitf_magic(data);
        assert!(result.is_err());
    }

    #[test]
    fn validate_nitf_magic_all_zeros() {
        let data = [0u8; 20];
        let result = validate_nitf_magic(&data);
        assert!(result.is_err());
    }

    #[test]
    fn validate_nitf_magic_binary_data() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49]; // JPEG header
        let result = validate_nitf_magic(&data);
        assert!(result.is_err());
    }
}

// Property-based tests for format detection
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    /// Property 1: Extension-Based Format Selection
    /// For any file path with extension .ntf, .nitf, or .nsif (case-insensitive),
    /// `is_nitf_extension()` SHALL return true. For other extensions, it SHALL return false.
    /// **Validates: Requirements 1.3, 1.4, 19.3**
    mod prop_1_extension_based_format_selection {
        use super::*;

        /// Strategy for generating valid NITF extensions (case variations)
        fn valid_nitf_extension() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("ntf".to_string()),
                Just("NTF".to_string()),
                Just("Ntf".to_string()),
                Just("nTf".to_string()),
                Just("ntF".to_string()),
                Just("NtF".to_string()),
                Just("nTF".to_string()),
                Just("nitf".to_string()),
                Just("NITF".to_string()),
                Just("Nitf".to_string()),
                Just("NiTf".to_string()),
                Just("niTF".to_string()),
                Just("nsif".to_string()),
                Just("NSIF".to_string()),
                Just("Nsif".to_string()),
                Just("nSiF".to_string()),
                Just("nsf".to_string()),
                Just("NSF".to_string()),
                Just("Nsf".to_string()),
            ]
        }

        /// Strategy for generating invalid (non-NITF) extensions
        fn invalid_extension() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("jpg".to_string()),
                Just("png".to_string()),
                Just("tiff".to_string()),
                Just("gif".to_string()),
                Just("bmp".to_string()),
                Just("txt".to_string()),
                Just("pdf".to_string()),
                Just("doc".to_string()),
                Just("xml".to_string()),
                Just("json".to_string()),
                Just("nit".to_string()),
                Just("nsi".to_string()),
            ]
        }

        /// Strategy for generating file names (without extension)
        fn file_name() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("image".to_string()),
                Just("data".to_string()),
                Just("test".to_string()),
                Just("sample".to_string()),
                Just("file_with_underscores".to_string()),
                Just("file-with-dashes".to_string()),
                Just("CamelCaseFile".to_string()),
                Just("123numeric".to_string()),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Valid NITF extensions are recognized
            #[test]
            fn valid_extensions_recognized(
                name in file_name(),
                ext in valid_nitf_extension(),
            ) {
                let path = PathBuf::from(format!("{}.{}", name, ext));
                prop_assert!(is_nitf_extension(&path),
                    "Extension '{}' should be recognized as NITF", ext);
            }

            /// Invalid extensions are rejected
            #[test]
            fn invalid_extensions_rejected(
                name in file_name(),
                ext in invalid_extension(),
            ) {
                let path = PathBuf::from(format!("{}.{}", name, ext));
                prop_assert!(!is_nitf_extension(&path),
                    "Extension '{}' should NOT be recognized as NITF", ext);
            }

            /// Files without extensions are rejected
            #[test]
            fn no_extension_rejected(name in file_name()) {
                let path = PathBuf::from(&name);
                prop_assert!(!is_nitf_extension(&path),
                    "File without extension should NOT be recognized as NITF");
            }

            /// Extension check is case-insensitive
            #[test]
            fn case_insensitive_check(
                name in file_name(),
                base_ext in prop_oneof![Just("ntf"), Just("nitf"), Just("nsif"), Just("nsf")],
            ) {
                // Test lowercase
                let lower_path = PathBuf::from(format!("{}.{}", name, base_ext.to_lowercase()));
                prop_assert!(is_nitf_extension(&lower_path),
                    "Lowercase extension '{}' should be recognized", base_ext.to_lowercase());

                // Test uppercase
                let upper_path = PathBuf::from(format!("{}.{}", name, base_ext.to_uppercase()));
                prop_assert!(is_nitf_extension(&upper_path),
                    "Uppercase extension '{}' should be recognized", base_ext.to_uppercase());
            }

            /// Paths with directories work correctly
            #[test]
            fn paths_with_directories(
                dir in prop_oneof![
                    Just("/path/to".to_string()),
                    Just("relative/path".to_string()),
                    Just("./current".to_string()),
                    Just("../parent".to_string()),
                ],
                name in file_name(),
                ext in valid_nitf_extension(),
            ) {
                let path = PathBuf::from(format!("{}/{}.{}", dir, name, ext));
                prop_assert!(is_nitf_extension(&path),
                    "Path with directory should recognize extension '{}'", ext);
            }
        }
    }

    /// Property 2: Magic Number Validation During Parse
    /// For any file opened by JBPDatasetReader, if the first 9 bytes are not
    /// "NITF02.10" or "NSIF01.00", the reader SHALL return an InvalidFormat error
    /// with the actual bytes found.
    /// **Validates: Requirements 1.5, 12.1, 12.2, 12.3**
    mod prop_2_magic_number_validation {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            /// Valid NITF 2.1 magic number is recognized
            #[test]
            fn nitf21_magic_recognized(
                suffix in proptest::collection::vec(any::<u8>(), 0..100),
            ) {
                let mut data = NITF21_MAGIC.to_vec();
                data.extend(suffix);

                let result = validate_nitf_magic(&data);
                prop_assert!(result.is_ok(), "NITF 2.1 magic should be recognized");
                prop_assert_eq!(result.unwrap(), NitfFormat::Nitf21);
            }

            /// Valid NSIF 1.0 magic number is recognized
            #[test]
            fn nsif10_magic_recognized(
                suffix in proptest::collection::vec(any::<u8>(), 0..100),
            ) {
                let mut data = NSIF10_MAGIC.to_vec();
                data.extend(suffix);

                let result = validate_nitf_magic(&data);
                prop_assert!(result.is_ok(), "NSIF 1.0 magic should be recognized");
                prop_assert_eq!(result.unwrap(), NitfFormat::Nsif10);
            }

            /// Files too small for magic number are rejected
            #[test]
            fn too_small_rejected(
                data in proptest::collection::vec(any::<u8>(), 0..MAGIC_SIZE),
            ) {
                let result = validate_nitf_magic(&data);
                prop_assert!(result.is_err(),
                    "Data of {} bytes should be rejected (need {})", data.len(), MAGIC_SIZE);
            }

            /// Invalid magic numbers are rejected with error containing actual bytes
            #[test]
            fn invalid_magic_rejected(
                // Generate 9+ bytes that are NOT valid magic numbers
                data in proptest::collection::vec(any::<u8>(), MAGIC_SIZE..50)
                    .prop_filter("Must not be valid magic", |d| {
                        d.len() >= MAGIC_SIZE &&
                        &d[0..MAGIC_SIZE] != NITF21_MAGIC &&
                        &d[0..MAGIC_SIZE] != NSIF10_MAGIC
                    }),
            ) {
                let result = validate_nitf_magic(&data);
                prop_assert!(result.is_err(),
                    "Invalid magic number should be rejected");

                // Verify error message contains the actual bytes found
                if let Err(err) = result {
                    let err_msg = err.to_string();
                    prop_assert!(err_msg.contains("Invalid NITF magic number"),
                        "Error should mention invalid magic number");
                }
            }

            /// Exact 9-byte valid magic is accepted
            #[test]
            fn exact_size_magic_accepted(
                format in prop_oneof![Just(NitfFormat::Nitf21), Just(NitfFormat::Nsif10)],
            ) {
                let data = match format {
                    NitfFormat::Nitf21 => NITF21_MAGIC.to_vec(),
                    NitfFormat::Nsif10 => NSIF10_MAGIC.to_vec(),
                };

                let result = validate_nitf_magic(&data);
                prop_assert!(result.is_ok(),
                    "Exact 9-byte magic should be accepted");
                prop_assert_eq!(result.unwrap(), format);
            }

            /// Magic number check is byte-exact (no partial matches)
            #[test]
            fn no_partial_matches(
                // Generate data that starts with partial magic but isn't complete
                partial in prop_oneof![
                    Just(b"NITF02.1".to_vec()),  // Missing last char
                    Just(b"NSIF01.0".to_vec()),  // Missing last char
                    Just(b"NITF02.11".to_vec()), // Wrong version
                    Just(b"NSIF01.01".to_vec()), // Wrong version
                    Just(b"NITF02.00".to_vec()), // Wrong version
                    Just(b"NSIF02.10".to_vec()), // Wrong format/version combo
                    Just(b"NITF01.00".to_vec()), // Wrong format/version combo
                ],
            ) {
                // Ensure we have at least 9 bytes
                let mut data = partial;
                while data.len() < MAGIC_SIZE {
                    data.push(0);
                }

                let result = validate_nitf_magic(&data);
                prop_assert!(result.is_err(),
                    "Partial/wrong magic {:?} should be rejected", &data[0..MAGIC_SIZE]);
            }
        }
    }
}

