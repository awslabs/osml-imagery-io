# DatasetWriter Encoding Hints

## Problem Statement

The current API has format-specific parameters (NITF's `imode`, `irep`) leaking into abstract interfaces:

```python
# Current problematic API - NITF concepts in generic provider
provider = BufferedImageAssetProvider.create(
    key="image_0",
    num_columns=512,
    num_rows=512,
    imode="B",  # NITF-specific!
)
```

This prevents the library from cleanly supporting multiple output formats (NITF, GeoTIFF, etc.).

## Solution: Encoding Hints via Asset Metadata

Use the existing `MetadataProvider` interface on assets to pass encoding hints. The writer reads format-specific hints from `asset.metadata()` using the exact same field names that come from parsing files.

```python
# Create metadata with encoding hints
metadata = BufferedMetadataProvider()
metadata.set("IMODE", "P")      # Same field name as from reader
metadata.set("IC", "C8")
metadata.set("NPPBH", "256")

# Pass to provider
provider = BufferedImageAssetProvider.create(
    key="image_0",
    num_columns=512,
    num_rows=512,
    metadata=metadata,
    ...
)

# Writer reads hints from asset.metadata()
```

### Why This Approach

1. No breaking changes to trait signatures
2. Leverages existing `MetadataProvider` infrastructure
3. Metadata already flows from assets to writers
4. Users can copy metadata from reader → writer directly
5. No mental translation between namespaced keys and actual field names
6. The writer knows what format it's writing, so it knows which fields to look for

## Format-Specific Encoding Options

### NITF Blocking & Compression

| Field | Values | Description |
|-------|--------|-------------|
| IMODE | B, P, R, S | Band interleaved by block, pixel, row, or sequential |
| IC | NC, NM, C1, C3, C4, C5, C6, C7, C8, M1, M3, M4, M5, M8, I1 | Compression codes |
| NPPBH | 1-8192 | Pixels per block horizontal |
| NPPBV | 1-8192 | Pixels per block vertical |
| COMRAT | varies | Compression ratio (for compressed images) |

### JPEG 2000 Encoding Hints (IC=C8 or IC=CD)

When writing JPEG 2000 compressed imagery, use COMRAT to control compression mode and ratio, plus additional hints for encoder parameters not covered by COMRAT:

| Field | Values | Description |
|-------|--------|-------------|
| COMRAT | Nnnn.n, Vnnn.n, nn.n | Compression ratio - controls lossless/lossy and quality |
| J2K_DECOMPOSITION_LEVELS | 1-32 (default: 5) | Number of wavelet decomposition levels (resolution pyramid depth) |
| J2K_QUALITY_LAYERS | 1-65535 (default: 1) | Number of quality layers (progressive decoding) |

**COMRAT Format:**
- `Nnnn.n` - Numerically lossless (e.g., "N001.0")
- `Vnnn.n` - Visually lossless with quality factor (e.g., "V020.0")
- `nn.n` - Target bits per pixel for lossy (e.g., "01.0" = 1.0 bpp, "00.5" = 0.5 bpp)

**Example - Lossy JPEG 2000 (target 1.0 bpp):**
```python
metadata = BufferedMetadataProvider()
metadata.set("IC", "C8")                      # JPEG 2000 Part 1
metadata.set("COMRAT", "01.0")                # Target 1.0 bpp (~8:1 compression)
metadata.set("J2K_DECOMPOSITION_LEVELS", "5") # 5 resolution levels
metadata.set("NPPBH", "1024")                 # Tile size
metadata.set("NPPBV", "1024")
```

**Example - Lossless JPEG 2000:**
```python
metadata = BufferedMetadataProvider()
metadata.set("IC", "C8")
metadata.set("COMRAT", "N001.0")              # Numerically lossless
metadata.set("J2K_DECOMPOSITION_LEVELS", "6")
```

**Example - HTJ2K (High-Throughput JPEG 2000):**
```python
metadata = BufferedMetadataProvider()
metadata.set("IC", "CD")                      # HTJ2K
metadata.set("COMRAT", "00.8")                # Target 0.8 bpp (~10:1 compression)
```

### GeoTIFF Blocking & Compression (Future)

| Field | Values | Description |
|-------|--------|-------------|
| PlanarConfiguration | 1 (chunky), 2 (planar) | Band interleaving |
| Compression | 1 (none), 5 (LZW), 7 (JPEG), 8 (Deflate), 34712 (JPEG2000) | Compression type |
| TileWidth | 16-65535 | Tile width in pixels |
| TileLength | 16-65535 | Tile height in pixels |
| Predictor | 1 (none), 2 (horizontal), 3 (floating point) | Compression predictor |

## Complete Workflow Example

Read a NITF, extract a chip, and write with new encoding options:

```python
from aws.osml.io import IO, BufferedMetadataProvider, BufferedImageAssetProvider
import numpy as np

# 1. Read a NITF file
with IO.open("input.ntf", "r") as reader:
    image_asset = reader.get_asset("image_segment_0")
    
    # Get original metadata - these are the EXACT field names from parsing
    original_meta = image_asset.get_metadata().as_dict()
    # Example output:
    # {
    #     "IM": "IM",
    #     "IID1": "JITC TEST",
    #     "IID2": "JITC TEST DATA",
    #     "IDATIM": "20170720184039",
    #     "IMODE": "B",
    #     "IC": "NC",
    #     "NPPBH": "1024",
    #     "NPPBV": "1024",
    #     "IREP": "RGB",
    #     "PVTYPE": "INT",
    #     "NBPP": "8",
    #     "ABPP": "8",
    #     ...
    # }
    
    # 2. Read pixels and apply a chip operation
    full_image = image_asset.get_pixels()
    chip = full_image[100:612, 200:712, :]  # 512x512 chip
    
    # 3. Create metadata with encoding hints
    # Use the SAME field names that come from parsing (no namespaces)
    metadata = BufferedMetadataProvider()
    
    # Copy over metadata we want to preserve from original
    metadata.set("IID1", original_meta.get("IID1", ""))
    metadata.set("IID2", f"Chip from {original_meta.get('IID2', '')}")
    metadata.set("IREP", original_meta.get("IREP", "RGB"))
    
    # Override blocking/compression for the output
    metadata.set("IMODE", "P")       # Change: pixel interleaved (was "B")
    metadata.set("IC", "C8")         # Change: JPEG 2000 (was "NC")
    metadata.set("COMRAT", "V020")   # Add: visually lossless ratio
    metadata.set("NPPBH", "256")     # Change: smaller blocks (was "1024")
    metadata.set("NPPBV", "256")
    
    # 4. Create provider with metadata
    provider = BufferedImageAssetProvider.create(
        key="chipped_image",
        num_columns=512,
        num_rows=512,
        num_bands=chip.shape[2],
        pixel_type=image_asset.pixel_value_type,
        metadata=metadata,  # Encoding hints flow through metadata
    )
    provider.set_pixels(chip)

# 5. Write - writer reads hints from provider.metadata()
with IO.open("output_chip.ntf", "w") as writer:
    writer.add_asset("image_0", provider)
    writer.close()
```

## Helper Function Example

```python
from aws.osml.io import IO, BufferedMetadataProvider, BufferedImageAssetProvider

def chip_and_recompress(
    input_path: str,
    output_path: str,
    chip_bounds: tuple,  # (row_start, row_end, col_start, col_end)
    new_imode: str = None,
    new_ic: str = None,
    new_block_size: tuple = None,
):
    """
    Read a NITF, extract a chip, and write with new encoding options.
    
    Args:
        input_path: Source NITF file
        output_path: Destination file
        chip_bounds: (row_start, row_end, col_start, col_end)
        new_imode: Override IMODE (B/P/R/S) or None to keep original
        new_ic: Override IC (NC/C8/etc) or None to keep original
        new_block_size: Override (NPPBH, NPPBV) or None to keep original
    """
    r0, r1, c0, c1 = chip_bounds
    
    with IO.open(input_path, "r") as reader:
        image_asset = reader.get_asset("image_segment_0")
        original_meta = image_asset.get_metadata().as_dict()
        
        # Extract chip
        full_image = image_asset.get_pixels()
        chip = full_image[r0:r1, c0:c1, :]
        chip_rows, chip_cols, chip_bands = chip.shape
        
        # Build output metadata - start with relevant original fields
        metadata = BufferedMetadataProvider()
        
        # Preserve descriptive metadata
        for field in ["IID1", "IID2", "TGTID", "ISORCE", "ICAT", "IREP", "PVTYPE"]:
            if field in original_meta:
                metadata.set(field, original_meta[field])
        
        # Set encoding options - use overrides or fall back to original
        metadata.set("IMODE", new_imode or original_meta.get("IMODE", "B"))
        metadata.set("IC", new_ic or original_meta.get("IC", "NC"))
        
        if new_block_size:
            metadata.set("NPPBH", str(new_block_size[0]))
            metadata.set("NPPBV", str(new_block_size[1]))
        else:
            # Keep original or default to image size (single block)
            metadata.set("NPPBH", original_meta.get("NPPBH", str(chip_cols)))
            metadata.set("NPPBV", original_meta.get("NPPBV", str(chip_rows)))
        
        # Add compression ratio if using JPEG 2000
        if (new_ic or original_meta.get("IC", "NC")) in ["C8", "M8"]:
            metadata.set("COMRAT", "V020")  # Visually lossless
        
        # Create provider
        provider = BufferedImageAssetProvider.create(
            key="chip_0",
            num_columns=chip_cols,
            num_rows=chip_rows,
            num_bands=chip_bands,
            pixel_type=image_asset.pixel_value_type,
            metadata=metadata,
        )
        provider.set_pixels(chip)
    
    # Write output
    with IO.open(output_path, "w") as writer:
        writer.add_asset("image_0", provider)
        writer.close()


# Example usage:

# Simple chip, keep original encoding
chip_and_recompress(
    "large_image.ntf",
    "chip_original_encoding.ntf",
    chip_bounds=(1000, 2024, 2000, 3024),
)

# Chip with pixel interleaving and smaller blocks
chip_and_recompress(
    "large_image.ntf",
    "chip_pixel_interleaved.ntf",
    chip_bounds=(1000, 2024, 2000, 3024),
    new_imode="P",
    new_block_size=(256, 256),
)

# Chip with JPEG 2000 compression
chip_and_recompress(
    "large_image.ntf",
    "chip_compressed.ntf",
    chip_bounds=(1000, 2024, 2000, 3024),
    new_ic="C8",
    new_block_size=(512, 512),
)

# Chip with band-sequential layout (good for spectral analysis)
chip_and_recompress(
    "multispectral.ntf",
    "chip_band_sequential.ntf",
    chip_bounds=(0, 1024, 0, 1024),
    new_imode="S",
)
```

## Implementation Notes

### Writer Behavior

The writer should:
1. Call `asset.metadata().as_dict(None)` to get all metadata fields
2. Look for known encoding hint fields (IMODE, IC, NPPBH, etc.)
3. Use hint values if present, otherwise use sensible defaults
4. Validate hint values at write time

### Validation

Encoding hints should be validated when the writer processes them:
- Invalid IMODE values → error
- IC values requiring unavailable codecs → error  
- Block sizes larger than image dimensions → warning, auto-adjust

### Conflicts

If metadata hints conflict with provider properties:
- Provider properties (num_bands, pixel_type) take precedence for structural values
- Metadata hints take precedence for encoding choices
- Example: provider has 3 bands but metadata says `IREP: "MONO"` → use provider's band count, warn about IREP mismatch

## Next Steps

- [x] Implement `BufferedMetadataProvider` helper class
- [x] Update `JBPDatasetWriter` to read encoding hints from metadata
- [x] Add validation for encoding hint values
- [x] Document supported hints per format (NITF blocking, JPEG 2000)
- [x] Remove `imode` parameter from `BufferedImageAssetProvider.create()`
