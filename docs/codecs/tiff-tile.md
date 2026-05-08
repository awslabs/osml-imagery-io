# TIFF Tile Codec

**Version:** 1.0  
**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile`  
**Codec type:** array-to-bytes  

Decodes compressed TIFF tiles into NumPy arrays. Supports LZW, JPEG, Deflate,
Adobe Deflate, PackBits, and uncompressed tiles, including horizontal
differencing predictors and YCbCr-to-RGB conversion for JPEG tiles.

## Document Conventions

The key words "MUST", "MUST NOT", "SHOULD", and "MAY" in this document are to be
interpreted as described in [RFC 2119][rfc2119].

## Codec Identifier

The value of the `name` member in the codec metadata MUST be
`https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile`.

## Encoded Representation

The encoded representation MUST be a single compressed TIFF tile as it appears
in the TIFF file's data area (the bytes referenced by a TileOffsets/TileByteCounts
entry). The compression format is determined by the `compression` configuration
parameter and MUST conform to the corresponding algorithm defined in TIFF
Revision 6.0 or the applicable TIFF Technical Note.

When `compression` is `7` (JPEG), the tile data MAY omit shared quantization and
Huffman tables, which MUST then be provided via the `jpeg_tables` configuration
parameter.

## Rationale: Why Compressed TIFF Tiles Need Metadata

Individual compressed tiles extracted from a TIFF file cannot be decoded in
isolation. The compressed tile bytes are an opaque payload — the decoder needs
IFD tag metadata (compression algorithm, predictor settings, photometric
interpretation, JPEG quantization tables, etc.) that lives in the file header,
not in the tile data itself. Without this metadata, the decoder cannot determine
how to decompress the bytes or interpret the resulting pixel values.

This codec solves the problem by storing the required IFD tag values in its
configuration. At decode time it constructs a minimal single-tile TIFF in
memory from the configuration and the compressed tile bytes, then hands it to
libtiff for decompression:

```{image} /_static/images/reconstructed-single-tile-tiff.png
:alt: Reconstruction of a minimal single-tile TIFF from IFD tag configuration + compressed tile bytes.
:width: 700px
:align: center
```

This approach delegates all decompression complexity (LZW, Deflate, JPEG,
predictor reversal, byte-order conversion, color space conversion) to libtiff
rather than reimplementing it. See the
[Synthetic Codestream Codec Pattern](../design/zarr-codec-design.md) design
document for further details.

## Configuration Parameters

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `compression` | `int` | No | `1` | TIFF compression tag value. See table below. |
| `bits_per_sample` | `int` | No | `8` | Bits per sample per band (`8`, `16`, `32`, or `64`). |
| `samples_per_pixel` | `int` | No | `1` | Number of bands. |
| `photometric` | `int` | No | `1` | Photometric interpretation. `0` = MinIsWhite, `1` = MinIsBlack, `2` = RGB, `6` = YCbCr. |
| `planar_config` | `int` | No | `1` | Planar configuration. `1` = chunky (interleaved), `2` = planar (separate). |
| `predictor` | `int` | No | `1` | Differencing predictor. `1` = none, `2` = horizontal differencing, `3` = floating-point predictor. |
| `tile_width` | `int` | No | `256` | Tile width in pixels. |
| `tile_height` | `int` | No | `256` | Tile height in pixels. |
| `sample_format` | `int` | No | `1` | Sample format. `1` = unsigned integer, `2` = signed integer, `3` = IEEE floating point. |
| `jpeg_tables` | `string` or `null` | No | `null` | Base64-encoded shared JPEG quantization and Huffman tables (TIFF tag 347). Required when `compression` is `7`. |

### Compression Tag Values

| Value | Name | Notes |
|-------|------|-------|
| `1` | None (uncompressed) | Raw tile bytes; still needs byte-order and planar conversion. |
| `5` | LZW | Supports horizontal differencing predictor (`predictor=2`). |
| `7` | JPEG | Requires `jpeg_tables`. Handles YCbCr-to-RGB conversion when `photometric=6`. |
| `8` | Deflate (zlib) | Supports horizontal differencing predictor. |
| `32773` | PackBits | Run-length encoding. |
| `32946` | Adobe Deflate | Equivalent to Deflate; legacy tag value. |

### Sample Format / Bits Per Sample to NumPy dtype Mapping

| Sample Format | Bits Per Sample | NumPy dtype |
|---------------|-----------------|-------------|
| `1` (uint) | 8 | `uint8` |
| `1` (uint) | 16 | `uint16` |
| `1` (uint) | 32 | `uint32` |
| `2` (int) | 8 | `int8` |
| `2` (int) | 16 | `int16` |
| `2` (int) | 32 | `int32` |
| `3` (float) | 32 | `float32` |
| `3` (float) | 64 | `float64` |

## Algorithm

### Decoding

1. Construct a minimal single-tile TIFF buffer in memory: an 8-byte TIFF header, an IFD containing the tag values from the codec configuration, and the compressed tile bytes appended after the IFD.
2. Open the buffer with libtiff's `TIFFClientOpen` using memory-backed I/O callbacks.
3. If `compression=7` (JPEG) and `photometric=6` (YCbCr), set `JPEGCOLORMODE_RGB` so libtiff performs YCbCr-to-RGB conversion during decode.
4. Call `TIFFReadEncodedTile(handle, 0, ...)` to decompress the tile. libtiff handles predictor reversal, byte-order conversion, and color space conversion internally.
5. If the decoded tile is smaller than the nominal tile dimensions (edge tile), pad with zeros to the full tile shape.
6. Convert from chunky (pixel-interleaved) to band-sequential (BSQ) format if `planar_config=1` and `samples_per_pixel > 1`.
7. Return an array with shape `(samples_per_pixel, tile_height, tile_width)` and the dtype corresponding to the `sample_format`/`bits_per_sample` combination.

### Encoding

Encoding is not currently specified. See [Implementation Notes](#implementation-notes).

## Example Configuration

### LZW with horizontal predictor

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile",
    "configuration": {
        "compression": 5,
        "bits_per_sample": 8,
        "samples_per_pixel": 3,
        "photometric": 2,
        "planar_config": 1,
        "predictor": 2,
        "tile_width": 256,
        "tile_height": 256,
        "sample_format": 1
    }
}
```

### JPEG with shared tables

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile",
    "configuration": {
        "compression": 7,
        "bits_per_sample": 8,
        "samples_per_pixel": 3,
        "photometric": 6,
        "planar_config": 1,
        "predictor": 1,
        "tile_width": 256,
        "tile_height": 256,
        "sample_format": 1,
        "jpeg_tables": "base64:...encoded JPEGTables tag bytes..."
    }
}
```

## References

- [TIFF Revision 6.0][tiff6] — Tag Image File Format Specification
- [TIFF Technical Note #2][tiff-tn2] — TIFF Trees (JPEG-in-TIFF)
- [RFC 2119][rfc2119] — Key words for use in RFCs to Indicate Requirement Levels

[tiff6]: https://www.itu.int/itudoc/itu-t/com16/tiff-fx/docs/tiff6.pdf
[tiff-tn2]: https://www.awaresystems.be/imaging/tiff/specification/TIFFTechNote2.txt
[rfc2119]: https://www.rfc-editor.org/rfc/rfc2119

## Implementation Notes

`aws.osml.io.zarr_codecs.TiffTileCodec` — see [API Reference](../api/zarr-codecs.md).

Only the decode path is implemented. Calling `encode()` raises `NotImplementedError`.
