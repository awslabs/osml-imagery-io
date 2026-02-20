meta:
  id: tre_rsmdca
  title: RSM Direct Error Covariance TRE
  endian: be

doc: |
  RSMDCA TRE - Replacement Sensor Model Direct Error Covariance
  
  Provides direct error covariance data for RSM adjustable parameters.
  Contains image identifiers, local coordinate system definition,
  parameter indices, and the full covariance matrix for the adjustable
  parameters across multiple images.
  
  CEL: 597-99988 bytes (variable based on number of parameters and IMAGES)
  
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

  - id: TID
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Triangulation ID
      40 BCS-A characters identifying the triangulation solution.

  - id: NPAR
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Number of Parameters per Image
      2 BCS-NPI positive integer (01-36).

  - id: NIMGE
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Images
      3 BCS-NPI positive integer.

  - id: NPART
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Total Number of Parameters
      5 BCS-NPI positive integer (NPAR * NIMGE).

  - id: IMAGES
    type: image_params_t
    repeat: expr
    repeat-expr: NIMGE.to_i
    doc: |
      Image Parameter Information
      NIMGE image parameter records.

  - id: XUOL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Coordinate of Local Origin
      21 BCS-N real number (meters).

  - id: YUOL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Coordinate of Local Origin
      21 BCS-N real number (meters).

  - id: ZUOL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Coordinate of Local Origin
      21 BCS-N real number (meters).

  - id: XUXL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Local X Axis
      21 BCS-N real number.

  - id: XUYL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Local X Axis
      21 BCS-N real number.

  - id: XUZL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Local X Axis
      21 BCS-N real number.

  - id: YUXL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Local Y Axis
      21 BCS-N real number.

  - id: YUYL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Local Y Axis
      21 BCS-N real number.

  - id: YUZL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Local Y Axis
      21 BCS-N real number.

  - id: ZUXL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Local Z Axis
      21 BCS-N real number.

  - id: ZUYL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Local Z Axis
      21 BCS-N real number.

  - id: ZUZL
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Local Z Axis
      21 BCS-N real number.

  - id: IRO
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row Offset Parameter Index
      2 BCS-NPI non-negative integer (00 = not used).

  - id: IRX
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row X Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row Y Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row Z Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRXX
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row XX Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRXY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row XY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRXZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row XZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRYY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row YY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRYZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row YZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: IRZZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row ZZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICO
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICX
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column X Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column Y Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column Z Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICXX
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column XX Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICXY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column XY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICXZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column XZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICYY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column YY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICYZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column YZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ICZZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column ZZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GXO
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground X Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GYO
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Y Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GZO
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Z Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GXR
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground X Rotation Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GYR
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Y Rotation Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GZR
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Z Rotation Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GS
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Scale Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GXX
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground XX Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GXY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground XY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GXZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground XZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GYY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground YY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GYZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground YZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: GZZ
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground ZZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: DERCOV
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: (NPART.to_i * (NPART.to_i + 1)) / 2
    doc: |
      Direct Error Covariance Matrix Elements
      Upper triangular covariance matrix stored row by row.
      Number of elements = NPART * (NPART + 1) / 2.

types:
  image_params_t:
    seq:
      - id: IIDI
        type: str
        size: 80
        encoding: BCS-A
        doc: |
          Image Identifier for Image i
          80 BCS-A characters.

      - id: NPARI
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Parameters for Image i
          2 BCS-NPI positive integer.
