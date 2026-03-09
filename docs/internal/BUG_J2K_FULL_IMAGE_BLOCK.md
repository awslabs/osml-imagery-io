# Bug: JPEG 2000 get_block() Returns Full Image Instead of Block

## Status: FIXED

This bug has been fixed. The JPEG 2000 decoder now properly uses `opj_get_decoded_tile` to decode individual tiles, matching the NITF block grid to the native J2K tile grid per the BPJ2K01.20 profile.

## Summary

The JPEG 2000 (`J2KImageAssetProvider`) implementation of `get_block()` was ignoring the `block_row` and `block_col` parameters and returning the entire image at the requested resolution level instead of the specific block.

## Root Cause

The original implementation treated the entire J2K codestream as a single block (block 0,0) and rejected non-zero coordinates with `InvalidBlockCoordinates` error. Per the BPJ2K01.20 profile, NITF blocks (NPPBH/NPPBV) must match the native J2K tile grid, so the decoder should use OpenJPEG's tile-based decoding API.

## Fix Applied

1. Added `get_tile_info()` method to `J2KCodec` trait - parses the SIZ marker to get tile dimensions and grid size
2. Added `decode_tile()` method to `J2KCodec` trait - decodes a single tile by index using `opj_get_decoded_tile`
3. Updated `Jpeg2000BlockDecoder::decode_block()` to:
   - Query the tile grid from the codestream
   - Validate block coordinates against the actual tile grid
   - Calculate `tile_index = block_row * num_tiles_x + block_col`
   - Call `decode_tile()` instead of decoding the full image
4. Updated `has_block()` to check against the actual tile grid

## Files Modified

- `src/jbp/j2k/codec.rs` - Added `get_tile_info()` and `decode_tile()` trait methods
- `src/jbp/j2k/ffi.rs` - Added `get_decoded_tile()` FFI wrapper
- `src/jbp/j2k/openjpeg.rs` - Implemented `get_tile_info()` (SIZ marker parsing) and `decode_tile()`
- `src/jbp/j2k/decoder.rs` - Updated `Jpeg2000BlockDecoder` to use tile-based decoding

## Expected Behavior (Now Working)

`get_block(block_row, block_col, resolution_level)` returns only the pixels for the specified block, with dimensions up to `block_shape` (smaller for edge blocks).

## Example

```python
from aws.osml.io import IO, AssetType

with IO.open(["test_j2k.ntf"], "r") as dataset:
    image = dataset.get_asset("image_segment_0")
    print(f"Image shape: {image.image_shape}")      # (5000, 4000, 1)
    print(f"Block shape: {image.block_shape}")      # (2048, 2048, 1)
    print(f"Block grid: {image.block_grid_size}")   # (3, 2)
    
    # Now correctly returns individual blocks
    block = image.get_block(0, 0, resolution_level=0)
    print(f"Block (0,0): {block.shape}")  # (2048, 2048, 1) - Correct
    
    block = image.get_block(0, 1, resolution_level=0)
    print(f"Block (0,1): {block.shape}")  # (2048, 1952, 1) - Correct (edge block)
```
