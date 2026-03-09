# JBP Complexity Level (CLEVEL) Assessment

## Introduction

The Joint BIIF Profile (JBP) defines Complexity Levels (CLEVELs) to enable implementation across hardware platforms with varying resources while maintaining interoperability. CLEVELs constrain file features like image dimensions, file size, number of segments, and supported compression formats.

### Reference

- **Specification**: Joint BIIF Profile (JBP) Version 2024.1
- **CLEVEL Definition**: Section 5.20, Annex G (Table G-1)
- **Conformance Guidelines**: Section 6.1

### CLEVEL Overview

JBP defines four primary still imagery complexity levels (03, 05, 06, 07) plus level 09 for files exceeding level 07 constraints. Motion Imagery uses separate CLEVELs (50-59) defined in NGA.STND.0044 (MIE4NITF).

| CLEVEL | CCS Extent | Max File Size | Max Image Size | Max Bands | Image Segments | DES |
|--------|------------|---------------|----------------|-----------|----------------|-----|
| 03 | 2048×2048 | 50 MB | 2048×2048 | 9 | 0-20 | 0-10 |
| 05 | 8192×8192 | 1 GB | 8192×8192 | 255 | 0-20 | 0-50 |
| 06 | 65536×65536 | 2 GB | 65536×65536 | 999 | 0-100 | 0-100 |
| 07 | 99999999×99999999 | 10 GB | 99999999×99999999 | 999 | 0-100 | 0-100 |
| 09 | Unrestricted | Unrestricted | Unrestricted | >999 | >100 | >100 |

A file is marked at the lowest CLEVEL for which it qualifies, but no lower than the highest CLEVEL feature it contains.

---

## Feature Support Matrix

This matrix compares JBP CLEVEL requirements against our current implementation status.

### Legend

- ✅ Implemented
- ⚠️ Partial implementation
- ❌ Not implemented
- Req = Required for conformance
- Opt = Optional feature

### File Structure & Core Features

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| NITF 2.1 File Header | Req | ✅ | Full header parsing and generation |
| NSIF 1.0 File Header | Req | ✅ | Variant support included |
| Security Fields | Req | ✅ | All classification fields supported |
| TRE Support | Req | ✅ | Generic TRE parsing with field definitions |
| DES Support | Req | ✅ | Data Extension Segments supported |
| TRE_OVERFLOW DES | Req | ✅ | Overflow handling implemented |
| RES Support | — | ❌ | No RES currently approved by NTB |

### Uncompressed Imagery (IC=NC/NM)

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

### JPEG 2000 Compression (IC=C8/M8/CD/MD)

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

### JPEG DCT Compression (IC=C3/M3)

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Monochrome 8-bit | Req | ✅ | Full encoding and decoding |
| Monochrome 12-bit | Req | ❌ | See [12-bit JPEG Limitation](#12-bit-jpeg-limitation) |
| RGB 24-bit (IMODE=P) | Req | ✅ | Full encoding and decoding |
| YCbCr601 24-bit (IMODE=P) | Req | ✅ | Full encoding and decoding with color space conversion |
| Multiband individual JPEG (IMODE=B,S) | Req | ✅ | Full encoding and decoding |

### Downsampled JPEG (IC=I1)

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Mono 8-bit, single block ≤2048×2048 | Req | ✅ | Full encoding and decoding with dimension validation |

### Vector Quantization (IC=C4/M4)

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Monochrome 8-bit with 4×4 kernel | Req | ❌ | |
| RGB/LUT 8-bit with 4×4 kernel | Req | ❌ | |

### Optional Compression Formats

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| JPEG Lossless (C5/M5) Mono 8,12,16-bit | Opt | ❌ | |
| JPEG Lossless (C5/M5) RGB 24-bit | Opt | ❌ | |
| Bi-Level (C1/M1) 1-bit | Opt | ❌ | Single block ≤2560×8192 |
| ZLIB (CC/MC) 32,64-bit | Opt | ❌ | Floating-point data |
| SARZip (C7/M7) complex | Opt | ❌ | SAR imagery |
| SARZip (C7/M7) magnitude | Opt | ❌ | SAR imagery |

### Special Data Types

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Elevation Data (ICAT=DTEM) | Opt | ⚠️ | Missing GEOSDE TRE support |
| Location Grid (ICAT=LOCG) | Opt | ❌ | |
| Matrix Data uncompressed | Opt | ⚠️ | Basic support only |
| Matrix Data J2K lossless | Opt | ⚠️ | Basic support only |
| Polar Coordinates (IREP=POLAR) | Req | ❌ | |

### Graphic Segments

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| CGM Graphic Subheader | Req | ✅ | Full subheader parsing per Table 5.15-1 |
| CGM Data (BPCGM01.00 profile) | Req | ✅ | Raw CGM data extraction via `raw_asset()` |
| Display Level (SDLVL) | Req | ✅ | Accessible via metadata |
| Attachment Level (SALVL) | Req | ✅ | Accessible via metadata |
| Bounding Box (SBND1, SBND2) | Req | ✅ | Accessible via metadata |
| Aggregate size limit (1-2 MB) | Req | ⚠️ | Validation not implemented |

### Text Segments

| Feature | CL03-07 | Status | Notes |
|---------|---------|--------|-------|
| Text Subheader | Req | ✅ | Full subheader parsing per Table 5.17-1 |
| Text Format Codes (STA, MTF, UT1, U8S) | Req | ✅ | Encoding-aware decoding via `text` property |
| Attachment Level (TXTALVL) | Req | ✅ | Accessible via metadata |
| Text Data (1-99999 bytes) | Req | ✅ | Raw data via `raw_asset()`, decoded via `text` |
| Line Delimiter Normalization | Req | ✅ | CR/LF to platform-native conversion |
| Extended Subheader Data (TRE) | Opt | ✅ | TRE parsing when TXSHDL > 0 |

### Motion Imagery (Separate CLEVELs 50-59)

Motion Imagery is defined by NGA.STND.0044 (MIE4NITF) with its own complexity levels. This is independent of still imagery CLEVELs and not covered in this assessment.

---

## Current Status Summary

**Fully Conformant Features**:
- File structure (NITF 2.1, NSIF 1.0)
- Uncompressed imagery (all CLEVELs)
- JPEG 2000 including HTJ2K (all CLEVELs)
- JPEG DCT compression (8-bit mono, RGB, YCbCr, multiband)
- Downsampled JPEG (I1) for thumbnails
- Image masking (NM, M8, MD, M3)
- TRE/DES extensions
- Graphic segments (CGM data extraction, display/attachment levels)
- Text segments (encoding-aware decoding, attachment levels)

**Required for Full CLEVEL Conformance**:
1. ~~Graphic segments (Phase 1)~~ ✅ Complete
2. ~~Text segments (Phase 2)~~ ✅ Complete
3. ~~JPEG DCT compression (Phase 3)~~ ✅ Complete (except 12-bit, see limitation)
4. Vector Quantization (Phase 4)

**Optional Enhancements**:
5. Legacy/specialized compression formats (Phase 5)
6. Special data types (Phase 6)
7. Graphic segment aggregate size validation

The library currently supports reading and writing files at all CLEVELs for uncompressed, JPEG 2000, and JPEG DCT imagery, which covers the majority of NITF usage. Graphic segment support enables access to CGM vector graphics and annotations. Text segment support enables access to textual data with proper encoding handling.

---

## Known Limitations

### 12-bit JPEG Limitation

12-bit JPEG encoding and decoding is **not supported** due to architectural constraints in libjpeg-turbo:

**Technical Background**:
- libjpeg-turbo's TurboJPEG API (used for 8-bit JPEG) only supports 8-bit samples
- 12-bit JPEG requires a separately compiled libjpeg library with `BITS_IN_JSAMPLE=12`
- This produces a different library (`libjpeg12`) with renamed symbols (e.g., `jpeg12_read_scanlines`)
- The TurboJPEG API is disabled when building with 12-bit support
- Supporting both 8-bit and 12-bit in the same application requires linking against two separate libraries

**Impact**:
- Files with 12-bit JPEG imagery (IC=C3/M3 with NBPP=12) cannot be decoded
- Attempting to decode 12-bit JPEG will return a clear error message explaining the limitation
- 12-bit JPEG is relatively rare in NITF files; most JPEG imagery uses 8-bit samples

**Workaround Options**:
1. Convert 12-bit imagery to JPEG 2000 (IC=C8) which fully supports 12-bit
2. Use uncompressed format (IC=NC) for 12-bit imagery
3. Future: Add optional `libjpeg-turbo-12bit` feature requiring separate libjpeg12 installation

**Conformance Note**: This limitation affects full CLEVEL conformance for files containing 12-bit JPEG imagery. All other JPEG DCT features (8-bit mono, RGB, YCbCr, multiband) are fully supported.
