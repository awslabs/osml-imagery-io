meta:
  id: tre_cswrpb
  title: Common Sensor Warping Terms TRE
  endian: be

doc: |
  CSWRPB TRE - Common Sensor Warping Terms
  Version 1.2
  
  Part of the GLAS/GFM (Generic Linear Array Scanner / Generic Frame-sequence Model)
  support data extensions. Accommodates the general case of a scanner when samples
  along a line of an image were not all imaged at the same time. For a framer, this
  TRE can model the effects of optical distortion in the image, pair-wise rectifying
  it to aid in stereo viewing, or image stabilization.
  
  This TRE provides the de-warping information needed to handle these situations,
  including polynomial coefficients for line and sample de-warping transformations.
  
  Reference: STDI-0002 Volume 2, Appendix M - GLAS-GFM

seq:
  - id: NUM_SETS_WARP_DATA
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of Sets of Warping Data
      Number of sets of warping data in this instance of the CSWRPB TRE.
      If the sensor is a scanner, there shall be one set of warping data.
      1 BCS-N integer, range 1-9.

  - id: SENSOR_TYPE
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Sensor Type
      Identifies the type of sensor that collected the image.
      S = line scanner, F = framing array.

  - id: WRP_INTERP
    type: str
    size: 1
    encoding: BCS-N
    if: SENSOR_TYPE == "F"
    doc: |
      Warping Interpolation Type (conditional: SENSOR_TYPE = F)
      Identifies the type of warping interpolation between sets of corrections.
      0 = nearest neighbor, 1 = linear.

  - id: WARP_SETS
    type: warp_set_t(_index)
    repeat: expr
    repeat-expr: NUM_SETS_WARP_DATA.to_i
    doc: |
      Warping Data Sets
      Array of warping data sets containing normalization parameters and
      polynomial coefficients.

  - id: RESERVED_LEN
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Length of Reserved Field
      This field value shall be "00000".
      5 BCS-N integer.

types:
  warp_set_t:
    params:
      - id: SET_INDEX
        type: s4
    doc: |
      Warping Data Set
      Contains normalization parameters and polynomial coefficients for
      line and sample de-warping.
    seq:
      - id: FL_WARP
        type: str
        size: 11
        encoding: BCS-N
        if: _root.SENSOR_TYPE == "F"
        doc: |
          Focal Length Associated with this Set of Warping Data
          (conditional: SENSOR_TYPE = F)
          11 BCS-N real number, range 00.00000000 to 99.99999999 meters.

      - id: OFFSET_LINE
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Line Coordinate Normalization Offset
          7 BCS-N integer, range 0000001 to 9999999 rows.

      - id: OFFSET_SAMP
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Sample Coordinate Normalization Offset
          7 BCS-N integer, range 0000001 to 9999999 columns.

      - id: SCALE_LINE
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Line Coordinate Normalization Scale
          7 BCS-N integer, range 0000001 to 9999999 rows.

      - id: SCALE_SAMP
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Sample Coordinate Normalization Scale
          7 BCS-N integer, range 0000001 to 9999999 columns.

      - id: OFFSET_LINE_UNWRP
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Unwarped Line Coordinate Normalization Offset
          7 BCS-N integer, range 0000001 to 9999999 rows.

      - id: OFFSET_SAMP_UNWRP
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Unwarped Sample Coordinate Normalization Offset
          7 BCS-N integer, range 0000001 to 9999999 columns.

      - id: SCALE_LINE_UNWRP
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Unwarped Line Coordinate Normalization Scale
          7 BCS-N integer, range 0000001 to 9999999 rows.

      - id: SCALE_SAMP_UNWRP
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Unwarped Sample Coordinate Normalization Scale
          7 BCS-N integer, range 0000001 to 9999999 columns.

      - id: LINE_POLY_ORDER_M1
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Order of Line De-Warping Polynomial (Line Dependency)
          1 BCS-N integer, range 0-9.

      - id: LINE_POLY_ORDER_M2
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Order of Line De-Warping Polynomial (Sample Dependency)
          1 BCS-N integer, range 0-9.

      - id: SAMP_POLY_ORDER_N1
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Order of Sample De-Warping Polynomial (Line Dependency)
          1 BCS-N integer, range 0-9.

      - id: SAMP_POLY_ORDER_N2
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Order of Sample De-Warping Polynomial (Sample Dependency)
          1 BCS-N integer, range 0-9.

      - id: LINE_POLY_COEFFS
        type: str
        size: 21
        encoding: BCS-A
        repeat: expr
        repeat-expr: (LINE_POLY_ORDER_M1.to_i + 1) * (LINE_POLY_ORDER_M2.to_i + 1)
        doc: |
          Line De-Warping Polynomial Coefficients A(i,j)
          Coefficients for the line de-warping polynomial.
          21 BCS-A scientific notation, range -9.99999999999999E±99 to +9.99999999999999E±99.
          Total coefficients = (LINE_POLY_ORDER_M1 + 1) × (LINE_POLY_ORDER_M2 + 1).

      - id: SAMP_POLY_COEFFS
        type: str
        size: 21
        encoding: BCS-A
        repeat: expr
        repeat-expr: (SAMP_POLY_ORDER_N1.to_i + 1) * (SAMP_POLY_ORDER_N2.to_i + 1)
        doc: |
          Sample De-Warping Polynomial Coefficients B(i,j)
          Coefficients for the sample de-warping polynomial.
          21 BCS-A scientific notation, range -9.99999999999999E±99 to +9.99999999999999E±99.
          Total coefficients = (SAMP_POLY_ORDER_N1 + 1) × (SAMP_POLY_ORDER_N2 + 1).
