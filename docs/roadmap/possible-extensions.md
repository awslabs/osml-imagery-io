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
- **TRE field lookup** — resolve TRE tag names to human-readable descriptions.

The `datetime.rs` Rust implementation could back the datetime conversion if
exposed through PyO3, or it could be implemented purely in Python.
