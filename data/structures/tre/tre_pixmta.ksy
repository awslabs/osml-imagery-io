meta:
  id: tre_pixmta
  title: Pixel Metric TRE
  endian: be

doc: |
  PIXMTA TRE - Pixel Metric Tagged Record Extension
  
  The PIXMTA TRE coupled with the Pixel Metric Image Segment (PMIS) allows
  a data provider to use a NITF image segment to specify a grid of data that
  are geometrically tied to the pixels of another image.
  
  The TRE specifies:
  - Semantic meaning of each pixel metric in the PMIS
  - Conversion of stored pixel values to engineering/scientific units
  - Image segment(s) to which the PMIS is associated
  - Geometric scale factor and origin relating PMIS to AIS coordinates
  - Sampling mode for projecting PMIS values onto AIS
  - Band mapping between PMIS and AIS
  
  Reference: STDI-0002 Volume 1, Appendix AJ - PIXMTA

seq:
  - id: NUMAIS
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Associated Image Segments (NUMAIS)
      Number of image segments associated with the PMIS.
      3 BCS-A characters: "000" to "998", or "ALL".
      "000" = not associated with other segments in same file.
      "ALL" = associated with all segments except other PMISs or PQSs.

  - id: AISDLVL
    type: str
    size: 3
    encoding: BCS-N
    repeat: expr
    repeat-expr: NUMAIS.to_i
    if: NUMAIS != "000" and NUMAIS != "ALL"
    doc: |
      Display Level of Associated Image Segment (AISDLVL)
      Image Display Level (IDLVL) of each associated image segment.
      3 BCS-N characters, range 001-999.
      Repeated NUMAIS times (omitted if NUMAIS is "000" or "ALL").

  - id: ORIGIN_X
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Column Position of Upper Left Pixel Metric (ORIGIN_X)
      Floating-point column of AIS corresponding to PMIS pixel [0,0].
      14 BCS-A characters in scientific notation (±n.nnnnnnnE±nn).
      Special value +4.9999999E+07 for compact form single-column PMIS.

  - id: ORIGIN_Y
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Row Position of Upper Left Pixel Metric (ORIGIN_Y)
      Floating-point row of AIS corresponding to PMIS pixel [0,0].
      14 BCS-A characters in scientific notation (±n.nnnnnnnE±nn).
      Special value +4.9999999E+07 for compact form single-row PMIS.

  - id: SCALE_X
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Column-Based Scale Factor (SCALE_X)
      Scale factor relating PMIS column positions to AIS column positions.
      14 BCS-A characters in scientific notation (+n.nnnnnnnE±nn).
      Special value +9.9999999E+07 for compact form single-column PMIS.

  - id: SCALE_Y
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Row-Based Scale Factor (SCALE_Y)
      Scale factor relating PMIS row positions to AIS row positions.
      14 BCS-A characters in scientific notation (+n.nnnnnnnE±nn).
      Special value +9.9999999E+07 for compact form single-row PMIS.

  - id: SAMPLE_MODE
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Pixel Metric Sampling Mode (SAMPLE_MODE)
      How PMIS metric values are projected onto AIS.
      "F" = Fill mode (entire region filled with metric value).
      "I" = Interpolated mode (interpolate between PMIS pixels).
      1 BCS-A character.

  - id: NUMMETRICS
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Metrics (NUMMETRICS)
      Number of separate metrics in the PMIS.
      5 BCS-N characters, range 00001-99999.

  - id: PERBAND
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Per Band Metric Flag (PERBAND)
      Whether metrics apply to all AIS bands or per-band.
      "A" = metrics apply to all AIS bands.
      "P" = separate metrics for each AIS band.
      1 BCS-A character.

  - id: METRICS
    type: metric_entry
    repeat: expr
    repeat-expr: NUMMETRICS.to_i
    doc: |
      Metric entries.
      Repeated NUMMETRICS times.

  - id: RESERVED_LEN
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Size of Reserved Field (RESERVED_LEN)
      Size in bytes of the RESERVED field.
      5 BCS-N characters, default "00000".

  - id: RESERVED
    size: RESERVED_LEN.to_i
    if: RESERVED_LEN.to_i > 0
    doc: |
      Reserved Data Field (RESERVED)
      Reserved for future use by NTB.
      Variable length based on RESERVED_LEN.

types:
  metric_entry:
    seq:
      - id: DESCRIPTION
        type: str
        size: 40
        encoding: BCS-A
        doc: |
          Description of Pixel Metric (DESCRIPTION)
          Descriptive label for the metric, maps to ISUBCATn in PMIS subheader.
          40 BCS-A characters.

      - id: UNIT
        type: str
        size: 40
        encoding: ECS-A
        doc: |
          Unit of Measure (UNIT)
          Unit of measure for the metric after conversion to engineering values.
          40 ECS-A characters.

      - id: FITTYPE
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Mathematical Form of Data Transformation (FITTYPE)
          "P" = polynomial transform (uses NUMCOEF and COEF fields).
          "D" = direct (stored values equal engineering values).
          1 BCS-A character.

      - id: NUMCOEF
        type: str
        size: 1
        encoding: BCS-N
        if: FITTYPE == "P"
        doc: |
          Number of Coefficients (NUMCOEF)
          Number of polynomial coefficients for transformation.
          1 BCS-N character, range 1-9.
          Only present if FITTYPE = "P".

      - id: COEF
        type: str
        size: 15
        encoding: BCS-A
        repeat: expr
        repeat-expr: NUMCOEF.to_i
        if: FITTYPE == "P"
        doc: |
          Transformation Coefficient (COEF)
          Polynomial coefficients for converting stored values to engineering values.
          15 BCS-A characters in scientific notation (±n.nnnnnnnnE±nn).
          Engineering value e = sum(COEF[j] * p^j) for j=0 to NUMCOEF-1.
          Only present if FITTYPE = "P".
