meta:
  id: tre_rsmdcb
  title: RSM Direct Covariance Version B TRE
  endian: be

doc: |
  RSMDCB TRE - Replacement Sensor Model Direct Error Covariance Version B

  Provides direct error covariance data for RSM error propagation.
  Contains image identifiers, an optional adjustable-parameter definition
  block (including the Local rectangular coordinate system, image/ground
  adjustable-parameter identification, and the optional basis matrix), and
  the cross-covariance (CRSCOV) payload that is the core data of the TRE.

  Reference: STDI-0002 Volume 1, Appendix U, Section 9.6, Table 6
  (pp. U-93 to U-105).

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

  - id: NROWCB
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Rows per Cross-Covariance Block
      2 BCS-N integer, range 01-36. NROWCB equals the number of active
      adjustable parameters (NROWCB=NPAR).

  - id: NIMGE
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Images
      3 BCS-N integer, range 001-999. Each image corresponds to the column
      dimension of a cross-covariance block and to one CRSCOV block.

  - id: IMAGE_RECORDS
    type: image_record
    repeat: expr
    repeat-expr: NIMGE.to_i
    doc: Image identification records (NIMGE entries).

  - id: INCAPD
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Adjustable Parameter Definitions Flag
      1 BCS-A. Y=adjustable parameters are identified/defined below, N=not.

  - id: AP_DATA
    type: adjustable_param_data
    if: INCAPD == "Y"
    doc: Adjustable parameter identification block (only when INCAPD=Y).

  - id: CRSCOV_BLOCKS
    type: crscov_block(_index)
    repeat: expr
    repeat-expr: NIMGE.to_i
    doc: |
      Cross-Covariance Elements (Direct Error Covariance).
      One per-image cross-covariance block per entry (NIMGE entries, in the
      order the images are listed in IIDI). Each block holds
      NROWCB*NCOLCB[image] elements of 21 BCS-A real in row major order, so the
      total element count is NROWCB * sum(NCOLCB over all NIMGE images).

      The per-image block boundaries are modeled directly: the block's element
      count reads NCOLCB from the corresponding IMAGE_RECORDS entry, selected by
      the per-image loop index bound into the nested type as `image_index`
      (`IMAGE_RECORDS[image_index].NCOLCB`). This replaces the earlier opaque
      end-of-stream blob, which relied on CRSCOV sitting at the very end of the
      record and lost the per-image structure on decode.

types:
  crscov_block:
    params:
      - id: image_index
        type: s4
        doc: |
          Zero-based index of the image this block belongs to, bound from the
          enclosing per-image loop (`crscov_block(_index)`). Selects this
          block's column count from the matching IMAGE_RECORDS entry.
    seq:
      - id: CRSCOV
        type: str
        size: 21
        encoding: BCS-A
        repeat: expr
        repeat-expr: NROWCB.to_i * IMAGE_RECORDS[image_index].NCOLCB.to_i
        doc: |
          Cross-Covariance Elements for one image block.
          NROWCB*NCOLCB entries of 21 BCS-A real in row major order, where
          NROWCB is the top-level row count and NCOLCB is this image's column
          count (IMAGE_RECORDS[image_index].NCOLCB).

  image_record:
    seq:
      - id: IIDI
        type: str
        size: 80
        encoding: BCS-A
        doc: Image Identifier for this cross-covariance block (80 BCS-A).
      - id: NCOLCB
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Columns per Cross-Covariance Block for image IIDI
          2 BCS-N integer, range 01-36. Equals the number of active
          adjustable parameters for image IIDI. If IIDI=IID this is the
          (auto-)covariance block and NCOLCB=NROWCB.

  adjustable_param_data:
    seq:
      - id: NPAR
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Number of Active RSM Adjustable Parameters
          2 BCS-N integer, range 01-36. Row dimension of the cross-covariance
          block. If APBASE=Y, NPAR is the number of active parameters (rows of
          matrix A).
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
          1 BCS-A. R=rectangular ground coordinates, N=non-rectangular
          (image row/column, geodetic height). If APTYP=G the only valid
          value is R.
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
          (only when LOCTYP=R). Twelve 21 BCS-A values: origin and
          orthogonal unit vectors.
      - id: APBASE
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Adjustable Parameter Basis Option
          1 BCS-A. Y=basis option on (active parameters are a linear
          combination of a basis set via matrix A), N=off.
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
          2 BCS-N integer, range 1-99 (only when APBASE=Y). Number of columns
          of matrix A; NBASIS >= NPAR and NPAR*NBASIS <= 1296.
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
