# JPEG Codec

**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/jpeg`

Decodes JPEG DCT streams into NumPy arrays. Handles interleave conversion and color space
transformation, producing band-sequential output regardless of the input interleave mode.

This codec is format-agnostic — it decodes any valid JPEG stream regardless of the source
container format.

## Configuration Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bits_per_pixel` | `int` | Yes | Bits per pixel: `8` or `12`. |
| `num_bands` | `int` | Yes | Number of bands (`1` for mono, `3` for RGB/YCbCr). |
| `block_width` | `int` | Yes | Block width in pixels. |
| `block_height` | `int` | Yes | Block height in pixels. |
| `imode` | `string` | Yes | Interleave mode: `"B"` (band interleaved by block), `"P"` (pixel interleaved), `"R"` (row interleaved), or `"S"` (band sequential). |
| `color_space` | `string` | Yes | JPEG color space: `"MONO"`, `"RGB"`, or `"YCbCr601"`. |

## Decoding Behavior

1. Decode the JPEG stream using libjpeg-turbo with the specified `color_space` and `bits_per_pixel`.
2. Convert the decoded pixels from the input `imode` to band-sequential (BSQ) format.
3. Return a NumPy ndarray with shape `(num_bands, block_height, block_width)` and `uint8` dtype (or `uint16` for 12-bit).

Encoding is not supported. Calling `encode()` raises `NotImplementedError`.

## Example Configuration

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/jpeg",
    "configuration": {
        "bits_per_pixel": 8,
        "num_bands": 3,
        "block_width": 256,
        "block_height": 256,
        "imode": "P",
        "color_space": "YCbCr601"
    }
}
```

## Python Class

`aws.osml.io.zarr_codecs.JpegCodec` — see [API Reference](../api/zarr-codecs.md).
