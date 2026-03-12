# Reference Materials

This directory contains reference materials and specifications for the image formats supported by this library.

## Contents

- NITF 2.0/2.1 specifications
- Joint BIIF Profile (JBP) documents
- GeoTIFF specifications
- SAR imagery standards (SICD, SIDD, SIPS)
- Related standards and profiles

## Directory Structure

```
reference-materials/
├── JBP/                                        # Joint BIIF Profile (NITF)
│   ├── Joint-BIIF-Profile-V2024.1_2024-01-18.pdf
│   ├── MIL-STD-188-199.pdf                    # VQ decompression standard
│   └── STDI-0002-2024.1_2023-10-26/           # TRE and DES definitions
├── GeoTIFF/                                    # TIFF and GeoTIFF specifications
│   ├── TIFF6.pdf                               # TIFF Revision 6.0 base format
│   ├── OGCGeoTIFFStandard.pdf                  # OGC GeoTIFF standard
│   └── OGCCloudOptimizedGeoTIFFStandard.pdf    # OGC Cloud Optimized GeoTIFF
├── SICD/                                       # Sensor Independent Complex Data
├── SIDD/                                       # Sensor Independent Derived Data
└── SIPS/                                       # SAR Image Processing Standard
```

## Obtaining the Specifications

These specifications are controlled by third parties and are not checked into the repository.

### NITF/JBP Standards

- [GWGNIA Wiki](https://nsgreg.nga.mil/gwg.jsp) - NITF and related standards
- [OGC GeoTIFF Standard](https://www.ogc.org/standard/geotiff/) - GeoTIFF specification
- [BIIF Profile](https://nsgreg.nga.mil/doc/view?i=5258) - Joint BIIF Profile
- [NGA Standards Registry](https://nsgreg.nga.mil/) - Search for other NGA standards

### SAR Imagery Standards

SAR (Synthetic Aperture Radar) imagery uses specialized NITF-based formats. Both SICD and SIDD files are NITF files following specific guidelines defined in the JBP specification.

Links sourced from the [SarPy project](https://github.com/ngageoint/sarpy):

#### SICD - Sensor Independent Complex Data (v1.3.0; 2021-11-30)

Standard for complex SAR imagery (Single Look Complex / Level 1 data).

- [Volume 1 - Design & Implementation Description Document](https://nsgreg.nga.mil/doc/view?i=5381)
- [Volume 2 - File Format Description Document](https://nsgreg.nga.mil/doc/view?i=5382)
- [Volume 3 - Image Projections Description Document](https://nsgreg.nga.mil/doc/view?i=5383)
- [Schema](https://nsgreg.nga.mil/doc/view?i=5384)

Files in `SICD/`:
- `NGA.STND.0024-1_1.3.0_SICD_DIDD_FINAL.pdf` - Design & Implementation
- `NGA.STND.0024-2_1.3.0_SICD_FFDD_FINAL.pdf` - File Format
- `NGA.STND.0024-3_1.3.0_SICD_IPDD_FINAL.pdf` - Image Projections

#### SIDD - Sensor Independent Derived Data (v3.0; 2021-11-30)

Standard for derived SAR products (detected imagery, etc.).

- [Volume 1 - Design and Implementation Description Document](https://nsgreg.nga.mil/doc/view?i=5385)
- [Volume 2 - NITF File Format Description Document](https://nsgreg.nga.mil/doc/view?i=5386)
- [Volume 3 - GeoTIFF File Format Description Document](https://nsgreg.nga.mil/doc/view?i=5387)
- [Schema](https://nsgreg.nga.mil/doc/view?i=5388)

Files in `SIDD/`:
- `NGA.STND.0025-1_3.0_SIDD_DIDD.pdf` - Design & Implementation
- `NGA.STND.0025-2_3.0_SIDD_NITF_FFDD.pdf` - NITF File Format
- `NGA.STND.0025-3_3.0-SIDD_GEOTIFF.pdf` - GeoTIFF File Format

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

Files in `GeoTIFF/`:
- `TIFF6.pdf` - TIFF Revision 6.0 base format specification
- `OGCGeoTIFFStandard.pdf` - OGC GeoTIFF standard
- `OGCCloudOptimizedGeoTIFFStandard.pdf` - OGC Cloud Optimized GeoTIFF standard

## Usage

Reference these specifications when implementing or validating format support. Download the relevant PDFs and place them in this directory for reference during development.
