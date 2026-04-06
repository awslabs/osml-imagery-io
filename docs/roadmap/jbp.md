# JBP Implementation Roadmap

This roadmap addresses gaps in CLEVEL conformance and known limitations in the JBP implementation.

## Vector Quantization (IC=C4/M4)

**Objective**: Read VQ compressed imagery.

**Scope**:
- VQ decoding with 4×4 kernel and 4 tables
- Monochrome and RGB/LUT variants
- Masked variant (M4) support

**JBP Requirements**: Table 5.13-5

**Specification**: MIL-STD-188-199 (35 pages, available in `reference-materials/JBP/`)

**Complexity**: Low-moderate. VQ is a codebook-based lossy compression using 4×4 pixel blocks with up to 4 lookup tables embedded in the image data. Decompression is table lookup only — no complex math. COMRAT is expressed as bits-per-pixel (e.g., "1.00").

**Notes**: VQ is a legacy format found in older NGA/DoD imagery archives; read-only support is sufficient. No new VQ imagery is being created. Note this as a limitation in docs.

## Writer Support for Image Lookup Tables (IREP=RGB/LUT)

**Objective**: Allow users to write uncompressed NITF images with per-band lookup tables.

**Current State**: The reader correctly parses LUT data from the image subheader — `NLUTSn`, `NELUTn`, and `LUTDnm` fields are accessible through the `ImageSubheaderFacade` (`nluts()`, `nelut()`, `lut_data()`). The Rust-side infrastructure is in place: `LookUpTable` provides `from_bytes()`, `apply()`, and `as_bytes()` methods, the `ImageBandInfoBuilder` supports `add_lut()`, and validation enforces LUT constraints (e.g. `RGB/LUT` requires exactly 3 LUTs, `NELUTn >= 2^ABPP`). However, the writer's `create_image_subheader_with_tres()` hardcodes `NLUTS=0` for every band and does not handle `IREP=RGB/LUT` in the `IREPBAND` field mapping. Users cannot currently write images with lookup tables.

**Scope**:
- Update the writer's band info loop to check for LUT data on the asset's metadata and write `NLUTSn`, `NELUTn`, and `LUTDnm` fields when present
- Handle `IREP=RGB/LUT` in the `IREPBAND` mapping (should emit `LU` for the single band)
- Handle `IREP=MONO` and `IREP=MULTI` bands with `IREPBANDn=LU` and 1–2 LUTs
- Expose LUT configuration through `BufferedMetadataProvider` or a dedicated API so users can attach LUT data to an image before writing
- Validate LUT constraints at write time: `PVTYPE` must be `INT` or `B`, `NLUTSn` ≤ 3, `NELUTn` ≤ 65536, and LUT count must match `IREP` requirements

**JBP Requirements**: §5.13.2.28 (`NLUTSn`), §5.13.2.29 (`NELUTn`), §5.13.2.30 (`LUTDnm`)

**Complexity**: Low. The parsing, data structures, builder, and validation already exist. The work is wiring the writer's subheader serialization to use them instead of hardcoding zero, and providing a user-facing way to attach LUT data to an image asset.

**Notes**: LUTs are only valid for uncompressed images (`IC=NC` or `NM`). For JPEG and JPEG 2000, color handling is internal to the codec and `NLUTSn` is always 0. The VQ codec (C4/M4) has its own codebook-based color lookup defined in MIL-STD-188-199, which is separate from the subheader LUT fields.

## ZLIB Compression (IC=CC/MC)

**Objective**: Read and write ZLIB compressed imagery for floating-point scientific data.

**Scope**:
- ZLIB/DEFLATE decoding and encoding per IETF RFC 1950/1951
- Numerically lossless compression of floating-point values
- Each JBP image block is an independent ZLIB stream (DEFLATE with CMF.CM=8)
- LZ77 window size ≤ 32768 bytes (CMF.CINFO ≤ 7), no preset dictionaries (FLG.FDICT=0)
- Pixel data formatted as uncompressed (NC/NM) with big-endian byte ordering prior to compression
- Masked variant (MC) with block mask and/or pad pixel mask
- COMRAT expressed as achieved bits-per-pixel-per-band (4-char float, scanf "%04f")

**JBP Requirements**: Table 5.13-5, Section 5.12.4.3

**Specifications**: IETF RFC 1950 (ZLIB format), IETF RFC 1951 (DEFLATE)

**Complexity**: Low. This is standard DEFLATE compression. The `flate2` Rust crate provides a ready-made implementation. The JBP-specific work is just the per-block stream framing and COMRAT handling.

**Dependencies**: `flate2` crate (MIT/Apache-2.0 licensed)

## JPEG Lossless Compression (IC=C5/M5)

**Objective**: Read and write JPEG lossless compressed imagery for archival and scientific use cases.

**Scope**:
- JPEG lossless decoding/encoding (2 to 16-bit sample precision)
- Spatial prediction + Huffman or arithmetic entropy coding on residuals
- Monochrome and multiband support
- Masked variant (M5) with block mask
- COMRAT format: XX.Y where XX is image data type (00=General, 01=VIS, 02=IR, 03=SAR) and Y=0 for lossless

**JBP Requirements**: Table 5.13-5, Section 6.1.6

**Specification**: MIL-STD-188-198A (same standard as C3/M3 JPEG DCT, but lossless mode uses a different coding process — predictive coding rather than DCT). Also defined in ISO/IEC 10918-1.

**Complexity**: Moderate. This is not the same code path as the lossy JPEG DCT already implemented via libjpeg-turbo's TurboJPEG API. Options include: (a) using libjpeg-turbo's lower-level API which does support lossless JPEG internally, or (b) implementing from scratch since the algorithm (spatial prediction + entropy coding on residuals) is relatively straightforward. Useful for archival imagery where exact pixel preservation is required.

## Bi-Level Compression (IC=C1/M1)

**Objective**: Read bi-level (1-bit) compressed imagery.

**Scope**:
- ITU-T T.4 Group 3 fax encoding/decoding
- One-dimensional coding (1D) and two-dimensional coding (2DS standard, 2DH high resolution)
- 1-bit imagery for maps, line drawings, and scanned documents
- Masked variant (M1) with pad pixel mask
- COMRAT values: "1D", "2DS" (K=2), "2DH" (K=4)

**JBP Requirements**: Table 5.13-5, Section 6.1.6

**Specification**: ITU-T T.4 (1993.03) Amendment 2 07/2003, also referenced in MIL-STD-188-198A

**Complexity**: Low-moderate. This is essentially Group 3 fax encoding — a well-understood algorithm. Niche use case (1-bit imagery) but the algorithm is simple. Read-only support is likely sufficient.

## SARZip Compression (IC=C7/M7)

**Objective**: Read SARZip compressed SAR complex and magnitude imagery.

**Scope**:
- SARZip bit stream decoding per USAF.RDUCE-001
- Complex (I/Q) and detected magnitude SAR data
- Lossless mode (typically >2:1 compression for 16-bit IQ)
- Near-lossless mode with continuously adjustable quality
- Tiled access for parallel decoding and chipping
- C7: single JBP image block; M7: multiple JBP image blocks (masked)
- COMRAT expressed as bits-per-sample (N.NN for BPS < 10.0, NN.N for BPS ≥ 10.0)
- Optional Reed-Solomon forward error correction (FEC)

**JBP Requirements**: Table 5.13-5, Section 6.1.6

**Specification**: USAF.RDUCE-001 V1.0.0 (143 pages, available in `reference-materials/JBP/`). The algorithm is a custom pipeline of: linear predictive decoder, arithmetic decoder with predefined probability tables, code page decoder, LSB image extractor, bit unpacker, and small lossless graphic (SLG) decoder.

**Complexity**: High. This is a fully custom SAR-specific compression algorithm — not related to standard ZIP/DEFLATE despite the name. There is no third-party library available. Implementation requires building the entire decoder from the 143-page spec. A C++ reference implementation exists but is only available to US Government organizations and their contractors. Read-only support is likely sufficient given the niche audience (SAR exploitation workflows).

## Motion Imagery (MIE4NITF)

**Objective**: Support motion imagery segments in NITF files per NGA.STND.0044 MIE4NITF.

**Scope**:
- Motion imagery IMODE values (T, D, E, F, X, Z)
- Temporal dimension handling in spatiotemporal blocks
- H.264/AVC compression (IC=C9/M9) per ISO/IEC 14496-10
- H.265/HEVC compression (IC=CA/MA) per ISO/IEC 23008-2
- JPEG 2000 with time association (IC=CB/MB, CE/ME)
- Motion imagery CLEVELs (CL51, CL54, CL57) per MIE4NITF Table 27
- COMRAT for video codecs expressed as compression ratio (e.g., "12.3" = 12.3:1)

**JBP Requirements**: Section 5.12.4.1, Table 5.13-5, Section 6.1.6, Annex F.2

**Specifications**: NGA.STND.0044 MIE4NITF Version 1.3, ISO/IEC 14496-10 (H.264), ISO/IEC 23008-2 (H.265), BPJ2K01.20 (JPEG 2000 profile)

**Complexity**: High. Requires video codec integration (H.264/H.265), temporal block management, and motion-specific IMODE handling. Video codec support would likely use FFI bindings to established libraries (e.g., openh264 for H.264, or ffmpeg). License compatibility must be verified — openh264 is BSD-2-Clause but H.265/HEVC has patent considerations.

## GEOSDE TRE Structure Definitions

**Objective**: Create `.ksy` structure definition files for the 10 missing GEOSDE TREs defined in STDI-0002 Volume 1, Appendix P.

**Background**: The GEOSDE (Geographic Support Data Extensions) family of TREs provides geolocation, projection, accuracy, and sensor metadata for elevation data (ICAT=DTEM), location grids (ICAT=LOCG), and matrix data (ICAT=MATR). The pixel data for these image categories is already readable (uncompressed or J2K lossless), and the special IREP values (POLAR, NVECTOR, VPH) are already handled at the code level (see `src/jbp/image/types.rs`). The remaining gap is the missing TRE structure definitions needed to parse the metadata that accompanies these data types.

All GEOSDE TREs are optional per JBP CLEVEL Table G-1.

**Existing definitions** (4 of 14, in `data/structures/tre/`):
- `tre_geolob.ksy` — Geographic Location (Table P-3)
- `tre_geopsb.ksy` — Geographic Coordinate System (Table P-5)
- `tre_prjpsb.ksy` — Projection Parameter (Table P-6)
- `tre_maplob.ksy` — Map Projection Location (Table P-3a)

**Missing definitions** (10 TREs to create):

| TRE | Spec Table | Description |
|-----|-----------|-------------|
| GRDPSB | Table P-4 | Ground Reference Point / Grid Definition |
| REGPTB | Table P-7 | Registration Point (geographic coordinates) |
| REGPTC | Table P-8 | Registration Point (local coordinates) |
| BNDPLB | Table P-9 | Bounding Polygon (geographic coordinates) |
| BNDPLC | Table P-9a | Bounding Polygon (local coordinates) |
| ACCPOB | Table P-10 | Positional Accuracy (absolute/relative CE/LE) |
| ACCHZB | Table P-11 | Horizontal Accuracy |
| ACCVTB | Table P-12 | Vertical Accuracy |
| SNSPSB | Table P-13 | Sensor Parameters |
| SOURCB | Table P-14 | Source Description |

**Specification**: `reference-materials/JBP/STDI-0002-2024.1_2023-10-26/Vol-1-App P - GEOSDE.pdf` (435 pages). Field definitions are in the tables listed above. Many of these TREs contain conditional modules and repeated field groups — read the implementation notes sections carefully.

**Complexity**: Low-moderate per TRE. Each is a straightforward field-by-field definition, but some (SOURCB, SNSPSB) have conditional presence logic and nested repeat groups that require careful reading of the spec. The total volume of work is moderate given 10 TREs.

**Notes**:
- FACCBB (Table P-15, Feature Attribute Coding Catalog) was listed in the appendix but is rarely encountered in practice; defer unless needed.
- Follow the existing `.ksy` format conventions established by `tre_geolob.ksy` and other definitions in `data/structures/tre/`.
- Each TRE definition should include doc comments referencing the specific STDI-0002 table number.

## CLEVEL Conformance Status

The JBP defines Complexity Levels (CLEVELs) to enable implementation across hardware platforms with varying resources. CLEVELs constrain file features like image dimensions, file size, number of segments, and supported compression formats. See JBP Section 5.20, Annex G (Table G-1).

### CLEVEL Overview

| CLEVEL | CCS Extent | Max File Size | Max Image Size | Max Bands | Image Segments | DES |
|--------|------------|---------------|----------------|-----------|----------------|-----|
| 03 | 2048×2048 | 50 MB | 2048×2048 | 9 | 0-20 | 0-10 |
| 05 | 8192×8192 | 1 GB | 8192×8192 | 255 | 0-20 | 0-50 |
| 06 | 65536×65536 | 2 GB | 65536×65536 | 999 | 0-100 | 0-100 |
| 07 | 99999999×99999999 | 10 GB | 99999999×99999999 | 999 | 0-100 | 0-100 |
| 09 | Unrestricted | Unrestricted | Unrestricted | >999 | >100 | >100 |

### Feature Support Matrix

Legend: ✅ Implemented, ⚠️ Partial, ❌ Not implemented, Req = Required, Opt = Optional

#### File Structure & Core Features

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| NITF 2.1 File Header | Req | ✅ | Full header parsing and generation |
| NSIF 1.0 File Header | Req | ✅ | Variant support included |
| Security Fields | Req | ✅ | All classification fields supported |
| TRE Support | Req | ✅ | Generic TRE parsing with field definitions |
| DES Support | Req | ✅ | Data Extension Segments supported |
| TRE_OVERFLOW DES | Req | ✅ | Overflow handling implemented |
| RES Support | — | ❌ | No RES currently approved by NTB |

#### Uncompressed Imagery (IC=NC/NM)

| Feature | CL03 | CL05 | CL06 | CL07 | Status |
|---------|------|------|------|------|--------|
| Monochrome (1,8,12,16,32,64-bit) | Req | Req | Req | Req | ✅ |
| RGB/LUT (1,8-bit with LUT) | Req | Req | Req | Req | ✅ |
| RGB 3-band (8-bit) | Req | — | — | — | ✅ |
| RGB 3-band (8,16,32-bit) | — | Req | Req | Req | ✅ |
| Multiband 2-9 bands | Req | — | — | — | ✅ |
| Multiband 2-255 bands | — | Req | — | — | ✅ |
| Multiband 2-999 bands | — | — | Req | Req | ✅ |
| IMODE B, P, R, S | Req | Req | Req | Req | ✅ |
| Image Blocking | Req | Req | Req | Req | ✅ |
| Image Data Mask (NM) | Req | Req | Req | Req | ✅ |

#### JPEG 2000 Compression (IC=C8/M8/CD/MD)

| Feature | CL03 | CL05 | CL06 | CL07 | Status |
|---------|------|------|------|------|--------|
| Monochrome 1-32 bit | Req | Req | Req | Req | ✅ |
| RGB/LUT 1-32 bit | Req | Req | Req | Req | ✅ |
| RGB 3-band | Req | Req | Req | Req | ✅ |
| YCbCr601 3-band | Req | Req | Req | Req | ⚠️ No internal color transform |
| Multiband 2-9 bands | Req | — | — | — | ✅ |
| Multiband 2-255 bands | — | Req | — | — | ✅ |
| Multiband 2-999 bands | — | — | Req | Req | ✅ |
| HTJ2K (CD/MD) | Req | Req | Req | Req | ✅ |
| Masked variants (M8/MD) | Req | Req | Req | Req | ✅ |

#### JPEG DCT Compression (IC=C3/M3)

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Monochrome 8-bit | Req | ✅ | Full encoding and decoding |
| Monochrome 12-bit | Req | ❌ | See [12-bit JPEG limitation](#12-bit-jpeg-limitation) below |
| RGB 24-bit (IMODE=P) | Req | ✅ | Full encoding and decoding |
| YCbCr601 24-bit (IMODE=P) | Req | ✅ | Full encoding and decoding with color space conversion |
| Multiband individual JPEG (IMODE=B,S) | Req | ✅ | Full encoding and decoding |

#### Downsampled JPEG (IC=I1)

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Mono 8-bit, single block ≤2048×2048 | Req | ✅ | Full encoding and decoding with dimension validation |

#### Vector Quantization (IC=C4/M4)

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Monochrome 8-bit with 4×4 kernel | Req | ❌ | See VQ roadmap item above |
| RGB/LUT 8-bit with 4×4 kernel | Req | ❌ | See VQ roadmap item above |

#### Optional Compression Formats

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| JPEG Lossless (C5/M5) Mono 8,12,16-bit | Opt | ❌ | See roadmap item above |
| JPEG Lossless (C5/M5) RGB 24-bit | Opt | ❌ | See roadmap item above |
| Bi-Level (C1/M1) 1-bit | Opt | ❌ | See roadmap item above |
| ZLIB (CC/MC) 32,64-bit | Opt | ❌ | See roadmap item above |
| SARZip (C7/M7) complex | Opt | ❌ | See roadmap item above |
| SARZip (C7/M7) magnitude | Opt | ❌ | See roadmap item above |

#### Special Data Types

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Elevation Data (ICAT=DTEM) | Opt | ⚠️ | Missing GEOSDE TRE support |
| Location Grid (ICAT=LOCG) | Opt | ❌ | |
| Matrix Data uncompressed | Opt | ⚠️ | Basic support only |
| Matrix Data J2K lossless | Opt | ⚠️ | Basic support only |
| Polar Coordinates (IREP=POLAR) | Req | ❌ | |

#### Graphic Segments

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| CGM Graphic Subheader | Req | ✅ | Full subheader parsing per Table 5.15-1 |
| CGM Data (BPCGM01.00 profile) | Req | ✅ | Raw CGM data extraction via `raw_asset()` |
| Aggregate size limit (1-2 MB) | Req | ⚠️ | Validation not implemented |

#### Text Segments

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Text Subheader | Req | ✅ | Full subheader parsing per Table 5.17-1 |
| Text Format Codes (STA, MTF, UT1, U8S) | Req | ✅ | Encoding-aware decoding via `text` property |
| Text Data (1-99999 bytes) | Req | ✅ | Raw data via `raw_asset()`, decoded via `text` |

### Conformance Summary

Fully conformant: file structure, uncompressed imagery, JPEG 2000 including HTJ2K, JPEG DCT (8-bit), downsampled JPEG, image masking, TRE/DES extensions, graphic segments, text segments.

Remaining for full CLEVEL conformance: Vector Quantization (C4/M4).

## Known Limitations

### 12-bit JPEG Limitation

12-bit JPEG encoding and decoding is not supported due to architectural constraints in libjpeg-turbo. The TurboJPEG API only supports 8-bit samples. 12-bit JPEG requires a separately compiled `libjpeg12` library with `BITS_IN_JSAMPLE=12` and renamed symbols. Supporting both 8-bit and 12-bit in the same application requires linking against two separate libraries.

Impact: files with 12-bit JPEG imagery (IC=C3/M3 with NBPP=12) cannot be decoded. 12-bit JPEG is relatively rare in NITF files.

Workarounds: convert to JPEG 2000 (IC=C8) which fully supports 12-bit, or use uncompressed format (IC=NC).

### KSY Structure Definition Limitations

The Kaitai Struct definitions under `data/structures/` have the following known gaps:

#### Partially Expanded TREs

| TRE | Limitation |
|-----|------------|
| BCHIPA | Sections B and C remain as raw bytes. Section B has nested conditionals; Section C has variable-length fields. |
| ILLUMB | Illumination set loop data remains as raw bytes. Per-set conditional fields reference parent scope not supported by KSY parser. |
| BANDSB | Auxiliary data (EXISTENCE_MASK bit 0) remains as raw bytes due to switch-on-value logic. All other 26 conditional field groups are fully parsed. |

Resolution: implement `_parent` scope resolution and variable-length field support in the KSY parser.

#### DES Workarounds

| DES | Limitation |
|-----|------------|
| CSSHPA | CC_SOURCE conditional uses size-based proxy instead of checking `SHAPE_USE == "CLOUD_SHAPES"`. |
| CSSHPB | NUM_SUPPORTING_FILES section (DESVER=02 only) captured as raw bytes because DESVER is not available in KSY scope. |
| XML_DATA_CONTENT | Conditional fields use `_root._io.size` checks as a proxy for DESSHL value. |

#### Unavailable Specifications

| Name | Status |
|------|--------|
| PIVECA | Spec marked "To Be Determined" in STDI-0002 |
| STDIDA / STDIDB | Referenced in NSDE collection (STDI-0001), not implemented |
| DPPDB | Reference to MIL-PRF-89034 (not publicly available) |
| WBRD_Frame | Specification not publicly available |

#### Missing NITF Structures

| Structure | Notes |
|-----------|-------|
| NITF 2.0 File Header | Different security field structure, different field sizes vs 2.1 |
| NITF 2.0 Image Subheader | Different security fields, no XBANDS |
| NITF 2.0 DES Subheader | Different DESID usage and security field layout |
| NSIF 1.0 segment subheaders | Structurally identical to NITF 2.1 — can reuse those definitions |

### Semantic Limitations

RPC00A polynomial term order differs from RPC00B; exact order is defined in STDI-0001 (not publicly available). Fields can be read/written correctly, but geolocation calculations using RPC00A coefficients will produce incorrect results without the term order mapping.
