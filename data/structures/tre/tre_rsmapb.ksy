meta:
  id: tre_rsmapb
  title: RSM Adjustable Parameters Version B TRE
  endian: be

doc: |
  RSMAPB TRE - Replacement Sensor Model Adjustable Parameters Version B
  
  Provides adjustable parameter values for RSM error propagation.
  Version B extends Version A with additional fields for adjustable
  parameter type, location type, normalization, and optional basis
  matrix support.
  
  Reference: STDI-0002 Volume 1, Appendix U, Section 11.4, Table 8

seq:
  - id: IID
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A.

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

  - id: NPAR
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Adjustable Parameters
      2 BCS-N integer, range 1-36.

  - id: APTS
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Adjustable Parameter Type
      1 BCS-A. I=Image, G=Ground.

  - id: LOCTYP
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Location Type
      1 BCS-A. R=Rectangular, G=Geodetic.

  - id: NSFX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Normalization Scale Factor X
      21 BCS-N real.

  - id: NSFY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Normalization Scale Factor Y
      21 BCS-N real.

  - id: NSFZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Normalization Scale Factor Z
      21 BCS-N real.

  - id: NOFFX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Normalization Offset X
      21 BCS-N real.

  - id: NOFFY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Normalization Offset Y
      21 BCS-N real.

  - id: NOFFZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Normalization Offset Z
      21 BCS-N real.

  - id: LOCAL_COORD
    type: local_coordinate_system
    if: LOCTYP == "R"
    doc: Local coordinate system (only when LOCTYP=R).

  - id: APBASE
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Adjustable Parameter Basis
      1 BCS-A. Y=basis provided, N=no basis.

  - id: IMAGE_AP
    type: image_adjustable_params
    if: APTS == "I"
    doc: Image-space adjustable parameters (only when APTYP=I).

  - id: GROUND_AP
    type: ground_adjustable_params
    if: APTS == "G"
    doc: Ground-space adjustable parameters (only when APTYP=G).

  - id: BASIS_DATA
    type: basis_matrix
    if: APBASE == "Y"
    doc: Basis matrix (only when APBASE=Y).

  - id: PARVAL
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: NPAR.to_i
    doc: |
      Adjustable Parameter Values
      NPAR values, each 21 BCS-N real.

types:
  local_coordinate_system:
    seq:
      - id: XUOL
        type: str
        size: 21
        encoding: BCS-N
        doc: X Coordinate of Local Origin (21 BCS-N real).
      - id: YUOL
        type: str
        size: 21
        encoding: BCS-N
        doc: Y Coordinate of Local Origin (21 BCS-N real).
      - id: ZUOL
        type: str
        size: 21
        encoding: BCS-N
        doc: Z Coordinate of Local Origin (21 BCS-N real).
      - id: XUXL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector X for Local X Axis (21 BCS-N real, -1 to 1).
      - id: XUYL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector Y for Local X Axis (21 BCS-N real, -1 to 1).
      - id: XUZL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector Z for Local X Axis (21 BCS-N real, -1 to 1).
      - id: YUXL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector X for Local Y Axis (21 BCS-N real, -1 to 1).
      - id: YUYL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector Y for Local Y Axis (21 BCS-N real, -1 to 1).
      - id: YUZL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector Z for Local Y Axis (21 BCS-N real, -1 to 1).
      - id: ZUXL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector X for Local Z Axis (21 BCS-N real, -1 to 1).
      - id: ZUYL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector Y for Local Z Axis (21 BCS-N real, -1 to 1).
      - id: ZUZL
        type: str
        size: 21
        encoding: BCS-N
        doc: Unit Vector Z for Local Z Axis (21 BCS-N real, -1 to 1).

  image_adjustable_params:
    seq:
      - id: NISAP
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Image-Space Adjustable Parameters (2 BCS-N, 1-99).
      - id: NISAPR
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Row Power Terms (2 BCS-N, 0-99).
      - id: ROW_POWERS
        type: power_term
        repeat: expr
        repeat-expr: NISAPR.to_i
        doc: Row power terms.
      - id: NISAPC
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Column Power Terms (2 BCS-N, 0-99).
      - id: COL_POWERS
        type: power_term
        repeat: expr
        repeat-expr: NISAPC.to_i
        doc: Column power terms.

  ground_adjustable_params:
    seq:
      - id: NGSAP
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Ground-Space Adjustable Parameters (2 BCS-N, 1-16).
      - id: GSAP_IDS
        type: str
        size: 4
        encoding: BCS-A
        repeat: expr
        repeat-expr: NGSAP.to_i
        doc: Ground-Space Adjustable Parameter IDs (4 BCS-A each).

  power_term:
    seq:
      - id: XPWR
        type: str
        size: 1
        encoding: BCS-N
        doc: X Power (1 BCS-N, 0-5).
      - id: YPWR
        type: str
        size: 1
        encoding: BCS-N
        doc: Y Power (1 BCS-N, 0-5).
      - id: ZPWR
        type: str
        size: 1
        encoding: BCS-N
        doc: Z Power (1 BCS-N, 0-5).

  basis_matrix:
    seq:
      - id: NBASIS
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Basis Vectors (2 BCS-N, 1-99).
