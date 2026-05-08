# JPEG Codec

**Version:** 1.0  
**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/jpeg`  
**Codec type:** array-to-bytes  

Decodes JPEG DCT streams into NumPy arrays. Handles interleave conversion and color space
transformation, producing band-sequential output regardless of the input interleave mode.

This codec is format-agnostic — it decodes any valid JPEG stream regardless of the source
container format.

## Document Conventions

The key words "MUST", "MUST NOT", "SHOULD", and "MAY" in this document are to be
interpreted as described in [RFC 2119][rfc2119].

## Codec Identifier

The value of the `name` member in the codec metadata MUST be
`https://awslabs.github.io/osml-imagery-io/codecs/jpeg`.

## Encoded Representation

The encoded representation MUST be a valid JPEG interchange format (JFIF) byte
sequence conforming to ITU-T T.81. The stream begins with an SOI marker (`0xFFD8`)
and ends with an EOI marker (`0xFFD9`).

The JPEG data MAY use 8-bit or 12-bit sample precision as indicated by the
`bits_per_pixel` configuration parameter.

## Configuration Parameters

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bits_per_pixel` | `int` | Yes | Bits per pixel: `8` or `12`. |
| `num_bands` | `int` | Yes | Number of bands (`1` for mono, `3` for RGB/YCbCr). |
| `block_width` | `int` | Yes | Block width in pixels. |
| `block_height` | `int` | Yes | Block height in pixels. |
| `imode` | `string` | Yes | Interleave mode: `"B"` (band interleaved by block), `"P"` (pixel interleaved), `"R"` (row interleaved), or `"S"` (band sequential). |
| `color_space` | `string` | Yes | JPEG color space: `"MONO"`, `"RGB"`, or `"YCbCr601"`. |

## Algorithm

### Decoding

1. Decode the JPEG stream using the specified `color_space` and `bits_per_pixel`.
2. Convert the decoded pixels from the input `imode` to band-sequential (BSQ) format.
3. Return an array with shape `(num_bands, block_height, block_width)` and `uint8` dtype (or `uint16` for 12-bit).

### Encoding

Encoding is not currently specified. See [Implementation Notes](#implementation-notes).

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

## References

- [ITU-T T.81][itu-t81] — Digital compression and coding of continuous-tone still images (JPEG)
- [RFC 2119][rfc2119] — Key words for use in RFCs to Indicate Requirement Levels

[itu-t81]: https://www.itu.int/rec/T-REC-T.81
[rfc2119]: https://www.rfc-editor.org/rfc/rfc2119

## Implementation Notes

`aws.osml.io.zarr_codecs.JpegCodec` — see [API Reference](../api/zarr-codecs.md).

Only the decode path is implemented. Calling `encode()` raises `NotImplementedError`.
