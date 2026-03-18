# Writing Imagery Assets

## Choosing the Output Format

This library supports writing imagery in multiple formats. NITF 2.1, NSIF 1.0, and
GeoTIFF are fully supported. When opening a file for writing you must pass the 
`format` parameter to `IO.open`. The format is not inferred from the file extension 
— you choose it explicitly:

```python
from aws.osml.io import IO

# Write a NITF 2.1 file
with IO.open(["output.ntf"], "w", "nitf") as writer:
    ...

# Write an NSIF 1.0 file
with IO.open(["output.nsf"], "w", "nsif") as writer:
    ...

# Write a GeoTIFF file
with IO.open(["output.tif"], "w", "geotiff") as writer:
    ...
```

The format flag is required because the file may not exist yet and the extension alone
is ambiguous — for example, `.ntf` could be NITF 2.1 or NSIF 1.0. Always specify the
format explicitly.

Accepted format strings:

| Format string | Output |
|---------------|--------|
| `"nitf"`, `"nitf21"`, `"nitf2.1"` | NITF 2.1 |
| `"nsif"`, `"nsif10"`, `"nsif1.0"` | NSIF 1.0 |
| `"geotiff"` | GeoTIFF |

For reading, `IO.open` auto-detects the format from the file extension (`.ntf`, `.nitf`,
`.nsf`, `.nsif`, `.tif`, `.tiff`) or from magic bytes in the file header. You can also
override detection with the `format` parameter.

## Metadata Controls Encoding

Metadata drives how the image is encoded. The `BufferedMetadataProvider` is a mutable,
in-memory key-value store that serves two roles:

1. Create metadata from scratch when building new images.
2. Copy and modify metadata from an existing image when transcoding or chipping.

The metadata values you set are highly dependent on the desired output format. A NITF
file needs fields like `IC`, `IMODE`, and `COMRAT`; a GeoTIFF file uses TIFF tags
and GeoKeys. The field names match what you see when reading files — no translation
layer sits between you and the format.

```python
from aws.osml.io import BufferedMetadataProvider

# Create from scratch
metadata = BufferedMetadataProvider()
metadata.set("IMODE", "B")
metadata.set("IC", "NC")

# Copy from an existing provider and modify
copied = BufferedMetadataProvider(source=existing_provider)
copied.set("IC", "C8")         # Switch to JPEG 2000
copied.set("COMRAT", "N001.0") # Lossless

# Query
value = metadata.get("IMODE")       # "B" or None
all_pairs = metadata.as_dict()
filtered = metadata.as_dict("NPP")  # {"NPPBH": "256", "NPPBV": "256"}
```

## Basic Write Workflow

Create a `BufferedImageAssetProvider` with tiled assets and a `BufferedMetadataProvider`
with encoding hints, then write through the `IO` interface. This example shows a
GeoTIFF workflow with Deflate compression and UTM georeferencing. The TIFF writer
expects numeric tag IDs as metadata keys (per the TIFF 6.0 specification). Use
`TagNameResolver` for convenient name-based access. `GeoModelType`,
`GeoRasterType`, `GeoProjectedCRS`, `GeoPixelScale`, and `GeoTiepoints` are derived
from GeoTIFF GeoKeys and coordinate transformation tags:

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
from aws.osml.io.tiff import TagNameResolver
import numpy as np

# Set up TIFF encoding hints using TagNameResolver for readable names
metadata = BufferedMetadataProvider()
tag_dict = metadata.as_dict()
resolver = TagNameResolver(tag_dict)
resolver["Compression"] = "Deflate"      # Tag 259: Deflate compression (LZW, None also supported)
resolver["TileWidth"] = "256"            # Tag 322: 256-pixel tile width
resolver["TileLength"] = "256"           # Tag 323: 256-pixel tile height
resolver["Predictor"] = "Horizontal"     # Tag 317: Horizontal differencing predictor
# Write resolved numeric keys back into the metadata provider
for key, value in tag_dict.items():
    metadata.set(key, str(value) if not isinstance(value, str) else value)

# GeoTIFF coordinate reference system (EPSG:32618 = WGS 84 / UTM zone 18N)
metadata.set("GeoModelType", "Projected")
metadata.set("GeoRasterType", "PixelIsArea")
metadata.set("GeoProjectedCRS", "32618")

# Pixel-to-model coordinate transformation
# ModelPixelScaleTag: [scale_x, scale_y, scale_z] in CRS units (meters for UTM)
metadata.set("GeoPixelScale", "[0.5, 0.5, 0.0]")
# ModelTiepointTag: [pixel_x, pixel_y, pixel_z, geo_x, geo_y, geo_z]
metadata.set("GeoTiepoints", "[0, 0, 0, 300000.0, 4500000.0, 0.0]")

# Create a tiled image asset
image_data = np.random.randint(0, 255, (3, 512, 512), dtype=np.uint8)

provider = BufferedImageAssetProvider.create(
    key="output_image",
    num_columns=512,
    num_rows=512,
    num_bands=3,
    block_width=256,
    block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)
provider.set_full_image(image_data)

# Write to disk
with IO.open(["output.tif"], "w", "geotiff") as writer:
    writer.add_asset("image_0", provider,
                     title="Synthetic RGB Image",
                     description="UTM-referenced test image",
                     roles=["data"])
```

The same pattern applies to NITF — only the metadata field names change. Where
GeoTIFF uses `Compression` and `TileWidth`, NITF uses `IC` and `NPPBH`. See the
[Format-Specific Encoding Options](#format-specific-encoding-options) section below
for the full set of fields per format.

## Copy-and-Modify Workflow

Read an existing file, modify metadata, and write a new file. This is the typical
pattern for transcoding, chipping, or re-compressing imagery.

The writer needs two kinds of metadata:

- **File metadata** — populates the file header (security markings, originator, etc.).
  Set this on the writer via `writer.metadata`.
- **Image metadata** — controls per-image encoding (compression, blocking, etc.).
  Attach this to the `BufferedImageAssetProvider`.

Both can be copied from the original file and selectively overridden:

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider

with IO.open(["input.ntf"], "r") as reader:
    image = reader.get_asset("image_segment_0")
    block = image.get_block(0, 0, resolution_level=0)

    # Copy file-level metadata from the original dataset
    file_metadata = BufferedMetadataProvider(source=reader.metadata)

    # Copy image-level metadata and override encoding hints
    image_metadata = BufferedMetadataProvider(source=image.metadata)
    image_metadata.set("IC", "C8")
    image_metadata.set("COMRAT", "00.5")
    image_metadata.set("J2K_DECOMPOSITION_LEVELS", "5")
    image_metadata.set("NPPBH", "1024")
    image_metadata.set("NPPBV", "1024")

    provider = BufferedImageAssetProvider.create(
        key="compressed",
        num_columns=image.num_columns,
        num_rows=image.num_rows,
        num_bands=image.num_bands,
        pixel_type=image.pixel_value_type,
        metadata=image_metadata,
    )
    provider.set_full_image(block)

with IO.open(["compressed.ntf"], "w", "nitf") as writer:
    writer.metadata = file_metadata
    writer.add_asset("image_0", provider,
                     title="Re-compressed image",
                     description="JPEG 2000 compressed copy",
                     roles=["data"])
```


## Format-Specific Encoding Options

### NITF

The NITF writer reads encoding hints from the asset's metadata. The `IC` field selects
the compression algorithm and determines which other fields are relevant. The `IMODE`
field controls how bands are interleaved within each block.

#### Uncompressed (IC=NC, NM)

No compression is applied. `NM` enables the blocked mask table, allowing sparse images
where some blocks contain no data.

| Field | Values | Description |
|-------|--------|-------------|
| IC | `NC` or `NM` | No compression / no compression with mask |
| IMODE | `B`, `P`, `R`, `S` | Band interleave mode |
| NPPBH | 1–8192 | Pixels per block horizontal |
| NPPBV | 1–8192 | Pixels per block vertical |

```python
metadata = BufferedMetadataProvider()
metadata.set("IC", "NC")
metadata.set("IMODE", "B")
metadata.set("NPPBH", "256")
metadata.set("NPPBV", "256")
```

For sparse images, use `NM` and only set the blocks that contain data. Missing blocks
are treated as masked:

```python
metadata = BufferedMetadataProvider()
metadata.set("IC", "NM")

provider = BufferedImageAssetProvider.create(
    key="sparse",
    num_columns=1024, num_rows=1024, num_bands=3,
    block_width=256, block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)

# Only populate blocks that have data
provider.set_block(0, 0, block_data)
provider.set_block(1, 2, block_data)
```

#### JPEG 2000 (IC=C8, M8, CD, MD)

JPEG 2000 compression. `C8` is standard J2K, `CD` is HTJ2K (Part 15) for faster
encode/decode. `M8` and `MD` are the masked variants that support sparse images.

| Field | Values | Description |
|-------|--------|-------------|
| IC | `C8`, `M8`, `CD`, `MD` | J2K / J2K masked / HTJ2K / HTJ2K masked |
| IMODE | `B` (required) | Band interleave mode. Must be `B` (block interleaved) for JPEG 2000 per BPJ2K01.20 |
| NPPBH | 1–8192 | Pixels per block horizontal |
| NPPBV | 1–8192 | Pixels per block vertical |
| NBPP | 1–38 | Number of bits per pixel. Must be 1–38 for JPEG 2000 per BPJ2K01.20 |
| ABPP | equals NBPP | Actual bits per pixel. Must equal NBPP for JPEG 2000 per BPJ2K01.20 |
| COMRAT | `Nnnn.n`, `Vnnn.n`, or `nn.n` | Compression ratio indicator representing bits-per-pixel-per-band. `Nnnn.n` = numerically lossless, where `nnn.n` is the achieved post-compression bpp (e.g. `N001.0`). `Vnnn.n` = visually lossless, where `nnn.n` is the target bpp (e.g. `V020.0`). `nn.n` = lossy target bpp (e.g. `01.0` for 1.0 bpp) |

##### JPEG 2000 Encoder Parameters (J2K_ fields)

The `J2K_` prefixed metadata fields are unique: they do not get written into the NITF
image subheader or any TRE. They exist only as encoding hints that guide the JPEG 2000
compression algorithm itself. When you read a NITF file, you will not see these fields
in the metadata — they are write-only parameters consumed by the encoder and discarded
afterward.

| Field | Values | Default | Description |
|-------|--------|---------|-------------|
| J2K_DECOMPOSITION_LEVELS | 1–32 | `5` | Wavelet decomposition levels (resolution levels) |
| J2K_QUALITY_LAYERS | 1–255 | `1` | Quality layers for progressive refinement |

HTJ2K mode (`IC=CD` or `MD`) is determined by the IC code — you do not set it
separately. Lossless vs lossy encoding and the compression ratio are derived from the
`COMRAT` field: `Nnnn.n` selects numerically lossless, `Vnnn.n` selects visually
lossless at the given bpp, and `nn.n` selects lossy at the target bpp.

```python
# Lossless JPEG 2000
metadata = BufferedMetadataProvider()
metadata.set("IC", "C8")
metadata.set("IMODE", "B")
metadata.set("COMRAT", "N001.0")
metadata.set("J2K_DECOMPOSITION_LEVELS", "5")
metadata.set("NPPBH", "1024")
metadata.set("NPPBV", "1024")

# Lossy JPEG 2000 at ~1.0 bpp (approximately 8:1 compression)
metadata = BufferedMetadataProvider()
metadata.set("IC", "C8")
metadata.set("IMODE", "B")
metadata.set("COMRAT", "01.0")
metadata.set("J2K_DECOMPOSITION_LEVELS", "5")
metadata.set("J2K_QUALITY_LAYERS", "1")
metadata.set("NPPBH", "1024")
metadata.set("NPPBV", "1024")

# HTJ2K (faster encode/decode)
metadata = BufferedMetadataProvider()
metadata.set("IC", "CD")
metadata.set("IMODE", "B")
metadata.set("COMRAT", "01.0")
metadata.set("NPPBH", "1024")
metadata.set("NPPBV", "1024")
```

#### JPEG DCT (IC=C3, M3)

JPEG DCT compression. `C3` is standard JPEG, `M3` is the masked variant. `I1` is
downsampled JPEG with a 2048×2048 dimension limit.

| Field | Values | Description |
|-------|--------|-------------|
| IC | `C3`, `M3`, `I1` | JPEG / JPEG masked / downsampled JPEG. `I1` is limited to images ≤ 2048×2048 |
| IMODE | `B`, `P`, `S` | Band interleave mode. `R` (row interleaved) is not supported for JPEG DCT |
| NPPBH | 1–8192 | Pixels per block horizontal |
| NPPBV | 1–8192 | Pixels per block vertical |
| NBPP | `8` | Number of bits per pixel. Must be 8 (8-bit pixels only) for JPEG DCT |
| COMRAT | `00.0`–`99.9` | Quality factor (default: 75.0) |

```python
# JPEG at quality 85
metadata = BufferedMetadataProvider()
metadata.set("IC", "C3")
metadata.set("IMODE", "B")
metadata.set("COMRAT", "85.0")
metadata.set("NPPBH", "256")
metadata.set("NPPBV", "256")
```

### GeoTIFF

The GeoTIFF writer reads encoding hints from the asset's metadata using numeric TIFF
tag IDs as keys. Use `TagNameResolver` for convenient name-based access. The writer
supports uncompressed, LZW, Deflate, and PackBits compression.

#### TIFF Encoding Hints

Standard TIFF tags that control how the image is stored on disk:

| Field | Values | Default | Description |
|-------|--------|---------|-------------|
| Compression | `None`, `LZW`, `Deflate` | `Deflate` | Compression algorithm |
| TileWidth | multiple of 16 | `256` | Tile width in pixels |
| TileHeight | multiple of 16 | `256` | Tile height in pixels |
| Predictor | `None`, `Horizontal` | `Horizontal` (for LZW/Deflate) | Differencing predictor |

#### GeoTIFF Metadata

GeoKeys and coordinate transformation tags from the OGC GeoTIFF 1.1 standard.
These control the georeferencing of the image:

| Field | Values | Description |
|-------|--------|-------------|
| GeoModelType | `Projected`, `Geographic` | GTModelTypeGeoKey — model coordinate type |
| GeoRasterType | `PixelIsArea`, `PixelIsPoint` | GTRasterTypeGeoKey — raster space interpretation |
| GeoProjectedCRS | EPSG code (e.g. `32618`) | ProjectedCRSGeoKey — projected CRS |
| GeoGeographicCRS | EPSG code (e.g. `4326`) | GeodeticCRSGeoKey — geographic CRS |
| GeoPixelScale | `[sx, sy, sz]` | ModelPixelScaleTag — pixel size in CRS units |
| GeoTiepoints | `[px, py, pz, gx, gy, gz]` | ModelTiepointTag — raster-to-model tie points |
| GeoTransform | `[ox, pw, rx, oy, ry, ph]` | 6-element affine transform (GDAL convention) |

```python
# Deflate-compressed GeoTIFF with UTM Zone 18N georeferencing
from aws.osml.io.tiff import TagNameResolver

metadata = BufferedMetadataProvider()
tag_dict = metadata.as_dict()
resolver = TagNameResolver(tag_dict)
resolver["Compression"] = "Deflate"   # Tag 259
resolver["TileWidth"] = "256"         # Tag 322
resolver["TileLength"] = "256"        # Tag 323
for key, value in tag_dict.items():
    metadata.set(key, str(value) if not isinstance(value, str) else value)
metadata.set("GeoModelType", "Projected")
metadata.set("GeoRasterType", "PixelIsArea")
metadata.set("GeoProjectedCRS", "32618")
metadata.set("GeoPixelScale", "[0.5, 0.5, 0.0]")
metadata.set("GeoTiepoints", "[0, 0, 0, 300000.0, 4500000.0, 0.0]")
```

## Example: NITF Chip with TRE Preservation

Create a NITF chip from an arbitrary pixel region — not just a single block — while
carrying forward all metadata including TREs. The chip region may span multiple
blocks, so you need to find the overlapping blocks, read each one, and assemble the
relevant portions into a single output array. An ICHIPB TRE is added to record where
the chip came from in the original image, which is required for downstream mensuration
and geopositioning tools to work correctly.

```python
import numpy as np
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider

# Define the chip region in pixel coordinates (column/row)
x_min, y_min = 100, 200   # top-left corner
x_max, y_max = 612, 456   # bottom-right corner (exclusive)

with IO.open(["input.ntf"], "r") as reader:
    image = reader.get_asset("image_segment_0")

    # Get image and block dimensions
    img_width = image.num_columns
    img_height = image.num_rows
    block_width = image.num_pixels_per_block_horizontal
    block_height = image.num_pixels_per_block_vertical

    # Handle non-blocked images (block size 0 means single block = full image)
    if block_width == 0:
        block_width = img_width
    if block_height == 0:
        block_height = img_height

    # Clamp bounds to image dimensions
    x_min = max(0, x_min)
    y_min = max(0, y_min)
    x_max = min(img_width, x_max)
    y_max = min(img_height, y_max)

    chip_width = x_max - x_min
    chip_height = y_max - y_min

    # Determine which blocks overlap the chip region
    block_col_start = x_min // block_width
    block_col_end = (x_max - 1) // block_width + 1
    block_row_start = y_min // block_height
    block_row_end = (y_max - 1) // block_height + 1

    # Allocate the output chip array (bands, height, width)
    dtype = np.dtype(image.pixel_value_type.to_numpy_dtype())
    chip = np.zeros((image.num_bands, chip_height, chip_width), dtype=dtype)

    # Read overlapping blocks and assemble the chip
    for block_row in range(block_row_start, block_row_end):
        for block_col in range(block_col_start, block_col_end):
            if not image.has_block(block_row, block_col, resolution_level=0):
                continue

            block = image.get_block(block_row, block_col, resolution_level=0)

            # Block's pixel coordinates in image space
            bx = block_col * block_width
            by = block_row * block_height

            # Overlap between this block and the chip region
            src_x0 = max(0, x_min - bx)
            src_y0 = max(0, y_min - by)
            src_x1 = min(block.shape[2], x_max - bx)
            src_y1 = min(block.shape[1], y_max - by)

            # Corresponding destination in the chip array
            dst_x0 = max(0, bx - x_min)
            dst_y0 = max(0, by - y_min)
            dst_x1 = dst_x0 + (src_x1 - src_x0)
            dst_y1 = dst_y0 + (src_y1 - src_y0)

            chip[:, dst_y0:dst_y1, dst_x0:dst_x1] = \
                block[:, src_y0:src_y1, src_x0:src_x1]

    # Preserve file-level metadata (security markings, originator, etc.)
    file_metadata = BufferedMetadataProvider(source=reader.metadata)

    # Preserve image-level metadata (TREs, etc.) and set chip encoding
    image_metadata = BufferedMetadataProvider(source=image.metadata)
    image_metadata.set("IC", "NC")
    image_metadata.set("IMODE", "B")

    # Add ICHIPB TRE to record the chip's origin in the full image.
    # This is required by STDI-0002 Vol 1 App B so that mensuration and
    # geopositioning tools can map chip coordinates back to the original image.
    #
    # Grid point layout (row, col):
    #   (1,1) = top-left       (1,2) = top-right
    #   (2,1) = bottom-left    (2,2) = bottom-right
    #
    # OP_ fields are the output product (chip) corner coordinates.
    # FI_ fields are the corresponding full image coordinates.
    # The .500 offset places the coordinate at the center of the pixel (grid
    # convention per ICHIPB spec Annex A).

    image_metadata.set("ICHIPB.XFRM_FLAG", "00")                # Non-dewarped imagery
    image_metadata.set("ICHIPB.SCALE_FACTOR", "0001.00000")      # Full resolution (R0)
    image_metadata.set("ICHIPB.ANAMRPH_CORR", "00")              # No anamorphic correction
    image_metadata.set("ICHIPB.SCANBLK_NUM", "00")               # No scan blocks

    # Output product corner coordinates (chip space)
    image_metadata.set("ICHIPB.OP_ROW_11", "00000000.500")                              # top-left row
    image_metadata.set("ICHIPB.OP_COL_11", "00000000.500")                              # top-left col
    image_metadata.set("ICHIPB.OP_ROW_12", "00000000.500")                              # top-right row
    image_metadata.set("ICHIPB.OP_COL_12", f"{chip_width - 1:08d}.500")                 # top-right col
    image_metadata.set("ICHIPB.OP_ROW_21", f"{chip_height - 1:08d}.500")                # bottom-left row
    image_metadata.set("ICHIPB.OP_COL_21", "00000000.500")                              # bottom-left col
    image_metadata.set("ICHIPB.OP_ROW_22", f"{chip_height - 1:08d}.500")                # bottom-right row
    image_metadata.set("ICHIPB.OP_COL_22", f"{chip_width - 1:08d}.500")                 # bottom-right col

    # Full image corner coordinates (where the chip came from)
    image_metadata.set("ICHIPB.FI_ROW_11", f"{y_min:08d}.500")                          # top-left row
    image_metadata.set("ICHIPB.FI_COL_11", f"{x_min:08d}.500")                          # top-left col
    image_metadata.set("ICHIPB.FI_ROW_12", f"{y_min:08d}.500")                          # top-right row
    image_metadata.set("ICHIPB.FI_COL_12", f"{x_max - 1:08d}.500")                      # top-right col
    image_metadata.set("ICHIPB.FI_ROW_21", f"{y_max - 1:08d}.500")                      # bottom-left row
    image_metadata.set("ICHIPB.FI_COL_21", f"{x_min:08d}.500")                          # bottom-left col
    image_metadata.set("ICHIPB.FI_ROW_22", f"{y_max - 1:08d}.500")                      # bottom-right row
    image_metadata.set("ICHIPB.FI_COL_22", f"{x_max - 1:08d}.500")                      # bottom-right col

    # Full image dimensions
    image_metadata.set("ICHIPB.FI_ROW", f"{img_height:08d}")
    image_metadata.set("ICHIPB.FI_COL", f"{img_width:08d}")

    provider = BufferedImageAssetProvider.create(
        key="chip",
        num_columns=chip_width,
        num_rows=chip_height,
        num_bands=image.num_bands,
        pixel_type=image.pixel_value_type,
        metadata=image_metadata,
    )
    provider.set_full_image(chip)

with IO.open(["chip.ntf"], "w", "nitf") as writer:
    writer.metadata = file_metadata
    writer.add_asset("image_0", provider,
                     title="Chipped image",
                     description="Region chip with ICHIPB provenance",
                     roles=["data"])
```

The ICHIPB fields follow the STDI-0002 Volume 1, Appendix B specification. The key
relationships:

| Field group | Purpose |
|-------------|---------|
| `ICHIPB.XFRM_FLAG` | `00` for non-dewarped (linear) imagery. Set to `01` if the image has been dewarped, in which case remaining fields are zero-filled. |
| `ICHIPB.SCALE_FACTOR` | Scale relative to full resolution R0. `0001.00000` = R0, `0002.00000` = R1, etc. |
| `ICHIPB.OP_ROW/COL_*` | The four corner grid points in the output chip's coordinate space. |
| `ICHIPB.FI_ROW/COL_*` | The same four corners mapped to the original full image coordinate space. |
| `ICHIPB.FI_ROW`, `ICHIPB.FI_COL` | Total rows and columns of the original full image (the extent to which the SDEs apply). |

The `.500` offset on all coordinates places the point at the center of the pixel,
following the ICHIPB grid convention (see Annex A of the ICHIPB spec). For a chip
starting at pixel `(100, 200)` in the full image, `ICHIPB.FI_COL_11` is
`00000100.500` and `ICHIPB.FI_ROW_11` is `00000200.500`.
