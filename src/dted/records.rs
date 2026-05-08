//! UHL, DSI, ACC, and data record parsing for DTED files.

use crate::error::CodecError;

// Fixed record sizes per MIL-PRF-89020B
pub const UHL_SIZE: usize = 80;
pub const DSI_SIZE: usize = 648;
pub const ACC_SIZE: usize = 2700;
pub const DATA_OFFSET: usize = UHL_SIZE + DSI_SIZE + ACC_SIZE; // 3428

/// Parsed User Header Label (80 bytes).
#[derive(Debug, Clone)]
pub struct Uhl {
    pub origin_lon: f64,
    pub origin_lat: f64,
    pub lon_interval_tenths: u16,
    pub lat_interval_tenths: u16,
    pub num_lon_lines: u16,
    pub num_lat_points: u16,
    pub vertical_accuracy: Option<u16>,
    pub security_code: char,
    pub multiple_accuracy: bool,
}

/// Parsed Data Set Identification record (648 bytes).
#[derive(Debug, Clone)]
pub struct Dsi {
    pub security_code: String,
    pub product_level: String,
    pub edition_number: String,
    pub compilation_date: String,
    pub producer_code: String,
    pub vertical_datum: String,
    pub horizontal_datum: String,
    pub partial_cell_indicator: String,
}

/// Parsed Accuracy Description record (2700 bytes).
#[derive(Debug, Clone)]
pub struct Acc {
    pub absolute_horizontal_accuracy: String,
    pub absolute_vertical_accuracy: String,
    pub relative_vertical_accuracy: String,
}

impl Uhl {
    pub fn parse(data: &[u8]) -> Result<Self, CodecError> {
        if data.len() < UHL_SIZE {
            return Err(CodecError::InvalidFormat(
                "DTED file too short for UHL record".to_string(),
            ));
        }

        if &data[0..3] != b"UHL" {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid UHL sentinel: expected 'UHL', got '{}'",
                String::from_utf8_lossy(&data[0..3])
            )));
        }

        let origin_lon = parse_longitude(&data[4..12])?;
        let origin_lat = parse_latitude(&data[12..20])?;
        let lon_interval_tenths = parse_ascii_u16(&data[20..24])?;
        let lat_interval_tenths = parse_ascii_u16(&data[24..28])?;
        let vertical_accuracy = parse_optional_u16(&data[28..32]);
        let security_code = data[32] as char;
        // Unique reference number: bytes 33..45 (ignored)
        let num_lon_lines = parse_ascii_u16(&data[47..51])?;
        let num_lat_points = parse_ascii_u16(&data[51..55])?;
        let multiple_accuracy = data[55] == b'1';

        Ok(Uhl {
            origin_lon,
            origin_lat,
            lon_interval_tenths,
            lat_interval_tenths,
            num_lon_lines,
            num_lat_points,
            vertical_accuracy,
            security_code,
            multiple_accuracy,
        })
    }
}

impl Dsi {
    pub fn parse(data: &[u8]) -> Result<Self, CodecError> {
        let offset = UHL_SIZE;
        if data.len() < offset + DSI_SIZE {
            return Err(CodecError::InvalidFormat(
                "DTED file too short for DSI record".to_string(),
            ));
        }

        let dsi = &data[offset..offset + DSI_SIZE];
        if &dsi[0..3] != b"DSI" {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid DSI sentinel: expected 'DSI', got '{}'",
                String::from_utf8_lossy(&dsi[0..3])
            )));
        }

        let security_code = ascii_field(dsi, 3, 1);
        let product_level = ascii_field(dsi, 59, 5);
        let edition_number = ascii_field(dsi, 87, 2);
        let compilation_date = ascii_field(dsi, 93, 4);
        let producer_code = ascii_field(dsi, 65, 8);
        let vertical_datum = ascii_field(dsi, 141, 3);
        let horizontal_datum = ascii_field(dsi, 144, 5);
        let partial_cell_indicator = ascii_field(dsi, 289, 2);

        Ok(Dsi {
            security_code,
            product_level,
            edition_number,
            compilation_date,
            producer_code,
            vertical_datum,
            horizontal_datum,
            partial_cell_indicator,
        })
    }
}

impl Acc {
    pub fn parse(data: &[u8]) -> Result<Self, CodecError> {
        let offset = UHL_SIZE + DSI_SIZE;
        if data.len() < offset + ACC_SIZE {
            return Err(CodecError::InvalidFormat(
                "DTED file too short for ACC record".to_string(),
            ));
        }

        let acc = &data[offset..offset + ACC_SIZE];
        if &acc[0..3] != b"ACC" {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid ACC sentinel: expected 'ACC', got '{}'",
                String::from_utf8_lossy(&acc[0..3])
            )));
        }

        let absolute_horizontal_accuracy = ascii_field(acc, 3, 4);
        let absolute_vertical_accuracy = ascii_field(acc, 7, 4);
        let relative_vertical_accuracy = ascii_field(acc, 11, 4);

        Ok(Acc {
            absolute_horizontal_accuracy,
            absolute_vertical_accuracy,
            relative_vertical_accuracy,
        })
    }
}

/// Convert DTED signed-magnitude big-endian i16 to native i16.
#[inline]
pub fn decode_elevation(bytes: [u8; 2]) -> i16 {
    let raw = u16::from_be_bytes(bytes);
    if raw == 0xFFFF {
        return -32767;
    }
    if raw & 0x8000 != 0 {
        -((raw & 0x7FFF) as i16)
    } else {
        raw as i16
    }
}

/// Convert native i16 to DTED signed-magnitude big-endian.
#[inline]
pub fn encode_elevation(value: i16) -> [u8; 2] {
    if value == -32767 {
        return [0xFF, 0xFF];
    }
    if value < 0 {
        let magnitude = (-value) as u16;
        (magnitude | 0x8000).to_be_bytes()
    } else {
        (value as u16).to_be_bytes()
    }
}

/// Compute the checksum for a data record (sum of all bytes as u32).
pub fn compute_record_checksum(record: &[u8]) -> u32 {
    record.iter().map(|&b| b as u32).sum()
}

/// Compute the expected record size for a column of elevation data.
#[inline]
pub fn record_size(num_lat_points: u16) -> usize {
    // sentinel(1) + block_count(3) + lon_count(2) + lat_count(2)
    // + elevations(num_lat_points * 2) + checksum(4)
    8 + (num_lat_points as usize * 2) + 4
}

/// Validate the checksum of a single data record.
pub fn validate_record_checksum(record: &[u8]) -> bool {
    if record.len() < 12 {
        return false;
    }
    let payload_len = record.len() - 4;
    let expected = u32::from_be_bytes([
        record[payload_len],
        record[payload_len + 1],
        record[payload_len + 2],
        record[payload_len + 3],
    ]);
    let computed: u32 = record[..payload_len].iter().map(|&b| b as u32).sum();
    computed == expected
}

// =========================================================================
// Internal parsing helpers
// =========================================================================

fn ascii_field(data: &[u8], offset: usize, len: usize) -> String {
    String::from_utf8_lossy(&data[offset..offset + len])
        .trim()
        .to_string()
}

fn parse_ascii_u16(data: &[u8]) -> Result<u16, CodecError> {
    let s = String::from_utf8_lossy(data).trim().to_string();
    s.parse::<u16>().map_err(|_| {
        CodecError::Parse(format!(
            "Failed to parse '{}' as u16",
            String::from_utf8_lossy(data)
        ))
    })
}

fn parse_optional_u16(data: &[u8]) -> Option<u16> {
    let s = String::from_utf8_lossy(data).trim().to_string();
    if s == "NA" || s.is_empty() {
        None
    } else {
        s.parse::<u16>().ok()
    }
}

/// Parse a DTED longitude field: DDDMMSSH (8 chars).
fn parse_longitude(data: &[u8]) -> Result<f64, CodecError> {
    if data.len() < 8 {
        return Err(CodecError::Parse("Longitude field too short".to_string()));
    }
    let s = String::from_utf8_lossy(data);
    let s = s.trim();
    if s.len() < 8 {
        return Err(CodecError::Parse(format!(
            "Longitude field too short: '{}'",
            s
        )));
    }

    let degrees: f64 = s[..3]
        .parse()
        .map_err(|_| CodecError::Parse(format!("Invalid longitude degrees: '{}'", &s[..3])))?;
    let minutes: f64 = s[3..5]
        .parse()
        .map_err(|_| CodecError::Parse(format!("Invalid longitude minutes: '{}'", &s[3..5])))?;
    let seconds: f64 = s[5..7]
        .parse()
        .map_err(|_| CodecError::Parse(format!("Invalid longitude seconds: '{}'", &s[5..7])))?;

    let hemisphere = s.as_bytes()[7];
    let sign = match hemisphere {
        b'E' => 1.0,
        b'W' => -1.0,
        _ => {
            return Err(CodecError::Parse(format!(
                "Invalid longitude hemisphere: '{}'",
                hemisphere as char
            )));
        }
    };

    Ok(sign * (degrees + minutes / 60.0 + seconds / 3600.0))
}

/// Parse a DTED latitude field: DDMMSSH (7 chars) within an 8-byte field.
///
/// The hemisphere character (N/S) is the last non-space, non-null character.
/// The field may be either 7 or 8 bytes, with trailing padding.
fn parse_latitude(data: &[u8]) -> Result<f64, CodecError> {
    if data.len() < 7 {
        return Err(CodecError::Parse("Latitude field too short".to_string()));
    }

    // Find the hemisphere character: scan from the end for N or S
    let hemisphere = data
        .iter()
        .rev()
        .find(|&&b| b == b'N' || b == b'S')
        .copied()
        .ok_or_else(|| {
            CodecError::Parse(format!(
                "No hemisphere (N/S) found in latitude field: '{}'",
                String::from_utf8_lossy(data)
            ))
        })?;

    // The numeric portion is everything before the hemisphere character
    let hemi_pos = data.iter().position(|&b| b == b'N' || b == b'S').unwrap();
    if hemi_pos < 6 {
        return Err(CodecError::Parse(format!(
            "Latitude field too short before hemisphere: '{}'",
            String::from_utf8_lossy(data)
        )));
    }

    let s = String::from_utf8_lossy(&data[..hemi_pos]);
    // Strip leading zeros/spaces for flexible parsing but require at least DDMMSS
    let s = s.trim();
    let numeric_len = s.len();

    // Parse: last 2 chars = seconds, preceding 2 = minutes, rest = degrees
    if numeric_len < 6 {
        return Err(CodecError::Parse(format!(
            "Latitude numeric portion too short: '{}'",
            s
        )));
    }
    let deg_end = numeric_len - 4;
    let degrees: f64 = s[..deg_end]
        .parse()
        .map_err(|_| CodecError::Parse(format!("Invalid latitude degrees: '{}'", &s[..deg_end])))?;
    let minutes: f64 = s[deg_end..deg_end + 2].parse().map_err(|_| {
        CodecError::Parse(format!(
            "Invalid latitude minutes: '{}'",
            &s[deg_end..deg_end + 2]
        ))
    })?;
    let seconds: f64 = s[deg_end + 2..deg_end + 4].parse().map_err(|_| {
        CodecError::Parse(format!(
            "Invalid latitude seconds: '{}'",
            &s[deg_end + 2..deg_end + 4]
        ))
    })?;

    let sign = match hemisphere {
        b'N' => 1.0,
        b'S' => -1.0,
        _ => unreachable!(),
    };

    Ok(sign * (degrees + minutes / 60.0 + seconds / 3600.0))
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_elevation_positive() {
        assert_eq!(decode_elevation([0x00, 0x64]), 100);
        assert_eq!(decode_elevation([0x00, 0x00]), 0);
        assert_eq!(decode_elevation([0x23, 0x28]), 9000);
    }

    #[test]
    fn test_decode_elevation_negative() {
        // -100 in signed-magnitude: 0x8064
        assert_eq!(decode_elevation([0x80, 0x64]), -100);
        // -1 in signed-magnitude: 0x8001
        assert_eq!(decode_elevation([0x80, 0x01]), -1);
        // -12000 in signed-magnitude: 0xAEE0
        assert_eq!(decode_elevation([0xAE, 0xE0]), -12000);
    }

    #[test]
    fn test_decode_elevation_null() {
        assert_eq!(decode_elevation([0xFF, 0xFF]), -32767);
    }

    #[test]
    fn test_decode_elevation_negative_zero() {
        // 0x8000 = signed-magnitude -0 → should decode as 0
        assert_eq!(decode_elevation([0x80, 0x00]), 0);
    }

    #[test]
    fn test_record_size() {
        assert_eq!(record_size(1201), 2414);
        assert_eq!(record_size(3601), 7214);
    }

    #[test]
    fn test_validate_record_checksum() {
        // Construct a minimal record: sentinel(1) + block(3) + lon(2) + lat(2) + 2 elevations + checksum
        let mut record = vec![
            0xAA, // sentinel
            0x00, 0x00, 0x00, // block count
            0x00, 0x01, // lon count
            0x00, 0x01, // lat count
            0x00, 0x64, // elevation 1 = 100
            0x00, 0xC8, // elevation 2 = 200
        ];
        let sum: u32 = record.iter().map(|&b| b as u32).sum();
        record.extend_from_slice(&sum.to_be_bytes());
        assert!(validate_record_checksum(&record));

        // Corrupt one byte
        record[8] = 0xFF;
        assert!(!validate_record_checksum(&record));
    }

    #[test]
    fn test_parse_longitude() {
        assert!((parse_longitude(b"1090000W").unwrap() - (-109.0)).abs() < 1e-10);
        assert!((parse_longitude(b"0000000E").unwrap() - 0.0).abs() < 1e-10);
        assert!((parse_longitude(b"1800000E").unwrap() - 180.0).abs() < 1e-10);
        assert!((parse_longitude(b"0013000E").unwrap() - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_parse_latitude() {
        assert!((parse_latitude(b"380000N ").unwrap() - 38.0).abs() < 1e-10);
        assert!((parse_latitude(b"000000N ").unwrap() - 0.0).abs() < 1e-10);
        assert!((parse_latitude(b"900000S ").unwrap() - (-90.0)).abs() < 1e-10);
        // 7-byte form also works
        assert!((parse_latitude(b"380000N").unwrap() - 38.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_uhl_valid() {
        let mut uhl = vec![0x20u8; UHL_SIZE];
        // UHL sentinel
        uhl[0] = b'U';
        uhl[1] = b'H';
        uhl[2] = b'L';
        uhl[3] = b'1'; // fixed '1'
                       // Origin longitude: 1090000W (DDDMMSSH = 8 chars)
        uhl[4..12].copy_from_slice(b"1090000W");
        // Origin latitude: 380000N + space pad (DDMMSSH in 8-byte field)
        uhl[12..20].copy_from_slice(b"380000N ");
        // Lon interval: 0030
        uhl[20..24].copy_from_slice(b"0030");
        // Lat interval: 0030
        uhl[24..28].copy_from_slice(b"0030");
        // Vertical accuracy: 0020
        uhl[28..32].copy_from_slice(b"0020");
        // Security code
        uhl[32] = b'U';
        // Num longitude lines
        uhl[47..51].copy_from_slice(b"1201");
        // Num latitude points
        uhl[51..55].copy_from_slice(b"1201");
        // Multiple accuracy
        uhl[55] = b'0';

        let result = Uhl::parse(&uhl).unwrap();
        assert!((result.origin_lon - (-109.0)).abs() < 1e-10);
        assert!((result.origin_lat - 38.0).abs() < 1e-10);
        assert_eq!(result.lon_interval_tenths, 30);
        assert_eq!(result.lat_interval_tenths, 30);
        assert_eq!(result.num_lon_lines, 1201);
        assert_eq!(result.num_lat_points, 1201);
        assert_eq!(result.vertical_accuracy, Some(20));
        assert_eq!(result.security_code, 'U');
        assert!(!result.multiple_accuracy);
    }

    #[test]
    fn test_parse_uhl_invalid_sentinel() {
        let data = vec![0x20u8; UHL_SIZE];
        let result = Uhl::parse(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid UHL sentinel"));
    }

    #[test]
    fn test_parse_dsi_valid() {
        let mut data = vec![0x20u8; UHL_SIZE + DSI_SIZE];
        // UHL sentinel (needed for offset)
        data[0..3].copy_from_slice(b"UHL");
        // DSI sentinel
        let dsi_start = UHL_SIZE;
        data[dsi_start..dsi_start + 3].copy_from_slice(b"DSI");
        data[dsi_start + 3] = b'U'; // security code
        data[dsi_start + 59..dsi_start + 64].copy_from_slice(b"DTED1");
        data[dsi_start + 87..dsi_start + 89].copy_from_slice(b"02");
        data[dsi_start + 93..dsi_start + 97].copy_from_slice(b"0502");
        data[dsi_start + 65..dsi_start + 67].copy_from_slice(b"US");
        data[dsi_start + 141..dsi_start + 144].copy_from_slice(b"MSL");
        data[dsi_start + 144..dsi_start + 149].copy_from_slice(b"WGS84");
        data[dsi_start + 289..dsi_start + 291].copy_from_slice(b"00");

        let result = Dsi::parse(&data).unwrap();
        assert_eq!(result.security_code, "U");
        assert_eq!(result.product_level, "DTED1");
        assert_eq!(result.edition_number, "02");
        assert_eq!(result.compilation_date, "0502");
        assert_eq!(result.vertical_datum, "MSL");
        assert_eq!(result.horizontal_datum, "WGS84");
        assert_eq!(result.partial_cell_indicator, "00");
    }

    #[test]
    fn test_parse_dsi_invalid_sentinel() {
        let mut data = vec![0x20u8; UHL_SIZE + DSI_SIZE];
        data[0..3].copy_from_slice(b"UHL");
        // DSI sentinel is missing (all spaces)
        let result = Dsi::parse(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid DSI sentinel"));
    }

    #[test]
    fn test_parse_acc_valid() {
        let mut data = vec![0x20u8; UHL_SIZE + DSI_SIZE + ACC_SIZE];
        data[0..3].copy_from_slice(b"UHL");
        data[UHL_SIZE..UHL_SIZE + 3].copy_from_slice(b"DSI");
        let acc_start = UHL_SIZE + DSI_SIZE;
        data[acc_start..acc_start + 3].copy_from_slice(b"ACC");
        data[acc_start + 3..acc_start + 7].copy_from_slice(b"0050");
        data[acc_start + 7..acc_start + 11].copy_from_slice(b"0030");
        data[acc_start + 11..acc_start + 15].copy_from_slice(b"0020");

        let result = Acc::parse(&data).unwrap();
        assert_eq!(result.absolute_horizontal_accuracy, "0050");
        assert_eq!(result.absolute_vertical_accuracy, "0030");
        assert_eq!(result.relative_vertical_accuracy, "0020");
    }

    #[test]
    fn test_parse_acc_invalid_sentinel() {
        let mut data = vec![0x20u8; UHL_SIZE + DSI_SIZE + ACC_SIZE];
        data[0..3].copy_from_slice(b"UHL");
        data[UHL_SIZE..UHL_SIZE + 3].copy_from_slice(b"DSI");
        // ACC sentinel is missing
        let result = Acc::parse(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid ACC sentinel"));
    }

    #[test]
    fn test_encode_elevation_positive() {
        assert_eq!(encode_elevation(100), [0x00, 0x64]);
        assert_eq!(encode_elevation(0), [0x00, 0x00]);
        assert_eq!(encode_elevation(9000), [0x23, 0x28]);
    }

    #[test]
    fn test_encode_elevation_negative() {
        assert_eq!(encode_elevation(-100), [0x80, 0x64]);
        assert_eq!(encode_elevation(-1), [0x80, 0x01]);
        assert_eq!(encode_elevation(-12000), [0xAE, 0xE0]);
    }

    #[test]
    fn test_encode_elevation_null() {
        assert_eq!(encode_elevation(-32767), [0xFF, 0xFF]);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        for value in [-12000i16, -1000, -1, 0, 1, 100, 9000, -32767] {
            assert_eq!(decode_elevation(encode_elevation(value)), value);
        }
    }

    #[test]
    fn test_compute_record_checksum() {
        let record = vec![0xAA, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x64];
        let checksum = compute_record_checksum(&record);
        let expected: u32 = record.iter().map(|&b| b as u32).sum();
        assert_eq!(checksum, expected);
    }
}
