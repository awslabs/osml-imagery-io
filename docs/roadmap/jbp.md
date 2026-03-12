# JBP Implementation Roadmap

This roadmap addresses gaps in CLEVEL conformance identified in the JBP CLEVEL Assessment (see `internal/JBP_CLEVEL_ASSESSMENT.md`).

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
