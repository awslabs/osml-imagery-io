# Block Shape Bug Analysis

**STATUS: FIXED** - All changes have been implemented and verified.

This document describes a discrepancy between the documented API behavior and the actual implementation of `get_block()` in the Python bindings that has now been resolved.

## Summary

The `API_DESIGN.md` documentation stated that `get_block()` returns arrays in CHW format `(bands, rows, cols)`, but the implementation was returning HWC format `(rows, cols, bands)`. This has been fixed.

## Documentation vs Implementation

### What API_DESIGN.md Says

From the "Image Data Format: Band-Sequential (Channels First)" section:

> This library uses band-sequential (BSQ) ordering for image data, where NumPy arrays have shape `(bands, rows, cols)`. This is also known as "channels first" or CHW format.

Example from docs:
```python
block_r0 = image.get_block(0, 0, resolution_level=0)
print(block_r0.shape)  # (3, 2048, 2048)
```

### What the Implementation Returns

Actual output from test files:
```python
from aws.osml.io import IO

with IO.open(['data/unit/test_nc.ntf'], 'r') as dataset:
    image = dataset.get_asset('image_segment_0')
    block = image.get_block(0, 0, 0)
    print(block.shape)   # (256, 256, 3) - NOT (3, 256, 256)
    print(block.strides) # (768, 3, 1)   - HWC strides, not CHW
```

## Root Cause

The issue is in `src/bindings/image.rs` in the `create_numpy_array` function. Each pixel type branch contains a reshape call like:

```rust
let reshaped = array.reshape([rows, cols, bands])?;
```

All 8 reshape calls (one per `PixelType` variant: UInt8, Int8, UInt16, Int16, UInt32, Int32, Float32, Float64) use `[rows, cols, bands]` (HWC) instead of `[bands, rows, cols]` (CHW).

## Data Flow Analysis

| Component | Memory Layout | Shape Declaration |
|-----------|---------------|-------------------|
| API_DESIGN.md | Claims BSQ | Claims CHW `(bands, rows, cols)` |
| UncompressedBlockDecoder | Outputs BSQ | Returns `[rows, cols, bands]` |
| Jpeg2000BlockDecoder | Outputs BSQ | Returns `[rows, cols, bands]` |
| Python bindings (image.rs) | Reshapes to HWC | Declares `[rows, cols, bands]` |
| BufferedImageAssetProvider | Stores/returns BIP | Returns `[rows, cols, bands]` |

## Verification

The following test confirms the data is in BIP (band-interleaved-by-pixel) format:

```python
from aws.osml.io import IO

with IO.open(['data/unit/test_nc.ntf'], 'r') as dataset:
    image = dataset.get_asset('image_segment_0')
    block = image.get_block(0, 0, 0)
    
    # Check memory layout
    print(f'Shape: {block.shape}')     # (256, 256, 3)
    print(f'Strides: {block.strides}') # (768, 3, 1) - HWC strides
    
    # First pixel RGB values (light checkerboard tile)
    print(f'Pixel (0,0): {block[0, 0, :]}')  # [200, 180, 160]
    
    # First 9 bytes in memory
    flat = block.ravel()
    print(f'First 9 bytes: {flat[:9]}')
    # Output: [200, 180, 160, 200, 180, 160, 200, 180, 160]
    # This is BIP: [R0,G0,B0, R1,G1,B1, R2,G2,B2]
    # If BSQ, would be: [R0,R1,R2,R3,R4,R5,R6,R7,R8] (all band 0)
```

## Impact

1. **Documentation mismatch**: Users following the docs will write incorrect code
2. **Unnecessary conversion**: The Rust decoders output BSQ, but the binding layer effectively converts to BIP by declaring the wrong shape
3. **PyTorch incompatibility**: PyTorch expects CHW format; users must transpose

## Proposed Fix

~~Change all 8 reshape calls in `src/bindings/image.rs` `create_numpy_array` function from:~~
```rust
let reshaped = array.reshape([rows, cols, bands])?;
```

~~To:~~
```rust
let reshaped = array.reshape([bands, rows, cols])?;
```

**DONE** - All reshape calls have been updated.

This change:
- Matches the documented behavior
- Is zero-cost (just a shape change, no data movement since underlying data is already BSQ)
- Enables direct band indexing: `block[0]` = band 0
- Is compatible with PyTorch's expected format

## Additional Changes Required

~~If the fix is applied, these components also need updates:~~

All changes have been implemented:

1. **Rust trait** (`src/traits/image.rs`): ✅ Updated `get_block` return shape documentation from `[rows, cols, bands]` to `[bands, rows, cols]`

2. **`image_shape` property**: ✅ Now returns `(bands, rows, cols)`

3. **`block_shape` property**: ✅ Now returns `(bands, rows, cols)`

4. **BufferedImageAssetProvider** (`src/buffered/image.rs`): ✅ Updated to store BSQ format and return `[bands, rows, cols]`

5. **Block decoders** (`src/jbp/image/decoder.rs`, `src/jbp/j2k/decoder.rs`): ✅ Updated to return `[bands, rows, cols]`

6. **Block encoder** (`src/jbp/image/encoder.rs`): ✅ Updated shape validation and all test assertions

7. **Python bindings** (`src/bindings/image.rs`, `src/bindings/buffered_image.rs`): ✅ Updated `create_numpy_array` to correctly interpret CHW shape

8. **Test files**: ✅ All Rust and Python tests updated and passing (1116 Rust tests, 331 Python tests)
