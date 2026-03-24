meta:
  id: tre_rsmecb
  title: RSM Error Covariance Version B TRE
  endian: be

doc: |
  RSMECB TRE - Replacement Sensor Model Error Covariance Version B
  
  Provides error covariance data for RSM error propagation.
  Contains original covariance, unmodeled error covariance,
  and adjustable parameter definitions with correlation segments.
  
  Reference: STDI-0002 Volume 1, Appendix U, Section 13.7, Table 10

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

  - id: INCLIC
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Original Covariance
      1 BCS-A. Y=included, N=not included.

  - id: INCLUC
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Include Unmodeled Error Covariance
      1 BCS-A. Y=included, N=not included.

  - id: ORIG_COV
    type: original_covariance
    if: INCLIC == "Y"
    doc: Original covariance data (only when INCLIC=Y).

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
        doc: Number of Original Parameters (2 BCS-N, 1-53).
      - id: IGN
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Independent Groups (2 BCS-N, 1-36).
      - id: CVDATE
        type: str
        size: 8
        encoding: BCS-A
        doc: Covariance Date (8 BCS-A).
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

  unmodeled_covariance:
    seq:
      - id: URR
        type: str
        size: 21
        encoding: BCS-N
        doc: Unmodeled Row-Row Covariance (21 BCS-N real).
      - id: URC
        type: str
        size: 21
        encoding: BCS-N
        doc: Unmodeled Row-Column Covariance (21 BCS-N real).
      - id: UCC
        type: str
        size: 21
        encoding: BCS-N
        doc: Unmodeled Column-Column Covariance (21 BCS-N real).
      - id: UACSMC
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Unmodeled Auto-Correlation Segment Model Choice
          1 BCS-A. Y=auto-correlation model, N=piecewise segments.
      - id: SEGMENT_DATA
        type: unmod_segment_data
        if: UACSMC == "N"
        doc: Piecewise segment data (only when UACSMC=N).
      - id: AUTOCORR_DATA
        type: unmod_autocorr_data
        if: UACSMC == "Y"
        doc: Auto-correlation model data (only when UACSMC=Y).

  unmod_segment_data:
    seq:
      - id: UNCSR
        type: str
        size: 1
        encoding: BCS-N
        doc: Number of Row Correlation Segments (1 BCS-N, 2-9).
      - id: ROW_SEGMENTS
        type: correlation_segment
        repeat: expr
        repeat-expr: UNCSR.to_i
        doc: Row correlation segments.
      - id: UNCSC
        type: str
        size: 1
        encoding: BCS-N
        doc: Number of Column Correlation Segments (1 BCS-N, 2-9).
      - id: COL_SEGMENTS
        type: correlation_segment
        repeat: expr
        repeat-expr: UNCSC.to_i
        doc: Column correlation segments.

  unmod_autocorr_data:
    seq:
      - id: UACR
        type: str
        size: 21
        encoding: BCS-N
        doc: Row Auto-Correlation A (21 BCS-N real, 0-1).
      - id: UALPCR
        type: str
        size: 21
        encoding: BCS-N
        doc: Row Auto-Correlation Alpha (21 BCS-N real, 0-1).
      - id: UBETCR
        type: str
        size: 21
        encoding: BCS-N
        doc: Row Auto-Correlation Beta (21 BCS-N real, 0-10).
      - id: UTCR
        type: str
        size: 21
        encoding: BCS-N
        doc: Row Auto-Correlation T (21 BCS-N real).
      - id: UACC
        type: str
        size: 21
        encoding: BCS-N
        doc: Column Auto-Correlation A (21 BCS-N real, 0-1).
      - id: UALPCC
        type: str
        size: 21
        encoding: BCS-N
        doc: Column Auto-Correlation Alpha (21 BCS-N real, 0-1).
      - id: UBETCC
        type: str
        size: 21
        encoding: BCS-N
        doc: Column Auto-Correlation Beta (21 BCS-N real, 0-10).
      - id: UTCC
        type: str
        size: 21
        encoding: BCS-N
        doc: Column Auto-Correlation T (21 BCS-N real).

  correlation_segment:
    seq:
      - id: UCOR
        type: str
        size: 21
        encoding: BCS-N
        doc: Correlation Value (21 BCS-N real, 0-1).
      - id: UTAU
        type: str
        size: 21
        encoding: BCS-N
        doc: Tau Value (21 BCS-N real).
