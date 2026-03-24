meta:
  id: tre_mensrb
  title: Mensuration Support Data TRE
  endian: be

doc: |
  MENSRB TRE - Mensuration Support Data
  
  Provides mensuration support data for imagery products including
  aircraft and reference point locations, range and azimuth offsets,
  direction cosines, and tile information.
  
  Reference: STDI-0002 Volume 1, Appendix E, Section E.3.7.2, Table E-14

seq:
  - id: ACFT_LOC
    type: str
    size: 25
    encoding: BCS-A
    doc: |
      Aircraft Location
      25 BCS-A.

  - id: ACFT_LOC_ACCY
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Aircraft Location Accuracy
      6 BCS-N real.

  - id: ACFT_ALT
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Aircraft Altitude
      6 BCS-N integer.

  - id: RP_LOC
    type: str
    size: 25
    encoding: BCS-A
    doc: |
      Reference Point Location
      25 BCS-A.

  - id: RP_LOC_ACCY
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Reference Point Location Accuracy
      6 BCS-N real.

  - id: RP_ELV
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Reference Point Elevation
      6 BCS-N integer, range -1000 to 30000.

  - id: OF_PC_R
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Offset from Patch Center in Range
      7 BCS-N real.

  - id: OF_PC_A
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Offset from Patch Center in Azimuth
      7 BCS-N real.

  - id: COSGRZ
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Cosine of Grazing Angle
      7 BCS-N real, range 0.0 to 1.0.

  - id: RGCRP
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Range to CRP
      7 BCS-N integer, range 0 to 3000000.

  - id: RLMAP
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Range/Line Map
      1 BCS-A.

  - id: RP_ROW
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Reference Point Row
      5 BCS-N integer, range 1-99999.

  - id: RP_COL
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Reference Point Column
      5 BCS-N integer, range 1-99999.

  - id: C_R_NC
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Range Direction Cosine - North Component
      10 BCS-N real, range -1.0 to 1.0.

  - id: C_R_EC
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Range Direction Cosine - East Component
      10 BCS-N real, range -1.0 to 1.0.

  - id: C_R_DC
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Range Direction Cosine - Down Component
      10 BCS-N real, range -1.0 to 1.0.

  - id: C_AZ_NC
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Azimuth Direction Cosine - North Component
      9 BCS-N real, range -1.0 to 1.0.

  - id: C_AZ_EC
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Azimuth Direction Cosine - East Component
      9 BCS-N real, range -1.0 to 1.0.

  - id: C_AZ_DC
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Azimuth Direction Cosine - Down Component
      9 BCS-N real, range -1.0 to 1.0.

  - id: C_AL_NC
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Along-Track Direction Cosine - North Component
      9 BCS-N real, range -1.0 to 1.0.

  - id: C_AL_EC
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Along-Track Direction Cosine - East Component
      9 BCS-N real, range -1.0 to 1.0.

  - id: C_AL_DC
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Along-Track Direction Cosine - Down Component
      9 BCS-N real, range -1.0 to 1.0.

  - id: TOTAL_TILES_COLS
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Total Tiles Columns
      3 BCS-N integer, range 1-999.

  - id: TOTAL_TILES_ROWS
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Total Tiles Rows
      5 BCS-N integer, range 1-99999.
