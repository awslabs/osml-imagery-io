# Working with PDF Reference Materials

This project uses PDF reference materials for NITF/NSIF format implementation. These PDFs are large (often 100-200+ pages) and cannot be read in their entirety. Always use targeted page reads.

## Reference Materials Location

PDF reference materials are located in `reference-materials/`:

- `JBP/` - Joint BIIF Profile (NITF format):
  - `Joint-BIIF-Profile-V2024.1_2024-01-18.pdf` - Main JBP format specification (201 pages)
  - `MIL-STD-188-199.pdf` - Vector Quantization (VQ) decompression standard (35 pages)
  - `STDI-0002-2024.1_2023-10-26/` - TRE and DES definitions:
    - `Vol-1-App {X} - {NAME}.pdf` - TRE specifications
    - `Vol-2-App {X} - {NAME}.pdf` - DES specifications
    - `STDI-0002-Volume-{N}-*.pdf` - Main reference documents
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

1. Read TOC: `pages: [1, 6, 7, 8]` from `Vol-1-App Z - SENSRB.pdf`
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
