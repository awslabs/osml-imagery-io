# Working with Pixels

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
    image = dataset.get_asset("image_segment_0")
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

<!-- TODO: Image operations — cropping, resampling, mosaicking, etc. -->
