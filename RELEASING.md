# Releasing

## How to Cut a Release

1. Update the version in both `Cargo.toml` and `pyproject.toml` (they must match).
2. Commit the version bump.
3. Tag the commit with a `v` prefix:
   ```bash
   git tag v0.2.0
   ```
4. Push the tag:
   ```bash
   git push origin v0.2.0
   ```

Pushing the tag triggers the release workflow (`.github/workflows/release.yml`).

## What the Release Workflow Does

1. Compiles OpenJPEG, libjpeg-turbo, and libtiff from source as static libraries for each target platform.
2. Builds abi3 wheels (Python 3.9+ stable ABI) for four platforms using `maturin` with the `static` feature flag.
3. Builds an sdist (source distribution) as a fallback for platforms without pre-compiled wheels.
4. Publishes all wheels and the sdist to PyPI via OIDC trusted publishing.

No API tokens are used. Authentication is handled by PyPI's OIDC trusted publisher mechanism.

## Supported Platforms

| Platform | Runner | Target Triple |
|----------|--------|---------------|
| Linux x86_64 | ubuntu-latest (manylinux_2_28) | x86_64-unknown-linux-gnu |
| Linux aarch64 | ubuntu-latest (manylinux_2_28 + QEMU) | aarch64-unknown-linux-gnu |
| macOS x86_64 | macos-13 | x86_64-apple-darwin |
| macOS arm64 | macos-14 | aarch64-apple-darwin |

## PyPI Trusted Publisher Setup

This is a one-time manual configuration on [pypi.org](https://pypi.org):

1. Go to **Your projects** → **osml-imagery-io** → **Publishing**.
2. Add a new trusted publisher:
   - Owner: `awslabs`
   - Repository: `osml-imagery-io`
   - Workflow: `release.yml`
   - Environment: `release`
3. Save.

Until this is configured, the publish step will fail with an authentication error.

## Pinned C Library Versions

The release workflow pins specific versions of each native C library for reproducible builds. These are defined as environment variables at the top of `release.yml`:

| Library | Version | Source |
|---------|---------|--------|
| OpenJPEG | 2.5.3 | [github.com/uclouvain/openjpeg](https://github.com/uclouvain/openjpeg) |
| libjpeg-turbo | 3.1.0 | [github.com/libjpeg-turbo/libjpeg-turbo](https://github.com/libjpeg-turbo/libjpeg-turbo) |
| libtiff | 4.7.0 | [download.osgeo.org/libtiff](https://download.osgeo.org/libtiff/) |

### Updating Library Versions

> **Warning:** Updating a C library version is not a simple dependency bump. These
> libraries are linked via hand-written FFI bindings (`src/*/sys.rs`). A version
> change can alter struct layouts, add or remove fields, or change ABI versions,
> causing silent memory corruption. See `docs/design/native-library-ffi.md` for the
> full procedure and background.

To bump a C library version:

1. Read the library's changelog for struct layout changes, ABI version bumps, or
   removed API functions.
2. Update the corresponding `env` variable in `.github/workflows/release.yml` (e.g.,
   `OPENJPEG_VERSION`, `LIBJPEG_TURBO_VERSION`, `LIBTIFF_VERSION`).
3. Update the matching package version in `environment.yml` so local development
   uses the same version.
4. Rebuild the conda environment: `conda env update -f environment.yml`.
5. Run the FFI struct verification procedure described in
   `docs/design/native-library-ffi.md` to confirm all `sizeof()` and `offsetof()`
   values match between C and Rust.
6. Fix any struct mismatches in `src/*/sys.rs` and update compile-time size assertions.
7. Run the full test suite (`cargo test`, `maturin develop`, `pytest`).
8. Test by pushing a tag to a fork or using `workflow_dispatch` if enabled.
9. Commit the version bump together with any FFI struct changes.

For libjpeg-turbo specifically, check `JPEG_LIB_VERSION` in the new version's
`jconfig.h` — this controls which fields exist in the compress/decompress structs.
See `docs/design/native-library-ffi.md` for details.
