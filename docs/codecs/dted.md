# DTED Tile Codec

**URI:** `https://awslabs.github.io/osml-imagery-io/codecs/dted`

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

## Configuration Schema

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
1201 × 1201 posts, record size = 8 + 1201×2 + 4 = 2414 bytes).

## Decoding Behavior

1. Validate that the input data length equals `num_lon_lines × record_size`.
2. For each of the `num_lon_lines` records:
   a. Skip the 8-byte header (sentinel + block/longitude/latitude counts).
   b. Read `num_lat_points` × 2 bytes of signed-magnitude big-endian elevations.
   c. Skip the 4-byte checksum.
   d. Convert each 2-byte value from signed-magnitude to native two's complement i16.
3. Transpose the column-major data into row-major order.
4. Apply boundary trimming (if any trim parameters are non-zero).
5. Return a buffer with shape `(1, output_rows, output_cols)` and dtype `int16`,
   where `output_rows = num_lat_points - trim_top - trim_bottom` and
   `output_cols = num_lon_lines - trim_left - trim_right`.

Encoding is not supported. Calling `encode()` raises `NotImplementedError`.

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

This decodes the full 1201×1201 cell and outputs a 1200×1200 array. Adjacent
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

## Python Class

`aws.osml.io.zarr_codecs.DtedTileCodec` — see [API Reference](../api/zarr-codecs.md).
