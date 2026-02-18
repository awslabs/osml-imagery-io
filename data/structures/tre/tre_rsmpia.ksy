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

  - id: r0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Normalization Offset
      21 BCS-N real number.

  - id: rx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row X Normalization Scale Factor
      21 BCS-N real number.

  - id: ry
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Y Normalization Scale Factor
      21 BCS-N real number.

  - id: rz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Z Normalization Scale Factor
      21 BCS-N real number.

  - id: rxx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row XX Normalization Scale Factor
      21 BCS-N real number.

  - id: rxy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row XY Normalization Scale Factor
      21 BCS-N real number.

  - id: rxz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: ryy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row YY Normalization Scale Factor
      21 BCS-N real number.

  - id: ryz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: rzz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: c0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Normalization Offset
      21 BCS-N real number.

  - id: cx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column X Normalization Scale Factor
      21 BCS-N real number.

  - id: cy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Y Normalization Scale Factor
      21 BCS-N real number.

  - id: cz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Z Normalization Scale Factor
      21 BCS-N real number.

  - id: cxx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column XX Normalization Scale Factor
      21 BCS-N real number.

  - id: cxy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column XY Normalization Scale Factor
      21 BCS-N real number.

  - id: cxz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column XZ Normalization Scale Factor
      21 BCS-N real number.

  - id: cyy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column YY Normalization Scale Factor
      21 BCS-N real number.

  - id: cyz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column YZ Normalization Scale Factor
      21 BCS-N real number.

  - id: czz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column ZZ Normalization Scale Factor
      21 BCS-N real number.

  - id: rnis
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Row Sections in Image
      3 BCS-NPI positive integer.

  - id: cnis
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Column Sections in Image
      3 BCS-NPI positive integer.

  - id: tnis
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Total Number of Image Sections
      3 BCS-NPI positive integer (RNIS * CNIS).

  - id: rssiz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Section Size
      21 BCS-N real number (rows per section).

  - id: cssiz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Section Size
      21 BCS-N real number (columns per section).
