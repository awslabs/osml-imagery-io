# JBP Block Codec

**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/jbp-block`

Decodes uncompressed JBP/NITF/NSIF image blocks into NumPy arrays. Performs interleave
conversion (from the source IMODE to band-sequential) and big-endian to native-endian
byte swap.

This codec is NITF-specific — it uses NITF interleave modes (IMODE) and pixel value
types (PVTYPE) to interpret the raw pixel bytes.

## Configuration Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `num_bands` | `int` | Yes | Number of bands. |
| `block_height` | `int` | Yes | Block height in pixels. |
| `block_width` | `int` | Yes | Block width in pixels. |
| `nbpp` | `int` | Yes | Bits per pixel per band (`8`, `16`, `32`, or `64`). |
| `imode` | `string` | Yes | NITF interleave mode: `"B"` (band interleaved by block), `"P"` (pixel interleaved), `"R"` (row interleaved), or `"S"` (band sequential). |
| `pvtype` | `string` | Yes | NITF pixel value type: `"INT"` (unsigned integer), `"SI"` (signed integer), `"R"` (real/float), or `"C"` (complex). |

### PVTYPE / NBPP → NumPy dtype Mapping

| PVTYPE | NBPP | NumPy dtype |
|--------|------|-------------|
| `INT` | 8 | `uint8` |
| `INT` | 16 | `uint16` |
| `INT` | 32 | `uint32` |
| `SI` | 8 | `int8` |
| `SI` | 16 | `int16` |
| `SI` | 32 | `int32` |
| `R` | 32 | `float32` |
| `R` | 64 | `float64` |

## Decoding Behavior

1. Validate that the input data length matches the expected size: `num_bands × block_height × block_width × (nbpp / 8)` bytes.
2. Convert the pixel data from the input `imode` to band-sequential (BSQ) format.
3. Swap big-endian bytes to native-endian byte order.
4. Return a NumPy ndarray with shape `(num_bands, block_height, block_width)` and the dtype corresponding to the `pvtype`/`nbpp` combination.

Encoding is not supported. Calling `encode()` raises `NotImplementedError`.

## Example Configuration

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/jbp-block",
    "configuration": {
        "num_bands": 3,
        "block_height": 256,
        "block_width": 256,
        "nbpp": 16,
        "imode": "P",
        "pvtype": "INT"
    }
}
```

## Python Class

`aws.osml.io.zarr_codecs.JbpBlockCodec` — see [API Reference](../api/zarr-codecs.md).
