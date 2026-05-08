# JBP Block Codec

**Version:** 1.0  
**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/jbp-block`  
**Codec type:** array-to-bytes  

Decodes uncompressed JBP/NITF/NSIF image blocks into NumPy arrays. Performs interleave
conversion (from the source IMODE to band-sequential) and big-endian to native-endian
byte swap.

This codec is NITF-specific — it uses NITF interleave modes (IMODE) and pixel value
types (PVTYPE) to interpret the raw pixel bytes.

## Document Conventions

The key words "MUST", "MUST NOT", "SHOULD", and "MAY" in this document are to be
interpreted as described in [RFC 2119][rfc2119].

## Codec Identifier

The value of the `name` member in the codec metadata MUST be
`https://awslabs.github.io/osml-imagery-io/codecs/jbp-block`.

## Encoded Representation

The encoded representation MUST be raw uncompressed pixel data as defined by
MIL-STD-2500C for NITF image data segments. The byte sequence contains pixel
values in big-endian byte order arranged according to the interleave mode
specified in the `imode` configuration parameter.

The total byte length MUST equal `num_bands * block_height * block_width * (nbpp / 8)`.

## Configuration Parameters

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `num_bands` | `int` | Yes | Number of bands. |
| `block_height` | `int` | Yes | Block height in pixels. |
| `block_width` | `int` | Yes | Block width in pixels. |
| `nbpp` | `int` | Yes | Bits per pixel per band (`8`, `16`, `32`, or `64`). |
| `imode` | `string` | Yes | NITF interleave mode: `"B"` (band interleaved by block), `"P"` (pixel interleaved), `"R"` (row interleaved), or `"S"` (band sequential). |
| `pvtype` | `string` | Yes | NITF pixel value type: `"INT"` (unsigned integer), `"SI"` (signed integer), `"R"` (real/float), or `"C"` (complex). |

### PVTYPE / NBPP to NumPy dtype Mapping

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

## Algorithm

### Decoding

1. Validate that the input data length equals `num_bands * block_height * block_width * (nbpp / 8)` bytes.
2. Convert the pixel data from the input `imode` to band-sequential (BSQ) format.
3. Swap big-endian bytes to native-endian byte order.
4. Return an array with shape `(num_bands, block_height, block_width)` and the dtype corresponding to the `pvtype`/`nbpp` combination.

### Encoding

Encoding is not currently specified. See [Implementation Notes](#implementation-notes).

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

## References

- [MIL-STD-2500C][mil-std-2500c] — National Imagery Transmission Format (NITF)
- [RFC 2119][rfc2119] — Key words for use in RFCs to Indicate Requirement Levels

[mil-std-2500c]: https://gwg.nga.mil/misb/docs/standards/MIL-STD-2500C.pdf
[rfc2119]: https://www.rfc-editor.org/rfc/rfc2119

## Implementation Notes

`aws.osml.io.zarr_codecs.JbpBlockCodec` — see [API Reference](../api/zarr-codecs.md).

Only the decode path is implemented. Calling `encode()` raises `NotImplementedError`.
