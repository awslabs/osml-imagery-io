meta:
  id: tre_rsmecb
  title: RSM Error Covariance Version B TRE
  endian: be

doc: |
  RSMECB TRE - Replacement Sensor Model Error Covariance Version B

  Provides indirect error covariance data for RSM error propagation.
  When INCLIC=Y the original-covariance block carries the adjustable
  parameter identification (Local rectangular coordinate system, image or
  ground adjustable parameters, optional basis matrix), the per-independent
  error subgroup covariance data, and the mapping matrix. When INCLUC=Y the
  unmodeled error covariance block is present.

  Reference: STDI-0002 Volume 1, Appendix U, Section 13.7, Table 10
  (pp. U-187 to U-203).

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
      40 BCS-A.

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

  - id: ORIG_COV
    type: original_covariance
    if: INCLIC == "Y"
    doc: Original/indirect error covariance data (only when INCLIC=Y).

  - id: UNMOD_COV
    type: unmodeled_covariance
    if: INCLUC == "Y"
    doc: Unmodeled error covariance data (only when INCLUC=Y).

types:
  original_covariance:
    seq:
      - id: NPARO
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Original Adjustable Parameters
          2 BCS-N integer, range 01-53. Both row and column dimension of the
          unmapped original image error covariance.
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
      - id: NPAR
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Active RSM Adjustable Parameters
          2 BCS-N integer, range 01-36. Row dimension of any RSM cross-
          covariance block and row dimension of the mapping matrix (MAP).
      - id: APTYP
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Adjustable Parameter Type
          1 BCS-A. I=image-space, G=ground-space.
      - id: LOCTYP
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Local Coordinate System Identifier
          1 BCS-A. R=rectangular ground coordinates, N=non-rectangular. If
          APTYP=G the only valid value is R.
      - id: NSFX
        type: str
        size: 21
        encoding: BCS-A
        doc: Normalization Scale Factor for X (21 BCS-A real).
      - id: NSFY
        type: str
        size: 21
        encoding: BCS-A
        doc: Normalization Scale Factor for Y (21 BCS-A real).
      - id: NSFZ
        type: str
        size: 21
        encoding: BCS-A
        doc: Normalization Scale Factor for Z (21 BCS-A real).
      - id: NOFFX
        type: str
        size: 21
        encoding: BCS-A
        doc: Normalization Offset for X (21 BCS-A real).
      - id: NOFFY
        type: str
        size: 21
        encoding: BCS-A
        doc: Normalization Offset for Y (21 BCS-A real).
      - id: NOFFZ
        type: str
        size: 21
        encoding: BCS-A
        doc: Normalization Offset for Z (21 BCS-A real).
      - id: LOCAL_COORD
        type: local_coordinate_system
        if: LOCTYP == "R"
        doc: |
          Local rectangular ground coordinate system definition
          (only when LOCTYP=R). Twelve 21 BCS-A values.
      - id: APBASE
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Adjustable Parameter Basis Option
          1 BCS-A. Y=basis option on, N=off.
      - id: IMAGE_AP
        type: image_adjustable_params
        if: APTYP == "I"
        doc: Image-space adjustable parameters (only when APTYP=I).
      - id: GROUND_AP
        type: ground_adjustable_params
        if: APTYP == "G"
        doc: Ground-space adjustable parameters (only when APTYP=G).
      - id: NBASIS
        type: str
        size: 2
        encoding: BCS-N
        if: APBASE == "Y"
        doc: |
          Number of Basis Adjustable Parameters
          2 BCS-N integer, range 1-99 (only when APBASE=Y). Columns of matrix
          A; NBASIS >= NPAR and NPAR*NBASIS <= 1296.
      - id: AEL
        type: str
        size: 21
        encoding: BCS-A
        repeat: expr
        repeat-expr: NPAR.to_i * NBASIS.to_i
        if: APBASE == "Y"
        doc: |
          Matrix A Elements
          21 BCS-A real, row major order (NPAR*NBASIS entries, only when
          APBASE=Y).
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
          2 BCS-N integer, range 01-53. Sum of NUMOPG over all IGN entries
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
      - id: ACSMC
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          CSM Correlation Option
          1 BCS-A. Y=CSM correlation functional form, N=piece-wise linear
          correlation segments.
      - id: SEGMENT_DATA
        type: piecewise_correlation
        if: ACSMC == "N"
        doc: Piece-wise linear correlation segments (only when ACSMC=N).
      - id: CSM_DATA
        type: csm_correlation
        if: ACSMC == "Y"
        doc: CSM correlation function parameters (only when ACSMC=Y).

  piecewise_correlation:
    seq:
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

  csm_correlation:
    seq:
      - id: AC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation function A parameter (21 BCS-A real).
      - id: ALPC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation function alpha parameter (21 BCS-A real).
      - id: BETC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation function beta parameter (21 BCS-A real).
      - id: TC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation function T parameter (21 BCS-A real).

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

  local_coordinate_system:
    seq:
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

  image_adjustable_params:
    seq:
      - id: NISAP
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Number of Image-Space Adjustable Parameters
          2 BCS-A integer (1-36 if APBASE=N, 1-99 if APBASE=Y).
      - id: NISAPR
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Number of Image-Space Adjustable Parameters for Image Row Coordinate
          2 BCS-A integer (0-36 if APBASE=N, 0-99 if APBASE=Y).
      - id: ROW_POWERS
        type: image_row_power
        repeat: expr
        repeat-expr: NISAPR.to_i
        doc: Row-adjustment power terms (NISAPR entries).
      - id: NISAPC
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Number of Image-Space Adjustable Parameters for Image Column Coordinate
          2 BCS-A integer (0-36 if APBASE=N, 0-99 if APBASE=Y).
      - id: COL_POWERS
        type: image_col_power
        repeat: expr
        repeat-expr: NISAPC.to_i
        doc: Column-adjustment power terms (NISAPC entries).

  image_row_power:
    seq:
      - id: XPWRR
        type: str
        size: 1
        encoding: BCS-A
        doc: Row Parameter Power of X (1 BCS-A, 0-5).
      - id: YPWRR
        type: str
        size: 1
        encoding: BCS-A
        doc: Row Parameter Power of Y (1 BCS-A, 0-5).
      - id: ZPWRR
        type: str
        size: 1
        encoding: BCS-A
        doc: Row Parameter Power of Z (1 BCS-A, 0-5).

  image_col_power:
    seq:
      - id: XPWRC
        type: str
        size: 1
        encoding: BCS-A
        doc: Column Parameter Power of X (1 BCS-A, 0-5).
      - id: YPWRC
        type: str
        size: 1
        encoding: BCS-A
        doc: Column Parameter Power of Y (1 BCS-A, 0-5).
      - id: ZPWRC
        type: str
        size: 1
        encoding: BCS-A
        doc: Column Parameter Power of Z (1 BCS-A, 0-5).

  ground_adjustable_params:
    seq:
      - id: NGSAP
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Number of Ground-Space Adjustable Parameters
          2 BCS-A integer, range 1-16.
      - id: GSAP_IDS
        type: str
        size: 4
        encoding: BCS-A
        repeat: expr
        repeat-expr: NGSAP.to_i
        doc: |
          Ground-Space Adjustable Parameter IDs (4 BCS-A each, NGSAP entries).
          One of OFFX, OFFY, OFFZ, ROTX, ROTY, ROTZ, SCAL, XRTX, XRTY, XRTZ,
          YRTX, YRTY, YRTZ, ZRTX, ZRTY, ZRTZ.

  unmodeled_covariance:
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
      - id: UACSMC
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Unmodeled CSM Correlation Option
          1 BCS-A. Y=CSM correlation functional form, N=piece-wise linear
          correlation segments.
      - id: SEGMENT_DATA
        type: unmod_segment_data
        if: UACSMC == "N"
        doc: Piece-wise segment data (only when UACSMC=N).
      - id: CSM_DATA
        type: unmod_csm_data
        if: UACSMC == "Y"
        doc: CSM correlation model data (only when UACSMC=Y).

  unmod_segment_data:
    seq:
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

  unmod_csm_data:
    seq:
      - id: UACR
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation A parameter, row distance (21 BCS-A real).
      - id: UALPCR
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation alpha parameter, row distance (21 BCS-A real).
      - id: UBETCR
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation beta parameter, row distance (21 BCS-A real).
      - id: UTCR
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation T parameter, row distance (21 BCS-A real).
      - id: UACC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation A parameter, column distance (21 BCS-A real).
      - id: UALPCC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation alpha parameter, column distance (21 BCS-A real).
      - id: UBETCC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation beta parameter, column distance (21 BCS-A real).
      - id: UTCC
        type: str
        size: 21
        encoding: BCS-A
        doc: CSM correlation T parameter, column distance (21 BCS-A real).
