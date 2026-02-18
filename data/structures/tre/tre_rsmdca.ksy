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
  
  CEL: 597-99988 bytes (variable based on number of parameters and images)
  
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

  - id: tid
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Triangulation ID
      40 BCS-A characters identifying the triangulation solution.

  - id: npar
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Number of Parameters per Image
      2 BCS-NPI positive integer (01-36).

  - id: nimge
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Images
      3 BCS-NPI positive integer.

  - id: npart
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Total Number of Parameters
      5 BCS-NPI positive integer (NPAR * NIMGE).

  - id: images
    type: image_params_t
    repeat: expr
    repeat-expr: nimge.to_i
    doc: |
      Image Parameter Information
      NIMGE image parameter records.

  - id: xuol
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Coordinate of Local Origin
      21 BCS-N real number (meters).

  - id: yuol
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Coordinate of Local Origin
      21 BCS-N real number (meters).

  - id: zuol
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Coordinate of Local Origin
      21 BCS-N real number (meters).

  - id: xuxl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Local X Axis
      21 BCS-N real number.

  - id: xuyl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Local X Axis
      21 BCS-N real number.

  - id: xuzl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Local X Axis
      21 BCS-N real number.

  - id: yuxl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Local Y Axis
      21 BCS-N real number.

  - id: yuyl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Local Y Axis
      21 BCS-N real number.

  - id: yuzl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Local Y Axis
      21 BCS-N real number.

  - id: zuxl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Local Z Axis
      21 BCS-N real number.

  - id: zuyl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Local Z Axis
      21 BCS-N real number.

  - id: zuzl
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Local Z Axis
      21 BCS-N real number.

  - id: iro
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row Offset Parameter Index
      2 BCS-NPI non-negative integer (00 = not used).

  - id: irx
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row X Parameter Index
      2 BCS-NPI non-negative integer.

  - id: iry
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row Y Parameter Index
      2 BCS-NPI non-negative integer.

  - id: irz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row Z Parameter Index
      2 BCS-NPI non-negative integer.

  - id: irxx
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row XX Parameter Index
      2 BCS-NPI non-negative integer.

  - id: irxy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row XY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: irxz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row XZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: iryy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row YY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: iryz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row YZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: irzz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Row ZZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: ico
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icx
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column X Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column Y Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column Z Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icxx
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column XX Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icxy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column XY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icxz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column XZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icyy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column YY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: icyz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column YZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: iczz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Image Column ZZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gxo
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground X Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gyo
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Y Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gzo
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Z Offset Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gxr
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground X Rotation Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gyr
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Y Rotation Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gzr
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Z Rotation Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gs
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Scale Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gxx
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground XX Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gxy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground XY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gxz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground XZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gyy
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground YY Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gyz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground YZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: gzz
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground ZZ Parameter Index
      2 BCS-NPI non-negative integer.

  - id: dercov
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: (npart.to_i * (npart.to_i + 1)) / 2
    doc: |
      Direct Error Covariance Matrix Elements
      Upper triangular covariance matrix stored row by row.
      Number of elements = NPART * (NPART + 1) / 2.

types:
  image_params_t:
    seq:
      - id: iidi
        type: str
        size: 80
        encoding: BCS-A
        doc: |
          Image Identifier for Image i
          80 BCS-A characters.

      - id: npari
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Parameters for Image i
          2 BCS-NPI positive integer.
