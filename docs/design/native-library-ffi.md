# Native Library FFI Design

This document describes how osml-imagery-io binds to native C libraries, the
relationship between library versions and FFI struct layouts, and the procedures
for verifying and updating these bindings.

## Overview

The project links against three native C libraries for image codec support:

| Library | Purpose | Linking | Rust module |
|---------|---------|---------|-------------|
| OpenJPEG (libopenjp2) | JPEG 2000 encode/decode | Dynamic (dev), Static (release) | `src/j2k/` |
| libjpeg-turbo | JPEG encode/decode | Dynamic (dev), Static (release) | `src/jpeg/` |
| libtiff | TIFF read/write | Dynamic (dev), Static (release) | `src/tiff/` |

Each library has a `sys.rs` module containing raw `#[repr(C)]` struct definitions and
`extern "C"` function declarations. These must exactly match the C headers for the
linked library version. A `ffi.rs` module provides safe Rust wrappers over the raw
bindings.

## Why Custom FFI Bindings

We use hand-written FFI bindings rather than `-sys` crates for two reasons:

1. **License compliance.** Some `-sys` crates (e.g., `openjpeg-sys`, `openjpeg2-sys`)
   have licenses incompatible with this project's requirements. Custom bindings avoid
   transitive license contamination.

2. **Control over linking.** The release workflow compiles C libraries from source and
   statically links them. This requires precise control over link directives, search
   paths, and force-load flags that `-sys` crates don't provide.

The tradeoff is that we own the correctness of the struct definitions. There is no
bindgen or automated tool keeping them in sync with the C headers.

## Pinned Library Versions

Each C library is pinned to a specific version. The pinned versions appear in two
places:

1. **`environment.yml`** — conda packages for local development (dynamic linking).
2. **`.github/workflows/release.yml`** — source archive versions for CI release builds
   (static linking).

These must be kept in sync. If the conda environment has a different version than the
release workflow, the FFI structs may match one but not the other.

### Current Pinned Versions

| Library | Version | ABI Notes |
|---------|---------|-----------|
| OpenJPEG | 2.5.3 | Stable public API. Struct layouts have been stable across 2.x releases but must be verified on major bumps. |
| libjpeg-turbo | 3.1.0 | `JPEG_LIB_VERSION = 80`. The ABI version controls which fields exist in `jpeg_compress_struct` and `jpeg_decompress_struct`. See below. |
| libtiff | 4.7.0 | Uses opaque `TIFF*` handles. Only `TIFFFieldInfo` is a public `#[repr(C)]` struct. Low risk of layout changes. |

## The libjpeg ABI Version Problem

libjpeg-turbo has **three separate version numbers** that measure different things.
Understanding the distinction is critical for FFI correctness.

### 1. libjpeg-turbo release version (e.g., 3.1.0)

This is the project's own version, following semantic versioning. Major version bumps
(2.x → 3.0) can include breaking changes to the TurboJPEG API. Minor and patch
releases within a major version are backward compatible. This is the version pinned
in `release.yml` and `environment.yml`.

Bumping this version (e.g., 3.0.0 → 3.1.0) does **not** change struct layouts.

### 2. `JPEG_LIB_VERSION` — the emulated ABI version (62, 70, or 80)

This is the ABI version of the **original IJG libjpeg** that libjpeg-turbo pretends
to be. It is a **compile-time choice** made by whoever builds the library, controlled
by CMake flags:

| Build flag | `JPEG_LIB_VERSION` | Emulates |
|------------|-------------------|----------|
| (default) | 62 | IJG libjpeg v6b (1998) |
| `-DWITH_JPEG7=1` | 70 | IJG libjpeg v7 (2009) |
| `-DWITH_JPEG8=1` | 80 | IJG libjpeg v8 (2010) |

The numbers 62, 70, 80 are not semver — they are opaque integers from the original
IJG libjpeg project. Each value defines a different struct layout for
`jpeg_compress_struct` and `jpeg_decompress_struct`, with fields inserted in the
middle (not appended), so every field after the insertion point shifts.

This value does **not** change when you bump libjpeg-turbo from 3.0 to 3.1. It only
changes if the builder passes a different `-DWITH_JPEG*` flag. This means:

- Bumping libjpeg-turbo 3.0 → 3.1 with the same build flags: **safe**, no struct
  layout changes.
- Switching from a package built without `-DWITH_JPEG8` to one built with it (or
  vice versa): **breaks struct layouts**, even at the same libjpeg-turbo version.

The dangerous scenario is not a version bump — it's a change in how the library is
**built**. This can happen silently if a conda-forge recipe changes its build flags.

### 3. Shared library SO version (e.g., libjpeg.so.62, libjpeg.so.8)

The dynamic linker's ABI version, derived from `JPEG_LIB_VERSION`. It prevents
loading a library with the wrong struct layout at runtime. With static linking (our
release builds), this protection does not exist.

### What about libjpeg v9?

IJG libjpeg v9 added another field (`color_transform`) to the compress struct,
making it ABI-incompatible with v8. libjpeg-turbo does not emulate v9 because the
feature it supports (lossless SmartScale) has not seen sufficient adoption. There is
no `-DWITH_JPEG9` flag.

### Our configuration

Our conda environment and release builds use libjpeg-turbo with `-DWITH_JPEG8=1`,
giving `JPEG_LIB_VERSION = 80`. The Rust `sys.rs` constant and struct definitions
must match this.

### 12-bit JPEG is orthogonal

12-bit sample precision is a separate feature, not related to `JPEG_LIB_VERSION`.
The same `jpeg_compress_struct` and `jpeg_decompress_struct` are used for 8-bit and
12-bit operations — the `data_precision` field inside the struct controls the bit
depth. libjpeg-turbo 2.2+ provides separate symbol names for different precisions
(`jpeg_read_scanlines` for 8-bit, `jpeg12_read_scanlines` for 12-bit) but the struct
layout is identical regardless of precision.

## FFI Surface Area by Library

### OpenJPEG (`src/j2k/sys.rs`)

Six `#[repr(C)]` structs cross the FFI boundary:

- `opj_image_comp_t` (64 bytes) — image component data
- `opj_image_t` (48 bytes) — image container
- `opj_image_cmptparm_t` (36 bytes) — component creation parameters
- `opj_poc_t` (148 bytes) — progression order change
- `opj_dparameters_t` (8,252 bytes) — decompression parameters
- `opj_cparameters_t` (18,720 bytes) — compression parameters

The codec, stream, and codestream info/index types are opaque (zero-sized).

**Risk level: High.** The parameter structs are large and allocated on the Rust side,
then passed to `opj_set_default_encoder_parameters` / `opj_set_default_decoder_parameters`
which write the full struct. A size mismatch causes stack corruption. This was the root
cause of the J2K encode segfault (see `docs/internal/BUG_STATIC_J2K_ENCODE_SEGFAULT.md`).

### libjpeg-turbo (`src/jpeg/sys.rs`)

Two API surfaces:

1. **TurboJPEG API** — uses opaque `tjhandle` (`*mut c_void`). No struct layout risk.
   This is what the codebase currently uses for all 8-bit JPEG operations.

2. **libjpeg API** — uses `jpeg_compress_struct` (584 bytes at version 80) and
   `jpeg_decompress_struct` (656 bytes at version 80). These are allocated on the Rust
   side and passed to `jpeg_CreateCompress` / `jpeg_CreateDecompress`. Currently unused
   (12-bit JPEG is stubbed out) but the struct definitions exist and must be correct
   for when 12-bit support is implemented.

Supporting structs (`JQUANT_TBL`, `JHUFF_TBL`, `jpeg_component_info`, `jpeg_error_mgr`,
`jpeg_destination_mgr`, `jpeg_source_mgr`) are embedded in or pointed to by the main
structs and must also be correct.

**Risk level: Medium.** Currently dormant because only TurboJPEG is used. Becomes high
when 12-bit JPEG support is enabled.

### libtiff (`src/tiff/sys.rs`)

One `#[repr(C)]` struct: `TIFFFieldInfo` (24 bytes), used for custom tag registration.

All other libtiff interaction uses opaque `*mut c_void` handles and variadic
`TIFFGetField` / `TIFFSetField` calls.

**Risk level: Low.** Small struct, stable layout, minimal surface area.

## Compile-Time Size Assertions

Every `#[repr(C)]` struct in a `sys.rs` file must have a compile-time size assertion:

```rust
// Compile-time verification that Rust struct sizes match C struct sizes.
// If any of these fail, the struct definition is out of sync with the C header.
const _: () = assert!(std::mem::size_of::<opj_poc_t>() == 148);
const _: () = assert!(std::mem::size_of::<opj_cparameters_t>() == 18720);
const _: () = assert!(std::mem::size_of::<opj_dparameters_t>() == 8252);
// ... etc
```

These assertions are zero-cost (evaluated at compile time) and will cause a build
failure if a struct definition drifts from the expected size. They do not verify field
offsets, but a size mismatch is a strong signal that something is wrong.

## Verification Procedure

When adding or modifying FFI struct definitions, or when updating a C library version:

### 1. Write a C verification program

For each library, write a small C program that prints `sizeof()` and `offsetof()` for
every struct and every field. Compile it against the actual installed headers:

```c
#include <stdio.h>
#include <stddef.h>
#include <openjpeg-2.5/openjpeg.h>  // or jpeglib.h, tiffio.h

#define PRINT_SIZEOF(type) printf("sizeof(%s) = %zu\n", #type, sizeof(type))
#define PRINT_OFFSET(type, field) printf("  offsetof(%s, %s) = %zu\n", \
    #type, #field, offsetof(type, field))

int main(void) {
    PRINT_SIZEOF(opj_cparameters_t);
    PRINT_OFFSET(opj_cparameters_t, tile_size_on);
    // ... all fields
}
```

### 2. Write a Rust verification program

Write an equivalent Rust program that prints `std::mem::size_of` and field offsets
using `std::ptr::addr_of!` on a null pointer:

```rust
macro_rules! print_offset {
    ($t:ty, $field:ident) => {
        let base = std::ptr::null::<$t>();
        let offset = unsafe { std::ptr::addr_of!((*base).$field) } as usize;
        println!("  offsetof({}, {}) = {}", stringify!($t), stringify!($field), offset);
    };
}
```

### 3. Compare outputs

Every `sizeof` and `offsetof` value must match exactly between C and Rust. Any
discrepancy indicates a missing field, wrong field type, or wrong field order.

### 4. Update compile-time assertions

After fixing any mismatches, update the `const _: () = assert!(...)` lines to reflect
the correct sizes.

## Process for Updating a C Library Version

Updating a pinned C library version is **not** a simple dependency bump. It requires
verifying that the FFI contract is preserved. Follow this procedure:

### 1. Read the changelog

Check the library's release notes for:
- Struct layout changes (added/removed/reordered fields)
- ABI version bumps
- Removed or renamed API functions
- New required dependencies

### 2. Update the version in both places

- `environment.yml` (conda, for local development)
- `.github/workflows/release.yml` (source build, for release wheels)

### 3. Rebuild the conda environment

```bash
conda env update -f environment.yml
```

### 4. Run the verification procedure

Follow the steps in "Verification Procedure" above for the updated library. Compare
struct sizes and field offsets against the Rust definitions.

### 5. Fix any mismatches

Update the `#[repr(C)]` struct definitions in `sys.rs` to match the new headers.
Update compile-time size assertions.

### 6. Run the full test suite

```bash
cargo test
maturin develop
pytest
```

### 7. Test the static build

If possible, test a static release build to catch optimization-sensitive issues:

```bash
# See release.yml for the full static build procedure
maturin build --release --features static
```

### Special considerations for libjpeg-turbo

When updating libjpeg-turbo, the release version bump itself (e.g., 3.0 → 3.1) is
unlikely to change struct layouts. The real risk is a change in the **emulated ABI
version** (`JPEG_LIB_VERSION`), which is controlled by build flags, not by the
libjpeg-turbo version number. Check:

- What `JPEG_LIB_VERSION` the new package reports (check `jconfig.h` after install)
- Whether the conda-forge recipe or release workflow CMake flags changed the
  `-DWITH_JPEG8` setting
- That the `JPEG_LIB_VERSION` constant in `sys.rs` matches the library's value
- That the struct definitions include all fields for the active ABI version

A mismatch between the `JPEG_LIB_VERSION` in `sys.rs` and the library's compiled
value will cause `jpeg_CreateCompress` / `jpeg_CreateDecompress` to reject the call
(best case) or silently corrupt memory (worst case).

## Historical Bugs

| Bug | Library | Root cause | Impact |
|-----|---------|------------|--------|
| J2K encode segfault | OpenJPEG | `opj_poc_t` was 80 bytes in Rust vs 148 in C. Cascaded into `opj_cparameters_t` being 2,176 bytes too small. | Stack corruption, segfault in static release builds. |
| JPEG struct mismatch | libjpeg-turbo | Structs written for `JPEG_LIB_VERSION = 62` but library is version 80. `jpeg_compress_struct` is 64 bytes too small. | Currently dormant (TurboJPEG API used). Will cause corruption when 12-bit JPEG is enabled. |

Both bugs were caused by Rust struct definitions that did not match the actual C
headers. The J2K bug was caught by a segfault in the static build. The JPEG bug was
caught by proactive verification before it caused runtime failures.
