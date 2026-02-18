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
  - id: iid
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: edition
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: gr0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Normalization Offset
      21 BCS-N real number.

  - id: grx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row X Normalization Scale Factor
      21 BCS-N real number.

  - id: gry
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Y Normalization Scale Factor
      21 BCS-N real number.

  - id: grz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Z Normalization Scale Factor
      21 BCS-N real number.

  - id: grxx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row XX Normalization Scale Factor
      21 BCS-N real number.

  - id: grxy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row XY Normalization Scale Factor
      21 BCS-N real number.

  - id: grxz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: gryy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row YY Normalization Scale Factor
      21 BCS-N real number.

  - id: gryz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: grzz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: gc0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Normalization Offset
      21 BCS-N real number.

  - id: gcx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column X Normalization Scale Factor
      21 BCS-N real number.

  - id: gcy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Y Normalization Scale Factor
      21 BCS-N real number.

  - id: gcz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Z Normalization Scale Factor
      21 BCS-N real number.

  - id: gcxx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column XX Normalization Scale Factor
      21 BCS-N real number.

  - id: gcxy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column XY Normalization Scale Factor
      21 BCS-N real number.

  - id: gcxz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: gcyy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column YY Normalization Scale Factor
      21 BCS-N real number.

  - id: gcyz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: gczz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: grnis
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Ground Row Sections
      3 BCS-NPI positive integer.

  - id: gcnis
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Ground Column Sections
      3 BCS-NPI positive integer.

  - id: gtnis
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Total Number of Ground Sections
      3 BCS-NPI positive integer (GRNIS * GCNIS).

  - id: grssiz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Section Size
      21 BCS-N real number.

  - id: gcssiz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Section Size
      21 BCS-N real number.
