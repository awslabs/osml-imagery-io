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

  - id: inclic
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Independent Error Covariance Flag
      1 BCS-A character: 'Y' or 'N'.

  - id: incluc
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Unmodeled Error Covariance Flag
      1 BCS-A character: 'Y' or 'N'.

  - id: independent_error
    type: independent_error_t
    if: inclic == "Y"
    doc: Independent error covariance data.

  - id: unmodeled_error
    type: unmodeled_error_t
    if: incluc == "Y"
    doc: Unmodeled error covariance data.

types:
  independent_error_t:
    seq:
      - id: npar
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Independent Error Parameters
          2 BCS-NPI positive integer (01-09).

      - id: nparo
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Original Parameters
          2 BCS-NPI positive integer.

      - id: ign
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Number of Ground Parameters
          2 BCS-NPI non-negative integer.

      - id: cvdate
        type: str
        size: 8
        encoding: BCS-NI
        doc: |
          Covariance Date (YYYYMMDD)
          8 BCS-NI date string.

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

      - id: ir0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row Constant Parameter Index
          2 BCS-NPI non-negative integer.

      - id: irx
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row X Parameter Index
          2 BCS-NPI non-negative integer.

      - id: iry
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row Y Parameter Index
          2 BCS-NPI non-negative integer.

      - id: irz
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Row Z Parameter Index
          2 BCS-NPI non-negative integer.

      - id: ic0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column Constant Parameter Index
          2 BCS-NPI non-negative integer.

      - id: icx
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column X Parameter Index
          2 BCS-NPI non-negative integer.

      - id: icy
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column Y Parameter Index
          2 BCS-NPI non-negative integer.

      - id: icz
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Column Z Parameter Index
          2 BCS-NPI non-negative integer.

      - id: gx0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Ground X Parameter Index
          2 BCS-NPI non-negative integer.

      - id: gy0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Ground Y Parameter Index
          2 BCS-NPI non-negative integer.

      - id: gz0
        type: str
        size: 2
        encoding: BCS-NPI
        doc: |
          Ground Z Parameter Index
          2 BCS-NPI non-negative integer.

      - id: covar
        type: str
        size: 21
        encoding: BCS-N
        repeat: expr
        repeat-expr: (npar.to_i * (npar.to_i + 1)) / 2
        doc: |
          Independent Error Covariance Matrix
          Upper triangular matrix, NPAR*(NPAR+1)/2 elements.

      - id: map_matrix
        type: map_matrix_t
        if: nparo.to_i > npar.to_i
        doc: Mapping matrix from original to reduced parameters.

  map_matrix_t:
    seq:
      - id: map
        type: str
        size: 21
        encoding: BCS-N
        repeat: expr
        repeat-expr: _parent.npar.to_i * _parent.nparo.to_i
        doc: |
          Mapping Matrix Elements
          NPAR x NPARO matrix stored row by row.

  unmodeled_error_t:
    seq:
      - id: ursrc
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row Variance - Source
          21 BCS-N real number (pixels^2).

      - id: ucsrc
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Column Variance - Source
          21 BCS-N real number (pixels^2).

      - id: urcsrc
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row-Column Covariance - Source
          21 BCS-N real number (pixels^2).

      - id: usnsr
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row Variance - Sensor
          21 BCS-N real number (pixels^2).

      - id: ucnsr
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Column Variance - Sensor
          21 BCS-N real number (pixels^2).

      - id: urcnsr
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Unmodeled Row-Column Covariance - Sensor
          21 BCS-N real number (pixels^2).

      - id: nrseg
        type: str
        size: 1
        encoding: BCS-NPI
        doc: |
          Number of Row Correlation Segments
          1 BCS-NPI digit (0-9).

      - id: row_segments
        type: correlation_segment_t
        repeat: expr
        repeat-expr: nrseg.to_i
        if: nrseg.to_i > 0
        doc: Row correlation segments.

      - id: ncseg
        type: str
        size: 1
        encoding: BCS-NPI
        doc: |
          Number of Column Correlation Segments
          1 BCS-NPI digit (0-9).

      - id: col_segments
        type: correlation_segment_t
        repeat: expr
        repeat-expr: ncseg.to_i
        if: ncseg.to_i > 0
        doc: Column correlation segments.

  correlation_segment_t:
    seq:
      - id: rho
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Correlation Coefficient
          21 BCS-N real number (-1 to 1).

      - id: tau
        type: str
        size: 21
        encoding: BCS-N
        doc: |
          Correlation Decay Coefficient
          21 BCS-N real number (pixels).
