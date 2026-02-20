meta:
  id: tre_rsmpia
  title: RSM Polynomial Identification TRE
  endian: be

doc: |
  RSMPIA TRE - Replacement Sensor Model Polynomial Identification
  
  Provides identification and normalization data for the RSM polynomial
  representation. Contains image identifier, edition, normalization
  offsets and scale factors for row, column, X, Y, and Z coordinates,
  and section counts.
  
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

  - id: R0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Normalization Offset
      21 BCS-N real number.

  - id: RX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row X Normalization Scale Factor
      21 BCS-N real number.

  - id: RY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Y Normalization Scale Factor
      21 BCS-N real number.

  - id: RZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Z Normalization Scale Factor
      21 BCS-N real number.

  - id: RXX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row XX Normalization Scale Factor
      21 BCS-N real number.

  - id: RXY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row XY Normalization Scale Factor
      21 BCS-N real number.

  - id: RXZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: RYY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row YY Normalization Scale Factor
      21 BCS-N real number.

  - id: RYZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: RZZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: C0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Normalization Offset
      21 BCS-N real number.

  - id: CX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column X Normalization Scale Factor
      21 BCS-N real number.

  - id: CY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Y Normalization Scale Factor
      21 BCS-N real number.

  - id: CZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Z Normalization Scale Factor
      21 BCS-N real number.

  - id: CXX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column XX Normalization Scale Factor
      21 BCS-N real number.

  - id: CXY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column XY Normalization Scale Factor
      21 BCS-N real number.

  - id: CXZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: CYY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column YY Normalization Scale Factor
      21 BCS-N real number.

  - id: CYZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: CZZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: RNIS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Row Sections in Image
      3 BCS-NPI positive integer.

  - id: CNIS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Column Sections in Image
      3 BCS-NPI positive integer.

  - id: TNIS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Total Number of Image Sections
      3 BCS-NPI positive integer (RNIS * CNIS).

  - id: RSSIZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Section Size
      21 BCS-N real number (rows per section).

  - id: CSSIZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Section Size
      21 BCS-N real number (columns per section).
