# Reference Materials

## Overview

Key reference material for working on, and effectively using, this library includes:

- NITF 2.0/2.1 specifications, especially the Joint BIIF Profile (JBP) document
- GeoTIFF specifications
- SAR imagery standards (SICD, SIDD, SIPS)

These specifications are controlled by third parties and are not checked into the repository.

### NITF/JBP Standards

The general source for NITF standards is the [NTB part](https://nsgreg.nga.mil/ntb.jsp) of the [NSG Standards Registry](https://nsgreg.nga.mil/).

The links below are to the citation entry, which is a stable URL. The actual documents are on short-term links off those entries - look for "Cited Document" near the top of
citation pages.

Particularly important documents are:
- [BIIF Profile](https://nsgreg.nga.mil/doc/view?i=5741) - Joint BIIF Profile (JBP)
- [STDI-0002](https://nsgreg.nga.mil/doc/view?i=5744) - Definitions for most TREs and DES
- [BIIF Profile for JPEG 2000](https://nsgreg.nga.mil/doc/view?i=5583) - How JPEG 2000 fits into NITF/JBP.

The JBP replaces MIL-STD-2500 and the body of STANAG 4545, resolving the historical minor differences between NITF and the NATO Secondary Imagery Format (NSIF).
STANAG 4545 now just refers to the JBP.

### SAR Imagery Standards

SAR (Synthetic Aperture Radar) imagery uses specialized NITF-based formats. Both SICD and SIDD files are NITF files following specific guidelines defined in the JBP specification.

Links sourced from the [SarPy project](https://github.com/ngageoint/sarpy):

#### SICD - Sensor Independent Complex Data (v1.3.0; 2021-11-30)

Standard for complex SAR imagery (Single Look Complex / Level 1 data).

- [Volume 1 - Design & Implementation Description Document](https://nsgreg.nga.mil/doc/view?i=5381)
- [Volume 2 - File Format Description Document](https://nsgreg.nga.mil/doc/view?i=5382)
- [Volume 3 - Image Projections Description Document](https://nsgreg.nga.mil/doc/view?i=5383)
- [Schema](https://nsgreg.nga.mil/doc/view?i=5418)

#### SIDD - Sensor Independent Derived Data (v3.0; 2021-11-30)

Standard for derived SAR products (detected imagery, etc.).

- [Volume 1 - Design and Implementation Description Document](https://nsgreg.nga.mil/doc/view?i=5385)
- [Volume 2 - NITF File Format Description Document](https://nsgreg.nga.mil/doc/view?i=5441)
- [Volume 3 - GeoTIFF File Format Description Document](https://nsgreg.nga.mil/doc/view?i=5387)
- [Schema](https://nsgreg.nga.mil/doc/view?i=5384)

#### SIPS - SAR Image Processing Standard

Sandia National Laboratories image processing algorithms for SAR data. Useful for implementing image operators.

Files in `SIPS/`:
- `SIPS_v24_21Aug2019.pdf` - Main SIPS specification
- `SAND2015-2309.pdf` - Supporting Sandia report
- `SAND2019-2371.pdf` - Supporting Sandia report

### GeoTIFF Standards

- [TIFF Revision 6.0](https://www.itu.int/itudoc/itu-t/com16/tiff-fx/docs/tiff6.pdf) - Base TIFF format specification
- [OGC GeoTIFF Standard](https://www.ogc.org/standard/geotiff/) - GeoTIFF geospatial extensions
- [OGC Cloud Optimized GeoTIFF](https://www.ogc.org/standard/cogtiff/) - COG standard for cloud-native access
