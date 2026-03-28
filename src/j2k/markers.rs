//! JPEG 2000 marker parsing for tile-part extraction.
//!
//! Provides functions to:
//! - Extract the main header (SOC to first SOT)
//! - Strip TLM markers to produce a decode header
//! - Parse TLM markers into a tile-part offset table
//! - Scan SOT markers into a tile-part offset table
//! - Construct minimal single-tile codestreams

use crate::error::CodecError;

/// Marker codes used in JPEG 2000 codestreams.
pub mod marker_codes {
    /// Start of Codestream
    pub const SOC: u16 = 0xFF4F;
    /// Image and Tile Size
    pub const SIZ: u16 = 0xFF51;
    /// Coding Style Default
    pub const COD: u16 = 0xFF52;
    /// Coding Style Component
    pub const COC: u16 = 0xFF53;
    /// Quantization Default
    pub const QCD: u16 = 0xFF5C;
    /// Tile-part Lengths
    pub const TLM: u16 = 0xFF55;
    /// Start of Tile-part
    pub const SOT: u16 = 0xFF90;
    /// Start of Data
    pub const SOD: u16 = 0xFF93;
    /// End of Codestream
    pub const EOC: u16 = 0xFFD9;
}

/// One entry in the tile-part offset table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePartEntry {
    /// Tile index (Isot from SOT marker).
    pub tile_index: u16,
    /// Byte offset of this tile-part relative to the start of the codestream.
    pub offset: u64,
    /// Byte length of this tile-part (SOT marker through end of compressed data).
    pub length: u64,
}

/// Complete tile-part offset table for a codestream.
pub type TilePartOffsetTable = Vec<TilePartEntry>;

/// Result of parsing the J2K main header.
#[derive(Debug, Clone)]
pub struct MainHeaderInfo {
    /// The full main header bytes (SOC through byte before first SOT).
    pub main_header: Vec<u8>,
    /// The decode header: main header with all TLM marker segments removed.
    pub decode_header: Vec<u8>,
    /// If TLM markers were present, the tile-part offset table parsed from them.
    pub tlm_offset_table: Option<TilePartOffsetTable>,
    /// Byte offset of the first SOT marker in the codestream.
    pub first_sot_offset: u64,
}

/// Read a big-endian u16 from a byte slice at the given offset.
fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

/// Read a big-endian u32 from a byte slice at the given offset.
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// Parse TLM entries from a single TLM marker segment body (after the marker code).
///
/// `tlm_body` starts at Ltlm (the length field), so the layout is:
///   Ltlm (2 bytes) | Ztlm (1 byte) | Stlm (1 byte) | (Ttlm_i, Ptlm_i) pairs...
///
/// `first_sot_offset` is used to compute absolute codestream offsets from the
/// tile-part lengths stored in TLM entries.
///
/// `running_offset` is the current accumulated offset (starts at first_sot_offset
/// for the first TLM, then accumulates across multiple TLM segments).
///
/// Returns the parsed entries and the updated running offset.
fn parse_tlm_entries(
    tlm_body: &[u8],
    marker_offset: usize,
    running_offset: u64,
) -> Result<(Vec<TilePartEntry>, u64), CodecError> {
    // Need at least Ltlm(2) + Ztlm(1) + Stlm(1) = 4 bytes
    if tlm_body.len() < 4 {
        return Err(CodecError::InvalidFormat(format!(
            "TLM marker segment truncated at offset {}",
            marker_offset
        )));
    }

    let ltlm = read_u16(tlm_body, 0) as usize;
    // Ztlm at offset 2 (index, not used for parsing)
    let stlm = tlm_body[3];

    // ST: bits 5-4 of Stlm — tile index size
    let st = (stlm >> 4) & 0x03;
    // SP: bit 6 of Stlm — tile-part length size
    let sp = (stlm >> 6) & 0x01;

    let tile_index_size: usize = match st {
        0 => 0, // no tile index (sequential)
        1 => 1, // 8-bit tile index
        2 => 2, // 16-bit tile index
        _ => {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid TLM ST value {} at offset {}",
                st, marker_offset
            )));
        }
    };

    let length_size: usize = if sp == 0 { 2 } else { 4 };
    let entry_size = tile_index_size + length_size;

    // Data after Ltlm(2) + Ztlm(1) + Stlm(1) = 4 bytes of header
    let data_len = ltlm.saturating_sub(2); // Ltlm includes itself but not marker code
    // The payload starts after Ztlm + Stlm = 2 bytes into the segment body (after Ltlm)
    let payload_start = 4; // Ltlm(2) + Ztlm(1) + Stlm(1)
    let payload_len = data_len.saturating_sub(2); // subtract Ztlm + Stlm

    if payload_start + payload_len > tlm_body.len() {
        return Err(CodecError::InvalidFormat(format!(
            "TLM marker segment truncated at offset {}",
            marker_offset
        )));
    }

    let mut entries = Vec::new();
    let mut offset = running_offset;
    let mut pos = payload_start;
    let payload_end = payload_start + payload_len;
    let mut sequential_index: u16 = 0;

    while pos + entry_size <= payload_end {
        let tile_index = match tile_index_size {
            0 => {
                let idx = sequential_index;
                sequential_index += 1;
                idx
            }
            1 => {
                let idx = tlm_body[pos] as u16;
                pos += 1;
                idx
            }
            2 => {
                let idx = read_u16(tlm_body, pos);
                pos += 2;
                idx
            }
            _ => unreachable!(),
        };

        let tile_part_length: u64 = if length_size == 2 {
            let len = read_u16(tlm_body, pos) as u64;
            pos += 2;
            len
        } else {
            let len = read_u32(tlm_body, pos) as u64;
            pos += 4;
            len
        };

        entries.push(TilePartEntry {
            tile_index,
            offset,
            length: tile_part_length,
        });

        offset += tile_part_length;
    }

    Ok((entries, offset))
}

/// Parse the main header from a J2K codestream.
///
/// Scans from SOC (offset 0) to the first SOT marker. Validates SOC and SIZ
/// presence. Extracts TLM data if present. Produces both the full main header
/// and the TLM-stripped decode header.
///
/// # Errors
/// - Missing SOC marker at offset 0
/// - Missing SIZ marker as first marker segment after SOC
/// - Codestream truncated before first SOT
/// - Malformed marker segments
pub fn parse_main_header(codestream: &[u8]) -> Result<MainHeaderInfo, CodecError> {
    // Validate SOC at offset 0
    if codestream.len() < 2 || read_u16(codestream, 0) != marker_codes::SOC {
        return Err(CodecError::InvalidFormat(
            "Invalid J2K codestream: missing SOC marker at offset 0".to_string(),
        ));
    }

    let mut pos: usize = 2; // past SOC

    // Validate SIZ as first marker segment after SOC
    if codestream.len() < pos + 2 || read_u16(codestream, pos) != marker_codes::SIZ {
        return Err(CodecError::InvalidFormat(
            "Invalid J2K codestream: SIZ marker must follow SOC".to_string(),
        ));
    }

    // Track TLM marker segment positions (start of marker code, total length including marker code)
    // so we can strip them when building decode_header
    let mut tlm_ranges: Vec<(usize, usize)> = Vec::new();
    // Accumulate all TLM entries across multiple TLM segments
    let mut all_tlm_entries: Vec<TilePartEntry> = Vec::new();
    // Running offset for TLM entries: accumulates tile-part lengths across TLM segments
    let mut tlm_running_offset: u64 = 0;
    // We'll set first_sot_offset when we find SOT, then use it to finalize TLM offsets
    let mut first_sot_offset: Option<u64> = None;

    // Scan markers from after SOC until we find SOT
    while pos + 2 <= codestream.len() {
        let marker = read_u16(codestream, pos);

        if marker == marker_codes::SOT {
            first_sot_offset = Some(pos as u64);
            break;
        }

        // All markers in the main header (except SOC which we already passed) have length fields
        if pos + 4 > codestream.len() {
            return Err(CodecError::InvalidFormat(format!(
                "J2K marker segment at offset {} extends beyond codestream",
                pos
            )));
        }

        let marker_length = read_u16(codestream, pos + 2) as usize;
        // marker_length includes the 2 bytes of the length field itself but not the marker code
        let segment_total = 2 + marker_length; // marker code (2) + marker_length (includes Lxxx)

        if pos + segment_total > codestream.len() {
            return Err(CodecError::InvalidFormat(format!(
                "J2K marker segment at offset {} extends beyond codestream",
                pos
            )));
        }

        if marker == marker_codes::TLM {
            // Record position for stripping later
            tlm_ranges.push((pos, segment_total));
            // Parse TLM entries with running_offset that accumulates across TLM segments.
            // We start from 0 and add first_sot_offset after the scan completes.
            let tlm_body = &codestream[pos + 2..pos + segment_total];
            let (entries, new_running_offset) = parse_tlm_entries(tlm_body, pos, tlm_running_offset)?;
            tlm_running_offset = new_running_offset;
            all_tlm_entries.extend(entries);
        }

        pos += segment_total;
    }

    let first_sot_offset = first_sot_offset.ok_or_else(|| {
        CodecError::InvalidFormat(
            "J2K codestream truncated: no SOT marker found".to_string(),
        )
    })?;

    // Fix up TLM entry offsets: the entries were parsed with running_offset=0,
    // meaning each entry's offset is the cumulative sum of preceding lengths
    // starting from 0. We need to shift them all by first_sot_offset.
    for entry in &mut all_tlm_entries {
        entry.offset += first_sot_offset;
    }

    // Extract main header bytes
    let main_header = codestream[..first_sot_offset as usize].to_vec();

    // Build decode_header by copying main_header and skipping TLM segments
    let decode_header = if tlm_ranges.is_empty() {
        main_header.clone()
    } else {
        let mut dh = Vec::with_capacity(main_header.len());
        let mut copy_from: usize = 0;
        for &(tlm_start, tlm_len) in &tlm_ranges {
            // Copy bytes before this TLM segment
            dh.extend_from_slice(&main_header[copy_from..tlm_start]);
            copy_from = tlm_start + tlm_len;
        }
        // Copy remaining bytes after last TLM
        dh.extend_from_slice(&main_header[copy_from..]);
        dh
    };

    let tlm_offset_table = if all_tlm_entries.is_empty() {
        None
    } else {
        Some(all_tlm_entries)
    };

    Ok(MainHeaderInfo {
        main_header,
        decode_header,
        tlm_offset_table,
        first_sot_offset,
    })
}

/// Build a tile-part offset table by scanning SOT markers in the codestream.
///
/// Starts scanning from `first_sot_offset` and reads each SOT marker to
/// extract tile index (Isot), tile-part length (Psot), and position.
///
/// # Arguments
/// * `codestream` - The full J2K codestream bytes
/// * `first_sot_offset` - Byte offset where the first SOT marker begins
///
/// # Errors
/// - Malformed SOT marker (truncated, invalid length)
/// - Psot value that exceeds codestream length
pub fn scan_sot_markers(
    codestream: &[u8],
    first_sot_offset: u64,
) -> Result<TilePartOffsetTable, CodecError> {
    let cs_len = codestream.len() as u64;
    let mut pos = first_sot_offset;
    let mut entries = Vec::new();

    while pos < cs_len {
        // Check for EOC marker (2 bytes)
        if pos + 2 <= cs_len && read_u16(codestream, pos as usize) == marker_codes::EOC {
            break;
        }

        // Need at least 2 (marker) + 10 (Lsot + fields) = 12 bytes for a complete SOT
        if pos + 12 > cs_len {
            return Err(CodecError::InvalidFormat(format!(
                "SOT marker truncated at offset {}",
                pos
            )));
        }

        let marker = read_u16(codestream, pos as usize);
        if marker != marker_codes::SOT {
            // Not an SOT marker — stop scanning
            break;
        }

        let lsot = read_u16(codestream, (pos + 2) as usize);
        if lsot != 10 {
            return Err(CodecError::InvalidFormat(format!(
                "Invalid SOT marker length {} at offset {} (expected 10)",
                lsot, pos
            )));
        }

        let isot = read_u16(codestream, (pos + 4) as usize);
        let psot = read_u32(codestream, (pos + 6) as usize) as u64;
        // TPsot at pos+10, TNsot at pos+11 — read but not stored in TilePartEntry
        // let _tpsot = codestream[(pos + 10) as usize];
        // let _tnsot = codestream[(pos + 11) as usize];

        if psot == 0 {
            // Tile-part extends to end of codestream
            let length = cs_len - pos;
            entries.push(TilePartEntry {
                tile_index: isot,
                offset: pos,
                length,
            });
            // Psot=0 means this is the last tile-part — stop scanning
            break;
        }

        // Validate Psot doesn't exceed codestream bounds
        if pos + psot > cs_len {
            return Err(CodecError::InvalidFormat(format!(
                "SOT Psot={} at offset {} exceeds codestream length",
                psot, pos
            )));
        }

        entries.push(TilePartEntry {
            tile_index: isot,
            offset: pos,
            length: psot,
        });

        // Advance by Psot bytes to the next tile-part
        pos += psot;
    }

    Ok(entries)
}

/// Construct a minimal single-tile codestream for decoding.
///
/// Concatenates: `decode_header + tile_part_bytes + EOC_MARKER`
///
/// For tiles with multiple tile-parts, all tile-parts are concatenated
/// in order between the decode header and EOC. Each tile-part's SOT marker
/// is patched so that Isot=0, because the minimal codestream contains only
/// one tile and OpenJPEG expects tile_index=0 to match the SOT.
///
/// # Arguments
/// * `decode_header` - Main header with TLM markers stripped
/// * `tile_parts` - Slice of (offset, length) pairs into the original codestream
/// * `codestream` - The full original codestream (for extracting tile-part bytes)
pub fn build_minimal_codestream(
    decode_header: &[u8],
    tile_parts: &[(u64, u64)],
    codestream: &[u8],
) -> Vec<u8> {
    let total_tile_bytes: u64 = tile_parts.iter().map(|(_, len)| len).sum();
    let capacity = decode_header.len() + total_tile_bytes as usize + 2; // +2 for EOC
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(decode_header);
    for &(offset, length) in tile_parts {
        let start = offset as usize;
        let end = start + length as usize;
        let tile_part = &codestream[start..end];
        // Patch SOT marker: set Isot (bytes 4-5) to 0 so OpenJPEG sees tile_index=0.
        // SOT layout: marker(2) + Lsot(2) + Isot(2) + Psot(4) + TPsot(1) + TNsot(1)
        if tile_part.len() >= 6
            && tile_part[0] == 0xFF
            && tile_part[1] == 0x90
        {
            out.extend_from_slice(&tile_part[..4]); // marker + Lsot
            out.extend_from_slice(&[0x00, 0x00]);   // Isot = 0
            out.extend_from_slice(&tile_part[6..]);  // rest of tile-part
        } else {
            out.extend_from_slice(tile_part);
        }
    }
    out.extend_from_slice(&[0xFF, 0xD9]); // EOC
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Helper: build a minimal valid codestream with SOC + SIZ + optional markers + SOT.
    fn build_codestream(markers: &[(u16, &[u8])]) -> Vec<u8> {
        let mut cs = Vec::new();
        // SOC
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        // SIZ with minimal body (just length field + dummy data)
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        let siz_body = [0u8; 8];
        let siz_len = (siz_body.len() + 2) as u16; // +2 for length field itself
        cs.extend_from_slice(&siz_len.to_be_bytes());
        cs.extend_from_slice(&siz_body);
        // Additional markers
        for &(code, body) in markers {
            cs.extend_from_slice(&code.to_be_bytes());
            let len = (body.len() + 2) as u16;
            cs.extend_from_slice(&len.to_be_bytes());
            cs.extend_from_slice(body);
        }
        // SOT marker (minimal: Lsot=10, Isot=0, Psot=0, TPsot=0, TNsot=1)
        cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes()); // Lsot
        cs.extend_from_slice(&0u16.to_be_bytes()); // Isot
        cs.extend_from_slice(&0u32.to_be_bytes()); // Psot
        cs.extend_from_slice(&[0u8, 1u8]); // TPsot, TNsot
        cs
    }

    #[test]
    fn test_parse_main_header_basic() {
        let cs = build_codestream(&[]);
        let info = parse_main_header(&cs).unwrap();
        // main_header should be SOC + SIZ segment
        assert_eq!(info.main_header, &cs[..info.first_sot_offset as usize]);
        assert!(info.tlm_offset_table.is_none());
        // decode_header should equal main_header when no TLM
        assert_eq!(info.decode_header, info.main_header);
    }

    #[test]
    fn test_missing_soc() {
        let cs = vec![0x00, 0x00, 0xFF, 0x51];
        let err = parse_main_header(&cs).unwrap_err();
        assert!(err.to_string().contains("missing SOC marker at offset 0"));
    }

    #[test]
    fn test_missing_siz() {
        // SOC followed by COD instead of SIZ
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::COD.to_be_bytes());
        cs.extend_from_slice(&4u16.to_be_bytes());
        cs.extend_from_slice(&[0u8; 2]);
        let err = parse_main_header(&cs).unwrap_err();
        assert!(err.to_string().contains("SIZ marker must follow SOC"));
    }

    #[test]
    fn test_truncated_no_sot() {
        // SOC + SIZ but no SOT
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        let siz_body = [0u8; 8];
        let siz_len = (siz_body.len() + 2) as u16;
        cs.extend_from_slice(&siz_len.to_be_bytes());
        cs.extend_from_slice(&siz_body);
        let err = parse_main_header(&cs).unwrap_err();
        assert!(err.to_string().contains("no SOT marker found"));
    }

    #[test]
    fn test_malformed_marker_extends_beyond() {
        // SOC + SIZ with length that extends beyond codestream
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        cs.extend_from_slice(&100u16.to_be_bytes()); // length way too large
        cs.extend_from_slice(&[0u8; 4]); // only 4 bytes of body
        let err = parse_main_header(&cs).unwrap_err();
        assert!(err.to_string().contains("extends beyond codestream"));
    }

    #[test]
    fn test_unknown_markers_preserved() {
        // Use an unknown marker code 0xFF30
        let unknown_body = [0xAA, 0xBB, 0xCC, 0xDD];
        let cs = build_codestream(&[(0xFF30, &unknown_body)]);
        let info = parse_main_header(&cs).unwrap();
        // main_header should include the unknown marker
        assert_eq!(info.main_header, &cs[..info.first_sot_offset as usize]);
        // decode_header should also include it (only TLM is stripped)
        assert_eq!(info.decode_header, info.main_header);
    }

    #[test]
    fn test_tlm_parsing_with_16bit_index_32bit_length() {
        // Build a TLM segment: Ztlm=0, Stlm with ST=2 (16-bit index), SP=1 (32-bit length)
        // Stlm = (2 << 4) | (1 << 6) = 0x20 | 0x40 = 0x60
        let mut tlm_body = Vec::new();
        let ztlm: u8 = 0;
        let stlm: u8 = 0x60; // ST=2, SP=1
        // Entry: tile_index=0, length=100
        let tile0_idx: u16 = 0;
        let tile0_len: u32 = 100;
        // Entry: tile_index=1, length=200
        let tile1_idx: u16 = 1;
        let tile1_len: u32 = 200;

        // Payload: Ztlm + Stlm + entries
        let entry_size = 2 + 4; // 16-bit index + 32-bit length
        let _payload_size = 2 + entry_size * 2; // Ztlm + Stlm + 2 entries
        // Ltlm = payload_size + 2 (for Ltlm itself)
        // But we don't include Ltlm in the body passed to build_codestream
        // because build_codestream adds the length field.
        // Actually, for TLM we need to build the raw body that goes after the marker code + length.
        // The build_codestream helper adds marker_code(2) + Lxxx(2) + body.
        // So body = Ztlm(1) + Stlm(1) + entries...
        // And Lxxx = body.len() + 2
        tlm_body.push(ztlm);
        tlm_body.push(stlm);
        tlm_body.extend_from_slice(&tile0_idx.to_be_bytes());
        tlm_body.extend_from_slice(&tile0_len.to_be_bytes());
        tlm_body.extend_from_slice(&tile1_idx.to_be_bytes());
        tlm_body.extend_from_slice(&tile1_len.to_be_bytes());

        let cs = build_codestream(&[(marker_codes::TLM, &tlm_body)]);
        let info = parse_main_header(&cs).unwrap();

        assert!(info.tlm_offset_table.is_some());
        let table = info.tlm_offset_table.unwrap();
        assert_eq!(table.len(), 2);

        // First entry: tile 0, offset = first_sot_offset, length = 100
        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].offset, info.first_sot_offset);
        assert_eq!(table[0].length, 100);

        // Second entry: tile 1, offset = first_sot_offset + 100, length = 200
        assert_eq!(table[1].tile_index, 1);
        assert_eq!(table[1].offset, info.first_sot_offset + 100);
        assert_eq!(table[1].length, 200);
    }

    #[test]
    fn test_tlm_stripping_in_decode_header() {
        // Build a TLM segment
        let mut tlm_body = Vec::new();
        tlm_body.push(0); // Ztlm
        tlm_body.push(0x60); // Stlm: ST=2, SP=1
        tlm_body.extend_from_slice(&0u16.to_be_bytes()); // tile index
        tlm_body.extend_from_slice(&100u32.to_be_bytes()); // length

        let cs = build_codestream(&[(marker_codes::TLM, &tlm_body)]);
        let info = parse_main_header(&cs).unwrap();

        // decode_header should be shorter than main_header (TLM removed)
        assert!(info.decode_header.len() < info.main_header.len());

        // decode_header should not contain TLM marker bytes
        let tlm_marker_bytes = marker_codes::TLM.to_be_bytes();
        let has_tlm = info
            .decode_header
            .windows(2)
            .any(|w| w == tlm_marker_bytes);
        assert!(!has_tlm, "decode_header should not contain TLM marker");

        // decode_header should still start with SOC
        assert_eq!(
            read_u16(&info.decode_header, 0),
            marker_codes::SOC
        );
    }

    #[test]
    fn test_tlm_sequential_no_tile_index() {
        // ST=0 means no tile index, sequential assignment
        // Stlm = (0 << 4) | (0 << 6) = 0x00 — ST=0, SP=0 (16-bit lengths)
        let mut tlm_body = Vec::new();
        tlm_body.push(0); // Ztlm
        tlm_body.push(0x00); // Stlm: ST=0, SP=0
        // Two entries with 16-bit lengths, no tile index
        tlm_body.extend_from_slice(&50u16.to_be_bytes());
        tlm_body.extend_from_slice(&75u16.to_be_bytes());

        let cs = build_codestream(&[(marker_codes::TLM, &tlm_body)]);
        let info = parse_main_header(&cs).unwrap();

        let table = info.tlm_offset_table.unwrap();
        assert_eq!(table.len(), 2);
        // Sequential: tile indices 0, 1
        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].length, 50);
        assert_eq!(table[1].tile_index, 1);
        assert_eq!(table[1].length, 75);
        // Offsets should be cumulative from first_sot_offset
        assert_eq!(table[0].offset, info.first_sot_offset);
        assert_eq!(table[1].offset, info.first_sot_offset + 50);
    }

    #[test]
    fn test_tlm_8bit_tile_index() {
        // ST=1 means 8-bit tile index
        // Stlm = (1 << 4) | (0 << 6) = 0x10 — ST=1, SP=0 (16-bit lengths)
        let mut tlm_body = Vec::new();
        tlm_body.push(0); // Ztlm
        tlm_body.push(0x10); // Stlm: ST=1, SP=0
        // Entry: tile_index=5 (8-bit), length=300 (16-bit)
        tlm_body.push(5u8);
        tlm_body.extend_from_slice(&300u16.to_be_bytes());

        let cs = build_codestream(&[(marker_codes::TLM, &tlm_body)]);
        let info = parse_main_header(&cs).unwrap();

        let table = info.tlm_offset_table.unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].tile_index, 5);
        assert_eq!(table[0].length, 300);
    }

    #[test]
    fn test_invalid_tlm_st_value() {
        // ST=3 is invalid
        let mut tlm_body = Vec::new();
        tlm_body.push(0); // Ztlm
        tlm_body.push(0x30); // Stlm: ST=3 (invalid), SP=0
        tlm_body.extend_from_slice(&100u16.to_be_bytes());

        let cs = build_codestream(&[(marker_codes::TLM, &tlm_body)]);
        let err = parse_main_header(&cs).unwrap_err();
        assert!(err.to_string().contains("Invalid TLM ST value 3"));
    }

    #[test]
    fn test_multiple_tlm_segments_combined() {
        // Two TLM segments, each with one entry
        let mut tlm1 = Vec::new();
        tlm1.push(0); // Ztlm=0
        tlm1.push(0x60); // ST=2, SP=1
        tlm1.extend_from_slice(&0u16.to_be_bytes());
        tlm1.extend_from_slice(&100u32.to_be_bytes());

        let mut tlm2 = Vec::new();
        tlm2.push(1); // Ztlm=1
        tlm2.push(0x60); // ST=2, SP=1
        tlm2.extend_from_slice(&1u16.to_be_bytes());
        tlm2.extend_from_slice(&200u32.to_be_bytes());

        let cs = build_codestream(&[
            (marker_codes::TLM, &tlm1),
            (marker_codes::TLM, &tlm2),
        ]);
        let info = parse_main_header(&cs).unwrap();

        let table = info.tlm_offset_table.unwrap();
        assert_eq!(table.len(), 2);
        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].length, 100);
        assert_eq!(table[0].offset, info.first_sot_offset);
        assert_eq!(table[1].tile_index, 1);
        assert_eq!(table[1].length, 200);
        assert_eq!(table[1].offset, info.first_sot_offset + 100);
    }

    #[test]
    fn test_first_sot_offset_correct() {
        let cs = build_codestream(&[]);
        let info = parse_main_header(&cs).unwrap();
        // SOC(2) + SIZ marker(2) + SIZ length(2) + SIZ body(8) = 14
        assert_eq!(info.first_sot_offset, 14);
    }

    #[test]
    fn test_empty_codestream() {
        let cs: Vec<u8> = Vec::new();
        let err = parse_main_header(&cs).unwrap_err();
        assert!(err.to_string().contains("missing SOC marker at offset 0"));
    }

    // ---- scan_sot_markers tests ----

    /// Helper: build a SOT marker segment (12 bytes total).
    fn build_sot(isot: u16, psot: u32, tpsot: u8, tnsot: u8) -> Vec<u8> {
        let mut sot = Vec::new();
        sot.extend_from_slice(&marker_codes::SOT.to_be_bytes());
        sot.extend_from_slice(&10u16.to_be_bytes()); // Lsot = 10
        sot.extend_from_slice(&isot.to_be_bytes());
        sot.extend_from_slice(&psot.to_be_bytes());
        sot.push(tpsot);
        sot.push(tnsot);
        sot
    }

    /// Helper: build a full codestream with main header + tile-parts + EOC.
    /// Each tile-part is (isot, psot, tpsot, tnsot, data_bytes).
    /// If psot > 0, the tile-part is padded with SOD + zeros to reach psot bytes total.
    /// If psot == 0, the tile-part gets SOD + data_bytes and extends to EOC.
    fn build_full_codestream(tile_parts: &[(u16, u32, u8, u8, &[u8])]) -> (Vec<u8>, u64) {
        let mut cs = Vec::new();
        // SOC
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        // SIZ with minimal body
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        let siz_body = [0u8; 8];
        let siz_len = (siz_body.len() + 2) as u16;
        cs.extend_from_slice(&siz_len.to_be_bytes());
        cs.extend_from_slice(&siz_body);

        let first_sot_offset = cs.len() as u64;

        for &(isot, psot, tpsot, tnsot, data) in tile_parts {
            let sot = build_sot(isot, psot, tpsot, tnsot);
            cs.extend_from_slice(&sot);
            // SOD marker
            cs.extend_from_slice(&marker_codes::SOD.to_be_bytes());
            cs.extend_from_slice(data);
            if psot > 0 {
                // Pad to reach psot total bytes for this tile-part
                // SOT header is 12 bytes, SOD is 2 bytes, data is data.len()
                let used = 12 + 2 + data.len();
                let remaining = (psot as usize).saturating_sub(used);
                cs.extend_from_slice(&vec![0u8; remaining]);
            }
        }

        // EOC
        cs.extend_from_slice(&marker_codes::EOC.to_be_bytes());

        (cs, first_sot_offset)
    }

    #[test]
    fn test_scan_sot_single_tile_psot_nonzero() {
        // Single tile-part: Isot=0, Psot=30, TPsot=0, TNsot=1
        let data = [0xAA; 10];
        let (cs, first_sot) = build_full_codestream(&[(0, 30, 0, 1, &data)]);

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].offset, first_sot);
        assert_eq!(table[0].length, 30);
    }

    #[test]
    fn test_scan_sot_single_tile_psot_zero() {
        // Single tile-part with Psot=0 (extends to end of codestream)
        let data = [0xBB; 5];
        let (cs, first_sot) = build_full_codestream(&[(0, 0, 0, 1, &data)]);

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].offset, first_sot);
        // Length should be from SOT to end of codestream (including EOC)
        let expected_length = cs.len() as u64 - first_sot;
        assert_eq!(table[0].length, expected_length);
    }

    #[test]
    fn test_scan_sot_multiple_tiles() {
        // Two tiles: tile 0 (Psot=24) and tile 1 (Psot=24)
        let data = [0xCC; 4];
        let (cs, first_sot) = build_full_codestream(&[
            (0, 24, 0, 1, &data),
            (1, 24, 0, 1, &data),
        ]);

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert_eq!(table.len(), 2);

        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].offset, first_sot);
        assert_eq!(table[0].length, 24);

        assert_eq!(table[1].tile_index, 1);
        assert_eq!(table[1].offset, first_sot + 24);
        assert_eq!(table[1].length, 24);
    }

    #[test]
    fn test_scan_sot_multi_tile_part() {
        // Tile 0 with two tile-parts: TPsot=0 and TPsot=1
        let data = [0xDD; 2];
        let (cs, first_sot) = build_full_codestream(&[
            (0, 20, 0, 2, &data),
            (0, 20, 1, 2, &data),
        ]);

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert_eq!(table.len(), 2);

        // Both entries should have tile_index=0
        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].offset, first_sot);
        assert_eq!(table[0].length, 20);

        assert_eq!(table[1].tile_index, 0);
        assert_eq!(table[1].offset, first_sot + 20);
        assert_eq!(table[1].length, 20);
    }

    #[test]
    fn test_scan_sot_stops_at_eoc() {
        // Build a codestream with one tile-part followed by EOC
        let data = [0xEE; 2];
        let (cs, first_sot) = build_full_codestream(&[(0, 20, 0, 1, &data)]);

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert_eq!(table.len(), 1);
        // Should stop at EOC, not try to read beyond
    }

    #[test]
    fn test_scan_sot_invalid_lsot() {
        // Build a codestream where the SOT has Lsot != 10
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes());
        cs.extend_from_slice(&[0u8; 8]);
        let first_sot = cs.len() as u64;
        // SOT with Lsot=8 (invalid, should be 10)
        cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
        cs.extend_from_slice(&8u16.to_be_bytes()); // wrong Lsot
        cs.extend_from_slice(&0u16.to_be_bytes()); // Isot
        cs.extend_from_slice(&0u32.to_be_bytes()); // Psot
        cs.extend_from_slice(&[0u8, 1u8]); // TPsot, TNsot

        let err = scan_sot_markers(&cs, first_sot).unwrap_err();
        assert!(err.to_string().contains("Invalid SOT marker length 8"));
        assert!(err.to_string().contains("expected 10"));
    }

    #[test]
    fn test_scan_sot_truncated() {
        // Codestream that ends mid-SOT
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes());
        cs.extend_from_slice(&[0u8; 8]);
        let first_sot = cs.len() as u64;
        // Only 6 bytes of SOT (need 12)
        cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes());
        // Missing Isot, Psot, TPsot, TNsot

        let err = scan_sot_markers(&cs, first_sot).unwrap_err();
        assert!(err.to_string().contains("SOT marker truncated at offset"));
    }

    #[test]
    fn test_scan_sot_psot_exceeds_codestream() {
        // SOT with Psot that goes beyond codestream length
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes());
        cs.extend_from_slice(&[0u8; 8]);
        let first_sot = cs.len() as u64;
        // SOT with Psot=9999 (way too large)
        cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes());
        cs.extend_from_slice(&0u16.to_be_bytes()); // Isot
        cs.extend_from_slice(&9999u32.to_be_bytes()); // Psot too large
        cs.extend_from_slice(&[0u8, 1u8]); // TPsot, TNsot

        let err = scan_sot_markers(&cs, first_sot).unwrap_err();
        assert!(err.to_string().contains("SOT Psot=9999"));
        assert!(err.to_string().contains("exceeds codestream length"));
    }

    #[test]
    fn test_scan_sot_empty_at_offset() {
        // Codestream where first_sot_offset points to EOC immediately
        let mut cs = Vec::new();
        cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        cs.extend_from_slice(&10u16.to_be_bytes());
        cs.extend_from_slice(&[0u8; 8]);
        let first_sot = cs.len() as u64;
        cs.extend_from_slice(&marker_codes::EOC.to_be_bytes());

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert!(table.is_empty());
    }

    #[test]
    fn test_scan_sot_last_tile_psot_zero_with_preceding() {
        // Two tiles: first with Psot=20, second with Psot=0
        let data = [0xFF; 2];
        let (cs, first_sot) = build_full_codestream(&[
            (0, 20, 0, 1, &data),
            (1, 0, 0, 1, &data),
        ]);

        let table = scan_sot_markers(&cs, first_sot).unwrap();
        assert_eq!(table.len(), 2);

        assert_eq!(table[0].tile_index, 0);
        assert_eq!(table[0].length, 20);

        assert_eq!(table[1].tile_index, 1);
        assert_eq!(table[1].offset, first_sot + 20);
        // Psot=0: length extends to end of codestream
        let expected_length = cs.len() as u64 - (first_sot + 20);
        assert_eq!(table[1].length, expected_length);
    }

    // ---- build_minimal_codestream tests ----

    #[test]
    fn test_build_minimal_codestream_single_tile_part() {
        // Simulate a decode header (SOC + SIZ)
        let mut decode_header = Vec::new();
        decode_header.extend_from_slice(&marker_codes::SOC.to_be_bytes());
        decode_header.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
        decode_header.extend_from_slice(&10u16.to_be_bytes());
        decode_header.extend_from_slice(&[0u8; 8]);

        // Build a fake codestream with tile-part data at a known offset
        let mut codestream = vec![0u8; 100];
        let tile_data = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
        let tile_offset = 50u64;
        codestream[50..55].copy_from_slice(&tile_data);

        let tile_parts = [(tile_offset, tile_data.len() as u64)];
        let result = build_minimal_codestream(&decode_header, &tile_parts, &codestream);

        // Verify: decode_header ++ tile_data ++ EOC
        let mut expected = Vec::new();
        expected.extend_from_slice(&decode_header);
        expected.extend_from_slice(&tile_data);
        expected.extend_from_slice(&[0xFF, 0xD9]);
        assert_eq!(result, expected);

        // Verify starts with SOC and ends with EOC
        assert_eq!(read_u16(&result, 0), marker_codes::SOC);
        assert_eq!(
            read_u16(&result, result.len() - 2),
            marker_codes::EOC
        );

        // Verify exact capacity was pre-allocated
        assert_eq!(result.len(), result.capacity());
    }

    #[test]
    fn test_build_minimal_codestream_multiple_tile_parts() {
        let decode_header = marker_codes::SOC.to_be_bytes().to_vec();

        let mut codestream = vec![0u8; 200];
        let part1 = [0x11, 0x22, 0x33];
        let part2 = [0x44, 0x55];
        codestream[10..13].copy_from_slice(&part1);
        codestream[80..82].copy_from_slice(&part2);

        let tile_parts = [(10u64, 3u64), (80u64, 2u64)];
        let result = build_minimal_codestream(&decode_header, &tile_parts, &codestream);

        let mut expected = Vec::new();
        expected.extend_from_slice(&decode_header);
        expected.extend_from_slice(&part1);
        expected.extend_from_slice(&part2);
        expected.extend_from_slice(&[0xFF, 0xD9]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_build_minimal_codestream_empty_tile_parts() {
        let decode_header = vec![0xFF, 0x4F, 0xFF, 0x51];
        let codestream = vec![0u8; 10];
        let tile_parts: &[(u64, u64)] = &[];

        let result = build_minimal_codestream(&decode_header, tile_parts, &codestream);

        let mut expected = Vec::new();
        expected.extend_from_slice(&decode_header);
        expected.extend_from_slice(&[0xFF, 0xD9]);
        assert_eq!(result, expected);
    }

    // ---- Property-based tests ----

    /// **Validates: Requirements 1.1, 1.2, 1.5**
    ///
    /// Property 1: Main Header Extraction Preserves Bytes
    /// For any valid J2K codestream containing SOC, SIZ, and at least one SOT marker,
    /// the bytes returned by parse_main_header().main_header shall be byte-identical
    /// to codestream[0..first_sot_offset].
    proptest! {
        #[test]
        fn prop_main_header_extraction_preserves_bytes(
            siz_body in prop::collection::vec(any::<u8>(), 4..20),
            extra_markers in prop::collection::vec(
                (0xFF30u16..0xFF40u16, prop::collection::vec(any::<u8>(), 0..30)),
                0..5usize
            ),
        ) {
            // Build a valid codestream: SOC + SIZ(random body) + random markers + SOT
            let mut cs = Vec::new();

            // SOC
            cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());

            // SIZ with random body
            cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
            let siz_len = (siz_body.len() + 2) as u16; // +2 for length field itself
            cs.extend_from_slice(&siz_len.to_be_bytes());
            cs.extend_from_slice(&siz_body);

            // Extra random marker segments (using safe codes 0xFF30..0xFF3F)
            for (marker_code, body) in &extra_markers {
                cs.extend_from_slice(&marker_code.to_be_bytes());
                let len = (body.len() + 2) as u16;
                cs.extend_from_slice(&len.to_be_bytes());
                cs.extend_from_slice(body);
            }

            // Record where SOT starts
            let expected_sot_offset = cs.len() as u64;

            // SOT marker (Lsot=10, Isot=0, Psot=0, TPsot=0, TNsot=1)
            cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
            cs.extend_from_slice(&10u16.to_be_bytes());
            cs.extend_from_slice(&0u16.to_be_bytes());
            cs.extend_from_slice(&0u32.to_be_bytes());
            cs.extend_from_slice(&[0u8, 1u8]);

            // Parse the main header
            let info = parse_main_header(&cs).unwrap();

            // Property: main_header bytes are identical to codestream[0..first_sot_offset]
            prop_assert_eq!(
                &info.main_header[..],
                &cs[..info.first_sot_offset as usize],
                "main_header must be byte-identical to codestream[0..first_sot_offset]"
            );

            // Property: first_sot_offset points to the SOT marker
            prop_assert_eq!(
                info.first_sot_offset,
                expected_sot_offset,
                "first_sot_offset must point to the SOT marker position"
            );

            // Verify the bytes at first_sot_offset are indeed the SOT marker
            prop_assert_eq!(
                read_u16(&cs, info.first_sot_offset as usize),
                marker_codes::SOT,
                "bytes at first_sot_offset must be the SOT marker code"
            );
        }
    }

    /// **Validates: Requirements 1.6**
    ///
    /// Property 2: TLM Stripping Round-Trip
    /// For any valid main header with 0–5 TLM segments and 0–3 non-TLM marker
    /// segments, the decode header produced by parse_main_header().decode_header
    /// shall satisfy:
    /// (a) it contains no TLM marker bytes (no 0xFF55 in 2-byte windows),
    /// (b) all non-TLM marker segments from the original main header are present
    ///     and byte-identical in the decode_header,
    /// (c) decode_header starts with SOC (0xFF4F),
    /// (d) decode_header length equals main_header length minus total TLM segment bytes.
    proptest! {
        #[test]
        fn prop_tlm_stripping_round_trip(
            num_tlm in 0u8..5,
            non_tlm_markers in prop::collection::vec(
                (prop::sample::select(vec![0xFF30u16, 0xFF31, 0xFF52, 0xFF53, 0xFF5C]),
                 prop::collection::vec(any::<u8>(), 2..20)),
                0..3usize
            ),
            tile_lengths in prop::collection::vec(1u32..10000, 1..4usize),
        ) {
            // Build TLM segment bodies (Stlm=0x60: ST=2/16-bit index, SP=1/32-bit length)
            let mut tlm_segments: Vec<Vec<u8>> = Vec::new();
            for i in 0..num_tlm {
                let mut body = Vec::new();
                body.push(i); // Ztlm
                body.push(0x60); // Stlm: ST=2, SP=1
                for (idx, &tlen) in tile_lengths.iter().enumerate() {
                    body.extend_from_slice(&(idx as u16).to_be_bytes());
                    body.extend_from_slice(&tlen.to_be_bytes());
                }
                tlm_segments.push(body);
            }

            // Build interleaved marker list: mix TLM and non-TLM markers
            // Place non-TLM markers first, then TLM markers (simple interleave)
            let mut markers: Vec<(u16, Vec<u8>)> = Vec::new();
            let mut non_tlm_idx = 0;
            let mut tlm_idx = 0;
            loop {
                let have_non_tlm = non_tlm_idx < non_tlm_markers.len();
                let have_tlm = tlm_idx < tlm_segments.len();
                if !have_non_tlm && !have_tlm {
                    break;
                }
                // Alternate: non-TLM first, then TLM
                if have_non_tlm {
                    let (code, ref body) = non_tlm_markers[non_tlm_idx];
                    markers.push((code, body.clone()));
                    non_tlm_idx += 1;
                }
                if have_tlm {
                    markers.push((marker_codes::TLM, tlm_segments[tlm_idx].clone()));
                    tlm_idx += 1;
                }
            }

            // Build codestream: SOC + SIZ + interleaved markers + SOT
            let mut cs = Vec::new();
            cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());
            // Minimal SIZ
            cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
            let siz_body = [0u8; 8];
            let siz_len = (siz_body.len() + 2) as u16;
            cs.extend_from_slice(&siz_len.to_be_bytes());
            cs.extend_from_slice(&siz_body);

            // Track total TLM bytes and non-TLM marker raw bytes for verification
            let mut total_tlm_bytes: usize = 0;
            let mut non_tlm_raw_segments: Vec<Vec<u8>> = Vec::new();

            for (code, body) in &markers {
                let seg_len = (body.len() + 2) as u16; // +2 for length field
                let total_seg = 2 + seg_len as usize; // marker code + length field + body
                if *code == marker_codes::TLM {
                    total_tlm_bytes += total_seg;
                } else {
                    // Capture the raw bytes of this non-TLM marker segment
                    let mut raw = Vec::new();
                    raw.extend_from_slice(&code.to_be_bytes());
                    raw.extend_from_slice(&seg_len.to_be_bytes());
                    raw.extend_from_slice(body);
                    non_tlm_raw_segments.push(raw);
                }
                cs.extend_from_slice(&code.to_be_bytes());
                cs.extend_from_slice(&seg_len.to_be_bytes());
                cs.extend_from_slice(body);
            }

            // SOT marker
            cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
            cs.extend_from_slice(&10u16.to_be_bytes());
            cs.extend_from_slice(&0u16.to_be_bytes());
            cs.extend_from_slice(&0u32.to_be_bytes());
            cs.extend_from_slice(&[0u8, 1u8]);

            // Parse
            let info = parse_main_header(&cs).unwrap();

            // (a) decode_header contains no TLM marker segments
            // Walk the decode_header marker-by-marker (skip SOC at offset 0)
            // and verify no marker code is TLM (0xFF55).
            {
                let dh = &info.decode_header;
                let mut dh_pos = 2; // skip SOC
                while dh_pos + 2 <= dh.len() {
                    let mk = read_u16(dh, dh_pos);
                    if mk == marker_codes::SOT {
                        break; // shouldn't appear in decode_header, but stop if found
                    }
                    prop_assert_ne!(mk, marker_codes::TLM,
                        "decode_header must not contain TLM marker segments");
                    if dh_pos + 4 <= dh.len() {
                        let ml = read_u16(dh, dh_pos + 2) as usize;
                        dh_pos += 2 + ml;
                    } else {
                        break;
                    }
                }
            }

            // (b) All non-TLM marker segments are present and byte-identical
            for raw_seg in &non_tlm_raw_segments {
                let found = info.decode_header.windows(raw_seg.len()).any(|w| w == raw_seg.as_slice());
                prop_assert!(found,
                    "non-TLM marker segment must be present in decode_header: {:02X?}",
                    &raw_seg[..2]
                );
            }

            // (c) decode_header starts with SOC
            prop_assert!(info.decode_header.len() >= 2, "decode_header must be at least 2 bytes");
            prop_assert_eq!(
                read_u16(&info.decode_header, 0),
                marker_codes::SOC,
                "decode_header must start with SOC (0xFF4F)"
            );

            // (d) decode_header length equals main_header length minus total TLM segment bytes
            prop_assert_eq!(
                info.decode_header.len(),
                info.main_header.len() - total_tlm_bytes,
                "decode_header length must equal main_header length minus TLM bytes"
            );
        }

        /// **Validates: Requirements 2.1, 2.3, 3.2**
        ///
        /// Property 3: TLM Parse and SOT Scan Equivalence
        /// For any valid J2K codestream that contains TLM markers, the
        /// TilePartOffsetTable produced by parsing TLM markers shall be
        /// entry-equivalent (same tile indices, offsets, and lengths) to the
        /// table produced by scanning SOT markers on the same codestream.
        #[test]
        fn prop_tlm_sot_equivalence(
            tile_data_sizes in prop::collection::vec(20u32..100, 1..4usize),
        ) {
            // Compute Psot for each tile: SOT(12) + SOD(2) + data_size
            let psots: Vec<u32> = tile_data_sizes.iter().map(|&ds| 14 + ds).collect();

            // --- Build TLM body ---
            // Stlm = 0x60: ST=2 (16-bit tile index), SP=1 (32-bit length)
            let mut tlm_body = Vec::new();
            tlm_body.push(0u8); // Ztlm
            tlm_body.push(0x60u8); // Stlm: ST=2, SP=1
            for (i, &psot) in psots.iter().enumerate() {
                tlm_body.extend_from_slice(&(i as u16).to_be_bytes());
                tlm_body.extend_from_slice(&psot.to_be_bytes());
            }

            // --- Build codestream: SOC + SIZ + TLM + tile-parts + EOC ---
            let mut cs = Vec::new();

            // SOC
            cs.extend_from_slice(&marker_codes::SOC.to_be_bytes());

            // Minimal SIZ
            cs.extend_from_slice(&marker_codes::SIZ.to_be_bytes());
            let siz_body = [0u8; 8];
            let siz_len = (siz_body.len() + 2) as u16;
            cs.extend_from_slice(&siz_len.to_be_bytes());
            cs.extend_from_slice(&siz_body);

            // TLM marker segment
            cs.extend_from_slice(&marker_codes::TLM.to_be_bytes());
            let tlm_seg_len = (tlm_body.len() + 2) as u16; // +2 for Ltlm itself
            cs.extend_from_slice(&tlm_seg_len.to_be_bytes());
            cs.extend_from_slice(&tlm_body);

            let first_sot_offset = cs.len() as u64;

            // Tile-parts: SOT + SOD + data for each tile
            for (i, &data_size) in tile_data_sizes.iter().enumerate() {
                let psot = psots[i];
                // SOT marker (12 bytes)
                cs.extend_from_slice(&marker_codes::SOT.to_be_bytes());
                cs.extend_from_slice(&10u16.to_be_bytes()); // Lsot
                cs.extend_from_slice(&(i as u16).to_be_bytes()); // Isot
                cs.extend_from_slice(&psot.to_be_bytes()); // Psot
                cs.push(0u8); // TPsot
                cs.push(1u8); // TNsot

                // SOD marker (2 bytes)
                cs.extend_from_slice(&marker_codes::SOD.to_be_bytes());

                // Data bytes (fill with tile index for variety)
                cs.extend_from_slice(&vec![i as u8; data_size as usize]);
            }

            // EOC
            cs.extend_from_slice(&marker_codes::EOC.to_be_bytes());

            // --- Parse via TLM (parse_main_header) ---
            let info = parse_main_header(&cs).unwrap();
            prop_assert_eq!(info.first_sot_offset, first_sot_offset,
                "first_sot_offset must match expected value");
            let tlm_table = info.tlm_offset_table.expect("TLM table must be present");

            // --- Parse via SOT scan ---
            let sot_table = scan_sot_markers(&cs, first_sot_offset).unwrap();

            // --- Compare entry-by-entry ---
            prop_assert_eq!(tlm_table.len(), sot_table.len(),
                "TLM and SOT tables must have the same number of entries");

            for (i, (tlm_entry, sot_entry)) in tlm_table.iter().zip(sot_table.iter()).enumerate() {
                prop_assert_eq!(tlm_entry.tile_index, sot_entry.tile_index,
                    "Entry {}: tile indices must match (TLM={}, SOT={})",
                    i, tlm_entry.tile_index, sot_entry.tile_index);
                prop_assert_eq!(tlm_entry.offset, sot_entry.offset,
                    "Entry {}: offsets must match (TLM={}, SOT={})",
                    i, tlm_entry.offset, sot_entry.offset);
                prop_assert_eq!(tlm_entry.length, sot_entry.length,
                    "Entry {}: lengths must match (TLM={}, SOT={})",
                    i, tlm_entry.length, sot_entry.length);
            }
        }

        /// **Validates: Requirements 5.2, 5.4**
        ///
        /// Property 5: Minimal Codestream Construction
        /// For any decode header and set of tile-part byte ranges,
        /// `build_minimal_codestream()` shall produce a byte sequence equal to
        /// `decode_header ++ tile_part_bytes_in_order ++ [0xFF, 0xD9]`, and the
        /// result shall begin with SOC (0xFF4F) and end with EOC (0xFFD9).
        #[test]
        fn prop_minimal_codestream_construction(
            header_extra in prop::collection::vec(any::<u8>(), 8..98),
            codestream_data in prop::collection::vec(any::<u8>(), 200..500),
            num_parts in 1u8..4,
        ) {
            // Build decode_header starting with SOC
            let mut decode_header = vec![0xFF, 0x4F]; // SOC
            decode_header.extend_from_slice(&header_extra);

            // Generate tile-part ranges that fit within codestream_data
            let cs_len = codestream_data.len();
            let part_size = cs_len / (num_parts as usize + 1);
            let tile_parts: Vec<(u64, u64)> = (0..num_parts as usize)
                .map(|i| (i as u64 * part_size as u64, part_size as u64))
                .collect();

            let result = build_minimal_codestream(&decode_header, &tile_parts, &codestream_data);

            // Verify starts with SOC
            prop_assert_eq!(result[0], 0xFF);
            prop_assert_eq!(result[1], 0x4F);

            // Verify ends with EOC
            let len = result.len();
            prop_assert_eq!(result[len - 2], 0xFF);
            prop_assert_eq!(result[len - 1], 0xD9);

            // Verify content: decode_header ++ tile_parts ++ EOC
            let mut expected = decode_header.clone();
            for &(offset, length) in &tile_parts {
                expected.extend_from_slice(&codestream_data[offset as usize..(offset + length) as usize]);
            }
            expected.extend_from_slice(&[0xFF, 0xD9]);
            prop_assert_eq!(&result, &expected);

            // Verify length: decode_header.len() + sum(tile_part_lengths) + 2
            let total_tile_bytes: u64 = tile_parts.iter().map(|(_, l)| l).sum();
            prop_assert_eq!(
                result.len(),
                decode_header.len() + total_tile_bytes as usize + 2
            );
        }
    }
}
