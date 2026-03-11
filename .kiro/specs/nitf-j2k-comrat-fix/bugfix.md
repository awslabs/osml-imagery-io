# Bugfix Requirements Document

## Introduction

The NITF JPEG 2000 writer uses redundant `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` metadata parameters that duplicate information already encoded in the standard NITF `COMRAT` field. The code treats these `J2K_` fields as the primary source of truth and regenerates the COMRAT subheader value from them via `generate_comrat()`, silently ignoring the user-supplied `COMRAT` string. This contradicts the design spec which states COMRAT should be the source of truth, creates ambiguity in the user guide, and allows users to set contradictory values without any warning.

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN a user sets `IC=C8` (or `CD`, `M8`, `MD`) with `J2K_LOSSLESS` and/or `J2K_COMPRESSION_RATIO` metadata fields THEN the system uses these `J2K_` fields as the primary source of truth for encoding, ignoring the user-supplied `COMRAT` value

1.2 WHEN a user sets contradictory values (e.g. `COMRAT=N001.0` with `J2K_LOSSLESS=false` and `J2K_COMPRESSION_RATIO=10.0`) THEN the system silently overwrites the COMRAT in the subheader with a value generated from the `J2K_` fields, producing a subheader that does not match the user's `COMRAT` input

1.3 WHEN `J2K_LOSSLESS` is not explicitly set by the user THEN the system defaults it to `false`, meaning a user who sets only `COMRAT=N001.0` (numerically lossless) without also setting `J2K_LOSSLESS=true` gets lossy encoding despite the lossless COMRAT value

1.4 WHEN the user guide documents both `COMRAT` and the `J2K_LOSSLESS`/`J2K_COMPRESSION_RATIO` fields as encoder parameters THEN the system creates confusion about which field actually controls compression behavior

### Expected Behavior (Correct)

2.1 WHEN a user sets `IC=C8` (or `CD`, `M8`, `MD`) with a `COMRAT` value THEN the system SHALL parse the `COMRAT` string using `J2KComrat::parse()` to derive the `lossless` flag and `compression_ratio` for `J2KEncodingHints`, making COMRAT the single source of truth

2.2 WHEN a user sets `COMRAT=N001.0` (numerically lossless) THEN the system SHALL configure the J2K encoder for lossless compression without requiring any additional `J2K_` metadata fields

2.3 WHEN a user sets `COMRAT=01.0` (lossy target bpp) THEN the system SHALL derive the compression ratio from the COMRAT bpp value and configure the J2K encoder accordingly, without requiring `J2K_COMPRESSION_RATIO`

2.4 WHEN the `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` metadata fields are provided THEN the system SHALL ignore them (they should be removed from the codebase and documentation entirely)

2.5 WHEN the user guide documents JPEG 2000 encoder parameters THEN the system SHALL list only `COMRAT`, `J2K_DECOMPOSITION_LEVELS`, and `J2K_QUALITY_LAYERS` as the valid encoder parameters, removing `J2K_LOSSLESS` and `J2K_COMPRESSION_RATIO` from the table and code examples

### Unchanged Behavior (Regression Prevention)

3.1 WHEN `J2K_DECOMPOSITION_LEVELS` is set by the user THEN the system SHALL CONTINUE TO use that value for wavelet decomposition levels (default 5)

3.2 WHEN `J2K_QUALITY_LAYERS` is set by the user THEN the system SHALL CONTINUE TO use that value for quality layers (default 1)

3.3 WHEN `IC=C8` is set without any COMRAT value THEN the system SHALL CONTINUE TO produce a valid JPEG 2000 encoded image using sensible defaults (numerically lossless)

3.4 WHEN `IC=CD` or `IC=MD` is set THEN the system SHALL CONTINUE TO enable HTJ2K mode based on the IC code

3.5 WHEN `IC=NC` or `IC=NM` (uncompressed) is set THEN the system SHALL CONTINUE TO skip J2K encoding hint extraction entirely

3.6 WHEN the COMRAT field is written into the image subheader for compressed images THEN the system SHALL CONTINUE TO write a properly formatted 4-character COMRAT value

3.7 WHEN non-J2K compression is used (e.g. `IC=C3` for JPEG DCT) THEN the system SHALL CONTINUE TO handle COMRAT as a quality factor without attempting J2K-specific parsing
