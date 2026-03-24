meta:
  id: tre_imrfca
  title: Image RPC TRE
  endian: be

doc: |
  IMRFCA TRE - Image Rational Polynomial Coefficients
  
  Provides rational polynomial coefficients for DPPDB
  (Digital Point Positioning Data Base) products. Contains
  four sets of 20 coefficients each for image-to-ground
  coordinate transformation.
  
  Fixed length: 1760 bytes (4 × 20 × 22).
  
  Derived from GDAL nitf_spec.xml definition (2026-03-24):
  https://github.com/OSGeo/gdal/blob/master/frmts/nitf/data/nitf_spec.xml
  
  Reference: Table 69 of DPPDB specification (MIL-STD-89034)

seq:
  - id: XINC
    type: str
    size: 22
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      X Image Numerator Coefficients
      20 coefficients, each 22 BCS-N real.

  - id: XIDC
    type: str
    size: 22
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      X Image Denominator Coefficients
      20 coefficients, each 22 BCS-N real.

  - id: YINC
    type: str
    size: 22
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      Y Image Numerator Coefficients
      20 coefficients, each 22 BCS-N real.

  - id: YIDC
    type: str
    size: 22
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      Y Image Denominator Coefficients
      20 coefficients, each 22 BCS-N real.
