meta:
  id: tre_iomapa
  title: Input/Output Amplitude Mapping TRE
  endian: be

doc: |
  IOMAPA TRE - Input/Output Amplitude Mapping Extension
  
  Contains data necessary to perform output amplitude mapping process
  for each scan within each image frame. Post-processing applied after
  image data has undergone expansion using 12-bit JPEG/DCT algorithm.
  
  The structure varies based on MAP_SELECT value:
  - Method 0: No mapping — S2 scale factor only (CEDATA = 6 bytes)
  - Method 1: Lookup table — TABLE_ID, S1, S2, 4096 output map values (CEDATA = 8202 bytes)
  - Method 2: Log mapping — TABLE_ID, S1, S2, R_WHOLE, R_FRACTION (CEDATA = 16 bytes)
  - Method 3: Polynomial — TABLE_ID, S1, S2, 3 segments with boundaries and coefficients (CEDATA = 91 bytes)
  
  All method-specific fields are fully parsed using string comparison
  conditions on MAP_SELECT. Output map values in Method 1 are 2-byte
  unsigned integers (big-endian). Polynomial coefficients in Method 3
  are IEEE 754 single-precision floats (4 bytes, big-endian).
  
  Reference: STDI-0002 Volume 1, Appendix F - IOMAPA

seq:
  - id: BAND_NUMBER
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Band Identifier. 000 for monochrome or single band imagery.
      Range: 000-999. 3 BCS-N.

  - id: MAP_SELECT
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Mapping Method to Apply.
      0 = No mapping, 1 = Lookup table, 2 = Log mapping, 3 = Polynomial.
      1 BCS-N, range 0-3.

  # --- Method 0: S2 only ---
  - id: S2_METHOD0
    type: str
    size: 2
    encoding: BCS-N
    if: "MAP_SELECT == \"0\""
    doc: |
      Scale Factor 2 (Method 0). Output precision scale change.
      Range: 00-11. 2 BCS-N.

  # --- Methods 1, 2, 3: Common fields (TABLE_ID, S1, S2) ---
  - id: TABLE_ID
    type: str
    size: 2
    encoding: BCS-N
    if: "MAP_SELECT == \"1\" or MAP_SELECT == \"2\" or MAP_SELECT == \"3\""
    doc: |
      I/O Table Used. Diagnostic identifier, not needed for output mapping.
      Range: 00-99. 2 BCS-N. Optional.

  - id: S1
    type: str
    size: 2
    encoding: BCS-N
    if: "MAP_SELECT == \"1\" or MAP_SELECT == \"2\" or MAP_SELECT == \"3\""
    doc: |
      Scale Factor 1. Scales input data precision up to 12 bits.
      For 8-bit input data, S1 = 4. Range: 00-11. 2 BCS-N.

  - id: S2
    type: str
    size: 2
    encoding: BCS-N
    if: "MAP_SELECT == \"1\" or MAP_SELECT == \"2\" or MAP_SELECT == \"3\""
    doc: |
      Scale Factor 2. Output precision scale change.
      Limited to S2 < (12 - S1). Range: 00-11. 2 BCS-N.

  # --- Method 1: Lookup table (4096 output map values) ---
  - id: OUTPUT_MAP_VALUES
    size: 2
    repeat: expr
    repeat-expr: 4096
    if: "MAP_SELECT == \"1\""
    doc: |
      Output mapping values for lookup table method.
      4096 entries, each a 2-byte unsigned integer (big-endian).
      Values range 0-4095. Index 0 through 4095.

  # --- Method 2: Log mapping (R_WHOLE, R_FRACTION) ---
  - id: R_WHOLE
    type: str
    size: 3
    encoding: BCS-N
    if: "MAP_SELECT == \"2\""
    doc: |
      R Scaling Factor - Whole Part.
      R = R_WHOLE + (R_FRACTION / 256).
      Range: 000-999. 3 BCS-N.

  - id: R_FRACTION
    type: str
    size: 3
    encoding: BCS-N
    if: "MAP_SELECT == \"2\""
    doc: |
      R Scaling Factor - Fractional Part.
      R = R_WHOLE + (R_FRACTION / 256).
      Range: 000-255. 3 BCS-N.

  # --- Method 3: Polynomial mapping (3 segments) ---
  - id: NO_OF_SEGMENTS
    type: str
    size: 1
    encoding: BCS-N
    if: "MAP_SELECT == \"3\""
    doc: |
      Number of polynomial segments. Always 3. 1 BCS-N.

  - id: XOB_1
    type: str
    size: 4
    encoding: BCS-N
    if: "MAP_SELECT == \"3\""
    doc: "Segment boundary 1. Range: 0000-4095. 4 BCS-N."

  - id: XOB_2
    type: str
    size: 4
    encoding: BCS-N
    if: "MAP_SELECT == \"3\""
    doc: "Segment boundary 2. Range: 0000-4095. 4 BCS-N."

  # Segment 1 coefficients (B0 through B5, IEEE 754 float)
  - id: OUT_B0_1
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B0 coefficient of 1st segment. IEEE 754 single-precision float."

  - id: OUT_B1_1
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B1 coefficient of 1st segment. IEEE 754 single-precision float."

  - id: OUT_B2_1
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B2 coefficient of 1st segment. IEEE 754 single-precision float."

  - id: OUT_B3_1
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B3 coefficient of 1st segment. IEEE 754 single-precision float."

  - id: OUT_B4_1
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B4 coefficient of 1st segment. IEEE 754 single-precision float."

  - id: OUT_B5_1
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B5 coefficient of 1st segment. IEEE 754 single-precision float."

  # Segment 2 coefficients
  - id: OUT_B0_2
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B0 coefficient of 2nd segment. IEEE 754 single-precision float."

  - id: OUT_B1_2
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B1 coefficient of 2nd segment. IEEE 754 single-precision float."

  - id: OUT_B2_2
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B2 coefficient of 2nd segment. IEEE 754 single-precision float."

  - id: OUT_B3_2
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B3 coefficient of 2nd segment. IEEE 754 single-precision float."

  - id: OUT_B4_2
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B4 coefficient of 2nd segment. IEEE 754 single-precision float."

  - id: OUT_B5_2
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B5 coefficient of 2nd segment. IEEE 754 single-precision float."

  # Segment 3 coefficients
  - id: OUT_B0_3
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B0 coefficient of 3rd segment. IEEE 754 single-precision float."

  - id: OUT_B1_3
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B1 coefficient of 3rd segment. IEEE 754 single-precision float."

  - id: OUT_B2_3
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B2 coefficient of 3rd segment. IEEE 754 single-precision float."

  - id: OUT_B3_3
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B3 coefficient of 3rd segment. IEEE 754 single-precision float."

  - id: OUT_B4_3
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B4 coefficient of 3rd segment. IEEE 754 single-precision float."

  - id: OUT_B5_3
    size: 4
    if: "MAP_SELECT == \"3\""
    doc: "B5 coefficient of 3rd segment. IEEE 754 single-precision float."
