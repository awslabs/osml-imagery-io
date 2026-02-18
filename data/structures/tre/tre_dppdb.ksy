meta:
  id: tre_dppdb
  title: Digital Point Positioning Data Base TREs Reference
  endian: be

doc: |
  DPPDB - Digital Point Positioning Data Base Tagged Record Extensions Reference
  
  This is a placeholder definition. The DPPDB appendix (Vol-1-App V) in STDI-0002
  does not define the TRE field specifications. Instead, it references MIL-PRF-89034
  for the actual TRE definitions.
  
  Digital Point Positioning Data Bases (DPPDBs) are developed by the National
  Geospatial-Intelligence Agency (NGA) over user-specified areas to provide a
  capability for deriving accurate positional data on a quick-response basis for
  any identifiable feature within a DPPDB area. This includes geodetic latitude,
  geodetic longitude, geodetic elevation, and associated accuracies.
  
  DPPDB data is certified NITF Version 2.0 compliant. The image support data
  consists of rational function data, accuracy data, segment-to-segment shear
  data, diagnostic points, and adverse area indicators.
  
  The following TREs are defined in MIL-PRF-89034 for DPPDB products:
  
  Segment-level TREs:
  - IMASDA: Image support data tag
  - IMCBDA: Image compressed blocks directory tag
  - IMRFCA: Image rational function coefficients
  - SEGSPA: Stereo image segment shear point data tag
  - SISDDA: Stereo image segment data tag
  - SSDPDA: Stereo image segment diagnostic point data tag
  - PTPRAA: Segment to segment relative accuracy tag
  
  Product-level TREs:
  - MSDIRA: Master product directory definition tag
  - PPRSDA: Product accuracy (shear) data
  - PRADAA: Product accuracy data (absolute) definition tag
  - PRADRA: Product accuracy data (relative) definition tag
  - PSUPDA: Product support data tag
  - RGRDRA: Reference graphic directory definition tag
  
  MIL-PRF-89034 can be downloaded from http://earth-info.nga.mil/publications/specs/
  
  Keywords: Digital Point Positioning Data Bases, exploitation, latitude,
  longitude, and elevation.
  
  Reference: STDI-0002 Volume 1, Appendix V - DPPDB
  Reference: MIL-PRF-89034 Digital Point Positioning Data Base

# Note: This file serves as documentation only. The actual DPPDB TREs are
# defined in MIL-PRF-89034. Individual TRE definitions would need to be
# created from that specification.
# No seq section is defined as DPPDB is not a single TRE but a collection.
