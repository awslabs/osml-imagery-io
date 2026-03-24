meta:
  id: tre_rsmdcb
  title: RSM Direct Covariance Version B TRE
  endian: be

doc: |
  RSMDCB TRE - Replacement Sensor Model Direct Covariance Version B
  
  Provides direct covariance data for RSM error propagation.
  Contains image identifiers, cross-covariance matrices, and
  optional adjustable parameter definitions.
  
  Reference: STDI-0002 Volume 1, Appendix U, Section 9.6, Table 6

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

  - id: NROWCB
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Rows in Cross-Covariance Block
      2 BCS-N integer.

  - id: NIMGE
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Images
      3 BCS-N integer.

  - id: IMAGE_RECORDS
    type: image_record
    repeat: expr
    repeat-expr: NIMGE.to_i
    doc: Image identification records.

  - id: INCAPD
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Adjustable Parameter Data
      1 BCS-A. Y=included, N=not included.

  - id: AP_DATA
    type: adjustable_param_data
    if: INCAPD == "Y"
    doc: Adjustable parameter data (only when INCAPD=Y).

types:
  image_record:
    seq:
      - id: IIDI
        type: str
        size: 80
        encoding: BCS-A
        doc: Image Identifier (80 BCS-A).
      - id: NCOLCB
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Columns in Cross-Covariance Block (2 BCS-N, 1-36).

  adjustable_param_data:
    seq:
      - id: NPAR
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Adjustable Parameters (2 BCS-N, 1-36).
      - id: APTYP
        type: str
        size: 1
        encoding: BCS-A
        doc: Adjustable Parameter Type (1 BCS-A, I or G).
      - id: LOCTYP
        type: str
        size: 1
        encoding: BCS-A
        doc: Location Type (1 BCS-A, R or G).
      - id: NSFX
        type: str
        size: 21
        encoding: BCS-N
        doc: Normalization Scale Factor X (21 BCS-N real).
      - id: NSFY
        type: str
        size: 21
        encoding: BCS-N
        doc: Normalization Scale Factor Y (21 BCS-N real).
      - id: NSFZ
        type: str
        size: 21
        encoding: BCS-N
        doc: Normalization Scale Factor Z (21 BCS-N real).
      - id: NOFFX
        type: str
        size: 21
        encoding: BCS-N
        doc: Normalization Offset X (21 BCS-N real).
      - id: NOFFY
        type: str
        size: 21
        encoding: BCS-N
        doc: Normalization Offset Y (21 BCS-N real).
      - id: NOFFZ
        type: str
        size: 21
        encoding: BCS-N
        doc: Normalization Offset Z (21 BCS-N real).
      - id: APBASE
        type: str
        size: 1
        encoding: BCS-A
        doc: Adjustable Parameter Basis (1 BCS-A, Y or N).
