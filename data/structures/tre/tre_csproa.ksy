meta:
  id: tre_csproa
  title: Processing Information TRE
  endian: be

doc: |
  CSPROA TRE - Processing Information
  
  Identifies processing options that were applied during image formation
  by the Commercial Data Provider.
  
  This TRE is required in image segment subheaders. If the data for a given
  sensor (sub-image) spans multiple image segments, the CSPROA TRE shall be
  identical in each of the image segments.
  
  The first 9 fields are reserved fill with fixed values that describe
  the processing pipeline steps. The BWC field indicates the bandwidth
  compression method used.
  
  Reference: STDI-0006 (NCDRD), Table 3.6-1

seq:
  - id: RESERVED_1
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: LATESTCAL (space padded to 12 characters)

  - id: RESERVED_2
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: All spaces

  - id: RESERVED_3
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: All spaces

  - id: RESERVED_4
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: MARKANDFIX (space padded to 12 characters)

  - id: RESERVED_5
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: Space character filled

  - id: RESERVED_6
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: CORR for MS, all spaces for PAN

  - id: RESERVED_7
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: SKIPAGM (space padded to 12 characters)

  - id: RESERVED_8
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: INTERP (space padded to 12 characters)

  - id: RESERVED_9
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: Space character filled

  - id: BWC
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Bandwidth Compression
      VISUAL = JPEG 2000 visually lossless
      NUMERICAL = JPEG 2000 numerically lossless
      UNCOMPRESSED = no compression
