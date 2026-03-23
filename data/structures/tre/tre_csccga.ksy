meta:
  id: tre_csccga
  title: Cloud Cover Grid Data TRE
  endian: be

doc: |
  CSCCGA TRE - Cloud Cover Grid Data
  
  Provides support data that identifies which image segment and sensors were
  used to create the cloud cover grid. CSCCGA also geometrically registers
  the cloud grid to the pixel grid of one of the image segments.
  
  When cloud cover information is included in the dataset, both the CSCCGA TRE
  and the Cloud Cover Shapefile DES (CSSHPA) shall be included in each dataset.
  
  Reference: STDI-0006 (NCDRD), Table 3.1-1

seq:
  - id: CCG_SOURCE
    type: str
    size: 18
    encoding: BCS-A
    doc: |
      Source of Grid
      Concatenation of all sensors used to create cloud cover grid
      separated by commas. Values: PAN, MS, or PAN,MS

  - id: REG_SENSOR
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Image Segment Sensor to which Cloud Cover Grid is registered.
      CCG is always registered to the synthetic array.
      Values: PAN or MS

  - id: ORIGIN_LINE
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Cloud Cover Grid Origin - Line
      Corresponding line in registered image segment.
      Value: 0000001

  - id: ORIGIN_SAMPLE
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Cloud Cover Grid Origin - Sample
      Corresponding sample in registered image segment.
      Value: 00001

  - id: AS_CELL_SIZE
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Along Scan Cell Size - Lines
      Cloud Cover Grid spacing in registered image segment lines.
      Range: 0000001 to 9999999

  - id: CS_CELL_SIZE
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Cross Scan Cell Size - Samples
      Cloud Cover Grid spacing in registered image segment samples.
      Range: 00001 to 99999

  - id: CCG_MAX_LINE
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Number of Rows in CC Grid
      Number of cells in lines direction.
      Range: 0000001 to 9999999

  - id: CCG_MAX_SAMPLE
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Columns in CC Grid
      Number of cells in sample direction.
      Range: 00001 to 99999
