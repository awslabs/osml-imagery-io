# TIFF Tile Codec

**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile`

Decodes compressed TIFF tiles into NumPy arrays. Supports LZW, JPEG, Deflate,
Adobe Deflate, PackBits, and uncompressed tiles, including horizontal
differencing predictors and YCbCr-to-RGB conversion for JPEG tiles.

Individual compressed tiles extracted from a TIFF file cannot be decoded in
isolation — the decoder needs IFD tag metadata (compression type, predictor,
photometric interpretation, JPEG tables, etc.) that lives in the file header,
not in the tile data itself. This codec stores the required IFD tag values in
its configuration. At decode time it constructs a minimal single-tile TIFF in
memory from the configuration and the compressed tile bytes, then hands it to
libtiff for decompression. See the
[Synthetic Codestream Codec Pattern](../design/zarr-codec-design.md) design
document for details on this approach.

## Configuration Schema

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `compression` | `int` | No | `1` | TIFF compression tag value. Supported: `1` (None), `5` (LZW), `7` (JPEG), `8` (Deflate), `32773` (PackBits), `32946` (Adobe Deflate). |
| `bits_per_sample` | `int` | No | `8` | Bits per sample per band (`8`, `16`, `32`, or `64`). |
| `samples_per_pixel` | `int` | No | `1` | Number of bands. |
| `photometric` | `int` | No | `1` | Photometric interpretation. `0` = MinIsWhite, `1` = MinIsBlack, `2` = RGB, `6` = YCbCr. |
| `planar_config` | `int` | No | `1` | Planar configuration. `1` = chunky (interleaved), `2` = planar (separate). |
| `predictor` | `int` | No | `1` | Differencing predictor. `1` = none, `2` = horizontal differencing, `3` = floating-point predictor. |
| `tile_width` | `int` | No | `256` | Tile width in pixels. |
| `tile_height` | `int` | No | `256` | Tile height in pixels. |
| `sample_format` | `int` | No | `1` | Sample format. `1` = unsigned integer, `2` = signed integer, `3` = IEEE floating point. |
| `jpeg_tables` | `string` or `null` | No | `null` | Base64-encoded shared JPEG quantization and Huffman tables (TIFF tag 347). Required when `compression` is `7` (JPEG). |

### Compression Tag Values

| Value | Name | Notes |
|-------|------|-------|
| `1` | None (uncompressed) | Raw tile bytes; still needs byte-order and planar conversion. |
| `5` | LZW | Supports horizontal differencing predictor (`predictor=2`). |
| `7` | JPEG | Requires `jpeg_tables`. Handles YCbCr-to-RGB conversion when `photometric=6`. |
| `8` | Deflate (zlib) | Supports horizontal differencing predictor. |
| `32773` | PackBits | Run-length encoding. |
| `32946` | Adobe Deflate | Equivalent to Deflate; legacy tag value. |

### Sample Format / Bits Per Sample → NumPy dtype Mapping

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

## Decoding Behavior

1. Construct a minimal single-tile TIFF buffer in memory: an 8-byte TIFF header, an IFD containing the tag values from the codec configuration, and the compressed tile bytes appended after the IFD.
2. Open the buffer with libtiff's `TIFFClientOpen` using memory-backed I/O callbacks.
3. If `compression=7` (JPEG) and `photometric=6` (YCbCr), set `JPEGCOLORMODE_RGB` so libtiff performs YCbCr-to-RGB conversion during decode.
4. Call `TIFFReadEncodedTile(handle, 0, ...)` to decompress the tile. libtiff handles predictor reversal, byte-order conversion, and color space conversion internally.
5. If the decoded tile is smaller than the nominal tile dimensions (edge tile), pad with zeros to the full tile shape.
6. Convert from chunky (pixel-interleaved) to band-sequential (BSQ) format if `planar_config=1` and `samples_per_pixel > 1`.
7. Return a NumPy ndarray with shape `(samples_per_pixel, tile_height, tile_width)` and the dtype corresponding to the `sample_format`/`bits_per_sample` combination.

Encoding is not supported. Calling `encode()` raises `NotImplementedError`.

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

## Python Class

`aws.osml.io.zarr_codecs.TiffTileCodec` — see [API Reference](../api/zarr-codecs.md).
