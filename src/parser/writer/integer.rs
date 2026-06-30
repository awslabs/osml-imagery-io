//! Integer encoding with endianness support.
//!
//! This module handles encoding signed and unsigned integers into binary format
//! with configurable byte order (big-endian or little-endian).
//!
//! Like the read path ([`crate::parser::accessor::read`]), encoding is generic
//! over any width in `1..=8` bytes rather than a `{1,2,4,8}` whitelist: NITF/BIIF
//! integer fields are arbitrary-width big-endian values (e.g. ILLUMB's 3-byte
//! `EXISTENCE_MASK`). The two sides are deliberately symmetric — every width the
//! reader accepts, the writer can produce, and vice versa.

use crate::parser::error::WriteError;
use crate::parser::types::Endian;

/// Serialize the low `size` bytes of a `u64` bit pattern honoring byte order.
///
/// Shared core for [`encode_unsigned`] and [`encode_signed`]. The caller is
/// responsible for range-checking; this only slices the two's-complement /
/// magnitude bytes. `size` is assumed to be in `1..=8` (callers validate it
/// while computing the value range, so an out-of-range width never reaches
/// here).
fn serialize_low_bytes(bits: u64, size: u8, endian: Endian) -> Vec<u8> {
    let n = size as usize;
    match endian {
        // Big-endian: most-significant byte first. The low `n` bytes of the
        // 8-byte big-endian image are its last `n` bytes.
        Endian::Big => bits.to_be_bytes()[8 - n..].to_vec(),
        // Little-endian: least-significant byte first — the first `n` bytes.
        Endian::Little => bits.to_le_bytes()[..n].to_vec(),
    }
}

/// Largest unsigned value representable in `byte_size` bytes, or `None` if the
/// width is outside the supported `1..=8` range.
fn unsigned_max(byte_size: u8) -> Option<u64> {
    match byte_size {
        1..=7 => Some((1u64 << (8 * byte_size as u32)) - 1),
        8 => Some(u64::MAX),
        _ => None,
    }
}

/// Inclusive signed range representable in `byte_size` bytes, or `None` if the
/// width is outside the supported `1..=8` range.
fn signed_range(byte_size: u8) -> Option<(i64, i64)> {
    match byte_size {
        1..=7 => {
            let bits = 8 * byte_size as u32;
            let max = (1i64 << (bits - 1)) - 1;
            let min = -(1i64 << (bits - 1));
            Some((min, max))
        }
        8 => Some((i64::MIN, i64::MAX)),
        _ => None,
    }
}

/// Encode an unsigned integer of any width in `1..=8` with the given endianness.
pub fn encode_unsigned(
    n: u64,
    byte_size: u8,
    endian: Endian,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    let Some(max_value) = unsigned_max(byte_size) else {
        return Err(WriteError::ValidationError {
            path: path.to_string(),
            message: format!("Unsupported integer size: {}", byte_size),
        });
    };

    if n > max_value {
        return Err(WriteError::ValueTooLarge {
            path: path.to_string(),
            max_size: byte_size as usize,
            actual_size: 8, // u64 size
        });
    }

    Ok(serialize_low_bytes(n, byte_size, endian))
}

/// Encode a signed integer of any width in `1..=8` with the given endianness.
///
/// The value is serialized as two's complement; the low `byte_size` bytes of the
/// 8-byte image carry the correct sign bits, so [`crate::parser::accessor::read::read_signed`]
/// sign-extends them back to the original value.
pub fn encode_signed(
    n: i64,
    byte_size: u8,
    endian: Endian,
    path: &str,
) -> Result<Vec<u8>, WriteError> {
    let Some((min_value, max_value)) = signed_range(byte_size) else {
        return Err(WriteError::ValidationError {
            path: path.to_string(),
            message: format!("Unsupported integer size: {}", byte_size),
        });
    };

    if n < min_value || n > max_value {
        return Err(WriteError::ValueTooLarge {
            path: path.to_string(),
            max_size: byte_size as usize,
            actual_size: 8,
        });
    }

    Ok(serialize_low_bytes(n as u64, byte_size, endian))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::accessor::read::{read_signed, read_unsigned};

    #[test]
    fn unsigned_width3_byte_layout() {
        // ILLUMB EXISTENCE_MASK upper bound, big-endian.
        assert_eq!(
            encode_unsigned(0xFFFF00, 3, Endian::Big, "X").unwrap(),
            vec![0xFF, 0xFF, 0x00]
        );
        // Same value little-endian is byte-reversed.
        assert_eq!(
            encode_unsigned(0xFFFF00, 3, Endian::Little, "X").unwrap(),
            vec![0x00, 0xFF, 0xFF]
        );
    }

    #[test]
    fn signed_width3_two_complement_layout() {
        // -1 over 3 bytes is 0xFFFFFF.
        assert_eq!(
            encode_signed(-1, 3, Endian::Big, "X").unwrap(),
            vec![0xFF, 0xFF, 0xFF]
        );
        // Most-negative 24-bit value.
        assert_eq!(
            encode_signed(-8_388_608, 3, Endian::Big, "X").unwrap(),
            vec![0x80, 0x00, 0x00]
        );
    }

    #[test]
    fn unsigned_round_trips_reader_all_widths() {
        // Every width the reader accepts, the writer produces — and the cycle
        // is lossless for both endians.
        for size in 1u8..=8 {
            let max = unsigned_max(size).unwrap();
            for &v in &[0u64, 1, max / 3, max] {
                for endian in [Endian::Big, Endian::Little] {
                    let bytes = encode_unsigned(v, size, endian, "X").unwrap();
                    assert_eq!(bytes.len(), size as usize);
                    assert_eq!(read_unsigned(&bytes, size, endian).unwrap(), v);
                }
            }
        }
    }

    #[test]
    fn signed_round_trips_reader_all_widths() {
        for size in 1u8..=8 {
            let (min, max) = signed_range(size).unwrap();
            for &v in &[min, -1, 0, 1, max] {
                for endian in [Endian::Big, Endian::Little] {
                    let bytes = encode_signed(v, size, endian, "X").unwrap();
                    assert_eq!(bytes.len(), size as usize);
                    assert_eq!(read_signed(&bytes, size, endian).unwrap(), v);
                }
            }
        }
    }

    #[test]
    fn rejects_unsupported_widths() {
        assert!(matches!(
            encode_unsigned(0, 0, Endian::Big, "X"),
            Err(WriteError::ValidationError { .. })
        ));
        assert!(matches!(
            encode_unsigned(0, 9, Endian::Big, "X"),
            Err(WriteError::ValidationError { .. })
        ));
        assert!(matches!(
            encode_signed(0, 0, Endian::Big, "X"),
            Err(WriteError::ValidationError { .. })
        ));
        assert!(matches!(
            encode_signed(0, 9, Endian::Big, "X"),
            Err(WriteError::ValidationError { .. })
        ));
    }

    #[test]
    fn rejects_out_of_range_values() {
        // 0xFFFF00 + 1 does not fit in 3 bytes.
        assert!(matches!(
            encode_unsigned(0x100_0000, 3, Endian::Big, "X"),
            Err(WriteError::ValueTooLarge { .. })
        ));
        // 2^23 overflows a signed 3-byte field.
        assert!(matches!(
            encode_signed(1 << 23, 3, Endian::Big, "X"),
            Err(WriteError::ValueTooLarge { .. })
        ));
        assert!(matches!(
            encode_signed(-(1 << 23) - 1, 3, Endian::Big, "X"),
            Err(WriteError::ValueTooLarge { .. })
        ));
    }
}
