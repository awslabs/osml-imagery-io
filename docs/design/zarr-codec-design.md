# Synthetic Codestream Codec Pattern

This document describes the core pattern used by osml-imagery-io's Zarr codecs
to decode compressed tile data from archival imagery formats. The pattern
applies to both JPEG 2000 (J2K) codestreams embedded in NITF files and
compressed tiles in TIFF/GeoTIFF/COG files, and is designed to extend to
future formats and decoder backends.

## The Problem

Zarr codecs are pure bytes-to-bytes transforms. A codec receives the raw bytes
for a single chunk and must produce uncompressed pixel data. No file handle, no
seeking, no access to anything outside the chunk bytes and the codec's own
configuration.

Archival imagery formats store compressed tile data that is not self-contained.
The compressed bytes for a single tile depend on shared state stored elsewhere
in the file:

- **JPEG 2000**: Each tile-part contains only wavelet coefficients. The
  decoding parameters — tile dimensions, quantization tables, wavelet
  decomposition levels, component counts — live in the codestream's main
  header (SIZ, COD, QCD markers). A decoder cannot reconstruct pixels from
  a tile-part alone.

- **TIFF JPEG (compression tag 7)**: Individual JPEG tiles are not standalone
  JFIF files. They depend on shared quantization and Huffman tables stored in
  the `JPEGTables` IFD tag (347). A standard JPEG decoder will fail on a bare
  TIFF JPEG tile.

- **TIFF LZW/Deflate with predictor**: The compressed bytes decompress to
  delta-encoded pixel values, not actual pixel values. The `Predictor` tag
  (317), `BitsPerSample`, and `SamplesPerPixel` from the IFD are needed to
  reverse the differencing step after decompression.

- **TIFF uncompressed**: Even raw bytes require the file's byte order
  (big-endian vs little-endian) and `PlanarConfiguration` (chunky vs planar)
  to be interpreted correctly.

The third-party decoder libraries (OpenJPEG, libtiff, libjpeg-turbo) are
designed to operate on complete, valid inputs — a full J2K codestream, a
complete TIFF file — not on isolated tile bytes with out-of-band parameters.
There is no `opj_decode_bare_tilepart()` or `TIFFDecodeTileBytes()` API.

## The Pattern: Synthetic Codestream Reconstruction

The solution is to reconstruct a minimal, valid input for the decoder library
from two pieces of information:

1. **Shared header state** — extracted from the source file at index time and
   stored in the codec configuration (serialized into `.zarray` metadata).
   This is a small, fixed-size blob: typically 100–500 bytes for J2K main
   headers, or a handful of IFD tag values for TIFF.

2. **Chunk bytes** — the raw compressed tile data fetched at read time via
   byte-range requests into the source file.

At decode time, the codec combines these two pieces into a synthetic but valid
input that the decoder library accepts as if it were a real file or codestream:

```
Codec configuration (from .zarray)     Chunk bytes (from source file)
┌─────────────────────────────┐        ┌──────────────────────────┐
│  Shared header / IFD tags   │   +    │  Compressed tile bytes   │
└─────────────────────────────┘        └──────────────────────────┘
                │                                    │
                └──────────┬─────────────────────────┘
                           ▼
              ┌──────────────────────────┐
              │  Synthetic valid input   │
              │  (codestream or TIFF)    │
              └──────────────────────────┘
                           │
                           ▼
              ┌──────────────────────────┐
              │  Third-party decoder     │
              │  (OpenJPEG / libtiff)    │
              └──────────────────────────┘
                           │
                           ▼
              ┌──────────────────────────┐
              │  Uncompressed pixels     │
              │  (NumPy array)           │
              └──────────────────────────┘
```

The decoder library receives what looks like a normal single-tile file and
decodes it. It does not know or care that the input was synthesized. The codec
performs no I/O — it is a pure function from `(config, compressed_bytes)` to
`pixels`.

## Precedent

This pattern has precedent in the JPEG 2000 ecosystem. JPIP (JPEG 2000
Interactive Protocol, ITU-T T.808) streams individual tile-parts to clients
that already hold the main header. The client reconstructs a decodable
codestream on the fly from the cached header and the received tile-part data.
Our J2K codec does exactly this.

For TIFF, the pattern is less established but equally valid. libtiff's
`TIFFClientOpen` API accepts custom I/O callbacks that operate on arbitrary
memory buffers. Constructing a minimal valid TIFF in memory and opening it
with `TIFFClientOpen` is the documented way to use libtiff without filesystem
access — the same approach this project already uses for reading full TIFF
files from byte slices.

## Format-Specific Details

### JPEG 2000

#### Shared state (codec configuration)

The J2K main header, base64-encoded. Contains the SIZ marker (image and tile
dimensions, component counts, bit depths), COD marker (coding style, wavelet
decomposition levels, progression order), and QCD marker (quantization
parameters). Typically 100–500 bytes.

#### Reconstruction

```
[patched main header] + [tile-part bytes with Isot=0] + [EOC marker]
```

The main header's SIZ marker is patched to describe a single-tile image with
the actual tile dimensions (handling edge tiles that may be smaller than the
nominal tile size). The tile-part's `Isot` field (tile index in the SOT
marker) is rewritten to 0 since the synthetic codestream contains only one
tile. An EOC (End of Codestream) marker is appended.

OpenJPEG receives a valid single-tile J2K codestream and decodes tile 0.

#### Codec configuration in `.zarray`

```json
{
  "name": "https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000",
  "configuration": {
    "main_header": "<base64-encoded main header bytes>",
    "resolution_level": 0
  }
}
```

#### Implementation

The reconstruction logic lives in `src/bindings/codecs.rs::decode_jpeg2000()`.
The SIZ patching logic lives in `src/j2k/markers.rs::rewrite_siz_for_tile()`.
The Python-side `Jpeg2000Codec` class in `python/aws/osml/io/zarr_codecs.py`
carries the base64-encoded main header in its configuration and passes it to
the Rust function at decode time.

### TIFF

#### Shared state (codec configuration)

A set of IFD tag values extracted from the source TIFF at index time:

| Tag | Name | Purpose |
|-----|------|---------|
| 256 | ImageWidth | Tile width (set to tile dimensions) |
| 257 | ImageLength | Tile height (set to tile dimensions) |
| 258 | BitsPerSample | Bits per sample per band |
| 259 | Compression | Compression type (1, 5, 7, 8, 32773, 32946) |
| 262 | PhotometricInterpretation | Color model (MinIsBlack, RGB, YCbCr) |
| 277 | SamplesPerPixel | Number of bands |
| 284 | PlanarConfiguration | Chunky (1) or planar (2) |
| 317 | Predictor | Differencing predictor (1=none, 2=horizontal, 3=float) |
| 322 | TileWidth | Tile width in pixels |
| 323 | TileLength | Tile height in pixels |
| 339 | SampleFormat | Data type (uint, int, float) |
| 347 | JPEGTables | Shared JPEG quantization/Huffman tables (JPEG only) |

Not all tags are needed for every compression type. Uncompressed tiles need
only the dimensional and pixel format tags. JPEG tiles additionally need
`JPEGTables` and `PhotometricInterpretation`. LZW/Deflate tiles additionally
need `Predictor`.

#### Reconstruction

A minimal valid TIFF byte buffer is constructed in memory:

```
[TIFF header (8 bytes)]
[IFD with tags from codec config]
[TileOffsets pointing to tile data]
[TileByteCounts with tile data length]
[Compressed tile bytes]
```

The TIFF header specifies byte order and the offset to the IFD. The IFD
contains the tag values from the codec configuration, plus `TileOffsets` and
`TileByteCounts` tags pointing to the appended compressed tile data. The
`ImageWidth` and `ImageLength` tags are set equal to the tile dimensions so
the synthetic TIFF describes a single-tile image.

This buffer is opened with `TIFFClientOpen` using the existing memory
read callbacks (`MemoryReadStreamData` in `src/tiff/ffi.rs`). A call to
`TIFFReadEncodedTile(handle, 0, ...)` decompresses the tile, applies
predictor reversal, performs byte-order conversion, and handles YCbCr→RGB
color space conversion — all within libtiff.

#### Codec configuration in `.zarray`

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
    "jpeg_tables": "<base64-encoded JPEGTables bytes, if present>"
  }
}
```

#### Implementation

- `src/tiff/image.rs` — `codec_configuration()` on `TIFFImageAssetProvider`
  returns IFD tag values (compression, bits_per_sample, samples_per_pixel,
  photometric, planar_config, predictor, tile_width, tile_height,
  sample_format, jpeg_tables) as a `HashMap<String, Vec<u8>>` for all
  supported compression types.

- `src/bindings/codecs.rs` — `decode_tiff_tile()` accepts compressed tile
  bytes and codec configuration parameters, constructs a synthetic single-tile
  TIFF buffer, opens it with `TIFFClientOpen`, calls `TIFFReadEncodedTile`,
  and returns decoded pixels as a NumPy array in BSQ format.

- `python/aws/osml/io/zarr_codecs.py` — `TiffTileCodec` class implements
  the Zarr v3 `BytesBytesCodec` interface and numcodecs filter protocol,
  registered with URI `https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile`.

- `python/aws/osml/io/virtualizarr_parsers.py` — `_build_codec_instance()`
  extended with a TIFF branch that detects configurations containing a
  `compression` key and constructs a `TiffTileCodec` instance.

### NITF Uncompressed (JbpBlockCodec)

NITF uncompressed tiles are a simpler case that does not require synthetic
codestream reconstruction. The raw bytes are self-contained but need
format-specific interpretation: big-endian to native byte swap and interleave
mode conversion (band-interleaved-by-pixel, by-line, by-block, or
band-sequential). The `JbpBlockCodec` carries the interleave mode (`imode`),
pixel value type (`pvtype`), and bits per pixel (`nbpp`) in its configuration
and performs the conversion directly in Rust without delegating to a
third-party library.

### NITF JPEG (JpegCodec)

NITF JPEG tiles are closer to standalone JFIF than TIFF JPEG tiles. The NITF
JPEG encoder produces complete JPEG streams per tile, but the codec still
needs format-specific parameters (color space, interleave mode, bits per
pixel) to correctly interpret the decoded output. The `JpegCodec` carries
these parameters and delegates to libjpeg-turbo via the Rust
`JpegBlockDecoder`.

## Why This Pattern

### Decoder library as a black box

The codec treats the decoder library as an opaque function:
`valid_input → pixels`. It does not depend on internal APIs, undocumented
behavior, or library-specific tile extraction functions. Any library that can
decode a valid J2K codestream or a valid TIFF file works as a backend.

### Backend swappability

Because the codec constructs a standard-format input, the decoder backend can
be replaced without changing the codec interface or the serialized
configuration. For JPEG 2000, this means the OpenJPEG backend could be
swapped for NVIDIA's nvJPEG2000 GPU-accelerated decoder, or for HTJ2K
decoders, without any changes to the Zarr codec layer or the tile index
format. The codec configuration (main header bytes, resolution level) is
format-defined, not library-defined. Any compliant J2K decoder accepts the
same reconstructed codestream.

Similarly, the TIFF codec could use any library that reads valid TIFF files
from memory — libtiff today, potentially a Rust-native TIFF decoder in the
future — without changing the codec configuration or the tile index.

### Automatic compression support

For TIFF, delegating to libtiff means the codec automatically supports every
compression scheme that libtiff supports, including schemes added in future
libtiff versions. The codec configuration captures the IFD tags; libtiff
interprets them. There is no compression-specific code in the codec itself
beyond constructing the synthetic TIFF buffer. Adding support for a new TIFF
compression type (e.g., WebP, ZSTD) requires only that libtiff supports it
and that `codec_configuration()` includes the relevant tags — no codec code
changes.

### Small configuration overhead

The shared state stored in the codec configuration is small relative to the
tile data:

- J2K main header: 100–500 bytes (base64-encoded: 130–670 bytes)
- TIFF IFD tags: ~10 integer values + optional JPEGTables blob (~200–600
  bytes for JPEG, negligible for other compressions)

This configuration is stored once per Zarr array in `.zarray`, not per chunk.
For a 4096×4096 image with 256×256 tiles (256 chunks), the configuration
overhead is amortized across all chunks.

### Pure codec, no I/O

The codec performs no I/O. The filesystem layer
(`MultiReferenceFileSystem`) handles fetching the compressed bytes via
byte-range requests. The codec receives those bytes and returns pixels. This
separation means the codec works identically for local files, S3 objects,
HTTP range requests, or any other byte source that fsspec supports.

## Relationship to the Tile Index Pipeline

The synthetic codestream pattern integrates with the tile index pipeline at
two points:

### Index generation (producer side)

`OversightMLParser` calls `asset.codec_configuration()` on each
`ImageAssetProvider` to extract the shared state. The
`_build_codec_instance()` function maps the configuration to a codec class
instance, which is serialized into the `.zarray` metadata for that array.

For J2K, `codec_configuration()` returns the main header bytes. For TIFF, it
returns the IFD tag values. For uncompressed NITF, it returns the interleave
and pixel format parameters.

### Tile reading (consumer side)

When Zarr reads a chunk, the registered codec deserializes its configuration
from `.zarray`, receives the compressed bytes from the filesystem, performs
the synthetic reconstruction, calls the decoder, and returns pixels.

```
.zarray metadata ──► Codec instance (with config)
                          │
Source file ──► fsspec ──► Compressed chunk bytes
                          │
                          ▼
                     Synthetic reconstruction
                          │
                          ▼
                     Decoder library
                          │
                          ▼
                     NumPy array
```

## Edge Tile Handling

Edge tiles (tiles at the right or bottom boundary of an image) may be smaller
than the nominal tile dimensions. The handling differs by format:

- **J2K**: The SIZ marker in the reconstructed codestream is patched to
  reflect the actual edge tile dimensions
  (`rewrite_siz_for_tile(header, tile_index)`). OpenJPEG decodes to the
  actual dimensions. The codec pads the result to the nominal chunk shape
  so Zarr's reshape succeeds; Zarr trims the padding at the array boundary.

- **TIFF**: libtiff handles edge tiles internally.
  `TIFFReadEncodedTile` returns the actual number of bytes decoded, which
  may be less than a full tile. The codec reads the actual dimensions from
  the decoded output and pads to the nominal chunk shape if needed.

- **NITF uncompressed**: The `JbpBlockCodec` receives exactly the bytes for
  the block, which may be smaller than the nominal block size for edge
  blocks. The caller (Zarr) handles the shape mismatch.

## Security Considerations

The synthetic codestream is constructed entirely from trusted data: the codec
configuration comes from the tile index (generated by this library), and the
chunk bytes come from the source file (fetched by fsspec). The decoder library
receives a well-formed input constructed by the codec, not arbitrary
user-supplied data.

However, the decoder libraries (OpenJPEG, libtiff, libjpeg-turbo) process
untrusted compressed data from the source file. These libraries have their own
security track records. The codec does not add attack surface beyond what the
decoder library already exposes — it merely provides a different entry point
to the same decompression code.

## Summary

| Format | Shared State | Reconstruction | Decoder | Codec URI |
|--------|-------------|----------------|---------|-----------|
| JPEG 2000 | Main header (SIZ, COD, QCD) | `[header] + [tile-part] + [EOC]` | OpenJPEG | `.../codecs/jpeg2000` |
| TIFF (all compressions) | IFD tag values + JPEGTables | Minimal single-tile TIFF buffer | libtiff | `.../codecs/tiff-tile` |
| NITF uncompressed | imode, pvtype, nbpp | Direct byte reinterpretation | Custom Rust | `.../codecs/jbp-block` |
| NITF JPEG | Color space, imode, bpp | Standalone JFIF (already complete) | libjpeg-turbo | `.../codecs/jpeg` |
