# Working with Pixels

## The Simple Path

For most pixel workflows, the convenience functions give you NumPy arrays
directly without thinking about blocks or assets:

```python
from aws.osml.io import imread, tiles

# Full image as a CHW NumPy array
pixels = imread("image.ntf")
print(pixels.shape)  # (3, 1024, 1024) — (bands, height, width)
print(pixels.dtype)  # uint8

# Process a large image in tiles without loading it all into memory
for tile in tiles("large_image.tif", tile_size=(256, 256)):
    process(tile.data)  # tile.data is a CHW NumPy array
```

The sections below cover the details of how pixel data is represented, how to
convert between channel orderings for different libraries, and how to work with
in-memory image buffers for writing.

## Image Data Arrays

Block data is returned as a [NumPy](https://numpy.org/) `ndarray`. NumPy is the
standard array interface shared by machine learning frameworks (PyTorch, TensorFlow),
computer vision libraries (OpenCV, Pillow), and the broader scientific Python ecosystem.
Returning pixel data as an ndarray means you can pass blocks directly into these tools
without an intermediate copy or conversion step.

For pixels wider than 8 bits (e.g. 16-bit or 32-bit imagery), the library automatically
converts from the format's stored byte order to the native byte order of your platform.
NITF files store multi-byte values in big-endian order; on a little-endian machine the
bytes are swapped during decode so the resulting ndarray is ready to use without manual
conversion. The NumPy dtype is selected automatically based on the image's pixel
type — an 8-bit unsigned image produces a `uint8` array, a 16-bit signed image produces
`int16`, a 32-bit float produces `float32`, and so on.

Each array has shape `(bands, rows, cols)` — a channels-first (CHW) layout. This
matches the convention used by PyTorch and many deep learning pipelines, where a batch
of images is shaped `(N, C, H, W)`. Channels-first ordering is also convenient for
remote sensing workflows where analysis steps typically operate on a subset of spectral
bands.

Other libraries expect different channel orderings:

| Library | Format | Shape |
|---------|--------|-------|
| osml-imagery-io | Channels First (CHW) | `(bands, rows, cols)` |
| PyTorch | Channels First (NCHW) | `(batch, channels, height, width)` |
| OpenCV | Channels Last (HWC) | `(rows, cols, channels)` |
| Pillow | Channels Last (HWC) | `(height, width, channels)` |

Use `np.transpose` to reshape a block for channels-last libraries:

```python
import numpy as np
import matplotlib.pyplot as plt
from PIL import Image
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    image = dataset.get_asset("image:0")
    block_chw = image.get_block(0, 0, resolution_level=0)

    # Convert to channels-last for display
    block_hwc = np.transpose(block_chw, (1, 2, 0))

    # Display with matplotlib
    plt.imshow(block_hwc)
    plt.title("Block (0, 0)")
    plt.show()

    # Or convert to a PIL Image for further manipulation
    pil_image = Image.fromarray(block_hwc)
```

## Creating an Image from Scratch

`BufferedImageAssetProvider` and `BufferedMetadataProvider` let you build images and
their associated metadata entirely in memory. `BufferedMetadataProvider` is a mutable
key-value store for encoding hints and format fields — things like compression type
(`IC`) and interleave mode (`IMODE`). `BufferedImageAssetProvider` holds the pixel data
and implements the same `ImageAssetProvider` interface used by file-backed images, so
in-memory images can be passed to any API that accepts an image asset, including the
writer. This is useful for creating synthetic test data, assembling mosaics, or
building images from processed results.

You can populate the image all at once with `set_full_image()`:

```python
from aws.osml.io import BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
import numpy as np

metadata = BufferedMetadataProvider()
metadata.set("IC", "NC")
metadata.set("IMODE", "B")

image_data = np.random.randint(0, 255, (3, 512, 512), dtype=np.uint8)

provider = BufferedImageAssetProvider.create(
    key="synthetic_image",
    num_columns=512,
    num_rows=512,
    num_bands=3,
    block_width=256,
    block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)
provider.set_full_image(image_data)
```

For large images or sparse data, set blocks individually with `set_block()` instead of
loading the full image into memory:

```python
provider = BufferedImageAssetProvider.create(
    key="tiled_image",
    num_columns=1024,
    num_rows=1024,
    num_bands=3,
    block_width=256,
    block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)

for row in range(4):
    for col in range(4):
        block = np.random.randint(0, 255, (3, 256, 256), dtype=np.uint8)
        provider.set_block(row, col, block)
```

## JPEG Color Space Handling in TIFF

TIFF files compressed with JPEG (compression code 7) store RGB pixel data internally
as YCbCr. This is a requirement of the JPEG-in-TIFF specification (TIFF Technical
Note 2) — libtiff sets `PhotometricInterpretation` to YCbCr (6) for images with 3 or
more bands and performs the RGB-to-YCbCr conversion automatically during encoding.

On the read side, libtiff converts YCbCr back to RGB during decoding. The pixel data
returned by `get_block()` is always RGB, and the `PhotometricInterpretation` reported
in metadata reflects the decoded color space (RGB), not the on-disk storage format.
Callers never need to handle YCbCr data directly.

On the write side, callers provide standard RGB pixel data and select JPEG compression
by setting encoding hint `"259"` to `7`. libtiff handles the RGB-to-YCbCr conversion
as part of JPEG encoding. JPEG quality is configurable via encoding hint `"65537"`
(values 1–100, default 75).

For single-band (grayscale) images, JPEG compression uses `PhotometricInterpretation`
MinIsBlack (1) and no color space conversion occurs.

The YCbCr conversion is an internal codec detail, similar to JPEG quantization. It is
lossy — writing and reading back a JPEG-compressed TIFF will not produce an exact pixel
match. Use PSNR or similar metrics to evaluate fidelity rather than exact comparison.

```python
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider, PixelType
import numpy as np

# Write a JPEG-compressed TIFF
metadata = BufferedMetadataProvider()
metadata.set_json("259", 7)       # JPEG compression
metadata.set_json("65537", 85)    # Quality 85

image_data = np.random.randint(0, 255, (3, 256, 256), dtype=np.uint8)
provider = BufferedImageAssetProvider.create(
    key="rgb_image",
    num_columns=256,
    num_rows=256,
    num_bands=3,
    block_width=256,
    block_height=256,
    pixel_type=PixelType.UInt8,
    metadata=metadata,
)
provider.set_full_image(image_data)

with IO.open(["output.tif"], "w", "tiff") as writer:
    writer.add_asset(provider)

# Read it back — pixels are RGB, not YCbCr
with IO.open(["output.tif"], "r") as reader:
    image = reader.get_asset("image:0")
    block = image.get_block(0, 0, resolution_level=0)
    print(block.dtype)   # uint8
    print(block.shape)   # (3, 256, 256) — RGB channels
```

## Indexed (Palette Color) Images

Some image formats store pixel values as indices into a color lookup table rather than
direct color values. Both TIFF and NITF support this concept, though the mechanisms
differ. In all cases, `ImageAssetProvider` returns the raw index values as stored in
the file — it does not apply lookup tables automatically.

The library does not perform palette expansion because many workflows need the raw
indices. Classification maps and thematic rasters use each index to represent a land
cover class or category, not a display color. Applying the lookup table to produce
RGB pixels is a separate processing step.

### TIFF Palette Color

In TIFF files, palette color is indicated by `PhotometricInterpretation = 3`. Each
pixel is a single-byte index and the actual RGB colors are defined in a separate
`ColorMap` tag. A palette-color TIFF will report 1 band of `uint8` data, and
`get_block()` returns the index array — not the expanded RGB pixels.

```python
from aws.osml.io import IO

with IO.open(["indexed_image.tif"], "r") as dataset:
    image = dataset.get_asset("image:0")
    print(image.num_bands)          # 1
    print(image.pixel_value_type)   # PixelType.UInt8

    block = image.get_block(0, 0, resolution_level=0)
    print(block.shape)              # (1, rows, cols) — index values, not RGB
```

### NITF Lookup Tables (LUTs)

NITF files support per-band lookup tables through the image subheader fields `NLUTSn`,
`NELUTn`, and `LUTDnm` (JBP §5.13.2.28–5.13.2.30). The most common case is
`IREP=RGB/LUT`: a single-band image where each pixel is an index and three LUTs
(red, green, blue) define the color mapping. Individual bands in `IREP=MONO` or
`IREP=MULTI` images can also carry LUTs when `IREPBANDn=LU`.

LUTs are only valid for uncompressed images (`IC=NC` or `NM`) with integer or binary
pixel types (`PVTYPE=INT` or `B`). For compressed formats like JPEG (`IC=C3`) and
JPEG 2000 (`IC=C8`), color handling is internal to the codec — the decoder outputs
final pixel values directly and `NLUTSn` is always 0. Vector Quantization (`IC=C4`)
uses its own codebook-based color lookup mechanism defined in MIL-STD-188-199, which
is separate from the subheader LUT fields.

As with TIFF, `get_block()` returns the raw stored values. For an `IREP=RGB/LUT`
image, this means 1 band of index values. The LUT data itself is accessible through
the image segment's metadata — the subheader fields `NLUTSn`, `NELUTn`, and `LUTDnm`
are parsed and available for applications that need to perform the lookup.

<!-- TODO: Image operations — cropping, resampling, mosaicking, etc. -->
