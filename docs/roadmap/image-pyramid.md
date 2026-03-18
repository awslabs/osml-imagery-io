# Image Pyramid (Multi-Resolution Overview) Roadmap

```{warning}
This roadmap is **incomplete**. The API design, resampling algorithm selection, and format-specific integration details require further work before implementation can begin. Sections marked with TODO need input from the team.
```

```{todo}
- Finalize the `ImagePyramid` / `OverviewProvider` trait design and its relationship to `ImageAssetProvider`
- Define the resampling kernel database format and default kernel set
- Decide on JPEG2000 decomposition level handling (decode-from-codestream vs. external RRDS)
- Specify quality metrics and verification tolerances for generated overviews
- Design the writer-side API for on-demand pyramid generation
- Evaluate whether intra-octave (non-power-of-two) scaling belongs in this feature or is a separate concern
```

## Motivation

Many geospatial image formats support multi-resolution representations — reduced-resolution copies of the full image that enable fast display at various zoom levels. These representations go by different names depending on the format:

- **TIFF/GeoTIFF**: Overview IFDs identified by `NewSubfileType` bit 0 = 1, each a separate tiled image at reduced dimensions
- **Cloud Optimized GeoTIFF (COG)**: Overviews stored as additional IFDs after the full-resolution IFD, ordered by descending resolution
- **JPEG2000 (NITF)**: Wavelet decomposition levels embedded in the codestream, accessible without full decode
- **NITF (non-J2K)**: External or embedded Reduced Resolution Data Sets (RRDS) as defined by SIPS

Despite the different storage mechanisms, the user-facing concept is the same: an image pyramid consisting of a full-resolution image (R0) and a series of progressively smaller images (R1, R2, ..., Rn), where each level is approximately half the dimensions of the previous level.

This roadmap addresses image pyramid support as a cross-format feature rather than a TIFF-specific concern.

## Key Distinction: Pyramid vs. Block-Level Resolution

A previous iteration of this design placed multi-resolution access on the `ImageAssetProvider` via `get_block(row, col, resolution_level)`. That approach conflates two different concepts:

1. **Block access at a single resolution**: `get_block(row, col)` retrieves one tile from a specific image. The row/col coordinates refer to the tile grid of that image.
2. **Image pyramid navigation**: Selecting which resolution level to work with. Each level is a complete tiled image with its own dimensions, tile grid, and pixel data.

The `resolution_level` parameter on `get_block` implies that a single block can be retrieved at different resolutions, but that is not how image pyramids work. Each overview level is a distinct image with its own tile grid. A block at position (3, 5) in the R0 image covers a completely different geographic extent than block (3, 5) in the R2 image.

The correct model is a pyramid of images, where each level exposes the full `ImageAssetProvider` interface (dimensions, tile grid, block access, metadata). The pyramid itself is a higher-level construct that organizes these levels.

## Format-Specific Storage

### TIFF / GeoTIFF / COG

TIFF stores each overview as a separate IFD (Image File Directory). Each overview IFD:
- Has `NewSubfileType` tag with bit 0 set (reduced-resolution image)
- Contains its own `ImageWidth`, `ImageLength`, `TileWidth`, `TileLength` tags
- Is independently tiled and compressed
- For COG files, overview IFDs follow the full-resolution IFD in descending resolution order

Reading overviews means navigating to the appropriate IFD via `TIFFSetDirectory()` and reading tiles from that directory. Each overview IFD is a self-contained tiled image.

### JPEG2000 (NITF IC=C8)

JPEG2000 embeds multiple resolution levels within the compressed codestream via wavelet decomposition. A J2K codestream with N decomposition levels provides N+1 resolution levels (R0 through RN) without storing separate images. The decoder can extract any resolution level directly from the codestream.

This is fundamentally different from TIFF overviews: the reduced-resolution images are not stored as separate entities but are an inherent property of the wavelet transform. OpenJPEG's `opj_set_decoded_resolution_factor()` controls which level is decoded.

For NITF files with J2K compression, the SIPS standard (Section 2.2.5.2) specifies that if the lowest J2K decomposition level is still larger than 512×512 pixels, additional uncompressed RRDS levels should be generated beyond what the codestream provides.

### NITF (Non-J2K Compression)

For NITF images using other compression schemes (VQ, JPEG, uncompressed), reduced resolution data sets are generated externally using the RRDS algorithm defined in SIPS Section 2.2. These are stored as separate image segments or companion files.

## SIPS RRDS Generation Algorithm

The Softcopy Image Processing Standard (NGA.STND.0014, Version 2.4) defines the reference algorithm for generating reduced resolution data sets. The key properties of this algorithm are relevant to any format's pyramid generation.

### Algorithm Overview

The RRDS algorithm (SIPS Section 2.2) generates a series of reduced-resolution images from a full-resolution source (R0). Each level Rn is formed from level Rn-1 (not directly from R0) through a three-step process applied independently to each band:

1. **Tonal remapping** (if applicable): Reverse any tonal mappings (e.g., PEDF or Lin-Log) before spatial processing
2. **Anti-aliasing**: Convolve with an anti-aliasing kernel to remove high spatial frequency content that would alias at the reduced resolution
3. **Interpolation**: Correlate with a spatial interpolation kernel (LaGrange interpolation coefficients) to compute sub-pixel values at the target sample positions
4. **Downsample**: Reduce dimensions by a factor of 2 in both row and column directions

### Downsampling Rules

- Even dimension: divide by 2
- Odd dimension: divide by 2, round to nearest integer (0.5 rounds up)
- Dimensions of level Rn depend on dimensions of level Rn-1, not on R0 directly

### Stopping Criterion

Generation stops when the smallest dimension of the output level is >= 256 and < 512 pixels. If the R0 image has either dimension < 512, the RRDS consists of only R0 (no overviews generated).

### Asymmetric Pixel Handling

When the R0 image has non-square pixels (e.g., 1.5:1 aspect ratio), the R0→R1 step corrects the aspect ratio to produce symmetric pixels. All subsequent levels (R2, R3, ...) operate in symmetric pixel space. This requires spatially-variant interpolation kernels during the R0→R1 step.

### Implementation Options

SIPS Section 2.2.2 notes that alternative interpolation algorithms may be used:
- High-order interpolation (cubic, modified sinc) for most electro-optical imagery
- Max-Pixel decimation for colorized change composite products (SIPS Section 2.10)

### Quality Considerations

The SIPS algorithm is more computationally expensive than simple averaging or nearest-neighbor downsampling, but produces higher quality results by properly handling anti-aliasing before subsampling. Naive downsampling without anti-aliasing introduces aliasing artifacts that degrade image quality at reduced zoom levels.


## Proposed API Design

```{todo}
The API design below is a starting point. It needs review and refinement before implementation.
```

### Reading: Pyramid Access

The current `ImageAssetProvider` trait exposes a single image's tile grid. Pyramid support should not overload this interface. Instead, a separate concept provides access to the collection of resolution levels:

```rust
/// A collection of resolution levels forming an image pyramid.
/// Each level is a complete ImageAssetProvider with its own
/// dimensions, tile grid, and pixel data.
pub trait ImagePyramidProvider {
    /// Number of resolution levels available (including full resolution).
    /// Level 0 is always the full-resolution image.
    fn num_levels(&self) -> usize;

    /// Access a specific resolution level as an ImageAssetProvider.
    /// Level 0 = full resolution, level 1 = 2× reduction, etc.
    fn level(&self, level: usize) -> Result<&dyn ImageAssetProvider>;

    /// Dimensions (width, height) at a given level.
    fn level_dimensions(&self, level: usize) -> Result<(u32, u32)>;
}
```

This design means:
- Each resolution level has its own tile grid — `get_block(row, col)` on a level's `ImageAssetProvider` uses that level's tile coordinates
- The pyramid provider manages the mapping between levels and their underlying storage (TIFF IFDs, J2K decomposition levels, external RRDS files)
- Format-specific readers implement `ImagePyramidProvider` using their native overview mechanism

### Writing: Pyramid Generation

```{todo}
Define the writer-side API. Key questions:
- Should pyramid generation be automatic (encoding hint) or explicit (separate method)?
- How does the caller specify resampling algorithm and anti-aliasing kernel?
- Should we support generating pyramids for formats that don't natively store them (e.g., writing RRDS for non-J2K NITF)?
```

Encoding hints for pyramid generation (preliminary):

| Hint | Description | Example Values |
|------|-------------|----------------|
| `Overviews` | Whether to generate overview levels | `true`, `false` (default: `false`) |
| `OverviewResampling` | Resampling algorithm | `average`, `nearest`, `lanczos`, `sips` (default: `average`) |
| `OverviewLevels` | Number of overview levels to generate | `auto` (default), or explicit count |

When `OverviewResampling` is set to `sips`, the implementation should follow the SIPS RRDS generation algorithm (anti-alias filtering + LaGrange interpolation + downsample). Other resampling modes provide simpler alternatives for cases where SIPS-level quality is not required.

## Format Integration

### TIFF

- `ifd.rs` — IFD navigation: detect overview IFDs by `NewSubfileType` bit 0, build level-to-IFD mapping, validate dimensions follow 2× reduction pattern
- `TIFFImagePyramidProvider` wraps a full-resolution IFD and its associated overview IFDs
- Writer generates overview IFDs with `NewSubfileType = 1`, each independently tiled

### COG

- COG files are tiled GeoTIFFs with overviews in a specific IFD order — the TIFF pyramid provider handles them naturally
- COG validation checks that overviews are present and correctly ordered

### JPEG2000 / NITF

- J2K decomposition levels are accessed via `opj_set_decoded_resolution_factor()` — each level produces a complete image that can be wrapped as an `ImageAssetProvider`
- For NITF files where J2K decomposition levels are insufficient (lowest level > 512×512), additional RRDS levels may need to be generated per SIPS Section 2.2.5.2

```{todo}
Determine whether we generate additional RRDS levels beyond J2K decomposition at read time, write time, or leave it to the caller.
```

## Resampling Algorithms

```{todo}
Define the resampling kernel library. At minimum, support:
- Nearest-neighbor (fast, no anti-aliasing)
- Box average (simple 2×2 averaging)
- Lanczos (high quality, windowed sinc)
- SIPS-compliant (anti-alias convolution + LaGrange interpolation, per SIPS Section 2.2.5)

The SIPS algorithm requires:
- Anti-aliasing convolution kernels (7×7, per SIPS Table 2.2)
- LaGrange interpolation kernels (4×4, per SIPS Table 2.3)
- Spatially-variant kernels for asymmetric pixel correction

Kernel databases and their file format need to be specified.
```

## Testing Plan

```{todo}
Define property-based and unit tests for pyramid support:
- Roundtrip: write image with overviews, read back, verify each level's dimensions and pixel content
- Level dimensions follow 2× reduction pattern
- Block access at each level returns correctly-sized tiles
- Cross-format consistency: pyramid generated for TIFF vs. NITF/J2K should produce equivalent results for the same resampling algorithm
- SIPS compliance: verify generated RRDS matches SIPS reference output for known test inputs (SIPS Section 2.2.7)
```

## Reference Materials

- **SIPS** (NGA.STND.0014, Version 2.4): Section 2.2 — Reduced Resolution Dataset Generation (algorithm details, anti-aliasing kernels, LaGrange interpolation, asymmetric pixel handling, JPEG2000 considerations, verification strategies). Section 2.7 — Scaling (intra-octave scaling between R-levels).
- **TIFF 6.0**: `NewSubfileType` tag definition, IFD structure
- **OGC COG Standard**: IFD ordering requirements for overviews
- **ISO/IEC 15444-1 (JPEG2000)**: Wavelet decomposition levels and resolution scalability
