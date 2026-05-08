# DTED Tile Codec

**Version:** 1.0  
**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/dted`  
**Codec type:** array-to-bytes  

Decodes DTED (Digital Terrain Elevation Data) file data sections into NumPy
arrays. Performs signed-magnitude to two's complement conversion, big-endian to
native-endian byte swap, per-record header/checksum stripping, column-major to
row-major transposition, and optional boundary-post trimming for seamless
mosaicking.

DTED is an uncompressed, column-major elevation format defined by MIL-PRF-89020B.
Each file contains a single 1-degree cell of signed 16-bit elevation posts. The
data section consists of sequential longitude column records, each containing a
sentinel byte, block/coordinate headers, elevation values, and a checksum. The
codec operates on the data section only (byte offset 3428 onward).

## Document Conventions

The key words "MUST", "MUST NOT", "SHOULD", and "MAY" in this document are to be
interpreted as described in [RFC 2119][rfc2119].

## Codec Identifier

The value of the `name` member in the codec metadata MUST be
`https://awslabs.github.io/osml-imagery-io/codecs/dted`.

## Encoded Representation

The encoded representation MUST be the data section of a DTED file as defined in
MIL-PRF-89020B Section 3.11. The byte sequence consists of `num_lon_lines`
sequential data records, each `record_size` bytes long. Each record contains:

- 1-byte data block recognition sentinel (`0xAA`)
- 3-byte sequential count
- 2-byte longitude count
- 2-byte latitude count
- `num_lat_points` x 2 bytes of signed-magnitude big-endian elevation values
- 4-byte checksum

The total byte length MUST equal `num_lon_lines * record_size`.

## Rationale: Why DTED Requires a Specialized Codec

DTED data cannot be consumed as raw bytes by a standard Zarr codec for three
reasons:

1. **Record framing.** The data section is not a flat array of elevation values.
   It consists of variable-length records, each wrapped with an 8-byte header
   (sentinel, block count, coordinate counts) and a 4-byte checksum. These
   must be stripped before the pixel data is usable.

2. **Signed-magnitude encoding.** DTED uses a non-standard numeric
   representation: elevations are stored as signed-magnitude big-endian 16-bit
   integers, not two's complement. The high bit indicates sign; the remaining
   15 bits are the absolute value. This requires explicit conversion to the
   two's complement representation that NumPy and all modern systems expect.

3. **Column-major storage with boundary overlap.** Elevation posts are stored
   column-by-column (longitude-first), but Zarr arrays and NumPy use row-major
   (C) order. Additionally, adjacent 1-degree DTED cells share their boundary
   posts — the easternmost column of one cell is identical to the westernmost
   column of its neighbor. For Zarr's non-overlapping chunk model to work, these
   shared edges must be trimmed during decode.

The codec handles all three transformations: stripping record framing, converting
signed-magnitude to two's complement, transposing to row-major order, and
trimming shared boundary posts. This enables representing an entire DTED archive
as a single contiguous Zarr array where each file becomes one chunk and consumers
see a seamless elevation surface with no preprocessing required.

## Configuration Parameters

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `num_lat_points` | `int` | No | `1201` | Number of elevation posts per column (row count). |
| `num_lon_lines` | `int` | No | `1201` | Number of longitude columns (record count). |
| `record_size` | `int` | No | `2414` | Size of each data record in bytes. |
| `trim_top` | `int` | No | `0` | Rows to discard from the top of the decoded raster. |
| `trim_bottom` | `int` | No | `0` | Rows to discard from the bottom. |
| `trim_left` | `int` | No | `0` | Columns to discard from the left. |
| `trim_right` | `int` | No | `0` | Columns to discard from the right. |

The defaults correspond to a DTED Level 1, Zone I cell (3-arcsecond spacing,
1201 x 1201 posts, record size = 8 + 1201x2 + 4 = 2414 bytes).

## Algorithm

### Decoding

1. Validate that the input data length equals `num_lon_lines * record_size`.
2. For each of the `num_lon_lines` records:
   a. Skip the 8-byte header (sentinel + block/longitude/latitude counts).
   b. Read `num_lat_points` x 2 bytes of signed-magnitude big-endian elevations.
   c. Skip the 4-byte checksum.
   d. Convert each 2-byte value from signed-magnitude to native two's complement i16.
3. Transpose the column-major data into row-major order.
4. Apply boundary trimming (if any trim parameters are non-zero).
5. Return an array with shape `(1, output_rows, output_cols)` and dtype `int16`,
   where `output_rows = num_lat_points - trim_top - trim_bottom` and
   `output_cols = num_lon_lines - trim_left - trim_right`.

### Encoding

Encoding is not currently specified. See [Implementation Notes](#implementation-notes).

## Overlap-Aware Edge Trimming

DTED cells are designed with inherent boundary-post overlap. Adjacent 1-degree
cells share their edge row/column — the easternmost column of one cell is
identical to the westernmost column of its neighbor. This ensures interpolation
continuity but creates a problem for Zarr arrays, which partition coordinate
space into non-overlapping chunks.

The `trim_*` parameters solve this at the codec level. By trimming one row and/or
column from each shared edge, the codec outputs non-overlapping chunks that tile
seamlessly. The Zarr manifest declares the trimmed output shape as the chunk
shape.

For example, trimming the east and south boundary posts of each cell:

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/dted",
    "configuration": {
        "num_lat_points": 1201,
        "num_lon_lines": 1201,
        "record_size": 2414,
        "trim_top": 0,
        "trim_bottom": 1,
        "trim_left": 0,
        "trim_right": 1
    }
}
```

This decodes the full 1201x1201 cell and outputs a 1200x1200 array. Adjacent
chunks tile perfectly with no duplication, enabling seamless global elevation
arrays without any data preprocessing.

## Example Configuration

### Standard Level 1 cell (no trimming)

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/dted",
    "configuration": {
        "num_lat_points": 1201,
        "num_lon_lines": 1201,
        "record_size": 2414
    }
}
```

### Level 2 cell with boundary trimming for mosaicking

```json
{
    "name": "https://awslabs.github.io/osml-imagery-io/codecs/dted",
    "configuration": {
        "num_lat_points": 3601,
        "num_lon_lines": 3601,
        "record_size": 7214,
        "trim_top": 0,
        "trim_bottom": 1,
        "trim_left": 0,
        "trim_right": 1
    }
}
```

## References

- [MIL-PRF-89020B][mil-prf-89020b] — Performance Specification: Digital Terrain Elevation Data (DTED)
- [RFC 2119][rfc2119] — Key words for use in RFCs to Indicate Requirement Levels

[mil-prf-89020b]: https://earth-info.nga.mil/publications/specs/printed/89020B/89020B.pdf
[rfc2119]: https://www.rfc-editor.org/rfc/rfc2119

## Implementation Notes

`aws.osml.io.zarr_codecs.DtedTileCodec` — see [API Reference](../api/zarr-codecs.md).

Only the decode path is implemented. Calling `encode()` raises `NotImplementedError`.
