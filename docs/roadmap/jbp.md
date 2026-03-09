# JBP Implementation Roadmap

This roadmap addresses gaps in CLEVEL conformance identified in the JBP CLEVEL Assessment (see `internal/JBP_CLEVEL_ASSESSMENT.md`), ordered by priority and dependency.

## Phase 1: Graphic Segments ✅ Complete

**Objective**: Parse and write CGM graphic segments with display layering.

**Scope**:
- ✅ Graphic subheader parsing/generation (Table 5.15-1)
- ✅ Display level (SDLVL) and attachment level (SALVL) handling
- ✅ Bounding box calculation and validation
- ✅ CGM data extraction (parsing CGM content is optional)
- ⚠️ Aggregate size validation per CLEVEL (optional, not implemented)

**JBP Requirements**: JBP-2021.2-081 to JBP-2021.2-086

**Validation**: JITC `Segments/Test Files/NITF_SYM_POS_*.ntf`

**Implementation Notes**:
- `GraphicsAssetProvider` trait implemented with full `AssetProvider` interface
- `JBPGraphicsAssetProvider` provides access to CGM data via `raw_asset()`
- Graphic metadata (SDLVL, SALVL, SLOC, SBND1, SBND2) accessible via `metadata().as_dict()`
- Python bindings available through `PyGraphicsAssetProvider`
- See [API Design](../design/api-design.md) for usage examples

## Phase 2: Text Segments ✅ Complete

**Objective**: Parse and write text segments with attachment levels.

**Scope**:
- ✅ Text subheader parsing/generation (Table 5.17-1)
- ✅ Text format code handling (STA, MTF, UT1, U8S)
- ✅ Attachment level handling (TXTALVL)
- ✅ Text data extraction with encoding-aware decoding
- ✅ Line delimiter normalization (CR/LF to platform-native)
- ✅ BufferedTextAssetProvider for programmatic text creation

**JBP Requirements**: Section 5.17

**Validation**: JITC `Segments/Test Files/NITF_TXT_POS_*.ntf`

**Implementation Notes**:
- `TextAssetProvider` trait implemented with `text()`, `encoding()`, and `format()` methods
- `JBPTextAssetProvider` provides access to decoded text content via `text` property
- Text metadata (TEXTID, TXTALVL, TXTDT, TXTITL, TXTFMT) accessible via `metadata().as_dict()`
- `BufferedTextAssetProvider` enables programmatic text segment creation
- Python bindings available through `PyTextAssetProvider`
- See [API Design](../design/api-design.md) for usage examples

## Phase 3: JPEG DCT Compression (IC=C3/M3/I1) ✅ Complete

**Objective**: Read and write JPEG DCT compressed imagery.

**Scope**:
- ✅ JPEG DCT decoding/encoding (8-bit)
- ❌ 12-bit JPEG (not supported, see limitation below)
- ✅ Monochrome and RGB support
- ✅ YCbCr601 color space handling
- ✅ Multiband individual JPEG (IMODE=B,S)
- ✅ Downsampled JPEG (I1) for thumbnails
- ✅ COMRAT parsing and generation
- ✅ Masked JPEG (M3) support

**JBP Requirements**: Table 5.13-5, Section 6.1.6

**Dependencies**: libjpeg-turbo FFI bindings

**Implementation Notes**:
- `JpegBlockDecoder` and `JpegBlockEncoder` implemented using libjpeg-turbo FFI
- COMRAT quality factor (00.0-99.9) maps to JPEG quality parameter (1-100)
- IC=I1 enforces 2048×2048 maximum dimension constraint
- 12-bit JPEG is not supported due to libjpeg-turbo architectural constraints (TurboJPEG API only supports 8-bit samples)
- For 12-bit imagery, use JPEG 2000 (IC=C8) or uncompressed (IC=NC)
- See `internal/JBP_CLEVEL_ASSESSMENT.md` for details on the 12-bit limitation
- Property-based tests validate lossy roundtrip quality (PSNR >= 30 dB, SSIM >= 0.95)

## Phase 4: Vector Quantization (IC=C4/M4)

**Objective**: Read VQ compressed imagery.

**Scope**:
- VQ decoding with 4×4 kernel and 4 tables
- Monochrome and RGB/LUT variants
- Masked variant (M4) support

**JBP Requirements**: Table 5.13-5

**Notes**: VQ is a legacy format; read-only support is sufficient. Note this as a limitation in docs.

## Phase 5: Optional Compression Formats

**Objective**: Support optional compression formats for specialized use cases.

**Scope** (implement as needed):
- JPEG Lossless (C5/M5) - Lossless compression for archival
- Bi-Level (C1/M1) - 1-bit imagery (maps, line drawings)
- ZLIB (CC/MC) - Floating-point scientific data
- SARZip (C7/M7) - SAR complex/magnitude data

**Priority**: Implement based on user requirements. ZLIB and SARZip are most relevant for scientific/SAR applications.

## Phase 6: Special Data Types

**Objective**: Complete support for non-display imagery types.

**Scope**:
- Polar coordinates (IREP=POLAR) - 2-band vector data
- Location grids (ICAT=LOCG) - Geolocation support data
- GEOSDE TRE for elevation/location data
- Matrix data validation improvements

**JBP Requirements**: Table G-1 special data sections
