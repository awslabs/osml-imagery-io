meta:
  id: tre_rsmgia
  title: RSM Grid Identification TRE
  endian: be

doc: |
  RSMGIA TRE - Replacement Sensor Model Grid Identification
  
  Provides identification and normalization data for the RSM grid
  representation. Contains image identifier, edition, normalization
  offsets and scale factors for ground row, ground column, X, Y, and Z
  coordinates, and section counts.
  
  CEL: 591 bytes
  
  Reference: STDI-0002 Volume 1, Appendix U - RSM

seq:
  - id: IID
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: EDITION
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: GR0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Normalization Offset
      21 BCS-N real number.

  - id: GRX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row X Normalization Scale Factor
      21 BCS-N real number.

  - id: GRY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Y Normalization Scale Factor
      21 BCS-N real number.

  - id: GRZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Z Normalization Scale Factor
      21 BCS-N real number.

  - id: GRXX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row XX Normalization Scale Factor
      21 BCS-N real number.

  - id: GRXY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row XY Normalization Scale Factor
      21 BCS-N real number.

  - id: GRXZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: GRYY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row YY Normalization Scale Factor
      21 BCS-N real number.

  - id: GRYZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: GRZZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: GC0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Normalization Offset
      21 BCS-N real number.

  - id: GCX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column X Normalization Scale Factor
      21 BCS-N real number.

  - id: GCY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Y Normalization Scale Factor
      21 BCS-N real number.

  - id: GCZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Z Normalization Scale Factor
      21 BCS-N real number.

  - id: GCXX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column XX Normalization Scale Factor
      21 BCS-N real number.

  - id: GCXY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column XY Normalization Scale Factor
      21 BCS-N real number.

  - id: GCXZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: GCYY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column YY Normalization Scale Factor
      21 BCS-N real number.

  - id: GCYZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: GCZZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: GRNIS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Ground Row Sections
      3 BCS-NPI positive integer.

  - id: GCNIS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Ground Column Sections
      3 BCS-NPI positive integer.

  - id: GTNIS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Total Number of Ground Sections
      3 BCS-NPI positive integer (GRNIS * GCNIS).

  - id: GRSSIZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Section Size
      21 BCS-N real number.

  - id: GCSSIZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Section Size
      21 BCS-N real number.
