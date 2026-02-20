meta:
  id: tre_s2evpa
  title: Stored Pixel Value to Engineering Value Polynomial TRE
  endian: be

doc: |
  S2EVPA TRE - Stored Pixel Value to Engineering Value Polynomial Tagged Record Extension
  
  Provides polynomial coefficients for converting stored pixel values to engineering
  or scientific values. This TRE is used when pixel values have been converted from
  floating point engineering values to scaled integers for storage efficiency.
  
  The polynomial conversion is defined as:
    e = SUM(COEF_n * p^(n-1)) for n = 1 to NUMCOEF
  
  Where:
  - e is the engineering value
  - p is the stored pixel value
  - COEF_n is the nth coefficient (COEF_1 is the constant term)
  
  The TRE has a variable length (32 to 152 bytes) with:
  - Optional quantity name and unit of measure
  - Band range specification (FIRST_BAND to LAST_BAND)
  - 1 to 9 polynomial coefficients
  
  Reference: STDI-0002 Volume 1, Appendix AT - S2EVPA v1.0

seq:
  - id: QUANTITY_NAME_LEN
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of QUANTITY_NAME field in bytes.
      3 BCS-N characters, range 000-999.

  - id: QUANTITY_NAME
    type: str
    size: QUANTITY_NAME_LEN.to_i
    encoding: ECS-A
    if: QUANTITY_NAME_LEN.to_i > 0
    doc: |
      Name of the engineering or scientific quantity specified by this TRE.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by QUANTITY_NAME_LEN).

  - id: UOM_LEN
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of UOM (Unit of Measure) field in bytes.
      3 BCS-N characters, range 000-999.

  - id: UOM
    type: str
    size: UOM_LEN.to_i
    encoding: ECS-A
    if: UOM_LEN.to_i > 0
    doc: |
      SI unit of measure of the engineering or scientific quantity.
      Value is case sensitive. See NITF Field Value Registry.
      For dimensionless quantities (ratios), set to "1".
      Variable length ECS-A characters (length specified by UOM_LEN).

  - id: FIRST_BAND
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Index of first band to which this polynomial applies.
      5 BCS-N characters, range 00001-99999.

  - id: LAST_BAND
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Index of last band to which this polynomial applies.
      Must be greater than or equal to FIRST_BAND.
      5 BCS-N characters, range 00001-99999.

  - id: NUMCOEF
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of polynomial coefficients.
      1 BCS-N character, range 1-9.

  - id: COEFFICIENTS
    type: coefficient
    repeat: expr
    repeat-expr: NUMCOEF.to_i
    doc: |
      Polynomial coefficients. COEF_1 is the constant term (0th degree),
      COEF_2 is the 1st degree coefficient, etc.

types:
  coefficient:
    seq:
      - id: VALUE
        type: str
        size: 15
        encoding: BCS-A
        doc: |
          Polynomial coefficient value.
          Recommended format: ±n.nnnnnnnnE±nn (scientific notation).
          Any string parseable by C99 scanf "%f" is valid.
          15 BCS-A characters.
