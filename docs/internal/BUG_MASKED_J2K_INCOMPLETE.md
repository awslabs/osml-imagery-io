# Bug: M8 (JPEG 2000 Masked) Implementation Incomplete

## Status

**Fixed** - M8 masked images now work correctly with per-block codestream encoding.

## Summary

The M8 (JPEG 2000 with mask) implementation has a critical bug in the writer that causes all block offsets to be set to 0, making it impossible to correctly decode individual blocks from masked J2K images.

## Background: How Masks Work with JPEG 2000

According to the JBP specification (sections 5.12.2, 5.13.3, 5.13.4):

1. The Image Data Mask table is placed at the beginning of the image data area
2. `IMDATOFF` (Image Data Offset) specifies the offset from the start of the mask table to the start of the blocked image data
3. `BMRnBNDm` (Block Mask Records) contain offsets from the start of the blocked image data to each block's data
4. For masked images, empty blocks have offset `0xFFFFFFFF` and are not stored
5. For M8 (JPEG 2000 with mask), each block should be stored as a separate J2K codestream at the offset specified in the mask table

## The Fix (Implemented)

The fix encodes each block as a separate single-tile J2K codestream for M8/MD masked images:

1. Instead of using a multi-tile encoder, create a new single-tile encoder for each provided block
2. Each block is encoded as a standalone J2K codestream with dimensions matching the block size
3. Track the byte offset where each codestream starts in the output buffer
4. Store those offsets in the mask table's `block_offsets` array
5. Concatenate all individual codestreams after the mask table

The key change in `src/jbp/writer.rs` (around line 1150):

```rust
// For M8/MD masked images, encode each block as a separate single-tile J2K codestream
let mut encoded_data = Vec::new();

for block_row in 0..grid_rows {
    for block_col in 0..grid_cols {
        if provided_blocks.contains(&(block_row, block_col)) {
            // Record the offset where this block's codestream starts
            mask.block_offsets[block_index] = encoded_data.len() as u32;
            
            // Create a single-tile encoder for this block
            let mut block_encoder = create_block_encoder(
                &encoding_ic,
                block_height,  // Single tile = block dimensions
                block_width,
                ...
            )?;
            
            // Encode the single block (tile 0,0 in this single-tile image)
            block_encoder.encode_block(0, 0, &tile_data, shape)?;
            
            // Finalize to get the codestream for this block
            let block_codestream = block_encoder.finalize()?;
            
            // Append the codestream
            encoded_data.extend_from_slice(&block_codestream);
        }
    }
}
```

## The Problem

In `src/jbp/writer.rs` (around line 1175), the M8 writer incorrectly sets all block offsets to 0:

```rust
// TODO: Implement proper per-tile offset tracking for M8/MD
for block_row in 0..grid_rows {
    for block_col in 0..grid_cols {
        let block_index = (block_row * grid_cols + block_col) as usize;
        if block_index < mask.block_offsets.len() {
            if provided_blocks.contains(&(block_row, block_col)) {
                mask.block_offsets[block_index] = 0;  // BUG: All blocks point to offset 0
            }
        }
    }
}
```

This is incorrect because:
- Each block should be encoded as a separate J2K codestream
- Each block's offset should point to where that specific codestream starts
- Currently all blocks point to offset 0, so only the first block can be decoded

## What Works

- **NM (Uncompressed Masked)**: Works correctly because it encodes blocks sequentially and tracks actual byte offsets
- **J2K Decoder**: The `decode_block_at_offset()` method is correctly implemented - it expects each block to be a separate codestream at the given offset

## What Needs to be Fixed

### 1. Writer (`src/jbp/writer.rs`)

For M8/MD masked images, change the encoding approach:
- Encode each provided block as a separate single-tile J2K codestream
- Track the byte offset where each codestream is written
- Store those offsets in the mask table's `block_offsets` array
- Concatenate all individual codestreams after the mask table

### 2. Encoder (`src/jbp/j2k/encoder.rs`)

The J2K encoder currently produces a single multi-tile codestream. For M8 masked images, it needs to either:
- Provide a method to encode a single tile and return its codestream bytes
- Or be called multiple times, once per block, producing separate codestreams

### Implementation Approach

Follow the same pattern as NM (uncompressed masked):

```rust
// Pseudocode for M8 fix
let mut encoded_data = Vec::new();

for block_row in 0..grid_rows {
    for block_col in 0..grid_cols {
        let block_index = (block_row * grid_cols + block_col) as usize;
        
        if provided_blocks.contains(&(block_row, block_col)) {
            // Record the offset where this block's codestream starts
            mask.block_offsets[block_index] = encoded_data.len() as u32;
            
            // Encode this single block as a J2K codestream
            let block_codestream = encode_single_block_j2k(block_row, block_col, ...)?;
            
            // Append the codestream
            encoded_data.extend_from_slice(&block_codestream);
        }
        // Masked blocks already have EMPTY_BLOCK_OFFSET
    }
}
```

## Files to Modify

1. `src/jbp/writer.rs` - Fix the M8/MD encoding logic
2. `src/jbp/j2k/encoder.rs` - Add single-block encoding capability
3. `src/jbp/j2k/mod.rs` - Expose new encoder functionality if needed

## Testing

After fixing, enable M8 tests in:
- `tests/property/test_masking.py` - Remove the `assume(False)` skip for M8
- `tests/property/test_roundtrip.py` - Enable M8 in `TestMaskedImageRoundtrip` if applicable

## Related

- **Spec**: `.kiro/specs/image-masking/` - Task 11 property tests currently skip M8
- **JBP Reference**: `reference-materials/JBP/Joint-BIIF-Profile-V2024.1_2024-01-18.pdf` sections 5.12.2, 5.13.3, 5.13.4

## TODO

- [x] Implement single-block J2K encoding in encoder
- [x] Fix writer to track per-block offsets for M8/MD
- [x] Enable M8 tests in `tests/property/test_masking.py`
- [ ] Update Task 11 in image-masking spec to mark M8 support as complete
