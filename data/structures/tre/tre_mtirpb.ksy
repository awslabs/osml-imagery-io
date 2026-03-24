meta:
  id: tre_mtirpb
  title: Moving Target Indicator Report TRE
  endian: be

doc: |
  MTIRPB TRE - Moving Target Indicator Report
  
  Provides moving target indicator (MTI) report data including
  aircraft position, sensor parameters, and detected target
  information with location, velocity, and classification.
  
  Reference: STDI-0002 Volume 1, Appendix E, Section E.3.10.2, Table E-19

seq:
  - id: MTI_DP
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      MTI Data Pedigree
      2 BCS-A.

  - id: MTI_PACKET_ID
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      MTI Packet ID
      3 BCS-N integer, range 1-999.

  - id: PATCH_NO
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Patch Number
      4 BCS-N integer, range 1-999.

  - id: WAMTI_FRAME_NO
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      WAMTI Frame Number
      5 BCS-A.

  - id: WAMTI_BAR_NO
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      WAMTI Bar Number
      1 BCS-A.

  - id: DATIME
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Date/Time
      14 BCS-A.

  - id: ACFT_LOC
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Aircraft Location
      21 BCS-A.

  - id: ACFT_ALT
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Aircraft Altitude
      6 BCS-N integer, range 0-999999.

  - id: ACFT_ALT_UNIT
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Aircraft Altitude Unit
      1 BCS-A.

  - id: ACFT_HEADING
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Aircraft Heading
      3 BCS-N integer, range 0-359.

  - id: MTI_LR
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      MTI Left/Right
      1 BCS-A.

  - id: SQUINT_ANGLE
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Squint Angle
      6 BCS-N real, range -60.0 to 85.00.

  - id: COSGRZ
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Cosine of Grazing Angle
      7 BCS-N real, range 0 to 9.99999.

  - id: NO_VALID_TARGETS
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Valid Targets
      3 BCS-N integer, range 1-999.

  - id: TARGETS
    type: target_record
    repeat: expr
    repeat-expr: NO_VALID_TARGETS.to_i
    doc: Target records.

types:
  target_record:
    seq:
      - id: TGT_LOC
        type: str
        size: 23
        encoding: BCS-A
        doc: |
          Target Location
          23 BCS-A.

      - id: TGT_LOC_ACCY
        type: str
        size: 6
        encoding: BCS-N
        doc: |
          Target Location Accuracy
          6 BCS-N real, range 0 to 999.99.

      - id: TGT_VEL_R
        type: str
        size: 4
        encoding: BCS-A
        doc: |
          Target Radial Velocity
          4 BCS-A, range -200 to 200.

      - id: TGT_SPEED
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Target Speed
          3 BCS-A, range 0-200.

      - id: TGT_HEADING
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Target Heading
          3 BCS-A, range 0-359.

      - id: TGT_AMPLITUDE
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Target Amplitude
          2 BCS-A, range 0-15.

      - id: TGT_CAT
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Target Category
          1 BCS-A.
