meta:
  id: tre_rpc00b
  title: Rapid Positioning Capability TRE (Type B)
  endian: be

doc: |
  RPC00B TRE - Rapid Positioning Capability Tagged Record Extension
  
  Provides rational polynomial coefficients for image-to-ground coordinate
  transformation. The RPC model relates image coordinates (line, sample) to
  ground coordinates (latitude, longitude, height) using rational polynomials.
  
  Total length: 1041 bytes
  
  The polynomial terms are ordered as:
  1, X, Y, Z, XY, XZ, YZ, X², Y², Z², XYZ, X³, XY², XZ², X²Y, Y³, YZ², X²Z, Y²Z, Z³
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE, Table E-22

seq:
  - id: SUCCESS
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Success Flag
      1 BCS-A character indicating if RPC fit was successful.
      '1' = success, '0' = failure

  - id: ERR_BIAS
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Error - Bias
      7 BCS-N real number representing the bias error in meters.
      Range: 0000.00 to 9999.99

  - id: ERR_RAND
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Error - Random
      7 BCS-N real number representing the random error in meters.
      Range: 0000.00 to 9999.99

  - id: LINE_OFF
    type: str
    size: 6
    encoding: BCS-NPI
    doc: |
      Line Offset
      6 BCS-NPI integer representing the line offset in pixels.
      Range: 000000 to 999999

  - id: SAMP_OFF
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Sample Offset
      5 BCS-NPI integer representing the sample offset in pixels.
      Range: 00000 to 99999

  - id: LAT_OFF
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Geodetic Latitude Offset
      8 BCS-N real number representing latitude offset in degrees.
      Range: ±90.0000

  - id: LONG_OFF
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Geodetic Longitude Offset
      9 BCS-N real number representing longitude offset in degrees.
      Range: ±180.0000

  - id: HEIGHT_OFF
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Geodetic Height Offset
      5 BCS-N integer representing height offset in meters.
      Range: ±9999

  - id: LINE_SCALE
    type: str
    size: 6
    encoding: BCS-NPI
    doc: |
      Line Scale
      6 BCS-NPI integer representing line scale factor in pixels.
      Range: 000001 to 999999

  - id: SAMP_SCALE
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Sample Scale
      5 BCS-NPI integer representing sample scale factor in pixels.
      Range: 00001 to 99999

  - id: LAT_SCALE
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Geodetic Latitude Scale
      8 BCS-N real number representing latitude scale in degrees.
      Range: 0.0000 to 90.0000

  - id: LONG_SCALE
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Geodetic Longitude Scale
      9 BCS-N real number representing longitude scale in degrees.
      Range: 0.0000 to 180.0000

  - id: HEIGHT_SCALE
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Geodetic Height Scale
      5 BCS-NPI integer representing height scale in meters.
      Range: 00001 to 99999

  - id: LINE_NUM_COEFF
    type: str
    size: 12
    encoding: BCS-A
    repeat: expr
    repeat-expr: 20
    doc: |
      Line Numerator Coefficients
      20 coefficients, each 12 BCS-A characters in scientific notation.
      Range: ±9.999999E±9

  - id: LINE_DEN_COEFF
    type: str
    size: 12
    encoding: BCS-A
    repeat: expr
    repeat-expr: 20
    doc: |
      Line Denominator Coefficients
      20 coefficients, each 12 BCS-A characters in scientific notation.
      Range: ±9.999999E±9

  - id: SAMP_NUM_COEFF
    type: str
    size: 12
    encoding: BCS-A
    repeat: expr
    repeat-expr: 20
    doc: |
      Sample Numerator Coefficients
      20 coefficients, each 12 BCS-A characters in scientific notation.
      Range: ±9.999999E±9

  - id: SAMP_DEN_COEFF
    type: str
    size: 12
    encoding: BCS-A
    repeat: expr
    repeat-expr: 20
    doc: |
      Sample Denominator Coefficients
      20 coefficients, each 12 BCS-A characters in scientific notation.
      Range: ±9.999999E±9
