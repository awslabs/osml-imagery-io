//! Row-aware sub-byte sample packing and unpacking.
//!
//! TIFF stores sub-byte samples (BitsPerSample ∈ {1, 2, 4}) packed multiple
//! samples per byte, MSB-first, with each *row* padded to a byte boundary
//! (TIFF 6.0 p.13). This module unpacks such data to one `u8` per sample (and
//! packs it back), so the rest of the pipeline can treat sub-byte imagery as
//! ordinary `uint8` arrays.
//!
//! # Relationship to JBP bilevel
//!
//! This is deliberately *separate* from JBP's `unpack_bilevel`
//! (`src/jbp/image/pixel.rs`), which treats the packed samples as a single flat
//! bitstream with no per-row padding. Unifying the two would require a mode flag
//! and risk a JBP change silently altering TIFF behavior. See the design doc
//! (`docs/internal/DESIGN_TIFF_MASK_BILEVEL.md`, "Components").
//!
//! # Layout
//!
//! For `bits_per_sample = N`, each byte holds `8 / N` samples. The first sample
//! in a byte occupies the most-significant `N` bits. Each row of
//! `samples_per_row` samples is stored in `ceil(samples_per_row * N / 8)` bytes;
//! any unused low bits in a row's final byte are padding.

/// Number of bytes used to store one row of `samples_per_row` samples at
/// `bits_per_sample` bits each, rounded up to a byte boundary.
fn bytes_per_row(bits_per_sample: u8, samples_per_row: u32) -> usize {
    let total_bits = samples_per_row as usize * bits_per_sample as usize;
    total_bits.div_ceil(8)
}

/// Unpack sub-byte samples (bits ∈ {1, 2, 4}) to one `u8` per sample, MSB-first,
/// respecting TIFF's per-row byte-boundary padding.
///
/// Each output byte holds the N-bit field value: 1-bit → {0, 1}, 2-bit →
/// {0..3}, 4-bit → {0..15}. The output has exactly `samples_per_row * num_rows`
/// elements, in row-major order.
///
/// # Arguments
/// * `packed` - The packed byte buffer (rows padded to byte boundaries).
/// * `bits_per_sample` - Bits per sample; must be 1, 2, or 4.
/// * `samples_per_row` - Number of samples in each row (e.g. tile row stride).
/// * `num_rows` - Number of rows.
///
/// # Panics
/// Panics if `bits_per_sample` is not 1, 2, or 4.
///
/// Samples for which `packed` is too short are emitted as `0` rather than
/// panicking, mirroring JBP's tolerant `unpack_bilevel`.
pub fn unpack_subbyte_msb_first(
    packed: &[u8],
    bits_per_sample: u8,
    samples_per_row: u32,
    num_rows: u32,
) -> Vec<u8> {
    assert!(
        matches!(bits_per_sample, 1 | 2 | 4),
        "bits_per_sample must be 1, 2, or 4, got {}",
        bits_per_sample
    );

    let bits = bits_per_sample as usize;
    let row_bytes = bytes_per_row(bits_per_sample, samples_per_row);
    let mask = (1u16 << bits) as u8 - 1;

    let mut result = Vec::with_capacity(samples_per_row as usize * num_rows as usize);

    for row in 0..num_rows as usize {
        let row_start = row * row_bytes;
        for col in 0..samples_per_row as usize {
            let bit_offset = col * bits;
            let byte_index = row_start + bit_offset / 8;
            // MSB-first: the sample's high bit sits `shift` bits above the byte's LSB.
            let shift = 8 - bits - (bit_offset % 8);
            let value = if byte_index < packed.len() {
                (packed[byte_index] >> shift) & mask
            } else {
                0
            };
            result.push(value);
        }
    }

    result
}

/// Pack one-`u8`-per-sample data into sub-byte samples (bits ∈ {1, 2, 4}),
/// MSB-first, padding each row to a byte boundary.
///
/// The inverse of [`unpack_subbyte_msb_first`]. Only the low `bits_per_sample`
/// bits of each input byte are stored; higher bits are ignored. The output has
/// exactly `ceil(samples_per_row * bits / 8) * num_rows` bytes.
///
/// # Arguments
/// * `unpacked` - One `u8` per sample, row-major, `samples_per_row * num_rows`
///   elements.
/// * `bits_per_sample` - Bits per sample; must be 1, 2, or 4.
/// * `samples_per_row` - Number of samples in each row.
/// * `num_rows` - Number of rows.
///
/// # Panics
/// Panics if `bits_per_sample` is not 1, 2, or 4.
///
/// Samples for which `unpacked` is too short are packed as `0`.
pub fn pack_subbyte_msb_first(
    unpacked: &[u8],
    bits_per_sample: u8,
    samples_per_row: u32,
    num_rows: u32,
) -> Vec<u8> {
    assert!(
        matches!(bits_per_sample, 1 | 2 | 4),
        "bits_per_sample must be 1, 2, or 4, got {}",
        bits_per_sample
    );

    let bits = bits_per_sample as usize;
    let row_bytes = bytes_per_row(bits_per_sample, samples_per_row);
    let mask = (1u16 << bits) as u8 - 1;

    let mut result = vec![0u8; row_bytes * num_rows as usize];

    for row in 0..num_rows as usize {
        let row_start = row * row_bytes;
        for col in 0..samples_per_row as usize {
            let sample_index = row * samples_per_row as usize + col;
            let value = unpacked.get(sample_index).copied().unwrap_or(0) & mask;
            if value == 0 {
                continue;
            }
            let bit_offset = col * bits;
            let byte_index = row_start + bit_offset / 8;
            let shift = 8 - bits - (bit_offset % 8);
            result[byte_index] |= value << shift;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Exact-value unit tests ------------------------------------------

    #[test]
    fn unpack_1bit_full_byte() {
        // 0b1010_1010, one row of 8 samples.
        let unpacked = unpack_subbyte_msb_first(&[0b1010_1010], 1, 8, 1);
        assert_eq!(unpacked, vec![1, 0, 1, 0, 1, 0, 1, 0]);
    }

    #[test]
    fn unpack_1bit_partial_final_byte_with_row_padding() {
        // 5 samples per row → 1 byte per row (5 bits + 3 padding bits).
        // Row 0: 0b1110_0000 (bits 11100 + padding), Row 1: 0b0011_1000 (00111).
        let unpacked = unpack_subbyte_msb_first(&[0b1110_0000, 0b0011_1000], 1, 5, 2);
        assert_eq!(unpacked, vec![1, 1, 1, 0, 0, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn unpack_2bit_values() {
        // 0b11_10_01_00 → 3, 2, 1, 0.
        let unpacked = unpack_subbyte_msb_first(&[0b11_10_01_00], 2, 4, 1);
        assert_eq!(unpacked, vec![3, 2, 1, 0]);
    }

    #[test]
    fn unpack_4bit_values() {
        // 0xAB → 0xA, 0xB.
        let unpacked = unpack_subbyte_msb_first(&[0xAB, 0xCD], 4, 4, 1);
        assert_eq!(unpacked, vec![0xA, 0xB, 0xC, 0xD]);
    }

    #[test]
    fn unpack_row_padding_distinct_from_flat_bitstream() {
        // 3 samples/row, 2 rows, 1-bit. Row-padded layout uses 1 byte per row:
        //   byte 0 = row 0, byte 1 = row 1.
        // A flat bitstream would pack all 6 bits into a single byte, so this
        // asserts the row stride is honored (the key JBP difference).
        let packed = [0b1010_0000, 0b0100_0000];
        let unpacked = unpack_subbyte_msb_first(&packed, 1, 3, 2);
        assert_eq!(unpacked, vec![1, 0, 1, 0, 1, 0]);

        // Same bytes read as a flat 6-bit stream (samples_per_row=6, 1 row)
        // would yield a different result, confirming padding matters.
        let flat = unpack_subbyte_msb_first(&packed, 1, 6, 1);
        assert_eq!(flat, vec![1, 0, 1, 0, 0, 0]);
        assert_ne!(unpacked[..6], flat[..6]);
    }

    #[test]
    fn unpack_short_buffer_pads_with_zero() {
        // Ask for more rows than the buffer provides.
        let unpacked = unpack_subbyte_msb_first(&[0xFF], 1, 8, 2);
        assert_eq!(
            unpacked,
            vec![1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn pack_1bit_partial_final_byte_with_row_padding() {
        let packed = pack_subbyte_msb_first(&[1, 1, 1, 0, 0, 0, 0, 1, 1, 1], 1, 5, 2);
        assert_eq!(packed, vec![0b1110_0000, 0b0011_1000]);
    }

    #[test]
    fn pack_2bit_values() {
        let packed = pack_subbyte_msb_first(&[3, 2, 1, 0], 2, 4, 1);
        assert_eq!(packed, vec![0b11_10_01_00]);
    }

    #[test]
    fn pack_4bit_values() {
        let packed = pack_subbyte_msb_first(&[0xA, 0xB, 0xC, 0xD], 4, 4, 1);
        assert_eq!(packed, vec![0xAB, 0xCD]);
    }

    #[test]
    fn pack_ignores_high_bits() {
        // For 1-bit, any nonzero-low-bit input maps to 1; but only the masked
        // low bit is stored. 0b10 has low bit 0 → stored as 0.
        let packed = pack_subbyte_msb_first(&[0b10, 0b01, 0b11, 0b00], 1, 4, 1);
        assert_eq!(packed, vec![0b0110_0000]);
    }

    #[test]
    fn packed_length_matches_row_padding() {
        // 5 samples/row at 1 bit = 1 byte/row; 3 rows = 3 bytes.
        let packed = pack_subbyte_msb_first(&[0; 15], 1, 5, 3);
        assert_eq!(packed.len(), 3);

        // 3 samples/row at 4 bits = ceil(12/8) = 2 bytes/row; 2 rows = 4 bytes.
        let packed = pack_subbyte_msb_first(&[0; 6], 4, 3, 2);
        assert_eq!(packed.len(), 4);
    }

    // ---- Property tests --------------------------------------------------

    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Round-trip: unpacking then repacking sub-byte data preserves the
        // packed bytes, and packing then unpacking preserves the sample values.
        // Mirrors JBP's `bilevel_round_trip` proptest but exercises row padding
        // and 2/4-bit widths.
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn unpack_pack_round_trip(
                bits in prop::sample::select(vec![1u8, 2u8, 4u8]),
                samples_per_row in 1u32..40,
                num_rows in 1u32..20,
            ) {
                let max = (1u16 << bits) as u8 - 1;
                let total = (samples_per_row * num_rows) as usize;
                // Generate random sample values in range for this width.
                let values: Vec<u8> = (0..total).map(|i| ((i as u32).wrapping_mul(2654435761) % (max as u32 + 1)) as u8).collect();

                let packed = pack_subbyte_msb_first(&values, bits, samples_per_row, num_rows);
                let unpacked = unpack_subbyte_msb_first(&packed, bits, samples_per_row, num_rows);
                prop_assert_eq!(unpacked, values);
            }

            #[test]
            fn pack_unpack_round_trip_from_values(
                bits in prop::sample::select(vec![1u8, 2u8, 4u8]),
                samples_per_row in 1u32..40,
                num_rows in 1u32..20,
                seed in any::<u64>(),
            ) {
                let max = (1u16 << bits) as u8 - 1;
                let total = (samples_per_row * num_rows) as usize;
                let mut state = seed;
                let values: Vec<u8> = (0..total).map(|_| {
                    // xorshift for deterministic pseudo-random values in range
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state % (max as u32 + 1) as u64) as u8
                }).collect();

                let packed = pack_subbyte_msb_first(&values, bits, samples_per_row, num_rows);
                // Packed length must match row-padded expectation.
                let row_bytes = super::super::bytes_per_row(bits, samples_per_row);
                prop_assert_eq!(packed.len(), row_bytes * num_rows as usize);

                let unpacked = unpack_subbyte_msb_first(&packed, bits, samples_per_row, num_rows);
                prop_assert_eq!(unpacked, values);
            }
        }
    }
}
