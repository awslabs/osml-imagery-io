# Working with PDF Reference Materials

This project uses PDF reference materials for NITF/NSIF format implementation. These PDFs are large (often 100-200+ pages) and cannot be read in their entirety. Always use targeted page reads.

## Reference Materials Location

PDF reference materials are located in `reference-materials/`:

- `Joint-BIIF-Profile-V2024.1_2024-01-18.pdf` - Main JBP format specification (201 pages)
- `STDI-0002-2024.1_2023-10-26/` - TRE and DES definitions:
  - `Vol-1-App {X} - {NAME}.pdf` - TRE specifications
  - `Vol-2-App {X} - {NAME}.pdf` - DES specifications
  - `STDI-0002-Volume-{N}-*.pdf` - Main reference documents

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
