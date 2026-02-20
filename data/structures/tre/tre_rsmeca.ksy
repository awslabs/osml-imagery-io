meta:
  id: tre_rsmeca
  title: RSM Error Covariance TRE
  endian: be

doc: |
  RSMECA TRE - Replacement Sensor Model Error Covariance
  
  Provides error covariance data for RSM. Contains independent and
  unmodeled error covariance information, correlation segments for
  row and column errors, and optional mapping matrix data.
  
  CEL: 354-43045 bytes (variable based on conditional sections)
  
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

  - id: INCLIC
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Independent Error Covariance Flag
      1 BCS-A character: 'Y' or 'N'.

  - id: INCLUC
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Unmodeled Error Covariance Flag
      1 BCS-A character: 'Y' or 'N'.

  - id: INDEPENDENT_ERROR
    type: independent_error_t
    if: INCLIC == "Y"
    doc: Independent error covariance data.

  - id: UNMODELED_ERROR
    type: unmodeled_error_t
    if: INCLUC == "Y"
    doc: Unmodeled error covariance data.

types:
  independent_error_t:
    seq:
      - id: NPAR
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Independent Error Parameters
          2 BCS-NPI positive integer (01-09).

      - id: NPARO
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Original Parameters
          2 BCS-NPI positive integer.

      - id: IGN
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Ground Parameters
          2 BCS-NPI non-negative integer.

      - id: CVDATE
        type: str
        size: 8
        encoding: BCS-NI
        doc: |
          Covariance Date (YYYYMMDD)
          8 BCS-NI date string.

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

      - id: IR0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row Constant Parameter Index
          2 BCS-NPI non-negative integer.

      - id: IRX
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row X Parameter Index
          2 BCS-NPI non-negative integer.

      - id: IRY
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row Y Parameter Index
          2 BCS-NPI non-negative integer.

      - id: IRZ
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row Z Parameter Index
          2 BCS-NPI non-negative integer.

      - id: IC0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column Constant Parameter Index
          2 BCS-NPI non-negative integer.

      - id: ICX
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column X Parameter Index
          2 BCS-NPI non-negative integer.

      - id: ICY
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column Y Parameter Index
          2 BCS-NPI non-negative integer.

      - id: ICZ
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column Z Parameter Index
          2 BCS-NPI non-negative integer.

      - id: GX0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Ground X Parameter Index
          2 BCS-NPI non-negative integer.

      - id: GY0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Ground Y Parameter Index
          2 BCS-NPI non-negative integer.

      - id: GZ0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Ground Z Parameter Index
          2 BCS-NPI non-negative integer.

      - id: COVAR
        type: str
        size: 21
        encoding: BCS-N
        repeat: expr
        repeat-expr: (NPAR.to_i * (NPAR.to_i + 1)) / 2
        doc: |
          Independent Error Covariance Matrix
          Upper triangular matrix, NPAR*(NPAR+1)/2 elements.

      - id: MAP_MATRIX
        type: map_matrix_t
        if: NPARO.to_i > NPAR.to_i
        doc: Mapping matrix from original to reduced parameters.

  map_matrix_t:
    seq:
      - id: MAP
        type: str
        size: 21
        encoding: BCS-N
        repeat: expr
        repeat-expr: _parent.NPAR.to_i * _parent.NPARO.to_i
        doc: |
          Mapping Matrix Elements
          NPAR x NPARO matrix stored row by row.

  unmodeled_error_t:
    seq:
      - id: URSRC
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row Variance - Source
          21 BCS-N real number (pixels^2).

      - id: UCSRC
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Column Variance - Source
          21 BCS-N real number (pixels^2).

      - id: URCSRC
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row-Column Covariance - Source
          21 BCS-N real number (pixels^2).

      - id: USNSR
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row Variance - Sensor
          21 BCS-N real number (pixels^2).

      - id: UCNSR
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Column Variance - Sensor
          21 BCS-N real number (pixels^2).

      - id: URCNSR
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row-Column Covariance - Sensor
          21 BCS-N real number (pixels^2).

      - id: NRSEG
        type: str
        size: 1
        encoding: BCS-NPI
        doc: |
          Number of Row Correlation Segments
          1 BCS-NPI digit (0-9).

      - id: ROW_SEGMENTS
        type: correlation_segment_t
        repeat: expr
        repeat-expr: NRSEG.to_i
        if: NRSEG.to_i > 0
        doc: Row correlation segments.

      - id: NCSEG
        type: str
        size: 1
        encoding: BCS-NPI
        doc: |
          Number of Column Correlation Segments
          1 BCS-NPI digit (0-9).

      - id: COL_SEGMENTS
        type: correlation_segment_t
        repeat: expr
        repeat-expr: NCSEG.to_i
        if: NCSEG.to_i > 0
        doc: Column correlation segments.

  correlation_segment_t:
    seq:
      - id: RHO
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Correlation Coefficient
          21 BCS-N real number (-1 to 1).

      - id: TAU
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Correlation Decay Coefficient
          21 BCS-N real number (pixels).
