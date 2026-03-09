# BUG: Redundant J2K_LOSSLESS and J2K_COMPRESSION_RATIO Parameters

## Summary

`J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` are redundant metadata parameters that
duplicate information already conveyed by the NITF `COMRAT` field. They should be
removed from both the code and the user guide.

## Problem

The writer (`src/jbp/writer.rs`) reads three overlapping sources of compression
configuration for JPEG 2000:

1. `COMRAT` — the standard NITF image subheader field that encodes lossless mode
   (`Nnnn.n`) or target bits-per-pixel (`nn.n`) per the JBP specification.
2. `J2K_LOSSLESS` — a synthetic metadata key that sets the `lossless` flag on
   `J2KEncodingHints`.
3. `J2K_COMPRESSION_RATIO` — a synthetic metadata key that sets the
   `compression_ratio` field on `J2KEncodingHints`.

`J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` express the same information as `COMRAT`
in a different form. The code currently treats the `J2K_` fields as the primary
source of truth for the encoder and then *regenerates* the COMRAT subheader value
from them via `generate_comrat()`, ignoring the user-supplied `COMRAT` string
whenever `J2KEncodingHints` are present.

This creates several issues:

- Users can set contradictory values (e.g. `COMRAT=N001.0` with
  `J2K_LOSSLESS=false` and `J2K_COMPRESSION_RATIO=10.0`). The encoder silently
  uses the `J2K_` values and overwrites the COMRAT in the subheader.
- The user guide documents both `COMRAT` and the `J2K_` fields, making it unclear
  which one actually controls behavior.
- The design spec (`.kiro/specs/jpeg2000-compression/design.md`) already states:
  "The writer parses COMRAT to determine lossless mode and compression ratio, so
  separate `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` hints are not needed." But
  the code does the opposite — it ignores COMRAT and uses the `J2K_` fields.

## Expected Behavior

The writer should derive lossless mode and compression ratio from the `COMRAT`
field, which is the standard NITF mechanism. The `J2K_` parameters for lossless
and compression ratio should not exist. Other `J2K_` parameters that have no
COMRAT equivalent (`J2K_DECOMPOSITION_LEVELS`, `J2K_QUALITY_LAYERS`) are fine
and should remain.

## Affected Code

- `src/jbp/writer.rs` — `extract_encoding_hints()` (lines ~668-698): reads
  `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from metadata dict.
- `src/jbp/j2k/comrat.rs` — `generate_comrat()`: converts `J2KEncodingHints`
  back to a COMRAT string, which is redundant if COMRAT was the input.
- `src/jbp/writer.rs` — subheader generation (lines ~1524-1528): prefers
  `generate_comrat(j2k_hints)` over the user-supplied COMRAT string.

## Affected Documentation

- `docs/user-guide/image-assets-writing.md` — the "JPEG 2000 Encoder Parameters"
  table lists `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` with descriptions and
  defaults. The code examples set both `COMRAT` and the `J2K_` fields together.

## Recommended Fix

1. Remove `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from `extract_encoding_hints()`.
2. Parse the user-supplied `COMRAT` string (using `J2KComrat::parse()`) to derive
   the `lossless` flag and `compression_ratio` for `J2KEncodingHints`.
3. Remove `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from the user guide table
   and code examples.
4. Keep `J2K_DECOMPOSITION_LEVELS` and `J2K_QUALITY_LAYERS` — these have no
   equivalent in COMRAT and are legitimate encoder-only hints.

## Design Principle to Enforce

The design spec (`.kiro/specs/jpeg2000-compression/design.md`) must also be
updated to reflect this fix and to codify the following principle:

Non-standard `J2K_` prefixed metadata fields should be minimized. A synthetic
encoding hint should only exist when there is absolutely no way to derive the
information from a standard NITF metadata field. `COMRAT` already encodes
lossless mode and compression ratio, so `J2K_LOSSLESS` and
`J2K_COMPRESSION_RATIO` violate this rule.

The remaining `J2K_` fields (`J2K_DECOMPOSITION_LEVELS`, `J2K_QUALITY_LAYERS`)
are acceptable because no standard NITF field carries that information. These
fields must have sensible defaults (currently 5 and 1 respectively) so that
users can ignore them entirely unless they have a specific need to tune encoder
behavior. The design spec should state this policy explicitly so future format
implementations (e.g. GeoTIFF) follow the same pattern: derive from standard
fields first, add synthetic hints only as a last resort, and always default to
reasonable values.
