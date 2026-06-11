//! Tile assembly utilities for mapping between source and output block grids.
//!
//! Format writers produce output tiles whose dimensions may differ from the
//! source provider's block grid (e.g., writing 512×512 TIFF tiles from a
//! provider that exposes 256×256 blocks). This module provides [`TileAssembler`]
//! to handle that mapping: given a source `&dyn ImageAssetProvider` and desired
//! output tile dimensions, it composites the correct pixels from one or more
//! source blocks into each output tile.
//!
//! # When to use
//!
//! - **Tiled writers** (TIFF, J2K with tile metadata): construct a
//!   `TileAssembler` with output tile dimensions and iterate
//!   [`TileAssembler::get_output_tile`] for each position in the output grid.
//! - **Untiled writers** (PNG, DTED, JPEG): call `reassemble_full_image` to
//!   collapse a multi-block source into a single contiguous buffer.
//! - **`BufferedImageAssetProvider`**: uses `TileAssembler` internally when
//!   the wrapped source has a different block grid.
//!
//! # Design constraints
//!
//! - **Stateless per-call**: the assembler holds only dimension metadata and a
//!   source reference. It does not cache source blocks between calls.
//! - **No caching**: repeated reads of the same source block (when multiple
//!   output tiles overlap it) are the caller's or provider's responsibility
//!   to optimize. A future provider-level block cache can sit beneath the
//!   assembler transparently.
//! - **BSQ format**: all data flows through in band-sequential layout with
//!   shape `[bands, rows, cols]`, matching the `get_block()` contract.
//! - **No resampling**: assembly is pixel-exact copying. No interpolation,
//!   reprojection, or band reordering occurs.

mod pad;
mod tile_assembler;

pub use pad::pad_pixel_bytes;
pub(crate) use tile_assembler::reassemble_full_image;
pub use tile_assembler::TileAssembler;
