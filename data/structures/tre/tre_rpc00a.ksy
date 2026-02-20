meta:
  id: tre_rpc00a
  title: Rapid Positioning Capability TRE (Type A)
  endian: be

doc: |
  RPC00A TRE - Rapid Positioning Capability Tagged Record Extension (Type A)
  
  Provides rational polynomial coefficients for image-to-ground coordinate
  transformation. The RPC model relates image coordinates (line, sample) to
  ground coordinates (latitude, longitude, height) using rational polynomials.
  
  Total length: 1041 bytes
  
  IMPORTANT: RPC00A uses a DIFFERENT polynomial term order than RPC00B.
  The exact term order for RPC00A is defined in STDI-0001, which is not
  publicly available. The field layout (sizes, types) is identical to RPC00B,
  but the interpretation of coefficients differs due to term ordering.
  
  Per STDI-0002 Vol-1-App E (ASDE), Section E.2.4:
  "Note: The order of terms differs between different applications.
  This order is used with RPC00B and the Digital Point Positioning Data Base.
  RPC00A uses a different term order."
  
  Reference: STDI-0001 (not publicly available)
  See also: STDI-0002 Volume 1, Appendix E - ASDE, Section E.3.12

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
    encoding: BCS-NPI
    doc: |
      Geodetic Height Offset
      5 BCS-NPI integer representing height offset in meters.
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
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      Line Numerator Coefficients
      20 coefficients, each 12 BCS-N real numbers.
      Term order differs from RPC00B - see STDI-0001 for details.
      Range: ±9.999999E±9

  - id: LINE_DEN_COEFF
    type: str
    size: 12
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      Line Denominator Coefficients
      20 coefficients, each 12 BCS-N real numbers.
      Term order differs from RPC00B - see STDI-0001 for details.
      Range: ±9.999999E±9

  - id: SAMP_NUM_COEFF
    type: str
    size: 12
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      Sample Numerator Coefficients
      20 coefficients, each 12 BCS-N real numbers.
      Term order differs from RPC00B - see STDI-0001 for details.
      Range: ±9.999999E±9

  - id: SAMP_DEN_COEFF
    type: str
    size: 12
    encoding: BCS-N
    repeat: expr
    repeat-expr: 20
    doc: |
      Sample Denominator Coefficients
      20 coefficients, each 12 BCS-N real numbers.
      Term order differs from RPC00B - see STDI-0001 for details.
      Range: ±9.999999E±9
