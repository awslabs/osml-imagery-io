meta:
  id: tre_rsmapa
  title: RSM Adjustable Parameters TRE
  endian: be

doc: |
  RSMAPA TRE - Replacement Sensor Model Adjustable Parameters
  
  Provides adjustable parameter values for RSM error propagation.
  Contains image identifier, triangulation ID, local coordinate system
  definition, parameter indices, and the current values of the
  adjustable parameters.
  
  CEL: 507-1243 bytes (variable based on number of parameters)
  
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
      Number of Adjustable Parameters
      2 BCS-NPI positive integer (01-35).

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

  - id: PARVAL
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: NPAR.to_i
    doc: |
      Adjustable Parameter Values
      NPAR parameter values, each 21 BCS-N real number.
