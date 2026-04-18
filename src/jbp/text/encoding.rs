//! Text encoding and decoding functions for JBP text segments.
//!
//! This module provides functions for encoding and decoding text content
//! in NITF text segments, handling different character encodings (STA, U8S,
//! UT1, MTF) and line delimiter normalization.
//!
//! # Text Format Codes
//!
//! - `STA` - Standard BCS (ASCII) text with CR/LF line delimiters
//! - `MTF` - Message Text Formatting per STANAG 5500/MIL-STD-6040 (ASCII-based)
//! - `UT1` - Legacy ECS (Extended Character Set) text formatting (ISO-8859-1)
//! - `U8S` - UTF-8 text formatting
//!
//! # Line Delimiters
//!
//! JBP text segments require CR/LF (0x0D 0x0A) line delimiters. When reading,
//! these are normalized to platform-native line endings. When writing,
//! platform-native line endings are converted to CR/LF.
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::text::encoding::{decode_and_normalize, encode_with_crlf};
//!
//! // Decode text from NITF file
//! let text = decode_and_normalize(raw_bytes, "U8S")?;
//!
//! // Encode text for writing to NITF file
//! let bytes = encode_with_crlf(&text, "UTF-8")?;
//! ```

use crate::error::CodecError;

/// Decode text bytes and normalize CR/LF to platform-native line endings.
///
/// This function decodes text bytes according to the specified format code
/// and normalizes CR/LF line delimiters to the platform-native format.
///
/// # Arguments
///
/// * `bytes` - Raw text bytes from the NITF file
/// * `txtfmt` - Text format code (STA, MTF, UT1, U8S)
///
/// # Returns
///
/// The decoded text with normalized line endings, or an error if the bytes
/// contain invalid encoding sequences.
///
/// # Errors
///
/// - `CodecError::Decode` - If the bytes contain invalid encoding sequences
///   for the specified format (e.g., invalid UTF-8 for U8S, non-ASCII for STA)
///
/// # Requirements
///
/// - 4.1: text() returns decoded text content as String
/// - 4.2: STA decoded as ASCII with CR/LF normalization
/// - 4.3: U8S decoded as UTF-8 with CR/LF normalization
/// - 4.4: UT1 decoded using ECS mapping with CR/LF normalization
/// - 4.5: Invalid encoding sequences return CodecError
pub fn decode_and_normalize(bytes: &[u8], txtfmt: &str) -> Result<String, CodecError> {
    let text = match txtfmt {
        "STA" => decode_sta(bytes)?,
        "U8S" => decode_u8s(bytes)?,
        "UT1" => decode_ut1(bytes),
        "MTF" => decode_mtf(bytes)?,
        _ => {
            // Unknown format - try UTF-8, fall back to lossy
            String::from_utf8_lossy(bytes).to_string()
        }
    };

    // Normalize CR/LF to platform-native line endings
    Ok(normalize_line_endings(&text))
}

/// Decode STA (Standard BCS/ASCII) text.
///
/// STA text must contain only valid ASCII characters (0x00-0x7F).
/// Returns an error if any byte is outside the ASCII range.
fn decode_sta(bytes: &[u8]) -> Result<String, CodecError> {
    // Check for non-ASCII bytes
    for (i, &byte) in bytes.iter().enumerate() {
        if byte > 0x7F {
            return Err(CodecError::Decode(format!(
                "Invalid ASCII byte 0x{:02X} at position {}",
                byte, i
            )));
        }
    }

    // All bytes are valid ASCII, which is a subset of UTF-8
    // Safe to use from_utf8 since we've verified all bytes are <= 0x7F
    String::from_utf8(bytes.to_vec())
        .map_err(|e| CodecError::Decode(format!("Invalid ASCII: {}", e)))
}

/// Decode U8S (UTF-8) text.
///
/// U8S text must be valid UTF-8.
/// Returns an error if the bytes are not valid UTF-8.
fn decode_u8s(bytes: &[u8]) -> Result<String, CodecError> {
    String::from_utf8(bytes.to_vec())
        .map_err(|e| CodecError::Decode(format!("Invalid UTF-8: {}", e)))
}

/// Decode UT1 (ECS/ISO-8859-1) text.
///
/// UT1 uses ISO-8859-1 encoding where each byte maps directly to a Unicode
/// code point. This is a lossless conversion since ISO-8859-1 code points
/// 0x00-0xFF map directly to Unicode U+0000-U+00FF.
fn decode_ut1(bytes: &[u8]) -> String {
    // ISO-8859-1: each byte maps directly to Unicode code point
    bytes.iter().map(|&b| b as char).collect()
}

/// Decode MTF (Message Text Format) text.
///
/// MTF is ASCII-based per STANAG 5500/MIL-STD-6040.
/// Returns an error if any byte is outside the ASCII range.
fn decode_mtf(bytes: &[u8]) -> Result<String, CodecError> {
    // MTF is ASCII-based
    for (i, &byte) in bytes.iter().enumerate() {
        if byte > 0x7F {
            return Err(CodecError::Decode(format!(
                "Invalid MTF byte 0x{:02X} at position {}",
                byte, i
            )));
        }
    }

    String::from_utf8(bytes.to_vec()).map_err(|e| CodecError::Decode(format!("Invalid MTF: {}", e)))
}

/// Normalize CR/LF to platform-native line endings.
///
/// On Windows, CR/LF is kept as-is. On Unix-like systems, CR/LF is converted
/// to LF only.
///
/// # Arguments
///
/// * `text` - Text with potentially mixed line endings
///
/// # Returns
///
/// Text with platform-native line endings.
pub fn normalize_line_endings(text: &str) -> String {
    #[cfg(windows)]
    {
        // On Windows, keep CR/LF as-is
        text.to_string()
    }
    #[cfg(not(windows))]
    {
        // On Unix, convert CR/LF to LF
        text.replace("\r\n", "\n")
    }
}

/// Encode text with CR/LF line delimiters for NITF output.
///
/// This function encodes text according to the specified encoding and
/// converts all line endings to CR/LF as required by JBP.
///
/// # Arguments
///
/// * `text` - Text content to encode
/// * `encoding` - Encoding name (ASCII, UTF-8, ECS, MTF)
///
/// # Returns
///
/// Encoded bytes with CR/LF line delimiters, or an error if the text
/// cannot be encoded in the specified encoding.
///
/// # Errors
///
/// - `CodecError::Encode` - If the text contains characters that cannot
///   be represented in the specified encoding
///
/// # Requirements
///
/// - 7.4: ASCII encoding returns bytes with CR/LF line delimiters
/// - 7.5: UTF-8 encoding returns UTF-8 bytes with CR/LF line delimiters
/// - 7.6: Platform-native line endings converted to CR/LF
pub fn encode_with_crlf(text: &str, encoding: &str) -> Result<Vec<u8>, CodecError> {
    // First normalize all line endings to CR/LF
    let normalized = normalize_to_crlf(text);

    match encoding {
        "ASCII" => encode_ascii(&normalized),
        "UTF-8" => Ok(normalized.into_bytes()),
        "ECS" => encode_ecs(&normalized),
        "MTF" => encode_mtf(&normalized),
        _ => {
            // Unknown encoding - default to UTF-8
            Ok(normalized.into_bytes())
        }
    }
}

/// Normalize all line endings to CR/LF.
///
/// Handles:
/// - CR/LF (already correct)
/// - LF only (Unix)
/// - CR only (old Mac)
fn normalize_to_crlf(text: &str) -> String {
    // First normalize any existing CR/LF to just LF
    let temp = text.replace("\r\n", "\n");
    // Then normalize any standalone CR to LF
    let temp = temp.replace('\r', "\n");
    // Finally convert all LF to CR/LF
    temp.replace('\n', "\r\n")
}

/// Encode text as ASCII.
///
/// Returns an error if the text contains non-ASCII characters.
fn encode_ascii(text: &str) -> Result<Vec<u8>, CodecError> {
    if !text.is_ascii() {
        // Find the first non-ASCII character for a helpful error message
        for (i, c) in text.chars().enumerate() {
            if !c.is_ascii() {
                return Err(CodecError::Encode(format!(
                    "Text contains non-ASCII character '{}' (U+{:04X}) at position {}",
                    c, c as u32, i
                )));
            }
        }
    }
    Ok(text.as_bytes().to_vec())
}

/// Encode text as ECS (ISO-8859-1).
///
/// Returns an error if the text contains characters outside the ISO-8859-1 range.
fn encode_ecs(text: &str) -> Result<Vec<u8>, CodecError> {
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            let code_point = c as u32;
            if code_point <= 255 {
                Ok(code_point as u8)
            } else {
                Err(CodecError::Encode(format!(
                    "Character '{}' (U+{:04X}) at position {} cannot be encoded in ISO-8859-1",
                    c, code_point, i
                )))
            }
        })
        .collect()
}

/// Encode text as MTF (ASCII-based).
///
/// Returns an error if the text contains non-ASCII characters.
fn encode_mtf(text: &str) -> Result<Vec<u8>, CodecError> {
    if !text.is_ascii() {
        for (i, c) in text.chars().enumerate() {
            if !c.is_ascii() {
                return Err(CodecError::Encode(format!(
                    "MTF text contains non-ASCII character '{}' (U+{:04X}) at position {}",
                    c, c as u32, i
                )));
            }
        }
    }
    Ok(text.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // decode_and_normalize tests
    // ========================================================================

    #[test]
    fn decode_sta_valid_ascii() {
        let bytes = b"Hello, World!";
        let result = decode_and_normalize(bytes, "STA").unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn decode_sta_with_crlf() {
        let bytes = b"Line 1\r\nLine 2\r\nLine 3";
        let result = decode_and_normalize(bytes, "STA").unwrap();

        #[cfg(windows)]
        assert_eq!(result, "Line 1\r\nLine 2\r\nLine 3");

        #[cfg(not(windows))]
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn decode_sta_invalid_byte() {
        let bytes = &[0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x80]; // "Hello" + 0x80
        let result = decode_and_normalize(bytes, "STA");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid ASCII byte 0x80"));
    }

    #[test]
    fn decode_u8s_valid_utf8() {
        let text = "Hello, 世界! 🌍";
        let bytes = text.as_bytes();
        let result = decode_and_normalize(bytes, "U8S").unwrap();
        assert_eq!(result, text);
    }

    #[test]
    fn decode_u8s_with_crlf() {
        let bytes = "Line 1\r\nLine 2".as_bytes();
        let result = decode_and_normalize(bytes, "U8S").unwrap();

        #[cfg(windows)]
        assert_eq!(result, "Line 1\r\nLine 2");

        #[cfg(not(windows))]
        assert_eq!(result, "Line 1\nLine 2");
    }

    #[test]
    fn decode_u8s_invalid_utf8() {
        let bytes = &[0x48, 0x65, 0x6C, 0x6C, 0x6F, 0xFF, 0xFE]; // Invalid UTF-8
        let result = decode_and_normalize(bytes, "U8S");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid UTF-8"));
    }

    #[test]
    fn decode_ut1_iso8859_1() {
        // ISO-8859-1 bytes: "Héllo" with é = 0xE9
        let bytes = &[0x48, 0xE9, 0x6C, 0x6C, 0x6F];
        let result = decode_and_normalize(bytes, "UT1").unwrap();
        assert_eq!(result, "Héllo");
    }

    #[test]
    fn decode_ut1_full_range() {
        // Test that all ISO-8859-1 bytes decode correctly
        // Note: We test decode_ut1 directly to avoid line ending normalization
        let bytes: Vec<u8> = (0u8..=255).collect();
        let result = decode_ut1(&bytes);
        assert_eq!(result.chars().count(), 256);

        // Verify specific characters
        assert_eq!(result.chars().nth(0xE9).unwrap(), 'é');
        assert_eq!(result.chars().nth(0xF1).unwrap(), 'ñ');
        assert_eq!(result.chars().nth(0x0D).unwrap(), '\r');
        assert_eq!(result.chars().nth(0x0A).unwrap(), '\n');
    }

    #[test]
    fn decode_mtf_valid_ascii() {
        let bytes = b"MTF MESSAGE TEXT";
        let result = decode_and_normalize(bytes, "MTF").unwrap();
        assert_eq!(result, "MTF MESSAGE TEXT");
    }

    #[test]
    fn decode_mtf_invalid_byte() {
        let bytes = &[0x4D, 0x54, 0x46, 0x80]; // "MTF" + 0x80
        let result = decode_and_normalize(bytes, "MTF");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid MTF byte 0x80"));
    }

    #[test]
    fn decode_unknown_format_uses_utf8_lossy() {
        let bytes = b"Hello, World!";
        let result = decode_and_normalize(bytes, "XYZ").unwrap();
        assert_eq!(result, "Hello, World!");
    }

    // ========================================================================
    // normalize_line_endings tests
    // ========================================================================

    #[test]
    fn normalize_crlf_to_platform() {
        let text = "Line 1\r\nLine 2\r\nLine 3";
        let result = normalize_line_endings(text);

        #[cfg(windows)]
        assert_eq!(result, "Line 1\r\nLine 2\r\nLine 3");

        #[cfg(not(windows))]
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn normalize_no_line_endings() {
        let text = "No line endings here";
        let result = normalize_line_endings(text);
        assert_eq!(result, "No line endings here");
    }

    // ========================================================================
    // encode_with_crlf tests
    // ========================================================================

    #[test]
    fn encode_ascii_valid() {
        let text = "Hello, World!";
        let result = encode_with_crlf(text, "ASCII").unwrap();
        assert_eq!(result, b"Hello, World!");
    }

    #[test]
    fn encode_ascii_with_lf() {
        let text = "Line 1\nLine 2";
        let result = encode_with_crlf(text, "ASCII").unwrap();
        assert_eq!(result, b"Line 1\r\nLine 2");
    }

    #[test]
    fn encode_ascii_non_ascii_error() {
        let text = "Hello, 世界!";
        let result = encode_with_crlf(text, "ASCII");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("non-ASCII character"));
    }

    #[test]
    fn encode_utf8_valid() {
        let text = "Hello, 世界! 🌍";
        let result = encode_with_crlf(text, "UTF-8").unwrap();
        assert_eq!(result, text.as_bytes());
    }

    #[test]
    fn encode_utf8_with_lf() {
        let text = "Line 1\nLine 2";
        let result = encode_with_crlf(text, "UTF-8").unwrap();
        assert_eq!(result, b"Line 1\r\nLine 2");
    }

    #[test]
    fn encode_ecs_valid() {
        let text = "Héllo"; // é is U+00E9, within ISO-8859-1
        let result = encode_with_crlf(text, "ECS").unwrap();
        assert_eq!(result, &[0x48, 0xE9, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn encode_ecs_out_of_range() {
        let text = "Hello, 世界!"; // 世 is U+4E16, outside ISO-8859-1
        let result = encode_with_crlf(text, "ECS");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot be encoded in ISO-8859-1"));
    }

    #[test]
    fn encode_mtf_valid() {
        let text = "MTF MESSAGE";
        let result = encode_with_crlf(text, "MTF").unwrap();
        assert_eq!(result, b"MTF MESSAGE");
    }

    #[test]
    fn encode_mtf_non_ascii_error() {
        let text = "MTF 世界";
        let result = encode_with_crlf(text, "MTF");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("non-ASCII character"));
    }

    #[test]
    fn encode_unknown_uses_utf8() {
        let text = "Hello, 世界!";
        let result = encode_with_crlf(text, "XYZ").unwrap();
        assert_eq!(result, text.as_bytes());
    }

    // ========================================================================
    // normalize_to_crlf tests
    // ========================================================================

    #[test]
    fn normalize_to_crlf_lf_only() {
        let text = "Line 1\nLine 2\nLine 3";
        let result = normalize_to_crlf(text);
        assert_eq!(result, "Line 1\r\nLine 2\r\nLine 3");
    }

    #[test]
    fn normalize_to_crlf_cr_only() {
        let text = "Line 1\rLine 2\rLine 3";
        let result = normalize_to_crlf(text);
        assert_eq!(result, "Line 1\r\nLine 2\r\nLine 3");
    }

    #[test]
    fn normalize_to_crlf_already_crlf() {
        let text = "Line 1\r\nLine 2\r\nLine 3";
        let result = normalize_to_crlf(text);
        assert_eq!(result, "Line 1\r\nLine 2\r\nLine 3");
    }

    #[test]
    fn normalize_to_crlf_mixed() {
        let text = "Line 1\nLine 2\r\nLine 3\rLine 4";
        let result = normalize_to_crlf(text);
        assert_eq!(result, "Line 1\r\nLine 2\r\nLine 3\r\nLine 4");
    }

    #[test]
    fn normalize_to_crlf_no_line_endings() {
        let text = "No line endings";
        let result = normalize_to_crlf(text);
        assert_eq!(result, "No line endings");
    }

    // ========================================================================
    // Round-trip tests
    // ========================================================================

    #[test]
    fn roundtrip_ascii() {
        let original = "Hello\nWorld";
        let encoded = encode_with_crlf(original, "ASCII").unwrap();
        let decoded = decode_and_normalize(&encoded, "STA").unwrap();

        // On Unix, the decoded text should have LF only
        #[cfg(not(windows))]
        assert_eq!(decoded, original);

        // On Windows, the decoded text should have CR/LF
        #[cfg(windows)]
        assert_eq!(decoded, "Hello\r\nWorld");
    }

    #[test]
    fn roundtrip_utf8() {
        let original = "Hello, 世界!\nLine 2";
        let encoded = encode_with_crlf(original, "UTF-8").unwrap();
        let decoded = decode_and_normalize(&encoded, "U8S").unwrap();

        #[cfg(not(windows))]
        assert_eq!(decoded, original);

        #[cfg(windows)]
        assert_eq!(decoded, "Hello, 世界!\r\nLine 2");
    }

    #[test]
    fn roundtrip_ecs() {
        let original = "Héllo\nWörld";
        let encoded = encode_with_crlf(original, "ECS").unwrap();
        let decoded = decode_and_normalize(&encoded, "UT1").unwrap();

        #[cfg(not(windows))]
        assert_eq!(decoded, original);

        #[cfg(windows)]
        assert_eq!(decoded, "Héllo\r\nWörld");
    }
}
