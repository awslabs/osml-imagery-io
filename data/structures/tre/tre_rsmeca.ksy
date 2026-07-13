meta:
  id: tre_rsmeca
  title: RSM Error Covariance TRE
  endian: be

doc: |
  RSMECA TRE - Replacement Sensor Model Error Covariance

  Provides indirect error covariance data for RSM. When INCLIC=Y the record
  carries the Local rectangular coordinate system definition, the RSM
  Adjustable Parameter Choice Set (36 index fields: 20 image-space + 16
  ground-space), the per-independent error subgroup covariance data, and the
  mapping matrix. When INCLUC=Y the unmodeled error covariance block is present.

  CEL: 354-43045 bytes (variable based on conditional sections).

  Reference: STDI-0002 Volume 1, Appendix U, Section 12.7, Table 9
  (pp. U-152 to U-165).

seq:
  - id: IID
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A. Identifies the original full image; all spaces if unavailable.

  - id: EDITION
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A.

  - id: TID
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Triangulation ID
      40 BCS-A. All spaces if there has been no such process.

  - id: INCLIC
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Indirect Error Covariance Flag
      1 BCS-A. Y=indirect error covariance included, N=not included.

  - id: INCLUC
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Unmodeled Error Covariance Flag
      1 BCS-A. Y=unmodeled error covariance included, N=not included.

  - id: INDIRECT_ERROR
    type: indirect_error_t
    if: INCLIC == "Y"
    doc: Indirect error covariance data (only when INCLIC=Y).

  - id: UNMODELED_ERROR
    type: unmodeled_error_t
    if: INCLUC == "Y"
    doc: Unmodeled error covariance data (only when INCLUC=Y).

types:
  indirect_error_t:
    seq:
      - id: NPAR
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of RSM Adjustable Parameters
          2 BCS-N integer, range 01-36. Both row and column dimension of the
          (mapped) RSM image error covariance and row dimension of MAP.
      - id: NPARO
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Original Adjustable Parameters
          2 BCS-N integer, range 01-36. Both row and column dimension of the
          (unmapped) original image error covariance and column dimension of MAP.
      - id: IGN
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Independent Subgroups
          2 BCS-N integer, range 01-36.
      - id: CVDATE
        type: str
        size: 8
        encoding: BCS-A
        doc: |
          Version Date of the Original Image Error Covariance (YYYYMMDD)
          8 BCS-A. All spaces if not populated.

      - id: XUOL
        type: str
        size: 21
        encoding: BCS-A
        doc: Local Coordinate Origin X (21 BCS-A real, meters).
      - id: YUOL
        type: str
        size: 21
        encoding: BCS-A
        doc: Local Coordinate Origin Y (21 BCS-A real, meters).
      - id: ZUOL
        type: str
        size: 21
        encoding: BCS-A
        doc: Local Coordinate Origin Z (21 BCS-A real, meters).
      - id: XUXL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector X component for Local X axis (21 BCS-A real, -1 to 1).
      - id: XUYL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector X component for Local Y axis (21 BCS-A real, -1 to 1).
      - id: XUZL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector X component for Local Z axis (21 BCS-A real, -1 to 1).
      - id: YUXL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector Y component for Local X axis (21 BCS-A real, -1 to 1).
      - id: YUYL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector Y component for Local Y axis (21 BCS-A real, -1 to 1).
      - id: YUZL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector Y component for Local Z axis (21 BCS-A real, -1 to 1).
      - id: ZUXL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector Z component for Local X axis (21 BCS-A real, -1 to 1).
      - id: ZUYL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector Z component for Local Y axis (21 BCS-A real, -1 to 1).
      - id: ZUZL
        type: str
        size: 21
        encoding: BCS-A
        doc: Unit Vector Z component for Local Z axis (21 BCS-A real, -1 to 1).

      # RSM Adjustable Parameter Choice Set: 20 image-space index fields
      # followed by 16 ground-space index fields. Each is a 2 BCS-A index into
      # the RSM error cross-covariance (01-36), all spaces if the parameter is
      # not active.
      - id: IRO
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row Constant Index (2 BCS-A, 01-36 or spaces).
      - id: IRX
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row X Index (2 BCS-A).
      - id: IRY
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row Y Index (2 BCS-A).
      - id: IRZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row Z Index (2 BCS-A).
      - id: IRXX
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row X^2 Index (2 BCS-A).
      - id: IRXY
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row XY Index (2 BCS-A).
      - id: IRXZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row XZ Index (2 BCS-A).
      - id: IRYY
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row Y^2 Index (2 BCS-A).
      - id: IRYZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row YZ Index (2 BCS-A).
      - id: IRZZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Row Z^2 Index (2 BCS-A).
      - id: ICO
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column Constant Index (2 BCS-A).
      - id: ICX
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column X Index (2 BCS-A).
      - id: ICY
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column Y Index (2 BCS-A).
      - id: ICZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column Z Index (2 BCS-A).
      - id: ICXX
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column X^2 Index (2 BCS-A).
      - id: ICXY
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column XY Index (2 BCS-A).
      - id: ICXZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column XZ Index (2 BCS-A).
      - id: ICYY
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column Y^2 Index (2 BCS-A).
      - id: ICYZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column YZ Index (2 BCS-A).
      - id: ICZZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Image Column Z^2 Index (2 BCS-A).
      - id: GXO
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground X Constant Index (2 BCS-A).
      - id: GYO
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Y Constant Index (2 BCS-A).
      - id: GZO
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Z Constant Index (2 BCS-A).
      - id: GXR
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Rotation X Index (2 BCS-A).
      - id: GYR
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Rotation Y Index (2 BCS-A).
      - id: GZR
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Rotation Z Index (2 BCS-A).
      - id: GS
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Scale Index (2 BCS-A).
      - id: GXX
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground X Adjustment Proportional to X Index (2 BCS-A).
      - id: GXY
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground X Adjustment Proportional to Y Index (2 BCS-A).
      - id: GXZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground X Adjustment Proportional to Z Index (2 BCS-A).
      - id: GYX
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Y Adjustment Proportional to X Index (2 BCS-A).
      - id: GYY
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Y Adjustment Proportional to Y Index (2 BCS-A).
      - id: GYZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Y Adjustment Proportional to Z Index (2 BCS-A).
      - id: GZX
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Z Adjustment Proportional to X Index (2 BCS-A).
      - id: GZY
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Z Adjustment Proportional to Y Index (2 BCS-A).
      - id: GZZ
        type: str
        size: 2
        encoding: BCS-A
        doc: Ground Z Adjustment Proportional to Z Index (2 BCS-A).

      - id: SUBGROUPS
        type: error_cov_subgroup
        repeat: expr
        repeat-expr: IGN.to_i
        doc: |
          Original image error covariance, one independent error subgroup per
          entry (IGN entries).
      - id: MAP
        type: str
        size: 21
        encoding: BCS-A
        repeat: expr
        repeat-expr: NPAR.to_i * NPARO.to_i
        doc: |
          Mapping Matrix Elements
          21 BCS-A real, row major order. The mapping matrix has NPAR rows and
          NPARO columns (NPAR*NPARO entries).

  error_cov_subgroup:
    seq:
      - id: NUMOPG
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Original Adjustable Parameters in Subgroup
          2 BCS-N integer, range 01-36. Sum of NUMOPG over all IGN entries
          equals NPARO.
      - id: ERRCVG
        type: str
        size: 21
        encoding: BCS-A
        repeat: expr
        repeat-expr: (NUMOPG.to_i * (NUMOPG.to_i + 1)) / 2
        doc: |
          Original Error Covariance Elements
          21 BCS-A real, upper triangular portion in row major order
          ((1/2)(NUMOPG+1)(NUMOPG) entries).
      - id: TCDF
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Time Correlation Domain Flag
          1 BCS-N. 0=all time intervals, 1=between images only, 2=within an
          image only.
      - id: NCSEG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Number of Correlation Segments
          1 BCS-N integer, range 2-9.
      - id: SEGMENTS
        type: correlation_segment
        repeat: expr
        repeat-expr: NCSEG.to_i
        doc: Correlation segments (NCSEG entries).

  correlation_segment:
    seq:
      - id: CORSEG
        type: str
        size: 21
        encoding: BCS-A
        doc: Segment Correlation Value (21 BCS-A real, 0 to 1).
      - id: TAUSEG
        type: str
        size: 21
        encoding: BCS-A
        doc: Segment Tau Value (21 BCS-A real, seconds).

  unmodeled_error_t:
    seq:
      - id: URR
        type: str
        size: 21
        encoding: BCS-A
        doc: Unmodeled Row Variance (21 BCS-A real, pixels^2).
      - id: URC
        type: str
        size: 21
        encoding: BCS-A
        doc: Unmodeled Row-Column Covariance (21 BCS-A real, pixels^2).
      - id: UCC
        type: str
        size: 21
        encoding: BCS-A
        doc: Unmodeled Column Variance (21 BCS-A real, pixels^2).
      - id: UNCSR
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Number of Correlation Segments for independent variable ROW distance
          1 BCS-N integer, range 2-9.
      - id: ROW_SEGMENTS
        type: unmod_row_segment
        repeat: expr
        repeat-expr: UNCSR.to_i
        doc: Row correlation segments (UNCSR entries).
      - id: UNCSC
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Number of Correlation Segments for independent variable Column distance
          1 BCS-N integer, range 2-9.
      - id: COL_SEGMENTS
        type: unmod_col_segment
        repeat: expr
        repeat-expr: UNCSC.to_i
        doc: Column correlation segments (UNCSC entries).

  unmod_row_segment:
    seq:
      - id: UCORSR
        type: str
        size: 21
        encoding: BCS-A
        doc: Segment Correlation Value, row distance (21 BCS-A real, 0 to 1).
      - id: UTAUSR
        type: str
        size: 21
        encoding: BCS-A
        doc: Segment Tau Value, row distance (21 BCS-A real, pixels).

  unmod_col_segment:
    seq:
      - id: UCORSC
        type: str
        size: 21
        encoding: BCS-A
        doc: Segment Correlation Value, column distance (21 BCS-A real, 0 to 1).
      - id: UTAUSC
        type: str
        size: 21
        encoding: BCS-A
        doc: Segment Tau Value, column distance (21 BCS-A real, pixels).
