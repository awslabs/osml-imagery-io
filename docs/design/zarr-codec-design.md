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
  to be interpreted correctly. A multiband chunky tile is pixel-interleaved on
  disk (`RGBRGBRGB…`), so it must be de-interleaved before it is a
  `(bands, rows, columns)` array — compression is not the only reason a TIFF
  tile needs a codec, and "uncompressed" does not imply "ready to reshape".

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

Single-band uncompressed tiles are the one case that needs no configuration at
all: the raw tile bytes already are the pixel data, and per TIFF 6.0
`PlanarConfiguration` is irrelevant when `SamplesPerPixel` is 1. For those,
`codec_configuration()` returns `None` and Zarr reads the bytes directly,
preserving a zero-copy path. Multiband uncompressed tiles do get a
configuration, because they still need sample reordering (see
[Chunking planar TIFF per plane](#chunking-planar-tiff-per-plane)).

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

#### Chunking planar TIFF per plane

A TIFF with `PlanarConfiguration = 2` stores each band in its own tile: per TIFF
6.0 (tags 324/325, p. 68) `TileOffsets` and `TileByteCounts` carry
`SamplesPerPixel × TilesPerImage` entries, laid out band-major — all of plane
0's tiles, then all of plane 1's, and so on. Nothing about that layout fits a
chunk grid whose chunks each span every band.

**Decision.** For planar multiband TIFF, the chunk grid is **band-granular**:
`chunk_shape = (1, block_height, block_width)`, chunk keys `{band}.{row}.{col}`,
one chunk per plane per tile position, with the array's codec instance
overridden to `samples_per_pixel = 1, planar_config = 1`. Each chunk is exactly
one single-plane TIFF tile, decoded independently; Zarr stacks the planes along
the band axis. Chunky arrays are untouched — they keep
`chunk_shape = (bands, block_height, block_width)` and `0.{row}.{col}` keys.

**The alternative, and why it lost.** The obvious way to keep the chunk grid
spatial-only is to have one chunk reference all N planes and concatenate them:
the multi-range mechanism the filesystem layer already provides for interleaved
J2K tile-parts. That works for uncompressed planar data, where each plane's
length is fixed by geometry. It breaks down for compressed planar data, because
each plane compresses to a different length, and those lengths vary per tile.
The decoder would have to be told where each plane ends inside the concatenated
buffer — per-tile information that cannot live in a per-array codec
configuration. Rescuing the approach means extending the configuration to carry
per-chunk plane boundaries, which is per-chunk state in a per-array field.

Per-plane chunking dissolves that problem rather than solving it. Each plane is
its own chunk with its own byte range, so plane boundaries are expressed by the
manifest — the structure that exists to express byte ranges — and never need to
be packed into a buffer or a config. Three further consequences all point the
same way:

- **No decoder change.** With `samples_per_pixel = 1` the decoder's existing
  single-band path already handles these chunks, edge-tile zero-padding
  included. Uncompressed and compressed planar stop being distinct cases.
- **Predictor semantics are correct by construction.** Per TIFF 6.0 Section 13,
  horizontal differencing on planar data works per plane exactly as it does on
  grayscale data, so presenting a plane as a standalone single-band tile needs
  no stride adjustment. (The chunky path is the one with a stride of
  `SamplesPerPixel`, which is why `samples_per_pixel` must stay truthful there.)
- **No trait change.** `tile_byte_ranges()` already returns
  `Vec<(u64, u64)>` per `(row, col)` key, so planar TIFF returns its N plane
  ranges under the existing key in band order. The signature
  (`src/traits/image.rs`) and the ten other implementors are untouched.

**No consumer-visible change.** The decision is confined to the chunk grid.
`shape` stays `(bands, rows, columns)` and `dimension_names` stays
`("bands", "y", "x")` for both layouts, so indexing semantics are identical and
any selection returns a band-first array — including windows crossing tile and
band boundaries, which Zarr's indexing layer already assembles from multiple
intersecting chunks. `IO.open()` never consults the chunk grid and is unaffected.

The consequence that *is* real is request count, and it is directionally mixed.
An all-bands read of one tile becomes N byte ranges instead of one — read
amplification on high-latency stores, mitigated by the ranges being disjoint (no
wasted bytes) and fetched concurrently. A single-band read becomes cheaper than
under a chunky grid, where a band's samples are interleaved with the others and
cannot be fetched separately. Both follow from the file itself: a planar TIFF
stores each band in its own tile, so a multiband read always had to touch
multiple disjoint regions. The prior behavior only looked cheaper because it
fetched one plane and silently dropped the rest.

**The one sharp edge.** That `Vec` now carries two meanings, and they are
indistinguishable by length: N entries means "N fragments of one chunk to
concatenate" for J2K, and "N independent per-band chunks" for planar TIFF. A
three-tile-part J2K chunk and a three-band planar tile look identical. The
parser therefore branches on the provider's declared `planar_config`, never on
`len(range_list)` — `_is_planar_multiband_tiff()` in
`virtualizarr_parsers.py`, with a comment at the branch.

Both Zarr consumer paths (numcodecs/Kerchunk-v2 via `ReferenceFileSystem`, and
native zarr v3) build their arrays through the same `_build_manifest_array()`, so
the geometry is defined once and applies to both.

#### Refusing invalid geometry

The failure mode this chunking scheme replaced was the dangerous kind: an array
whose chunks referenced only plane 0 read back as *plausible* pixels with no
error at all. Wrong output that raises nothing is worse than wrong output that
crashes, so `_build_manifest_array()` validates geometry before returning an
array and raises `ValueError` naming the offending configuration (bands,
compression, `planar_config`) rather than emitting a store. Four invariants:

1. A multiband asset must have a codec configuration — the direct signature of
   the "uncompressed multiband needs no codec" mistake.
2. The chunk grid must cover the declared shape (`ceil(shape / chunk_shape)`
   per axis).
3. No grid position may be unreferenced — a hole reads back as `fill_value`,
   silently.
4. Each chunk's referenced byte length must be consistent with its geometry.

The check is pure geometry, run once per array at index-build time; it fetches
and decodes nothing. Two details it has to get right, both found by sweeping
real files rather than by reasoning about them: trailing blocks may be *clipped*
rather than padded (a tiled TIFF pads edge tiles to the nominal tile, but a
stripped TIFF's last strip carries only the rows it covers, and both are
readable), and a non-contiguous chunk's effective length is the sum of its
fragments, not the length of the placeholder manifest entry that spans only the
first one.

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

## One Class, Two Codec Protocols

Every codec class here subclasses zarr v3's `BytesBytesCodec` **and** implements
the numcodecs filter protocol. That is deliberate: the two consumer paths a tile
index can be read through resolve and call codecs differently, and a single class
has to satisfy both.

| | numcodecs / Kerchunk v2 | native zarr v3 |
|---|---|---|
| Read by | fsspec `ReferenceFileSystem` + `.zarray` filters | `zarr.open` / `xarray` + `zarr.json` codec chain |
| Codec named by | the `id` field | the codec URI |
| Resolved through | `numcodecs.codecs` entry points | `zarr.codecs` entry points |
| `decode` receives | **one buffer**: `decode(buf)` | **a batch**: `decode([(buf, spec), ...])` |
| `decode` is | synchronous, returns bytes | `async`, returns buffers |

### The collision

Both protocols name the method `decode`, with incompatible signatures. Because
the synchronous single-buffer `decode` is defined in the class body, it
**shadows** the inherited async batched `BytesBytesCodec.decode`. When zarr v3's
pipeline does `await codec.decode(<iterable of (bytes, spec)>)` — see
`BatchedCodecPipeline.decode_batch` in `zarr/core/codec_pipeline.py` — the call
lands on the numcodecs shim, which treats the whole batch iterable as one chunk's
bytes and hands it to the Rust decoder:

```
JbpBlockCodec(...).decode([])   ->  ValueError: Data size mismatch: expected 4 bytes, got 0
```

Left unresolved this makes **every** native v3 read fail, which is precisely what
happened: the shadowing shipped undetected because no producer emitted a v3 store
and no test read one.

### The discriminator

`decode` therefore begins by asking which protocol is calling, and hands batch
calls back to the shadowed base method:

```python
def decode(self, buf, out=None):
    # Defining decode() shadows BytesBytesCodec.decode; hand v3 pipeline calls back to it.
    if not _is_numcodecs_buffer(buf):
        return super().decode(buf)
    ...  # synchronous single-buffer path
```

`_is_numcodecs_buffer` tests for **a single buffer** — `bytes`, `bytearray`,
`memoryview`, anything exposing `__array__`, or anything `memoryview()` accepts —
rather than testing for a batch. That direction matters: the batch is whatever
iterable zarr happens to construct (a list, a `zip`, a generator), so it is not a
stable thing to pattern-match, whereas "is a single buffer" is a property of the
argument itself. Note that returning `super().decode(buf)` returns a *coroutine*,
un-awaited, which is exactly what the async pipeline expects to await.

The v3 path proper is `_decode_single(chunk_bytes, chunk_spec)`, which the
inherited batched `decode` fans out to. It runs the same Rust decoder as the
numcodecs path via `asyncio.to_thread`, so neither route blocks the event loop
and both produce identical pixels.

### Where the two routes can drift

The routes share the decoder but not their surrounding code, and edge-tile
padding is where that shows. Both must pad an undersized edge tile up to the
nominal chunk shape so the consumer's reshape succeeds, and they derive that
shape independently:

- The numcodecs route re-derives it from the format's own metadata — for J2K, by
  `struct.unpack`-ing `XTsiz`/`YTsiz` out of the base64 main header in its
  configuration and ceil-dividing by `2 ** resolution_level`.
- The v3 route reads `chunk_spec.shape`, which zarr supplies from the array
  metadata.

Those must agree, including on odd reduced sizes at non-zero resolution levels,
and they are asserted to in `tests/property/zarr/test_v3_pipeline.py` — both for
cross-protocol pixel equality and specifically for non-block-aligned edge tiles.
(TIFF turns out never to reach either padding path: per TIFF 6.0 tag 322 tile
data is stored padded to the full nominal tile, so `decode_tiff_tile` already
returns a complete tile and both padding steps are no-ops. The tests assert that
explicitly rather than assuming it.)

### Why not two classes

Splitting each codec into a numcodecs `Codec` and a zarr `BytesBytesCodec` over a
shared Rust core would make `decode` mean exactly one thing per class and remove
the runtime discriminator entirely. That is the structural fix, and it is the
right end state. It is not urgent: the cost is duplicated registration and
configuration plumbing on both sides, and the awkward shape is a symptom of the
ecosystem's v2-to-v3 migration rather than something inherent to this design.
Keeping one class per format also keeps one configuration schema per format,
which is what guarantees a v2 and a v3 index of the same file describe the same
codec.

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
