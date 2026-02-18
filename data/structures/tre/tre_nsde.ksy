meta:
  id: tre_nsde
  title: National Support Data Extensions Reference
  endian: be

doc: |
  NSDE - National Support Data Extensions
  
  This is a placeholder definition. The NSDE appendix (Vol-1-App R) in STDI-0002
  does not define a specific TRE structure. Instead, it references STDI-0001
  "National Support Data Extensions (SDE) (Version 1.3) for the National Imagery
  Transmission Format Standard (NITFS), 2 October 1998, with Change Notice 3,
  dated 18 March 2010."
  
  NSDE is a collection of National Support Data Extensions that includes multiple
  TREs defined in STDI-0001. Common NSDE TREs include:
  - STDIDC (Standard ID Extension) - Already defined in tre_stdidc.ksy
  - STDIDB (Standard ID Extension B)
  - STDIDA (Standard ID Extension A)
  - And others defined in STDI-0001
  
  For information regarding NSG standardization documentation, see the National
  System for Geospatial Intelligence Standards Registry at: https://nsgreg.nga.mil
  
  Keywords: National Data, Library, Discovery, Searchable, Coverage,
  Geopositioning, and Exploitation.
  
  Reference: STDI-0002 Volume 1, Appendix R - NSDE
  Reference: STDI-0001 National Support Data Extensions (SDE) Version 1.3

# Note: This file serves as documentation only. The actual NSDE TREs are
# defined in their individual .ksy files (e.g., tre_stdidc.ksy).
# No seq section is defined as NSDE is not a single TRE but a collection.
