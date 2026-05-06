---
inclusion: fileMatch
fileMatchPattern: 'src/jbp/**|src/parser/**|src/tiff/**|src/j2k/**|src/jpeg/**|src/png/**|reference-materials/**|**/tre_*|**/des_*|docs/roadmap/**|docs/codecs/**'
---

# Working with PDF Reference Materials

This file loads only when you're working in format-implementation or spec-reading areas (NITF/TRE/DES code, format modules, codec design docs, or the PDFs themselves). It is not always-on because the large PDF inventory is irrelevant to most tasks.

This project uses PDF reference materials for NITF/NSIF format implementation. These PDFs are large (often 100-200+ pages) and cannot be read in their entirety. Always use targeted page reads.

## Reference Materials Location

PDF reference materials are located in `reference-materials/`:

- `JBP/` - Joint BIIF Profile (NITF format):
  - `Joint-BIIF-Profile-V2024.1_2024-01-18.pdf` - Main JBP format specification (201 pages)
  - `NITF_MIL_STD_2500a.pdf` - MIL-STD-2500A, legacy NITF 2.0 specification. Useful for understanding NITF 2.0 file structure and backward-compatibility cases.
  - `MIL-STD-188-199.pdf` - Vector Quantization (VQ) decompression standard (35 pages)
  - `NCDRD_18February2010.pdf` - NITF 2.1 Commercial Dataset Requirements Document (78 pages). Defines requirements for commercial imagery datasets from CDPs.
  - `NGA.IP.0002_1.0 HRE.pdf` - High Resolution Elevation (HRE) Products Implementation Profile (148 pages). Specifies data content, structure, and metadata for raster elevation data products.
  - `NGA.STND.0044_1.3.3_MIE4NITF_202601.pdf` - Motion Imagery Extension for NITF 2.1, v1.3.3 (146 pages). Defines how motion imagery is packaged in NITF files.
  - `USAF SARzip Standard V1.0.0.pdf` - SAR Compression (SARzip) Standard, v1.0.0 (143 pages). Defines compression for Synthetic Aperture Radar data.
  - `STDI-0002-v2025.2-202601/` - Support Data Extensions (SDE) Compendium, v2025.2 (2025-06-10):
    - `STDI-0002-SDE-Fundamentals-MainBody-V2025-2_202601.pdf` - SDE Fundamentals (34 pages)
    - `STDI-0002-SDE-Fundamentals-Excel-Tables-V2025-2_202507.xlsx` - SDE field tables in Excel format
    - `STDI-0002-Volume-1-TREs-V2025-2_202601.pdf` - Volume 1: TRE index and overview (14 pages)
    - `STDI-0002-Volume-2-DESs-and-DESs-TREs-Combinations-V2025-2_202601.pdf` - Volume 2: DES index and overview (15 pages)
    - `STDI-0002-Volume-3-SDE-Profiles-and-Implementation-Guidance-V2025-2_202601.pdf` - Volume 3: SDE Profiles and Implementation Guidance (7 pages)
    - `Vol1-App{XX}-{NAME}_{YYYYMM}.pdf` - Individual TRE appendices (see below)
    - `Vol2-App{X}-{NAME}_{YYYYMM}.pdf` - Individual DES appendices (see below)
    - `Vol3-App{X}-{NAME}_{YYYYMM}.pdf` - Individual profile appendices
    - Some appendices also include `.xsd` schema files alongside the PDF
- `GeoTIFF/` - TIFF and GeoTIFF specifications:
  - `TIFF6.pdf` - TIFF Revision 6.0 base format specification (121 pages)
  - `OGCGeoTIFFStandard.pdf` - OGC GeoTIFF standard (112 pages)
  - `OGCCloudOptimizedGeoTIFFStandard.pdf` - OGC Cloud Optimized GeoTIFF standard (34 pages)
- `SICD/` - Sensor Independent Complex Data (SAR complex imagery):
  - `NGA.STND.0024-1_1.3.0_SICD_DIDD_FINAL.pdf` - Design & Implementation
  - `NGA.STND.0024-2_1.3.0_SICD_FFDD_FINAL.pdf` - File Format
  - `NGA.STND.0024-3_1.3.0_SICD_IPDD_FINAL.pdf` - Image Projections
- `SIDD/` - Sensor Independent Derived Data (SAR derived products):
  - `NGA.STND.0025-1_3.0_SIDD_DIDD.pdf` - Design & Implementation
  - `NGA.STND.0025-2_3.0_SIDD_NITF_FFDD.pdf` - NITF File Format
  - `NGA.STND.0025-3_3.0-SIDD_GEOTIFF.pdf` - GeoTIFF File Format
- `SIPS/` - SAR Image Processing Standard (image operators):
  - `SIPS_v24_21Aug2019.pdf` - Main SIPS specification
  - `SAND2015-2309.pdf`, `SAND2019-2371.pdf` - Supporting Sandia reports

## STDI-0002 Appendix Naming Convention

The v2025.2 release uses a new naming convention for appendices:

```
Vol{V}-App{XX}-{NAME}_{YYYYMM}.pdf
```

- `V` = Volume number (1 = TREs, 2 = DESs, 3 = Profiles)
- `XX` = Appendix letter(s) (e.g., B, C, AA, AX)
- `NAME` = TRE/DES name(s) (e.g., ICHIPB, SENSRB, CSSHPA-CSSHPB)
- `YYYYMM` = Appendix revision date

### Volume 1 TRE Appendices

| Appendix | TRE Name(s) | File |
|----------|-------------|------|
| B | ICHIPB | `Vol1-AppB-ICHIPB_202410.pdf` |
| C | PIAE | `Vol1-AppC-PIAE_202506.pdf` |
| D | CSDE | `Vol1-AppD-CSDE_202502.pdf` |
| E | ASDE | `Vol1-AppE-ASDE_202502.pdf` |
| F | IOMAPA | `Vol1-AppF-IOMAPA_202110.pdf` |
| I | NBLOCA | `Vol1-AppI-NBLOCA_202110.pdf` |
| L | HISTOA | `Vol1-AppL-HISTOA_202110.pdf` |
| N | ENGRDA | `Vol1-AppN-ENGRDA_202110.pdf` |
| O | MITOCA | `Vol1-AppO-MITOCA_202110.pdf` |
| P | GEOSDE | `Vol1-AppP-GEOSDE_202404.pdf` |
| R | NSDE | `Vol1-AppR-NSDE_202410.pdf` |
| U | RSM | `Vol1-AppU-RSM_202207.pdf` |
| V | DPPDB | `Vol1-AppV-DPPDB_202110.pdf` |
| W | ATTPTA | `Vol1-AppW-ATTPTA_202502.pdf` |
| X | BANDSB | `Vol1-AppX-BANDSB_202502.pdf` |
| Y | J2KLRA/J2KLRB | `Vol1-AppY-J2KLRA-J2KLRB_202407.pdf` |
| Z | SENSRB | `Vol1-AppZ-SENSRB_202506.pdf` |
| AA | PIXQLA | `Vol1-AppAA-PIXQLA_202210.pdf` |
| AD | RELCCA | `Vol1-AppAD-RELCCA_202310.pdf` |
| AE | XMLDCA | `Vol1-AppAE-XMLDCA_202110.pdf` |
| AF | MIE4NITF | `Vol1-AppAF-MIE4NITF_202506.pdf` |
| AG | CCINFA | `Vol1-AppAG-CCINFA_202506.pdf` |
| AH | GLAS-GFM | `Vol1-AppAH-GLAS-GFM_202110.pdf` |
| AI | SECURA | `Vol1-AppAI-SECURA_202410.pdf` |
| AJ | PIXMTA | `Vol1-AppAJ-PIXMTA_202506.pdf` |
| AK | MATESA | `Vol1-AppAK-MATESA_202506.pdf` |
| AL | ILLUMA/ILLUMB | `Vol1-AppAL-ILLUMA-ILLUMB_202504.pdf` (+`.xsd`) |
| AM | PIVECA | `Vol1-AppAM-PIVECA_UnderDevelopment.pdf` |
| AN | FRMSGA | `Vol1-AppAN-FRMSGA_202506.pdf` (+`.xsd`) |
| AP | SODDXA | `Vol1-AppAP-SODDXA_202504.pdf` (+`.xsd`) |
| AQ | ASTORA | `Vol1-AppAQ-ASTORA_202204.pdf` |
| AR | BCHIPA | `Vol1-AppAR-BCHIPA_202404.pdf` |
| AS | CSDIDA/SYSIDA | `Vol1-AppAS-CSDIDA-SYSIDA_202502.pdf` |
| AT | S2EVPA | `Vol1-AppAT-S2EVPA_202506.pdf` |
| AU | COMNTA | `Vol1-AppAU-COMNTA_202306.pdf` |
| AV | CCIS-CSCCGA | `Vol1-AppAV-CCIS-CSCCGA_202406.pdf` |
| AW | CSCRNAandFCRNSA | `Vol1-AppAW-CSCRNAandFCRNSA_202404.pdf` |
| AX | ISAR | `Vol1-AppAX-ISAR_202402.pdf` |
| AY | SORBXA | `Vol1-AppAY-SORBXA_202504.pdf` (+`.xsd`) |

### Volume 2 DES Appendices

| Appendix | DES Name(s) | File |
|----------|-------------|------|
| A | TRE Overflow | `Vol2-AppA-TREOverflow_202110.pdf` |
| C | CSATTA | `Vol2-AppC-CSATTA_202110.pdf` |
| D | CSSHPA/CSSHPB | `Vol2-AppD-CSSHPA-CSSHPB_202506.pdf` |
| E | WBRD Frame | `Vol2-AppE-WBRD_Frame_202110.pdf` |
| F | XML_DATA_CONTENT | `Vol2-AppF-XML_DATA_CONTENT_202401.pdf` |
| G | Moving Target Report | `Vol2-AppG-MovingTargetReport_202405.pdf` |
| J | LIDARA | `Vol2-AppJ-LIDARA_202110.pdf` |
| K | EXT_DEF_CONTENT | `Vol2-AppK-EXT_DEF_CONTENT_202110.pdf` |
| L | WEATHER_DATA | `Vol2-AppL-WEATHER_DATA_202310.pdf` |
| M | GLAS-GFM | `Vol2-AppM-GLAS-GFM_202505.pdf` |
| O | MRGXMA | `Vol2-AppO-MRGXMA_202504.pdf` (+`.xsd`) |

### Volume 3 Profile Appendices

| Appendix | Profile Name | File |
|----------|-------------|------|
| A | PAPX | `Vol3-AppA-PAPX_202504.pdf` (+`.xsd`) |
| C | Profile of SENSRB | `Vol3-AppC-ProfileOfSENSRB_202110.pdf` |

## General Strategy for Reading PDFs

### Step 1: Always Read TOC First

Never attempt to read an entire PDF. Start with pages 1-10 to find:
- Title and version info (page 1)
- Change log (page 3)
- Table of Contents (pages 6-10)

```
mcp_pdf_reader_read_pdf with pages: [1, 6, 7, 8, 9, 10]
```

### Step 2: Use TOC to Find Relevant Sections

From the TOC, identify page numbers for the specific information you need, then read only those pages.

### Step 3: Read in Small Batches

Read 5-10 pages at a time maximum. If you need more context, make additional targeted reads.

## Document-Specific Guidance

### Joint BIIF Profile (JBP)

The main format specification (201 pages). Key sections:

| Topic | Section | Approx Pages |
|-------|---------|--------------|
| File Structure | 4.4 | 17-18 |
| Field Types | 4.6, 5.2 | 24-28 |
| Security Fields | 5.10 | 34-44 |
| File Header | 5.11 | 44-54 |
| Image Subheader | 5.13 | 66-89 |
| Graphic Subheader | 5.15 | 90-95 |
| Text Subheader | 5.17 | 95-98 |
| DES Structure | 5.18 | 98-103 |
| TRE Placement | 5.9 | 31-34 |

### MIL-STD-188-199 (VQ Decompression)

Vector Quantization decompression standard (35 pages). Defines the IC=C4/M4 compression format.

| Topic | Approx Pages |
|-------|--------------|
| Title, Scope | 1-3 |
| Applicable Documents | 4-5 |
| Definitions & Acronyms | 6-10 |
| Decompression Requirements | 10-25 |
| Codebook Structure | 15-25 |
| Appendices | 25-35 |

Key concepts:
- Codebook-based lossy compression using 4×4 pixel blocks
- Up to 4 lookup tables embedded in the image data
- COMRAT expressed as bits-per-pixel (e.g., "1.00")
- Decompression is table lookup only (no complex math)

### NCDRD (Commercial Dataset Requirements)

Commercial imagery dataset requirements (78 pages). Defines how commercial data providers package imagery in NITF 2.1. Useful for understanding TRE/DES requirements for commercial imagery products and JPEG 2000 packaging conventions.

### HRE (High Resolution Elevation)

Implementation profile for elevation products (148 pages). Specifies how raster elevation data is structured and tagged in NITF. Relevant for DEM/DSM imagery support.

### MIE4NITF (Motion Imagery Extension)

Motion imagery extension for NITF 2.1, v1.3.3 (146 pages). Defines how video/motion imagery frames are packaged in NITF files. Related TRE appendix: `Vol1-AppAF-MIE4NITF_202506.pdf`.

### SARzip (SAR Compression)

SAR compression standard, v1.0.0 (143 pages). Defines a compression scheme for Synthetic Aperture Radar data. Relevant for SAR imagery workflows alongside SICD/SIDD.

### STDI-0002 Structure (v2025.2)

The v2025.2 release reorganized STDI-0002 into a clearer structure:

- **SDE Fundamentals** (34 pages) - Core concepts, field types (BCS-A, BCS-N, binary), and general rules for all SDEs. Read this first when working with any TRE or DES.
- **Volume 1** (14 pages) - Index of all TRE appendices with status and version info.
- **Volume 2** (15 pages) - Index of all DES appendices with status and version info.
- **Volume 3** (7 pages) - SDE profiles and implementation guidance.
- **Excel Tables** (`.xlsx`) - Machine-readable field tables for all SDEs.
- **Individual Appendices** - Each TRE/DES has its own PDF (and sometimes `.xsd` schema).

### STDI-0002 TRE Appendices

Individual TRE specifications. Common structure:

| Content | Typical Pages |
|---------|---------------|
| Title, Change Log | 1-5 |
| Table of Contents | 6-8 |
| Introduction | 9-12 |
| Field Specifications | 12-35 |
| Implementation Notes | 35+ |

Key sections to look for:
- "FIELD SPECIFICATIONS" - The main table defining all fields
- "Implementation Notes" - Conditional logic and special cases
- "Sample TRE" - Example data

### STDI-0002 DES Appendices

Similar structure to TRE appendices but for Data Extension Segments.

## What to Extract from Specifications

When implementing parsers, extract:
- Field name
- Field size (bytes)
- Field type (BCS-A, BCS-N, binary, etc.)
- Valid value ranges
- Conditional presence logic
- Repeat counts for looped fields

## Common Pitfalls

1. **Don't read entire PDFs** - Use targeted page reads based on TOC
2. **Check for conditional fields** - Complex TREs/DES have modules that may or may not be present
3. **Watch for repeated fields** - Loop counts followed by repeated field groups
4. **Note field types** - BCS-A (ASCII), BCS-N (numeric), binary have different parsing rules
5. **Cross-reference JBP and STDI-0002** - JBP defines structure, STDI-0002 defines TRE/DES content

## Workflow Example

To implement SENSRB TRE:

1. Read TOC: `pages: [1, 6, 7, 8]` from `Vol1-AppZ-SENSRB_202506.pdf`
2. Find "Field Specifications" section in TOC (e.g., Section Z.3, page Z-18)
3. Read field specs: `pages: [18, 19, 20, 21, 22, 23, 24, 25]`
4. If TRE has conditional modules, read implementation notes section
5. Create definition file based on extracted field information

## SAR Standards (SICD, SIDD, SIPS)

### Overview

SICD and SIDD are specialized NITF-based formats for SAR (Synthetic Aperture Radar) imagery. They build on the JBP specification with additional XML metadata stored in Data Extension Segments (DES).

- **SICD** - Complex SAR data (Single Look Complex / Level 1). Contains phase and magnitude information.
- **SIDD** - Derived SAR products (detected imagery, etc.). The output of processing SICD data.
- **SIPS** - Image processing algorithms for SAR. Useful for implementing image operators.

### SICD Documents

| Document | Content | Use For |
|----------|---------|---------|
| Volume 1 (DIDD) | Design & Implementation | Understanding SICD concepts, XML schema structure |
| Volume 2 (FFDD) | File Format | NITF structure, DES placement, TRE requirements |
| Volume 3 (IPDD) | Image Projections | Coordinate transformations, geolocation algorithms |

### SIDD Documents

| Document | Content | Use For |
|----------|---------|---------|
| Volume 1 (DIDD) | Design & Implementation | Understanding SIDD concepts, XML schema structure |
| Volume 2 (NITF FFDD) | NITF File Format | NITF structure for SIDD files |
| Volume 3 (GeoTIFF) | GeoTIFF File Format | Alternative GeoTIFF packaging |

### SIPS Documents

The SIPS specification defines image processing algorithms. Key topics include:
- Radiometric calibration
- Geometric corrections
- Filtering and enhancement
- Phase history processing

### Reading Strategy for SAR Documents

1. Start with Volume 1 (DIDD) to understand concepts and XML schema
2. Use Volume 2 (FFDD) for NITF file structure and DES/TRE requirements
3. Reference Volume 3 for projection/geolocation (SICD) or GeoTIFF (SIDD)
4. Cross-reference with JBP for underlying NITF structure

## GeoTIFF Standards

### TIFF 6.0

The base TIFF format specification (121 pages). Defines the fundamental file structure, IFD (Image File Directory) layout, tag definitions, and compression schemes.

### OGC GeoTIFF Standard

The OGC GeoTIFF standard (112 pages). Extends TIFF with geospatial metadata via GeoKeys stored in TIFF tags. Defines coordinate reference systems, model transformations, and projection parameters.

### OGC Cloud Optimized GeoTIFF (COG)

Cloud Optimized GeoTIFF standard (34 pages). Defines conventions for organizing GeoTIFF files for efficient HTTP range-request access. Covers internal tiling, overview levels, and IFD ordering requirements.
