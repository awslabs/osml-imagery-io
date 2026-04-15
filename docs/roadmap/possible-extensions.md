# Possible Extensions

Ideas for future enhancements that are not currently planned for a specific release.

## NITF Metadata Utilities

### Unused `datetime.rs`

`src/jbp/datetime.rs` provides `parse_nitf_datetime`, `NitfDateTime`, and
`DateTimeParseError` for parsing NITF FDT (File Date Time) strings. The module
is re-exported from `jbp/mod.rs` but is not consumed anywhere in the Rust
codebase. It is effectively dead code with a full test suite.

Options:

1. Wire it into the JBP reader so parsed datetimes surface through the
   metadata provider.
2. Keep it as a standalone utility exposed to Python via PyO3.

### Python NITF Metadata Helper

NITF metadata is currently returned as raw string encodings (BCS-A / BCS-N
values). A Python helper class (analogous to `aws.osml.io.tiff.TagNameResolver`)
could offer:

- **DateTime conversion** — parse 14-character `CCYYMMDDhhmmss` FDT strings
  into `datetime.datetime` objects (handling the `"--"` unknown-component
  convention).
- **Numeric field coercion** — convert BCS-N string values to `int` / `float`
  where appropriate (e.g. NROWS, NCOLS, COMRAT).
- **Security marking helpers** — surface CLAS, CLSY, CODE, etc. as a
  structured object.

The `datetime.rs` Rust implementation could back the datetime conversion if
exposed through PyO3, or it could be implemented purely in Python.

## GeoZarr Conventions

The hierarchical tile index currently declares the
[GeoZarr multiscales convention](https://github.com/zarr-conventions/multiscales)
(UUID `d35379db-88df-4056-af3a-620245f8e347`) in the `zarr_conventions` array.
Two additional GeoZarr conventions are planned but not yet implemented.

### `proj:` Convention (CRS)

UUID `f17cb550-5864-4468-aeb7-f3180cfb622f`

Provides Coordinate Reference System (CRS) information for the dataset. When
implemented, attributes such as `proj:code` will be added to the root group
and a corresponding entry will be appended to the `zarr_conventions` array.

### `spatial:` Convention (Affine Transforms and Bounding Boxes)

UUID `689b58e2-cf7b-45e0-9fff-9cfc0883d6b4`

Provides affine transforms, bounding boxes, and spatial dimension metadata.
When implemented, attributes such as `spatial:transform`, `spatial:bbox`, and
`spatial:dimensions` will be added to the root group and a corresponding entry
will be appended to the `zarr_conventions` array.
