# Assembly Module Design

## Extraction Rationale

The tile assembly algorithm was originally implemented inside
`src/jbp/image/encoder.rs` as part of the JBP (NITF) encoder. That encoder
needed to map arbitrary source block grids to NITF's NPPBH/NPPBV tile
dimensions — the same problem every format writer faces when the source
provider's block layout doesn't match the output tile grid.

Rather than duplicate this logic across TIFF, J2K, PNG, DTED, and JPEG writers
(each with subtly different edge-tile handling), the algorithm was extracted
into `src/assembly/` as a shared utility. All format writers now use the same
proven code path, and the JBP encoder imports from here instead of owning its
own copy.

## Module Boundaries

The assembly module depends only on:
- `src/traits/` — `ImageAssetProvider` trait (the source interface)
- `src/error.rs` — `CodecError` (the error type)
- `src/types.rs` — `PixelType` (for `pad_pixel_bytes`)

It does **not** depend on any format-specific module, avoiding circular
dependencies. Format writers depend on `assembly`, never the reverse.

## Relationship to Provider-Level Caching

`TileAssembler` is intentionally stateless: it reads source blocks on demand
and does not memoize results. When output tiles are smaller than source blocks,
adjacent output tiles will re-read the same source blocks. This is acceptable
for most workflows because:

1. File-backed providers typically have OS-level page cache hits for
   recently-decoded blocks.
2. `BufferedImageAssetProvider` already holds block data in memory (the
   local override map serves as a de facto cache for `set_block` data).
3. Adding caching to the assembler would couple it to memory management
   policy, making it harder to reason about lifetime and ownership.

A future provider-level block cache (decorator pattern around
`ImageAssetProvider`) is the correct place to optimize repeated reads. The
assembler benefits passively from any caching provider without code changes.
This cache should live in a separate module (`src/cache/` or
`src/block_cache/`), not here.

## Key Design Decisions

### Borrow-based lifetime (`TileAssembler<'a>`)

The assembler borrows the source provider rather than owning it via `Arc`.
This makes it cheap to construct per-call (8 integer copies + one reference)
and avoids reference-counting overhead in tight writer loops. The tradeoff is
that the assembler cannot outlive the provider — acceptable since writers
always hold the provider for the duration of `close()`.

### Edge tiles return actual dimensions

`get_output_tile` returns the actual pixel dimensions of edge tiles (which may
be smaller than the requested output tile size). It does **not** pad to full
tile dimensions. Padding policy differs by format (TIFF pads to full tile
size; J2K edge tiles are naturally smaller), so padding remains each writer's
responsibility.

### `has_block` is a pure geometry check

`TileAssembler::has_block` checks whether an output tile's pixel region
overlaps the source image extent using only stored dimensions. It does not
call the source provider's `has_block` method and performs no I/O. This
enables `BufferedImageAssetProvider` to cheaply determine whether a requested
block falls entirely outside the source extent (returning a pad-filled buffer)
without touching the source at all.

### `reassemble_full_image` as convenience wrapper

Untiled formats need the entire image as one contiguous buffer. Rather than
each writer reimplementing the "read all blocks and stitch" logic,
`reassemble_full_image` wraps `TileAssembler` with output tile dimensions
equal to the full image size, producing a single-tile output. The assembler's
existing `grids_match()` fast path ensures this is zero-cost when the source
is already a single block.
